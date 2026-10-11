// Three independent 4096-element K partitions. Parent rows retain stride12288.
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

kernel void clef_split3_partial(device const half *x [[buffer(0)]],
    device const half *w [[buffer(1)]], device float *p0 [[buffer(2)]],
    device float *p1 [[buffer(3)]], device float *p2 [[buffer(4)]],
    constant uint3 &nmk [[buffer(5)]], uint3 physical [[threadgroup_position_in_grid]]) {
    if (nmk.z != 12288 || nmk.x == 0 || nmk.y == 0) return;
    const uint columns = (nmk.y + 127) / 128, rows = (nmk.x + 63) / 64;
    const uint tiles = rows * columns;
    const uint partition = physical.x / tiles, tile = physical.x % tiles;
    if (partition >= 3) return;
    const uint row = (tile / columns) * 64, column = (tile % columns) * 128;
    const uint k_start = partition * 4096;
    device float *partial = partition == 0 ? p0 : (partition == 1 ? p1 : p2);
    // Explicit finite K extent prevents a dynamic tail slice from reading later partitions.
    const auto xe = dextents<int32_t, 2>(4096, int(nmk.x));
    const auto we = dextents<int32_t, 2>(4096, int(nmk.y));
    const array<int32_t, 2> parent_strides = {1, 12288};
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tx(
        (device half *)x + k_start, xe, parent_strides);
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tw(
        (device half *)w + k_start, we, parent_strides);
    tensor<device float, dextents<int32_t, 2>, tensor_inline> to(
        partial, dextents<int32_t, 2>(int(nmk.y), int(nmk.x)));
    constexpr auto desc = matmul2d_descriptor(64, 128, static_cast<int>(dynamic_extent),
        false, true, false, matmul2d_descriptor::mode::multiply);
    matmul2d<desc, execution_simdgroups<4>> op;
    if (nmk.x - row >= 64 && nmk.y - column >= 128) {
        auto sx = tx.template slice<dynamic_extent, 64>(0, int(row));
        auto sw = tw.template slice<dynamic_extent, 128>(0, int(column));
        auto so = to.template slice<128, 64>(int(column), int(row));
        op.run(sx, sw, so);
    } else {
        auto sx = tx.slice(0, int(row));
        auto sw = tw.slice(0, int(column));
        auto so = to.slice(int(column), int(row));
        op.run(sx, sw, so);
    }
}
