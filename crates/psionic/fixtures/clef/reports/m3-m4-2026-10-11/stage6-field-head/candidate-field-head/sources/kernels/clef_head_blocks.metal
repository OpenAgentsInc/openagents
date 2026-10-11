// Field decoder helpers. The caller checks all extents before dispatch.
kernel void clef_add_f32(
    device float *target [[buffer(0)]], device const float *update [[buffer(1)]],
    constant uint &count [[buffer(2)]], uint index [[thread_position_in_grid]]) {
    if (index < count) target[index] += update[index];
}

// One threadgroup per query/head. Scores fit in a fixed 64-row workspace.
kernel void clef_field_self_attention(
    device const float *q [[buffer(0)]], device const float *k [[buffer(1)]],
    device const float *v [[buffer(2)]], device float *out [[buffer(3)]],
    constant uint4 &nhw [[buffer(4)]], uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const uint n = nhw.x, heads = nhw.y, width = nhw.z, d = width / heads;
    const uint row = group / heads, head = group % heads, base = head * d;
    threadgroup float scores[64];
    if (tid < n) {
        float lanes[8] = {0.0f};
        uint i = 0;
        for (; i + 8 <= d; i += 8) {
            for (uint lane = 0; lane < 8; ++lane)
                lanes[lane] += q[row * width + base + i + lane] * k[tid * width + base + i + lane];
        }
        float tail = 0.0f;
        for (; i < d; ++i) tail += q[row * width + base + i] * k[tid * width + base + i];
        float sum = 0.0f;
        for (uint lane = 0; lane < 8; ++lane) sum += lanes[lane];
        scores[tid] = (sum + tail) / sqrt(float(d));
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tid == 0) {
        float maximum = scores[0];
        for (uint i = 1; i < n; ++i) maximum = max(maximum, scores[i]);
        float denominator = 0.0f;
        for (uint i = 0; i < n; ++i) {
            scores[i] = exp(scores[i] - maximum);
            denominator += scores[i];
        }
        for (uint i = 0; i < n; ++i) scores[i] /= denominator;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint column = tid; column < d; column += 64) {
        float result = 0.0f;
        for (uint i = 0; i < n; ++i) result += scores[i] * v[i * width + base + column];
        out[row * width + base + column] = result;
    }
}
