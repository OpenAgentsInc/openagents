// DeepSeek-V4.1 attention kernels (#12392): tail RoPE and YaRN, the cache
// quantizers, window and compressed latent attention with the sink, the
// compressor, compressed-row stores, the indexer, and top-k and candidate
// selection.
//
// Every kernel repeats the CPU path's operations (`deepseek_v41_cpu`, the
// `ReferenceBf16` arithmetic) in the CPU's order: dot products, matrix rows,
// and RMS sums are sequential sums from element 0, the attention keeps the
// CPU's 64-slot online softmax, and selection follows its total order with
// ties to the lower index. That costs some parallelism inside a dot product
// but keeps every kernel equal to the CPU wherever no transcendental is
// involved. Arithmetic uses explicit round-to-nearest intrinsics, so the
// compiler can't contract or reorder it.
//
// `exp`, `cos`, and `sin` run in double precision and are rounded to float
// once. The host models in `v41_attention.rs` do the same and reproduce the
// device bit for bit.
//
// Caches hold BF16 values as 16-bit words. Positions are device `int32`
// values, so a recorded CUDA graph can replay a decode step at any position.
// Nonfinite input writes NaN where the CPU path returns an error, and an
// invalid selection writes INT32_MIN, which makes the attention output NaN.
//
// Nothing here is architecture-specific: no tensor cores, FP8 conversion
// instructions, or Hopper-only features, so the same source builds for SM89
// and SM120. Build it without fast math: the decoders rely on float32
// subnormals and the kernels on IEEE division and square roots.

#include <cuda_runtime.h>
#include <math_constants.h>
#include <stdint.h>

namespace {

constexpr int kThreads = 256;
constexpr int kSelectThreads = 1024;
constexpr int kMaxPosition = 1048576;
constexpr int kPoison = INT32_MIN;
constexpr uint8_t kPoisonFlag = 0xffu;

// Quantization kinds, as `deepseek_v41_cpu::quantization::Quantization`.
constexpr int kQuantFp8 = 0;
constexpr int kQuantIndexFp4 = 1;
constexpr int kQuantCompressedFp4 = 2;

// Dense weight formats.
constexpr int kDenseF32 = 0;
constexpr int kDenseBf16 = 1;
constexpr int kDenseFp8Block = 2;

__device__ __forceinline__ float exp_once(float x) {
    return __double2float_rn(exp(static_cast<double>(x)));
}

__device__ __forceinline__ float cos_once(float x) {
    return __double2float_rn(cos(static_cast<double>(x)));
}

__device__ __forceinline__ float sin_once(float x) {
    return __double2float_rn(sin(static_cast<double>(x)));
}

// Round to the nearest BF16, ties to even, returned as float32. NaN stays a
// quiet NaN with its sign, as `half::bf16::from_f32` does.
__device__ __forceinline__ float bf16_round(float value) {
    const uint32_t bits = __float_as_uint(value);
    if ((bits & 0x7fffffffu) > 0x7f800000u) {
        return __uint_as_float((bits | 0x00400000u) & 0xffff0000u);
    }
    const uint32_t rounded = bits + 0x7fffu + ((bits >> 16) & 1u);
    return __uint_as_float(rounded & 0xffff0000u);
}

__device__ __forceinline__ uint16_t bf16_bits(float value) {
    return static_cast<uint16_t>(__float_as_uint(bf16_round(value)) >> 16);
}

__device__ __forceinline__ float bf16_value(uint16_t bits) {
    return __uint_as_float(static_cast<uint32_t>(bits) << 16);
}

__device__ __forceinline__ float e4m3(uint32_t byte) {
    const uint32_t magnitude = byte & 0x7fu;
    float value = __fmul_rn(__uint_as_float(magnitude << 20), 0x1p120f);
    if (magnitude == 0x7fu) {
        value = CUDART_NAN_F;
    }
    return __uint_as_float(__float_as_uint(value) | ((byte & 0x80u) << 24));
}

__device__ __forceinline__ float e2m1(uint32_t code) {
    const float magnitudes[8] = {0.0f, 0.5f, 1.0f, 1.5f, 2.0f, 3.0f, 4.0f, 6.0f};
    const float value = magnitudes[code & 7u];
    return (code & 8u) ? -value : value;
}

__device__ __forceinline__ float e8m0(uint32_t byte) {
    if (byte == 0xffu) {
        return CUDART_NAN_F;
    }
    return byte == 0 ? __uint_as_float(0x00400000u) : __uint_as_float(byte << 23);
}

// `quantization::nearest` over E4M3 codes 0 to 126: the first code whose
// value is at least x, then the closer of it and the code below, ties to the
// even code. x is finite and at most 448.
__device__ __forceinline__ uint32_t nearest_e4m3(float x) {
    int lo = 0;
    int hi = 127;
    while (lo < hi) {
        const int mid = (lo + hi) / 2;
        if (e4m3(static_cast<uint32_t>(mid)) < x) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    int upper = lo < 126 ? lo : 126;
    if (upper == 0) {
        return 0;
    }
    const int lower = upper - 1;
    const float a = __fsub_rn(x, e4m3(static_cast<uint32_t>(lower)));
    const float b = __fsub_rn(e4m3(static_cast<uint32_t>(upper)), x);
    return (a < b || (a == b && lower % 2 == 0)) ? static_cast<uint32_t>(lower)
                                                 : static_cast<uint32_t>(upper);
}

__device__ __forceinline__ uint32_t encode_fp8(float value) {
    const uint32_t sign = signbit(value) ? 0x80u : 0u;
    return nearest_e4m3(fminf(fabsf(value), 448.0f)) | sign;
}

__device__ __forceinline__ uint32_t encode_fp4(float value) {
    const float magnitudes[8] = {0.0f, 0.5f, 1.0f, 1.5f, 2.0f, 3.0f, 4.0f, 6.0f};
    const float x = fminf(fabsf(value), 6.0f);
    int upper = 0;
    while (upper < 7 && magnitudes[upper] < x) {
        ++upper;
    }
    uint32_t code = 0;
    if (upper != 0) {
        const int lower = upper - 1;
        const float a = __fsub_rn(x, magnitudes[lower]);
        const float b = __fsub_rn(magnitudes[upper], x);
        code = (a < b || (a == b && lower % 2 == 0)) ? static_cast<uint32_t>(lower)
                                                     : static_cast<uint32_t>(upper);
    }
    return code | (signbit(value) ? 8u : 0u);
}

// `ceil_power_scale`: the next power of two at or above `value`. Returns NaN
// where the CPU refuses (nonfinite, below the smallest normal, or overflow).
__device__ __forceinline__ float ceil_power(float value) {
    if (!isfinite(value) || value < 0x1p-126f) {
        return CUDART_NAN_F;
    }
    const uint32_t bits = __float_as_uint(value);
    const uint32_t exponent = ((bits >> 23) & 255u) + ((bits & 0x7fffffu) != 0u ? 1u : 0u);
    if (exponent >= 255u) {
        return CUDART_NAN_F;
    }
    return __uint_as_float(exponent << 23);
}

// One value of a quantization block, the block held one value per lane:
// `inplace_bf16` for blocks of `n` values (32, or 16 for the compressed KV)
// aligned within the warp. Every lane of the warp calls it. Rounds to BF16,
// takes the block's maximum magnitude (exact in any order), quantizes,
// dequantizes, and rounds again; a nonfinite input, a scale the CPU refuses,
// or an overflowing value gives the whole block NaN.
__device__ float quant_lane_parts(int kind, float in, int n, uint32_t* code, uint32_t* scale_byte);

__device__ float quant_lane(int kind, float in, int n) {
    return quant_lane_parts(kind, in, n, nullptr, nullptr);
}

// As `quant_lane`, also giving the value's code and its block's scale byte
// (#12471): an E4M3 byte for the compressed FP4 form, else a UE8M0 power of
// two. A NaN block gives the scale byte that decodes as NaN.
__device__ float quant_lane_parts(int kind, float in, int n, uint32_t* code, uint32_t* scale_byte) {
    const unsigned all = 0xffffffffu;
    const int lane = threadIdx.x & 31;
    const unsigned segment = n == 32 ? all : (0xffffu << (lane & 16));
    const float x = bf16_round(in);
    const bool finite = (__ballot_sync(all, !isfinite(x)) & segment) == 0u;
    float amax = fabsf(x);
    for (int offset = n / 2; offset > 0; offset /= 2) {
        amax = fmaxf(amax, __shfl_xor_sync(all, amax, offset, n));
    }
    float scale;
    if (kind == kQuantFp8) {
        scale = ceil_power(__fmul_rn(fmaxf(amax, 1e-4f), __fdiv_rn(1.0f, 448.0f)));
    } else if (kind == kQuantIndexFp4) {
        scale = ceil_power(__fmul_rn(fmaxf(amax, 6.0f * 0x1p-126f), __fdiv_rn(1.0f, 6.0f)));
    } else {
        const float value = __fdiv_rn(fmaxf(amax, 6.0f * 0x1p-9f), 6.0f);
        scale = value > 448.0f ? CUDART_NAN_F : e4m3(encode_fp8(value));
    }
    const float q = __fdiv_rn(x, scale);
    const uint32_t c = kind == kQuantFp8 ? encode_fp8(q) : encode_fp4(q);
    const float decoded = kind == kQuantFp8 ? e4m3(c) : e2m1(c);
    const float y = bf16_round(__fmul_rn(decoded, scale));
    // The CPU refuses a block whose dequantized values overflow.
    const bool overflow = (__ballot_sync(all, !isfinite(y)) & segment) != 0u;
    const bool bad = !finite || isnan(scale) || overflow;
    if (code != nullptr) {
        *code = c;
        if (kind == kQuantCompressedFp4) {
            *scale_byte = bad ? 0x7fu : encode_fp8(scale);
        } else {
            *scale_byte = bad ? 0xffu : (__float_as_uint(scale) >> 23) & 0xffu;
        }
    }
    return bad ? CUDART_NAN_F : y;
}

// Packed KV rows (#12471): codes, then one scale byte per block.
//   ring        FP8 E4M3 codes [width], UE8M0 per 32  -> width + width / 32
//   compressed  FP4 E2M1 pairs [width / 2], E4M3 per 16 -> width / 2 + width / 16
//   index key   FP4 E2M1 pairs [width / 2], UE8M0 per 32 -> width / 2 + width / 32
// A row reads back as the BF16 words the unpacked store writes.
__host__ __device__ __forceinline__ long long packed_row_bytes(int kind, int width) {
    if (kind == kQuantFp8) {
        return width + width / 32;
    }
    return width / 2 + width / (kind == kQuantCompressedFp4 ? 16 : 32);
}

__device__ __forceinline__ uint16_t packed_word(int kind, const uint8_t* row, int width, int i) {
    float value;
    if (kind == kQuantFp8) {
        const uint32_t s = row[width + i / 32];
        if (s == 0xffu) {
            return bf16_bits(CUDART_NAN_F);
        }
        value = __fmul_rn(e4m3(row[i]), __uint_as_float(s << 23));
    } else {
        const uint32_t byte = row[i / 2];
        const uint32_t c = i % 2 == 0 ? byte & 15u : byte >> 4;
        const uint32_t s = row[width / 2 + i / (kind == kQuantCompressedFp4 ? 16 : 32)];
        float scale;
        if (kind == kQuantCompressedFp4) {
            if (s == 0x7fu) {
                return bf16_bits(CUDART_NAN_F);
            }
            scale = e4m3(s);
        } else {
            if (s == 0xffu) {
                return bf16_bits(CUDART_NAN_F);
            }
            scale = __uint_as_float(s << 23);
        }
        value = __fmul_rn(e2m1(c), scale);
    }
    return bf16_bits(bf16_round(value));
}

// Stores one lane's quantized value into a packed row: the lanes of a warp
// hold consecutive values, so FP4 pairs combine through a shuffle and the
// first lane of each block writes its scale. Every lane of the warp calls it.
__device__ __forceinline__ void store_packed(
    int kind, uint8_t* row, int width, int i, uint32_t code, uint32_t scale_byte) {
    const int lane = threadIdx.x & 31;
    if (kind == kQuantFp8) {
        if (i < width) {
            row[i] = static_cast<uint8_t>(code);
            if (lane % 32 == 0) {
                row[width + i / 32] = static_cast<uint8_t>(scale_byte);
            }
        }
        return;
    }
    const uint32_t odd = __shfl_down_sync(0xffffffffu, code, 1);
    const int block = kind == kQuantCompressedFp4 ? 16 : 32;
    if (i < width) {
        if (i % 2 == 0) {
            row[i / 2] = static_cast<uint8_t>((code & 15u) | ((odd & 15u) << 4));
        }
        if (i % block == 0) {
            row[width / 2 + i / block] = static_cast<uint8_t>(scale_byte);
        }
    }
}

__host__ __device__ __forceinline__ int quant_block_width(int kind) {
    return kind == kQuantCompressedFp4 ? 16 : 32;
}

// Rotates one head's tail pairs and optionally rounds the whole head. Threads
// of the block share the head. Returns nothing; `out` may alias `in`.
__device__ void rope_head(
    const float* in,
    float* out,
    const float* frequencies,
    int rope_dim,
    int head_width,
    int position,
    bool inverse,
    bool round) {
    const int prefix = head_width - rope_dim;
    const bool valid = position >= 0 && position < kMaxPosition;
    for (int i = threadIdx.x; i < prefix; i += blockDim.x) {
        const float v = valid ? in[i] : CUDART_NAN_F;
        out[i] = round ? bf16_round(v) : v;
    }
    for (int p = threadIdx.x; p < rope_dim / 2; p += blockDim.x) {
        const float a = in[prefix + 2 * p];
        const float b = in[prefix + 2 * p + 1];
        float first = CUDART_NAN_F;
        float second = CUDART_NAN_F;
        if (valid) {
            const float angle = __fmul_rn(static_cast<float>(position), frequencies[p]);
            const float c = cos_once(angle);
            const float s = inverse ? -sin_once(angle) : sin_once(angle);
            first = __fsub_rn(__fmul_rn(a, c), __fmul_rn(b, s));
            second = __fadd_rn(__fmul_rn(a, s), __fmul_rn(b, c));
        }
        out[prefix + 2 * p] = round ? bf16_round(first) : first;
        out[prefix + 2 * p + 1] = round ? bf16_round(second) : second;
    }
}

// The CPU's `rms_norm` in place on a shared vector: a sequential sum of
// squares by thread 0, `1 / sqrt(mean + eps)`, then `bf16(w) * (x * scale)`,
// rounded to BF16 when asked. Ends with the block synchronized.
__device__ void rms_norm_shared(
    float* x,
    const float* weight,
    int width,
    float epsilon,
    bool round,
    float* scratch) {
    __syncthreads();
    if (threadIdx.x == 0) {
        float sum = -0.0f;
        int i = 0;
        for (; i + 16 <= width; i += 16) {
            float a[16];
#pragma unroll
            for (int k = 0; k < 16; ++k) {
                a[k] = x[i + k];
            }
#pragma unroll
            for (int k = 0; k < 16; ++k) {
                sum = __fadd_rn(sum, __fmul_rn(a[k], a[k]));
            }
        }
        for (; i < width; ++i) {
            sum = __fadd_rn(sum, __fmul_rn(x[i], x[i]));
        }
        const float variance = __fdiv_rn(sum, static_cast<float>(width));
        scratch[0] = isfinite(variance)
                         ? __fdiv_rn(1.0f, __fsqrt_rn(__fadd_rn(variance, epsilon)))
                         : CUDART_NAN_F;
    }
    __syncthreads();
    const float scale = scratch[0];
    for (int i = threadIdx.x; i < width; i += blockDim.x) {
        const float v = __fmul_rn(bf16_round(weight[i]), __fmul_rn(x[i], scale));
        x[i] = round ? bf16_round(v) : v;
    }
    __syncthreads();
}

// -------------------------------------------------------------------------
// RoPE
// -------------------------------------------------------------------------

__global__ void v41_rope_kernel(
    float* values,
    const float* frequencies,
    const int* positions,
    int heads,
    int head_width,
    int rope_dim,
    int back,
    int inverse,
    int round_bf16) {
    const int token = blockIdx.y;
    const int head = blockIdx.x;
    float* row = values + (static_cast<long long>(token) * heads + head) * head_width;
    const int position = positions[token] - back;
    rope_head(row, row, frequencies, rope_dim, head_width, position, inverse != 0, round_bf16 != 0);
}

// -------------------------------------------------------------------------
// Cache quantization and the window ring
// -------------------------------------------------------------------------

__global__ void v41_cache_quant_kernel(
    const float* input, float* out, int kind, long long total) {
    const long long e = static_cast<long long>(blockIdx.x) * blockDim.x + threadIdx.x;
    const float v = quant_lane(kind, e < total ? input[e] : 0.0f, quant_block_width(kind));
    if (e < total) {
        out[e] = v;
    }
}

__global__ void v41_window_store_kernel(
    const float* input,
    uint16_t* ring,
    const int* positions,
    int ring_rows,
    int width,
    int packed) {
    const int token = blockIdx.x;
    const int position = positions[token];
    if (position < 0) {
        return;
    }
    const long long slot = position % ring_rows;
    for (int base = 0; base < width; base += blockDim.x) {
        const int i = base + threadIdx.x;
        uint32_t code = 0;
        uint32_t scale = 0;
        const float v = quant_lane_parts(
            kQuantFp8, i < width ? input[static_cast<long long>(token) * width + i] : 0.0f, 32,
            &code, &scale);
        if (packed) {
            store_packed(kQuantFp8, reinterpret_cast<uint8_t*>(ring) +
                         slot * packed_row_bytes(kQuantFp8, width), width, i, code, scale);
        } else if (i < width) {
            ring[slot * width + i] = bf16_bits(v);
        }
    }
}

// -------------------------------------------------------------------------
// Dense matrix-vector products and RMS norms in the CPU's order
// -------------------------------------------------------------------------

__device__ __forceinline__ float dense_weight(
    const void* weights, const uint8_t* scales, int format, long long row, int col, int cols) {
    const long long at = row * cols + col;
    if (format == kDenseF32) {
        return static_cast<const float*>(weights)[at];
    }
    if (format == kDenseBf16) {
        return bf16_value(static_cast<const uint16_t*>(weights)[at]);
    }
    const float value = __fmul_rn(
        e4m3(static_cast<const uint8_t*>(weights)[at]),
        e8m0(scales[(row / 32) * (cols / 32) + col / 32]));
    return bf16_round(value);
}

constexpr int kDenseRows = 8;
constexpr int kDenseCols = 512;
constexpr int kDenseThreads = 256;

// Each block computes `kDenseRows` outputs for one token: `0 + sum_k w[r, k]
// x[k]`, the sum sequential from -0 as `row_product` adds it. Tiles of the
// rows are staged through shared memory with coalesced loads; one thread per
// row then adds its tile in order. Grouped matrices read their group's
// input slice.
__global__ void __launch_bounds__(kDenseThreads) v41_dense_kernel(
    const void* weights,
    const uint8_t* scales,
    const float* input,
    float* out,
    int format,
    int rows,
    int cols,
    int groups,
    int round_input,
    int round_bf16) {
    __shared__ float tile[kDenseRows][kDenseCols + 1];
    __shared__ float xs[kDenseRows][kDenseCols + 1];
    const int first = blockIdx.x * kDenseRows;
    const int token = blockIdx.y;
    const int mine = first + threadIdx.x;
    const bool owner = threadIdx.x < kDenseRows && mine < rows;
    const int per_group = rows / groups;
    // The next tile's loads are issued before the current tile is added.
    constexpr int kPerThread = kDenseRows * kDenseCols / kDenseThreads;
    float wv[kPerThread];
    float xv[kPerThread];
    auto load = [&](int base) {
#pragma unroll
        for (int k = 0; k < kPerThread; ++k) {
            const int e = threadIdx.x + k * kDenseThreads;
            const int r = e / kDenseCols;
            const int c = e % kDenseCols;
            const bool ok = base + c < cols && first + r < rows;
            wv[k] = ok ? dense_weight(weights, scales, format, first + r, base + c, cols) : 0.0f;
            xv[k] = ok ? input[(static_cast<long long>(token) * groups + (first + r) / per_group) *
                                   cols +
                               base + c]
                       : 0.0f;
        }
    };
    load(0);
    float sum = -0.0f;
    for (int base = 0; base < cols; base += kDenseCols) {
        const int width = cols - base < kDenseCols ? cols - base : kDenseCols;
        __syncthreads();
#pragma unroll
        for (int k = 0; k < kPerThread; ++k) {
            const int e = threadIdx.x + k * kDenseThreads;
            tile[e / kDenseCols][e % kDenseCols] = wv[k];
            xs[e / kDenseCols][e % kDenseCols] = round_input ? bf16_round(xv[k]) : xv[k];
        }
        __syncthreads();
        if (base + kDenseCols < cols) {
            load(base + kDenseCols);
        }
        if (owner) {
            // Sixteen operands are read before they're added, so the shared
            // loads overlap the dependent additions.
            int c = 0;
            for (; c + 16 <= width; c += 16) {
                float a[16];
                float b[16];
#pragma unroll
                for (int k = 0; k < 16; ++k) {
                    a[k] = tile[threadIdx.x][c + k];
                    b[k] = xs[threadIdx.x][c + k];
                }
#pragma unroll
                for (int k = 0; k < 16; ++k) {
                    sum = __fadd_rn(sum, __fmul_rn(a[k], b[k]));
                }
            }
            for (; c < width; ++c) {
                sum = __fadd_rn(sum, __fmul_rn(tile[threadIdx.x][c], xs[threadIdx.x][c]));
            }
        }
    }
    if (owner) {
        const float total = __fadd_rn(0.0f, sum);
        out[static_cast<long long>(token) * rows + mine] = round_bf16 ? bf16_round(total) : total;
    }
}

// The `consumer-q` profile's faster reductions (#12465, lossy and
// unqualified): a warp per output row and a block-wide tree for norms, so
// the sums no longer follow the CPU path's order. The exact profile never
// launches these.
// The same products as `v41_dense_kernel` for many tokens at once (#12464):
// each block takes kDenseTileRows rows and kDenseTileTokens tokens, stages a
// 32-column slice of the rows and of the tokens' inputs in shared memory, and
// each thread adds its row's slice into one running sum per token. Every
// sum still starts at -0 and adds `w[r, k] x[k]` in column order, then adds
// to 0, so the results are bit-equal to one token at a time. The block's
// rows must lie in one group.
constexpr int kDenseTileRows = 128;
constexpr int kDenseTileTokens = 16;
constexpr int kDenseTileCols = 32;

__global__ void __launch_bounds__(kDenseTileRows) v41_dense_tokens_kernel(
    const void* weights,
    const uint8_t* scales,
    const float* input,
    float* out,
    int format,
    int rows,
    int cols,
    int groups,
    int tokens,
    int round_input,
    int round_bf16) {
    __shared__ float tile[kDenseTileRows][kDenseTileCols + 1];
    __shared__ float xs[kDenseTileTokens][kDenseTileCols];
    const int first = blockIdx.x * kDenseTileRows;
    const int token0 = blockIdx.y * kDenseTileTokens;
    const int count = min(kDenseTileTokens, tokens - token0);
    const int mine = first + threadIdx.x;
    const int per_group = rows / groups;
    const int group = first / per_group;
    float sum[kDenseTileTokens];
#pragma unroll
    for (int t = 0; t < kDenseTileTokens; ++t) {
        sum[t] = -0.0f;
    }
    for (int base = 0; base < cols; base += kDenseTileCols) {
        const int width = min(kDenseTileCols, cols - base);
        __syncthreads();
        for (int e = threadIdx.x; e < kDenseTileRows * kDenseTileCols; e += kDenseTileRows) {
            const int r = e / kDenseTileCols;
            const int c = e % kDenseTileCols;
            tile[r][c] = (c < width && first + r < rows)
                ? dense_weight(weights, scales, format, first + r, base + c, cols)
                : 0.0f;
        }
        for (int e = threadIdx.x; e < kDenseTileTokens * kDenseTileCols; e += kDenseTileRows) {
            const int t = e / kDenseTileCols;
            const int c = e % kDenseTileCols;
            float x = 0.0f;
            if (t < count && c < width) {
                x = input[(static_cast<long long>(token0 + t) * groups + group) * cols + base + c];
                if (round_input) {
                    x = bf16_round(x);
                }
            }
            xs[t][c] = x;
        }
        __syncthreads();
        if (mine < rows) {
            for (int c = 0; c < width; ++c) {
                const float w = tile[threadIdx.x][c];
#pragma unroll
                for (int t = 0; t < kDenseTileTokens; ++t) {
                    sum[t] = __fadd_rn(sum[t], __fmul_rn(w, xs[t][c]));
                }
            }
        }
    }
    if (mine < rows) {
#pragma unroll
        for (int t = 0; t < kDenseTileTokens; ++t) {
            if (t < count) {
                const float total = __fadd_rn(0.0f, sum[t]);
                out[static_cast<long long>(token0 + t) * rows + mine] =
                    round_bf16 ? bf16_round(total) : total;
            }
        }
    }
}

__global__ void __launch_bounds__(kDenseThreads) v41_dense_fast_kernel(
    const void* weights,
    const uint8_t* scales,
    const float* input,
    float* out,
    int format,
    int rows,
    int cols,
    int groups,
    int round_input,
    int round_bf16) {
    const int warp = threadIdx.x >> 5;
    const int lane = threadIdx.x & 31;
    const int row = blockIdx.x * (kDenseThreads / 32) + warp;
    const int token = blockIdx.y;
    if (row >= rows) {
        return;
    }
    const int per_group = rows / groups;
    const float* x = input + (static_cast<long long>(token) * groups + row / per_group) * cols;
    float acc = 0.0f;
    for (int c = lane; c < cols; c += 32) {
        const float w = dense_weight(weights, scales, format, row, c, cols);
        const float v = round_input ? bf16_round(x[c]) : x[c];
        acc = fmaf(w, v, acc);
    }
    for (int offset = 16; offset > 0; offset >>= 1) {
        acc += __shfl_xor_sync(0xffffffffu, acc, offset);
    }
    if (lane == 0) {
        out[static_cast<long long>(token) * rows + row] = round_bf16 ? bf16_round(acc) : acc;
    }
}

__global__ void v41_rms_norm_fast_kernel(
    const float* input, const float* weight, float* out, int width, float epsilon, int round_bf16) {
    __shared__ float partial[32];
    const long long base = static_cast<long long>(blockIdx.x) * width;
    float sum = 0.0f;
    for (int i = threadIdx.x; i < width; i += blockDim.x) {
        const float v = input[base + i];
        sum = fmaf(v, v, sum);
    }
    for (int offset = 16; offset > 0; offset >>= 1) {
        sum += __shfl_xor_sync(0xffffffffu, sum, offset);
    }
    if ((threadIdx.x & 31) == 0) {
        partial[threadIdx.x >> 5] = sum;
    }
    __syncthreads();
    if (threadIdx.x < 32) {
        float s = threadIdx.x < (blockDim.x >> 5) ? partial[threadIdx.x] : 0.0f;
        for (int offset = 16; offset > 0; offset >>= 1) {
            s += __shfl_xor_sync(0xffffffffu, s, offset);
        }
        if (threadIdx.x == 0) {
            const float variance = s / static_cast<float>(width);
            partial[0] = isfinite(variance) ? rsqrtf(variance + epsilon) : CUDART_NAN_F;
        }
    }
    __syncthreads();
    const float scale = partial[0];
    for (int i = threadIdx.x; i < width; i += blockDim.x) {
        const float v = bf16_round(weight[i]) * (input[base + i] * scale);
        out[base + i] = round_bf16 ? bf16_round(v) : v;
    }
}

__global__ void v41_rms_norm_kernel(
    const float* input, const float* weight, float* out, int width, float epsilon, int round_bf16) {
    extern __shared__ float shared[];
    float* x = shared;
    const long long base = static_cast<long long>(blockIdx.x) * width;
    for (int i = threadIdx.x; i < width; i += blockDim.x) {
        x[i] = input[base + i];
    }
    rms_norm_shared(x, weight, width, epsilon, round_bf16 != 0, shared + width);
    for (int i = threadIdx.x; i < width; i += blockDim.x) {
        out[base + i] = x[i];
    }
}

// -------------------------------------------------------------------------
// Compression
// -------------------------------------------------------------------------

// The latent of every token whose group completes: the gated softmax pool of
// its `ratio` members (the earlier ones from this chunk or from `state`),
// rounded to BF16, then the RMS norm with the BF16 norm weight, rounded.
// Positions of a multi-token chunk are consecutive.
__global__ void v41_compress_kernel(
    const float* values,
    const float* gates,
    const float* norm,
    const float* state,
    const int* positions,
    float* latents,
    int ratio,
    int width,
    float epsilon) {
    extern __shared__ float shared[];
    float* pooled = shared;
    const int token = blockIdx.x;
    const int position = positions[token];
    if (position < 0 || (position + 1) % ratio != 0) {
        return;
    }
    for (int d = threadIdx.x; d < width; d += blockDim.x) {
        float member_values[8];
        float member_gates[8];
        for (int r = 0; r < ratio; ++r) {
            const int source = token - (ratio - 1) + r;
            if (source >= 0) {
                member_values[r] = values[static_cast<long long>(source) * width + d];
                member_gates[r] = ratio > 1 ? gates[static_cast<long long>(source) * width + d] : 0.0f;
            } else {
                const int slot = (position - (ratio - 1) + r) % ratio;
                member_values[r] = state[(2 * slot) * width + d];
                member_gates[r] = state[(2 * slot + 1) * width + d];
            }
        }
        float value;
        if (ratio == 1) {
            value = member_values[0];
        } else {
            float max = -CUDART_INF_F;
            for (int r = 0; r < ratio; ++r) {
                max = fmaxf(max, member_gates[r]);
            }
            float denominator = -0.0f;
            for (int r = 0; r < ratio; ++r) {
                denominator = __fadd_rn(denominator, exp_once(__fsub_rn(member_gates[r], max)));
            }
            value = -0.0f;
            for (int r = 0; r < ratio; ++r) {
                const float weight =
                    __fdiv_rn(exp_once(__fsub_rn(member_gates[r], max)), denominator);
                value = __fadd_rn(value, __fmul_rn(member_values[r], weight));
            }
        }
        pooled[d] = bf16_round(value);
    }
    rms_norm_shared(pooled, norm, width, epsilon, true, shared + width);
    for (int d = threadIdx.x; d < width; d += blockDim.x) {
        latents[static_cast<long long>(token) * width + d] = pooled[d];
    }
}

// Keeps the chunk's trailing incomplete group in `state` for the next call.
__global__ void v41_compress_state_kernel(
    const float* values,
    const float* gates,
    float* state,
    const int* positions,
    int tokens,
    int ratio,
    int width) {
    const int last = positions[tokens - 1];
    if (last < 0) {
        return;
    }
    const int pending = (last + 1) % ratio;
    for (int k = 0; k < pending; ++k) {
        const int token = tokens - pending + k;
        if (token < 0) {
            continue;
        }
        const int slot = positions[token] % ratio;
        for (int d = threadIdx.x; d < width; d += blockDim.x) {
            state[(2 * slot) * width + d] = values[static_cast<long long>(token) * width + d];
            state[(2 * slot + 1) * width + d] =
                gates != nullptr ? gates[static_cast<long long>(token) * width + d] : 0.0f;
        }
    }
}

// For every token whose group completes: the index key (BF16 `wk` times the
// unrotated latent, the BF16 `k_norm`, RoPE at the group's first position,
// and the per-32 FP4/UE8M0 form) and the compressed KV row (RoPE on the
// latent, then the per-16 FP4/E4M3 form), stored at row `(p + 1) / ratio - 1`.
__global__ void v41_compressed_store_kernel(
    const float* latents,
    const uint16_t* wk,
    const float* k_norm,
    const float* frequencies,
    const int* positions,
    uint16_t* kv,
    uint16_t* index_keys,
    int ratio,
    int capacity,
    int head_dim,
    int index_dim,
    int rope_dim,
    float epsilon,
    int packed) {
    extern __shared__ float shared[];
    float* latent = shared;
    float* key = latent + head_dim;
    float* scratch = key + index_dim;
    const int token = blockIdx.x;
    const int position = positions[token];
    if (position < 0 || (position + 1) % ratio != 0) {
        return;
    }
    const int row = (position + 1) / ratio - 1;
    if (row >= capacity) {
        return;
    }
    const int group_position = position + 1 - ratio;
    for (int i = threadIdx.x; i < head_dim; i += blockDim.x) {
        latent[i] = latents[static_cast<long long>(token) * head_dim + i];
    }
    __syncthreads();
    // `wk` times the latent, one thread per key row, read in 16-byte loads
    // eight at a time.
    for (int r = threadIdx.x; r < index_dim; r += blockDim.x) {
        const uint4* row4 = reinterpret_cast<const uint4*>(wk + static_cast<long long>(r) * head_dim);
        float sum = -0.0f;
        for (int c0 = 0; c0 < head_dim; c0 += 64) {
            uint4 v[8];
#pragma unroll
            for (int k = 0; k < 8; ++k) {
                v[k] = row4[c0 / 8 + k];
            }
#pragma unroll
            for (int k = 0; k < 8; ++k) {
                const uint32_t words[4] = {v[k].x, v[k].y, v[k].z, v[k].w};
#pragma unroll
                for (int h = 0; h < 4; ++h) {
                    const int c = c0 + k * 8 + h * 2;
                    sum = __fadd_rn(sum, __fmul_rn(__uint_as_float(words[h] << 16), bf16_round(latent[c])));
                    sum = __fadd_rn(
                        sum, __fmul_rn(__uint_as_float(words[h] & 0xffff0000u), bf16_round(latent[c + 1])));
                }
            }
        }
        key[r] = bf16_round(__fadd_rn(0.0f, sum));
    }
    rms_norm_shared(key, k_norm, index_dim, epsilon, true, scratch);
    rope_head(key, key, frequencies, rope_dim, index_dim, group_position, false, true);
    rope_head(latent, latent, frequencies, rope_dim, head_dim, group_position, false, true);
    __syncthreads();
    for (int base = 0; base < index_dim; base += blockDim.x) {
        const int i = base + threadIdx.x;
        uint32_t code = 0;
        uint32_t scale = 0;
        const float v =
            quant_lane_parts(kQuantIndexFp4, i < index_dim ? key[i] : 0.0f, 32, &code, &scale);
        if (packed) {
            store_packed(kQuantIndexFp4, reinterpret_cast<uint8_t*>(index_keys) +
                         row * packed_row_bytes(kQuantIndexFp4, index_dim), index_dim, i, code, scale);
        } else if (i < index_dim) {
            index_keys[static_cast<long long>(row) * index_dim + i] = bf16_bits(v);
        }
    }
    for (int base = 0; base < head_dim; base += blockDim.x) {
        const int i = base + threadIdx.x;
        uint32_t code = 0;
        uint32_t scale = 0;
        const float v = quant_lane_parts(
            kQuantCompressedFp4, i < head_dim ? latent[i] : 0.0f, 16, &code, &scale);
        if (packed) {
            store_packed(kQuantCompressedFp4, reinterpret_cast<uint8_t*>(kv) +
                         row * packed_row_bytes(kQuantCompressedFp4, head_dim), head_dim, i, code, scale);
        } else if (i < head_dim) {
            kv[static_cast<long long>(row) * head_dim + i] = bf16_bits(v);
        }
    }
}

// -------------------------------------------------------------------------
// Indexer scores
// -------------------------------------------------------------------------

constexpr int kScoreWarps = 8;
// Keys one index-score block scores: each warp takes every eighth.
constexpr int kScoreKeys = 32;

// One warp per key and one lane per index head (heads <= 32): each lane takes
// its head's dot product sequentially from -0, then every lane adds
// `max(dot_h, 0) * bf16(w_h * scale)` over the heads in order from +0, as the
// CPU's reference scores do. A block stages the query once (rows padded so
// the lanes read different banks) and 32 keys in shared memory.
__global__ void __launch_bounds__(kScoreWarps * 32) v41_index_scores_kernel(
    const float* query,
    const float* weights,
    const uint16_t* keys,
    const int* positions,
    float* scores,
    long long score_stride,
    int capacity,
    int ratio,
    int heads,
    int dim,
    float scale,
    int packed) {
    extern __shared__ float shared[];
    const int padded = dim + 1;
    float* q = shared;
    float* w = q + heads * padded;
    uint16_t* k = reinterpret_cast<uint16_t*>(w + heads);
    const int token = blockIdx.y;
    for (int i = threadIdx.x; i < heads * dim; i += blockDim.x) {
        q[(i / dim) * padded + i % dim] = query[static_cast<long long>(token) * heads * dim + i];
    }
    for (int h = threadIdx.x; h < heads; h += blockDim.x) {
        w[h] = bf16_round(__fmul_rn(weights[static_cast<long long>(token) * heads + h], scale));
    }
    const int position = positions[token];
    const long long visible = position < 0 ? 0 : (static_cast<long long>(position) + 1) / ratio;
    const long long first = static_cast<long long>(blockIdx.x) * kScoreKeys;
    for (int i = threadIdx.x; i < kScoreKeys * dim; i += blockDim.x) {
        const long long key = first + i / dim;
        if (key < visible && key < capacity) {
            k[i] = packed ? packed_word(kQuantIndexFp4,
                                        reinterpret_cast<const uint8_t*>(keys) +
                                            key * packed_row_bytes(kQuantIndexFp4, dim),
                                        dim, i % dim)
                          : keys[key * dim + i % dim];
        } else {
            k[i] = 0;
        }
    }
    __syncthreads();
    const int warp = threadIdx.x / 32;
    const int lane = threadIdx.x & 31;
    for (int local = warp; local < kScoreKeys; local += kScoreWarps) {
        const long long key = first + local;
        if (key >= visible || key >= capacity) {
            break;
        }
        float term = 0.0f;
        bool finite = true;
        if (lane < heads) {
            float dot = -0.0f;
            const float* qh = q + lane * padded;
            const uint16_t* kk = k + local * dim;
            int i = 0;
            for (; i + 16 <= dim; i += 16) {
                float a[16];
                float b[16];
#pragma unroll
                for (int j = 0; j < 16; ++j) {
                    a[j] = qh[i + j];
                    b[j] = bf16_value(kk[i + j]);
                }
#pragma unroll
                for (int j = 0; j < 16; ++j) {
                    dot = __fadd_rn(dot, __fmul_rn(a[j], b[j]));
                }
            }
            for (; i < dim; ++i) {
                dot = __fadd_rn(dot, __fmul_rn(qh[i], bf16_value(kk[i])));
            }
            finite = isfinite(dot);
            term = __fmul_rn(fmaxf(dot, 0.0f), w[lane]);
        }
        const bool poison = __any_sync(0xffffffffu, !finite);
        // Every lane gathers the 32 terms first, then adds them in order.
        float terms[32];
#pragma unroll
        for (int h = 0; h < 32; ++h) {
            terms[h] = __shfl_sync(0xffffffffu, term, h);
        }
        float score = 0.0f;
#pragma unroll
        for (int h = 0; h < 32; ++h) {
            if (h < heads) {
                score = __fadd_rn(score, terms[h]);
            }
        }
        if (lane == 0) {
            scores[static_cast<long long>(token) * score_stride + key] =
                poison ? CUDART_NAN_F : score;
        }
    }
}

// -------------------------------------------------------------------------
// Selection
// -------------------------------------------------------------------------

// `total_cmp` order as an unsigned key: larger floats give larger keys.
__device__ __forceinline__ uint32_t order_key(float value) {
    const uint32_t bits = __float_as_uint(value);
    return (bits & 0x80000000u) ? ~bits : (bits | 0x80000000u);
}

// Block-wide exclusive prefix of `flag` over the 1024 threads; returns this
// thread's prefix and writes the total to `*total`. Uses `warp_counts`.
__device__ __forceinline__ int block_prefix(bool flag, int* warp_counts, int* total) {
    const int lane = threadIdx.x & 31;
    const int warp = threadIdx.x >> 5;
    const unsigned ballot = __ballot_sync(0xffffffffu, flag);
    const int within = __popc(ballot & ((1u << lane) - 1u));
    __syncthreads();
    if (lane == 0) {
        warp_counts[warp] = __popc(ballot);
    }
    __syncthreads();
    int before = 0;
    int sum = 0;
    for (int w = 0; w < kSelectThreads / 32; ++w) {
        if (w < warp) {
            before += warp_counts[w];
        }
        sum += warp_counts[w];
    }
    *total = sum;
    return before + within;
}

// Top `top_k` of each token's first `n` values by (value descending, index
// ascending), written in ascending index order with -1 after the selected
// ones, as `select_indices` and `candidate_mask` order them.
//
// `n = ceil(((p + 1) / ratio) / group)`. A value is eligible when it isn't
// -inf and, with candidate flags, its group of `flag_group` values is kept.
// NaN, +inf (unless `allow_inf`), or a poisoned flag poison the whole row.
// With `flags_out`, writes 1 for selected values and 0 for the others.
__global__ void __launch_bounds__(kSelectThreads) v41_select_kernel(
    const float* values,
    long long value_stride,
    const uint8_t* flags,
    long long flag_stride,
    int flag_group,
    const int* positions,
    int ratio,
    int group,
    int top_k,
    int allow_inf,
    int* out,
    long long out_stride,
    uint8_t* flags_out,
    long long flags_out_stride) {
    __shared__ unsigned histogram[256];
    __shared__ int warp_counts[kSelectThreads / 32];
    __shared__ int shared_state[4];
    const int token = blockIdx.x;
    const float* row = values + token * value_stride;
    const uint8_t* flag_row = flags != nullptr ? flags + token * flag_stride : nullptr;
    int* out_row = out + token * out_stride;
    uint8_t* flags_out_row = flags_out != nullptr ? flags_out + token * flags_out_stride : nullptr;
    const int position = positions[token];
    const long long visible = position < 0 ? 0 : (static_cast<long long>(position) + 1) / ratio;
    const long long wanted = (visible + group - 1) / group;
    // A position past the buffer's rows sees only the rows it has.
    const int n = static_cast<int>(wanted < value_stride ? wanted : value_stride);
    const int count = top_k < n ? top_k : n;

    // Poison and eligibility.
    if (threadIdx.x == 0) {
        shared_state[0] = 0;
        shared_state[1] = 0;
    }
    __syncthreads();
    int eligible_count = 0;
    bool poison = false;
#pragma unroll 4
    for (int i = threadIdx.x; i < n; i += kSelectThreads) {
        const float v = row[i];
        const uint8_t flag = flag_row != nullptr ? flag_row[i / flag_group] : 1u;
        if (isnan(v) || (v == CUDART_INF_F && !allow_inf) || flag == kPoisonFlag) {
            poison = true;
        }
        if (v != -CUDART_INF_F && flag != 0u) {
            ++eligible_count;
        }
    }
    if (poison) {
        atomicOr(&shared_state[0], 1);
    }
    atomicAdd(&shared_state[1], eligible_count);
    __syncthreads();
    if (shared_state[0] != 0) {
        for (int i = threadIdx.x; i < top_k; i += kSelectThreads) {
            out_row[i] = kPoison;
        }
        if (flags_out_row != nullptr) {
            for (int i = threadIdx.x; i < n; i += kSelectThreads) {
                flags_out_row[i] = kPoisonFlag;
            }
        }
        return;
    }
    const int eligible = shared_state[1];
    const int take = count < eligible ? count : eligible;

    // Radix-select the key of the take-th largest eligible value.
    uint32_t prefix = 0;
    uint32_t mask = 0;
    int remaining = take;
    for (int shift = 24; shift >= 0 && take > 0; shift -= 8) {
        for (int d = threadIdx.x; d < 256; d += kSelectThreads) {
            histogram[d] = 0;
        }
        __syncthreads();
#pragma unroll 4
        for (int i = threadIdx.x; i < n; i += kSelectThreads) {
            const float v = row[i];
            const uint8_t flag = flag_row != nullptr ? flag_row[i / flag_group] : 1u;
            if (v == -CUDART_INF_F || flag == 0u) {
                continue;
            }
            const uint32_t key = order_key(v);
            if ((key & mask) == prefix) {
                atomicAdd(&histogram[(key >> shift) & 255u], 1u);
            }
        }
        __syncthreads();
        if (threadIdx.x == 0) {
            int above = 0;
            int digit = 255;
            for (; digit > 0; --digit) {
                if (above + static_cast<int>(histogram[digit]) >= remaining) {
                    break;
                }
                above += static_cast<int>(histogram[digit]);
            }
            shared_state[2] = digit;
            shared_state[3] = remaining - above;
        }
        __syncthreads();
        prefix |= static_cast<uint32_t>(shared_state[2]) << shift;
        mask |= 255u << shift;
        remaining = shared_state[3];
        __syncthreads();
    }
    const uint32_t threshold = prefix;
    const int ties_needed = remaining;

    // Write the selection in index order.
    int written = 0;
    int ties_seen = 0;
    for (int base = 0; base < n; base += kSelectThreads) {
        const int i = base + threadIdx.x;
        bool above = false;
        bool tie = false;
        if (i < n && take > 0) {
            const float v = row[i];
            const uint8_t flag = flag_row != nullptr ? flag_row[i / flag_group] : 1u;
            if (v != -CUDART_INF_F && flag != 0u) {
                const uint32_t key = order_key(v);
                above = key > threshold;
                tie = key == threshold;
            }
        }
        int tie_total = 0;
        const int tie_rank = ties_seen + block_prefix(tie, warp_counts, &tie_total);
        const bool selected = above || (tie && tie_rank < ties_needed);
        int selected_total = 0;
        const int slot = written + block_prefix(selected, warp_counts, &selected_total);
        if (selected) {
            out_row[slot] = i;
        }
        if (flags_out_row != nullptr && i < n) {
            flags_out_row[i] = selected ? 1u : 0u;
        }
        written += selected_total;
        ties_seen += tie_total;
    }
    for (int i = take + threadIdx.x; i < top_k; i += kSelectThreads) {
        out_row[i] = -1;
    }
}

// Each 8-value block's maximum score, with the newest (possibly partial)
// block pinned to +inf. NaN or +inf in a block poisons it with NaN.
__global__ void v41_block_max_kernel(
    const float* scores,
    long long score_stride,
    float* block_scores,
    long long block_stride,
    const int* positions,
    int ratio,
    int block_size) {
    const int token = blockIdx.y;
    const int position = positions[token];
    long long visible = position < 0 ? 0 : (static_cast<long long>(position) + 1) / ratio;
    visible = visible < score_stride ? visible : score_stride;
    long long blocks = (visible + block_size - 1) / block_size;
    blocks = blocks < block_stride ? blocks : block_stride;
    const long long b = static_cast<long long>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (b >= blocks) {
        return;
    }
    const float* row = scores + token * score_stride;
    float max = -CUDART_INF_F;
    bool poison = false;
    for (long long i = b * block_size; i < (b + 1) * block_size && i < visible; ++i) {
        const float v = row[i];
        poison = poison || isnan(v) || v == CUDART_INF_F;
        max = fmaxf(max, v);
    }
    if (b == (visible - 1) / block_size) {
        max = CUDART_INF_F;
    }
    block_scores[token * block_stride + b] = poison ? CUDART_NAN_F : max;
}

// -------------------------------------------------------------------------
// Sparse latent attention
// -------------------------------------------------------------------------

constexpr int kAttentionThreads = 256;
// A slot's KV row: the ring row (>= 0), compressed row `-(code + 2)`, or -1
// when masked.
__device__ __forceinline__ const uint16_t* slot_row(
    int code, const uint16_t* ring, const uint16_t* compressed, int head_dim) {
    if (code >= 0) {
        return ring + static_cast<long long>(code) * head_dim;
    }
    if (code == -1) {
        return nullptr;
    }
    return compressed + static_cast<long long>(-(code + 2)) * head_dim;
}

// Eight BF16 words of a slot's row from column `col` (a multiple of 8), as a
// 16-byte vector; zeros for a masked slot. Packed rows (#12471) decode to
// the same words.
__device__ __forceinline__ uint4 slot_words(
    int code, int col, const uint16_t* ring, const uint16_t* compressed, int head_dim, int packed) {
    if (code == -1) {
        return make_uint4(0, 0, 0, 0);
    }
    if (!packed) {
        return *reinterpret_cast<const uint4*>(slot_row(code, ring, compressed, head_dim) + col);
    }
    const int kind = code >= 0 ? kQuantFp8 : kQuantCompressedFp4;
    const long long r = code >= 0 ? code : -(code + 2);
    const uint8_t* row = reinterpret_cast<const uint8_t*>(code >= 0 ? ring : compressed) +
                         r * packed_row_bytes(kind, head_dim);
    uint32_t w[4];
#pragma unroll
    for (int h = 0; h < 4; ++h) {
        w[h] = static_cast<uint32_t>(packed_word(kind, row, head_dim, col + 2 * h)) |
               (static_cast<uint32_t>(packed_word(kind, row, head_dim, col + 2 * h + 1)) << 16);
    }
    return make_uint4(w[0], w[1], w[2], w[3]);
}

// Dimensions one attention block writes.
constexpr int kAttentionSplit = 128;
// BF16 words per row of a staged value tile (128 dimensions plus padding,
// keeping rows 16-byte aligned).
constexpr int kValueStride = kAttentionSplit + 8;

// Adds eight BF16 values times `q[d..d + 8]` to `dot` in order. The query
// values are read before the dependent additions start.
__device__ __forceinline__ void dot_words(float& dot, const float* q, int d, uint4 v) {
    const float4 qa = *reinterpret_cast<const float4*>(q + d);
    const float4 qb = *reinterpret_cast<const float4*>(q + d + 4);
    const float qs[8] = {qa.x, qa.y, qa.z, qa.w, qb.x, qb.y, qb.z, qb.w};
    const uint32_t words[4] = {v.x, v.y, v.z, v.w};
#pragma unroll
    for (int h = 0; h < 4; ++h) {
        dot = __fadd_rn(dot, __fmul_rn(qs[2 * h], __uint_as_float(words[h] << 16)));
        dot = __fadd_rn(dot, __fmul_rn(qs[2 * h + 1], __uint_as_float(words[h] & 0xffff0000u)));
    }
}

// Heads one attention block computes; they share every row it loads.
constexpr int kAttentionHeads = 4;

// One block per (4 heads, 128 output dimensions, token). Slots are the window
// rows oldest first, then the selected compressed rows (masked -1 slots keep
// their place), in blocks of 64 with the CPU's online softmax: BF16 weights
// in the value product, a float32 running denominator, and the sink added to
// the denominator last. Every block of a head computes the same scores,
// maxima, and denominators; each writes its own dimensions.
//
// Each slot's score is one thread's sequential dot product per head over
// 16-byte loads, the next eight loads issued before the current eight are
// added. The value rows of each 64-slot block are staged in shared memory,
// the next block's loads issued before the current block is added.
__global__ void __launch_bounds__(kAttentionThreads) v41_sparse_attention_kernel(
    const float* query,
    const float* sinks,
    const uint16_t* ring,
    int ring_rows,
    const uint16_t* compressed,
    int capacity,
    const int* indices,
    long long index_stride,
    const int* positions,
    float* out,
    int heads,
    int head_dim,
    int window,
    int ratio,
    int top_k,
    float scale,
    int packed) {
    constexpr int G = kAttentionHeads;
    extern __shared__ float shared[];
    __shared__ __align__(16) uint16_t values[64 * kValueStride];
    __shared__ float running[G][3];
    __shared__ int poisoned[G];
    const int max_slots = window + (ratio > 0 ? top_k : 0);
    float* q = shared;
    // Each block's weights replace its scores, which are read only before.
    float* scores = q + G * head_dim;
    float* weights = scores;
    int* codes = reinterpret_cast<int*>(scores + G * max_slots);
    const int head0 = blockIdx.x * G;
    const int dims = blockIdx.y * kAttentionSplit;
    const int token = blockIdx.z;
    const int position = positions[token];
    const long long query_base = (static_cast<long long>(token) * heads + head0) * head_dim;
    // In the value phase, thread t adds dimension `dims + t % 128` for heads
    // `2 (t / 128)` and `2 (t / 128) + 1` of the block's four.
    const int lane = threadIdx.x % kAttentionSplit;
    const int pair = threadIdx.x / kAttentionSplit;
    const int d = dims + lane;
    const bool writes = d < head_dim;
    for (int i = threadIdx.x; i < G * head_dim; i += blockDim.x) {
        q[i] = bf16_round(query[query_base + i]);
    }
    if (threadIdx.x < G) {
        poisoned[threadIdx.x] = position < 0 ? 1 : 0;
    }
    __syncthreads();
    if (position < 0) {
        for (int g = 2 * pair; g < 2 * pair + 2 && writes; ++g) {
            out[query_base + g * head_dim + d] = CUDART_NAN_F;
        }
        return;
    }
    const int window_rows = position + 1 < window ? position + 1 : window;
    const long long visible = ratio > 0 ? (static_cast<long long>(position) + 1) / ratio : 0;
    const int selected = ratio > 0 ? static_cast<int>(visible < top_k ? visible : top_k) : 0;
    const int slots = window_rows + selected;
    const int first = position + 1 - window_rows;
    for (int slot = threadIdx.x; slot < slots; slot += blockDim.x) {
        int code;
        if (slot < window_rows) {
            code = (first + slot) % ring_rows;
        } else {
            const int id = indices[token * index_stride + (slot - window_rows)];
            const bool invalid = id < -1 || id >= visible || id >= capacity;
            if (invalid) {
                // An invalid index poisons every head and is never read.
                for (int g = 0; g < G; ++g) {
                    atomicOr(&poisoned[g], 1);
                }
            }
            code = id < 0 || invalid ? -1 : -(id + 2);
        }
        codes[slot] = code;
    }
    __syncthreads();

    // Scores. A thread's work items are (slot, 64-dimension tile) pairs in
    // order; each item's eight loads are issued while the previous item is
    // added to the four heads' dot products.
    {
        const int tiles = head_dim / 64;
        const int mine = slots > static_cast<int>(threadIdx.x)
                             ? (slots - 1 - static_cast<int>(threadIdx.x)) / kAttentionThreads + 1
                             : 0;
        const int items = mine * tiles;
        auto load_item = [&](int item, uint4* out) {
            const int slot = threadIdx.x + (item / tiles) * kAttentionThreads;
#pragma unroll
            for (int k = 0; k < 8; ++k) {
                out[k] = slot_words(codes[slot], (item % tiles) * 64 + k * 8, ring, compressed,
                                    head_dim, packed);
            }
        };
        uint4 next[8];
        if (items > 0) {
            load_item(0, next);
        }
        float dot[G];
#pragma unroll
        for (int g = 0; g < G; ++g) {
            dot[g] = -0.0f;
        }
        for (int item = 0; item < items; ++item) {
            uint4 current[8];
#pragma unroll
            for (int k = 0; k < 8; ++k) {
                current[k] = next[k];
            }
            if (item + 1 < items) {
                load_item(item + 1, next);
            }
            const int tile = item % tiles;
#pragma unroll
            for (int k = 0; k < 8; ++k) {
#pragma unroll
                for (int g = 0; g < G; ++g) {
                    dot_words(dot[g], q + g * head_dim, tile * 64 + k * 8, current[k]);
                }
            }
            if (tile == tiles - 1) {
                const int slot = threadIdx.x + (item / tiles) * kAttentionThreads;
                const bool masked = codes[slot] == -1;
#pragma unroll
                for (int g = 0; g < G; ++g) {
                    float score = 0.0f;
                    if (!masked) {
                        score = __fmul_rn(dot[g], scale);
                        if (!isfinite(score)) {
                            atomicOr(&poisoned[g], 1);
                        }
                    }
                    scores[g * max_slots + slot] = score;
                    dot[g] = -0.0f;
                }
            }
        }
    }

    // Value rows of a 64-slot block: 64 rows of this block's 128 dimensions,
    // eight 16-byte loads per thread.
    constexpr int kStaged = 64 * kAttentionSplit / 8 / kAttentionThreads;
    auto load_values = [&](int chunk, uint4* staged) {
#pragma unroll
        for (int k = 0; k < kStaged; ++k) {
            const int e = threadIdx.x + k * kAttentionThreads;
            const int r = e / (kAttentionSplit / 8);
            const int c = (e % (kAttentionSplit / 8)) * 8;
            staged[k] = chunk + r < slots && dims + c < head_dim
                            ? slot_words(codes[chunk + r], dims + c, ring, compressed, head_dim, packed)
                            : make_uint4(0, 0, 0, 0);
        }
    };
    uint4 staged[kStaged];
    load_values(0, staged);
    float acc[2] = {0.0f, 0.0f};
    if (threadIdx.x < G) {
        running[threadIdx.x][0] = -1e30f;  // max
        running[threadIdx.x][1] = 0.0f;    // sum
    }
    for (int chunk = 0; chunk < slots; chunk += 64) {
        const int end = chunk + 64 < slots ? chunk + 64 : slots;
        __syncthreads();
#pragma unroll
        for (int k = 0; k < kStaged; ++k) {
            const int e = threadIdx.x + k * kAttentionThreads;
            *reinterpret_cast<uint4*>(
                values + (e / (kAttentionSplit / 8)) * kValueStride + (e % (kAttentionSplit / 8)) * 8) =
                staged[k];
        }
        if (threadIdx.x < G) {
            const int g = threadIdx.x;
            const float* head_scores = scores + g * max_slots;
            const float previous = running[g][0];
            float max = previous;
#pragma unroll 8
            for (int slot = chunk; slot < end; ++slot) {
                if (codes[slot] != -1) {
                    max = fmaxf(max, head_scores[slot]);
                }
            }
            running[g][0] = max;
            running[g][2] = exp_once(__fsub_rn(previous, max));
            running[g][1] = __fmul_rn(running[g][1], running[g][2]);
        }
        __syncthreads();
        for (int e = threadIdx.x; e < G * 64; e += blockDim.x) {
            const int g = e / 64;
            const int slot = chunk + e % 64;
            if (slot < end) {
                weights[g * max_slots + slot] =
                    codes[slot] != -1
                        ? exp_once(__fsub_rn(scores[g * max_slots + slot], running[g][0]))
                        : 0.0f;
            }
        }
        __syncthreads();
        if (threadIdx.x < G) {
            const int g = threadIdx.x;
            float sum = running[g][1];
#pragma unroll 8
            for (int slot = chunk; slot < end; ++slot) {
                if (codes[slot] != -1) {
                    sum = __fadd_rn(sum, weights[g * max_slots + slot]);
                }
            }
            running[g][1] = sum;
        }
        if (end < slots) {
            load_values(end, staged);
        }
        // A masked slot adds +0 (its weight and staged row are zero), which
        // leaves the sum unchanged: the sum is never -0.
        float value[2];
#pragma unroll
        for (int h = 0; h < 2; ++h) {
            value[h] = __fmul_rn(acc[h], running[2 * pair + h][2]);
        }
        const float* w0 = weights + (2 * pair) * max_slots;
        const float* w1 = w0 + max_slots;
        int slot = chunk;
        for (; slot + 8 <= end; slot += 8) {
            float v[8];
            float a[8];
            float b[8];
#pragma unroll
            for (int k = 0; k < 8; ++k) {
                v[k] = bf16_value(values[(slot + k - chunk) * kValueStride + lane]);
                a[k] = bf16_round(w0[slot + k]);
                b[k] = bf16_round(w1[slot + k]);
            }
#pragma unroll
            for (int k = 0; k < 8; ++k) {
                value[0] = __fadd_rn(value[0], __fmul_rn(a[k], v[k]));
                value[1] = __fadd_rn(value[1], __fmul_rn(b[k], v[k]));
            }
        }
        for (; slot < end; ++slot) {
            const float v = bf16_value(values[(slot - chunk) * kValueStride + lane]);
            value[0] = __fadd_rn(value[0], __fmul_rn(bf16_round(w0[slot]), v));
            value[1] = __fadd_rn(value[1], __fmul_rn(bf16_round(w1[slot]), v));
        }
        acc[0] = value[0];
        acc[1] = value[1];
    }
    __syncthreads();
#pragma unroll
    for (int h = 0; h < 2; ++h) {
        if (!writes) {
            break;
        }
        const int g = 2 * pair + h;
        const long long at = query_base + g * head_dim + d;
        if (poisoned[g]) {
            out[at] = CUDART_NAN_F;
            continue;
        }
        const float sum =
            __fadd_rn(running[g][1], exp_once(__fsub_rn(sinks[head0 + g], running[g][0])));
        out[at] = bf16_round(__fdiv_rn(acc[h], sum));
    }
}

unsigned blocks_for(long long n, int threads) {
    return static_cast<unsigned>((n + threads - 1) / threads);
}

}  // namespace

extern "C" int psionic_v41_rope(
    void* values,
    const void* frequencies,
    const void* positions,
    int tokens,
    int heads,
    int head_width,
    int rope_dim,
    int back,
    int inverse,
    int round_bf16,
    void* stream) {
    v41_rope_kernel<<<dim3(heads, tokens), 128, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<float*>(values), static_cast<const float*>(frequencies),
        static_cast<const int*>(positions), heads, head_width, rope_dim, back, inverse,
        round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_cache_quant(
    const void* input, void* out, int kind, int rows, int width, void* stream) {
    const long long total = static_cast<long long>(rows) * width;
    v41_cache_quant_kernel<<<blocks_for(total, kThreads), kThreads, 0,
                             static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(input), static_cast<float*>(out), kind, total);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_window_store(
    const void* input,
    void* ring,
    const void* positions,
    int tokens,
    int ring_rows,
    int width,
    int packed,
    void* stream) {
    v41_window_store_kernel<<<tokens, 128, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(input), static_cast<uint16_t*>(ring),
        static_cast<const int*>(positions), ring_rows, width, packed);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_dense(
    const void* weights,
    const void* scales,
    const void* input,
    void* out,
    int format,
    int rows,
    int cols,
    int groups,
    int tokens,
    int round_input,
    int round_bf16,
    void* stream) {
    const int per_group = groups > 0 ? rows / groups : 0;
    if (tokens >= 4 && per_group > 0 && (groups == 1 || per_group % kDenseTileRows == 0)) {
        v41_dense_tokens_kernel<<<dim3(blocks_for(rows, kDenseTileRows),
                                       blocks_for(tokens, kDenseTileTokens)),
                                  kDenseTileRows, 0, static_cast<cudaStream_t>(stream)>>>(
            weights, static_cast<const uint8_t*>(scales), static_cast<const float*>(input),
            static_cast<float*>(out), format, rows, cols, groups, tokens, round_input, round_bf16);
        return static_cast<int>(cudaGetLastError());
    }
    v41_dense_kernel<<<dim3(blocks_for(rows, kDenseRows), tokens), kDenseThreads, 0,
                       static_cast<cudaStream_t>(stream)>>>(
        weights, static_cast<const uint8_t*>(scales), static_cast<const float*>(input),
        static_cast<float*>(out), format, rows, cols, groups, round_input, round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_rms_norm(
    const void* input,
    const void* weight,
    void* out,
    int tokens,
    int width,
    float epsilon,
    int round_bf16,
    void* stream) {
    const size_t shared = (static_cast<size_t>(width) + 1) * sizeof(float);
    v41_rms_norm_kernel<<<tokens, 128, shared, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(input), static_cast<const float*>(weight),
        static_cast<float*>(out), width, epsilon, round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_compress(
    const void* values,
    const void* gates,
    const void* norm,
    void* state,
    const void* positions,
    void* latents,
    int tokens,
    int ratio,
    int width,
    float epsilon,
    void* stream) {
    const cudaStream_t s = static_cast<cudaStream_t>(stream);
    const size_t shared = (static_cast<size_t>(width) + 1) * sizeof(float);
    v41_compress_kernel<<<tokens, 128, shared, s>>>(
        static_cast<const float*>(values), static_cast<const float*>(gates),
        static_cast<const float*>(norm), static_cast<const float*>(state),
        static_cast<const int*>(positions), static_cast<float*>(latents), ratio, width, epsilon);
    if (ratio > 1) {
        v41_compress_state_kernel<<<1, 128, 0, s>>>(
            static_cast<const float*>(values), static_cast<const float*>(gates),
            static_cast<float*>(state), static_cast<const int*>(positions), tokens, ratio, width);
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_compressed_store(
    const void* latents,
    const void* wk,
    const void* k_norm,
    const void* frequencies,
    const void* positions,
    void* kv,
    void* index_keys,
    int tokens,
    int ratio,
    int capacity,
    int head_dim,
    int index_dim,
    int rope_dim,
    float epsilon,
    int packed,
    void* stream) {
    const size_t shared = (static_cast<size_t>(head_dim) + index_dim + 1) * sizeof(float);
    v41_compressed_store_kernel<<<tokens, 128, shared, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(latents), static_cast<const uint16_t*>(wk),
        static_cast<const float*>(k_norm), static_cast<const float*>(frequencies),
        static_cast<const int*>(positions), static_cast<uint16_t*>(kv),
        static_cast<uint16_t*>(index_keys), ratio, capacity, head_dim, index_dim, rope_dim,
        epsilon, packed);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_index_scores(
    const void* query,
    const void* weights,
    const void* keys,
    const void* positions,
    void* scores,
    long long score_stride,
    int tokens,
    int capacity,
    int ratio,
    int heads,
    int dim,
    float scale,
    int packed,
    void* stream) {
    const size_t shared = (static_cast<size_t>(heads) * (dim + 1) + heads) * sizeof(float) +
                          static_cast<size_t>(kScoreKeys) * dim * sizeof(uint16_t);
    v41_index_scores_kernel<<<dim3(blocks_for(capacity, kScoreKeys), tokens), kScoreWarps * 32,
                              shared, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(query), static_cast<const float*>(weights),
        static_cast<const uint16_t*>(keys), static_cast<const int*>(positions),
        static_cast<float*>(scores), score_stride, capacity, ratio, heads, dim, scale, packed);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_select(
    const void* values,
    long long value_stride,
    const void* flags,
    long long flag_stride,
    int flag_group,
    const void* positions,
    int tokens,
    int ratio,
    int group,
    int top_k,
    int allow_inf,
    void* out,
    long long out_stride,
    void* flags_out,
    long long flags_out_stride,
    void* stream) {
    v41_select_kernel<<<tokens, kSelectThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(values), value_stride, static_cast<const uint8_t*>(flags),
        flag_stride, flag_group, static_cast<const int*>(positions), ratio, group, top_k,
        allow_inf, static_cast<int*>(out), out_stride, static_cast<uint8_t*>(flags_out),
        flags_out_stride);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_block_max(
    const void* scores,
    long long score_stride,
    void* block_scores,
    long long block_stride,
    const void* positions,
    int tokens,
    int block_capacity,
    int ratio,
    int block_size,
    void* stream) {
    v41_block_max_kernel<<<dim3(blocks_for(block_capacity, kThreads), tokens), kThreads, 0,
                           static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(scores), score_stride, static_cast<float*>(block_scores),
        block_stride, static_cast<const int*>(positions), ratio, block_size);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_sparse_attention(
    const void* query,
    const void* sinks,
    const void* ring,
    int ring_rows,
    const void* compressed,
    int capacity,
    const void* indices,
    long long index_stride,
    const void* positions,
    void* out,
    int tokens,
    int heads,
    int head_dim,
    int window,
    int ratio,
    int top_k,
    float scale,
    int packed,
    void* stream) {
    const int max_slots = window + (ratio > 0 ? top_k : 0);
    const size_t shared =
        (static_cast<size_t>(kAttentionHeads) * (head_dim + max_slots) + max_slots) *
        sizeof(float);
    const int splits = (head_dim + kAttentionSplit - 1) / kAttentionSplit;
    v41_sparse_attention_kernel<<<dim3(heads / kAttentionHeads, splits, tokens), kAttentionThreads,
                                  shared,
                                  static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(query), static_cast<const float*>(sinks),
        static_cast<const uint16_t*>(ring), ring_rows, static_cast<const uint16_t*>(compressed),
        capacity, static_cast<const int*>(indices), index_stride, static_cast<const int*>(positions),
        static_cast<float*>(out), heads, head_dim, window, ratio, top_k, scale, packed);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_dense_fast(
    const void* weights,
    const void* scales,
    const void* input,
    void* out,
    int format,
    int rows,
    int cols,
    int groups,
    int tokens,
    int round_input,
    int round_bf16,
    void* stream) {
    v41_dense_fast_kernel<<<dim3(blocks_for(rows, kDenseThreads / 32), tokens), kDenseThreads, 0,
                            static_cast<cudaStream_t>(stream)>>>(
        weights, static_cast<const uint8_t*>(scales), static_cast<const float*>(input),
        static_cast<float*>(out), format, rows, cols, groups, round_input, round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_rms_norm_fast(
    const void* input,
    const void* weight,
    void* out,
    int tokens,
    int width,
    float epsilon,
    int round_bf16,
    void* stream) {
    v41_rms_norm_fast_kernel<<<tokens, 256, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(input), static_cast<const float*>(weight),
        static_cast<float*>(out), width, epsilon, round_bf16);
    return static_cast<int>(cudaGetLastError());
}
