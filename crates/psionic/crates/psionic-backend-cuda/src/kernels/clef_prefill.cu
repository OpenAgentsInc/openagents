// Sequence-prefill kernels for the Clef decision lane (qwen35 hybrid trunk +
// joint-head memory), OpenAgentsInc/openagents #11195.
//
// The trunk runs a chunk of tokens through one layer at a time:
// projections are dequantize-to-f16 + cuBLAS GEMMs (launched from Rust);
// these kernels do everything between the GEMMs. Every reduction has a fixed
// order (no atomics), so a repeated launch is bitwise identical.

#include <cuda_fp16.h>
#include <cuda_fp8.h>
#include <cuda_runtime.h>
#include <stdint.h>

namespace {

constexpr int kWarp = 32;

__device__ __forceinline__ float warp_sum(float value) {
    for (int offset = kWarp / 2; offset > 0; offset >>= 1) {
        value += __shfl_xor_sync(0xffffffffu, value, offset);
    }
    return value;
}

// Block-wide sum for blockDim.x a multiple of 32 (<= 1024); every thread
// gets the result.
__device__ __forceinline__ float block_sum(float value, float *scratch) {
    const int lane = threadIdx.x % kWarp;
    const int warp = threadIdx.x / kWarp;
    value = warp_sum(value);
    __syncthreads();
    if (lane == 0) {
        scratch[warp] = value;
    }
    __syncthreads();
    const int warps = (blockDim.x + kWarp - 1) / kWarp;
    float total = 0.0f;
    for (int i = 0; i < warps; ++i) {
        total += scratch[i];
    }
    return total;
}

__device__ __forceinline__ float block_max(float value, float *scratch) {
    const int lane = threadIdx.x % kWarp;
    const int warp = threadIdx.x / kWarp;
    for (int offset = kWarp / 2; offset > 0; offset >>= 1) {
        value = fmaxf(value, __shfl_xor_sync(0xffffffffu, value, offset));
    }
    __syncthreads();
    if (lane == 0) {
        scratch[warp] = value;
    }
    __syncthreads();
    const int warps = (blockDim.x + kWarp - 1) / kWarp;
    float best = -INFINITY;
    for (int i = 0; i < warps; ++i) {
        best = fmaxf(best, scratch[i]);
    }
    return best;
}

__device__ __forceinline__ float load_half(const uint8_t *bytes) {
    __half value;
    memcpy(&value, bytes, sizeof(value));
    return __half2float(value);
}

__device__ __forceinline__ void cp_async16(void *shared, const void *global, int bytes) {
    const unsigned address = static_cast<unsigned>(__cvta_generic_to_shared(shared));
    asm volatile("cp.async.cg.shared.global [%0], [%1], 16, %2;\n" ::"r"(address), "l"(global), "r"(bytes));
}

__device__ __forceinline__ void cp_async_commit() { asm volatile("cp.async.commit_group;\n" ::); }

template <int N>
__device__ __forceinline__ void cp_async_wait() {
    asm volatile("cp.async.wait_group %0;\n" ::"n"(N));
}

__device__ __forceinline__ float silu(float x) { return x / (1.0f + expf(-x)); }
__device__ __forceinline__ float sigmoid(float x) { return 1.0f / (1.0f + expf(-x)); }

// ---- dequantization (GGML block formats) to row-major f16 ----

// Q8_0: 34-byte blocks of 32 (f16 scale, 32 x int8). One thread per four
// consecutive outputs, so stores are coalesced.
__global__ void dequant_q8_0_kernel(const uint8_t *src, __half *dst, long long groups) {
    const long long g = blockIdx.x * (long long)blockDim.x + threadIdx.x;
    if (g >= groups) {
        return;
    }
    const long long block = g / 8;
    const int within = static_cast<int>(g % 8) * 4;
    const uint8_t *b = src + block * 34;
    const float d = load_half(b);
    const uint16_t lo = *reinterpret_cast<const uint16_t *>(b + 2 + within);
    const uint16_t hi = *reinterpret_cast<const uint16_t *>(b + 4 + within);
    const int8_t q0 = static_cast<int8_t>(lo & 0xff), q1 = static_cast<int8_t>(lo >> 8);
    const int8_t q2 = static_cast<int8_t>(hi & 0xff), q3 = static_cast<int8_t>(hi >> 8);
    __half2 *out = reinterpret_cast<__half2 *>(dst + g * 4);
    out[0] = __floats2half2_rn(d * q0, d * q1);
    out[1] = __floats2half2_rn(d * q2, d * q3);
}

// Q4_K: 144-byte super-blocks of 256 (f16 d, f16 dmin, 12 scale bytes,
// 128 nibble bytes). Sub-block j (64 values) takes the low nibbles of
// qs[32j..32j+32] (scale 2j), then the high nibbles (scale 2j+1). One
// thread per eight consecutive outputs.
__global__ void dequant_q4_k_kernel(const uint8_t *src, __half *dst, long long groups) {
    const long long g = blockIdx.x * (long long)blockDim.x + threadIdx.x;
    if (g >= groups) {
        return;
    }
    const long long block = g / 32;
    const int e = static_cast<int>(g % 32) * 8;  // element within the super-block
    const int j = e / 64;
    const int r = e % 64;
    const bool high = r >= 32;
    const int is = 2 * j + (high ? 1 : 0);
    const uint8_t *b = src + block * 144;
    const float d = load_half(b);
    const float dmin = load_half(b + 2);
    const uint8_t *scales = b + 4;
    uint8_t sc, m;
    if (is < 4) {
        sc = scales[is] & 63;
        m = scales[is + 4] & 63;
    } else {
        sc = (scales[is + 4] & 0xF) | ((scales[is - 4] >> 6) << 4);
        m = (scales[is + 4] >> 4) | ((scales[is] >> 6) << 4);
    }
    const float scale = d * sc, minimum = dmin * m;
    const uint2 packed = *reinterpret_cast<const uint2 *>(b + 16 + 32 * j + (r % 32));
    const uint32_t words[2] = {packed.x, packed.y};
    __half2 out[4];
#pragma unroll
    for (int w = 0; w < 2; ++w) {
        uint32_t v = words[w];
        if (high) {
            v >>= 4;
        }
        const float a0 = scale * ((v >> 0) & 0xF) - minimum;
        const float a1 = scale * ((v >> 8) & 0xF) - minimum;
        const float a2 = scale * ((v >> 16) & 0xF) - minimum;
        const float a3 = scale * ((v >> 24) & 0xF) - minimum;
        out[2 * w] = __floats2half2_rn(a0, a1);
        out[2 * w + 1] = __floats2half2_rn(a2, a3);
    }
    *reinterpret_cast<uint4 *>(dst + g * 8) = *reinterpret_cast<const uint4 *>(out);
}

// Token embedding rows gathered from a Q4_K table on the device (#11444):
// out[t, :] = row ids[t] of the [vocab, hidden] table, in f32 with the host
// decoder's arithmetic (d * sc * q - dmin * m, each product and the
// difference rounded once), so the rows are bitwise the host's. Block t,
// four consecutive elements per thread and step.
__global__ void embed_q4_k_kernel(const uint8_t *__restrict__ table, const int *__restrict__ ids,
                                  float *__restrict__ out, int hidden, long long row_bytes) {
    const int t = blockIdx.x;
    const uint8_t *row = table + static_cast<long long>(ids[t]) * row_bytes;
    float *dst = out + static_cast<long long>(t) * hidden;
    for (int e = threadIdx.x * 4; e < hidden; e += blockDim.x * 4) {
        const uint8_t *b = row + (e / 256) * 144;
        const int within = e % 256;
        const int j = within / 64;
        const int r = within % 64;
        const bool high = r >= 32;
        const int is = 2 * j + (high ? 1 : 0);
        const float d = load_half(b);
        const float dmin = load_half(b + 2);
        const uint8_t *scales = b + 4;
        uint8_t sc, m;
        if (is < 4) {
            sc = scales[is] & 63;
            m = scales[is + 4] & 63;
        } else {
            sc = (scales[is + 4] & 0xF) | ((scales[is - 4] >> 6) << 4);
            m = (scales[is + 4] >> 4) | ((scales[is] >> 6) << 4);
        }
        const float scale = __fmul_rn(d, static_cast<float>(sc));
        const float minimum = __fmul_rn(dmin, static_cast<float>(m));
        uint32_t v = *reinterpret_cast<const uint32_t *>(b + 16 + 32 * j + (r % 32));
        if (high) {
            v >>= 4;
        }
        float4 o;
        o.x = __fsub_rn(__fmul_rn(scale, static_cast<float>(v & 0xF)), minimum);
        o.y = __fsub_rn(__fmul_rn(scale, static_cast<float>((v >> 8) & 0xF)), minimum);
        o.z = __fsub_rn(__fmul_rn(scale, static_cast<float>((v >> 16) & 0xF)), minimum);
        o.w = __fsub_rn(__fmul_rn(scale, static_cast<float>((v >> 24) & 0xF)), minimum);
        *reinterpret_cast<float4 *>(dst + e) = o;
    }
}

// Dense f32 rows to f16.
__global__ void f32_to_f16_kernel(const float *src, __half *dst, long long count) {
    const long long i = blockIdx.x * (long long)blockDim.x + threadIdx.x;
    if (i < count) {
        dst[i] = __float2half_rn(src[i]);
    }
}

// dst (+)= f32(src).
__global__ void f16_to_f32_kernel(const __half *src, float *dst, long long count, int accumulate) {
    const long long i = blockIdx.x * (long long)blockDim.x + threadIdx.x;
    if (i < count) {
        const float v = __half2float(src[i]);
        dst[i] = accumulate ? dst[i] + v : v;
    }
}

// ---- norms ----

// out16[t] = RMSNorm(x[t]) * w (f16), one block per row.
__global__ void rms_norm_to_f16_kernel(const float *x, const float *w, __half *out, int d, float eps) {
    __shared__ float scratch[32];
    const float *row = x + static_cast<long long>(blockIdx.x) * d;
    float sum = 0.0f;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        sum += row[i] * row[i];
    }
    const float scale = rsqrtf(block_sum(sum, scratch) / d + eps);
    __half *dst = out + static_cast<long long>(blockIdx.x) * d;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        dst[i] = __float2half_rn(row[i] * scale * w[i]);
    }
}

// out[t] = RMSNorm(x[t]) * w (f32).
__global__ void rms_norm_f32_kernel(const float *x, const float *w, float *out, int d, float eps) {
    __shared__ float scratch[32];
    const float *row = x + static_cast<long long>(blockIdx.x) * d;
    float sum = 0.0f;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        sum += row[i] * row[i];
    }
    const float scale = rsqrtf(block_sum(sum, scratch) / d + eps);
    float *dst = out + static_cast<long long>(blockIdx.x) * d;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        dst[i] = row[i] * scale * w[i];
    }
}

// out[t] = LayerNorm(x[t]) * w + b (f32, two-pass mean/variance), one
// block per row; `w`/`b` may be null (plain normalization).
__global__ void layer_norm_f32_kernel(const float *x, const float *w, const float *b, float *out, int d, float eps) {
    __shared__ float scratch[32];
    const float *row = x + static_cast<long long>(blockIdx.x) * d;
    float sum = 0.0f;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        sum += row[i];
    }
    const float mean = block_sum(sum, scratch) / d;
    float var = 0.0f;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        const float c = row[i] - mean;
        var += c * c;
    }
    const float inv = rsqrtf(block_sum(var, scratch) / d + eps);
    float *dst = out + static_cast<long long>(blockIdx.x) * d;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        const float v = (row[i] - mean) * inv;
        dst[i] = w ? v * w[i] + b[i] : v;
    }
}

// ---- Gated DeltaNet ----

// Causal depthwise conv (kernel k <= 4) + SiLU over n tokens. Output t
// reads inputs t-k+1..t, the earliest from the carried state (k-1 inputs
// per channel, oldest first), so every (t, channel) is independent.
__global__ void conv1d_seq_silu_kernel(const float *in, const float *state, const float *w, float *out, int n, int channels, int k) {
    const long long index = blockIdx.x * (long long)blockDim.x + threadIdx.x;
    if (index >= static_cast<long long>(n) * channels) {
        return;
    }
    const int t = static_cast<int>(index / channels);
    const int c = static_cast<int>(index % channels);
    const int s = k - 1;
    float acc = 0.0f;
    for (int j = 0; j < s; ++j) {
        // tap j reads input t - s + j
        const int source = t - s + j;
        const float x = source >= 0 ? in[static_cast<long long>(source) * channels + c] : state[c * s + (s + source)];
        acc += x * w[c * k + j];
    }
    acc += in[index] * w[c * k + s];
    out[index] = silu(acc);
}

// The same conv, one thread per channel over a run of kConvRun tokens with
// the window in registers (#11444): every input is read once and the
// arithmetic is conv1d_seq_silu_kernel's, term for term, so the output is
// bitwise the same.
constexpr int kConvRun = 16;
template <int K>
__global__ void conv1d_seq_silu_run_kernel(const float *__restrict__ in, const float *__restrict__ state,
                                           const float *__restrict__ w, float *__restrict__ out, int n,
                                           int channels) {
    const int c = blockIdx.x * blockDim.x + threadIdx.x;
    if (c >= channels) {
        return;
    }
    constexpr int s = K - 1;
    const int t0 = blockIdx.y * kConvRun;
    float wk[K];
#pragma unroll
    for (int j = 0; j < K; ++j) {
        wk[j] = w[c * K + j];
    }
    // window[j] holds input t - s + j for the next output t
    float window[K];
#pragma unroll
    for (int j = 0; j < s; ++j) {
        const int source = t0 - s + j;
        window[j] = source >= 0 ? in[static_cast<long long>(source) * channels + c] : state[c * s + (s + source)];
    }
    const int end = min(t0 + kConvRun, n);
    for (int t = t0; t < end; ++t) {
        window[s] = in[static_cast<long long>(t) * channels + c];
        float acc = 0.0f;
#pragma unroll
        for (int j = 0; j < s; ++j) {
            acc += window[j] * wk[j];
        }
        acc += window[s] * wk[s];
        out[static_cast<long long>(t) * channels + c] = silu(acc);
#pragma unroll
        for (int j = 0; j < s; ++j) {
            window[j] = window[j + 1];
        }
    }
}

// The carried conv state after n tokens: the last k-1 inputs (from the
// chunk, or the old state when n < k-1). Runs after the conv.
__global__ void conv1d_state_kernel(const float *in, float *state, int n, int channels, int k) {
    const int c = blockIdx.x * blockDim.x + threadIdx.x;
    if (c >= channels) {
        return;
    }
    const int s = k - 1;
    float next[3] = {0.0f, 0.0f, 0.0f};
    for (int j = 0; j < s; ++j) {
        const int source = n - s + j;
        next[j] = source >= 0 ? in[static_cast<long long>(source) * channels + c] : state[c * s + (s + source)];
    }
    for (int j = 0; j < s; ++j) {
        state[c * s + j] = next[j];
    }
}

// The conv state after the first n of a chunk's tokens, written to `out`
// (the carried `state` is left alone): conv1d_state_kernel's arithmetic for
// a prefix snapshot inside the chunk (#11442).
__global__ void conv1d_state_into_kernel(const float *in, const float *state, float *out, int n, int channels, int k) {
    const int c = blockIdx.x * blockDim.x + threadIdx.x;
    if (c >= channels) {
        return;
    }
    const int s = k - 1;
    for (int j = 0; j < s; ++j) {
        const int source = n - s + j;
        out[c * s + j] = source >= 0 ? in[static_cast<long long>(source) * channels + c] : state[c * s + (s + source)];
    }
}

// L2-normalize q and k per key head (q also scaled by 1/sqrt(dim)); decay
// and beta per value head. Block (t, key head), dim threads (dim <= 1024).
__global__ void delta_prep_kernel(const float *conv, const float *alpha, const float *beta_in,
                                  const float *ssm_a, const float *ssm_dt, float *qn, float *kn,
                                  float *decay, float *beta, float *kq, int key_heads, int value_heads, int dim,
                                  int conv_width) {
    __shared__ float scratch[32];
    const int t = blockIdx.x;
    const int h = blockIdx.y;
    const float *row = conv + static_cast<long long>(t) * conv_width;
    const int i = threadIdx.x;
    const float q = i < dim ? row[h * dim + i] : 0.0f;
    const float k = i < dim ? row[key_heads * dim + h * dim + i] : 0.0f;
    // Match Qwen3.5/FLA: epsilon belongs inside the squared norm.
    const float qinv = rsqrtf(block_sum(q * q, scratch) + 1e-6f);
    const float kinv = rsqrtf(block_sum(k * k, scratch) + 1e-6f);
    const float qv = q * qinv * rsqrtf(static_cast<float>(dim));
    const float kv = k * kinv;
    if (i < dim) {
        const long long base = (static_cast<long long>(t) * key_heads + h) * dim + i;
        qn[base] = qv;
        kn[base] = kv;
    }
    const float dot = block_sum(i < dim ? qv * kv : 0.0f, scratch);
    if (i == 0) {
        kq[static_cast<long long>(t) * key_heads + h] = dot;
    }
    // value heads: key head h handles value heads h, h + key_heads, ...
    for (int v = h; v < value_heads; v += key_heads) {
        if (i == 0) {
            const float a = alpha[static_cast<long long>(t) * value_heads + v] + ssm_dt[v];
            const float sp = a > 20.0f ? a : logf(1.0f + expf(a));
            decay[static_cast<long long>(t) * value_heads + v] = expf(sp * ssm_a[v]);
            beta[static_cast<long long>(t) * value_heads + v] = sigmoid(beta_in[static_cast<long long>(t) * value_heads + v]);
        }
    }
}

// The delta rule over n tokens, sequential in t. One warp per (value head,
// value row); a lane holds dim/32 state entries in registers. State layout
// [value_heads, dim(row), dim(key)], the decode kernel's layout.
//
// Per token, with S' = g S:  kv = S'.k,  delta = (v - kv) b,
// S'' = S' + delta k,  o = S''.q = S'.q + delta (k.q).
// S'.k and S'.q reduce together, and k.q comes from the prep kernel, so a
// token costs one warp reduction instead of two.
template <int PER_LANE, int ROWS>
__global__ void delta_seq_kernel(const float *qn, const float *kn, const float *conv, const float *decay,
                                 const float *beta, const float *kq, float *state, float *out, int n, int key_heads,
                                 int value_heads, int v_head_reordered, int conv_width, int value_offset) {
    // ROWS value rows per warp: independent reductions interleave, and the
    // grid fits one wave of resident warps.
    constexpr int dim = PER_LANE * kWarp;
    const int warp_global = (blockIdx.x * blockDim.x + threadIdx.x) / kWarp;
    const int lane = threadIdx.x % kWarp;
    const int vh = (warp_global * ROWS) / dim;
    const int row0 = (warp_global * ROWS) % dim;
    if (vh >= value_heads) {
        return;
    }
    const int repeat = value_heads / key_heads;
    const int kh = v_head_reordered ? vh % key_heads : vh / repeat;
    float reg[ROWS][PER_LANE];
#pragma unroll
    for (int r = 0; r < ROWS; ++r) {
        const float *s = state + (static_cast<long long>(vh) * dim + row0 + r) * dim;
#pragma unroll
        for (int j = 0; j < PER_LANE; ++j) {
            reg[r][j] = s[lane + kWarp * j];
        }
    }
    for (int t = 0; t < n; ++t) {
        const long long kb = (static_cast<long long>(t) * key_heads + kh) * dim;
        const float g = decay[static_cast<long long>(t) * value_heads + vh];
        const float b = beta[static_cast<long long>(t) * value_heads + vh];
        const float k_dot_q = kq[static_cast<long long>(t) * key_heads + kh];
        const float *vrow = conv + static_cast<long long>(t) * conv_width + value_offset + vh * dim + row0;
        float kreg[PER_LANE], qreg[PER_LANE];
#pragma unroll
        for (int j = 0; j < PER_LANE; ++j) {
            kreg[j] = kn[kb + lane + kWarp * j];
            qreg[j] = qn[kb + lane + kWarp * j];
        }
        float sk[ROWS], sq[ROWS];
#pragma unroll
        for (int r = 0; r < ROWS; ++r) {
            sk[r] = 0.0f;
            sq[r] = 0.0f;
#pragma unroll
            for (int j = 0; j < PER_LANE; ++j) {
                reg[r][j] *= g;
                sk[r] += reg[r][j] * kreg[j];
                sq[r] += reg[r][j] * qreg[j];
            }
        }
#pragma unroll
        for (int offset = kWarp / 2; offset > 0; offset >>= 1) {
#pragma unroll
            for (int r = 0; r < ROWS; ++r) {
                sk[r] += __shfl_xor_sync(0xffffffffu, sk[r], offset);
                sq[r] += __shfl_xor_sync(0xffffffffu, sq[r], offset);
            }
        }
#pragma unroll
        for (int r = 0; r < ROWS; ++r) {
            const float delta = (vrow[r] - sk[r]) * b;
#pragma unroll
            for (int j = 0; j < PER_LANE; ++j) {
                reg[r][j] += kreg[j] * delta;
            }
            if (lane == r) {
                out[(static_cast<long long>(t) * value_heads + vh) * dim + row0 + r] = sq[r] + delta * k_dot_q;
            }
        }
    }
#pragma unroll
    for (int r = 0; r < ROWS; ++r) {
        float *s = state + (static_cast<long long>(vh) * dim + row0 + r) * dim;
#pragma unroll
        for (int j = 0; j < PER_LANE; ++j) {
            s[lane + kWarp * j] = reg[r][j];
        }
    }
}

// The same delta rule for dim 128, staged: a CTA owns 32 state rows of one
// value head (4 warps; 8 lanes per row, each lane 16 key entries of 2
// rows), and the token inputs (k, q, the CTA's 32 values, decay, beta, k.q)
// come in tiles of kScanTile tokens through shared memory (cp.async, double
// buffered), so the serial per-token chain is arithmetic and a 3-level
// shuffle instead of global-load latency and a 5-level one.
namespace scan {
constexpr int kDim = 128;
constexpr int kRowsPerCta = 32;
constexpr int kThreads = 128;
constexpr int kTile = 16;
}  // namespace scan

__device__ __forceinline__ void cp_async4(void *shared, const void *global) {
    const unsigned address = static_cast<unsigned>(__cvta_generic_to_shared(shared));
    asm volatile("cp.async.ca.shared.global [%0], [%1], 4;\n" ::"r"(address), "l"(global));
}

__global__ void __launch_bounds__(scan::kThreads)
    delta_scan128_kernel(const float *__restrict__ qn, const float *__restrict__ kn, const float *__restrict__ conv,
                         const float *__restrict__ decay, const float *__restrict__ beta, const float *__restrict__ kq,
                         float *__restrict__ state, float *__restrict__ out, int n, int key_heads, int value_heads,
                         int v_head_reordered, int conv_width, int value_offset) {
    using namespace scan;
    __shared__ __align__(16) float ks[2][kTile][kDim];
    __shared__ __align__(16) float qs[2][kTile][kDim];
    __shared__ __align__(16) float vs[2][kTile][kRowsPerCta];
    __shared__ float gs[2][kTile], bs[2][kTile], kqs[2][kTile];
    const int blocks_per_head = kDim / kRowsPerCta;
    const int vh = blockIdx.x / blocks_per_head;
    const int row0 = (blockIdx.x % blocks_per_head) * kRowsPerCta;
    const int repeat = value_heads / key_heads;
    const int kh = v_head_reordered ? vh % key_heads : vh / repeat;
    const int tid = threadIdx.x;
    const int lane = tid % kWarp;
    const int warp = tid / kWarp;
    const int group = lane / 8;  // 4 row groups per warp
    const int l = lane % 8;      // lane within the row group
    const int local_row = warp * 8 + group * 2;  // this thread's first row within the CTA
    float s[2][16];
#pragma unroll
    for (int r = 0; r < 2; ++r) {
        const float *row = state + (static_cast<long long>(vh) * kDim + row0 + local_row + r) * kDim;
#pragma unroll
        for (int i = 0; i < 4; ++i) {
            const float4 v = *reinterpret_cast<const float4 *>(row + 4 * l + 32 * i);
            s[r][4 * i] = v.x;
            s[r][4 * i + 1] = v.y;
            s[r][4 * i + 2] = v.z;
            s[r][4 * i + 3] = v.w;
        }
    }
    auto load_tile = [&](int tile, int buf) {
        const int t0 = tile * kTile;
        // k and q: kTile tokens x 2 x 32 float4
        for (int c = tid; c < kTile * 2 * (kDim / 4); c += kThreads) {
            const int t = c / (2 * (kDim / 4));
            const int which = (c / (kDim / 4)) % 2;
            const int part = c % (kDim / 4);
            if (t0 + t < n) {
                const long long base = (static_cast<long long>(t0 + t) * key_heads + kh) * kDim + part * 4;
                cp_async16(which ? &qs[buf][t][part * 4] : &ks[buf][t][part * 4], (which ? qn : kn) + base, 16);
            }
        }
        // values: kTile tokens x 8 float4
        {
            const int t = tid / 8;
            const int part = tid % 8;
            if (t < kTile && t0 + t < n) {
                const long long base =
                    static_cast<long long>(t0 + t) * conv_width + value_offset + vh * kDim + row0 + part * 4;
                cp_async16(&vs[buf][t][part * 4], conv + base, 16);
            }
        }
        if (tid < 3 * kTile) {
            const int t = tid % kTile;
            const int which = tid / kTile;
            if (t0 + t < n) {
                if (which == 0) {
                    cp_async4(&gs[buf][t], decay + static_cast<long long>(t0 + t) * value_heads + vh);
                } else if (which == 1) {
                    cp_async4(&bs[buf][t], beta + static_cast<long long>(t0 + t) * value_heads + vh);
                } else {
                    cp_async4(&kqs[buf][t], kq + static_cast<long long>(t0 + t) * key_heads + kh);
                }
            }
        }
        cp_async_commit();
    };
    const int tiles = (n + kTile - 1) / kTile;
    load_tile(0, 0);
    for (int tile = 0; tile < tiles; ++tile) {
        const int buf = tile & 1;
        if (tile + 1 < tiles) {
            load_tile(tile + 1, buf ^ 1);
            cp_async_wait<1>();
        } else {
            cp_async_wait<0>();
        }
        __syncthreads();
        const int count = min(kTile, n - tile * kTile);
        for (int t = 0; t < count; ++t) {
            float kr[16], qr[16];
#pragma unroll
            for (int i = 0; i < 4; ++i) {
                const float4 k4 = *reinterpret_cast<const float4 *>(&ks[buf][t][4 * l + 32 * i]);
                const float4 q4 = *reinterpret_cast<const float4 *>(&qs[buf][t][4 * l + 32 * i]);
                kr[4 * i] = k4.x;
                kr[4 * i + 1] = k4.y;
                kr[4 * i + 2] = k4.z;
                kr[4 * i + 3] = k4.w;
                qr[4 * i] = q4.x;
                qr[4 * i + 1] = q4.y;
                qr[4 * i + 2] = q4.z;
                qr[4 * i + 3] = q4.w;
            }
            const float g = gs[buf][t];
            const float b = bs[buf][t];
            const float k_dot_q = kqs[buf][t];
            float sk[2], sq[2];
#pragma unroll
            for (int r = 0; r < 2; ++r) {
                float sk0 = 0.0f, sk1 = 0.0f, sq0 = 0.0f, sq1 = 0.0f;
#pragma unroll
                for (int j = 0; j < 16; j += 2) {
                    s[r][j] *= g;
                    s[r][j + 1] *= g;
                    sk0 += s[r][j] * kr[j];
                    sk1 += s[r][j + 1] * kr[j + 1];
                    sq0 += s[r][j] * qr[j];
                    sq1 += s[r][j + 1] * qr[j + 1];
                }
                sk[r] = sk0 + sk1;
                sq[r] = sq0 + sq1;
            }
#pragma unroll
            for (int offset = 4; offset > 0; offset >>= 1) {
#pragma unroll
                for (int r = 0; r < 2; ++r) {
                    sk[r] += __shfl_xor_sync(0xffffffffu, sk[r], offset);
                    sq[r] += __shfl_xor_sync(0xffffffffu, sq[r], offset);
                }
            }
            const long long out_base = (static_cast<long long>(tile * kTile + t) * value_heads + vh) * kDim + row0;
#pragma unroll
            for (int r = 0; r < 2; ++r) {
                const float delta = (vs[buf][t][local_row + r] - sk[r]) * b;
#pragma unroll
                for (int j = 0; j < 16; ++j) {
                    s[r][j] += kr[j] * delta;
                }
                if (l == r) {
                    out[out_base + local_row + r] = sq[r] + delta * k_dot_q;
                }
            }
        }
        __syncthreads();
    }
#pragma unroll
    for (int r = 0; r < 2; ++r) {
        float *row = state + (static_cast<long long>(vh) * kDim + row0 + local_row + r) * kDim;
#pragma unroll
        for (int i = 0; i < 4; ++i) {
            *reinterpret_cast<float4 *>(row + 4 * l + 32 * i) =
                make_float4(s[r][4 * i], s[r][4 * i + 1], s[r][4 * i + 2], s[r][4 * i + 3]);
        }
    }
}

// The delta rule for dim 128 in blocks of 16 tokens at absolute positions,
// on tensor cores (#11444). Within a block, with S0 the state before it, g
// the decays, Gamma_j = g_0 ... g_j, R_ij = g_{i+1} ... g_j and the Gram
// terms KK_ij = k_i . k_j, KQ_ij = k_i . q_j, the per-token recurrence
// above is exactly
//   u_j = b_j (v_j - Gamma_j S0 . k_j),
//   delta = T u  with  T = (I + L)^-1,  L_ji = b_j R_ij KK_ij (i < j),
//   o_j = Gamma_j S0 . q_j + sum_i W_ij delta_i,  W_ij = R_ij KQ_ij (i <= j),
//   S16 = Gamma_15 S0 + sum_i R_i15 delta_i k_i.
// T, T^T W and T^T diag(R_i15) depend only on the value head and the block
// (delta_block_coef_kernel computes them, in parallel over blocks), so per
// state row a block is four matrix products and no serial chain:
//   X | Y = S0 [K | Q]^T,  O = Gamma Y + U (T^T W),  C = U (T^T diag R_15),
//   S16 = Gamma_15 S0 + C K.
// They run as m16n8k8 TF32 mma with the f32 operands split into high and
// low TF32 parts (three products, the low x low term dropped), which keeps
// about f32 accuracy. A warp holds 16 state rows in registers in the mma
// accumulator layout for the whole chunk (thread (g, c): rows g and g + 8 at
// columns 8t + 2c and 8t + 2c + 1); an accumulator is used again as an A
// fragment by permuting the k index within each 8-wide step (position c is
// column 2c, position c + 4 is column 2c + 1) and loading B with the same
// permutation. A CTA owns 16 rows of one value head; its SPLIT warps split
// the columns and add their partial X | Y in a fixed order. Every sum has a
// fixed order and blocks sit at chunk offsets that are multiples of 16, so
// with chunks that start at multiples of 16 (the caller checks) the result
// does not depend on the chunking; it differs from the per-token kernels by
// rounding only.
namespace bscan {
constexpr int kDim = 128;
constexpr int kT = 16;
constexpr int kGamma = 0;
constexpr int kBeta = kT;
constexpr int kTW = 2 * kT;               // [i][j] = sum_m T_mi W_mj
constexpr int kTC = kTW + kT * kT;        // [i][j] = T_ji R_j15
constexpr int kCoef = kTC + kT * kT;      // 544 floats per (block, value head)
constexpr int kLd = kDim + 8;             // padded K / Q row in shared memory
}  // namespace bscan

// One CTA per (block, key head): the Gram terms once, then the
// coefficients of each value head that reads this key head.
__global__ void __launch_bounds__(256)
    delta_block_coef_kernel(const float *__restrict__ qn, const float *__restrict__ kn, const float *__restrict__ decay,
                            const float *__restrict__ beta, float *__restrict__ coef, int n, int key_heads,
                            int value_heads, int v_head_reordered) {
    using namespace bscan;
    constexpr int kLdg = kDim + 4;  // float4 rows, conflict-free for 8 distinct rows per phase
    __shared__ __align__(16) float ks[kT][kLdg];
    __shared__ __align__(16) float qs[kT][kLdg];
    __shared__ float ms[kT][kT + 1], ws[kT][kT + 1], ts[kT][kT + 1];
    __shared__ float gs[kT], bs[kT], cl[kT];
    const int blk = blockIdx.x;
    const int kh = blockIdx.y;
    const int repeat = value_heads / key_heads;
    const int t0 = blk * kT;
    const int tid = threadIdx.x;
    for (int idx = tid; idx < kT * (kDim / 4); idx += blockDim.x) {
        const int i = idx / (kDim / 4);
        const int d = (idx % (kDim / 4)) * 4;
        const int t = t0 + i;
        const long long at = (static_cast<long long>(t) * key_heads + kh) * kDim + d;
        const float4 zero = make_float4(0.0f, 0.0f, 0.0f, 0.0f);
        *reinterpret_cast<float4 *>(&ks[i][d]) = t < n ? *reinterpret_cast<const float4 *>(kn + at) : zero;
        *reinterpret_cast<float4 *>(&qs[i][d]) = t < n ? *reinterpret_cast<const float4 *>(qn + at) : zero;
    }
    __syncthreads();
    const int i = tid / kT;
    const int j = tid % kT;
    float kk = 0.0f, kq = 0.0f;
#pragma unroll 8
    for (int d = 0; d < kDim; d += 4) {
        const float4 a = *reinterpret_cast<const float4 *>(&ks[i][d]);
        const float4 b = *reinterpret_cast<const float4 *>(&ks[j][d]);
        const float4 q = *reinterpret_cast<const float4 *>(&qs[j][d]);
        kk = fmaf(a.x, b.x, kk);
        kk = fmaf(a.y, b.y, kk);
        kk = fmaf(a.z, b.z, kk);
        kk = fmaf(a.w, b.w, kk);
        kq = fmaf(a.x, q.x, kq);
        kq = fmaf(a.y, q.y, kq);
        kq = fmaf(a.z, q.z, kq);
        kq = fmaf(a.w, q.w, kq);
    }
    for (int rep = 0; rep < repeat; ++rep) {
        const int vh = v_head_reordered ? kh + rep * key_heads : kh * repeat + rep;
        if (tid < kT) {
            // past the chunk: no decay, no write (the token changes nothing)
            const int t = t0 + tid;
            gs[tid] = t < n ? decay[static_cast<long long>(t) * value_heads + vh] : 1.0f;
            bs[tid] = t < n ? beta[static_cast<long long>(t) * value_heads + vh] : 0.0f;
        }
        __syncthreads();
        float r = 1.0f;
        for (int m = i + 1; m <= j; ++m) {
            r *= gs[m];
        }
        ms[i][j] = i < j ? bs[j] * r * kk : 0.0f;
        ws[i][j] = i <= j ? r * kq : 0.0f;
        if (j == kT - 1) {
            cl[i] = r;
        }
        float *cf = coef + (static_cast<long long>(blk) * value_heads + vh) * kCoef;
        if (i == 0) {
            float gamma = 1.0f;
            for (int m = 0; m <= j; ++m) {
                gamma *= gs[m];
            }
            cf[kGamma + j] = gamma;
            cf[kBeta + j] = bs[j];
        }
        __syncthreads();
        // T = (I + L)^-1 by forward substitution, one column per thread
        if (tid < kT) {
            const int col = tid;
            for (int row = 0; row < kT; ++row) {
                float v = row == col ? 1.0f : 0.0f;
                if (row > col) {
                    float sum = 0.0f;
                    for (int m = col; m < row; ++m) {
                        sum = fmaf(ms[m][row], ts[m][col], sum);
                    }
                    v = -sum;
                }
                ts[row][col] = v;
            }
        }
        __syncthreads();
        float tw = 0.0f;
        for (int m = 0; m < kT; ++m) {
            tw = fmaf(ts[m][i], ws[m][j], tw);
        }
        cf[kTW + i * kT + j] = tw;
        cf[kTC + i * kT + j] = ts[j][i] * cl[j];
        __syncthreads();
    }
}

__device__ __forceinline__ uint32_t tf32_bits(float x) {
    uint32_t r;
    asm("cvt.rna.tf32.f32 %0, %1;\n" : "=r"(r) : "f"(x));
    return r;
}

__device__ __forceinline__ void tf32_split(float x, uint32_t &hi, uint32_t &lo) {
    hi = tf32_bits(x);
    lo = tf32_bits(x - __uint_as_float(hi));
}

__device__ __forceinline__ void mma_tf32(float (&d)[4], const uint32_t (&a)[4], uint32_t b0, uint32_t b1) {
    asm volatile(
        "mma.sync.aligned.m16n8k8.row.col.f32.tf32.tf32.f32 {%0,%1,%2,%3}, {%4,%5,%6,%7}, {%8,%9}, {%0,%1,%2,%3};\n"
        : "+f"(d[0]), "+f"(d[1]), "+f"(d[2]), "+f"(d[3])
        : "r"(a[0]), "r"(a[1]), "r"(a[2]), "r"(a[3]), "r"(b0), "r"(b1));
}

// An A fragment split into high and low TF32 parts.
struct Tf32Frag {
    uint32_t hi[4], lo[4];
};

__device__ __forceinline__ Tf32Frag tf32_frag(float a0, float a1, float a2, float a3) {
    Tf32Frag f;
    tf32_split(a0, f.hi[0], f.lo[0]);
    tf32_split(a1, f.hi[1], f.lo[1]);
    tf32_split(a2, f.hi[2], f.lo[2]);
    tf32_split(a3, f.hi[3], f.lo[3]);
    return f;
}

// d += a b in about f32 precision: a_lo b_hi + a_hi b_lo + a_hi b_hi.
__device__ __forceinline__ void mma_3xtf32(float (&d)[4], const Tf32Frag &a, float b0, float b1) {
    uint32_t bh0, bl0, bh1, bl1;
    tf32_split(b0, bh0, bl0);
    tf32_split(b1, bh1, bl1);
    mma_tf32(d, a.lo, bh0, bh1);
    mma_tf32(d, a.hi, bl0, bl1);
    mma_tf32(d, a.hi, bh0, bh1);
}

// The A fragment of the accumulator-layout values {(g, 2c), (g, 2c + 1),
// (g + 8, 2c), (g + 8, 2c + 1)} of one 8-wide step, k permuted.
__device__ __forceinline__ Tf32Frag acc_as_a(const float (&v)[4]) { return tf32_frag(v[0], v[2], v[1], v[3]); }

constexpr int bscan_smem_bytes(int split) {
    return 4 * (4 * bscan::kT * bscan::kLd + 2 * bscan::kT * 16 + 2 * bscan::kCoef + split * 16 * 32);
}

template <int SPLIT>
__global__ void __launch_bounds__(32 * SPLIT)
    delta_block_scan_kernel(const float *__restrict__ qn, const float *__restrict__ kn, const float *__restrict__ conv,
                            const float *__restrict__ coef, float *__restrict__ state, float *__restrict__ out, int n,
                            int key_heads, int value_heads, int v_head_reordered, int conv_width, int value_offset,
                            int snap_blocks, float *__restrict__ snap) {
    using namespace bscan;
    constexpr int kRows = 16;
    constexpr int kThreads = 32 * SPLIT;
    constexpr int kTiles = 16 / SPLIT;  // 8-column tiles per warp
    // dynamic shared memory (bscan_smem_bytes): K, Q, V and the coefficients
    // double buffered, then the warps' partial X | Y
    extern __shared__ __align__(16) float bsmem[];
    auto ks = reinterpret_cast<float (*)[kT][kLd]>(bsmem);
    auto qs = reinterpret_cast<float (*)[kT][kLd]>(bsmem + 2 * kT * kLd);
    auto vs = reinterpret_cast<float (*)[kT][kRows]>(bsmem + 4 * kT * kLd);
    auto cs = reinterpret_cast<float (*)[kCoef]>(bsmem + 4 * kT * kLd + 2 * kT * kRows);
    auto xs = reinterpret_cast<float (*)[16][32]>(bsmem + 4 * kT * kLd + 2 * kT * kRows + 2 * kCoef);
    const int blocks_per_head = kDim / kRows;
    const int vh = blockIdx.x / blocks_per_head;
    const int row0 = (blockIdx.x % blocks_per_head) * kRows;
    const int repeat = value_heads / key_heads;
    const int kh = v_head_reordered ? vh % key_heads : vh / repeat;
    const int tid = threadIdx.x;
    const int lane = tid % kWarp;
    const int warp = tid / kWarp;
    const int g = lane / 4;
    const int c = lane % 4;
    const int t_first = warp * kTiles;
    // s[i] = {S[g][8t+2c], S[g][8t+2c+1], S[g+8][8t+2c], S[g+8][8t+2c+1]} for t = t_first + i
    float s[kTiles][4];
#pragma unroll
    for (int r = 0; r < 2; ++r) {
        const float *row = state + (static_cast<long long>(vh) * kDim + row0 + g + 8 * r) * kDim;
#pragma unroll
        for (int i = 0; i < kTiles; ++i) {
            const float2 v = *reinterpret_cast<const float2 *>(row + 8 * (t_first + i) + 2 * c);
            s[i][2 * r] = v.x;
            s[i][2 * r + 1] = v.y;
        }
    }
    const int blocks = (n + kT - 1) / kT;
    const float *k_base = kn + static_cast<long long>(kh) * kDim;
    const float *q_base = qn + static_cast<long long>(kh) * kDim;
    const float *v_base = conv + value_offset + vh * kDim + row0;
    const long long kq_stride = static_cast<long long>(key_heads) * kDim;
    auto load_block = [&](int blk, int buf) {
        const int t0 = blk * kT;
        // K and Q rows: 32 float4 each
#pragma unroll
        for (int i = tid; i < kT * 2 * (kDim / 4); i += kThreads) {
            const int t = i / (2 * (kDim / 4));
            const int which = (i / (kDim / 4)) % 2;
            const int part = i % (kDim / 4);
            const bool live = t0 + t < n;
            const long long at = (live ? t0 + t : 0) * kq_stride + part * 4;
            cp_async16(which ? &qs[buf][t][part * 4] : &ks[buf][t][part * 4], (which ? q_base : k_base) + at,
                       live ? 16 : 0);
        }
        for (int i = tid; i < kT * (kRows / 4); i += kThreads) {
            const int t = i / (kRows / 4);
            const int part = i % (kRows / 4);
            const bool live = t0 + t < n;
            cp_async16(&vs[buf][t][part * 4], v_base + static_cast<long long>(live ? t0 + t : 0) * conv_width + part * 4,
                       live ? 16 : 0);
        }
        const float *cf = coef + (static_cast<long long>(blk) * value_heads + vh) * kCoef;
        for (int i = tid; i < kCoef / 4; i += kThreads) {
            cp_async16(&cs[buf][i * 4], cf + i * 4, 16);
        }
        cp_async_commit();
    };
    load_block(0, 0);
    for (int blk = 0; blk < blocks; ++blk) {
        const int buf = blk & 1;
        if (blk + 1 < blocks) {
            load_block(blk + 1, buf ^ 1);
            cp_async_wait<1>();
        } else {
            cp_async_wait<0>();
        }
        __syncthreads();
        const float *cf = cs[buf];
        // partial X = S0 K^T, Y = S0 Q^T over this warp's columns, in two
        // chains (column tile parity) added in a fixed order
        constexpr int kChains = kTiles < 2 ? kTiles : 2;
        float px[kChains][2][4], py[kChains][2][4];
#pragma unroll
        for (int p = 0; p < kChains; ++p) {
#pragma unroll
            for (int jt = 0; jt < 2; ++jt) {
#pragma unroll
                for (int e = 0; e < 4; ++e) {
                    px[p][jt][e] = 0.0f;
                    py[p][jt][e] = 0.0f;
                }
            }
        }
#pragma unroll
        for (int i = 0; i < kTiles; ++i) {
            const int t = t_first + i;
            const Tf32Frag a = acc_as_a(s[i]);
#pragma unroll
            for (int jt = 0; jt < 2; ++jt) {
                // B (k = column 8t + perm, n = token 8 jt + g)
                const float2 kb = *reinterpret_cast<const float2 *>(&ks[buf][8 * jt + g][8 * t + 2 * c]);
                const float2 qb = *reinterpret_cast<const float2 *>(&qs[buf][8 * jt + g][8 * t + 2 * c]);
                mma_3xtf32(px[i % kChains][jt], a, kb.x, kb.y);
                mma_3xtf32(py[i % kChains][jt], a, qb.x, qb.y);
            }
        }
        // x[jt] / y[jt]: accumulator layout over tokens 8 jt + 2c (+1), the warps' partials in order
        float x[2][4], y[2][4];
#pragma unroll
        for (int jt = 0; jt < 2; ++jt) {
#pragma unroll
            for (int e = 0; e < 4; ++e) {
                float xv = px[0][jt][e], yv = py[0][jt][e];
#pragma unroll
                for (int p = 1; p < kChains; ++p) {
                    xv += px[p][jt][e];
                    yv += py[p][jt][e];
                }
                x[jt][e] = xv;
                y[jt][e] = yv;
            }
        }
        if constexpr (SPLIT > 1) {
#pragma unroll
            for (int i = 0; i < 8; ++i) {
                xs[warp][i][lane] = x[i / 4][i % 4];
                xs[warp][8 + i][lane] = y[i / 4][i % 4];
            }
            __syncthreads();
#pragma unroll
            for (int i = 0; i < 8; ++i) {
                float xv = xs[0][i][lane], yv = xs[0][8 + i][lane];
#pragma unroll
                for (int w = 1; w < SPLIT; ++w) {
                    xv += xs[w][i][lane];
                    yv += xs[w][8 + i][lane];
                }
                x[i / 4][i % 4] = xv;
                y[i / 4][i % 4] = yv;
            }
        }
        // u_j = b_j (v_j - Gamma_j x_j) in the accumulator layout
        float u[2][4];
#pragma unroll
        for (int jt = 0; jt < 2; ++jt) {
#pragma unroll
            for (int e = 0; e < 4; ++e) {
                const int j = 8 * jt + 2 * c + (e & 1);
                const int r = e >> 1;
                u[jt][e] = cf[kBeta + j] * (vs[buf][j][g + 8 * r] - cf[kGamma + j] * x[jt][e]);
            }
        }
        const Tf32Frag ua[2] = {acc_as_a(u[0]), acc_as_a(u[1])};
        // C = U (T^T diag R_15): the state update's coefficients, per token i
        float cc[2][4];
#pragma unroll
        for (int jt = 0; jt < 2; ++jt) {
#pragma unroll
            for (int e = 0; e < 4; ++e) {
                cc[jt][e] = 0.0f;
            }
#pragma unroll
            for (int kt = 0; kt < 2; ++kt) {
                mma_3xtf32(cc[jt], ua[kt], cf[kTC + (8 * kt + 2 * c) * kT + 8 * jt + g],
                           cf[kTC + (8 * kt + 2 * c + 1) * kT + 8 * jt + g]);
            }
        }
        // O = Gamma Y + U (T^T W); the warps split the two token tiles
        const int t0 = blk * kT;
#pragma unroll
        for (int jt = 0; jt < 2; ++jt) {
            if (SPLIT > 1 && (jt != (warp & 1) || warp >= 2)) {
                continue;
            }
            float o[4] = {0.0f, 0.0f, 0.0f, 0.0f};
#pragma unroll
            for (int kt = 0; kt < 2; ++kt) {
                mma_3xtf32(o, ua[kt], cf[kTW + (8 * kt + 2 * c) * kT + 8 * jt + g],
                           cf[kTW + (8 * kt + 2 * c + 1) * kT + 8 * jt + g]);
            }
#pragma unroll
            for (int e = 0; e < 4; ++e) {
                const int j = 8 * jt + 2 * c + (e & 1);
                const int r = e >> 1;
                if (t0 + j < n) {
                    out[(static_cast<long long>(t0 + j) * value_heads + vh) * kDim + row0 + g + 8 * r] =
                        fmaf(cf[kGamma + j], y[jt][e], o[e]);
                }
            }
        }
        // S16 = Gamma_15 S0 + C K on this warp's columns
        const float gamma_last = cf[kGamma + kT - 1];
        const Tf32Frag ca[2] = {acc_as_a(cc[0]), acc_as_a(cc[1])};
#pragma unroll
        for (int i = 0; i < kTiles; ++i) {
            const int t = t_first + i;
#pragma unroll
            for (int e = 0; e < 4; ++e) {
                s[i][e] *= gamma_last;
            }
#pragma unroll
            for (int kt = 0; kt < 2; ++kt) {
                // B (k = token 8 kt + perm, n = column 8t + g)
                mma_3xtf32(s[i], ca[kt], ks[buf][8 * kt + 2 * c][8 * t + g], ks[buf][8 * kt + 2 * c + 1][8 * t + g]);
            }
        }
        if (blk + 1 == snap_blocks) {
            // the state after the first snap_blocks blocks: a prefix snapshot
            // taken inside the chunk (#11442), in the state's layout
#pragma unroll
            for (int r = 0; r < 2; ++r) {
                float *row = snap + (static_cast<long long>(vh) * kDim + row0 + g + 8 * r) * kDim;
#pragma unroll
                for (int i = 0; i < kTiles; ++i) {
                    *reinterpret_cast<float2 *>(row + 8 * (t_first + i) + 2 * c) =
                        make_float2(s[i][2 * r], s[i][2 * r + 1]);
                }
            }
        }
        __syncthreads();
    }
#pragma unroll
    for (int r = 0; r < 2; ++r) {
        float *row = state + (static_cast<long long>(vh) * kDim + row0 + g + 8 * r) * kDim;
#pragma unroll
        for (int i = 0; i < kTiles; ++i) {
            *reinterpret_cast<float2 *>(row + 8 * (t_first + i) + 2 * c) = make_float2(s[i][2 * r], s[i][2 * r + 1]);
        }
    }
}

// Per-head RMSNorm(o) * w * silu(z) -> f16. Block (t, head), dim threads.
__global__ void gated_norm_to_f16_kernel(const float *o, const float *z, const float *w, __half *out, int heads, int dim, float eps) {
    __shared__ float scratch[32];
    const int t = blockIdx.x;
    const int h = blockIdx.y;
    const int i = threadIdx.x;
    const long long base = (static_cast<long long>(t) * heads + h) * dim;
    const float value = i < dim ? o[base + i] : 0.0f;
    const float scale = rsqrtf(block_sum(value * value, scratch) / dim + eps);
    if (i < dim) {
        out[base + i] = __float2half_rn(value * scale * w[i] * silu(z[base + i]));
    }
}

// ---- full attention ----

// Split interleaved [q | gate] per head, RMSNorm q and k per head, neox
// rotary on the first `rot` dims (cos/sin table [n, rot/2, 2]); q scaled by
// `scale`, written f16 [n, heads, dim]; gate f32 [n, heads*dim]; k and v
// appended to the f16 caches [cap, kv_heads, dim] at pos0 + t.
// Block (t, head index over heads + kv_heads), dim threads.
__global__ void attention_prep_kernel(const float *qg, const float *k, const float *v, const float *qw, const float *kw,
                                      const float *cos_sin, __half *q16, float *gate, __half *kcache, __half *vcache,
                                      int heads, int kv_heads, int dim, int rot, int pos0, float scale, float eps) {
    __shared__ float scratch[32];
    __shared__ float rowbuf[1024];
    const int t = blockIdx.x;
    const int h = blockIdx.y;
    const int i = threadIdx.x;
    const bool is_q = h < heads;
    const int head = is_q ? h : h - heads;
    float x = 0.0f;
    if (i < dim) {
        x = is_q ? qg[(static_cast<long long>(t) * heads + head) * dim * 2 + i]
                 : k[(static_cast<long long>(t) * kv_heads + head) * dim + i];
    }
    const float inv = rsqrtf(block_sum(x * x, scratch) / dim + eps);
    if (i < dim) {
        rowbuf[i] = x * inv * (is_q ? qw[i] : kw[i]);
    }
    __syncthreads();
    float y = i < dim ? rowbuf[i] : 0.0f;
    const int half_rot = rot / 2;
    if (i < rot) {
        const int pair = i < half_rot ? i : i - half_rot;
        const float c = cos_sin[(static_cast<long long>(t) * half_rot + pair) * 2];
        const float s = cos_sin[(static_cast<long long>(t) * half_rot + pair) * 2 + 1];
        const float x0 = rowbuf[pair];
        const float x1 = rowbuf[pair + half_rot];
        y = i < half_rot ? x0 * c - x1 * s : x0 * s + x1 * c;
    }
    if (i < dim) {
        if (is_q) {
            q16[(static_cast<long long>(t) * heads + head) * dim + i] = __float2half_rn(y * scale);
            gate[(static_cast<long long>(t) * heads + head) * dim + i] =
                qg[(static_cast<long long>(t) * heads + head) * dim * 2 + dim + i];
        } else {
            const long long slot = (static_cast<long long>(pos0 + t) * kv_heads + head) * dim + i;
            kcache[slot] = __float2half_rn(y);
            vcache[slot] = __float2half_rn(v[(static_cast<long long>(t) * kv_heads + head) * dim + i]);
        }
    }
}

// Causal softmax over score rows [rows, keys] (f32) to f16 probabilities;
// row r = head_offset * n + i sees keys <= pos0 + i. One block per row.
__global__ void causal_softmax_to_f16_kernel(const float *scores, __half *probs, int n, int keys, int pos0) {
    __shared__ float scratch[32];
    const long long r = blockIdx.x;
    const int i = static_cast<int>(r % n);
    const int visible = pos0 + i + 1;
    const float *row = scores + r * keys;
    __half *dst = probs + r * keys;
    float best = -INFINITY;
    for (int j = threadIdx.x; j < visible; j += blockDim.x) {
        best = fmaxf(best, row[j]);
    }
    best = block_max(best, scratch);
    float sum = 0.0f;
    for (int j = threadIdx.x; j < visible; j += blockDim.x) {
        sum += expf(row[j] - best);
    }
    const float inv = 1.0f / block_sum(sum, scratch);
    for (int j = threadIdx.x; j < keys; j += blockDim.x) {
        dst[j] = __float2half_rn(j < visible ? expf(row[j] - best) * inv : 0.0f);
    }
}

// Softmax over full rows [rows, keys] in place (f32), after scaling.
__global__ void softmax_rows_f32_kernel(float *scores, int keys, float scale) {
    __shared__ float scratch[32];
    float *row = scores + static_cast<long long>(blockIdx.x) * keys;
    float best = -INFINITY;
    for (int j = threadIdx.x; j < keys; j += blockDim.x) {
        best = fmaxf(best, row[j] * scale);
    }
    best = block_max(best, scratch);
    float sum = 0.0f;
    for (int j = threadIdx.x; j < keys; j += blockDim.x) {
        sum += expf(row[j] * scale - best);
    }
    const float inv = 1.0f / block_sum(sum, scratch);
    for (int j = threadIdx.x; j < keys; j += blockDim.x) {
        row[j] = expf(row[j] * scale - best) * inv;
    }
}

// out16 = o * sigmoid(gate) (f16).
__global__ void sigmoid_gate_to_f16_kernel(const float *o, const float *gate, __half *out, long long count) {
    const long long i = blockIdx.x * (long long)blockDim.x + threadIdx.x;
    if (i < count) {
        out[i] = __float2half_rn(o[i] * sigmoid(gate[i]));
    }
}

// out16 = silu(gate) * up (f16).
__global__ void silu_mul_to_f16_kernel(const float *gate, const float *up, __half *out, long long count) {
    const long long i = blockIdx.x * (long long)blockDim.x + threadIdx.x;
    if (i < count) {
        out[i] = __float2half_rn(silu(gate[i]) * up[i]);
    }
}

// Span sums of rows: spans [count, 2] (absolute start, end), rows cover
// absolute positions [first, first + n); sums [count, d] accumulate in
// token order. One block per span, threads over d.
__global__ void span_sums_kernel(const float *rows, const int *spans, float *sums, int d, int first, int n) {
    const int span = blockIdx.x;
    const int start = max(spans[2 * span], first);
    const int end = min(spans[2 * span + 1], first + n);
    for (int c = threadIdx.x; c < d; c += blockDim.x) {
        float acc = sums[static_cast<long long>(span) * d + c];
        for (int t = start; t < end; ++t) {
            acc += rows[static_cast<long long>(t - first) * d + c];
        }
        sums[static_cast<long long>(span) * d + c] = acc;
    }
}

// ---- fused dequantize + tensor-core linear ----
//
// out[t, m] (+)= sum_k x[t, k] W[m, k] with x f16 row-major [n, K] and W in
// its GGUF layout (Q8_0 or Q4_K rows). A CTA owns 128 tokens x 128 weight
// rows; each 64-wide k tile of W is dequantized (to the same f16 values as
// the dequant kernels above) straight into shared memory, so the weights are
// read once in their quantized form and never written out as f16.
//
// Accumulation order is fixed per output element: k tiles in order, inside
// a tile the mma k steps in order, nothing split across CTAs. A token's
// output therefore does not depend on which other tokens share its chunk
// (bitwise chunk invariance). With SEG > 0 the mma accumulates in f16 over
// SEG k steps (16 each) and the partial is added to an f32 accumulator
// (full-rate f16 tensor math, f32 error growth); SEG == 0 accumulates the
// mma in f32.
namespace fused {
constexpr int kTokens = 128;
constexpr int kRows = 128;
constexpr int kTileK = 64;
constexpr int kLds = kTileK + 8;  // halves per shared row (conflict-free ldmatrix)
constexpr int kXStages = 3;
constexpr int kThreads = 256;
constexpr int kSmemBytes = (kXStages * kTokens + 2 * kRows) * kLds * 2;
}  // namespace fused

constexpr int kFormatQ8_0 = 0;
constexpr int kFormatQ4K = 1;

__device__ __forceinline__ void ldmatrix_x4(uint32_t (&r)[4], const __half *shared) {
    const unsigned address = static_cast<unsigned>(__cvta_generic_to_shared(shared));
    asm volatile("ldmatrix.sync.aligned.m8n8.x4.shared.b16 {%0,%1,%2,%3}, [%4];\n"
                 : "=r"(r[0]), "=r"(r[1]), "=r"(r[2]), "=r"(r[3])
                 : "r"(address));
}

__device__ __forceinline__ uint32_t half2_bits(__half2 value) {
    uint32_t bits;
    memcpy(&bits, &value, sizeof(bits));
    return bits;
}

__device__ __forceinline__ float2 bits_to_float2(uint32_t bits) {
    __half2 value;
    memcpy(&value, &bits, sizeof(bits));
    return __half22float2(value);
}

// Raw bytes of one thread's 32-element slice of a weight k tile: Q8_0 one
// 34-byte block (9 aligned words), Q4_K the super-block header (4 words)
// plus the 32 nibble bytes of the sub-block pair (8 words).
template <int FMT>
struct RawSlice {
    uint32_t words[FMT == kFormatQ8_0 ? 9 : 12];
};

template <int FMT>
__device__ __forceinline__ void load_slice(RawSlice<FMT> &raw, const uint8_t *w, long long row_bytes, int row, int rows,
                                           int kt, int half_index) {
    constexpr int kWords = FMT == kFormatQ8_0 ? 9 : 12;
    if (row >= rows) {
#pragma unroll
        for (int i = 0; i < kWords; ++i) {
            raw.words[i] = 0;
        }
        return;
    }
    const uint8_t *base = w + static_cast<long long>(row) * row_bytes;
    if constexpr (FMT == kFormatQ8_0) {
        // block (2 kt + half) starts at 34 * block; read the aligned
        // 36-byte window around it
        const long long start = static_cast<long long>(2 * kt + half_index) * 34;
        const uint32_t *words = reinterpret_cast<const uint32_t *>(base + (start & ~3LL));
#pragma unroll
        for (int i = 0; i < 9; ++i) {
            raw.words[i] = __ldg(words + i);
        }
    } else {
        const int k0 = kt * fused::kTileK;
        const uint8_t *super_block = base + static_cast<long long>(k0 / 256) * 144;
        const int j = (k0 % 256) / 64;
        const uint4 header = __ldg(reinterpret_cast<const uint4 *>(super_block));
        const uint4 q0 = __ldg(reinterpret_cast<const uint4 *>(super_block + 16 + 32 * j));
        const uint4 q1 = __ldg(reinterpret_cast<const uint4 *>(super_block + 16 + 32 * j + 16));
        raw.words[0] = header.x;
        raw.words[1] = header.y;
        raw.words[2] = header.z;
        raw.words[3] = header.w;
        raw.words[4] = q0.x;
        raw.words[5] = q0.y;
        raw.words[6] = q0.z;
        raw.words[7] = q0.w;
        raw.words[8] = q1.x;
        raw.words[9] = q1.y;
        raw.words[10] = q1.z;
        raw.words[11] = q1.w;
    }
}

// Dequantizes a slice into 32 f16 values at `dst` (16-byte aligned), with
// the arithmetic of dequant_q8_0_kernel / dequant_q4_k_kernel.
template <int FMT>
__device__ __forceinline__ void store_slice(const RawSlice<FMT> &raw, __half *dst, int kt, int half_index) {
    uint32_t packed[16];
    if constexpr (FMT == kFormatQ8_0) {
        const int shift = static_cast<int>(((2 * kt + half_index) * 34) & 3);  // 0 or 2
        const uint16_t d_bits = shift ? static_cast<uint16_t>(raw.words[0] >> 16) : static_cast<uint16_t>(raw.words[0]);
        __half d_half;
        memcpy(&d_half, &d_bits, sizeof(d_half));
        const float d = __half2float(d_half);
#pragma unroll
        for (int i = 0; i < 8; ++i) {
            // the 4 quants at bytes shift + 2 + 4i of the window
            const uint32_t q = shift ? raw.words[i + 1] : __funnelshift_r(raw.words[i], raw.words[i + 1], 16);
            const int8_t q0 = static_cast<int8_t>(q & 0xff), q1 = static_cast<int8_t>((q >> 8) & 0xff);
            const int8_t q2 = static_cast<int8_t>((q >> 16) & 0xff), q3 = static_cast<int8_t>(q >> 24);
            packed[2 * i] = half2_bits(__floats2half2_rn(d * q0, d * q1));
            packed[2 * i + 1] = half2_bits(__floats2half2_rn(d * q2, d * q3));
        }
    } else {
        const int k0 = kt * fused::kTileK;
        const int j = (k0 % 256) / 64;
        const bool high = half_index != 0;
        const int is = 2 * j + (high ? 1 : 0);
        uint8_t header[16];
        memcpy(header, raw.words, 16);
        __half d_half, dmin_half;
        memcpy(&d_half, header, 2);
        memcpy(&dmin_half, header + 2, 2);
        const float d = __half2float(d_half);
        const float dmin = __half2float(dmin_half);
        const uint8_t *scales = header + 4;
        uint8_t sc, m;
        if (is < 4) {
            sc = scales[is] & 63;
            m = scales[is + 4] & 63;
        } else {
            sc = (scales[is + 4] & 0xF) | ((scales[is - 4] >> 6) << 4);
            m = (scales[is + 4] >> 4) | ((scales[is] >> 6) << 4);
        }
        const float scale = d * sc, minimum = dmin * m;
#pragma unroll
        for (int i = 0; i < 8; ++i) {
            uint32_t v = raw.words[4 + i];
            if (high) {
                v >>= 4;
            }
            const float a0 = scale * ((v >> 0) & 0xF) - minimum;
            const float a1 = scale * ((v >> 8) & 0xF) - minimum;
            const float a2 = scale * ((v >> 16) & 0xF) - minimum;
            const float a3 = scale * ((v >> 24) & 0xF) - minimum;
            packed[2 * i] = half2_bits(__floats2half2_rn(a0, a1));
            packed[2 * i + 1] = half2_bits(__floats2half2_rn(a2, a3));
        }
    }
    uint4 *out = reinterpret_cast<uint4 *>(dst);
#pragma unroll
    for (int i = 0; i < 4; ++i) {
        out[i] = make_uint4(packed[4 * i], packed[4 * i + 1], packed[4 * i + 2], packed[4 * i + 3]);
    }
}

template <int FMT, int SEG>
__global__ void __launch_bounds__(fused::kThreads, 1)
    fused_linear_kernel(const __half *__restrict__ x, const uint8_t *__restrict__ w, float *__restrict__ out, int n,
                        int rows, int k, long long row_bytes, int accumulate) {
    using namespace fused;
    extern __shared__ __align__(16) unsigned char smem_raw[];
    __half *xs = reinterpret_cast<__half *>(smem_raw);
    __half *ws = xs + kXStages * kTokens * kLds;
    const int tid = threadIdx.x;
    const int lane = tid % kWarp;
    const int warp = tid / kWarp;
    const int t0 = blockIdx.x * kTokens;
    const int m0 = blockIdx.y * kRows;
    const int warp_t = (warp & 1) * 64;   // 2 warps along tokens
    const int warp_m = (warp >> 1) * 32;  // 4 warps along weight rows
    const int tiles = k / kTileK;

    auto load_x = [&](int kt, int stage) {
        __half *dst = xs + stage * kTokens * kLds;
#pragma unroll
        for (int i = 0; i < 4; ++i) {
            const int chunk = tid + i * kThreads;
            const int row = chunk >> 3;
            const int col = (chunk & 7) * 8;
            const int t = t0 + row;
            const __half *src = x + static_cast<long long>(t < n ? t : n - 1) * k + kt * kTileK + col;
            cp_async16(dst + row * kLds + col, src, t < n ? 16 : 0);
        }
    };
    const int slice_row = tid >> 1;
    const int slice_half = tid & 1;
    RawSlice<FMT> raw;

    float acc[4][4][4];
#pragma unroll
    for (int i = 0; i < 4; ++i) {
#pragma unroll
        for (int j = 0; j < 4; ++j) {
#pragma unroll
            for (int e = 0; e < 4; ++e) {
                acc[i][j][e] = 0.0f;
            }
        }
    }

    uint32_t seg[4][4][2];  // the f16 partials (SEG > 0), carried across k tiles

    load_x(0, 0);
    cp_async_commit();
    if (tiles > 1) {
        load_x(1, 1);
    }
    cp_async_commit();
    load_slice<FMT>(raw, w, row_bytes, m0 + slice_row, rows, 0, slice_half);
    store_slice<FMT>(raw, ws + slice_row * kLds + slice_half * 32, 0, slice_half);
    if (tiles > 1) {
        load_slice<FMT>(raw, w, row_bytes, m0 + slice_row, rows, 1, slice_half);
    }
    cp_async_wait<1>();
    __syncthreads();

    for (int kt = 0; kt < tiles; ++kt) {
        if (kt + 2 < tiles) {
            load_x(kt + 2, (kt + 2) % kXStages);
        }
        cp_async_commit();
        const __half *xa = xs + (kt % kXStages) * kTokens * kLds;
        const __half *wb = ws + (kt & 1) * kRows * kLds;
        // fragments double-buffered across the k steps: the next step's
        // ldmatrix loads issue before this step's mma
        uint32_t a[2][4][4];
        uint32_t b[2][4][2];
        auto load_fragments = [&](int ks, int fb) {
#pragma unroll
            for (int i = 0; i < 4; ++i) {
                ldmatrix_x4(a[fb][i], xa + (warp_t + i * 16 + (lane % 16)) * kLds + ks * 16 + (lane / 16) * 8);
            }
#pragma unroll
            for (int jj = 0; jj < 2; ++jj) {
                const int mat = lane / 8;
                uint32_t r[4];
                ldmatrix_x4(r, wb + (warp_m + jj * 16 + (mat / 2) * 8 + (lane % 8)) * kLds + ks * 16 + (mat % 2) * 8);
                b[fb][2 * jj][0] = r[0];
                b[fb][2 * jj][1] = r[1];
                b[fb][2 * jj + 1][0] = r[2];
                b[fb][2 * jj + 1][1] = r[3];
            }
        };
        load_fragments(0, 0);
#pragma unroll
        for (int ks = 0; ks < kTileK / 16; ++ks) {
            const int fb = ks & 1;
            if (ks + 1 < kTileK / 16) {
                load_fragments(ks + 1, fb ^ 1);
            }
            if constexpr (SEG == 0) {
#pragma unroll
                for (int i = 0; i < 4; ++i) {
#pragma unroll
                    for (int j = 0; j < 4; ++j) {
                        asm volatile(
                            "mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32 {%0,%1,%2,%3}, {%4,%5,%6,%7}, "
                            "{%8,%9}, {%0,%1,%2,%3};\n"
                            : "+f"(acc[i][j][0]), "+f"(acc[i][j][1]), "+f"(acc[i][j][2]), "+f"(acc[i][j][3])
                            : "r"(a[fb][i][0]), "r"(a[fb][i][1]), "r"(a[fb][i][2]), "r"(a[fb][i][3]), "r"(b[fb][j][0]),
                              "r"(b[fb][j][1]));
                    }
                }
            } else {
                if ((kt * (kTileK / 16) + ks) % SEG == 0) {
#pragma unroll
                    for (int i = 0; i < 4; ++i) {
#pragma unroll
                        for (int j = 0; j < 4; ++j) {
                            seg[i][j][0] = 0u;
                            seg[i][j][1] = 0u;
                        }
                    }
                }
#pragma unroll
                for (int i = 0; i < 4; ++i) {
#pragma unroll
                    for (int j = 0; j < 4; ++j) {
                        asm volatile(
                            "mma.sync.aligned.m16n8k16.row.col.f16.f16.f16.f16 {%0,%1}, {%2,%3,%4,%5}, {%6,%7}, "
                            "{%0,%1};\n"
                            : "+r"(seg[i][j][0]), "+r"(seg[i][j][1])
                            : "r"(a[fb][i][0]), "r"(a[fb][i][1]), "r"(a[fb][i][2]), "r"(a[fb][i][3]), "r"(b[fb][j][0]),
                              "r"(b[fb][j][1]));
                    }
                }
                if ((kt * (kTileK / 16) + ks + 1) % SEG == 0 || (kt + 1 == tiles && ks + 1 == kTileK / 16)) {
#pragma unroll
                    for (int i = 0; i < 4; ++i) {
#pragma unroll
                        for (int j = 0; j < 4; ++j) {
                            const float2 lo = bits_to_float2(seg[i][j][0]);
                            const float2 hi = bits_to_float2(seg[i][j][1]);
                            acc[i][j][0] += lo.x;
                            acc[i][j][1] += lo.y;
                            acc[i][j][2] += hi.x;
                            acc[i][j][3] += hi.y;
                        }
                    }
                }
            }
        }
#ifndef CLEF_FUSED_EXPERIMENT_NO_DEQUANT
        if (kt + 1 < tiles) {
            store_slice<FMT>(raw, ws + ((kt + 1) & 1) * kRows * kLds + slice_row * kLds + slice_half * 32, kt + 1,
                             slice_half);
            if (kt + 2 < tiles) {
                load_slice<FMT>(raw, w, row_bytes, m0 + slice_row, rows, kt + 2, slice_half);
            }
        }
#endif
        cp_async_wait<1>();
        __syncthreads();
    }
    cp_async_wait<0>();

    // D fragment: (row g, cols 2c..2c+1) and (row g + 8, same cols)
    const int g = lane / 4;
    const int c = lane % 4;
#pragma unroll
    for (int i = 0; i < 4; ++i) {
#pragma unroll
        for (int j = 0; j < 4; ++j) {
            const int m = m0 + warp_m + j * 8 + 2 * c;
            if (m >= rows) {
                continue;
            }
#pragma unroll
            for (int h = 0; h < 2; ++h) {
                const int t = t0 + warp_t + i * 16 + g + 8 * h;
                if (t >= n) {
                    continue;
                }
                float2 *dst = reinterpret_cast<float2 *>(out + static_cast<long long>(t) * rows + m);
                float2 value = make_float2(acc[i][j][2 * h], acc[i][j][2 * h + 1]);
                if (accumulate) {
                    const float2 old = *dst;
                    value.x = old.x + value.x;
                    value.y = old.y + value.y;
                }
                *dst = value;
            }
        }
    }
}

// ---- f32 linear with a fixed reduction order ----
// out[n, m] = x[n, k] . w[m, k]^T (f32). Each output sums k in order in
// one thread, so a row's result does not depend on the other rows in the
// call. 64 x 64 outputs per CTA, 16 x 16 threads with 4 x 4 each, k tiles
// of 16 through shared memory.
__global__ void __launch_bounds__(256) linear_f32_ordered_kernel(const float *__restrict__ x, const float *__restrict__ w,
                                                                 float *__restrict__ out, int n, int m, int k) {
    __shared__ float xs[16][64 + 1];
    __shared__ float ws[16][64 + 1];
    const int tx = threadIdx.x % 16, ty = threadIdx.x / 16;
    const int row0 = blockIdx.y * 64, col0 = blockIdx.x * 64;
    float acc[4][4] = {};
    for (int k0 = 0; k0 < k; k0 += 16) {
        for (int i = threadIdx.x; i < 64 * 16; i += 256) {
            const int r = i / 16, kk = i % 16;
            const int row = row0 + r, col = col0 + r, kidx = k0 + kk;
            xs[kk][r] = row < n && kidx < k ? x[static_cast<long long>(row) * k + kidx] : 0.0f;
            ws[kk][r] = col < m && kidx < k ? w[static_cast<long long>(col) * k + kidx] : 0.0f;
        }
        __syncthreads();
#pragma unroll
        for (int kk = 0; kk < 16; ++kk) {
#pragma unroll
            for (int i = 0; i < 4; ++i) {
#pragma unroll
                for (int j = 0; j < 4; ++j) {
                    acc[i][j] = fmaf(xs[kk][ty * 4 + i], ws[kk][tx * 4 + j], acc[i][j]);
                }
            }
        }
        __syncthreads();
    }
#pragma unroll
    for (int i = 0; i < 4; ++i) {
        const int row = row0 + ty * 4 + i;
        if (row >= n) {
            continue;
        }
#pragma unroll
        for (int j = 0; j < 4; ++j) {
            const int col = col0 + tx * 4 + j;
            if (col < m) {
                out[static_cast<long long>(row) * m + col] = acc[i][j];
            }
        }
    }
}

// ---- the joint head's memory attention, query side ----
// side[i, h, :] = sum_{d in head h} q[i, d] W_k[d, :]  (W_k [width, width],
// rows are outputs). One block per (i, h), threads over the width.
__global__ void head_side_kernel(const float *__restrict__ q, const float *__restrict__ wk, float *__restrict__ side,
                                 int heads, int width, int head_dim) {
    const int i = blockIdx.x / heads;
    const int h = blockIdx.x % heads;
    const float *query = q + static_cast<long long>(i) * width + h * head_dim;
    float *dst = side + static_cast<long long>(blockIdx.x) * width;
    for (int c = threadIdx.x; c < width; c += blockDim.x) {
        float acc = 0.0f;
        for (int d = 0; d < head_dim; ++d) {
            acc = fmaf(query[d], wk[static_cast<long long>(h * head_dim + d) * width + c], acc);
        }
        dst[c] = acc;
    }
}

// context[i, e] = W_v[e, :] . mixed[i, head(e), :] + b_v[e]. One warp per
// (i, e), a fixed-order lane split and butterfly.
__global__ void head_context_kernel(const float *__restrict__ mixed, const float *__restrict__ wv,
                                    const float *__restrict__ bv, float *__restrict__ out, int n, int heads, int width,
                                    int head_dim) {
    const long long warp_id = (static_cast<long long>(blockIdx.x) * blockDim.x + threadIdx.x) / kWarp;
    const int lane = threadIdx.x % kWarp;
    if (warp_id >= static_cast<long long>(n) * width) {
        return;
    }
    const int i = static_cast<int>(warp_id / width);
    const int e = static_cast<int>(warp_id % width);
    const int h = e / head_dim;
    const float *z = mixed + (static_cast<long long>(i) * heads + h) * width;
    const float *row = wv + static_cast<long long>(e) * width;
    float acc = 0.0f;
    for (int c = lane; c < width; c += kWarp) {
        acc = fmaf(row[c], z[c], acc);
    }
    acc = warp_sum(acc);
    if (lane == 0) {
        out[static_cast<long long>(i) * width + e] = acc + bv[e];
    }
}

// x[r, c] += b[c], then GELU (erf form, in double like the host head) when
// `gelu` is set. In place.
__global__ void bias_act_kernel(float *x, const float *b, long long count, int cols, int gelu) {
    const long long i = blockIdx.x * (long long)blockDim.x + threadIdx.x;
    if (i >= count) {
        return;
    }
    float v = x[i] + b[i % cols];
    if (gelu) {
        const double d = static_cast<double>(v);
        v = static_cast<float>(0.5 * d * (1.0 + erf(d / 1.4142135623730951)));
    }
    x[i] = v;
}

// ---- causal flash attention, fixed order ----
//
// out[t, h, :] = softmax_j(q[t, h] . k[j]) v[j] over keys j <= first + t,
// for head_dim 256, f16 q (already scaled) and f16 K/V caches
// [positions, kv_heads, 256], f32 out [n, heads, 256].
//
// A CTA owns 16 queries and one KV head; each of its 4 warps takes one of
// the 4 query heads that share it, holding its 16 x 256 queries in
// registers. Keys stream in tiles of 16 at absolute positions 0, 16, 32,
// ... through double-buffered shared memory shared by the 4 heads. Each
// query row runs the online softmax over those tiles in order, so its
// result depends only on its own position and the cache, never on the
// chunk it came in: a tile past its diagonal is all masked and leaves its
// state exactly unchanged (alpha = 1, p = 0). Keys at or beyond
// `first + n` are zero-filled, so masked lanes never read garbage. Scores
// accumulate in f32 (f16 tensor inputs); probabilities are f16 for the
// P V product, which accumulates in f32.
namespace flash {
constexpr int kDim = 256;
constexpr int kQueries = 16;
constexpr int kKeys = 16;
constexpr int kGroup = 4;  // query heads per KV head
constexpr int kLds = kDim + 8;
constexpr int kThreads = 32 * kGroup;
// K and V double buffered; the query staging (kGroup x 16 rows) reuses it
constexpr int kSmemBytes = 4 * kKeys * kLds * 2;
static_assert(kGroup * kQueries <= 4 * kKeys, "query staging fits the K/V buffers");
}  // namespace flash

__device__ __forceinline__ void ldmatrix_x4_trans(uint32_t (&r)[4], const __half *shared) {
    const unsigned address = static_cast<unsigned>(__cvta_generic_to_shared(shared));
    asm volatile("ldmatrix.sync.aligned.m8n8.x4.trans.shared.b16 {%0,%1,%2,%3}, [%4];\n"
                 : "=r"(r[0]), "=r"(r[1]), "=r"(r[2]), "=r"(r[3])
                 : "r"(address));
}

__device__ __forceinline__ uint32_t pack_half2(float lo, float hi) {
    return half2_bits(__floats2half2_rn(lo, hi));
}

__global__ void __launch_bounds__(flash::kThreads, 2)
    flash_attention256_kernel(const __half *__restrict__ q16, const __half *__restrict__ kcache,
                              const __half *__restrict__ vcache, float *__restrict__ out, int n, int heads,
                              int kv_heads, int first) {
    using namespace flash;
    extern __shared__ __align__(16) unsigned char smem_raw[];
    __half *ks = reinterpret_cast<__half *>(smem_raw);  // [2][kKeys][kLds]
    __half *vs = ks + 2 * kKeys * kLds;                  // [2][kKeys][kLds]
    const int tid = threadIdx.x;
    const int lane = tid % kWarp;
    const int warp = tid / kWarp;
    const int kvh = blockIdx.y;
    const int head = kvh * kGroup + warp;
    const int t0 = blockIdx.x * kQueries;  // first query of the CTA, within the chunk
    const int g = lane / 4;
    const int c = lane % 4;

    // stage the group's queries (zero past n) and take them into registers
    for (int chunk = tid; chunk < kGroup * kQueries * (kDim / 8); chunk += kThreads) {
        const int row = chunk / (kDim / 8);  // head-major: row = h * 16 + query
        const int col = (chunk % (kDim / 8)) * 8;
        const int t = t0 + row % kQueries;
        const int h = kvh * kGroup + row / kQueries;
        const __half *src = q16 + (static_cast<long long>(t < n ? t : n - 1) * heads + h) * kDim + col;
        cp_async16(ks + row * kLds + col, src, t < n ? 16 : 0);
    }
    cp_async_commit();
    cp_async_wait<0>();
    __syncthreads();
    uint32_t qa[kDim / 16][4];
#pragma unroll
    for (int kk = 0; kk < kDim / 16; ++kk) {
        ldmatrix_x4(qa[kk], ks + (warp * kQueries + (lane % 16)) * kLds + kk * 16 + (lane / 16) * 8);
    }
    __syncthreads();

    const int limit = first + n;  // keys that exist
    const int last_query = first + min(t0 + kQueries, n) - 1;
    const int tiles = last_query / kKeys + 1;
    auto load_kv = [&](int tile, int buf) {
        const int j0 = tile * kKeys;
        for (int chunk = tid; chunk < kKeys * (kDim / 8); chunk += kThreads) {
            const int row = chunk / (kDim / 8);
            const int col = (chunk % (kDim / 8)) * 8;
            const int j = j0 + row;
            const long long offset = (static_cast<long long>(j < limit ? j : 0) * kv_heads + kvh) * kDim + col;
            const int bytes = j < limit ? 16 : 0;
            cp_async16(ks + (buf * kKeys + row) * kLds + col, kcache + offset, bytes);
            cp_async16(vs + (buf * kKeys + row) * kLds + col, vcache + offset, bytes);
        }
        cp_async_commit();
    };
    load_kv(0, 0);

    float o[kDim / 8][4];
#pragma unroll
    for (int i = 0; i < kDim / 8; ++i) {
        o[i][0] = o[i][1] = o[i][2] = o[i][3] = 0.0f;
    }
    float row_max[2] = {-INFINITY, -INFINITY};
    float row_sum[2] = {0.0f, 0.0f};
    const int pos[2] = {first + t0 + g, first + t0 + g + 8};

    for (int tile = 0; tile < tiles; ++tile) {
        const int buf = tile & 1;
        if (tile + 1 < tiles) {
            load_kv(tile + 1, buf ^ 1);
            cp_async_wait<1>();
        } else {
            cp_async_wait<0>();
        }
        __syncthreads();
        const __half *kt = ks + buf * kKeys * kLds;
        const __half *vt = vs + buf * kKeys * kLds;
        // S = Q K^T: 16 rows x 16 keys (2 n8 tiles)
        float s[2][4];
#pragma unroll
        for (int j = 0; j < 2; ++j) {
            s[j][0] = s[j][1] = s[j][2] = s[j][3] = 0.0f;
        }
#pragma unroll
        for (int kk = 0; kk < kDim / 16; ++kk) {
            const int mat = lane / 8;
            uint32_t r[4];
            ldmatrix_x4(r, kt + ((mat / 2) * 8 + (lane % 8)) * kLds + kk * 16 + (mat % 2) * 8);
#pragma unroll
            for (int h = 0; h < 2; ++h) {
                float *acc = s[h];
                asm volatile(
                    "mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32 {%0,%1,%2,%3}, {%4,%5,%6,%7}, {%8,%9}, "
                    "{%0,%1,%2,%3};\n"
                    : "+f"(acc[0]), "+f"(acc[1]), "+f"(acc[2]), "+f"(acc[3])
                    : "r"(qa[kk][0]), "r"(qa[kk][1]), "r"(qa[kk][2]), "r"(qa[kk][3]), "r"(r[2 * h]),
                      "r"(r[2 * h + 1]));
            }
        }
        // mask, online softmax per row (rows g and g + 8)
        const int j0 = tile * kKeys;
        float tile_max[2] = {-INFINITY, -INFINITY};
#pragma unroll
        for (int j = 0; j < 2; ++j) {
#pragma unroll
            for (int e = 0; e < 4; ++e) {
                const int key = j0 + j * 8 + 2 * c + (e & 1);
                const int r = e >> 1;
                if (key > pos[r]) {
                    s[j][e] = -INFINITY;
                }
                tile_max[r] = fmaxf(tile_max[r], s[j][e]);
            }
        }
        float alpha[2], new_max[2];
#pragma unroll
        for (int r = 0; r < 2; ++r) {
            tile_max[r] = fmaxf(tile_max[r], __shfl_xor_sync(0xffffffffu, tile_max[r], 1));
            tile_max[r] = fmaxf(tile_max[r], __shfl_xor_sync(0xffffffffu, tile_max[r], 2));
            new_max[r] = fmaxf(row_max[r], tile_max[r]);
            alpha[r] = new_max[r] == row_max[r] ? 1.0f : expf(row_max[r] - new_max[r]);
            row_max[r] = new_max[r];
        }
        float tile_sum[2] = {0.0f, 0.0f};
        uint32_t p[4];  // the A fragment of P (16 rows x 16 keys)
#pragma unroll
        for (int j = 0; j < 2; ++j) {
            float e[4];
#pragma unroll
            for (int k = 0; k < 4; ++k) {
                const int r = k >> 1;
                e[k] = s[j][k] == -INFINITY ? 0.0f : expf(s[j][k] - new_max[r]);
                tile_sum[r] += e[k];
            }
            // (rows g, keys 0-7), (rows g+8, keys 0-7), (rows g, keys 8-15), (rows g+8, keys 8-15)
            p[2 * j] = pack_half2(e[0], e[1]);
            p[2 * j + 1] = pack_half2(e[2], e[3]);
        }
#pragma unroll
        for (int r = 0; r < 2; ++r) {
            tile_sum[r] += __shfl_xor_sync(0xffffffffu, tile_sum[r], 1);
            tile_sum[r] += __shfl_xor_sync(0xffffffffu, tile_sum[r], 2);
            row_sum[r] = row_sum[r] * alpha[r] + tile_sum[r];
        }
        if (alpha[0] != 1.0f || alpha[1] != 1.0f) {
#pragma unroll
            for (int i = 0; i < kDim / 8; ++i) {
                o[i][0] *= alpha[0];
                o[i][1] *= alpha[0];
                o[i][2] *= alpha[1];
                o[i][3] *= alpha[1];
            }
        }
        // O += P V
#pragma unroll
        for (int d = 0; d < kDim / 16; ++d) {
            const int mat = lane / 8;
            uint32_t r[4];
            ldmatrix_x4_trans(r, vt + ((mat % 2) * 8 + (lane % 8)) * kLds + d * 16 + (mat / 2) * 8);
#pragma unroll
            for (int h = 0; h < 2; ++h) {
                float *acc = o[2 * d + h];
                asm volatile(
                    "mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32 {%0,%1,%2,%3}, {%4,%5,%6,%7}, {%8,%9}, "
                    "{%0,%1,%2,%3};\n"
                    : "+f"(acc[0]), "+f"(acc[1]), "+f"(acc[2]), "+f"(acc[3])
                    : "r"(p[0]), "r"(p[1]), "r"(p[2]), "r"(p[3]), "r"(r[2 * h]), "r"(r[2 * h + 1]));
            }
        }
        __syncthreads();
    }
    // out = O / l
#pragma unroll
    for (int r = 0; r < 2; ++r) {
        const int t = t0 + g + 8 * r;
        if (t >= n) {
            continue;
        }
        const float inv = 1.0f / row_sum[r];
        float *dst = out + (static_cast<long long>(t) * heads + head) * kDim;
#pragma unroll
        for (int i = 0; i < kDim / 8; ++i) {
            *reinterpret_cast<float2 *>(dst + i * 8 + 2 * c) = make_float2(o[i][2 * r] * inv, o[i][2 * r + 1] * inv);
        }
    }
}

// ---- the joint head on the device (#11446) ----
//
// The whole head after the trunk runs in one submission
// (ClefCudaTrunk::head_device). These kernels are the pieces between its
// cuBLAS products and follow the host head (clef/head.rs) step by step:
// f32 dots in the host's eight-lane order, softmax with f64 exponentials,
// LayerNorm and L2 statistics in f64, and products and sums rounded one at
// a time (no fused multiply-add), so the sequential pieces round like the
// host's.

constexpr unsigned kFull = 0xffffffffu;

__device__ __forceinline__ double warp_sum_f64(double value) {
    for (int offset = kWarp / 2; offset > 0; offset >>= 1) {
        value += __shfl_xor_sync(kFull, value, offset);
    }
    return value;
}

__device__ __forceinline__ double block_sum_f64(double value, double *scratch) {
    const int lane = threadIdx.x % kWarp;
    const int warp = threadIdx.x / kWarp;
    value = warp_sum_f64(value);
    __syncthreads();
    if (lane == 0) {
        scratch[warp] = value;
    }
    __syncthreads();
    const int warps = (blockDim.x + kWarp - 1) / kWarp;
    double total = 0.0;
    for (int i = 0; i < warps; ++i) {
        total += scratch[i];
    }
    return total;
}

// The host `dot` over n elements `term(i)` (already a rounded product):
// eight lane sums in order, the lanes summed 0..7, then the tail. Called by
// a whole warp; every lane gets the result.
template <typename Term>
__device__ __forceinline__ float head_dot8(Term term, int n) {
    const int lane = threadIdx.x % kWarp;
    const int chunks = n / 8;
    float acc = 0.0f;
    if (lane < 8) {
        for (int c = 0; c < chunks; ++c) {
            acc = __fadd_rn(acc, term(c * 8 + lane));
        }
    }
    float total = 0.0f;
    for (int i = 0; i < 8; ++i) {
        total = __fadd_rn(total, __shfl_sync(kFull, acc, i));
    }
    float tail = 0.0f;
    for (int i = chunks * 8; i < n; ++i) {
        tail = __fadd_rn(tail, term(i));
    }
    return __fadd_rn(total, tail);
}

// The host `l2_normalize` scale of n elements `value(i)`: 1 / max(|v|, eps)
// with |v| summed in f64. Called by a whole warp.
template <typename Value>
__device__ __forceinline__ float head_l2_scale(Value value, int n, float eps) {
    const int lane = threadIdx.x % kWarp;
    double sum = 0.0;
    for (int i = lane; i < n; i += kWarp) {
        const double v = static_cast<double>(value(i));
        sum += v * v;
    }
    const float norm = static_cast<float>(sqrt(warp_sum_f64(sum)));
    return __fdiv_rn(1.0f, fmaxf(norm, eps));
}

// The host softmax of `values[0..n)` in place (one thread).
__device__ void head_softmax_serial(float *values, int n) {
    float best = -INFINITY;
    for (int i = 0; i < n; ++i) {
        best = fmaxf(best, values[i]);
    }
    double total = 0.0;
    for (int i = 0; i < n; ++i) {
        total += exp(static_cast<double>(__fsub_rn(values[i], best)));
    }
    for (int i = 0; i < n; ++i) {
        values[i] = static_cast<float>(exp(static_cast<double>(__fsub_rn(values[i], best))) / total);
    }
}

__device__ __forceinline__ float head_gelu(float value) {
    const double x = static_cast<double>(value);
    return static_cast<float>(0.5 * x * (1.0 + erf(x / 1.4142135623730951)));
}

// out[r] = LayerNorm(x[r]) * w + b with the statistics in f64 (the host
// head's `layer_norm`), one block per row.
__global__ void head_layer_norm_kernel(const float *x, const float *w, const float *b, float *out, int d, float eps) {
    __shared__ double scratch[32];
    const float *row = x + static_cast<long long>(blockIdx.x) * d;
    double sum = 0.0;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        sum += static_cast<double>(row[i]);
    }
    const double mean = block_sum_f64(sum, scratch) / d;
    double var = 0.0;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        const double c = static_cast<double>(row[i]) - mean;
        var = __dadd_rn(var, __dmul_rn(c, c));
    }
    const double inverse = 1.0 / sqrt(block_sum_f64(var, scratch) / d + static_cast<double>(eps));
    float *dst = out + static_cast<long long>(blockIdx.x) * d;
    for (int i = threadIdx.x; i < d; i += blockDim.x) {
        const float v = static_cast<float>(__dmul_rn(static_cast<double>(row[i]) - mean, inverse));
        dst[i] = __fadd_rn(__fmul_rn(v, w[i]), b[i]);
    }
}

// x += y.
__global__ void head_add_kernel(float *x, const float *y, long long count) {
    const long long i = blockIdx.x * static_cast<long long>(blockDim.x) + threadIdx.x;
    if (i < count) {
        x[i] = __fadd_rn(x[i], y[i]);
    }
}

// routed[o] = context[o] + lexical[o] + question[owner[o]] (each [., w]).
__global__ void head_route_kernel(const float *context, const float *lexical, const float *question, const int *owner,
                                  float *routed, int options, int w) {
    const long long i = blockIdx.x * static_cast<long long>(blockDim.x) + threadIdx.x;
    if (i >= static_cast<long long>(options) * w) {
        return;
    }
    const int o = static_cast<int>(i / w);
    const int c = static_cast<int>(i % w);
    routed[i] = __fadd_rn(__fadd_rn(context[i], lexical[i]), question[static_cast<long long>(owner[o]) * w + c]);
}

// scores[o] = routed[o] . base[owner[o]] / sqrt(w), one warp per option.
__global__ void head_route_scores_kernel(const float *routed, const float *base, const int *owner, float *scores,
                                         int w) {
    const int o = blockIdx.x;
    const float *r = routed + static_cast<long long>(o) * w;
    const float *f = base + static_cast<long long>(owner[o]) * w;
    const float dot = head_dot8([&](int i) { return __fmul_rn(r[i], f[i]); }, w);
    if (threadIdx.x == 0) {
        scores[o] = __fdiv_rn(dot, sqrtf(static_cast<float>(w)));
    }
}

// One block per question: the routing softmax over its options' scores
// (meta [questions, 3]: first option, option count, question type), the
// weighted option summary, its LayerNorm, and
// fields[q] = base[q] + summary + global + type_embedding[type].
// Dynamic shared memory: w floats.
__global__ void head_fields_kernel(const float *routed, float *scores, const float *base, const float *global,
                                   const float *types, const int *meta, const float *norm_w, const float *norm_b,
                                   float *fields, int w, float eps) {
    extern __shared__ float summary[];
    __shared__ double scratch[32];
    const int q = blockIdx.x;
    const int first = meta[3 * q];
    const int count = meta[3 * q + 1];
    const int type = meta[3 * q + 2];
    if (threadIdx.x == 0) {
        head_softmax_serial(scores + first, count);
    }
    __syncthreads();
    for (int c = threadIdx.x; c < w; c += blockDim.x) {
        float acc = 0.0f;
        for (int k = 0; k < count; ++k) {
            acc = __fadd_rn(acc, __fmul_rn(scores[first + k], routed[static_cast<long long>(first + k) * w + c]));
        }
        summary[c] = acc;
    }
    __syncthreads();
    double sum = 0.0;
    for (int c = threadIdx.x; c < w; c += blockDim.x) {
        sum += static_cast<double>(summary[c]);
    }
    const double mean = block_sum_f64(sum, scratch) / w;
    double var = 0.0;
    for (int c = threadIdx.x; c < w; c += blockDim.x) {
        const double d = static_cast<double>(summary[c]) - mean;
        var = __dadd_rn(var, __dmul_rn(d, d));
    }
    const double inverse = 1.0 / sqrt(block_sum_f64(var, scratch) / w + static_cast<double>(eps));
    for (int c = threadIdx.x; c < w; c += blockDim.x) {
        const float v = static_cast<float>(__dmul_rn(static_cast<double>(summary[c]) - mean, inverse));
        const float s = __fadd_rn(__fmul_rn(v, norm_w[c]), norm_b[c]);
        const long long at = static_cast<long long>(q) * w + c;
        fields[at] = __fadd_rn(__fadd_rn(__fadd_rn(base[at], s), global[c]), types[static_cast<long long>(type) * w + c]);
    }
}

// The field layers' self-attention over the `rows` questions, before the
// output projection: one warp per (row, head); `scores` [rows, heads, rows].
__global__ void head_self_attention_kernel(const float *q, const float *k, const float *v, float *scores,
                                           float *context, int rows, int heads, int w, float scale) {
    const int i = blockIdx.x / heads;
    const int h = blockIdx.x % heads;
    const int hd = w / heads;
    const int lane = threadIdx.x % kWarp;
    const float *query = q + static_cast<long long>(i) * w + h * hd;
    float *row = scores + static_cast<long long>(blockIdx.x) * rows;
    for (int j = 0; j < rows; ++j) {
        const float *key = k + static_cast<long long>(j) * w + h * hd;
        const float dot = head_dot8([&](int e) { return __fmul_rn(query[e], key[e]); }, hd);
        if (lane == 0) {
            row[j] = __fmul_rn(dot, scale);
        }
    }
    __syncwarp();
    if (lane == 0) {
        head_softmax_serial(row, rows);
    }
    __syncwarp();
    for (int e = h * hd + lane; e < (h + 1) * hd; e += kWarp) {
        float acc = 0.0f;
        for (int j = 0; j < rows; ++j) {
            acc = __fadd_rn(acc, __fmul_rn(row[j], v[static_cast<long long>(j) * w + e]));
        }
        context[static_cast<long long>(i) * w + e] = acc;
    }
}

// features[o] = [f, x, f * x, |f - x|] with f = fields[owner[o]] and
// x = options[o].
__global__ void head_features_kernel(const float *fields, const float *options, const int *owner, float *features,
                                     int w) {
    const int o = blockIdx.x;
    const float *f = fields + static_cast<long long>(owner[o]) * w;
    const float *x = options + static_cast<long long>(o) * w;
    float *dst = features + static_cast<long long>(o) * 4 * w;
    for (int c = threadIdx.x; c < w; c += blockDim.x) {
        dst[c] = f[c];
        dst[w + c] = x[c];
        dst[2 * w + c] = __fmul_rn(f[c], x[c]);
        dst[3 * w + c] = fabsf(__fsub_rn(f[c], x[c]));
    }
}

// One warp per option: the host head's final logit
//   prior_scale * unit(lexical[o]) . unit(question[q] + global)
//   + gate * (joint_scale * unit(fields[q]) . unit(options[o])
//             + sum_c gelu(hidden[o, c]) scorer_out[c] + scorer_out_bias)
// for q = owner[o] (lexical, question and global d wide, the rest w wide).
// Dynamic shared memory: w floats per block.
__global__ void head_logits_kernel(const float *lexical, const float *questions, const float *global,
                                   const int *owner, const float *fields, const float *options, const float *hidden,
                                   const float *scorer_out, float *logits, int d, int w, float scorer_out_bias,
                                   float prior_scale, float joint_scale, float gate) {
    extern __shared__ float terms[];
    const int o = blockIdx.x;
    const int q = owner[o];
    const int lane = threadIdx.x % kWarp;
    const float *lex = lexical + static_cast<long long>(o) * d;
    const float *question = questions + static_cast<long long>(q) * d;
    const float *field = fields + static_cast<long long>(q) * w;
    const float *option = options + static_cast<long long>(o) * w;
    const float *h = hidden + static_cast<long long>(o) * w;
    auto anchor = [&](int i) { return __fadd_rn(question[i], global[i]); };
    const float anchor_scale = head_l2_scale(anchor, d, 1e-12f);
    const float lexical_scale = head_l2_scale([&](int i) { return lex[i]; }, d, 1e-12f);
    const float prior = __fmul_rn(
        prior_scale,
        head_dot8([&](int i) { return __fmul_rn(__fmul_rn(lex[i], lexical_scale), __fmul_rn(anchor(i), anchor_scale)); },
                  d));
    const float field_scale = head_l2_scale([&](int i) { return field[i]; }, w, 1e-8f);
    const float option_scale = head_l2_scale([&](int i) { return option[i]; }, w, 1e-8f);
    const float cosine = head_dot8(
        [&](int i) { return __fmul_rn(__fmul_rn(field[i], field_scale), __fmul_rn(option[i], option_scale)); }, w);
    for (int c = lane; c < w; c += kWarp) {
        terms[c] = __fmul_rn(head_gelu(h[c]), scorer_out[c]);
    }
    __syncwarp();
    if (lane == 0) {
        float sum = 0.0f;
        for (int c = 0; c < w; ++c) {
            sum = __fadd_rn(sum, terms[c]);
        }
        const float residual = __fadd_rn(sum, scorer_out_bias);
        const float joint = __fadd_rn(__fmul_rn(joint_scale, cosine), residual);
        logits[o] = __fadd_rn(prior, __fmul_rn(gate, joint));
    }
}

inline int blocks_for(long long count, int threads) {
    return static_cast<int>((count + threads - 1) / threads);
}

inline int threads_for(int width) {
    int t = ((width + 31) / 32) * 32;
    return t > 1024 ? 1024 : (t < 32 ? 32 : t);
}

}  // namespace

#define STREAM static_cast<cudaStream_t>(stream)
#define DONE return static_cast<int>(cudaGetLastError())

extern "C" int psionic_clef_dequant_q8_0_f16(const void *src, void *dst, long long blocks, void *stream) {
    const long long groups = blocks * 8;
    dequant_q8_0_kernel<<<blocks_for(groups, 256), 256, 0, STREAM>>>(
        static_cast<const uint8_t *>(src), static_cast<__half *>(dst), groups);
    DONE;
}

extern "C" int psionic_clef_dequant_q4_k_f16(const void *src, void *dst, long long super_blocks, void *stream) {
    const long long groups = super_blocks * 32;
    dequant_q4_k_kernel<<<blocks_for(groups, 256), 256, 0, STREAM>>>(
        static_cast<const uint8_t *>(src), static_cast<__half *>(dst), groups);
    DONE;
}

extern "C" int psionic_clef_f16_to_f32(const void *src, void *dst, long long count, int accumulate, void *stream);

// Gathers n token rows of a Q4_K [vocab, hidden] table into f32 out [n, hidden]
// (hidden a multiple of 256; ids checked by the caller).
extern "C" int psionic_clef_embed_q4_k(const void *table, const void *ids, void *out, int n, int hidden, void *stream) {
    if (n <= 0) {
        return 0;
    }
    if (hidden % 256 != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    embed_q4_k_kernel<<<n, 256, 0, STREAM>>>(static_cast<const uint8_t *>(table), static_cast<const int *>(ids),
                                             static_cast<float *>(out), hidden,
                                             static_cast<long long>(hidden) / 256 * 144);
    DONE;
}

extern "C" int psionic_clef_f32_to_f16(const void *src, void *dst, long long count, void *stream) {
    f32_to_f16_kernel<<<blocks_for(count, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(src), static_cast<__half *>(dst), count);
    DONE;
}

extern "C" int psionic_clef_rms_norm_to_f16(const void *x, const void *w, void *out, int rows, int d, float eps, void *stream) {
    rms_norm_to_f16_kernel<<<rows, 256, 0, STREAM>>>(
        static_cast<const float *>(x), static_cast<const float *>(w), static_cast<__half *>(out), d, eps);
    DONE;
}

extern "C" int psionic_clef_rms_norm_f32(const void *x, const void *w, void *out, int rows, int d, float eps, void *stream) {
    rms_norm_f32_kernel<<<rows, 256, 0, STREAM>>>(
        static_cast<const float *>(x), static_cast<const float *>(w), static_cast<float *>(out), d, eps);
    DONE;
}

extern "C" int psionic_clef_layer_norm_f32(const void *x, const void *w, const void *b, void *out, int rows, int d, float eps, void *stream) {
    layer_norm_f32_kernel<<<rows, 256, 0, STREAM>>>(
        static_cast<const float *>(x), static_cast<const float *>(w), static_cast<const float *>(b),
        static_cast<float *>(out), d, eps);
    DONE;
}

extern "C" int psionic_clef_conv1d_seq_silu(const void *in, void *state, const void *w, void *out, int n, int channels, int k, void *stream) {
    if (k < 1 || k > 4) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    if (k == 4) {
        const dim3 grid(blocks_for(channels, 128), (n + kConvRun - 1) / kConvRun);
        conv1d_seq_silu_run_kernel<4><<<grid, 128, 0, STREAM>>>(
            static_cast<const float *>(in), static_cast<const float *>(state), static_cast<const float *>(w),
            static_cast<float *>(out), n, channels);
    } else {
        conv1d_seq_silu_kernel<<<blocks_for(static_cast<long long>(n) * channels, 256), 256, 0, STREAM>>>(
            static_cast<const float *>(in), static_cast<const float *>(state), static_cast<const float *>(w),
            static_cast<float *>(out), n, channels, k);
    }
    conv1d_state_kernel<<<blocks_for(channels, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(in), static_cast<float *>(state), n, channels, k);
    DONE;
}

extern "C" int psionic_clef_conv1d_state_into(const void *in, const void *state, void *out, int n, int channels,
                                             int k, void *stream) {
    if (k < 1 || k > 4 || n < 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    conv1d_state_into_kernel<<<blocks_for(channels, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(in), static_cast<const float *>(state), static_cast<float *>(out), n, channels, k);
    DONE;
}

extern "C" int psionic_clef_delta_prep(const void *conv, const void *alpha, const void *beta_in, const void *ssm_a,
                                       const void *ssm_dt, void *qn, void *kn, void *decay, void *beta, void *kq, int n,
                                       int key_heads, int value_heads, int dim, int conv_width, void *stream) {
    if (dim > 1024 || n <= 0) {
        return n <= 0 ? 0 : static_cast<int>(cudaErrorInvalidValue);
    }
    dim3 grid(n, key_heads);
    delta_prep_kernel<<<grid, threads_for(dim), 0, STREAM>>>(
        static_cast<const float *>(conv), static_cast<const float *>(alpha), static_cast<const float *>(beta_in),
        static_cast<const float *>(ssm_a), static_cast<const float *>(ssm_dt), static_cast<float *>(qn),
        static_cast<float *>(kn), static_cast<float *>(decay), static_cast<float *>(beta), static_cast<float *>(kq),
        key_heads, value_heads, dim, conv_width);
    DONE;
}

extern "C" int psionic_clef_delta_seq(const void *qn, const void *kn, const void *conv, const void *decay,
                                      const void *beta, const void *kq, void *state, void *out, int n, int key_heads, int value_heads,
                                      int dim, int v_head_reordered, int conv_width, int value_offset, int staged, void *stream) {
    if (n <= 0) {
        return 0;
    }
    if (staged && dim == scan::kDim && (value_offset % 4) == 0 && (conv_width % 4) == 0) {
        delta_scan128_kernel<<<value_heads * (scan::kDim / scan::kRowsPerCta), scan::kThreads, 0, STREAM>>>(
            static_cast<const float *>(qn), static_cast<const float *>(kn), static_cast<const float *>(conv),
            static_cast<const float *>(decay), static_cast<const float *>(beta), static_cast<const float *>(kq),
            static_cast<float *>(state), static_cast<float *>(out), n, key_heads, value_heads, v_head_reordered,
            conv_width, value_offset);
        DONE;
    }
    constexpr int kRows = 2;
    const long long threads = static_cast<long long>(value_heads) * dim / kRows * kWarp;
    const int block = 128;
    const int grid = blocks_for(threads, block);
#define DELTA(P)                                                                                              \
    delta_seq_kernel<P, kRows><<<grid, block, 0, STREAM>>>(                                                          \
        static_cast<const float *>(qn), static_cast<const float *>(kn), static_cast<const float *>(conv),     \
        static_cast<const float *>(decay), static_cast<const float *>(beta), static_cast<const float *>(kq),   \
        static_cast<float *>(state),                                                                          \
        static_cast<float *>(out), n, key_heads, value_heads, v_head_reordered, conv_width, value_offset)
    switch (dim) {
        case 64: DELTA(2); break;
        case 128: DELTA(4); break;
        case 256: DELTA(8); break;
        default: return static_cast<int>(cudaErrorInvalidValue);
    }
#undef DELTA
    DONE;
}

// The blocked delta rule (delta_block_scan_kernel) for dim 128: coef is
// scratch of ceil(n / 16) x value_heads x 544 floats; variant 101, 102 or
// 104 sets 1, 2 or 4 warps splitting the columns of a CTA's 16 state rows.
// The chunk must start at an absolute position that is a multiple of 16
// for the result not to depend on the chunking (the caller checks). With
// snap_blocks > 0 the state after that many blocks is also written to snap.
extern "C" int psionic_clef_delta_block(const void *qn, const void *kn, const void *conv, const void *decay,
                                        const void *beta, void *coef, void *state, void *out, int n, int key_heads,
                                        int value_heads, int dim, int v_head_reordered, int conv_width,
                                        int value_offset, int variant, int snap_blocks, void *snap,
                                        void *stream) {
    if (n <= 0) {
        return 0;
    }
    if (dim != bscan::kDim || (value_offset % 4) != 0 || (conv_width % 4) != 0 ||
        (variant != 101 && variant != 102 && variant != 104)) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    const int blocks = (n + bscan::kT - 1) / bscan::kT;
    delta_block_coef_kernel<<<dim3(blocks, key_heads), 256, 0, STREAM>>>(
        static_cast<const float *>(qn), static_cast<const float *>(kn), static_cast<const float *>(decay),
        static_cast<const float *>(beta), static_cast<float *>(coef), n, key_heads, value_heads, v_head_reordered);
#define BLOCK_SCAN(W)                                                                                          \
    do {                                                                                                       \
        static bool configured = false;                                                                        \
        if (!configured) {                                                                                     \
            const cudaError_t code = cudaFuncSetAttribute(                                                     \
                delta_block_scan_kernel<W>, cudaFuncAttributeMaxDynamicSharedMemorySize, bscan_smem_bytes(W)); \
            if (code != cudaSuccess) {                                                                         \
                return static_cast<int>(code);                                                                 \
            }                                                                                                  \
            configured = true;                                                                                 \
        }                                                                                                      \
        delta_block_scan_kernel<W><<<value_heads * (bscan::kDim / 16), 32 * W, bscan_smem_bytes(W), STREAM>>>( \
            static_cast<const float *>(qn), static_cast<const float *>(kn), static_cast<const float *>(conv),  \
            static_cast<const float *>(coef), static_cast<float *>(state), static_cast<float *>(out), n,       \
            key_heads, value_heads, v_head_reordered, conv_width, value_offset, snap_blocks,                   \
            static_cast<float *>(snap));                                                                       \
    } while (0)
    switch (variant) {
        case 101: BLOCK_SCAN(1); break;
        case 102: BLOCK_SCAN(2); break;
        default: BLOCK_SCAN(4); break;
    }
#undef BLOCK_SCAN
    DONE;
}

extern "C" int psionic_clef_gated_norm_to_f16(const void *o, const void *z, const void *w, void *out, int n, int heads, int dim, float eps, void *stream) {
    if (dim > 1024) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    dim3 grid(n, heads);
    gated_norm_to_f16_kernel<<<grid, threads_for(dim), 0, STREAM>>>(
        static_cast<const float *>(o), static_cast<const float *>(z), static_cast<const float *>(w),
        static_cast<__half *>(out), heads, dim, eps);
    DONE;
}

extern "C" int psionic_clef_attention_prep(const void *qg, const void *k, const void *v, const void *qw, const void *kw,
                                           const void *cos_sin, void *q16, void *gate, void *kcache, void *vcache, int n,
                                           int heads, int kv_heads, int dim, int rot, int pos0, float scale, float eps,
                                           void *stream) {
    if (dim > 1024 || rot > dim) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    dim3 grid(n, heads + kv_heads);
    attention_prep_kernel<<<grid, threads_for(dim), 0, STREAM>>>(
        static_cast<const float *>(qg), static_cast<const float *>(k), static_cast<const float *>(v),
        static_cast<const float *>(qw), static_cast<const float *>(kw), static_cast<const float *>(cos_sin),
        static_cast<__half *>(q16), static_cast<float *>(gate), static_cast<__half *>(kcache),
        static_cast<__half *>(vcache), heads, kv_heads, dim, rot, pos0, scale, eps);
    DONE;
}

extern "C" int psionic_clef_causal_softmax_to_f16(const void *scores, void *probs, int rows, int n, int keys, int pos0, void *stream) {
    causal_softmax_to_f16_kernel<<<rows, 256, 0, STREAM>>>(
        static_cast<const float *>(scores), static_cast<__half *>(probs), n, keys, pos0);
    DONE;
}

extern "C" int psionic_clef_softmax_rows_f32(void *scores, int rows, int keys, float scale, void *stream) {
    softmax_rows_f32_kernel<<<rows, 256, 0, STREAM>>>(static_cast<float *>(scores), keys, scale);
    DONE;
}

extern "C" int psionic_clef_sigmoid_gate_to_f16(const void *o, const void *gate, void *out, long long count, void *stream) {
    sigmoid_gate_to_f16_kernel<<<blocks_for(count, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(o), static_cast<const float *>(gate), static_cast<__half *>(out), count);
    DONE;
}

// FP8 simulation (#11445): rounds f16 rows [rows, k] in place to what an
// E4M3 operand with one f32 scale per 128 consecutive k of a row carries:
// scale = 448 / amax of the block, x = f16(e4m3(x * scale) / scale). A GEMM
// on two such operands with f32 accumulation has the numerics of an FP8
// GEMM with per-row, per-128-k scaling (and exact products).
__global__ void fake_e4m3_rows_kernel(__half *x, int k) {
    __shared__ float warp_max[4];
    const long long index = static_cast<long long>(blockIdx.x) * k + blockIdx.y * 128 + threadIdx.x;
    const bool live = blockIdx.y * 128 + threadIdx.x < k;
    const float v = live ? __half2float(x[index]) : 0.0f;
    float m = fabsf(v);
    for (int offset = 16; offset > 0; offset >>= 1) {
        m = fmaxf(m, __shfl_xor_sync(0xffffffffu, m, offset));
    }
    if (threadIdx.x % 32 == 0) {
        warp_max[threadIdx.x / 32] = m;
    }
    __syncthreads();
    const float amax = fmaxf(fmaxf(warp_max[0], warp_max[1]), fmaxf(warp_max[2], warp_max[3]));
    if (!live || amax == 0.0f) {
        return;
    }
    const float scale = 448.0f / amax;
    const __nv_fp8_e4m3 q(v * scale);
    x[index] = __float2half_rn(static_cast<float>(q) / scale);
}

extern "C" int psionic_clef_fake_e4m3_rows(void *x, int rows, int k, void *stream) {
    if (rows <= 0 || k <= 0) {
        return 0;
    }
    fake_e4m3_rows_kernel<<<dim3(rows, (k + 127) / 128), 128, 0, STREAM>>>(static_cast<__half *>(x), k);
    DONE;
}

extern "C" int psionic_clef_silu_mul_to_f16(const void *gate, const void *up, void *out, long long count, void *stream) {
    silu_mul_to_f16_kernel<<<blocks_for(count, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(gate), static_cast<const float *>(up), static_cast<__half *>(out), count);
    DONE;
}

extern "C" int psionic_clef_span_sums(const void *rows, const void *spans, void *sums, int span_count, int d, int first, int n, void *stream) {
    if (span_count <= 0) {
        return 0;
    }
    span_sums_kernel<<<span_count, 256, 0, STREAM>>>(
        static_cast<const float *>(rows), static_cast<const int *>(spans), static_cast<float *>(sums), d, first, n);
    DONE;
}

extern "C" int psionic_clef_f16_to_f32(const void *src, void *dst, long long count, int accumulate, void *stream) {
    f16_to_f32_kernel<<<blocks_for(count, 256), 256, 0, STREAM>>>(
        static_cast<const __half *>(src), static_cast<float *>(dst), count, accumulate);
    DONE;
}

template <int FMT, int SEG>
static int launch_fused_linear(const void *x, const void *w, void *out, int n, int rows, int k, long long row_bytes,
                               int accumulate, cudaStream_t stream) {
    static bool configured = false;
    if (!configured) {
        const cudaError_t code = cudaFuncSetAttribute(fused_linear_kernel<FMT, SEG>,
                                                      cudaFuncAttributeMaxDynamicSharedMemorySize, fused::kSmemBytes);
        if (code != cudaSuccess) {
            return static_cast<int>(code);
        }
        configured = true;
    }
    dim3 grid((n + fused::kTokens - 1) / fused::kTokens, (rows + fused::kRows - 1) / fused::kRows);
    fused_linear_kernel<FMT, SEG><<<grid, fused::kThreads, fused::kSmemBytes, stream>>>(
        static_cast<const __half *>(x), static_cast<const uint8_t *>(w), static_cast<float *>(out), n, rows, k,
        row_bytes, accumulate);
    return static_cast<int>(cudaGetLastError());
}

// out[n, rows] (+)= x16[n, k] . W[rows, k]^T with W in GGUF layout (format 0
// = Q8_0, 1 = Q4_K). `segment` is the f16 accumulation span in 16-wide k
// steps (1, 2, 4, 8 or 16), or 0 for f32 accumulation. k must be a multiple of 64
// (Q4_K: 256) and rows even.
extern "C" int psionic_clef_fused_linear(const void *x, const void *w, void *out, int n, int rows, int k, int format,
                                         int segment, int accumulate, void *stream) {
    if (n <= 0) {
        return 0;
    }
    if (k % fused::kTileK != 0 || rows % 2 != 0 || (format == kFormatQ4K && k % 256 != 0)) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    const long long row_bytes =
        format == kFormatQ8_0 ? static_cast<long long>(k) / 32 * 34 : static_cast<long long>(k) / 256 * 144;
    if (row_bytes % 4 != 0 || (format == kFormatQ4K && row_bytes % 16 != 0)) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
#define FUSED(F, S) launch_fused_linear<F, S>(x, w, out, n, rows, k, row_bytes, accumulate, STREAM)
    if (format == kFormatQ8_0) {
        switch (segment) {
            case 0: return FUSED(kFormatQ8_0, 0);
            case 1: return FUSED(kFormatQ8_0, 1);
            case 2: return FUSED(kFormatQ8_0, 2);
            case 4: return FUSED(kFormatQ8_0, 4);
            case 8: return FUSED(kFormatQ8_0, 8);
            case 16: return FUSED(kFormatQ8_0, 16);
            default: break;
        }
    } else if (format == kFormatQ4K) {
        switch (segment) {
            case 0: return FUSED(kFormatQ4K, 0);
            case 1: return FUSED(kFormatQ4K, 1);
            case 2: return FUSED(kFormatQ4K, 2);
            case 4: return FUSED(kFormatQ4K, 4);
            case 8: return FUSED(kFormatQ4K, 8);
            case 16: return FUSED(kFormatQ4K, 16);
            default: break;
        }
    }
#undef FUSED
    return static_cast<int>(cudaErrorInvalidValue);
}

extern "C" int psionic_clef_linear_f32_ordered(const void *x, const void *w, void *out, int n, int m, int k, void *stream) {
    if (n <= 0) {
        return 0;
    }
    dim3 grid((m + 63) / 64, (n + 63) / 64);
    linear_f32_ordered_kernel<<<grid, 256, 0, STREAM>>>(static_cast<const float *>(x), static_cast<const float *>(w),
                                                        static_cast<float *>(out), n, m, k);
    DONE;
}

extern "C" int psionic_clef_bias_act(void *x, const void *b, long long rows, int cols, int gelu, void *stream) {
    const long long count = rows * cols;
    if (count <= 0) {
        return 0;
    }
    bias_act_kernel<<<blocks_for(count, 256), 256, 0, STREAM>>>(static_cast<float *>(x), static_cast<const float *>(b),
                                                                count, cols, gelu);
    DONE;
}

extern "C" int psionic_clef_head_side(const void *q, const void *wk, void *side, int n, int heads, int width,
                                      void *stream) {
    if (n <= 0) {
        return 0;
    }
    if (heads <= 0 || width % heads != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    head_side_kernel<<<n * heads, 256, 0, STREAM>>>(static_cast<const float *>(q), static_cast<const float *>(wk),
                                                   static_cast<float *>(side), heads, width, width / heads);
    DONE;
}

extern "C" int psionic_clef_head_context(const void *mixed, const void *wv, const void *bv, void *out, int n,
                                         int heads, int width, void *stream) {
    if (n <= 0) {
        return 0;
    }
    if (heads <= 0 || width % heads != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    const long long threads = static_cast<long long>(n) * width * kWarp;
    head_context_kernel<<<blocks_for(threads, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(mixed), static_cast<const float *>(wv), static_cast<const float *>(bv),
        static_cast<float *>(out), n, heads, width, width / heads);
    DONE;
}

// Causal attention of n queries at positions first.. over the caches
// (head_dim 256 and 4 query heads per KV head; see flash_attention256_kernel).
extern "C" int psionic_clef_flash_attention(const void *q16, const void *kcache, const void *vcache, void *out, int n,
                                            int heads, int kv_heads, int dim, int first, void *stream) {
    if (n <= 0) {
        return 0;
    }
    if (dim != flash::kDim || kv_heads <= 0 || heads != kv_heads * flash::kGroup) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    static bool configured = false;
    if (!configured) {
        const cudaError_t code = cudaFuncSetAttribute(flash_attention256_kernel,
                                                      cudaFuncAttributeMaxDynamicSharedMemorySize, flash::kSmemBytes);
        if (code != cudaSuccess) {
            return static_cast<int>(code);
        }
        configured = true;
    }
    dim3 grid((n + flash::kQueries - 1) / flash::kQueries, kv_heads);
    flash_attention256_kernel<<<grid, flash::kThreads, flash::kSmemBytes, STREAM>>>(
        static_cast<const __half *>(q16), static_cast<const __half *>(kcache), static_cast<const __half *>(vcache),
        static_cast<float *>(out), n, heads, kv_heads, first);
    DONE;
}

// ---- a second stream for weight dequantization ----
// The prefill dequantizes the next weight on its own stream while the
// current GEMM runs; events order the two (dequant -> GEMM, and a GEMM
// before the next dequant into the same scratch slot).

extern "C" int psionic_clef_stream_create(void **stream) {
    cudaStream_t created = nullptr;
    const cudaError_t code = cudaStreamCreateWithFlags(&created, cudaStreamNonBlocking);
    *stream = created;
    return static_cast<int>(code);
}

extern "C" int psionic_clef_stream_destroy(void *stream) {
    return static_cast<int>(cudaStreamDestroy(static_cast<cudaStream_t>(stream)));
}

extern "C" int psionic_clef_event_create(void **event) {
    cudaEvent_t created = nullptr;
    const cudaError_t code = cudaEventCreateWithFlags(&created, cudaEventDisableTiming);
    *event = created;
    return static_cast<int>(code);
}

// A timing event (for device timestamps in profiles).
extern "C" int psionic_clef_timing_event_create(void **event) {
    cudaEvent_t created = nullptr;
    const cudaError_t code = cudaEventCreate(&created);
    *event = created;
    return static_cast<int>(code);
}

// Milliseconds between two completed timing events.
extern "C" int psionic_clef_event_elapsed(void *start, void *end, float *ms) {
    return static_cast<int>(cudaEventElapsedTime(ms, static_cast<cudaEvent_t>(start), static_cast<cudaEvent_t>(end)));
}

extern "C" int psionic_clef_event_destroy(void *event) {
    return static_cast<int>(cudaEventDestroy(static_cast<cudaEvent_t>(event)));
}

extern "C" int psionic_clef_event_record(void *event, void *stream) {
    return static_cast<int>(cudaEventRecord(static_cast<cudaEvent_t>(event), STREAM));
}

extern "C" int psionic_clef_stream_wait_event(void *stream, void *event) {
    return static_cast<int>(cudaStreamWaitEvent(STREAM, static_cast<cudaEvent_t>(event), 0));
}

// ---- the joint head on the device (#11446) ----

extern "C" int psionic_clef_head_layer_norm(const void *x, const void *w, const void *b, void *out, int rows, int d,
                                            float eps, void *stream) {
    if (rows <= 0) {
        return 0;
    }
    head_layer_norm_kernel<<<rows, 256, 0, STREAM>>>(static_cast<const float *>(x), static_cast<const float *>(w),
                                                    static_cast<const float *>(b), static_cast<float *>(out), d, eps);
    DONE;
}

extern "C" int psionic_clef_head_add(void *x, const void *y, long long count, void *stream) {
    if (count <= 0) {
        return 0;
    }
    head_add_kernel<<<blocks_for(count, 256), 256, 0, STREAM>>>(static_cast<float *>(x), static_cast<const float *>(y),
                                                               count);
    DONE;
}

extern "C" int psionic_clef_head_route(const void *context, const void *lexical, const void *question,
                                       const void *owner, void *routed, int options, int w, void *stream) {
    const long long count = static_cast<long long>(options) * w;
    if (count <= 0) {
        return 0;
    }
    head_route_kernel<<<blocks_for(count, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(context), static_cast<const float *>(lexical), static_cast<const float *>(question),
        static_cast<const int *>(owner), static_cast<float *>(routed), options, w);
    DONE;
}

extern "C" int psionic_clef_head_route_scores(const void *routed, const void *base, const void *owner, void *scores,
                                              int options, int w, void *stream) {
    if (options <= 0) {
        return 0;
    }
    head_route_scores_kernel<<<options, kWarp, 0, STREAM>>>(static_cast<const float *>(routed),
                                                           static_cast<const float *>(base),
                                                           static_cast<const int *>(owner),
                                                           static_cast<float *>(scores), w);
    DONE;
}

extern "C" int psionic_clef_head_fields(const void *routed, void *scores, const void *base, const void *global,
                                        const void *types, const void *meta, const void *norm_w, const void *norm_b,
                                        void *fields, int questions, int w, float eps, void *stream) {
    if (questions <= 0) {
        return 0;
    }
    head_fields_kernel<<<questions, 256, static_cast<size_t>(w) * sizeof(float), STREAM>>>(
        static_cast<const float *>(routed), static_cast<float *>(scores), static_cast<const float *>(base),
        static_cast<const float *>(global), static_cast<const float *>(types), static_cast<const int *>(meta),
        static_cast<const float *>(norm_w), static_cast<const float *>(norm_b), static_cast<float *>(fields), w, eps);
    DONE;
}

extern "C" int psionic_clef_head_self_attention(const void *q, const void *k, const void *v, void *scores,
                                                void *context, int rows, int heads, int w, float scale,
                                                void *stream) {
    if (rows <= 0) {
        return 0;
    }
    if (heads <= 0 || w % heads != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    head_self_attention_kernel<<<rows * heads, kWarp, 0, STREAM>>>(
        static_cast<const float *>(q), static_cast<const float *>(k), static_cast<const float *>(v),
        static_cast<float *>(scores), static_cast<float *>(context), rows, heads, w, scale);
    DONE;
}

extern "C" int psionic_clef_head_features(const void *fields, const void *options, const void *owner,
                                          void *features, int count, int w, void *stream) {
    if (count <= 0) {
        return 0;
    }
    head_features_kernel<<<count, 256, 0, STREAM>>>(static_cast<const float *>(fields),
                                                   static_cast<const float *>(options),
                                                   static_cast<const int *>(owner), static_cast<float *>(features), w);
    DONE;
}

extern "C" int psionic_clef_head_logits(const void *lexical, const void *questions, const void *global,
                                        const void *owner, const void *fields, const void *options,
                                        const void *hidden, const void *scorer_out, void *logits, int count, int d,
                                        int w, float scorer_out_bias, float prior_scale, float joint_scale,
                                        float gate, void *stream) {
    if (count <= 0) {
        return 0;
    }
    head_logits_kernel<<<count, kWarp, static_cast<size_t>(w) * sizeof(float), STREAM>>>(
        static_cast<const float *>(lexical), static_cast<const float *>(questions),
        static_cast<const float *>(global), static_cast<const int *>(owner), static_cast<const float *>(fields),
        static_cast<const float *>(options), static_cast<const float *>(hidden),
        static_cast<const float *>(scorer_out), static_cast<float *>(logits), d, w, scorer_out_bias, prior_scale,
        joint_scale, gate);
    DONE;
}
