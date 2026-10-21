// DeepSeek-V4.1 hyper-connection mixing, Engram gather and gating, and
// expert routing kernels (#12393).
//
// Every kernel repeats the CPU path's operations (`deepseek_v41_cpu`) in the
// same order wherever the order doesn't cost parallelism: element-wise steps,
// the Sinkhorn normalization, the four-term stream sums, the top-k rule, and
// the expert sums. Long reductions (the mixing projection, RMS and dot
// products, and the router logits) use a fixed parallel order instead, which
// `v41_mix.rs` documents and its host models repeat. Arithmetic uses explicit
// round-to-nearest intrinsics, so the compiler can't contract or reorder it.
//
// `exp` and `log1p` run in double precision and are rounded to float once,
// so they're almost always the correctly rounded float result. The host
// models do the same, which makes them reproduce the device bit for bit.
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
constexpr int kWarps = kThreads / 32;
// Blocks that split the K dimension of the mixing projection.
constexpr int kHcSplits = 16;
constexpr int kRouteThreads = 1024;

__device__ __forceinline__ float exp_once(float x) {
    return __double2float_rn(exp(static_cast<double>(x)));
}

__device__ __forceinline__ float log1p_once(float x) {
    return __double2float_rn(log1p(static_cast<double>(x)));
}

// The CPU path's `sigmoid`: 1 / (1 + e^-x) for x >= 0, e^x / (1 + e^x) below.
__device__ __forceinline__ float sigmoid(float x) {
    if (x >= 0.0f) {
        return __fdiv_rn(1.0f, __fadd_rn(1.0f, exp_once(-x)));
    }
    const float e = exp_once(x);
    return __fdiv_rn(e, __fadd_rn(1.0f, e));
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

__device__ __forceinline__ float store_value(float value, int round_bf16) {
    return round_bf16 ? bf16_round(value) : value;
}

__device__ __forceinline__ float e4m3(uint32_t byte) {
    const uint32_t magnitude = byte & 0x7fu;
    float value = __fmul_rn(__uint_as_float(magnitude << 20), 0x1p120f);
    if (magnitude == 0x7fu) {
        value = CUDART_NAN_F;
    }
    return __uint_as_float(__float_as_uint(value) | ((byte & 0x80u) << 24));
}

__device__ __forceinline__ float e8m0(uint32_t byte) {
    if (byte == 0xffu) {
        return CUDART_NAN_F;
    }
    return byte == 0 ? __uint_as_float(0x00400000u) : __uint_as_float(byte << 23);
}

// Sums one value per thread of a 256-thread block: a shuffle-down tree in
// each warp (offsets 16, 8, 4, 2, 1), then warp 0's total plus warps 1 to 7
// in order. The result is valid in thread 0 only. Every thread must call it.
__device__ float block_sum(float value, float* scratch) {
    for (int offset = 16; offset > 0; offset >>= 1) {
        value = __fadd_rn(value, __shfl_down_sync(0xffffffffu, value, offset));
    }
    __syncthreads();  // a previous call may still be reading `scratch`
    if ((threadIdx.x & 31) == 0) {
        scratch[threadIdx.x >> 5] = value;
    }
    __syncthreads();
    float total = 0.0f;
    if (threadIdx.x == 0) {
        total = scratch[0];
        for (int w = 1; w < kWarps; ++w) {
            total = __fadd_rn(total, scratch[w]);
        }
    }
    return total;
}

// ---------------------------------------------------------------------------
// Hyper-connection mixing
// ---------------------------------------------------------------------------

// Grid (kHcSplits, mixes + 1). Block (s, y) sums one K slice of row y of the
// projection times the residual, or for y == mixes the squares of the
// residual. Thread t adds the slice's elements t, t + 256, ... in order.
__global__ void __launch_bounds__(kThreads) v41_hc_project_kernel(
    const float* __restrict__ residual,
    const float* __restrict__ weights,
    float* __restrict__ partials,
    int mixes,
    int elements) {
    __shared__ float scratch[kWarps];
    const int split = blockIdx.x;
    const int row = blockIdx.y;
    const int chunk = (elements + kHcSplits - 1) / kHcSplits;
    const int begin = split * chunk;
    const int end = min(elements, begin + chunk);
    const float* w = weights + static_cast<long long>(row) * elements;
    float acc = 0.0f;
    for (int i = begin + threadIdx.x; i < end; i += kThreads) {
        const float x = residual[i];
        acc = __fadd_rn(acc, __fmul_rn(row < mixes ? w[i] : x, x));
    }
    const float total = block_sum(acc, scratch);
    if (threadIdx.x == 0) {
        partials[row * kHcSplits + split] = total;
    }
}

// Pre and post coefficients and the Sinkhorn-normalized residual mix, from
// lane i's logit `logit`, as `HyperConnections::split_sinkhorn` computes them.
// Lanes 0 to mixes - 1 hold logits in the CPU's order (pre, post, then comb
// row-major `[source, target]`); all 32 lanes must call it. Returns lane i's
// output value.
__device__ float split_sinkhorn(
    float logit,
    int streams,
    float mix_epsilon,
    int iterations) {
    const int lane = threadIdx.x & 31;
    const int h = streams;
    const int mixes = (h + 2) * h;
    const bool active = lane < mixes;
    const bool bad = __any_sync(0xffffffffu, active && !isfinite(logit));
    if (bad) {
        return CUDART_NAN_F;  // the CPU path refuses nonfinite logits
    }
    // Pre and post lanes compute their value here but stay for the
    // shuffles below. Comb lanes keep their row and column; other lanes
    // follow along with harmless values so every shuffle has all 32 lanes.
    float coefficient = 0.0f;
    if (lane < h) {
        coefficient = __fadd_rn(sigmoid(logit), mix_epsilon);
    } else if (lane < 2 * h) {
        coefficient = __fmul_rn(2.0f, sigmoid(logit));
    }
    const int c = lane - 2 * h;
    const bool comb = c >= 0 && lane < mixes;
    const int r = comb ? c / h : 0;
    const int k = comb ? c % h : 0;
    const int row_base = 2 * h + r * h;
    const int col_base = 2 * h + k;
    // Row softmax plus epsilon.
    float x = logit;
    float max = -CUDART_INF_F;
    for (int j = 0; j < h; ++j) {
        max = fmaxf(max, __shfl_sync(0xffffffffu, x, row_base + j));
    }
    float e = comb ? exp_once(__fsub_rn(x, max)) : 1.0f;
    float sum = __shfl_sync(0xffffffffu, e, row_base);
    for (int j = 1; j < h; ++j) {
        sum = __fadd_rn(sum, __shfl_sync(0xffffffffu, e, row_base + j));
    }
    x = __fadd_rn(__fdiv_rn(e, sum), mix_epsilon);
    for (int iteration = 0; iteration < iterations; ++iteration) {
        if (iteration > 0) {
            float s = __shfl_sync(0xffffffffu, x, row_base);
            for (int j = 1; j < h; ++j) {
                s = __fadd_rn(s, __shfl_sync(0xffffffffu, x, row_base + j));
            }
            x = __fdiv_rn(x, __fadd_rn(s, mix_epsilon));
        }
        float s = __shfl_sync(0xffffffffu, x, col_base);
        for (int j = 1; j < h; ++j) {
            s = __fadd_rn(s, __shfl_sync(0xffffffffu, x, col_base + j * h));
        }
        x = __fdiv_rn(x, __fadd_rn(s, mix_epsilon));
    }
    return lane < 2 * h ? coefficient : x;
}

__device__ __forceinline__ float hc_logit(
    float projected,
    const float* scale,
    const float* base,
    int i,
    int h) {
    const float s = scale[i < h ? 0 : (i < 2 * h ? 1 : 2)];
    return __fadd_rn(__fmul_rn(projected, s), base[i]);
}

// One warp: finishes the projection (partials summed in split order), the
// joint RMS, and the mixes.
__global__ void v41_hc_finish_kernel(
    const float* __restrict__ partials,
    const float* __restrict__ scale,
    const float* __restrict__ base,
    float* __restrict__ projected,
    float* __restrict__ out,
    int streams,
    int elements,
    float norm_epsilon,
    float mix_epsilon,
    int iterations) {
    const int lane = threadIdx.x;
    const int h = streams;
    const int mixes = (h + 2) * h;
    float squares = partials[mixes * kHcSplits];
    for (int s = 1; s < kHcSplits; ++s) {
        squares = __fadd_rn(squares, partials[mixes * kHcSplits + s]);
    }
    const float variance = __fdiv_rn(squares, static_cast<float>(elements));
    const float inv_rms = __frcp_rn(__fsqrt_rn(__fadd_rn(variance, norm_epsilon)));
    float logit = 0.0f;
    if (lane < mixes) {
        float dot = partials[lane * kHcSplits];
        for (int s = 1; s < kHcSplits; ++s) {
            dot = __fadd_rn(dot, partials[lane * kHcSplits + s]);
        }
        const float p = isfinite(variance) ? __fmul_rn(dot, inv_rms) : CUDART_NAN_F;
        projected[lane] = p;
        logit = hc_logit(p, scale, base, lane, h);
    }
    const float value = split_sinkhorn(logit, h, mix_epsilon, iterations);
    if (lane < mixes) {
        out[lane] = value;
    }
}

// One warp: the mixes from already projected values.
__global__ void v41_hc_split_kernel(
    const float* __restrict__ projected,
    const float* __restrict__ scale,
    const float* __restrict__ base,
    float* __restrict__ out,
    int streams,
    float mix_epsilon,
    int iterations) {
    const int lane = threadIdx.x;
    const int h = streams;
    const int mixes = (h + 2) * h;
    const float logit = lane < mixes ? hc_logit(projected[lane], scale, base, lane, h) : 0.0f;
    const float value = split_sinkhorn(logit, h, mix_epsilon, iterations);
    if (lane < mixes) {
        out[lane] = value;
    }
}

// out[d] = sum over j of pre[j] * residual[j, d], added in stream order.
__global__ void __launch_bounds__(kThreads) v41_hc_collapse_kernel(
    const float* __restrict__ residual,
    const float* __restrict__ mixes,
    float* __restrict__ out,
    int streams,
    int width,
    int round_bf16) {
    const int d = blockIdx.x * kThreads + threadIdx.x;
    if (d >= width) {
        return;
    }
    float acc = __fmul_rn(mixes[0], residual[d]);
    for (int j = 1; j < streams; ++j) {
        acc = __fadd_rn(acc, __fmul_rn(mixes[j], residual[static_cast<long long>(j) * width + d]));
    }
    out[d] = store_value(acc, round_bf16);
}

// out[k, d] = post[k] * output[d] + sum over j of comb[j, k] * residual[j, d].
__global__ void __launch_bounds__(kThreads) v41_hc_expand_kernel(
    const float* __restrict__ output,
    const float* __restrict__ residual,
    const float* __restrict__ mixes,
    float* __restrict__ out,
    int streams,
    int width,
    int round_bf16) {
    const long long index = static_cast<long long>(blockIdx.x) * kThreads + threadIdx.x;
    if (index >= static_cast<long long>(streams) * width) {
        return;
    }
    const int k = static_cast<int>(index / width);
    const int d = static_cast<int>(index % width);
    const int h = streams;
    const float* comb = mixes + 2 * h;
    float acc = __fmul_rn(comb[k], residual[d]);
    for (int j = 1; j < h; ++j) {
        acc = __fadd_rn(acc, __fmul_rn(comb[j * h + k], residual[static_cast<long long>(j) * width + d]));
    }
    out[index] = store_value(__fadd_rn(__fmul_rn(mixes[h + k], output[d]), acc), round_bf16);
}

// ---------------------------------------------------------------------------
// Engram
// ---------------------------------------------------------------------------

// One warp per requested row. Each lane reads 8 code bytes at a time and the
// scale byte of their 32-value group, straight from host memory, so only the
// requested rows cross the bus. An id outside the table writes NaN.
__global__ void v41_engram_gather_kernel(
    const uint8_t* __restrict__ codes,
    const uint8_t* __restrict__ scales,
    long long rows,
    const long long* __restrict__ ids,
    float* __restrict__ out,
    int width,
    int round_bf16) {
    const int slot = blockIdx.x;
    const int lane = threadIdx.x;
    const long long id = ids[slot];
    float* row_out = out + static_cast<long long>(slot) * width;
    const bool valid = id >= 0 && id < rows;
    const uint8_t* row = codes + (valid ? id : 0) * width;
    const uint8_t* row_scales = scales + (valid ? id : 0) * (width / 32);
    for (int i = lane * 8; i < width; i += 32 * 8) {
        if (!valid) {
            for (int j = 0; j < 8; ++j) {
                row_out[i + j] = CUDART_NAN_F;
            }
            continue;
        }
        const uint2 packed = *reinterpret_cast<const uint2*>(row + i);
        const float scale = e8m0(row_scales[i / 32]);
        const uint32_t words[2] = {packed.x, packed.y};
#pragma unroll
        for (int q = 0; q < 2; ++q) {
#pragma unroll
            for (int t = 0; t < 4; ++t) {
                const float value = __fmul_rn(e4m3((words[q] >> (8 * t)) & 0xffu), scale);
                row_out[i + q * 4 + t] = store_value(value, round_bf16);
            }
        }
    }
}

// One block per stream: the per-stream RMS of the residual and the key, the
// gated dot product, then residual + gate * value.
__global__ void __launch_bounds__(kThreads) v41_engram_gate_kernel(
    const float* __restrict__ residual,
    const float* __restrict__ projected,
    const float* __restrict__ q_weight,
    const float* __restrict__ k_weight,
    float* __restrict__ out,
    int streams,
    int width,
    float epsilon,
    float inv_sqrt_width,
    int round_bf16) {
    __shared__ float scratch[kWarps];
    __shared__ float gate_shared;
    const int h = blockIdx.x;
    const long long start = static_cast<long long>(h) * width;
    const float* x = residual + start;
    const float* key = projected + start;
    const float* value = projected + static_cast<long long>(streams) * width;
    float xs = 0.0f;
    float ks = 0.0f;
    float dot = 0.0f;
    for (int d = threadIdx.x; d < width; d += kThreads) {
        const float xv = x[d];
        const float kv = key[d];
        xs = __fadd_rn(xs, __fmul_rn(xv, xv));
        ks = __fadd_rn(ks, __fmul_rn(kv, kv));
        const float qk = __fmul_rn(q_weight[start + d], k_weight[start + d]);
        dot = __fadd_rn(dot, __fmul_rn(__fmul_rn(xv, qk), kv));
    }
    xs = block_sum(xs, scratch);
    ks = block_sum(ks, scratch);
    dot = block_sum(dot, scratch);
    if (threadIdx.x == 0) {
        const float xvar = __fdiv_rn(xs, static_cast<float>(width));
        const float kvar = __fdiv_rn(ks, static_cast<float>(width));
        const float rstd = __fmul_rn(
            __frcp_rn(__fsqrt_rn(__fadd_rn(xvar, epsilon))),
            __frcp_rn(__fsqrt_rn(__fadd_rn(kvar, epsilon))));
        const float scaled = __fmul_rn(__fmul_rn(dot, rstd), inv_sqrt_width);
        float gate = CUDART_NAN_F;  // the CPU path refuses these
        if (isfinite(xvar) && isfinite(kvar) && isfinite(scaled)) {
            const float root = __fsqrt_rn(fmaxf(fabsf(scaled), 1e-6f));
            gate = sigmoid(copysignf(root, scaled));
        }
        gate_shared = gate;
    }
    __syncthreads();
    const float gate = gate_shared;
    for (int d = threadIdx.x; d < width; d += kThreads) {
        out[start + d] = store_value(__fadd_rn(x[d], __fmul_rn(gate, value[d])), round_bf16);
    }
}

// ---------------------------------------------------------------------------
// Routing and experts
// ---------------------------------------------------------------------------

// One warp per expert row of the BF16 gate: lane l takes 8-value chunks
// l, l + 32, ... and adds their float32 products in order, then a
// shuffle-down tree adds the lanes.
__global__ void __launch_bounds__(kThreads) v41_gate_logits_kernel(
    const uint16_t* __restrict__ weight,
    const float* __restrict__ input,
    float* __restrict__ logits,
    int experts,
    int cols) {
    const int warp = threadIdx.x >> 5;
    const int lane = threadIdx.x & 31;
    const int row = blockIdx.x * kWarps + warp;
    if (row >= experts) {
        return;  // uniform across the warp
    }
    const uint16_t* w = weight + static_cast<long long>(row) * cols;
    float acc = 0.0f;
    for (int chunk = lane; chunk * 8 < cols; chunk += 32) {
        const uint4 packed = *reinterpret_cast<const uint4*>(w + chunk * 8);
        const uint32_t words[4] = {packed.x, packed.y, packed.z, packed.w};
#pragma unroll
        for (int q = 0; q < 4; ++q) {
            const float lo = __uint_as_float(words[q] << 16);
            const float hi = __uint_as_float(words[q] & 0xffff0000u);
            acc = __fadd_rn(acc, __fmul_rn(lo, input[chunk * 8 + q * 2]));
            acc = __fadd_rn(acc, __fmul_rn(hi, input[chunk * 8 + q * 2 + 1]));
        }
    }
    for (int offset = 16; offset > 0; offset >>= 1) {
        acc = __fadd_rn(acc, __shfl_down_sync(0xffffffffu, acc, offset));
    }
    if (lane == 0) {
        logits[row] = acc;
    }
}

struct Candidate {
    float value;
    int id;
};

// Higher value first; equal values pick the lower id.
__device__ __forceinline__ bool better(Candidate a, Candidate b) {
    return a.value > b.value || (a.value == b.value && a.id < b.id);
}

__device__ __forceinline__ Candidate warp_best(Candidate c) {
    for (int offset = 16; offset > 0; offset >>= 1) {
        Candidate other;
        other.value = __shfl_down_sync(0xffffffffu, c.value, offset);
        other.id = __shfl_down_sync(0xffffffffu, c.id, offset);
        if (better(other, c)) {
            c = other;
        }
    }
    return c;
}

// One block: sqrtsoftplus scores, biased ranking, top-k with ties to the
// lower id, and weights from the unbiased scores. Writes the selected ids in
// ascending id order (the order the reference adds expert outputs) with
// their weights. Any nonfinite logit, scaled logit, or biased score writes
// id -1 and a NaN weight in every slot instead.
__global__ void __launch_bounds__(kRouteThreads) v41_route_kernel(
    const float* __restrict__ logits,
    const float* __restrict__ bias,
    int* __restrict__ ids,
    float* __restrict__ weights,
    int experts,
    int top_k,
    float temperature,
    int normalize,
    float route_scale) {
    __shared__ float scores[kRouteThreads];
    __shared__ unsigned char taken[kRouteThreads];
    __shared__ Candidate warp_winners[kRouteThreads / 32];
    __shared__ int rank_ids[32];
    const int e = threadIdx.x;
    float ranked = -CUDART_INF_F;
    bool bad = false;
    if (e < experts) {
        const float logit = logits[e];
        const float x = __fdiv_rn(logit, temperature);
        // PyTorch softplus returns its input above the threshold 20.
        const float score = x > 20.0f ? __fsqrt_rn(x) : __fsqrt_rn(log1p_once(exp_once(x)));
        ranked = __fadd_rn(score, bias[e]);
        bad = !isfinite(logit) || !isfinite(x) || !isfinite(ranked);
        scores[e] = score;
    }
    taken[e] = 0;
    if (__syncthreads_or(bad)) {
        if (e < top_k) {
            ids[e] = -1;
            weights[e] = CUDART_NAN_F;
        }
        return;
    }
    for (int r = 0; r < top_k; ++r) {
        Candidate c;
        c.value = (e < experts && !taken[e]) ? ranked : -CUDART_INF_F;
        c.id = (e < experts && !taken[e]) ? e : 0x7fffffff;
        c = warp_best(c);
        if ((e & 31) == 0) {
            warp_winners[e >> 5] = c;
        }
        __syncthreads();
        if (e == 0) {
            Candidate best = warp_winners[0];
            for (int w = 1; w < kRouteThreads / 32; ++w) {
                if (better(warp_winners[w], best)) {
                    best = warp_winners[w];
                }
            }
            rank_ids[r] = best.id;
            taken[best.id] = 1;
        }
        __syncthreads();
    }
    if (e == 0) {
        float w[32];
        int id[32];
        for (int r = 0; r < top_k; ++r) {
            id[r] = rank_ids[r];
            w[r] = scores[id[r]];
        }
        if (normalize && top_k > 1) {
            float sum = w[0];
            for (int r = 1; r < top_k; ++r) {
                sum = __fadd_rn(sum, w[r]);
            }
            sum = __fadd_rn(sum, 1e-20f);
            for (int r = 0; r < top_k; ++r) {
                w[r] = __fdiv_rn(w[r], sum);
            }
        }
        for (int r = 0; r < top_k; ++r) {
            w[r] = __fmul_rn(w[r], route_scale);
        }
        // Ascending ids; the ids are distinct.
        for (int i = 1; i < top_k; ++i) {
            const int key = id[i];
            const float value = w[i];
            int j = i - 1;
            while (j >= 0 && id[j] > key) {
                id[j + 1] = id[j];
                w[j + 1] = w[j];
                --j;
            }
            id[j + 1] = key;
            w[j + 1] = value;
        }
        for (int r = 0; r < top_k; ++r) {
            ids[r] = id[r];
            weights[r] = w[r];
        }
    }
}

// Clamped SwiGLU for each slot: the gate clamped above at `limit`, the up
// projection clamped to [-limit, limit], silu(gate) * up, times the slot's
// route weight. NaN inputs give NaN.
__global__ void __launch_bounds__(kThreads) v41_swiglu_kernel(
    const float* __restrict__ gate,
    const float* __restrict__ up,
    const float* __restrict__ route_weights,
    float* __restrict__ out,
    int slots,
    int width,
    float limit,
    int round_bf16) {
    const long long index = static_cast<long long>(blockIdx.x) * kThreads + threadIdx.x;
    if (index >= static_cast<long long>(slots) * width) {
        return;
    }
    float g = gate[index];
    float u = up[index];
    const float w = route_weights == nullptr ? 1.0f : route_weights[index / width];
    float value;
    if (isnan(g) || isnan(u)) {
        value = CUDART_NAN_F;
    } else {
        if (limit > 0.0f) {
            g = fminf(g, limit);
            u = fminf(fmaxf(u, -limit), limit);
        }
        value = __fmul_rn(__fmul_rn(__fmul_rn(g, sigmoid(g)), u), w);
    }
    out[index] = store_value(value, round_bf16);
}

// out[d] = 0 + routed[0, d] + ... + routed[slots - 1, d] (+ shared[d]).
__global__ void __launch_bounds__(kThreads) v41_moe_combine_kernel(
    const float* __restrict__ routed,
    const float* __restrict__ shared,
    float* __restrict__ out,
    int slots,
    int width,
    int round_bf16) {
    const int d = blockIdx.x * kThreads + threadIdx.x;
    if (d >= width) {
        return;
    }
    float acc = 0.0f;
    for (int s = 0; s < slots; ++s) {
        acc = __fadd_rn(acc, routed[static_cast<long long>(s) * width + d]);
    }
    if (shared != nullptr) {
        acc = __fadd_rn(acc, shared[d]);
    }
    out[d] = store_value(acc, round_bf16);
}

unsigned blocks_for(long long n) {
    return static_cast<unsigned>((n + kThreads - 1) / kThreads);
}

}  // namespace

extern "C" int psionic_v41_hc_mixes(
    const void* residual,
    const void* weights,
    const void* scale,
    const void* base,
    void* scratch,
    void* out,
    int streams,
    int width,
    float norm_epsilon,
    float mix_epsilon,
    int iterations,
    void* stream) {
    const cudaStream_t s = static_cast<cudaStream_t>(stream);
    const int mixes = (streams + 2) * streams;
    const int elements = streams * width;
    float* partials = static_cast<float*>(scratch);
    v41_hc_project_kernel<<<dim3(kHcSplits, mixes + 1), kThreads, 0, s>>>(
        static_cast<const float*>(residual), static_cast<const float*>(weights), partials, mixes,
        elements);
    v41_hc_finish_kernel<<<1, 32, 0, s>>>(
        partials, static_cast<const float*>(scale), static_cast<const float*>(base),
        partials + (mixes + 1) * kHcSplits, static_cast<float*>(out), streams, elements,
        norm_epsilon, mix_epsilon, iterations);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_hc_split(
    const void* projected,
    const void* scale,
    const void* base,
    void* out,
    int streams,
    float mix_epsilon,
    int iterations,
    void* stream) {
    v41_hc_split_kernel<<<1, 32, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(projected), static_cast<const float*>(scale),
        static_cast<const float*>(base), static_cast<float*>(out), streams, mix_epsilon,
        iterations);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_hc_collapse(
    const void* residual,
    const void* mixes,
    void* out,
    int streams,
    int width,
    int round_bf16,
    void* stream) {
    v41_hc_collapse_kernel<<<blocks_for(width), kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(residual), static_cast<const float*>(mixes),
        static_cast<float*>(out), streams, width, round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_hc_expand(
    const void* output,
    const void* residual,
    const void* mixes,
    void* out,
    int streams,
    int width,
    int round_bf16,
    void* stream) {
    const long long n = static_cast<long long>(streams) * width;
    v41_hc_expand_kernel<<<blocks_for(n), kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(output), static_cast<const float*>(residual),
        static_cast<const float*>(mixes), static_cast<float*>(out), streams, width, round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_engram_gather(
    const void* codes,
    const void* scales,
    long long rows,
    const void* ids,
    void* out,
    int count,
    int width,
    int round_bf16,
    void* stream) {
    v41_engram_gather_kernel<<<count, 32, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t*>(codes), static_cast<const uint8_t*>(scales), rows,
        static_cast<const long long*>(ids), static_cast<float*>(out), width, round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_engram_gate(
    const void* residual,
    const void* projected,
    const void* q_weight,
    const void* k_weight,
    void* out,
    int streams,
    int width,
    float epsilon,
    float inv_sqrt_width,
    int round_bf16,
    void* stream) {
    v41_engram_gate_kernel<<<streams, kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(residual), static_cast<const float*>(projected),
        static_cast<const float*>(q_weight), static_cast<const float*>(k_weight),
        static_cast<float*>(out), streams, width, epsilon, inv_sqrt_width, round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_gate_logits(
    const void* weight,
    const void* input,
    void* logits,
    int experts,
    int cols,
    void* stream) {
    const unsigned grid = static_cast<unsigned>((experts + kWarps - 1) / kWarps);
    v41_gate_logits_kernel<<<grid, kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint16_t*>(weight), static_cast<const float*>(input),
        static_cast<float*>(logits), experts, cols);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_route(
    const void* logits,
    const void* bias,
    void* ids,
    void* weights,
    int experts,
    int top_k,
    float temperature,
    int normalize,
    float route_scale,
    void* stream) {
    v41_route_kernel<<<1, kRouteThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(logits), static_cast<const float*>(bias), static_cast<int*>(ids),
        static_cast<float*>(weights), experts, top_k, temperature, normalize, route_scale);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_swiglu(
    const void* gate,
    const void* up,
    const void* route_weights,
    void* out,
    int slots,
    int width,
    float limit,
    int round_bf16,
    void* stream) {
    const long long n = static_cast<long long>(slots) * width;
    v41_swiglu_kernel<<<blocks_for(n), kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(gate), static_cast<const float*>(up),
        static_cast<const float*>(route_weights), static_cast<float*>(out), slots, width, limit,
        round_bf16);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_moe_combine(
    const void* routed,
    const void* shared,
    void* out,
    int slots,
    int width,
    int round_bf16,
    void* stream) {
    v41_moe_combine_kernel<<<blocks_for(width), kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(routed), static_cast<const float*>(shared),
        static_cast<float*>(out), slots, width, round_bf16);
    return static_cast<int>(cudaGetLastError());
}
