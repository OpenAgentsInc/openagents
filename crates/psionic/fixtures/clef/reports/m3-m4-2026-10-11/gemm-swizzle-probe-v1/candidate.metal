// One fixed tile remapping; all projection arithmetic calls the original helper.
inline bool swizzle_four(uint3 physical, uint3 nmk, uint cols, thread uint3 &logical) {
    logical = uint3(physical.x >> 2, (physical.y << 2) + (physical.x & 3), physical.z);
    return logical.x < (nmk.y + cols - 1) / cols && logical.y < (nmk.x + 63) / 64;
}
kernel void swizzle_32(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
 device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 physical [[threadgroup_position_in_grid]]) {
    uint3 logical;
    if (!swizzle_four(physical, nmk, 32, logical)) return;
    clef_gemm_shape_impl<64, 32, false>(x, w, out, nmk, logical);
}
kernel void swizzle_32_accumulate(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
 device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 physical [[threadgroup_position_in_grid]]) {
    uint3 logical;
    if (!swizzle_four(physical, nmk, 32, logical)) return;
    clef_gemm_shape_impl<64, 32, true>(x, w, out, nmk, logical);
}
kernel void swizzle_64(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
 device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 physical [[threadgroup_position_in_grid]]) {
    uint3 logical;
    if (!swizzle_four(physical, nmk, 64, logical)) return;
    clef_gemm_impl<false>(x, w, out, nmk, logical.xy);
}
kernel void swizzle_64_accumulate(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
 device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 physical [[threadgroup_position_in_grid]]) {
    uint3 logical;
    if (!swizzle_four(physical, nmk, 64, logical)) return;
    clef_gemm_impl<true>(x, w, out, nmk, logical.xy);
}
kernel void swizzle_128(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
 device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 physical [[threadgroup_position_in_grid]]) {
    uint3 logical;
    if (!swizzle_four(physical, nmk, 128, logical)) return;
    clef_gemm_shape_impl<64, 128, false>(x, w, out, nmk, logical);
}
kernel void swizzle_128_accumulate(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
 device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 physical [[threadgroup_position_in_grid]]) {
    uint3 logical;
    if (!swizzle_four(physical, nmk, 128, logical)) return;
    clef_gemm_shape_impl<64, 128, true>(x, w, out, nmk, logical);
}
