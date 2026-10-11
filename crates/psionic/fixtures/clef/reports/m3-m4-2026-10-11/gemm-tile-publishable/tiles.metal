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
                      uint3 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_impl<false>(x, w, out, nmk, tgid.xy);
}

kernel void clef_gemm_accumulate(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]],
                                 device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                                 uint3 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_impl<true>(x, w, out, nmk, tgid.xy);
}

template <uint TOKEN_ROWS, uint OUTPUT_COLS, bool ACCUMULATE, uint GROUPS>
inline void clef_gemm_shape_impl(device const half *x, device const half *weight,
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
    matmul2d<desc, execution_simdgroups<GROUPS>> op;
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


kernel void tile_64_128_4(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]], device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_shape_impl<64, 128, false, 4>(x,w,out,nmk,tgid);
}

kernel void tile_128_128_4(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]], device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_shape_impl<128, 128, false, 4>(x,w,out,nmk,tgid);
}

kernel void tile_64_128_8(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]], device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_shape_impl<64, 128, false, 8>(x,w,out,nmk,tgid);
}

kernel void tile_128_128_8(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]], device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_shape_impl<128, 128, false, 8>(x,w,out,nmk,tgid);
}

kernel void tile_128_64_8(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]], device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_shape_impl<128, 64, false, 8>(x,w,out,nmk,tgid);
}

kernel void tile_128_256_8(device const half *x [[buffer(0)]], device const half *w [[buffer(1)]], device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]], uint3 tgid [[threadgroup_position_in_grid]]) {
    clef_gemm_shape_impl<128, 256, false, 8>(x,w,out,nmk,tgid);
}

kernel void probe_silu(device const float *gate [[buffer(0)]], device const float *up [[buffer(1)]], device half *out [[buffer(2)]], constant uint &count [[buffer(3)]], uint i [[thread_position_in_grid]]) {
    if (i < count) { float g = gate[i]; out[i] = half(g / (1.0f + exp(-g)) * up[i]); }
}
