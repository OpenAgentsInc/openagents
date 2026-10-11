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
