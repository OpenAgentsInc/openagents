// Scratch-only resident-weight FFN experiment. No model or product integration.
// Distinct input and output allocations are required across all threadgroups.
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

template <uint COLS, typename X, typename W>
inline void pair_tile(thread X &x, thread W &gate, thread W &up,
                      device half *out, uint3 nmk, uint token_base, uint column_base) {
    constexpr auto desc = matmul2d_descriptor(
        64, COLS, static_cast<int>(dynamic_extent), false, true, false,
        matmul2d_descriptor::mode::multiply);
    matmul2d<desc, execution_simdgroups<4>> op;
    auto g = op.template get_destination_cooperative_tensor<X, W, float>();
    auto u = op.template get_destination_cooperative_tensor<X, W, float>();
    #pragma unroll
    for (uint16_t i = 0; i < g.get_capacity(); ++i) {
        if (g.is_valid_element(i)) { g[i] = 0.0f; u[i] = 0.0f; }
    }
    op.run(x, gate, g);
    op.run(x, up, u);
    #pragma unroll
    for (uint16_t i = 0; i < g.get_capacity(); ++i) {
        if (g.is_valid_element(i)) {
            const auto coordinate = g.get_multidimensional_index(i);
            const uint column = column_base + uint(coordinate[0]);
            const uint token = token_base + uint(coordinate[1]);
            if (column < nmk.y && token < nmk.x) {
                // Match the existing float SiLU and final half conversion.
                out[ulong(token) * nmk.y + column] = half((g[i] / (1.0f + exp(-g[i]))) * u[i]);
            }
        }
    }
}

template <uint COLS>
inline void pair_impl(device const half *x, device const half *gate, device const half *up,
                      device half *out, uint3 nmk, uint2 group) {
    const uint token_base = group.y * 64, column_base = group.x * COLS;
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tx(
        (device half *)x, dextents<int32_t, 2>(int(nmk.z), int(nmk.x)));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tg(
        (device half *)gate, dextents<int32_t, 2>(int(nmk.z), int(nmk.y)));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tu(
        (device half *)up, dextents<int32_t, 2>(int(nmk.z), int(nmk.y)));
    if (token_base < nmk.x && nmk.x - token_base >= 64 &&
        column_base < nmk.y && nmk.y - column_base >= COLS) {
        auto sx = tx.template slice<dynamic_extent, 64>(0, int(token_base));
        auto sg = tg.template slice<dynamic_extent, COLS>(0, int(column_base));
        auto su = tu.template slice<dynamic_extent, COLS>(0, int(column_base));
        pair_tile<COLS>(sx, sg, su, out, nmk, token_base, column_base);
    } else {
        auto sx = tx.slice(0, int(token_base));
        auto sg = tg.slice(0, int(column_base));
        auto su = tu.slice(0, int(column_base));
        pair_tile<COLS>(sx, sg, su, out, nmk, token_base, column_base);
    }
}

#define PAIR_KERNEL(NAME, COLS) \
kernel void NAME(device const half *x [[buffer(0)]], device const half *gate [[buffer(1)]], \
                 device const half *up [[buffer(2)]], device half *out [[buffer(3)]], \
                 constant uint3 &nmk [[buffer(4)]], uint2 group [[threadgroup_position_in_grid]]) { \
    pair_impl<COLS>(x, gate, up, out, nmk, group); \
}
PAIR_KERNEL(ffn_pair_64, 64)
PAIR_KERNEL(ffn_pair_128, 128)
