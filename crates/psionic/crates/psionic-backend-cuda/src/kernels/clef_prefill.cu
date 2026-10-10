// Sequence-prefill kernels for the Clef decision lane (qwen35 hybrid trunk +
// joint-head memory), OpenAgentsInc/openagents #11195.
//
// The trunk runs a chunk of tokens through one layer at a time:
// projections are dequantize-to-f16 + cuBLAS GEMMs (launched from Rust);
// these kernels do everything between the GEMMs. Every reduction has a fixed
// order (no atomics), so a repeated launch is bitwise identical.

#include <cuda_fp16.h>
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
    const float qnorm = fmaxf(sqrtf(block_sum(q * q, scratch)), 1e-6f);
    const float knorm = fmaxf(sqrtf(block_sum(k * k, scratch)), 1e-6f);
    const float qv = q / qnorm * rsqrtf(static_cast<float>(dim));
    const float kv = k / knorm;
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
    conv1d_seq_silu_kernel<<<blocks_for(static_cast<long long>(n) * channels, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(in), static_cast<const float *>(state), static_cast<const float *>(w),
        static_cast<float *>(out), n, channels, k);
    conv1d_state_kernel<<<blocks_for(channels, 256), 256, 0, STREAM>>>(
        static_cast<const float *>(in), static_cast<float *>(state), n, channels, k);
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
                                      int dim, int v_head_reordered, int conv_width, int value_offset, void *stream) {
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

extern "C" int psionic_clef_event_destroy(void *event) {
    return static_cast<int>(cudaEventDestroy(static_cast<cudaEvent_t>(event)));
}

extern "C" int psionic_clef_event_record(void *event, void *stream) {
    return static_cast<int>(cudaEventRecord(static_cast<cudaEvent_t>(event), STREAM));
}

extern "C" int psionic_clef_stream_wait_event(void *stream, void *event) {
    return static_cast<int>(cudaStreamWaitEvent(STREAM, static_cast<cudaEvent_t>(event), 0));
}
