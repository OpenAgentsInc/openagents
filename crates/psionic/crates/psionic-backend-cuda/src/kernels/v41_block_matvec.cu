// DeepSeek-V4.1 block-scaled matrix-vector kernels (#12391).
//
// Weights stay packed on the device: FP8 E4M3 with one UE8M0 scale per 32x32
// block, or FP4 E2M1 (two per byte, the low nibble holding element 2i) with one
// UE8M0 scale per 32 values along K. Activations are FP8 E4M3 with one UE8M0
// scale per 32 values, as the reference's `act_quant` writes them.
//
// Accumulation follows `fp8_gemm_kernel` and `fp4_gemm_kernel` in the
// reference's `inference/kernel.py` (revision 1bae0e1): products of raw codes
// are summed per 32-wide K block in float32, the block sum is multiplied by the
// activation scale and then the weight scale, and the scaled block sums are
// added to a second float32 accumulator in ascending block order. The
// reference sums each block on tensor cores, in an order the hardware picks;
// here one lane sums it sequentially, element 0 first, with fused
// multiply-adds. Every code product is exact in float32, so a fused and an
// unfused product give the same block sum. The scale multiplies and the outer
// additions use explicit round-to-nearest intrinsics, so the compiler can't
// contract them and a host model reproduces the result bit for bit.
//
// Nothing here is architecture-specific: no FP8 conversion instructions, no
// tensor-core or Hopper-only features. Decoding is integer and float32
// arithmetic, so the same source builds for SM89 and SM120. Build it without
// fast math: E2M1 and E4M3 decoding relies on float32 subnormals.

#include <cuda_runtime.h>
#include <math_constants.h>
#include <stdint.h>

namespace {

constexpr int kWarps = 8;
constexpr int kThreads = kWarps * 32;
constexpr int kBlock = 32;
// K blocks of activation staged in shared memory per pass (4,096 values).
constexpr int kStageBlocks = 128;
// One padding float per staged block keeps lane reads on distinct banks.
constexpr int kStageStride = kBlock + 1;
// A slot-map entry for an expert the host CPU computes (#12463).
constexpr int kLaneHost = -2;

__device__ __forceinline__ float e4m3(uint32_t byte) {
    const uint32_t magnitude = byte & 0x7fu;
    // The 7 magnitude bits placed at float32 bit 20 form 2^-120 times the
    // E4M3 value, subnormals included; scaling by 2^120 is exact.
    float value = __fmul_rn(__uint_as_float(magnitude << 20), 0x1p120f);
    if (magnitude == 0x7fu) {
        value = CUDART_NAN_F;
    }
    return __uint_as_float(__float_as_uint(value) | ((byte & 0x80u) << 24));
}

__device__ __forceinline__ float e2m1(uint32_t code) {
    // Same idea: 3 magnitude bits at float32 bit 22 are 2^-126 times the
    // E2M1 value. Code 8 is negative zero.
    const float value = __fmul_rn(__uint_as_float((code & 7u) << 22), 0x1p126f);
    return __uint_as_float(__float_as_uint(value) | ((code & 8u) << 28));
}

__device__ __forceinline__ float e8m0(uint32_t byte) {
    if (byte == 0xffu) {
        return CUDART_NAN_F;
    }
    return byte == 0 ? __uint_as_float(0x00400000u) : __uint_as_float(byte << 23);
}

// The `consumer-q` profile's formats (#12465, lossy): format ids 2 and 3
// carry 2- and 3-bit codes per value, a log-scale byte per 32-value block,
// and an f32 scale per row. v41_matvec.rs repeats these tables.
constexpr int kFormatFp8 = 0;
constexpr int kFormatFp4 = 1;
constexpr int kFormatQ2 = 2;
constexpr int kFormatQ3 = 3;
__constant__ float kQ2Levels[4] = {-1.5104f, -0.4528f, 0.4528f, 1.5104f};
__constant__ float kQ3Levels[8] = {-2.152f, -1.344f, -0.756f, -0.2451f, 0.2451f, 0.756f, 1.344f, 2.152f};
__constant__ uint32_t kQFractions[32] = {
    1065353216u, 1065536903u, 1065724611u, 1065916431u, 1066112450u, 1066312762u, 1066517459u,
    1066726640u, 1066940400u, 1067158842u, 1067382066u, 1067610179u, 1067843287u, 1068081499u,
    1068324927u, 1068573686u, 1068827891u, 1069087663u, 1069353124u, 1069624397u, 1069901610u,
    1070184894u, 1070474380u, 1070770206u, 1071072509u, 1071381432u, 1071697119u, 1072019719u,
    1072349383u, 1072686266u, 1073030525u, 1073382323u};

// row × 2^((u - 255) / 32): the power of two is exact, one rounding.
__device__ __forceinline__ float q_block_scale(float row, uint32_t u) {
    const int t = static_cast<int>(u) - 255;
    const int a = t >= 0 ? t / 32 : -((31 - t) / 32);
    const int f = t - 32 * a;
    const float power = __uint_as_float(static_cast<uint32_t>(127 + a) << 23);
    return __fmul_rn(row, __fmul_rn(__uint_as_float(kQFractions[f]), power));
}

template <int kFormat>
__host__ __device__ constexpr long long row_weight_bytes(int cols) {
    return kFormat == kFormatFp8 ? cols : kFormat == kFormatFp4 ? cols / 2 : kFormat == kFormatQ2 ? cols / 4 : cols * 3 / 8;
}

// Decodes K block `b` of a weight row into 32 floats, element 0 first.
template <int kFormat>
__device__ __forceinline__ void decode_block(const uint8_t* w_row, int b, float* w) {
    if (kFormat == kFormatFp4) {
        const uint4 packed = *reinterpret_cast<const uint4*>(w_row + b * 16);
        const uint32_t words[4] = {packed.x, packed.y, packed.z, packed.w};
#pragma unroll
        for (int q = 0; q < 4; ++q) {
#pragma unroll
            for (int t = 0; t < 4; ++t) {
                const uint32_t byte = (words[q] >> (8 * t)) & 0xffu;
                w[q * 8 + t * 2] = e2m1(byte & 15u);
                w[q * 8 + t * 2 + 1] = e2m1(byte >> 4);
            }
        }
    } else if (kFormat == kFormatFp8) {
        const uint4* p = reinterpret_cast<const uint4*>(w_row + b * 32);
        const uint4 lo = p[0];
        const uint4 hi = p[1];
        const uint32_t words[8] = {lo.x, lo.y, lo.z, lo.w, hi.x, hi.y, hi.z, hi.w};
#pragma unroll
        for (int q = 0; q < 8; ++q) {
#pragma unroll
            for (int t = 0; t < 4; ++t) {
                w[q * 4 + t] = e4m3((words[q] >> (8 * t)) & 0xffu);
            }
        }
    } else if (kFormat == kFormatQ2) {
        const uint2 packed = *reinterpret_cast<const uint2*>(w_row + b * 8);
#pragma unroll
        for (int j = 0; j < 32; ++j) {
            const uint32_t word = j < 16 ? packed.x : packed.y;
            w[j] = kQ2Levels[(word >> (2 * (j % 16))) & 3u];
        }
    } else {
        const uint32_t* p = reinterpret_cast<const uint32_t*>(w_row + b * 12);
        const uint32_t words[3] = {p[0], p[1], p[2]};
#pragma unroll
        for (int j = 0; j < 32; ++j) {
            const int bit = 3 * j;
            const int shift = bit % 32;
            uint32_t v = words[bit / 32] >> shift;
            if (shift > 29) {
                v |= words[bit / 32 + 1] << (32 - shift);
            }
            w[j] = kQ3Levels[v & 7u];
        }
    }
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

// Nearest E4M3 code, ties to the even code, saturating at 448. The caller
// passes a finite value.
__device__ __forceinline__ uint32_t encode_e4m3(float value) {
    const float magnitude = fminf(fabsf(value), 448.0f);
    uint32_t code;
    if (magnitude < 0x1p-6f) {
        // Subnormal step 2^-9; the product is exact and the conversion
        // rounds half to even.
        code = __float2uint_rn(__fmul_rn(magnitude, 512.0f));
    } else {
        const uint32_t bits = __float_as_uint(magnitude);
        const uint32_t exponent = (bits >> 23) - 120u;  // E4M3 biased exponent
        uint32_t mantissa = (bits >> 20) & 7u;
        const uint32_t rest = bits & 0xfffffu;
        if (rest > 0x80000u || (rest == 0x80000u && (mantissa & 1u))) {
            mantissa += 1u;  // a carry moves to the next exponent
        }
        code = (exponent << 3) + mantissa;
    }
    code = code > 126u ? 126u : code;
    return code | (signbit(value) ? 0x80u : 0u);
}

// One warp quantizes one block of 32 values in a row.
__global__ void __launch_bounds__(kThreads) v41_act_quant_kernel(
    const float* __restrict__ input,
    uint8_t* __restrict__ codes,
    uint8_t* __restrict__ scales,
    int rows,
    int cols) {
    const int blocks = cols / kBlock;
    const long long block = (static_cast<long long>(blockIdx.x) * kThreads + threadIdx.x) / 32;
    const int lane = threadIdx.x & 31;
    if (block >= static_cast<long long>(rows) * blocks) {
        return;  // uniform across the warp
    }
    const long long index = block * kBlock + lane;
    const float value = bf16_round(input[index]);
    bool bad = !isfinite(value);
    float amax = fabsf(value);
    for (int offset = 16; offset > 0; offset >>= 1) {
        amax = fmaxf(amax, __shfl_xor_sync(0xffffffffu, amax, offset));
    }
    bad = __any_sync(0xffffffffu, bad);
    uint32_t exponent = 0xffu;
    if (!bad) {
        // scale = 2^ceil(log2(max(amax, 1e-4) / 448)), as `fast_round_scale`.
        const uint32_t bits = __float_as_uint(__fmul_rn(fmaxf(amax, 1e-4f), 1.0f / 448.0f));
        exponent = ((bits >> 23) & 0xffu) + ((bits & 0x7fffffu) != 0u ? 1u : 0u);
        bad = exponent >= 0xffu;
    }
    if (bad) {
        // A nonfinite block gets a NaN scale and NaN codes, so every product
        // that reads it is NaN. The CPU path refuses such input instead.
        codes[index] = 0x7fu;
        if (lane == 0) {
            scales[block] = 0xffu;
        }
        return;
    }
    codes[index] = static_cast<uint8_t>(encode_e4m3(__fdiv_rn(value, __uint_as_float(exponent << 23))));
    if (lane == 0) {
        scales[block] = static_cast<uint8_t>(exponent);
    }
}

template <int kFormat>
__global__ void __launch_bounds__(kThreads) v41_block_matvec_kernel(
    const uint8_t* __restrict__ weights,
    const uint8_t* __restrict__ weight_scales,
    long long matrix_bytes,
    long long matrix_scale_bytes,
    int matrices,
    const int* __restrict__ ids,
    const uint8_t* __restrict__ act,
    const uint8_t* __restrict__ act_scales,
    int act_shared,
    float* __restrict__ out,
    int rows,
    int cols,
    int round_bf16,
    const uint8_t* __restrict__ cache_weights,
    const uint8_t* __restrict__ cache_scales,
    const int* __restrict__ slot_map) {
    __shared__ float stage[kStageBlocks * kStageStride];
    __shared__ float stage_scale[kStageBlocks];
    const int slot = blockIdx.y;
    const int warp = threadIdx.x >> 5;
    const int lane = threadIdx.x & 31;
    const int row = blockIdx.x * kWarps + warp;
    const int blocks = cols / kBlock;
    const int matrix = ids == nullptr ? 0 : ids[slot];
    bool valid = matrix >= 0 && matrix < matrices;
    // With an expert cache (#12463), the slot map says where each matrix
    // lives: a cache slot, host memory over the bus, or the host CPU lane,
    // whose output another kernel writes. The arithmetic is the same.
    const uint8_t* weight_base = weights;
    const uint8_t* scale_base = weight_scales;
    long long place = matrix;
    if (slot_map != nullptr && valid) {
        const int at = slot_map[matrix];
        if (at == kLaneHost) {
            return;  // uniform across the block: no barrier has run yet
        }
        if (at >= 0) {
            weight_base = cache_weights;
            scale_base = cache_scales;
            place = at;
        } else if (weights == nullptr) {
            valid = false;
        }
    }
    const long long input_row = act_shared ? 0 : slot;
    const uint8_t* x_codes = act + input_row * cols;
    const uint8_t* x_scales = act_scales + input_row * blocks;
    const uint8_t* w = valid ? weight_base + place * matrix_bytes : cache_weights != nullptr ? cache_weights : weights;
    const uint8_t* ws = valid ? scale_base + place * matrix_scale_bytes : cache_scales != nullptr ? cache_scales : weight_scales;
    constexpr bool kFp4 = kFormat == kFormatFp4;
    constexpr bool kQ = kFormat == kFormatQ2 || kFormat == kFormatQ3;
    const long long row_bytes = row_weight_bytes<kFormat>(cols);
    const uint8_t* w_row = w + static_cast<long long>(row) * row_bytes;
    const uint8_t* ws_row = ws + static_cast<long long>(kFormat != kFormatFp8 ? row : row / kBlock) * blocks;
    // A `consumer-q` row's own scale follows the block scales.
    const long long rows_at = (static_cast<long long>(rows) * blocks + 3) / 4 * 4;
    const float d_row = kQ && valid && row < rows ? *reinterpret_cast<const float*>(ws + rows_at + 4LL * row) : 0.0f;
    float acc = 0.0f;
    for (int base = 0; base < blocks; base += kStageBlocks) {
        const int count = min(kStageBlocks, blocks - base);
        __syncthreads();
        for (int i = threadIdx.x; i < count * kBlock; i += kThreads) {
            stage[(i / kBlock) * kStageStride + i % kBlock] =
                e4m3(x_codes[static_cast<long long>(base) * kBlock + i]);
        }
        for (int i = threadIdx.x; i < count; i += kThreads) {
            stage_scale[i] = e8m0(x_scales[base + i]);
        }
        __syncthreads();
        if (row >= rows || !valid) {
            continue;  // uniform across the warp; the barriers above still run
        }
        for (int sub = 0; sub < count; sub += 32) {
            const int local = sub + lane;
            float scaled = 0.0f;
            if (local < count) {
                const int b = base + local;
                const float* x = stage + local * kStageStride;
                float partial = 0.0f;
                if (kFp4) {
                    const uint4 packed = *reinterpret_cast<const uint4*>(w_row + b * 16);
                    const uint32_t words[4] = {packed.x, packed.y, packed.z, packed.w};
#pragma unroll
                    for (int q = 0; q < 4; ++q) {
#pragma unroll
                        for (int t = 0; t < 4; ++t) {
                            const uint32_t byte = (words[q] >> (8 * t)) & 0xffu;
                            const int j = q * 8 + t * 2;
                            partial = __fmaf_rn(x[j], e2m1(byte & 15u), partial);
                            partial = __fmaf_rn(x[j + 1], e2m1(byte >> 4), partial);
                        }
                    }
                } else if (kFormat == kFormatFp8) {
                    const uint4* p = reinterpret_cast<const uint4*>(w_row + b * 32);
                    const uint4 lo = p[0];
                    const uint4 hi = p[1];
                    const uint32_t words[8] = {lo.x, lo.y, lo.z, lo.w, hi.x, hi.y, hi.z, hi.w};
#pragma unroll
                    for (int q = 0; q < 8; ++q) {
#pragma unroll
                        for (int t = 0; t < 4; ++t) {
                            partial = __fmaf_rn(x[q * 4 + t], e4m3((words[q] >> (8 * t)) & 0xffu), partial);
                        }
                    }
                } else {
                    float wq[kBlock];
                    decode_block<kFormat>(w_row, b, wq);
#pragma unroll
                    for (int j = 0; j < kBlock; ++j) {
                        partial = __fmaf_rn(x[j], wq[j], partial);
                    }
                }
                const float weight_scale = kQ ? q_block_scale(d_row, ws_row[b]) : e8m0(ws_row[b]);
                scaled = __fmul_rn(__fmul_rn(partial, stage_scale[local]), weight_scale);
            }
            const int n = min(32, count - sub);
            for (int t = 0; t < n; ++t) {
                acc = __fadd_rn(acc, __shfl_sync(0xffffffffu, scaled, t));
            }
        }
    }
    if (row < rows && lane == 0) {
        float value = valid ? acc : CUDART_NAN_F;
        if (round_bf16) {
            value = bf16_round(value);
        }
        out[static_cast<long long>(slot) * rows + row] = value;
    }
}

// ---------------------------------------------------------------------------
// Batched products for prefill (#12464)
// ---------------------------------------------------------------------------
//
// `v41_block_matmul_kernel` computes the same products as
// `v41_block_matvec_kernel` for many activation rows at once: each block
// takes a tile of up to kTileRows rows that use the same matrix, so every
// weight block is read and decoded once for the whole tile instead of once
// per row. Each (output row, activation row) pair still sums exactly as the
// matrix-vector kernel does: a fused chain over each 32-value block from
// element 0, the block sum times the activation scale and then the weight
// scale, and the scaled sums added in ascending block order. The results
// are therefore bit-equal to one matrix-vector product per row.
//
// Work is a list of entries sorted by matrix (`entries`), cut into tiles
// (`tiles`: matrix, first entry, count). Entry `e` reads activation row
// `act_shared ? e / top_k : e` and writes output row `e`. A dense product
// uses one matrix and entries 0..T-1 with top_k 1; routed experts use the
// entries `v41_group_entries_kernel` sorts by expert.

constexpr int kTileRows = 8;
constexpr int kTileStage = 32;  // K blocks staged per pass

template <int kFormat>
__global__ void __launch_bounds__(kThreads) v41_block_matmul_kernel(
    const uint8_t* __restrict__ weights,
    const uint8_t* __restrict__ weight_scales,
    long long matrix_bytes,
    long long matrix_scale_bytes,
    int matrices,
    const int* __restrict__ entries,
    const int4* __restrict__ tiles,
    const int* __restrict__ tile_count,
    int top_k,
    int act_shared,
    const uint8_t* __restrict__ act,
    const uint8_t* __restrict__ act_scales,
    float* __restrict__ out,
    int rows,
    int cols,
    int round_bf16,
    const uint8_t* __restrict__ cache_weights,
    const uint8_t* __restrict__ cache_scales,
    const int* __restrict__ slot_map) {
    __shared__ float stage[kTileRows][kTileStage * kStageStride];
    __shared__ float stage_scale[kTileRows][kTileStage];
    __shared__ int act_rows[kTileRows];
    __shared__ int out_rows[kTileRows];
    // Each warp's scaled block sums, so lane t adds tile row t's in block
    // order instead of every lane shuffling every row's.
    __shared__ float sums[kWarps][kTileRows][32];
    if (static_cast<int>(blockIdx.y) >= *tile_count) {
        return;  // uniform: a tile past the list
    }
    const int4 tile = tiles[blockIdx.y];
    const int matrix = tile.x;
    const int first = tile.y;
    const int n = tile.z;
    const int warp = threadIdx.x >> 5;
    const int lane = threadIdx.x & 31;
    const int row = blockIdx.x * kWarps + warp;
    const int blocks = cols / kBlock;
    bool valid = matrix >= 0 && matrix < matrices;
    const uint8_t* weight_base = weights;
    const uint8_t* scale_base = weight_scales;
    long long place = matrix;
    if (slot_map != nullptr && valid) {
        const int at = slot_map[matrix];
        if (at == kLaneHost) {
            return;  // uniform across the block
        }
        if (at >= 0) {
            weight_base = cache_weights;
            scale_base = cache_scales;
            place = at;
        } else if (weights == nullptr) {
            valid = false;
        }
    }
    if (threadIdx.x < kTileRows) {
        const int t = threadIdx.x;
        const int e = t < n ? entries[first + t] : 0;
        act_rows[t] = act_shared ? e / top_k : e;
        out_rows[t] = e;
    }
    constexpr bool kQ = kFormat == kFormatQ2 || kFormat == kFormatQ3;
    const long long row_bytes = row_weight_bytes<kFormat>(cols);
    const uint8_t* w_row = valid ? weight_base + place * matrix_bytes + static_cast<long long>(row) * row_bytes : nullptr;
    const uint8_t* ws_row = valid
        ? scale_base + place * matrix_scale_bytes + static_cast<long long>(kFormat != kFormatFp8 ? row : row / kBlock) * blocks
        : nullptr;
    const long long rows_at = (static_cast<long long>(rows) * blocks + 3) / 4 * 4;
    const float d_row = kQ && valid && row < rows
        ? *reinterpret_cast<const float*>(scale_base + place * matrix_scale_bytes + rows_at + 4LL * row)
        : 0.0f;
    // Lane t holds tile row t's running sum.
    float acc = 0.0f;
    for (int base = 0; base < blocks; base += kTileStage) {
        const int count = min(kTileStage, blocks - base);
        __syncthreads();
        for (int t = 0; t < n; ++t) {
            const long long r = act_rows[t];
            for (int i = threadIdx.x; i < count * kBlock; i += kThreads) {
                stage[t][(i / kBlock) * kStageStride + i % kBlock] =
                    e4m3(act[r * cols + static_cast<long long>(base) * kBlock + i]);
            }
            for (int i = threadIdx.x; i < count; i += kThreads) {
                stage_scale[t][i] = e8m0(act_scales[r * blocks + base + i]);
            }
        }
        __syncthreads();
        if (row >= rows || !valid) {
            continue;  // uniform across the warp; the barriers above still run
        }
        float scaled[kTileRows];
        if (lane < count) {
            const int b = base + lane;
            float w[kBlock];
            decode_block<kFormat>(w_row, b, w);
            const float weight_scale = kQ ? q_block_scale(d_row, ws_row[b]) : e8m0(ws_row[b]);
#pragma unroll
            for (int t = 0; t < kTileRows; ++t) {
                if (t < n) {
                    const float* x = &stage[t][lane * kStageStride];
                    float partial = 0.0f;
#pragma unroll
                    for (int j = 0; j < kBlock; ++j) {
                        partial = __fmaf_rn(x[j], w[j], partial);
                    }
                    scaled[t] = __fmul_rn(__fmul_rn(partial, stage_scale[t][lane]), weight_scale);
                } else {
                    scaled[t] = 0.0f;
                }
            }
        } else {
#pragma unroll
            for (int t = 0; t < kTileRows; ++t) {
                scaled[t] = 0.0f;
            }
        }
#pragma unroll
        for (int t = 0; t < kTileRows; ++t) {
            sums[warp][t][lane] = scaled[t];
        }
        __syncwarp();
        if (lane < n) {
            for (int i = 0; i < count; ++i) {
                acc = __fadd_rn(acc, sums[warp][lane][i]);
            }
        }
        __syncwarp();
    }
    if (row < rows && lane < n) {
        float value = valid ? acc : CUDART_NAN_F;
        if (round_bf16) {
            value = bf16_round(value);
        }
        out[static_cast<long long>(out_rows[lane]) * rows + row] = value;
    }
}

// Sorts `count` routed entries (ids[e] is entry e's expert) by expert into
// `entries`, and cuts each expert's run into tiles of at most kTileRows.
// Ids outside [0, experts) go to a last run whose tiles carry matrix -1,
// which the product writes as NaN. One block; `scratch` holds
// 2 × (experts + 1) ints.
__global__ void __launch_bounds__(1024) v41_group_entries_kernel(
    const int* __restrict__ ids,
    int count,
    int experts,
    int* __restrict__ scratch,
    int* __restrict__ entries,
    int4* __restrict__ tiles,
    int* __restrict__ tile_count) {
    int* counts = scratch;
    int* cursor = scratch + experts + 1;
    for (int i = threadIdx.x; i <= experts; i += blockDim.x) {
        counts[i] = 0;
    }
    __syncthreads();
    for (int e = threadIdx.x; e < count; e += blockDim.x) {
        const int id = ids[e];
        atomicAdd(&counts[(id >= 0 && id < experts) ? id : experts], 1);
    }
    __syncthreads();
    if (threadIdx.x == 0) {
        int at = 0;
        int n = 0;
        for (int m = 0; m <= experts; ++m) {
            cursor[m] = at;
            for (int first = 0; first < counts[m]; first += kTileRows) {
                tiles[n++] = make_int4(m == experts ? -1 : m, at + first, min(kTileRows, counts[m] - first), 0);
            }
            at += counts[m];
        }
        *tile_count = n;
    }
    __syncthreads();
    for (int e = threadIdx.x; e < count; e += blockDim.x) {
        const int id = ids[e];
        const int m = (id >= 0 && id < experts) ? id : experts;
        entries[atomicAdd(&cursor[m], 1)] = e;
    }
}


// Calibration for the `consumer-q` quantizer (#12465): adds each entry's
// squared FP8 activations to its expert's per-column sums, and counts the
// entry, so a quantizer can weigh each column by the mean square it sees.
__global__ void v41_importance_kernel(
    const int* __restrict__ ids,
    int top_k,
    int act_shared,
    const uint8_t* __restrict__ act,
    const uint8_t* __restrict__ act_scales,
    int cols,
    int experts,
    float* __restrict__ sums,
    float* __restrict__ counts) {
    const int e = blockIdx.x;
    const int expert = ids[e];
    if (expert < 0 || expert >= experts) {
        return;
    }
    const long long row = act_shared ? e / top_k : e;
    const int c = blockIdx.y * blockDim.x + threadIdx.x;
    if (c < cols) {
        const float x = __fmul_rn(e4m3(act[row * cols + c]), e8m0(act_scales[row * (cols / kBlock) + c / kBlock]));
        atomicAdd(&sums[static_cast<long long>(expert) * cols + c], __fmul_rn(x, x));
    }
    if (c == 0) {
        atomicAdd(&counts[expert], 1.0f);
    }
}

}  // namespace

extern "C" int psionic_v41_block_matmul(
    int fp4,
    const void* weights,
    const void* weight_scales,
    long long matrix_bytes,
    long long matrix_scale_bytes,
    int matrices,
    const void* entries,
    const void* tiles,
    const void* tile_count,
    int max_tiles,
    int top_k,
    int act_shared,
    const void* act,
    const void* act_scales,
    void* out,
    int rows,
    int cols,
    int round_bf16,
    const void* cache_weights,
    const void* cache_scales,
    const void* slot_map,
    void* stream) {
    const dim3 grid(static_cast<unsigned>((rows + kWarps - 1) / kWarps), static_cast<unsigned>(max_tiles));
    const cudaStream_t s = static_cast<cudaStream_t>(stream);
#define V41_MATMUL(F)                                                                              \
    v41_block_matmul_kernel<F><<<grid, kThreads, 0, s>>>(                                          \
        static_cast<const uint8_t*>(weights), static_cast<const uint8_t*>(weight_scales),          \
        matrix_bytes, matrix_scale_bytes, matrices, static_cast<const int*>(entries),              \
        static_cast<const int4*>(tiles), static_cast<const int*>(tile_count), top_k, act_shared,   \
        static_cast<const uint8_t*>(act), static_cast<const uint8_t*>(act_scales),                 \
        static_cast<float*>(out), rows, cols, round_bf16,                                          \
        static_cast<const uint8_t*>(cache_weights), static_cast<const uint8_t*>(cache_scales),     \
        static_cast<const int*>(slot_map))
    // `fp4` carries the format id: 0 FP8, 1 FP4, 2 Q2, 3 Q3 (#12465).
    switch (fp4) {
        case kFormatFp8: V41_MATMUL(kFormatFp8); break;
        case kFormatFp4: V41_MATMUL(kFormatFp4); break;
        case kFormatQ2: V41_MATMUL(kFormatQ2); break;
        case kFormatQ3: V41_MATMUL(kFormatQ3); break;
        default: return static_cast<int>(cudaErrorInvalidValue);
    }
#undef V41_MATMUL
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_group_entries(
    const void* ids,
    int count,
    int experts,
    void* scratch,
    void* entries,
    void* tiles,
    void* tile_count,
    void* stream) {
    v41_group_entries_kernel<<<1, 1024, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const int*>(ids), count, experts, static_cast<int*>(scratch),
        static_cast<int*>(entries), static_cast<int4*>(tiles), static_cast<int*>(tile_count));
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_act_quant_fp8(
    const void* input,
    void* codes,
    void* scales,
    int rows,
    int cols,
    void* stream) {
    const long long warps = static_cast<long long>(rows) * (cols / kBlock);
    const long long grid = (warps + kWarps - 1) / kWarps;
    v41_act_quant_kernel<<<static_cast<unsigned>(grid), kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(input),
        static_cast<uint8_t*>(codes),
        static_cast<uint8_t*>(scales),
        rows,
        cols);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_block_matvec(
    int fp4,
    const void* weights,
    const void* weight_scales,
    long long matrix_bytes,
    long long matrix_scale_bytes,
    int matrices,
    const void* ids,
    const void* act,
    const void* act_scales,
    int act_shared,
    void* out,
    int rows,
    int cols,
    int slots,
    int round_bf16,
    void* stream) {
    const dim3 grid(static_cast<unsigned>((rows + kWarps - 1) / kWarps), static_cast<unsigned>(slots));
    const cudaStream_t s = static_cast<cudaStream_t>(stream);
#define V41_MATVEC(F)                                                                              \
    v41_block_matvec_kernel<F><<<grid, kThreads, 0, s>>>(                                          \
        static_cast<const uint8_t*>(weights), static_cast<const uint8_t*>(weight_scales),          \
        matrix_bytes, matrix_scale_bytes, matrices, static_cast<const int*>(ids),                  \
        static_cast<const uint8_t*>(act), static_cast<const uint8_t*>(act_scales), act_shared,     \
        static_cast<float*>(out), rows, cols, round_bf16, nullptr, nullptr, nullptr)
    // `fp4` carries the format id: 0 FP8, 1 FP4, 2 Q2, 3 Q3 (#12465).
    switch (fp4) {
        case kFormatFp8: V41_MATVEC(kFormatFp8); break;
        case kFormatFp4: V41_MATVEC(kFormatFp4); break;
        case kFormatQ2: V41_MATVEC(kFormatQ2); break;
        case kFormatQ3: V41_MATVEC(kFormatQ3); break;
        default: return static_cast<int>(cudaErrorInvalidValue);
    }
#undef V41_MATVEC
    return static_cast<int>(cudaGetLastError());
}

// The cached form (#12463): `ids` holds expert ids, `slot_map` maps each of
// the `experts` ids to a slot of the cache stacks, the bus lane (read from
// `weights`, which may be null when no expert uses it), or the host lane
// (skipped; another kernel writes those rows).
extern "C" int psionic_v41_block_matvec_cached(
    int format,
    const void* weights,
    const void* weight_scales,
    const void* cache_weights,
    const void* cache_scales,
    const void* slot_map,
    long long matrix_bytes,
    long long matrix_scale_bytes,
    int experts,
    const void* ids,
    const void* act,
    const void* act_scales,
    int act_shared,
    void* out,
    int rows,
    int cols,
    int slots,
    int round_bf16,
    void* stream) {
    const dim3 grid(static_cast<unsigned>((rows + kWarps - 1) / kWarps), static_cast<unsigned>(slots));
    const cudaStream_t s = static_cast<cudaStream_t>(stream);
#define V41_CACHED(F)                                                                              \
    v41_block_matvec_kernel<F><<<grid, kThreads, 0, s>>>(                                          \
        static_cast<const uint8_t*>(weights), static_cast<const uint8_t*>(weight_scales),          \
        matrix_bytes, matrix_scale_bytes, experts, static_cast<const int*>(ids),                   \
        static_cast<const uint8_t*>(act), static_cast<const uint8_t*>(act_scales), act_shared,     \
        static_cast<float*>(out), rows, cols, round_bf16,                                          \
        static_cast<const uint8_t*>(cache_weights), static_cast<const uint8_t*>(cache_scales),     \
        static_cast<const int*>(slot_map))
    switch (format) {
        case kFormatFp4: V41_CACHED(kFormatFp4); break;
        case kFormatQ2: V41_CACHED(kFormatQ2); break;
        case kFormatQ3: V41_CACHED(kFormatQ3); break;
        default: return static_cast<int>(cudaErrorInvalidValue);
    }
#undef V41_CACHED
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_importance(
    const void* ids,
    int count,
    int top_k,
    int act_shared,
    const void* act,
    const void* act_scales,
    int cols,
    int experts,
    void* sums,
    void* counts,
    void* stream) {
    const dim3 grid(static_cast<unsigned>(count), static_cast<unsigned>((cols + 255) / 256));
    v41_importance_kernel<<<grid, 256, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const int*>(ids), top_k, act_shared, static_cast<const uint8_t*>(act),
        static_cast<const uint8_t*>(act_scales), cols, experts, static_cast<float*>(sums),
        static_cast<float*>(counts));
    return static_cast<int>(cudaGetLastError());
}
