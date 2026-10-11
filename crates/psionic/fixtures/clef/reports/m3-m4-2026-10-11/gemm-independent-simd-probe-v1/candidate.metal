// Scratch projection: four independent SIMD groups own disjoint 32 by 32
// output tiles. Inputs remain device half; each f32 accumulator spans all K.
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

template <bool ACCUMULATE>
inline void independent_product(device const half *x, device const half *weight,
                                device float *out, uint3 nmk, uint3 group,
                                uint simd_group, uint3 threads) {
    // Uniform guards precede every barrier. The host applies tighter bounds.
    constexpr uint int_max = 0x7fffffff;
    if (threads.x != 128 || threads.y != 1 || threads.z != 1 ||
        nmk.x == 0 || nmk.y == 0 || nmk.z == 0 ||
        nmk.x > int_max || nmk.y > int_max || nmk.z > int_max ||
        group.x >= (nmk.y + 63) / 64 || group.y >= (nmk.x + 63) / 64) return;
    const uint row = group.y * 64 + (simd_group / 2) * 32;
    const uint col = group.x * 64 + (simd_group % 2) * 32;
    const bool active = row < nmk.x && col < nmk.y;
    const int n = int(nmk.x), m = int(nmk.y), k = int(nmk.z);
    // Parent views only determine the cooperative tensor type. They are valid
    // even when this SIMD group's output subtile lies outside a matrix tail.
    tensor<device half, dextents<int, 2>, tensor_inline> parent_x(
        (device half *)x, dextents<int, 2>(k, n));
    tensor<device half, dextents<int, 2>, tensor_inline> parent_w(
        (device half *)weight, dextents<int, 2>(k, m));
    constexpr auto desc = matmul2d_descriptor(
        32, 32, 128, false, true, false, matmul2d_descriptor::mode::multiply_accumulate);
    matmul2d<desc, execution_simdgroup> op;
    auto result = op.get_destination_cooperative_tensor<decltype(parent_x), decltype(parent_w), float>();
    #pragma clang loop unroll(full)
    for (uint16_t i = 0; i < result.get_capacity(); ++i)
        if (result.is_valid_element(i)) result[i] = 0.0f;
    if (ACCUMULATE && active) {
        tensor<device float, dextents<int, 2>, tensor_inline> residual(
            out + ulong(row) * nmk.y + col,
            dextents<int, 2>(int(min(32u, nmk.y - col)), int(min(32u, nmk.x - row))),
            array<int, 2>{1, m});
        result.load(residual);
    }
    for (uint k0 = 0; k0 < nmk.z; k0 += 128) {
        if (active) {
            const uint kc = min(128u, nmk.z - k0);
            const uint nr = min(32u, nmk.x - row), mc = min(32u, nmk.y - col);
            // Explicit K extents cannot extend into a later reduction segment.
            // Original row strides preserve the noncontiguous parent layout.
            tensor<device half, dextents<int, 2>, tensor_inline> tx(
                (device half *)(x + ulong(row) * nmk.z + k0),
                dextents<int, 2>(int(kc), int(nr)), array<int, 2>{1, k});
            tensor<device half, dextents<int, 2>, tensor_inline> tw(
                (device half *)(weight + ulong(col) * nmk.z + k0),
                dextents<int, 2>(int(kc), int(mc)), array<int, 2>{1, k});
            if (kc == 128 && nr == 32 && mc == 32) {
                auto sx = tx.template slice<128, 32>(0, 0);
                auto sw = tw.template slice<128, 32>(0, 0);
                op.run(sx, sw, result);
            } else {
                op.run(tx, tw, result);
            }
        }
        // Inactive tail SIMD groups also reach this scheduling barrier.
        threadgroup_barrier(mem_flags::mem_none);
    }
    if (active) {
        tensor<device float, dextents<int, 2>, tensor_inline> target(
            out + ulong(row) * nmk.y + col,
            dextents<int, 2>(int(min(32u, nmk.y - col)), int(min(32u, nmk.x - row))),
            array<int, 2>{1, m});
        result.store(target);
    }
}

kernel void independent_simd(device const half *x [[buffer(0)]],
                             device const half *weight [[buffer(1)]],
                             device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                             uint3 group [[threadgroup_position_in_grid]],
                             uint simd_group [[simdgroup_index_in_threadgroup]],
                             uint3 threads [[threads_per_threadgroup]]) {
    independent_product<false>(x, weight, out, nmk, group, simd_group, threads);
}
kernel void independent_simd_accumulate(device const half *x [[buffer(0)]],
                                        device const half *weight [[buffer(1)]],
                                        device float *out [[buffer(2)]], constant uint3 &nmk [[buffer(3)]],
                                        uint3 group [[threadgroup_position_in_grid]],
                                        uint simd_group [[simdgroup_index_in_threadgroup]],
                                        uint3 threads [[threads_per_threadgroup]]) {
    independent_product<true>(x, weight, out, nmk, group, simd_group, threads);
}
