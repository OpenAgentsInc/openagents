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

inline float silu(float x) { return x / (1.0f + exp(-x)); }

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

kernel void clef_silu_mul_to_f16(device const float *gate [[buffer(0)]], device const float *up [[buffer(1)]],
                                 device half *out [[buffer(2)]], constant ulong &count [[buffer(3)]],
                                 uint gid [[thread_position_in_grid]]) {
    if (gid < count) {
        out[gid] = half(silu(gate[gid]) * up[gid]);
    }
}
