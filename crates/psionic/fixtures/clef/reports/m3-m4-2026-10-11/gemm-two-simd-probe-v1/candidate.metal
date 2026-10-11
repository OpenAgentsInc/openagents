// Scratch two-SIMD-group variant of the unchanged 64×128 full-K projection.
// Only the execution group count and matching host thread count differ.

#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>

using namespace metal;
using namespace mpp::tensor_ops;

// out[n, m] (+)= x[n, k] * weight[m, k]^T, with f16 inputs and f32 output.
// MPP's descriptor M is the token tile height; N is the output tile width.
// Dispatch (ceil(m / OUTPUT_COLS), ceil(n / TOKEN_ROWS), 1) threadgroups
// of exactly two SIMD groups (64 threads on the supported device).
// Dynamic K lets MPP perform the complete reduction in one operation.
template <uint TOKEN_ROWS, uint OUTPUT_COLS, bool ACCUMULATE>
inline void two_simd_shape_impl(device const half *x, device const half *weight,
                                 device float *out, uint3 nmk, uint3 tgid) {
    const uint token_base = tgid.y * TOKEN_ROWS;
    const uint output_base = tgid.x * OUTPUT_COLS;
    const int n = int(nmk.x), m = int(nmk.y), k = int(nmk.z);
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tx(
        (device half *)x, dextents<int32_t, 2>(k, n));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tw(
        (device half *)weight, dextents<int32_t, 2>(k, m));
    tensor<device float, dextents<int32_t, 2>, tensor_inline> to(
        out, dextents<int32_t, 2>(m, n));
    constexpr auto desc = matmul2d_descriptor(
        TOKEN_ROWS, OUTPUT_COLS, static_cast<int>(dynamic_extent), false, true, false,
        ACCUMULATE ? matmul2d_descriptor::mode::multiply_accumulate : matmul2d_descriptor::mode::multiply);
    matmul2d<desc, execution_simdgroups<2>> op;
    if (token_base < nmk.x && nmk.x - token_base >= TOKEN_ROWS &&
        output_base < nmk.y && nmk.y - output_base >= OUTPUT_COLS) {
        // Static interior slices avoid edge checks without padding the inputs.
        auto sx = tx.template slice<dynamic_extent, TOKEN_ROWS>(0, int(token_base));
        auto sw = tw.template slice<dynamic_extent, OUTPUT_COLS>(0, int(output_base));
        auto so = to.template slice<OUTPUT_COLS, TOKEN_ROWS>(int(output_base), int(token_base));
        op.run(sx, sw, so);
    } else {
        // Dynamic slices retain the original tensor bounds for partial tiles.
        auto sx = tx.slice(0, int(token_base));
        auto sw = tw.slice(0, int(output_base));
        auto so = to.slice(int(output_base), int(token_base));
        op.run(sx, sw, so);
    }
}

kernel void two_simd_64x128(device const half *x [[buffer(0)]],
                             device const half *weight [[buffer(1)]],
                             device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                             uint3 tgid [[threadgroup_position_in_grid]]) {
    two_simd_shape_impl<64, 128, false>(x, weight, out, nmk, tgid);
}

kernel void two_simd_64x128_accumulate(device const half *x [[buffer(0)]],
                                        device const half *weight [[buffer(1)]],
                                        device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                                        uint3 tgid [[threadgroup_position_in_grid]]) {
    two_simd_shape_impl<64, 128, true>(x, weight, out, nmk, tgid);
}
