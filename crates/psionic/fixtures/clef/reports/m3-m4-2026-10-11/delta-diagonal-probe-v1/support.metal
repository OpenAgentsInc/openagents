#include <metal_stdlib>
using namespace metal;
constant constexpr uint kSimd = 32;
inline float group_sum(float value, threadgroup float *scratch, uint tid, uint threads) {
    value = simd_sum(value);
    const uint lane = tid % kSimd;
    const uint group = tid / kSimd;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (lane == 0) {
        scratch[group] = value;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float total = 0.0f;
    const uint groups = (threads + kSimd - 1) / kSimd;
    for (uint i = 0; i < groups; ++i) {
        total += scratch[i];
    }
    return total;
}

inline float sigmoid(float x) { return 1.0f / (1.0f + exp(-x)); }
struct DeltaPrepArgs {
    uint key_heads;
    uint value_heads;
    uint dim;
    uint conv_width;
};

// L2-normalize q and k per key head (q also scaled by 1/sqrt(dim)); decay
// and beta per value head. Threadgroup (t, key head), dim threads.
kernel void clef_delta_prep(device const float *conv [[buffer(0)]], device const float *alpha [[buffer(1)]],
                            device const float *beta_in [[buffer(2)]], device const float *ssm_a [[buffer(3)]],
                            device const float *ssm_dt [[buffer(4)]], device float *qn [[buffer(5)]],
                            device float *kn [[buffer(6)]], device float *decay [[buffer(7)]],
                            device float *beta [[buffer(8)]], device float *kq [[buffer(9)]],
                            constant DeltaPrepArgs &a [[buffer(10)]], uint2 tg [[threadgroup_position_in_grid]],
                            uint2 i_v [[thread_position_in_threadgroup]], uint2 threads_v [[threads_per_threadgroup]]) {
    const uint i = i_v.x;
    const uint threads = threads_v.x;
    threadgroup float scratch[32];
    const uint t = tg.x, h = tg.y;
    device const float *row = conv + ulong(t) * a.conv_width;
    const float q = i < a.dim ? row[h * a.dim + i] : 0.0f;
    const float k = i < a.dim ? row[a.key_heads * a.dim + h * a.dim + i] : 0.0f;
    // Match Qwen3.5/FLA: epsilon belongs inside the squared norm.
    const float qinv = rsqrt(group_sum(q * q, scratch, i, threads) + 1e-6f);
    const float kinv = rsqrt(group_sum(k * k, scratch, i, threads) + 1e-6f);
    const float qv = q * qinv * rsqrt(float(a.dim));
    const float kv = k * kinv;
    if (i < a.dim) {
        const ulong base = (ulong(t) * a.key_heads + h) * a.dim + i;
        qn[base] = qv;
        kn[base] = kv;
    }
    const float dot = group_sum(i < a.dim ? qv * kv : 0.0f, scratch, i, threads);
    if (i == 0) {
        kq[ulong(t) * a.key_heads + h] = dot;
        for (uint v = h; v < a.value_heads; v += a.key_heads) {
            const float x = alpha[ulong(t) * a.value_heads + v] + ssm_dt[v];
            const float sp = x > 20.0f ? x : log(1.0f + exp(x));
            decay[ulong(t) * a.value_heads + v] = exp(sp * ssm_a[v]);
            beta[ulong(t) * a.value_heads + v] = sigmoid(beta_in[ulong(t) * a.value_heads + v]);
        }
    }
}

struct DeltaScanArgs {
    uint n;
    uint key_heads;
    uint value_heads;
    uint v_head_reordered;
    uint conv_width;
    uint value_offset;
};

// The gated delta rule for dim 128, sequential in t. A threadgroup of 128
// threads owns 16 R state rows of one value head (4 simdgroups x 4 row
// groups of 8 lanes; each lane holds 16 key entries of R rows); token inputs
// come in tiles of 16 through threadgroup memory. Per token, with S' = g S:
// kv = S'.k, delta = (v - kv) b, S'' = S' + delta k, o = S'.q + delta (k.q).
template <uint R, uint kThreads>
inline void delta_scan128_impl(device const float *qn, device const float *kn, device const float *conv,
                               device const float *decay, device const float *beta, device const float *kq,
                               device float *state, device float *out, constant DeltaScanArgs &a, uint tg, uint tid,
                               threadgroup float4 (*ks)[32], threadgroup float4 (*qs)[32],
                               threadgroup float (*vs)[(kThreads / 8) * R], threadgroup float *gs,
                               threadgroup float *bs, threadgroup float *kqs) {
    constexpr uint kDim = 128, kRowsPerGroup = (kThreads / 8) * R, kTile = 16;
    const uint blocks_per_head = kDim / kRowsPerGroup;
    const uint vh = tg / blocks_per_head;
    const uint row0 = (tg % blocks_per_head) * kRowsPerGroup;
    const uint repeat = a.value_heads / a.key_heads;
    const uint kh = a.v_head_reordered != 0 ? vh % a.key_heads : vh / repeat;
    const uint lane = tid % kSimd;
    const uint warp = tid / kSimd;
    const uint group = lane / 8;
    const uint l = lane % 8;
    const uint local_row = (warp * 4 + group) * R;  // 4 row groups per simdgroup
    float4 s[R][4];
#pragma unroll
    for (uint r = 0; r < R; ++r) {
        device const float4 *row = (device const float4 *)(state + (ulong(vh) * kDim + row0 + local_row + r) * kDim);
#pragma unroll
        for (uint i = 0; i < 4; ++i) {
            s[r][i] = row[l + 8 * i];
        }
    }
    const uint n = a.n;
    for (uint t0 = 0; t0 < n; t0 += kTile) {
        for (uint c = tid; c < kTile * 32 * 2; c += kThreads) {
            const uint t = c / 64;
            const uint which = (c / 32) % 2;
            const uint e = c % 32;
            if (t0 + t < n) {
                device const float4 *src =
                    (device const float4 *)((which != 0 ? qn : kn) + (ulong(t0 + t) * a.key_heads + kh) * kDim);
                if (which != 0) {
                    qs[t][e] = src[e];
                } else {
                    ks[t][e] = src[e];
                }
            }
        }
        for (uint c = tid; c < kTile * kRowsPerGroup; c += kThreads) {
            const uint t = c / kRowsPerGroup;
            const uint e = c % kRowsPerGroup;
            if (t0 + t < n) {
                vs[t][e] = conv[ulong(t0 + t) * a.conv_width + a.value_offset + vh * kDim + row0 + e];
            }
        }
        if (tid < kTile && t0 + tid < n) {
            gs[tid] = decay[ulong(t0 + tid) * a.value_heads + vh];
            bs[tid] = beta[ulong(t0 + tid) * a.value_heads + vh];
            kqs[tid] = kq[ulong(t0 + tid) * a.key_heads + kh];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        const uint count = min(kTile, n - t0);
        for (uint t = 0; t < count; ++t) {
            float4 kr[4], qr[4];
#pragma unroll
            for (uint i = 0; i < 4; ++i) {
                kr[i] = ks[t][l + 8 * i];
                qr[i] = qs[t][l + 8 * i];
            }
            const float g = gs[t], b = bs[t], k_dot_q = kqs[t];
            float sk[R], sq[R];
#pragma unroll
            for (uint r = 0; r < R; ++r) {
                float4 ak = 0.0f, aq = 0.0f;
#pragma unroll
                for (uint i = 0; i < 4; ++i) {
                    s[r][i] *= g;
                    ak = fma(s[r][i], kr[i], ak);
                    aq = fma(s[r][i], qr[i], aq);
                }
                sk[r] = (ak.x + ak.y) + (ak.z + ak.w);
                sq[r] = (aq.x + aq.y) + (aq.z + aq.w);
            }
#pragma unroll
            for (uint offset = 4; offset > 0; offset >>= 1) {
#pragma unroll
                for (uint r = 0; r < R; ++r) {
                    sk[r] += simd_shuffle_xor(sk[r], ushort(offset));
                    sq[r] += simd_shuffle_xor(sq[r], ushort(offset));
                }
            }
            const ulong out_base = (ulong(t0 + t) * a.value_heads + vh) * kDim + row0;
#pragma unroll
            for (uint r = 0; r < R; ++r) {
                const float delta = (vs[t][local_row + r] - sk[r]) * b;
#pragma unroll
                for (uint i = 0; i < 4; ++i) {
                    s[r][i] = fma(kr[i], float4(delta), s[r][i]);
                }
                if (l == r) {
                    out[out_base + local_row + r] = sq[r] + delta * k_dot_q;
                }
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
#pragma unroll
    for (uint r = 0; r < R; ++r) {
        device float4 *row = (device float4 *)(state + (ulong(vh) * kDim + row0 + local_row + r) * kDim);
#pragma unroll
        for (uint i = 0; i < 4; ++i) {
            row[l + 8 * i] = s[r][i];
        }
    }
}

#define CLEF_DELTA_SCAN(NAME, R, THREADS)                                                                       \
    kernel void NAME(device const float *qn [[buffer(0)]], device const float *kn [[buffer(1)]],                 \
                     device const float *conv [[buffer(2)]], device const float *decay [[buffer(3)]],             \
                     device const float *beta [[buffer(4)]], device const float *kq [[buffer(5)]],                \
                     device float *state [[buffer(6)]], device float *out [[buffer(7)]],                          \
                     constant DeltaScanArgs &a [[buffer(8)]], uint tg [[threadgroup_position_in_grid]],           \
                     uint tid [[thread_position_in_threadgroup]]) {                                              \
        threadgroup float4 ks[16][32];                                                                           \
        threadgroup float4 qs[16][32];                                                                           \
        threadgroup float vs[16][(THREADS / 8) * R];                                                             \
        threadgroup float gs[16], bs[16], kqs[16];                                                               \
        delta_scan128_impl<R, THREADS>(qn, kn, conv, decay, beta, kq, state, out, a, tg, tid, ks, qs, vs, gs, bs, \
                                       kqs);                                                                     \
    }

// Rows per threadgroup (THREADS / 8) * R: 128 is one value head.
CLEF_DELTA_SCAN(clef_delta_scan128, 2, 512)
CLEF_DELTA_SCAN(clef_delta_scan128_256x2, 2, 256)
CLEF_DELTA_SCAN(clef_delta_scan128_128x2, 2, 128)
CLEF_DELTA_SCAN(clef_delta_scan128_1024x1, 1, 1024)


// Synthetic fixtures already contain normalized/scaled Q and K. This uses
// the exact same group_sum operation as production delta_prep for K·Q.
kernel void probe_prepared_dot(
    device const float *q [[buffer(0)]], device const float *k [[buffer(1)]],
    device float *kq [[buffer(2)]], constant uint2 &shape [[buffer(3)]],
    uint3 tg [[threadgroup_position_in_grid]], uint tid [[thread_index_in_threadgroup]],
    uint3 threads [[threads_per_threadgroup]]) {
    if (tg.x >= shape.x || tg.y >= shape.y || threads.x != 128 || threads.y != 1 || threads.z != 1) return;
    threadgroup float scratch[32];
    const ulong base = (ulong(tg.x) * shape.y + tg.y) * 128;
    const float dot = group_sum(q[base + tid] * k[base + tid], scratch, tid, 128);
    if (tid == 0) kq[ulong(tg.x) * shape.y + tg.y] = dot;
}
