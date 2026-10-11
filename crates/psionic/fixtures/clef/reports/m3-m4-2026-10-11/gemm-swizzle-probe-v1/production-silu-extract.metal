inline float silu(float x) { return x / (1.0f + exp(-x)); }
kernel void clef_silu_mul_to_f16(device const float *gate [[buffer(0)]], device const float *up [[buffer(1)]],
                                 device half *out [[buffer(2)]], constant ulong &count [[buffer(3)]],
                                 uint gid [[thread_position_in_grid]]) {
    if (gid < count) {
        out[gid] = half(silu(gate[gid]) * up[gid]);
    }
}

