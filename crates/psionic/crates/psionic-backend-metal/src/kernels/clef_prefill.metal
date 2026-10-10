// Sequence-prefill kernels for the Clef decision lane on Metal
// (OpenAgentsInc/openagents #11196), the counterpart of
// psionic-backend-cuda/src/kernels/clef_prefill.cu.
//
// Projections are f16 x f16 -> f32 GEMMs on the Metal Performance
// Primitives `matmul2d` tensor op (the M5 GPU's neural accelerators), over
// weights dequantized to f16 once at load. Everything between the GEMMs is
// ported from the CUDA kernels with the same arithmetic and the same fixed
// reduction orders, so a token's rows do not depend on the chunk it came in.

#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>

using namespace metal;
using namespace mpp::tensor_ops;

constant constexpr uint kSimd = 32;

// Threadgroup-wide sum for up to 1024 threads (32 simdgroups); every thread
// gets the result. `scratch` holds 32 floats.
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

inline float group_max(float value, threadgroup float *scratch, uint tid, uint threads) {
    value = simd_max(value);
    const uint lane = tid % kSimd;
    const uint group = tid / kSimd;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (lane == 0) {
        scratch[group] = value;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float best = -INFINITY;
    const uint groups = (threads + kSimd - 1) / kSimd;
    for (uint i = 0; i < groups; ++i) {
        best = max(best, scratch[i]);
    }
    return best;
}

inline float silu(float x) { return x / (1.0f + exp(-x)); }
inline float sigmoid(float x) { return 1.0f / (1.0f + exp(-x)); }

// ---- weights: GGML blocks to row-major f16 (once, at load) ----

// Q8_0: 34-byte blocks of 32 (f16 scale, 32 x int8). One thread per value.
kernel void clef_dequant_q8_0(device const uchar *src [[buffer(0)]], device half *dst [[buffer(1)]],
                              constant ulong &count [[buffer(2)]], uint gid [[thread_position_in_grid]]) {
    const ulong i = gid;
    if (i >= count) {
        return;
    }
    const ulong block = i / 32;
    const uint within = uint(i % 32);
    device const uchar *b = src + block * 34;
    const float d = float(as_type<half>(ushort(ushort(b[0]) | (ushort(b[1]) << 8))));
    dst[i] = half(d * float(as_type<char>(b[2 + within])));
}

// Q4_K: 144-byte super-blocks of 256 (f16 d, f16 dmin, 12 scale bytes, 128
// nibble bytes); sub-block j (64 values) takes the low nibbles of
// qs[32j..32j+32] (scale 2j), then the high nibbles (scale 2j+1).
kernel void clef_dequant_q4_k(device const uchar *src [[buffer(0)]], device half *dst [[buffer(1)]],
                              constant ulong &count [[buffer(2)]], uint gid [[thread_position_in_grid]]) {
    const ulong i = gid;
    if (i >= count) {
        return;
    }
    const ulong block = i / 256;
    const uint e = uint(i % 256);
    const uint j = e / 64;
    const uint r = e % 64;
    const bool high = r >= 32;
    const uint is = 2 * j + (high ? 1 : 0);
    device const uchar *b = src + block * 144;
    const float d = float(as_type<half>(ushort(ushort(b[0]) | (ushort(b[1]) << 8))));
    const float dmin = float(as_type<half>(ushort(ushort(b[2]) | (ushort(b[3]) << 8))));
    device const uchar *scales = b + 4;
    uchar sc, m;
    if (is < 4) {
        sc = scales[is] & 63;
        m = scales[is + 4] & 63;
    } else {
        sc = (scales[is + 4] & 0xF) | ((scales[is - 4] >> 6) << 4);
        m = (scales[is + 4] >> 4) | ((scales[is] >> 6) << 4);
    }
    const uchar q = b[16 + 32 * j + (r % 32)];
    const uint nibble = high ? (q >> 4) : (q & 0xF);
    dst[i] = half(fma(d * float(sc), float(nibble), -(dmin * float(m))));
}

// Dense f32 to f16.
kernel void clef_f32_to_f16(device const float *src [[buffer(0)]], device half *dst [[buffer(1)]],
                            constant ulong &count [[buffer(2)]], uint gid [[thread_position_in_grid]]) {
    if (gid < count) {
        dst[gid] = half(src[gid]);
    }
}

// ---- projections: out[n, m] (+)= x16[n, k] . W16[m, k]^T on the tensor op ----
//
// A threadgroup of 4 simdgroups owns a 64 (tokens) x 64 (weight rows) tile
// and runs the whole k extent in the op, with f32 accumulation. The tile
// shape is fixed, so each output element's reduction does not depend on n.

template <bool ACCUMULATE>
inline void clef_gemm_impl(device const half *x, device const half *w, device float *out, uint3 nmk, uint2 tgid) {
    const int n = int(nmk.x), m = int(nmk.y), k = int(nmk.z);
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tx((device half *)x, dextents<int32_t, 2>(k, n));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tw((device half *)w, dextents<int32_t, 2>(k, m));
    tensor<device float, dextents<int32_t, 2>, tensor_inline> to(out, dextents<int32_t, 2>(m, n));
    constexpr auto desc = matmul2d_descriptor(
        64, 64, static_cast<int>(dynamic_extent), false, true, false,
        ACCUMULATE ? matmul2d_descriptor::mode::multiply_accumulate : matmul2d_descriptor::mode::multiply);
    matmul2d<desc, execution_simdgroups<4>> op;
    if (int(tgid.y) * 64 + 63 < n && int(tgid.x) * 64 + 63 < m) {
        // an interior tile: static extents, no edge checks
        auto sx = tx.template slice<dynamic_extent, 64>(0, int(tgid.y) * 64);
        auto sw = tw.template slice<dynamic_extent, 64>(0, int(tgid.x) * 64);
        auto so = to.template slice<64, 64>(int(tgid.x) * 64, int(tgid.y) * 64);
        op.run(sx, sw, so);
    } else {
        auto sx = tx.slice(0, int(tgid.y) * 64);
        auto sw = tw.slice(0, int(tgid.x) * 64);
        auto so = to.slice(int(tgid.x) * 64, int(tgid.y) * 64);
        op.run(sx, sw, so);
    }
}

kernel void clef_gemm(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
                      device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                      uint2 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_impl<false>(x, w, out, nmk, tgid);
}

kernel void clef_gemm_accumulate(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
                                 device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                                 uint2 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_impl<true>(x, w, out, nmk, tgid);
}

// f32 x f32 -> f32 for the head's dense products, the same tiling.
kernel void clef_gemm_f32(device const float *x [[buffer(0)]], device const float *w [[buffer(1)]],
                          device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                          uint2 tgid [[threadgroup_position_in_grid]]) {
    const int n = int(nmk.x), m = int(nmk.y), k = int(nmk.z);
    tensor<device float, dextents<int32_t, 2>, tensor_inline> tx((device float *)x, dextents<int32_t, 2>(k, n));
    tensor<device float, dextents<int32_t, 2>, tensor_inline> tw((device float *)w, dextents<int32_t, 2>(k, m));
    tensor<device float, dextents<int32_t, 2>, tensor_inline> to(out, dextents<int32_t, 2>(m, n));
    constexpr auto desc = matmul2d_descriptor(64, 64, static_cast<int>(dynamic_extent), false, true, false);
    matmul2d<desc, execution_simdgroups<4>> op;
    auto sx = tx.slice(0, int(tgid.y) * 64);
    auto sw = tw.slice(0, int(tgid.x) * 64);
    auto so = to.slice(int(tgid.x) * 64, int(tgid.y) * 64);
    op.run(sx, sw, so);
}

// out[n, m] = x[n, k] . w[k, m] (f32, w row-major [k, m]): the head's
// memory-attention mix, P . M.
kernel void clef_gemm_f32_nn(device const float *x [[buffer(0)]], device const float *w [[buffer(1)]],
                             device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                             uint2 tgid [[threadgroup_position_in_grid]]) {
    const int n = int(nmk.x), m = int(nmk.y), k = int(nmk.z);
    tensor<device float, dextents<int32_t, 2>, tensor_inline> tx((device float *)x, dextents<int32_t, 2>(k, n));
    tensor<device float, dextents<int32_t, 2>, tensor_inline> tw((device float *)w, dextents<int32_t, 2>(m, k));
    tensor<device float, dextents<int32_t, 2>, tensor_inline> to(out, dextents<int32_t, 2>(m, n));
    constexpr auto desc = matmul2d_descriptor(64, 64, static_cast<int>(dynamic_extent), false, false, false);
    matmul2d<desc, execution_simdgroups<4>> op;
    auto sx = tx.slice(0, int(tgid.y) * 64);
    auto sw = tw.slice(int(tgid.x) * 64, 0);
    auto so = to.slice(int(tgid.x) * 64, int(tgid.y) * 64);
    op.run(sx, sw, so);
}

// erf for f32 (Abramowitz & Stegun 7.1.26 is too coarse; this is the
// rational approximation from W. J. Cody's erf, max error ~1e-7 relative),
// so the head's GELU matches the host's f64 erf to f32 precision.
inline float clef_erf(float x) {
    const float ax = fabs(x);
    float r;
    if (ax < 0.5f) {
        const float t = x * x;
        const float num = fma(fma(fma(fma(0.185777706184603153f, t, 3.16112374387056560f), t, 113.864154151050156f), t,
                                  377.485237685302021f), t, 3209.37758913846947f);
        const float den = fma(fma(fma(fma(1.0f, t, 23.6012909523441209f), t, 244.024637934444173f), t,
                                  1282.61652607737228f), t, 2844.23683343917062f);
        return x * num / den;
    } else if (ax < 4.0f) {
        const float num = fma(fma(fma(fma(fma(fma(fma(fma(2.15311535474403846e-8f, ax, 0.564188496988670089f), ax,
                                                          8.88314979438837594f), ax, 66.1191906371416295f), ax,
                                                  298.635138197400131f), ax, 881.952221241769090f), ax,
                                          1712.04761263407058f), ax, 2051.07837782607147f), ax, 1230.33935479799725f);
        const float den = fma(fma(fma(fma(fma(fma(fma(fma(1.0f, ax, 15.7449261107098347f), ax, 117.693950891312499f), ax,
                                                  537.181101862009858f), ax, 1621.38957456669019f), ax,
                                          3290.79923573345963f), ax, 4362.61909014324716f), ax, 3439.36767414372164f), ax,
                              1230.33935480374942f);
        r = 1.0f - exp(-ax * ax) * num / den;
    } else {
        r = 1.0f;
    }
    return x < 0.0f ? -r : r;
}

// x[r, c] += b[c], then GELU (erf form) when `gelu` is set. In place.
kernel void clef_bias_act(device float *x [[buffer(0)]], device const float *b [[buffer(1)]],
                          constant uint3 &rcg [[buffer(2)]], uint gid [[thread_position_in_grid]]) {
    const uint rows = rcg.x, cols = rcg.y, gelu = rcg.z;
    if (gid >= rows * cols) {
        return;
    }
    float v = x[gid] + b[gid % cols];
    if (gelu != 0) {
        v = 0.5f * v * (1.0f + clef_erf(v * 0.70710678118654752f));
    }
    x[gid] = v;
}

// ---- norms ----

// out16[t] = RMSNorm(x[t]) * w (f16); one threadgroup per row.
kernel void clef_rms_norm_to_f16(device const float *x [[buffer(0)]], device const float *w [[buffer(1)]],
                                 device half *out [[buffer(2)]], constant uint &d [[buffer(3)]],
                                 constant float &eps [[buffer(4)]], uint row [[threadgroup_position_in_grid]],
                                 uint tid [[thread_position_in_threadgroup]],
                                 uint threads [[threads_per_threadgroup]]) {
    threadgroup float scratch[32];
    device const float *r = x + ulong(row) * d;
    float sum = 0.0f;
    for (uint i = tid; i < d; i += threads) {
        sum += r[i] * r[i];
    }
    const float scale = rsqrt(group_sum(sum, scratch, tid, threads) / float(d) + eps);
    device half *o = out + ulong(row) * d;
    for (uint i = tid; i < d; i += threads) {
        o[i] = half(r[i] * scale * w[i]);
    }
}

kernel void clef_rms_norm_f32(device const float *x [[buffer(0)]], device const float *w [[buffer(1)]],
                              device float *out [[buffer(2)]], constant uint &d [[buffer(3)]],
                              constant float &eps [[buffer(4)]], uint row [[threadgroup_position_in_grid]],
                              uint tid [[thread_position_in_threadgroup]], uint threads [[threads_per_threadgroup]]) {
    threadgroup float scratch[32];
    device const float *r = x + ulong(row) * d;
    float sum = 0.0f;
    for (uint i = tid; i < d; i += threads) {
        sum += r[i] * r[i];
    }
    const float scale = rsqrt(group_sum(sum, scratch, tid, threads) / float(d) + eps);
    device float *o = out + ulong(row) * d;
    for (uint i = tid; i < d; i += threads) {
        o[i] = r[i] * scale * w[i];
    }
}

// out[t] = LayerNorm(x[t]) (* w + b when affine), two-pass mean/variance.
kernel void clef_layer_norm_f32(device const float *x [[buffer(0)]], device const float *w [[buffer(1)]],
                                device const float *b [[buffer(2)]], device float *out [[buffer(3)]],
                                constant uint &d [[buffer(4)]], constant float &eps [[buffer(5)]],
                                constant uint &affine [[buffer(6)]], uint row [[threadgroup_position_in_grid]],
                                uint tid [[thread_position_in_threadgroup]],
                                uint threads [[threads_per_threadgroup]]) {
    threadgroup float scratch[32];
    device const float *r = x + ulong(row) * d;
    float sum = 0.0f;
    for (uint i = tid; i < d; i += threads) {
        sum += r[i];
    }
    const float mean = group_sum(sum, scratch, tid, threads) / float(d);
    float var = 0.0f;
    for (uint i = tid; i < d; i += threads) {
        const float c = r[i] - mean;
        var += c * c;
    }
    const float inv = rsqrt(group_sum(var, scratch, tid, threads) / float(d) + eps);
    device float *o = out + ulong(row) * d;
    for (uint i = tid; i < d; i += threads) {
        const float v = (r[i] - mean) * inv;
        o[i] = affine != 0 ? v * w[i] + b[i] : v;
    }
}

// ---- Gated DeltaNet ----

// Causal depthwise conv (kernel k <= 4) + SiLU over n tokens with k-1
// carried inputs per channel (oldest first).
kernel void clef_conv1d_seq_silu(device const float *in [[buffer(0)]], device const float *state [[buffer(1)]],
                                 device const float *w [[buffer(2)]], device float *out [[buffer(3)]],
                                 constant uint3 &nck [[buffer(4)]], uint gid [[thread_position_in_grid]]) {
    const uint n = nck.x, channels = nck.y, k = nck.z;
    if (gid >= n * channels) {
        return;
    }
    const int t = int(gid / channels);
    const uint c = gid % channels;
    const int s = int(k) - 1;
    float acc = 0.0f;
    for (int j = 0; j < s; ++j) {
        const int source = t - s + j;
        const float x = source >= 0 ? in[ulong(source) * channels + c] : state[c * uint(s) + uint(s + source)];
        acc += x * w[c * k + uint(j)];
    }
    acc += in[gid] * w[c * k + uint(s)];
    out[gid] = silu(acc);
}

// The carried conv state after n tokens (runs after the conv).
kernel void clef_conv1d_state(device const float *in [[buffer(0)]], device float *state [[buffer(1)]],
                              constant uint3 &nck [[buffer(2)]], uint c [[thread_position_in_grid]]) {
    const int n = int(nck.x);
    const uint channels = nck.y, k = nck.z;
    if (c >= channels) {
        return;
    }
    const int s = int(k) - 1;
    float next[3] = {0.0f, 0.0f, 0.0f};
    for (int j = 0; j < s; ++j) {
        const int source = n - s + j;
        next[j] = source >= 0 ? in[ulong(source) * channels + c] : state[c * uint(s) + uint(s + source)];
    }
    for (int j = 0; j < s; ++j) {
        state[c * uint(s) + uint(j)] = next[j];
    }
}

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
    const float qnorm = max(sqrt(group_sum(q * q, scratch, i, threads)), 1e-6f);
    const float knorm = max(sqrt(group_sum(k * k, scratch, i, threads)), 1e-6f);
    const float qv = q / qnorm * rsqrt(float(a.dim));
    const float kv = k / knorm;
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

// The same delta rule with one simdgroup per state row (lane l holds key
// entries 4l..4l+3), inputs read straight from device memory: many more
// simdgroups to hide the per-token latency. Threadgroups of 8 simdgroups
// (8 rows); the grid covers value_heads x 128 rows.
kernel void clef_delta_scan128_rows(device const float *qn [[buffer(0)]], device const float *kn [[buffer(1)]],
                                    device const float *conv [[buffer(2)]], device const float *decay [[buffer(3)]],
                                    device const float *beta [[buffer(4)]], device const float *kq [[buffer(5)]],
                                    device float *state [[buffer(6)]], device float *out [[buffer(7)]],
                                    constant DeltaScanArgs &a [[buffer(8)]], uint tg [[threadgroup_position_in_grid]],
                                    uint tid [[thread_position_in_threadgroup]]) {
    constexpr uint kDim = 128;
    const uint lane = tid % kSimd;
    const uint global_row = tg * 8 + tid / kSimd;  // over value_heads * 128
    const uint vh = global_row / kDim;
    const uint row = global_row % kDim;
    const uint repeat = a.value_heads / a.key_heads;
    const uint kh = a.v_head_reordered != 0 ? vh % a.key_heads : vh / repeat;
    device float4 *srow = (device float4 *)(state + (ulong(vh) * kDim + row) * kDim);
    float4 s = srow[lane];
    for (uint t = 0; t < a.n; ++t) {
        const ulong kb = (ulong(t) * a.key_heads + kh) * kDim;
        const float4 k = ((device const float4 *)(kn + kb))[lane];
        const float4 q = ((device const float4 *)(qn + kb))[lane];
        const float g = decay[ulong(t) * a.value_heads + vh];
        const float b = beta[ulong(t) * a.value_heads + vh];
        const float v = conv[ulong(t) * a.conv_width + a.value_offset + vh * kDim + row];
        s *= g;
        const float4 pk = s * k;
        const float4 pq = s * q;
        const float sk = simd_sum((pk.x + pk.y) + (pk.z + pk.w));
        const float sq = simd_sum((pq.x + pq.y) + (pq.z + pq.w));
        const float delta = (v - sk) * b;
        s = fma(k, float4(delta), s);
        if (lane == 0) {
            out[(ulong(t) * a.value_heads + vh) * kDim + row] = sq + delta * kq[ulong(t) * a.key_heads + kh];
        }
    }
    srow[lane] = s;
}

// Per-head RMSNorm(o) * w * silu(z) -> f16. Threadgroup (t, head), dim threads.
kernel void clef_gated_norm_to_f16(device const float *o [[buffer(0)]], device const float *z [[buffer(1)]],
                                   device const float *w [[buffer(2)]], device half *out [[buffer(3)]],
                                   constant uint2 &hd [[buffer(4)]], constant float &eps [[buffer(5)]],
                                   uint2 tg [[threadgroup_position_in_grid]], uint2 i_v [[thread_position_in_threadgroup]],
                                   uint2 threads_v [[threads_per_threadgroup]]) {
    const uint i = i_v.x;
    const uint threads = threads_v.x;
    threadgroup float scratch[32];
    const uint heads = hd.x, dim = hd.y;
    const ulong base = (ulong(tg.x) * heads + tg.y) * dim;
    const float value = i < dim ? o[base + i] : 0.0f;
    const float scale = rsqrt(group_sum(value * value, scratch, i, threads) / float(dim) + eps);
    if (i < dim) {
        out[base + i] = half(value * scale * w[i] * silu(z[base + i]));
    }
}

// ---- full attention ----

struct AttentionPrepArgs {
    uint heads;
    uint kv_heads;
    uint dim;
    uint rot;
    uint pos0;
    float scale;
    float eps;
};

// Split interleaved [q | gate] per head, RMSNorm q and k per head, neox
// rotary on the first `rot` dims; q scaled, written f16 [n, heads, dim];
// gate f32; k and v appended to the f16 caches at pos0 + t.
kernel void clef_attention_prep(device const float *qg [[buffer(0)]], device const float *k [[buffer(1)]],
                                device const float *v [[buffer(2)]], device const float *qw [[buffer(3)]],
                                device const float *kw [[buffer(4)]], device const float *cos_sin [[buffer(5)]],
                                device half *q16 [[buffer(6)]], device float *gate [[buffer(7)]],
                                device half *kcache [[buffer(8)]], device half *vcache [[buffer(9)]],
                                constant AttentionPrepArgs &a [[buffer(10)]], uint2 tg [[threadgroup_position_in_grid]],
                                uint2 i_v [[thread_position_in_threadgroup]], uint2 threads_v [[threads_per_threadgroup]]) {
    const uint i = i_v.x;
    const uint threads = threads_v.x;
    threadgroup float scratch[32];
    threadgroup float rowbuf[1024];
    const uint t = tg.x, h = tg.y;
    const bool is_q = h < a.heads;
    const uint head = is_q ? h : h - a.heads;
    float x = 0.0f;
    if (i < a.dim) {
        x = is_q ? qg[(ulong(t) * a.heads + head) * a.dim * 2 + i] : k[(ulong(t) * a.kv_heads + head) * a.dim + i];
    }
    const float inv = rsqrt(group_sum(x * x, scratch, i, threads) / float(a.dim) + a.eps);
    if (i < a.dim) {
        rowbuf[i] = x * inv * (is_q ? qw[i] : kw[i]);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float y = i < a.dim ? rowbuf[i] : 0.0f;
    const uint half_rot = a.rot / 2;
    if (i < a.rot) {
        const uint pair = i < half_rot ? i : i - half_rot;
        const float c = cos_sin[(ulong(t) * half_rot + pair) * 2];
        const float s = cos_sin[(ulong(t) * half_rot + pair) * 2 + 1];
        const float x0 = rowbuf[pair];
        const float x1 = rowbuf[pair + half_rot];
        y = i < half_rot ? x0 * c - x1 * s : x0 * s + x1 * c;
    }
    if (i < a.dim) {
        if (is_q) {
            q16[(ulong(t) * a.heads + head) * a.dim + i] = half(y * a.scale);
            gate[(ulong(t) * a.heads + head) * a.dim + i] = qg[(ulong(t) * a.heads + head) * a.dim * 2 + a.dim + i];
        } else {
            const ulong slot = (ulong(a.pos0 + t) * a.kv_heads + head) * a.dim + i;
            kcache[slot] = half(y);
            vcache[slot] = half(v[(ulong(t) * a.kv_heads + head) * a.dim + i]);
        }
    }
}

struct FlashArgs {
    uint n;
    uint heads;
    uint kv_heads;
    uint first;
};

// Causal attention, head dim 256, fixed order: a threadgroup owns 8 queries
// of one head (one simdgroup per query); keys stream in tiles of 32 at
// absolute positions through threadgroup memory, and each query runs the
// online softmax over them in order, so its result depends only on its
// position and the cache. Lane l holds dims 8l..8l+7 of q and of the
// output. Scores accumulate in f32 over f16 inputs.
kernel void clef_flash_attention256(device const half *q16 [[buffer(0)]], device const half *kcache [[buffer(1)]],
                                    device const half *vcache [[buffer(2)]], device float *out [[buffer(3)]],
                                    constant FlashArgs &a [[buffer(4)]], uint2 tg [[threadgroup_position_in_grid]],
                                    uint2 tid_v [[thread_position_in_threadgroup]]) {
    const uint tid = tid_v.x;
    constexpr uint kDim = 256, kQueries = 8, kKeys = 32;
    threadgroup half kt[kKeys][kDim];
    threadgroup half vt[kKeys][kDim];
    const uint head = tg.y;
    const uint kvh = head / (a.heads / a.kv_heads);
    const uint q_base = tg.x * kQueries;
    const uint warp = tid / kSimd;
    const uint lane = tid % kSimd;
    const uint t = q_base + warp;  // this simdgroup's query, within the chunk
    const bool active = t < a.n;
    const uint pos = a.first + min(t, a.n - 1);
    float q[8];
    for (uint e = 0; e < 8; ++e) {
        q[e] = float(q16[(ulong(min(t, a.n - 1)) * a.heads + head) * kDim + 8 * lane + e]);
    }
    float o[8] = {0, 0, 0, 0, 0, 0, 0, 0};
    float row_max = -INFINITY, row_sum = 0.0f;
    const uint limit = a.first + a.n;
    const uint last_query = a.first + min(q_base + kQueries, a.n) - 1;
    const uint tiles = last_query / kKeys + 1;
    for (uint tile = 0; tile < tiles; ++tile) {
        const uint j0 = tile * kKeys;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint c = tid; c < kKeys * kDim; c += kQueries * kSimd) {
            const uint j = c / kDim, e = c % kDim;
            const uint key = j0 + j;
            const ulong slot = (ulong(key < limit ? key : 0) * a.kv_heads + kvh) * kDim + e;
            kt[j][e] = key < limit ? kcache[slot] : half(0);
            vt[j][e] = key < limit ? vcache[slot] : half(0);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        float s[kKeys];
        float tile_max = -INFINITY;
        for (uint j = 0; j < kKeys; ++j) {
            float partial = 0.0f;
            for (uint e = 0; e < 8; ++e) {
                partial = fma(q[e], float(kt[j][8 * lane + e]), partial);
            }
            const float score = simd_sum(partial);
            s[j] = j0 + j > pos ? -INFINITY : score;
            tile_max = max(tile_max, s[j]);
        }
        const float new_max = max(row_max, tile_max);
        const float alpha = new_max == row_max ? 1.0f : exp(row_max - new_max);
        row_max = new_max;
        float tile_sum = 0.0f;
        for (uint e = 0; e < 8; ++e) {
            o[e] *= alpha;
        }
        for (uint j = 0; j < kKeys; ++j) {
            const float p = s[j] == -INFINITY ? 0.0f : float(half(exp(s[j] - new_max)));
            tile_sum += p;
            for (uint e = 0; e < 8; ++e) {
                o[e] = fma(p, float(vt[j][8 * lane + e]), o[e]);
            }
        }
        row_sum = row_sum * alpha + tile_sum;
    }
    if (active) {
        const float inv = 1.0f / row_sum;
        device float *dst = out + (ulong(t) * a.heads + head) * kDim + 8 * lane;
        for (uint e = 0; e < 8; ++e) {
            dst[e] = o[e] * inv;
        }
    }
}

// Causal attention, head dim 256, on simdgroup matrices, fixed order. A
// threadgroup owns 8 queries and one KV head; each of its 4 simdgroups takes
// one of the 4 query heads sharing it. Keys stream in tiles of 16 at
// absolute positions through threadgroup memory shared by the 4 heads; each
// query row runs the online softmax over the tiles in order, so its result
// depends only on its position and the cache. Rows past n are computed and
// written into the padding of `out` (callers size `q16` and `out` to a
// multiple of 8 rows).
template <uint QB>
inline void flash_attention256_sg_impl(device const half *q16, device const half *kcache, device const half *vcache,
                                       device float *out, constant FlashArgs &a, uint2 tg, uint tid,
                                       threadgroup half *kt, threadgroup half *vt, threadgroup float (*sbuf)[128],
                                       threadgroup half (*pbuf)[128], threadgroup float (*dbuf)[64]) {
    constexpr uint kDim = 256, kQueries = 8, kKeys = 16, kGroup = 4, kLds = kDim + 8;
    // simdgroup w: head (w % kGroup) of the KV group, query block (w / kGroup)
    const uint warp = tid / kSimd;
    const uint lane = tid % kSimd;
    const uint kvh = tg.y;
    const uint head = kvh * kGroup + warp % kGroup;
    const uint block_t0 = tg.x * kQueries * QB;
    const uint t0 = block_t0 + (warp / kGroup) * kQueries;
    const uint row_stride = a.heads * kDim;
    // queries: 8 x 256 as 32 simdgroup tiles
    simdgroup_half8x8 qm[kDim / 8];
    device const half *qbase = q16 + ulong(t0) * row_stride + head * kDim;
    #pragma unroll
    for (uint d = 0; d < kDim / 8; ++d) {
        simdgroup_load(qm[d], qbase + d * 8, row_stride);
    }
    simdgroup_float8x8 om[kDim / 8];
    #pragma unroll
    for (uint d = 0; d < kDim / 8; ++d) {
        om[d] = make_filled_simdgroup_matrix<float, 8, 8>(0.0f);
    }
    // softmax ownership: lane -> row lane / 4, keys 4 (lane % 4) .. + 3
    const uint my_row = lane / 4;
    const uint my_col = (lane % 4) * 4;
    const uint pos = a.first + t0 + my_row;
    float row_max = -INFINITY, row_sum = 0.0f;
    const uint limit = a.first + a.n;
    const uint last_query = a.first + min(block_t0 + kQueries * QB, a.n) - 1;
    const uint tiles = last_query / kKeys + 1;
    for (uint tile = 0; tile < tiles; ++tile) {
        const uint j0 = tile * kKeys;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint c = tid; c < kKeys * kDim / 4; c += kGroup * QB * kSimd) {
            const uint j = (c * 4) / kDim, e = (c * 4) % kDim;
            const uint key = j0 + j;
            const ulong slot = (ulong(key < limit ? key : 0) * a.kv_heads + kvh) * kDim + e;
            const half4 kv = key < limit ? *(device const half4 *)(kcache + slot) : half4(0);
            const half4 vv = key < limit ? *(device const half4 *)(vcache + slot) : half4(0);
            *(threadgroup half4 *)(kt + j * kLds + e) = kv;
            *(threadgroup half4 *)(vt + j * kLds + e) = vv;
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        // S = Q K^T (8 x 16), f32 accumulation
        simdgroup_float8x8 s0 = make_filled_simdgroup_matrix<float, 8, 8>(0.0f);
        simdgroup_float8x8 s1 = make_filled_simdgroup_matrix<float, 8, 8>(0.0f);
        #pragma unroll
        for (uint d = 0; d < kDim / 8; ++d) {
            simdgroup_half8x8 k0, k1;
            simdgroup_load(k0, kt + d * 8, kLds, ulong2(0, 0), true);
            simdgroup_load(k1, kt + 8 * kLds + d * 8, kLds, ulong2(0, 0), true);
            simdgroup_multiply_accumulate(s0, qm[d], k0, s0);
            simdgroup_multiply_accumulate(s1, qm[d], k1, s1);
        }
        simdgroup_store(s0, sbuf[warp], kKeys);
        simdgroup_store(s1, sbuf[warp] + 8, kKeys);
        simdgroup_barrier(mem_flags::mem_threadgroup);
        // mask and online softmax for this lane's row and 4 keys
        float sv[4];
        float tile_max = -INFINITY;
        #pragma unroll
        for (uint c = 0; c < 4; ++c) {
            const uint key = j0 + my_col + c;
            const float v = sbuf[warp][my_row * kKeys + my_col + c];
            sv[c] = key > pos ? -INFINITY : v;
            tile_max = max(tile_max, sv[c]);
        }
        tile_max = max(tile_max, simd_shuffle_xor(tile_max, ushort(1)));
        tile_max = max(tile_max, simd_shuffle_xor(tile_max, ushort(2)));
        const float new_max = max(row_max, tile_max);
        const float alpha = new_max == row_max ? 1.0f : exp(row_max - new_max);
        row_max = new_max;
        float tile_sum = 0.0f;
        #pragma unroll
        for (uint c = 0; c < 4; ++c) {
            const half p = sv[c] == -INFINITY ? half(0) : half(exp(sv[c] - new_max));
            pbuf[warp][my_row * kKeys + my_col + c] = p;
            tile_sum += float(p);
        }
        tile_sum += simd_shuffle_xor(tile_sum, ushort(1));
        tile_sum += simd_shuffle_xor(tile_sum, ushort(2));
        row_sum = row_sum * alpha + tile_sum;
        // O = diag(alpha) O when any row's max moved
        if (simd_any(alpha != 1.0f)) {
            for (uint i = lane; i < 64; i += kSimd) {
                dbuf[warp][i] = 0.0f;
            }
            simdgroup_barrier(mem_flags::mem_threadgroup);
            if (lane % 4 == 0) {
                dbuf[warp][my_row * 9] = alpha;
            }
            simdgroup_barrier(mem_flags::mem_threadgroup);
            simdgroup_float8x8 dm;
            simdgroup_load(dm, dbuf[warp], 8);
            #pragma unroll
            for (uint d = 0; d < kDim / 8; ++d) {
                simdgroup_multiply(om[d], dm, om[d]);
            }
        }
        simdgroup_barrier(mem_flags::mem_threadgroup);
        // O += P V
        simdgroup_half8x8 p0, p1;
        simdgroup_load(p0, pbuf[warp], kKeys);
        simdgroup_load(p1, pbuf[warp] + 8, kKeys);
        #pragma unroll
        for (uint d = 0; d < kDim / 8; ++d) {
            simdgroup_half8x8 v0, v1;
            simdgroup_load(v0, vt + d * 8, kLds);
            simdgroup_load(v1, vt + 8 * kLds + d * 8, kLds);
            simdgroup_multiply_accumulate(om[d], p0, v0, om[d]);
            simdgroup_multiply_accumulate(om[d], p1, v1, om[d]);
        }
    }
    // out = diag(1 / l) O
    for (uint i = lane; i < 64; i += kSimd) {
        dbuf[warp][i] = 0.0f;
    }
    simdgroup_barrier(mem_flags::mem_threadgroup);
    if (lane % 4 == 0) {
        dbuf[warp][my_row * 9] = 1.0f / row_sum;
    }
    simdgroup_barrier(mem_flags::mem_threadgroup);
    simdgroup_float8x8 dm;
    simdgroup_load(dm, dbuf[warp], 8);
    device float *obase = out + ulong(t0) * row_stride + head * kDim;
    #pragma unroll
    for (uint d = 0; d < kDim / 8; ++d) {
        simdgroup_float8x8 r;
        simdgroup_multiply(r, dm, om[d]);
        simdgroup_store(r, obase + d * 8, row_stride);
    }
}

kernel void clef_flash_attention256_sg(device const half *q16 [[buffer(0)]], device const half *kcache [[buffer(1)]],
                                       device const half *vcache [[buffer(2)]], device float *out [[buffer(3)]],
                                       constant FlashArgs &a [[buffer(4)]], uint2 tg [[threadgroup_position_in_grid]],
                                       uint2 tid_v [[thread_position_in_threadgroup]]) {
    threadgroup half kt[16 * 264];
    threadgroup half vt[16 * 264];
    threadgroup float sbuf[4][128];
    threadgroup half pbuf[4][128];
    threadgroup float dbuf[4][64];
    flash_attention256_sg_impl<1>(q16, kcache, vcache, out, a, tg, tid_v.x, kt, vt, sbuf, pbuf, dbuf);
}

kernel void clef_flash_attention256_sg2(device const half *q16 [[buffer(0)]], device const half *kcache [[buffer(1)]],
                                        device const half *vcache [[buffer(2)]], device float *out [[buffer(3)]],
                                        constant FlashArgs &a [[buffer(4)]], uint2 tg [[threadgroup_position_in_grid]],
                                        uint2 tid_v [[thread_position_in_threadgroup]]) {
    threadgroup half kt[16 * 264];
    threadgroup half vt[16 * 264];
    threadgroup float sbuf[8][128];
    threadgroup half pbuf[8][128];
    threadgroup float dbuf[8][64];
    flash_attention256_sg_impl<2>(q16, kcache, vcache, out, a, tg, tid_v.x, kt, vt, sbuf, pbuf, dbuf);
}

// ---- attention on the tensor op ----
//
// Per query head h: scores[i, j] = q[i, h, :] . k[j, kvh, :] (f32), a
// causal softmax to f16 probabilities, then out[i, h, :] = sum_j p[i, j]
// v[j, kvh, :]. Both products run on matmul2d over strided views of the
// caches; a key past a query's diagonal has probability exactly 0.

struct AttnArgs {
    uint n;       // queries
    uint keys;    // first + n
    uint heads;
    uint kv_heads;
    uint head;    // the query head this dispatch computes
};

// scores [n, keys] for one head; 64 x 64 tiles.
kernel void clef_attn_scores(device const half *q16 [[buffer(0)]], device const half *kcache [[buffer(1)]],
                             device float *scores [[buffer(2)]], constant AttnArgs &a [[buffer(3)]],
                             uint2 tgid [[threadgroup_position_in_grid]]) {
    const int dim = 256;
    const int kvh = int(a.head / (a.heads / a.kv_heads));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tq(
        (device half *)(q16 + a.head * dim), dextents<int32_t, 2>(dim, int(a.n)),
        array<int32_t, 2>{1, int(a.heads) * dim});
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tk(
        (device half *)(kcache + kvh * dim), dextents<int32_t, 2>(dim, int(a.keys)),
        array<int32_t, 2>{1, int(a.kv_heads) * dim});
    tensor<device float, dextents<int32_t, 2>, tensor_inline> ts(scores, dextents<int32_t, 2>(int(a.keys), int(a.n)));
    constexpr auto desc = matmul2d_descriptor(64, 64, static_cast<int>(dynamic_extent), false, true, false);
    matmul2d<desc, execution_simdgroups<4>> op;
    auto sq = tq.slice(0, int(tgid.y) * 64);
    auto sk = tk.slice(0, int(tgid.x) * 64);
    auto so = ts.slice(int(tgid.x) * 64, int(tgid.y) * 64);
    op.run(sq, sk, so);
}

// Causal softmax over score rows [n, keys] to f16 probabilities; row i sees
// keys <= first + i. One threadgroup per row.
kernel void clef_causal_softmax_to_f16(device const float *scores [[buffer(0)]], device half *probs [[buffer(1)]],
                                       constant uint3 &nkf [[buffer(2)]], uint row [[threadgroup_position_in_grid]],
                                       uint tid [[thread_position_in_threadgroup]],
                                       uint threads [[threads_per_threadgroup]]) {
    threadgroup float scratch[32];
    const uint keys = nkf.y, first = nkf.z;
    const uint visible = first + row + 1;
    device const float *r = scores + ulong(row) * keys;
    device half *p = probs + ulong(row) * keys;
    float best = -INFINITY;
    for (uint j = tid; j < visible; j += threads) {
        best = max(best, r[j]);
    }
    best = group_max(best, scratch, tid, threads);
    float sum = 0.0f;
    for (uint j = tid; j < visible; j += threads) {
        sum += exp(r[j] - best);
    }
    const float inv = 1.0f / group_sum(sum, scratch, tid, threads);
    for (uint j = tid; j < keys; j += threads) {
        p[j] = half(j < visible ? exp(r[j] - best) * inv : 0.0f);
    }
}

// out[i, head, :] = probs[i, :] . v[:, kvh, :]; 64 x 64 tiles over (dim, n).
kernel void clef_attn_pv(device const half *probs [[buffer(0)]], device const half *vcache [[buffer(1)]],
                         device float *out [[buffer(2)]], constant AttnArgs &a [[buffer(3)]],
                         uint2 tgid [[threadgroup_position_in_grid]]) {
    const int dim = 256;
    const int kvh = int(a.head / (a.heads / a.kv_heads));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tp((device half *)probs,
                                                               dextents<int32_t, 2>(int(a.keys), int(a.n)));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tv(
        (device half *)(vcache + kvh * dim), dextents<int32_t, 2>(dim, int(a.keys)),
        array<int32_t, 2>{1, int(a.kv_heads) * dim});
    tensor<device float, dextents<int32_t, 2>, tensor_inline> to(
        out + a.head * dim, dextents<int32_t, 2>(dim, int(a.n)), array<int32_t, 2>{1, int(a.heads) * dim});
    constexpr auto desc = matmul2d_descriptor(64, 64, static_cast<int>(dynamic_extent), false, false, false);
    matmul2d<desc, execution_simdgroups<4>> op;
    auto sp = tp.slice(0, int(tgid.y) * 64);
    auto sv = tv.slice(int(tgid.x) * 64, 0);
    auto so = to.slice(int(tgid.x) * 64, int(tgid.y) * 64);
    op.run(sp, sv, so);
}

// Softmax over full rows [rows, keys] in place (f32), after scaling.
kernel void clef_softmax_rows_f32(device float *scores [[buffer(0)]], constant uint &keys [[buffer(1)]],
                                  constant float &scale [[buffer(2)]], uint row [[threadgroup_position_in_grid]],
                                  uint tid [[thread_position_in_threadgroup]],
                                  uint threads [[threads_per_threadgroup]]) {
    threadgroup float scratch[32];
    device float *r = scores + ulong(row) * keys;
    float best = -INFINITY;
    for (uint j = tid; j < keys; j += threads) {
        best = max(best, r[j] * scale);
    }
    best = group_max(best, scratch, tid, threads);
    float sum = 0.0f;
    for (uint j = tid; j < keys; j += threads) {
        sum += exp(r[j] * scale - best);
    }
    const float inv = 1.0f / group_sum(sum, scratch, tid, threads);
    for (uint j = tid; j < keys; j += threads) {
        r[j] = exp(r[j] * scale - best) * inv;
    }
}

// out16 = o * sigmoid(gate).
kernel void clef_sigmoid_gate_to_f16(device const float *o [[buffer(0)]], device const float *gate [[buffer(1)]],
                                     device half *out [[buffer(2)]], constant ulong &count [[buffer(3)]],
                                     uint gid [[thread_position_in_grid]]) {
    if (gid < count) {
        out[gid] = half(o[gid] * sigmoid(gate[gid]));
    }
}

// out16 = silu(gate) * up.
kernel void clef_silu_mul_to_f16(device const float *gate [[buffer(0)]], device const float *up [[buffer(1)]],
                                 device half *out [[buffer(2)]], constant ulong &count [[buffer(3)]],
                                 uint gid [[thread_position_in_grid]]) {
    if (gid < count) {
        out[gid] = half(silu(gate[gid]) * up[gid]);
    }
}

// Span sums of rows in token order: spans [count, 2] (absolute [start, end)),
// rows cover absolute positions [first, first + n).
kernel void clef_span_sums(device const float *rows [[buffer(0)]], device const int *spans [[buffer(1)]],
                           device float *sums [[buffer(2)]], constant uint3 &dfn [[buffer(3)]],
                           uint span [[threadgroup_position_in_grid]], uint tid [[thread_position_in_threadgroup]],
                           uint threads [[threads_per_threadgroup]]) {
    const uint d = dfn.x, first = dfn.y, n = dfn.z;
    const int start = max(spans[2 * span], int(first));
    const int end = min(spans[2 * span + 1], int(first + n));
    for (uint c = tid; c < d; c += threads) {
        float acc = sums[ulong(span) * d + c];
        for (int t = start; t < end; ++t) {
            acc += rows[ulong(t - int(first)) * d + c];
        }
        sums[ulong(span) * d + c] = acc;
    }
}

// out[n, m] = x[n, k] . w[m, k]^T (f32), each output summed in k order by
// one thread, so a row's result does not depend on the other rows. 16 x 16
// threads, 4 x 4 outputs each, k tiles of 16.
kernel void clef_linear_f32_ordered(device const float *x [[buffer(0)]], device const float *w [[buffer(1)]],
                                    device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                                    uint2 tg [[threadgroup_position_in_grid]],
                                    uint2 tid_v [[thread_position_in_threadgroup]]) {
    const uint tid = tid_v.x;
    threadgroup float xs[16][65];
    threadgroup float ws[16][65];
    const uint n = nmk.x, m = nmk.y, k = nmk.z;
    const uint tx = tid % 16, ty = tid / 16;
    const uint row0 = tg.y * 64, col0 = tg.x * 64;
    float acc[4][4];
    for (uint i = 0; i < 4; ++i) {
        for (uint j = 0; j < 4; ++j) {
            acc[i][j] = 0.0f;
        }
    }
    for (uint k0 = 0; k0 < k; k0 += 16) {
        for (uint i = tid; i < 64 * 16; i += 256) {
            const uint r = i / 16, kk = i % 16;
            const uint row = row0 + r, col = col0 + r, kidx = k0 + kk;
            xs[kk][r] = row < n && kidx < k ? x[ulong(row) * k + kidx] : 0.0f;
            ws[kk][r] = col < m && kidx < k ? w[ulong(col) * k + kidx] : 0.0f;
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint kk = 0; kk < 16; ++kk) {
            for (uint i = 0; i < 4; ++i) {
                for (uint j = 0; j < 4; ++j) {
                    acc[i][j] = fma(xs[kk][ty * 4 + i], ws[kk][tx * 4 + j], acc[i][j]);
                }
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    for (uint i = 0; i < 4; ++i) {
        const uint row = row0 + ty * 4 + i;
        if (row >= n) {
            continue;
        }
        for (uint j = 0; j < 4; ++j) {
            const uint col = col0 + tx * 4 + j;
            if (col < m) {
                out[ulong(row) * m + col] = acc[i][j];
            }
        }
    }
}

// The joint head's memory attention, query side:
// side[i, h, :] = sum_{d in head h} q[i, d] W_k[d, :]. Threadgroup per (i, h).
kernel void clef_head_side(device const float *q [[buffer(0)]], device const float *wk [[buffer(1)]],
                           device float *side [[buffer(2)]], constant uint2 &hw [[buffer(3)]],
                           uint g [[threadgroup_position_in_grid]], uint tid [[thread_position_in_threadgroup]],
                           uint threads [[threads_per_threadgroup]]) {
    const uint heads = hw.x, width = hw.y, head_dim = width / heads;
    const uint i = g / heads, h = g % heads;
    device const float *query = q + ulong(i) * width + h * head_dim;
    device float *dst = side + ulong(g) * width;
    for (uint c = tid; c < width; c += threads) {
        float acc = 0.0f;
        for (uint d = 0; d < head_dim; ++d) {
            acc = fma(query[d], wk[ulong(h * head_dim + d) * width + c], acc);
        }
        dst[c] = acc;
    }
}

// context[i, e] = W_v[e, :] . mixed[i, head(e), :] + b_v[e]; one simdgroup
// per (i, e).
kernel void clef_head_context(device const float *mixed [[buffer(0)]], device const float *wv [[buffer(1)]],
                              device const float *bv [[buffer(2)]], device float *out [[buffer(3)]],
                              constant uint3 &nhw [[buffer(4)]], uint gid [[thread_position_in_grid]]) {
    const uint n = nhw.x, heads = nhw.y, width = nhw.z, head_dim = width / heads;
    const uint simd_id = gid / kSimd;
    const uint lane = gid % kSimd;
    if (simd_id >= n * width) {
        return;
    }
    const uint i = simd_id / width, e = simd_id % width, h = e / head_dim;
    device const float *z = mixed + (ulong(i) * heads + h) * width;
    device const float *row = wv + ulong(e) * width;
    float acc = 0.0f;
    for (uint c = lane; c < width; c += kSimd) {
        acc = fma(row[c], z[c], acc);
    }
    acc = simd_sum(acc);
    if (lane == 0) {
        out[ulong(i) * width + e] = acc + bv[e];
    }
}
