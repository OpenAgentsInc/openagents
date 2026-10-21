// Flint (RecursiveSI/flint, Apache-2.0) on Metal: MLX affine INT4 weights.
//
// Portions of this file are adapted from Recursive's `rec` engine
// (https://github.com/recursive-oss/rec, release v0.1.0, commit e53663b),
// Copyright Recursive Superintelligence, Inc., licensed under the Apache
// License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0). The
// adapted parts are the INT4 group matrix-vector product with prescaled
// activations, the 256-thread RMSNorm, the split-K attention with a learned
// sink and its merge, sigmoid top-6 routing, and the BF16 rounding points of
// the expert combine. They were rewritten as standalone kernels for Psionic;
// rec runs them inside one persistent MLX kernel. Changes are Psionic's.

#include <metal_stdlib>
#if defined(__HAVE_TENSOR__)
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
#endif
using namespace metal;

typedef bfloat bf16;

constant float FLINT_EPS = 1e-6f;
constant float FLINT_LOGBASE = 13.287712379549449f; // log2(10000)

// ---------------------------------------------------------------------------
// Shared helpers.

// Sixteen activations, prescaled so that masked nibbles need no shifts.
template <typename P>
inline float load16(P x, thread float* v) {
    float sum = 0.0f;
    for (uint i = 0; i < 16; i += 4) {
        sum += x[i] + x[i + 1] + x[i + 2] + x[i + 3];
        v[i] = x[i];
        v[i + 1] = x[i + 1] / 16.0f;
        v[i + 2] = x[i + 2] / 256.0f;
        v[i + 3] = x[i + 3] / 4096.0f;
    }
    return sum;
}

// One group-64 affine INT4 slice of sixteen weights: s * sum(q * x) + b * sum(x).
inline float dot16(const device uchar* w, thread float* x, float s, float b, float sum) {
    const device ushort* p = (const device ushort*)w;
    float a = 0.0f;
    for (uint j = 0; j < 4; ++j)
        a += x[4 * j] * (p[j] & 15u) + x[4 * j + 1] * (p[j] & 240u)
           + x[4 * j + 2] * (p[j] & 3840u) + x[4 * j + 3] * (p[j] & 61440u);
    return s * a + sum * b;
}

struct Q4 {
    const device uchar* w;
    const device bf16* s;
    const device bf16* b;
};

// One SIMD group computes four consecutive rows of an INT4 matrix-vector product.
template <uint K, typename P>
inline void q4_rows4(Q4 m, P x, uint first, uint lane, thread float* out) {
    float a[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    for (uint block = 0; block < K; block += 512) {
        uint scalar = block + lane * 16;
        float v[16];
        float sum = load16(x + scalar, v);
        for (uint r = 0; r < 4; ++r) {
            uint row = first + r;
            uint gi = row * (K / 64) + scalar / 64;
            a[r] += dot16(m.w + size_t(row) * (K / 2) + scalar / 2, v, float(m.s[gi]), float(m.b[gi]), sum);
        }
    }
    for (uint r = 0; r < 4; ++r) out[r] = simd_sum(a[r]);
}

// RMSNorm of the 2048-wide residual into threadgroup memory, 256 threads.
inline void rms2048(const device bf16* h, const device bf16* w, threadgroup bf16* out,
                    threadgroup float* temp, uint tid, uint lane, uint sg) {
    bf16 x[8];
    for (uint j = 0; j < 8; ++j) x[j] = h[(tid + (j / 4) * 256) * 4 + j % 4];
    float sums[2];
    for (uint set = 0; set < 2; ++set) {
        float squared = 0.0f;
        for (uint j = 0; j < 4; ++j) { float v = float(x[set * 4 + j]); squared += v * v; }
        sums[set] = simd_sum(squared);
    }
    if (lane == 0) { temp[sg] = sums[0]; temp[sg + 8] = sums[1]; }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float leaf = lane < 16 ? temp[lane] : 0.0f;
    float total = simd_sum(leaf);
    float inverse = metal::precise::rsqrt(total / 2048.0f + FLINT_EPS);
    for (uint j = 0; j < 8; ++j) {
        uint i = (tid + (j / 4) * 256) * 4 + j % 4;
        out[i] = w[i] * bf16(float(x[j]) * inverse);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
}

// Per-head RMSNorm with optional NeoX RoPE, one SIMD group per 128-wide head.
inline void head_norm(device bf16* p, const device bf16* w, bool rotary, uint position, uint lane) {
    float squared = 0.0f;
    for (uint j = 0; j < 4; ++j) { float v = p[lane * 4 + j]; squared += v * v; }
    float inverse = metal::precise::rsqrt(simd_sum(squared) / 128.0f + FLINT_EPS);
    if (rotary) {
        bf16 a[2], b[2];
        for (uint j = 0; j < 2; ++j) {
            uint pair = lane * 2 + j;
            a[j] = w[pair] * bf16(float(p[pair]) * inverse);
            b[j] = w[pair + 64] * bf16(float(p[pair + 64]) * inverse);
        }
        simdgroup_barrier(mem_flags::mem_device);
        for (uint j = 0; j < 2; ++j) {
            uint pair = lane * 2 + j;
            float theta = float(position) * metal::exp2(-float(pair) / 64.0f * FLINT_LOGBASE);
            float c = metal::fast::cos(theta), s = metal::fast::sin(theta);
            p[pair] = bf16(float(a[j]) * c - float(b[j]) * s);
            p[pair + 64] = bf16(float(a[j]) * s + float(b[j]) * c);
        }
    } else {
        bf16 out[4];
        for (uint j = 0; j < 4; ++j) { uint d = lane * 4 + j; out[j] = w[d] * bf16(float(p[d]) * inverse); }
        simdgroup_barrier(mem_flags::mem_device);
        for (uint j = 0; j < 4; ++j) p[lane * 4 + j] = out[j];
    }
}

inline float sigmoid_precise(float a) {
    float tail = 1.0f / (1.0f + metal::precise::exp(metal::abs(a)));
    return a < 0 ? tail : 1.0f - tail;
}

// SiLU(g) * u with rec's BF16 rounding. `fast` selects rec's routed-expert exp.
inline bf16 swiglu(bf16 g, bf16 u, bool fast) {
    float e = fast ? metal::fast::exp(metal::abs(float(g))) : metal::exp(metal::abs(float(g)));
    float tail = 1.0f / (1.0f + e);
    bf16 sig = bf16(float(g) < 0 ? tail : 1.0f - tail);
    return bf16(float(bf16(float(g) * float(sig))) * float(u));
}

// Sigmoid top-6 with lower-index ties; writes ids and normalized weights.
inline void route_top6(const device float* probs, threadgroup uint* ids, threadgroup float* weights, uint lane) {
    float p[8], b[8];
    for (uint j = 0; j < 8; ++j) { p[j] = probs[lane + 32 * j]; b[j] = p[j]; }
    float chosen = 0.0f;
    for (uint rank = 0; rank < 6; ++rank) {
        float best = -3.402823466e+38f; uint idx = 0xffffffffu;
        for (uint j = 0; j < 8; ++j) { uint i = lane + 32 * j; if (b[j] > best || (b[j] == best && i < idx)) { best = b[j]; idx = i; } }
        float peak = simd_max(best);
        uint selected = simd_min(best == peak ? idx : 0xffffffffu);
        float value = simd_shuffle(p[selected / 32], ushort(selected % 32));
        if (lane == rank) chosen = value;
        if (lane == 0) ids[rank] = selected;
        for (uint j = 0; j < 8; ++j) if (lane + 32 * j == selected) b[j] = -3.402823466e+38f;
    }
    float sum = 0.0f;
    for (uint j = 0; j < 6; ++j) sum = simd_shuffle(chosen, ushort(j)) + sum;
    sum += 1.0e-20f;
    if (lane < 6) weights[lane] = chosen / sum;
}

// ---------------------------------------------------------------------------
// Decode: one token through one layer per few dispatches.

struct DecodeParams {
    uint position;
    uint cap;      // KV capacity of this layer (513 for sliding layers)
    uint rotary;   // 1 on sliding layers
    uint global;   // 1 on full-attention layers
};

// INT8 embedding row into the BF16 residual.
kernel void flint_embed8(const device uchar* w [[buffer(0)]],
                         const device bf16* s [[buffer(1)]],
                         const device bf16* b [[buffer(2)]],
                         device bf16* h [[buffer(3)]],
                         const device uint* tokens [[buffer(4)]],
                         constant uint& count [[buffer(5)]],
                         uint2 gid [[thread_position_in_grid]]) {
    uint i = gid.x, t = gid.y;
    if (i >= 2048 || t >= count) return;
    size_t row = tokens[t];
    uint code = w[row * 2048 + i];
    size_t gi = row * 32 + i / 64;
    h[size_t(t) * 2048 + i] = bf16(float(bf16(float(s[gi]) * float(bf16(float(code))))) + float(b[gi]));
}

// Input norm, then one Q, K or V head per threadgroup (40 groups): projection,
// head norm, RoPE and KV publication. Buffers: q (4096), k and v land in the cache.
kernel void flint_qkv_head(const device bf16* h [[buffer(0)]],
                           const device bf16* norm [[buffer(1)]],
                           const device uchar* qw [[buffer(2)]], const device bf16* qs [[buffer(3)]], const device bf16* qb [[buffer(4)]],
                           const device uchar* kw [[buffer(5)]], const device bf16* ks [[buffer(6)]], const device bf16* kb [[buffer(7)]],
                           const device uchar* vw [[buffer(8)]], const device bf16* vs [[buffer(9)]], const device bf16* vb [[buffer(10)]],
                           const device bf16* q_norm [[buffer(11)]], const device bf16* k_norm [[buffer(12)]],
                           device bf16* q [[buffer(13)]], device bf16* kv [[buffer(14)]],
                           device bf16* kv_new [[buffer(15)]],
                           constant DecodeParams& P [[buffer(16)]],
                           uint group [[threadgroup_position_in_grid]],
                           uint tid [[thread_index_in_threadgroup]],
                           uint lane [[thread_index_in_simdgroup]],
                           uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup bf16 n[2048];
    threadgroup float temp[32];
    rms2048(h, norm, n, temp, tid, lane, sg);
    uint head = group;
    Q4 m;
    uint local;
    device bf16* out;
    if (head < 32) { m = {qw, qs, qb}; local = head; out = q + local * 128; }
    else if (head < 36) { m = {kw, ks, kb}; local = head - 32; out = kv_new + local * 128; }
    else { m = {vw, vs, vb}; local = head - 36; out = kv_new + 512 + local * 128; }
    m.w += size_t(local) * 128 * 1024; m.s += local * 128 * 32; m.b += local * 128 * 32;
    for (uint first = sg * 4; first < 128; first += 32) {
        float r[4];
        q4_rows4<2048>(m, (const threadgroup bf16*)n, first, lane, r);
        if (lane == 0) for (uint j = 0; j < 4; ++j) out[first + j] = bf16(r[j]);
    }
    threadgroup_barrier(mem_flags::mem_device);
    if (sg == 0 && head < 36)
        head_norm(out, head < 32 ? q_norm : k_norm, P.rotary != 0, P.position, lane);
    threadgroup_barrier(mem_flags::mem_device);
    if (head >= 32 && tid < 128) {
        uint slot = P.global ? P.position : P.position % P.cap;
        uint which = head < 36 ? 0 : 1;
        kv[((size_t(which) * 4 + local) * P.cap + slot) * 128 + tid] = out[tid];
    }
}

struct AttentionParams {
    uint count;   // keys visible to the query
    uint cap;     // stride between KV heads
    uint parts;   // split-K partitions
    uint rounded; // BF16 partials (long full-attention context)
    uint heads;   // query heads per SIMD group: 4, or 8 past 1024 keys
};

// Split-K attention with a learned sink logit, after rec's attention_partial.
template <uint HEADS, bool ROUNDED>
inline void attention_partial_impl(const device bf16* q, const device bf16* k, const device bf16* v,
                                   const device bf16* sinks, device float* partial, device float* stats,
                                   uint count, uint cap, uint parts, uint task, uint lane) {
    if (task >= (32 / HEADS) * parts) return;
    uint kv = task / ((8 / HEADS) * parts), part = (task / (8 / HEADS)) % parts;
    uint first = kv * 8 + (task % (8 / HEADS)) * HEADS;
    const device bf16* key_base = k + size_t(kv) * cap * 128 + lane * 4;
    const device bf16* value_base = v + size_t(kv) * cap * 128 + lane * 4;
    const device bf16* query_base = q + first * 128 + lane * 4;
    float query[HEADS][4], acc[HEADS][4];
    float hi = part == 0 && lane < HEADS ? float(sinks[first + lane]) : -3.402823466e+38f;
    float z = part == 0 && lane < HEADS ? 1.0f : 0.0f;
    for (uint h = 0; h < HEADS; ++h) for (uint j = 0; j < 4; ++j) {
        query[h][j] = float(query_base[h * 128 + j]) * 0.08838834764831845f; acc[h][j] = 0.0f;
    }
    uint token = part;
    if (ROUNDED && part != 0 && token < count) {
        uint at = token * 128;
        float key[4], value[4];
        for (uint j = 0; j < 4; ++j) { key[j] = float(key_base[at + j]); value[j] = float(value_base[at + j]); }
        for (uint h = 0; h < HEADS; ++h) {
            float score = 0.0f; for (uint j = 0; j < 4; ++j) score += query[h][j] * key[j];
            score = simd_sum(score); if (lane == h) { hi = score; z = 1.0f; }
            for (uint j = 0; j < 4; ++j) acc[h][j] = 0.0f + value[j];
        }
        token += parts;
    }
    for (; token < count; token += parts) {
        uint at = token * 128;
        float key[4]; bf16 value[4];
        for (uint j = 0; j < 4; ++j) { key[j] = float(key_base[at + j]); value[j] = value_base[at + j]; }
        float scores[HEADS];
        for (uint h = 0; h < HEADS; ++h) {
            float score = 0.0f; for (uint j = 0; j < 4; ++j) score += query[h][j] * key[j];
            scores[h] = simd_sum(score);
        }
        float old = 0.0f, add = 0.0f;
        if (lane < HEADS) {
            float score = scores[lane], next = max(hi, score), low = metal::fast::exp(-metal::abs(score - hi));
            bool wins = score > hi; old = wins ? low : 1.0f; add = wins ? 1.0f : low;
            z = z * old + add; hi = next;
        }
        for (uint h = 0; h < HEADS; ++h) {
            float factor = simd_shuffle(old, ushort(h)), weight = simd_shuffle(add, ushort(h));
            for (uint j = 0; j < 4; ++j) acc[h][j] = acc[h][j] * factor + weight * float(value[j]);
        }
    }
    if (lane < HEADS) {
        device float* stat_out = stats + (first * parts + part) * 2 + lane * parts * 2;
        stat_out[0] = hi; stat_out[1] = z;
    }
    for (uint h = 0; h < HEADS; ++h) {
        if (ROUNDED) {
            device bf16* out = ((device bf16*)partial) + (((first + h) * 32 + lane) * parts + part) * 4;
            for (uint j = 0; j < 4; ++j) out[j] = bf16(acc[h][j]);
        } else {
            device float* out = partial + ((first + h) * parts + part) * 128 + lane * 4;
            for (uint j = 0; j < 4; ++j) out[j] = acc[h][j];
        }
    }
}

kernel void flint_attention_partial(const device bf16* q [[buffer(0)]],
                                    const device bf16* kv [[buffer(1)]],
                                    const device bf16* sinks [[buffer(2)]],
                                    device float* partial [[buffer(3)]],
                                    device float* stats [[buffer(4)]],
                                    constant AttentionParams& P [[buffer(5)]],
                                    uint group [[threadgroup_position_in_grid]],
                                    uint lane [[thread_index_in_simdgroup]],
                                    uint sg [[simdgroup_index_in_threadgroup]]) {
    uint task = group * 8 + sg;
    const device bf16* k = kv;
    const device bf16* v = kv + size_t(P.cap) * 512;
    if (P.heads == 8) {
        if (P.rounded) attention_partial_impl<8, true>(q, k, v, sinks, partial, stats, P.count, P.cap, P.parts, task, lane);
        else attention_partial_impl<8, false>(q, k, v, sinks, partial, stats, P.count, P.cap, P.parts, task, lane);
    } else {
        if (P.rounded) attention_partial_impl<4, true>(q, k, v, sinks, partial, stats, P.count, P.cap, P.parts, task, lane);
        else attention_partial_impl<4, false>(q, k, v, sinks, partial, stats, P.count, P.cap, P.parts, task, lane);
    }
}

// Merge the partitions: one SIMD group owns sixteen output dimensions of a head.
kernel void flint_attention_merge(device bf16* out [[buffer(0)]],
                                  const device float* partial [[buffer(1)]],
                                  const device float* stats [[buffer(2)]],
                                  constant AttentionParams& P [[buffer(3)]],
                                  uint group [[threadgroup_position_in_grid]],
                                  uint lane [[thread_index_in_simdgroup]],
                                  uint sg [[simdgroup_index_in_threadgroup]]) {
    uint task = group * 8 + sg;
    if (task >= 32 * 8) return;
    uint parts = P.parts;
    uint h = task / 8, dim = (task % 8) * 16;
    float top = -3.402823466e+38f;
    for (uint p = lane; p < parts; p += 32) top = max(top, stats[(h * parts + p) * 2]);
    top = simd_max(top);
    float z = 0.0f;
    float4 a[4] = {float4(0), float4(0), float4(0), float4(0)};
    for (uint p = lane; p < parts; p += 32) {
        uint row = h * parts + p;
        float factor = metal::fast::exp(stats[row * 2] - top);
        z += factor * stats[row * 2 + 1];
        for (uint tile = 0; tile < 4; ++tile) {
            float4 value;
            if (P.rounded) {
                uint at = ((h * 32 + dim / 4 + tile) * parts + p) * 4;
                const device bf16* raw = ((const device bf16*)partial) + at;
                value = float4(float(raw[0]), float(raw[1]), float(raw[2]), float(raw[3]));
            } else {
                value = *((const device float4*)(partial + (h * parts + p) * 128 + dim + tile * 4));
            }
            a[tile] += factor * value;
        }
    }
    z = simd_sum(z);
    float owned[2] = {0.0f, 0.0f};
    for (uint tile = 0; tile < 4; ++tile) for (uint c = 0; c < 4; ++c) {
        uint j = tile * 4 + c;
        float total = simd_sum(a[tile][c]);
        if (lane == j / 2) owned[j % 2] = total;
    }
    if (lane < 8) for (uint t = 0; t < 2; ++t) {
        uint j = lane * 2 + t;
        out[h * 128 + dim + j] = bf16(z == 0.0f ? owned[t] : owned[t] / z);
    }
}

// Rows of an INT4 matrix-vector product, then a BF16 residual add: h += W x.
// Also the dense down projection (K = 6144).
template <uint K>
inline void residual_rows(Q4 m, const device bf16* x, device bf16* h, uint rows, uint group, uint lane, uint sg) {
    uint first = (group * 8 + sg) * 4;
    if (first >= rows) return;
    float r[4];
    q4_rows4<K>(m, x, first, lane, r);
    if (lane == 0) for (uint j = 0; j < 4; ++j) h[first + j] = bf16(float(h[first + j]) + float(bf16(r[j])));
}

kernel void flint_residual_4096(const device uchar* w [[buffer(0)]], const device bf16* s [[buffer(1)]], const device bf16* b [[buffer(2)]],
                                const device bf16* x [[buffer(3)]], device bf16* h [[buffer(4)]],
                                uint group [[threadgroup_position_in_grid]], uint lane [[thread_index_in_simdgroup]], uint sg [[simdgroup_index_in_threadgroup]]) {
    residual_rows<4096>({w, s, b}, x, h, 2048, group, lane, sg);
}

kernel void flint_residual_6144(const device uchar* w [[buffer(0)]], const device bf16* s [[buffer(1)]], const device bf16* b [[buffer(2)]],
                                const device bf16* x [[buffer(3)]], device bf16* h [[buffer(4)]],
                                uint group [[threadgroup_position_in_grid]], uint lane [[thread_index_in_simdgroup]], uint sg [[simdgroup_index_in_threadgroup]]) {
    residual_rows<6144>({w, s, b}, x, h, 2048, group, lane, sg);
}

// Post-attention norm (published by group 0) and the BF16 router: 8 experts per group.
kernel void flint_norm_router(const device bf16* h [[buffer(0)]],
                              const device bf16* norm [[buffer(1)]],
                              const device bf16* router [[buffer(2)]],
                              device bf16* n_out [[buffer(3)]],
                              device float* probs [[buffer(4)]],
                              constant uint& sparse [[buffer(5)]],
                              uint group [[threadgroup_position_in_grid]],
                              uint tid [[thread_index_in_threadgroup]],
                              uint lane [[thread_index_in_simdgroup]],
                              uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup bf16 n[2048];
    threadgroup float temp[32];
    rms2048(h, norm, n, temp, tid, lane, sg);
    if (group == 0) for (uint i = tid; i < 2048; i += 256) n_out[i] = n[i];
    if (!sparse) return;
    uint row = group * 8 + sg;
    float a = 0.0f;
    for (uint blk = 0; blk < 16; ++blk) {
        uint at = blk * 128 + lane * 4;
        for (uint j = 0; j < 4; ++j) a += float(router[row * 2048 + at + j]) * float(n[at + j]);
    }
    for (ushort d = 16; d >= 1; d >>= 1) a += simd_shuffle_down(a, d);
    if (lane == 0) probs[row] = sigmoid_precise(float(bf16(a)));
}

// Gate and up projections of the six routed experts and the shared expert,
// with SwiGLU. 4096 activation rows, two per SIMD group. Group 0 publishes routing.
kernel void flint_moe_gateup(const device bf16* n [[buffer(0)]],
                             const device float* probs [[buffer(1)]],
                             const device uchar* gw [[buffer(2)]], const device bf16* gs [[buffer(3)]], const device bf16* gb [[buffer(4)]],
                             const device uchar* uw [[buffer(5)]], const device bf16* us [[buffer(6)]], const device bf16* ub [[buffer(7)]],
                             const device uchar* sgw [[buffer(8)]], const device bf16* sgs [[buffer(9)]], const device bf16* sgb [[buffer(10)]],
                             const device uchar* suw [[buffer(11)]], const device bf16* sus [[buffer(12)]], const device bf16* sub [[buffer(13)]],
                             device bf16* act [[buffer(14)]],
                             device float* routing [[buffer(15)]],
                             uint group [[threadgroup_position_in_grid]],
                             uint tid [[thread_index_in_threadgroup]],
                             uint lane [[thread_index_in_simdgroup]],
                             uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup uint ids[6];
    threadgroup float weights[6];
    threadgroup bf16 x[2048];
    for (uint i = tid; i < 2048; i += 256) x[i] = n[i];
    if (sg == 0) route_top6(probs, ids, weights, lane);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (group == 0 && tid < 6) { ((device uint*)routing)[tid] = ids[tid]; routing[8 + tid] = weights[tid]; }
    uint first = (group * 8 + sg) * 2;
    bool shared = first >= 3072;
    uint role = first / 512;
    uint local = shared ? first - 3072 : first % 512;
    Q4 g = shared ? Q4{sgw, sgs, sgb} : Q4{gw, gs, gb};
    Q4 u = shared ? Q4{suw, sus, sub} : Q4{uw, us, ub};
    uint rows = shared ? 1024 : 512;
    uint expert = shared ? 0 : ids[role];
    float ga[2] = {0.0f, 0.0f}, ua[2] = {0.0f, 0.0f};
    for (uint blk = 0; blk < 2048; blk += 512) {
        uint scalar = blk + lane * 16;
        float v[16];
        float sum = load16((const threadgroup bf16*)x + scalar, v);
        for (uint j = 0; j < 2; ++j) {
            uint row = expert * rows + local + j, gi = row * 32 + scalar / 64;
            ga[j] += dot16(g.w + size_t(row) * 1024 + scalar / 2, v, float(g.s[gi]), float(g.b[gi]), sum);
            ua[j] += dot16(u.w + size_t(row) * 1024 + scalar / 2, v, float(u.s[gi]), float(u.b[gi]), sum);
        }
    }
    for (uint j = 0; j < 2; ++j) {
        float2 val = simd_sum(float2(ga[j], ua[j]));
        if (lane == 0) act[first + j] = swiglu(bf16(val.x), bf16(val.y), !shared);
    }
}

// Down projections of the six experts and the shared expert, the weighted
// BF16 combine, and the residual add. Four output rows per SIMD group.
kernel void flint_moe_down(const device bf16* act [[buffer(0)]],
                           const device float* routing [[buffer(1)]],
                           const device uchar* dw [[buffer(2)]], const device bf16* ds [[buffer(3)]], const device bf16* db [[buffer(4)]],
                           const device uchar* sdw [[buffer(5)]], const device bf16* sds [[buffer(6)]], const device bf16* sdb [[buffer(7)]],
                           device bf16* h [[buffer(8)]],
                           uint group [[threadgroup_position_in_grid]],
                           uint lane [[thread_index_in_simdgroup]],
                           uint sg [[simdgroup_index_in_threadgroup]]) {
    uint first = (group * 8 + sg) * 4;
    float d[7][4];
    for (uint r = 0; r < 6; ++r) {
        uint expert = ((const device uint*)routing)[r];
        q4_rows4<512>({dw, ds, db}, act + r * 512, expert * 2048 + first, lane, d[r]);
    }
    q4_rows4<1024>({sdw, sds, sdb}, act + 3072, first, lane, d[6]);
    if (lane != 0) return;
    for (uint j = 0; j < 4; ++j) {
        float sums = float(bf16(float(bf16(d[0][j])) * float(bf16(routing[8])))) + 0.0f;
        for (uint r = 1; r < 6; ++r)
            sums = float(bf16((float(bf16(float(bf16(d[r][j])) * float(bf16(routing[8 + r])))) + 0.0f) + sums));
        bf16 mlp = bf16(float(bf16(sums)) + float(bf16(d[6][j])));
        h[first + j] = bf16(float(h[first + j]) + float(mlp));
    }
}

// Dense SwiGLU gate and up rows (6144), four rows per SIMD group.
kernel void flint_dense_gateup(const device bf16* n [[buffer(0)]],
                               const device uchar* gw [[buffer(1)]], const device bf16* gs [[buffer(2)]], const device bf16* gb [[buffer(3)]],
                               const device uchar* uw [[buffer(4)]], const device bf16* us [[buffer(5)]], const device bf16* ub [[buffer(6)]],
                               device bf16* act [[buffer(7)]],
                               uint group [[threadgroup_position_in_grid]],
                               uint tid [[thread_index_in_threadgroup]],
                               uint lane [[thread_index_in_simdgroup]],
                               uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup bf16 x[2048];
    for (uint i = tid; i < 2048; i += 256) x[i] = n[i];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    uint first = (group * 8 + sg) * 4;
    float g[4], u[4];
    q4_rows4<2048>({gw, gs, gb}, (const threadgroup bf16*)x, first, lane, g);
    q4_rows4<2048>({uw, us, ub}, (const threadgroup bf16*)x, first, lane, u);
    if (lane == 0) for (uint j = 0; j < 4; ++j) act[first + j] = swiglu(bf16(g[j]), bf16(u[j]), false);
}

// Final norm and the INT4 vocabulary head. Logits are BF16 values held as FP32.
kernel void flint_head(const device bf16* h [[buffer(0)]],
                       const device bf16* norm [[buffer(1)]],
                       const device uchar* w [[buffer(2)]], const device bf16* s [[buffer(3)]], const device bf16* b [[buffer(4)]],
                       device float* logits [[buffer(5)]],
                       constant uint& vocab [[buffer(6)]],
                       uint group [[threadgroup_position_in_grid]],
                       uint tid [[thread_index_in_threadgroup]],
                       uint lane [[thread_index_in_simdgroup]],
                       uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup bf16 n[2048];
    threadgroup float temp[32];
    rms2048(h, norm, n, temp, tid, lane, sg);
    uint first = (group * 8 + sg) * 4;
    if (first >= vocab) return;
    float r[4];
    q4_rows4<2048>({w, s, b}, (const threadgroup bf16*)n, first, lane, r);
    if (lane == 0) for (uint j = 0; j < 4; ++j) logits[first + j] = float(bf16(r[j]));
}

// Greedy choice over BF16 logits, lowest index on ties, as one threadgroup.
kernel void flint_argmax(const device float* logits [[buffer(0)]],
                         device uint* choice [[buffer(1)]],
                         constant uint& vocab [[buffer(2)]],
                         uint tid [[thread_index_in_threadgroup]],
                         uint lane [[thread_index_in_simdgroup]],
                         uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup float best_value[32];
    threadgroup uint best_index[32];
    float best = -INFINITY; uint index = 0xffffffffu;
    for (uint i = tid; i < vocab; i += 1024) {
        float v = logits[i];
        if (v > best || (v == best && i < index)) { best = v; index = i; }
    }
    float peak = simd_max(best);
    uint first = simd_min(best == peak ? index : 0xffffffffu);
    if (lane == 0) { best_value[sg] = peak; best_index[sg] = first; }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (sg == 0) {
        float v = best_value[lane]; uint i = best_index[lane];
        float top = simd_max(v);
        uint pick = simd_min(v == top ? i : 0xffffffffu);
        if (lane == 0) choice[0] = pick;
    }
}


// Top-k over the vocabulary in two stages, k <= 32. Stage one: each
// threadgroup ranks a 2048-entry slice by repeated argmax (highest logit,
// lower index on ties) and flags non-finite logits.
inline void argmax_tg(threadgroup float* v, threadgroup uint* ix, uint n, uint tid, uint lane, uint sg,
                      threadgroup float* bv, threadgroup uint* bi, thread float& best, thread uint& index) {
    best = -INFINITY; index = 0xffffffffu;
    for (uint i = tid; i < n; i += 256) {
        float x = v[i]; uint j = ix[i];
        if (x > best || (x == best && j < index)) { best = x; index = j; }
    }
    float peak = simd_max(best);
    uint first = simd_min(best == peak ? index : 0xffffffffu);
    if (lane == 0) { bv[sg] = peak; bi[sg] = first; }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float x = lane < 8 ? bv[lane] : -INFINITY;
    uint j = lane < 8 ? bi[lane] : 0xffffffffu;
    float top = simd_max(x);
    index = simd_min(x == top ? j : 0xffffffffu);
    best = top;
    threadgroup_barrier(mem_flags::mem_threadgroup);
}

kernel void flint_topk_partial(const device float* logits [[buffer(0)]],
                               device float* cand_v [[buffer(1)]],
                               device uint* cand_i [[buffer(2)]],
                               device atomic_uint* bad [[buffer(3)]],
                               constant uint& vocab [[buffer(4)]],
                               constant uint& k [[buffer(5)]],
                               uint group [[threadgroup_position_in_grid]],
                               uint tid [[thread_index_in_threadgroup]],
                               uint lane [[thread_index_in_simdgroup]],
                               uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup float v[2048];
    threadgroup uint ix[2048];
    threadgroup float bv[8];
    threadgroup uint bi[8];
    uint base = group * 2048;
    bool finite = true;
    for (uint i = tid; i < 2048; i += 256) {
        uint j = base + i;
        float x = j < vocab ? logits[j] : -INFINITY;
        if (j < vocab && !isfinite(x)) finite = false;
        v[i] = isfinite(x) ? x : -INFINITY;
        ix[i] = j < vocab ? j : 0xffffffffu;
    }
    if (!finite) atomic_fetch_or_explicit(bad, 1u, memory_order_relaxed);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint r = 0; r < k; ++r) {
        float best; uint index;
        argmax_tg(v, ix, 2048, tid, lane, sg, bv, bi, best, index);
        if (tid == 0) { cand_v[group * k + r] = best; cand_i[group * k + r] = index; }
        if (index != 0xffffffffu && index >= base && index - base < 2048 && tid == (index - base) % 256)
            v[index - base] = -INFINITY;
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
}

// Stage two: one threadgroup ranks every slice's candidates.
kernel void flint_topk_merge(const device float* cand_v [[buffer(0)]],
                             const device uint* cand_i [[buffer(1)]],
                             device float* out_v [[buffer(2)]],
                             device uint* out_i [[buffer(3)]],
                             constant uint& count [[buffer(4)]],
                             constant uint& k [[buffer(5)]],
                             uint tid [[thread_index_in_threadgroup]],
                             uint lane [[thread_index_in_simdgroup]],
                             uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup float v[2560];
    threadgroup uint ix[2560];
    threadgroup float bv[8];
    threadgroup uint bi[8];
    for (uint i = tid; i < count; i += 256) { v[i] = cand_v[i]; ix[i] = cand_i[i]; }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint r = 0; r < k; ++r) {
        float best; uint index;
        argmax_tg(v, ix, count, tid, lane, sg, bv, bi, best, index);
        if (tid == 0) { out_v[r] = best; out_i[r] = index; }
        for (uint i = tid; i < count; i += 256) if (ix[i] == index) v[i] = -INFINITY;
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
}


struct SampleParams {
    uint greedy;       // 1: take the greedy choice
    float temperature;
    float top_p;
    uint k;            // ranked candidates in top_v and top_i
    uint step;         // index into draws and out_tokens
};

// Choose the next token on the GPU and hand it to the next step: the greedy
// choice, or Sampling::choose_ranked's rule (top-k, temperature, top-p, one
// uniform draw per step from the host's seeded generator) in FP32.
kernel void flint_sample(const device float* top_v [[buffer(0)]],
                         const device uint* top_i [[buffer(1)]],
                         const device uint* greedy_choice [[buffer(2)]],
                         const device float* draws [[buffer(3)]],
                         device uint* token_slot [[buffer(4)]],
                         device uint* out_tokens [[buffer(5)]],
                         constant SampleParams& P [[buffer(6)]],
                         uint tid [[thread_index_in_threadgroup]]) {
    if (tid != 0) return;
    uint token;
    if (P.greedy || P.temperature == 0.0f) {
        token = greedy_choice[0];
    } else {
        float top = top_v[0];
        float w[32];
        float total = 0.0f;
        for (uint i = 0; i < P.k; ++i) { w[i] = metal::precise::exp((top_v[i] - top) / P.temperature); total += w[i]; }
        float cumulative = 0.0f;
        uint keep = 0;
        for (uint i = 0; i < P.k; ++i) {
            cumulative += w[i] / total;
            keep += 1;
            if (cumulative >= P.top_p) break;
        }
        float kept = 0.0f;
        for (uint i = 0; i < keep; ++i) kept += w[i];
        float draw = draws[P.step] * kept;
        token = top_i[keep - 1];
        for (uint i = 0; i < keep; ++i) {
            draw -= w[i];
            if (draw <= 0.0f) { token = top_i[i]; break; }
        }
    }
    token_slot[0] = token;
    out_tokens[P.step] = token;
}

// ---------------------------------------------------------------------------
// Prefill: a chunk of T tokens at once.

// RMSNorm of T rows into a BF16 matrix; one threadgroup per row.
kernel void flint_rms_rows(const device bf16* h [[buffer(0)]],
                           const device bf16* norm [[buffer(1)]],
                           device bf16* out [[buffer(2)]],
                           uint row [[threadgroup_position_in_grid]],
                           uint tid [[thread_index_in_threadgroup]],
                           uint lane [[thread_index_in_simdgroup]],
                           uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup bf16 n[2048];
    threadgroup float temp[32];
    rms2048(h + size_t(row) * 2048, norm, n, temp, tid, lane, sg);
    for (uint i = tid; i < 2048; i += 256) out[size_t(row) * 2048 + i] = n[i];
}

struct GemmParams {
    uint m;          // rows of X (or routed slots)
    uint n;          // output columns (weight rows)
    uint k;          // reduction size, a multiple of 64
    uint ldx;        // X row stride in elements
    uint ldy;        // Y row stride in elements
    uint mode;       // 0 store, 1 residual add into Y, 2 SwiGLU of (gate, up) pairs,
                     // 3 BF16 router weights with sigmoid FP32 output
    uint gathered;   // 0 dense; 1 expert tiles with X rows from `rows`; 2 expert tiles, X by slot
    uint tiles;      // tile count when gathered
    uint expert_stride_w; // bytes between experts in W
    uint expert_stride_g; // elements between experts in scales and biases
};

// INT4 matrix product Y = X W^T on BF16 simdgroup matrices with FP32
// accumulation. 64x64 tiles; four SIMD groups of 32x32. Weights dequantize to
// BF16 as MLX's qmm does: bf16(bf16(scale * q) + bias). In mode 2 a tile holds
// 32 gate rows (W) and the same 32 up rows (W2), and Y gets SwiGLU(gate, up).
kernel void flint_gemm_q4(const device bf16* x [[buffer(0)]],
                          const device uchar* w [[buffer(1)]], const device bf16* s [[buffer(2)]], const device bf16* b [[buffer(3)]],
                          const device uchar* w2 [[buffer(4)]], const device bf16* s2 [[buffer(5)]], const device bf16* b2 [[buffer(6)]],
                          device bf16* y [[buffer(7)]],
                          constant GemmParams& P [[buffer(8)]],
                          const device uint* rows [[buffer(9)]],
                          const device uint* tiles [[buffer(10)]],
                          uint2 tg [[threadgroup_position_in_grid]],
                          uint tid [[thread_index_in_threadgroup]],
                          uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup float stage[64 * 64];
    threadgroup bf16* xs = (threadgroup bf16*)stage;
    threadgroup bf16* ws = xs + 64 * 64;
    bool swiglu_mode = P.mode == 2;
    uint out_cols = swiglu_mode ? 32 : 64;
    uint n0 = tg.x * out_cols;
    uint m0, m_end, expert = 0;
    if (P.gathered) {
        if (tg.y >= tiles[0]) return;
        expert = tiles[1 + tg.y * 3];
        m0 = tiles[2 + tg.y * 3];
        m_end = m0 + tiles[3 + tg.y * 3];
    } else {
        m0 = tg.y * 64; m_end = min(m0 + 64, P.m);
    }
    if (m0 >= m_end || n0 >= P.n) return;
    uint groups = P.k / 64;
    // This thread's load assignment: one row half (32 values) of each tile.
    uint lr = tid / 2, lc = (tid % 2) * 32;
    uint mrow = m0 + lr;
    bool x_valid = mrow < m_end;
    const device bf16* xrow = x;
    if (x_valid) xrow = x + size_t(P.gathered == 1 ? rows[mrow] : mrow) * P.ldx;
    bool up_row = swiglu_mode && lr >= 32;
    uint nrow = swiglu_mode ? n0 + lr % 32 : n0 + lr;
    bool w_valid = nrow < P.n;
    const device uchar* wb = (up_row ? w2 : w) + size_t(expert) * P.expert_stride_w + size_t(nrow) * (P.k / 2);
    const device bf16* sb = (up_row ? s2 : s) + size_t(expert) * P.expert_stride_g + size_t(nrow) * groups;
    const device bf16* bb = (up_row ? b2 : b) + size_t(expert) * P.expert_stride_g + size_t(nrow) * groups;
    uint sm = sg / 2, sn = sg % 2;
    simdgroup_float8x8 acc[4][4];
    for (uint i = 0; i < 4; ++i) for (uint j = 0; j < 4; ++j) acc[i][j] = simdgroup_float8x8(0.0f);
    for (uint kb = 0; kb < P.k; kb += 64) {
        threadgroup bf16* xd = xs + lr * 64 + lc;
        if (x_valid) {
            const device bf16* xp = xrow + kb + lc;
            for (uint j = 0; j < 32; ++j) xd[j] = xp[j];
        } else {
            for (uint j = 0; j < 32; ++j) xd[j] = bf16(0.0f);
        }
        threadgroup bf16* wd = ws + lr * 64 + lc;
        if (w_valid && P.mode == 3) {
            const device bf16* wp = (const device bf16*)w + size_t(nrow) * P.k + kb + lc;
            for (uint j = 0; j < 32; ++j) wd[j] = wp[j];
        } else if (w_valid) {
            float sc = float(sb[kb / 64]), bi = float(bb[kb / 64]);
            const device uint2* wp = (const device uint2*)(wb + (kb + lc) / 2);
            for (uint part = 0; part < 2; ++part) {
                uint2 packed = wp[part];
                for (uint word = 0; word < 2; ++word) {
                    uint bits = packed[word];
                    for (uint j = 0; j < 8; ++j) {
                        float q = float((bits >> (4 * j)) & 15u);
                        wd[part * 16 + word * 8 + j] = bf16(float(bf16(sc * q)) + bi);
                    }
                }
            }
        } else {
            for (uint j = 0; j < 32; ++j) wd[j] = bf16(0.0f);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint kk = 0; kk < 64; kk += 8) {
            simdgroup_matrix<bf16, 8, 8> a[4], bm[4];
            for (uint i = 0; i < 4; ++i) simdgroup_load(a[i], xs + (sm * 32 + i * 8) * 64 + kk, 64);
            for (uint j = 0; j < 4; ++j) simdgroup_load(bm[j], ws + (sn * 32 + j * 8) * 64 + kk, 64, ulong2(0, 0), true);
            for (uint i = 0; i < 4; ++i) for (uint j = 0; j < 4; ++j) simdgroup_multiply_accumulate(acc[i][j], a[i], bm[j], acc[i][j]);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    for (uint i = 0; i < 4; ++i) for (uint j = 0; j < 4; ++j)
        simdgroup_store(acc[i][j], stage + (sm * 32 + i * 8) * 64 + sn * 32 + j * 8, 64);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint e = tid; e < 64 * out_cols; e += 128) {
        uint r = e / out_cols, c = e % out_cols;
        uint row = m0 + r, col = n0 + c;
        if (row >= m_end || col >= P.n) continue;
        device bf16* out = y + size_t(row) * P.ldy + col;
        float v = stage[r * 64 + c];
        if (P.mode == 3) ((device float*)y)[size_t(row) * P.ldy + col] = sigmoid_precise(float(bf16(v)));
        else if (P.mode == 1) *out = bf16(float(*out) + float(bf16(v)));
        else if (swiglu_mode) *out = swiglu(bf16(v), bf16(stage[r * 64 + 32 + c]), P.gathered != 0);
        else *out = bf16(v);
    }
}


#if defined(__HAVE_TENSOR__)
// The same INT4 product with the multiply on Metal 4 tensor ops, which use the
// GPU's matrix units where the chip has them. Staging and epilogue match
// flint_gemm_q4.
kernel void flint_gemm_q4_tensor(const device bf16* x [[buffer(0)]],
                          const device uchar* w [[buffer(1)]], const device bf16* s [[buffer(2)]], const device bf16* b [[buffer(3)]],
                          const device uchar* w2 [[buffer(4)]], const device bf16* s2 [[buffer(5)]], const device bf16* b2 [[buffer(6)]],
                          device bf16* y [[buffer(7)]],
                          constant GemmParams& P [[buffer(8)]],
                          const device uint* rows [[buffer(9)]],
                          const device uint* tiles [[buffer(10)]],
                          uint2 tg [[threadgroup_position_in_grid]],
                          uint tid [[thread_index_in_threadgroup]]) {
    using namespace mpp::tensor_ops;
    threadgroup float stage[64 * 64];
    threadgroup bf16* xs = (threadgroup bf16*)stage;
    threadgroup bf16* ws = xs + 64 * 64;
    bool swiglu_mode = P.mode == 2;
    uint out_cols = swiglu_mode ? 32 : 64;
    uint n0 = tg.x * out_cols;
    uint m0, m_end, expert = 0;
    if (P.gathered) {
        if (tg.y >= tiles[0]) return;
        expert = tiles[1 + tg.y * 3];
        m0 = tiles[2 + tg.y * 3];
        m_end = m0 + tiles[3 + tg.y * 3];
    } else {
        m0 = tg.y * 64; m_end = min(m0 + 64, P.m);
    }
    if (m0 >= m_end || n0 >= P.n) return;
    uint groups = P.k / 64;
    uint lr = tid / 2, lc = (tid % 2) * 32;
    uint mrow = m0 + lr;
    bool x_valid = mrow < m_end;
    const device bf16* xrow = x;
    if (x_valid) xrow = x + size_t(P.gathered == 1 ? rows[mrow] : mrow) * P.ldx;
    bool up_row = swiglu_mode && lr >= 32;
    uint nrow = swiglu_mode ? n0 + lr % 32 : n0 + lr;
    bool w_valid = nrow < P.n;
    const device uchar* wb = (up_row ? w2 : w) + size_t(expert) * P.expert_stride_w + size_t(nrow) * (P.k / 2);
    const device bf16* sb = (up_row ? s2 : s) + size_t(expert) * P.expert_stride_g + size_t(nrow) * groups;
    const device bf16* bb = (up_row ? b2 : b) + size_t(expert) * P.expert_stride_g + size_t(nrow) * groups;
    auto tx = tensor<threadgroup bf16, dextents<int32_t, 2>, tensor_inline>(xs, dextents<int32_t, 2>(64, 64));
    auto tw = tensor<threadgroup bf16, dextents<int32_t, 2>, tensor_inline>(ws, dextents<int32_t, 2>(64, 64));
    constexpr auto desc = matmul2d_descriptor(64, 64, 64, false, true, false, matmul2d_descriptor::mode::multiply_accumulate);
    matmul2d<desc, execution_simdgroups<4>> op;
    auto acc = op.get_destination_cooperative_tensor<decltype(tx), decltype(tw), float>();
    #pragma clang loop unroll(full)
    for (uint16_t i = 0; i < acc.get_capacity(); ++i) if (acc.is_valid_element(i)) acc[i] = 0.0f;
    for (uint kb = 0; kb < P.k; kb += 64) {
        threadgroup bf16* xd = xs + lr * 64 + lc;
        if (x_valid) {
            const device bf16* xp = xrow + kb + lc;
            for (uint j = 0; j < 32; ++j) xd[j] = xp[j];
        } else {
            for (uint j = 0; j < 32; ++j) xd[j] = bf16(0.0f);
        }
        threadgroup bf16* wd = ws + lr * 64 + lc;
        if (w_valid && P.mode == 3) {
            const device bf16* wp = (const device bf16*)w + size_t(nrow) * P.k + kb + lc;
            for (uint j = 0; j < 32; ++j) wd[j] = wp[j];
        } else if (w_valid) {
            float sc = float(sb[kb / 64]), bi = float(bb[kb / 64]);
            const device uint2* wp = (const device uint2*)(wb + (kb + lc) / 2);
            for (uint part = 0; part < 2; ++part) {
                uint2 packed = wp[part];
                for (uint word = 0; word < 2; ++word) {
                    uint bits = packed[word];
                    for (uint j = 0; j < 8; ++j) {
                        float q = float((bits >> (4 * j)) & 15u);
                        wd[part * 16 + word * 8 + j] = bf16(float(bf16(sc * q)) + bi);
                    }
                }
            }
        } else {
            for (uint j = 0; j < 32; ++j) wd[j] = bf16(0.0f);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        op.run(tx, tw, acc);
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    auto ts = tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline>(stage, dextents<int32_t, 2>(64, 64));
    acc.store(ts);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint e = tid; e < 64 * out_cols; e += 128) {
        uint r = e / out_cols, c = e % out_cols;
        uint row = m0 + r, col = n0 + c;
        if (row >= m_end || col >= P.n) continue;
        device bf16* out = y + size_t(row) * P.ldy + col;
        float v = stage[r * 64 + c];
        if (P.mode == 3) ((device float*)y)[size_t(row) * P.ldy + col] = sigmoid_precise(float(bf16(v)));
        else if (P.mode == 1) *out = bf16(float(*out) + float(bf16(v)));
        else if (swiglu_mode) *out = swiglu(bf16(v), bf16(stage[r * 64 + 32 + c]), P.gathered != 0);
        else *out = bf16(v);
    }
}
#endif

struct RowParams {
    uint start;  // absolute position of the first row
    uint cap;    // KV capacity for this layer
    uint rotary;
    uint global;
    uint count;  // rows in the chunk
};

// Head norm and RoPE for a chunk: one SIMD group per (row, head). qkv rows are
// [q 4096 | k 512 | v 512]. Keys and values go to the chunk KV buffer.
kernel void flint_rows_head_norm(device bf16* qkv [[buffer(0)]],
                                 const device bf16* q_norm [[buffer(1)]],
                                 const device bf16* k_norm [[buffer(2)]],
                                 constant RowParams& P [[buffer(3)]],
                                 uint group [[threadgroup_position_in_grid]],
                                 uint lane [[thread_index_in_simdgroup]],
                                 uint sg [[simdgroup_index_in_threadgroup]]) {
    uint task = group * 8 + sg;
    uint row = task / 36, head = task % 36;
    if (row >= P.count) return;
    device bf16* p = qkv + size_t(row) * 5120 + head * 128;
    head_norm(p, head < 32 ? q_norm : k_norm, P.rotary != 0, P.start + row, lane);
}

// Copy chunk keys and values into the layer cache (ring slots on sliding layers).
kernel void flint_rows_store_kv(const device bf16* qkv [[buffer(0)]],
                                device bf16* kv [[buffer(1)]],
                                constant RowParams& P [[buffer(2)]],
                                uint2 gid [[thread_position_in_grid]]) {
    uint i = gid.x, row = gid.y;
    if (row >= P.count || i >= 1024) return;
    uint position = P.start + row;
    if (!P.global && position + P.cap < P.start + P.count) return;
    uint slot = P.global ? position : position % P.cap;
    uint which = i / 512, head = (i % 512) / 128, dim = i % 128;
    kv[((size_t(which) * 4 + head) * P.cap + slot) * 128 + dim] = qkv[size_t(row) * 5120 + 4096 + i];
}

// Position of this thread's two elements in an 8x8 simdgroup matrix: row fm,
// columns fn and fn + 1 (the layout MLX's steel kernels use).
inline short2 frag_coord(ushort lane) {
    short qid = lane / 4;
    short fm = (qid & 4) + ((lane / 2) % 4);
    short fn = (qid & 2) * 2 + (lane % 2) * 2;
    return short2(fn, fm);
}

// Causal flash attention for a chunk with a learned sink. A threadgroup owns
// one KV head and eight query positions; each of its eight SIMD groups owns
// one position as an 8-row tile whose rows are that KV head's eight query
// heads, so the tile shares one causal limit. Queries stay in registers and
// the threadgroup shares each 32-key block of K and V. Keys before the chunk
// come from the cache (ring slots on sliding layers); chunk keys from qkv.
kernel void flint_rows_attention(const device bf16* qkv [[buffer(0)]],
                                 const device bf16* kv [[buffer(1)]],
                                 const device bf16* sinks [[buffer(2)]],
                                 device bf16* out [[buffer(3)]],
                                 constant RowParams& P [[buffer(4)]],
                                 constant uint& window [[buffer(5)]],
                                 uint2 tg [[threadgroup_position_in_grid]],
                                 uint tid [[thread_index_in_threadgroup]],
                                 ushort lane [[thread_index_in_simdgroup]],
                                 ushort sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup bf16 ks[32 * 128];
    threadgroup bf16 vs[32 * 128];
    uint g = tg.y, pb = tg.x * 8;
    if (pb >= P.count) return;
    uint rows = min(8u, P.count - pb);
    short2 xy = frag_coord(lane);
    short fn = xy.x, fm = xy.y;
    bool live = sg < rows;
    uint pos = P.start + pb + sg;
    uint lo = P.global ? 0 : (pos + 1 > window ? pos + 1 - window : 0);
    // Scores are kept in base-2 units so the softmax can use exp2.
    const float log2e = 1.4426950408889634f;
    float m = float(sinks[g * 8 + fm]) * log2e, l = 1.0f;
    simdgroup_float8x8 o[16];
    simdgroup_matrix<bf16, 8, 8> qa[16];
    const device bf16* q = qkv + size_t(pb + (live ? sg : 0)) * 5120 + g * 8 * 128;
    for (uint d = 0; d < 16; ++d) {
        o[d] = simdgroup_float8x8(0.0f);
        simdgroup_load(qa[d], q + d * 8, 128);
    }
    uint p_first = P.start + pb, p_last = P.start + pb + rows - 1;
    uint k_first = P.global ? 0 : (p_first + 1 > window ? p_first + 1 - window : 0);
    const device bf16* ck = kv + size_t(g) * P.cap * 128;
    const device bf16* cv = kv + size_t(4 + g) * P.cap * 128;
    const float scale = 0.08838834764831845f * log2e;
    for (uint kb = k_first; kb <= p_last; kb += 32) {
        for (uint i = tid; i < 32 * 16; i += 256) {
            uint j = i / 16, c = (i % 16) * 8, t = kb + j;
            threadgroup bf16* kd = ks + j * 128 + c;
            threadgroup bf16* vd = vs + j * 128 + c;
            if (t <= p_last) {
                const device bf16* kp;
                const device bf16* vp;
                if (t >= P.start) {
                    size_t at = size_t(t - P.start) * 5120 + g * 128 + c;
                    kp = qkv + 4096 + at; vp = qkv + 4608 + at;
                } else {
                    uint slot = P.global ? t : t % P.cap;
                    kp = ck + size_t(slot) * 128 + c; vp = cv + size_t(slot) * 128 + c;
                }
                *(threadgroup uint4*)kd = *(const device uint4*)kp;
                *(threadgroup uint4*)vd = *(const device uint4*)vp;
            } else {
                *(threadgroup uint4*)kd = uint4(0);
                *(threadgroup uint4*)vd = uint4(0);
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (live && kb <= pos && kb + 32 > lo) {
            simdgroup_float8x8 sc[4];
            for (uint t = 0; t < 4; ++t) sc[t] = simdgroup_float8x8(0.0f);
            for (uint d = 0; d < 16; ++d) {
                for (uint t = 0; t < 4; ++t) {
                    simdgroup_matrix<bf16, 8, 8> kf;
                    simdgroup_load(kf, ks + (t * 8) * 128 + d * 8, 128, ulong2(0, 0), true);
                    simdgroup_multiply_accumulate(sc[t], qa[d], kf, sc[t]);
                }
            }
            float top = -INFINITY;
            for (uint t = 0; t < 4; ++t) {
                for (uint e = 0; e < 2; ++e) {
                    uint key = kb + t * 8 + fn + e;
                    float v = sc[t].thread_elements()[e] * scale;
                    v = (key >= lo && key <= pos) ? v : -INFINITY;
                    sc[t].thread_elements()[e] = v;
                    top = max(top, v);
                }
            }
            top = max(top, simd_shuffle_xor(top, 1));
            top = max(top, simd_shuffle_xor(top, 8));
            float next = max(m, top);
            float factor = metal::exp2(m - next);
            float sum = 0.0f;
            simdgroup_matrix<bf16, 8, 8> pm[4];
            for (uint t = 0; t < 4; ++t) {
                for (uint e = 0; e < 2; ++e) {
                    float p = metal::exp2(sc[t].thread_elements()[e] - next);
                    sum += p;
                    pm[t].thread_elements()[e] = bf16(p);
                }
            }
            sum += simd_shuffle_xor(sum, 1);
            sum += simd_shuffle_xor(sum, 8);
            l = l * factor + sum;
            m = next;
            for (uint d = 0; d < 16; ++d) {
                o[d].thread_elements()[0] *= factor;
                o[d].thread_elements()[1] *= factor;
                for (uint t = 0; t < 4; ++t) {
                    simdgroup_matrix<bf16, 8, 8> vf;
                    simdgroup_load(vf, vs + (t * 8) * 128 + d * 8, 128);
                    simdgroup_multiply_accumulate(o[d], pm[t], vf, o[d]);
                }
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (!live) return;
    device bf16* dst = out + size_t(pb + sg) * 4096 + (g * 8 + fm) * 128 + fn;
    for (uint d = 0; d < 16; ++d) {
        dst[d * 8] = bf16(o[d].thread_elements()[0] / l);
        dst[d * 8 + 1] = bf16(o[d].thread_elements()[1] / l);
    }
}

// Top-6 per row: ids and weights into `routes` (row * 6 + rank), and per-expert counts.
kernel void flint_rows_route(const device float* probs [[buffer(0)]],
                             device uint* ids_out [[buffer(1)]],
                             device float* weights_out [[buffer(2)]],
                             device atomic_uint* counts [[buffer(3)]],
                             constant uint& count [[buffer(4)]],
                             uint group [[threadgroup_position_in_grid]],
                             uint lane [[thread_index_in_simdgroup]],
                             uint sg [[simdgroup_index_in_threadgroup]]) {
    threadgroup uint ids[8][6];
    threadgroup float weights[8][6];
    uint row = group * 8 + sg;
    if (row >= count) return;
    route_top6(probs + size_t(row) * 256, ids[sg], weights[sg], lane);
    simdgroup_barrier(mem_flags::mem_threadgroup);
    if (lane < 6) {
        ids_out[row * 6 + lane] = ids[sg][lane];
        weights_out[row * 6 + lane] = weights[sg][lane];
        atomic_fetch_add_explicit(&counts[ids[sg][lane]], 1u, memory_order_relaxed);
    }
}

// One threadgroup: exclusive offsets per expert and a 64-row tile list.
// tiles[0] = tile count, then (expert, start, count) triples.
kernel void flint_rows_plan(device uint* counts [[buffer(0)]],
                            device uint* offsets [[buffer(1)]],
                            device uint* tiles [[buffer(2)]],
                            uint tid [[thread_index_in_threadgroup]]) {
    if (tid != 0) return;
    uint total = 0, ntiles = 0;
    for (uint e = 0; e < 256; ++e) {
        uint c = counts[e];
        offsets[e] = total;
        for (uint s = 0; s < c; s += 64) {
            tiles[1 + ntiles * 3] = e;
            tiles[2 + ntiles * 3] = total + s;
            tiles[3 + ntiles * 3] = min(64u, c - s);
            ++ntiles;
        }
        total += c;
        counts[e] = 0;
    }
    offsets[256] = total;
    tiles[0] = ntiles;
}

// Scatter each (row, rank) into its expert's slot range. `slot_of` maps back.
// Slots within an expert are ordered by row, then rank, for determinism.
kernel void flint_rows_scatter(const device uint* ids [[buffer(0)]],
                               const device uint* offsets [[buffer(1)]],
                               device uint* slot_rows [[buffer(2)]],
                               device uint* slot_of [[buffer(3)]],
                               constant uint& count [[buffer(4)]],
                               uint e [[thread_position_in_grid]]) {
    if (e >= 256) return;
    uint at = offsets[e];
    for (uint row = 0; row < count; ++row)
        for (uint r = 0; r < 6; ++r)
            if (ids[row * 6 + r] == e) { slot_rows[at] = row; slot_of[row * 6 + r] = at; ++at; }
}

// Combine the six expert outputs (by slot) and the shared output, then the residual.
kernel void flint_rows_combine(const device bf16* down [[buffer(0)]],
                               const device bf16* shared_down [[buffer(1)]],
                               const device uint* slot_of [[buffer(2)]],
                               const device float* weights [[buffer(3)]],
                               device bf16* h [[buffer(4)]],
                               constant uint& count [[buffer(5)]],
                               uint2 gid [[thread_position_in_grid]]) {
    uint i = gid.x, row = gid.y;
    if (i >= 2048 || row >= count) return;
    float sums = 0.0f;
    for (uint r = 0; r < 6; ++r) {
        float d = float(down[size_t(slot_of[row * 6 + r]) * 2048 + i]);
        float term = float(bf16(d * float(bf16(weights[row * 6 + r]))));
        sums = r == 0 ? term + 0.0f : float(bf16((term + 0.0f) + sums));
    }
    bf16 mlp = bf16(float(bf16(sums)) + float(shared_down[size_t(row) * 2048 + i]));
    size_t at = size_t(row) * 2048 + i;
    h[at] = bf16(float(h[at]) + float(mlp));
}
