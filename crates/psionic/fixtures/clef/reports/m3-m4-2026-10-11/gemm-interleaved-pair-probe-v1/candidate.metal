// One full-K product pairs 128 gate rows with their corresponding 128 up rows.
// The separate SiLU preserves f32 gate/up values until the existing half cast.
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

// args = [tokens, logical FFN columns, hidden K, packed columns]. The host packs
// each 128-column block as gate[128], up[128], with zero weights for tail rows.
kernel void interleaved_gate_up(device const half *x [[buffer(0)]],
                                device const half *weight [[buffer(1)]],
                                device float *out [[buffer(2)]],
                                constant uint4 &args [[buffer(3)]],
                                uint3 group [[threadgroup_position_in_grid]],
                                uint3 threads [[threads_per_threadgroup]]) {
    constexpr uint imax = 0x7fffffff;
    if (threads.x != 128 || threads.y != 1 || threads.z != 1 ||
        args.x == 0 || args.y == 0 || args.z == 0 || args.x > imax ||
        args.y > (imax - 255) / 2 || args.z > imax ||
        args.w != ((args.y + 127) / 128) * 256 ||
        group.x >= args.w / 256 || group.y >= (args.x + 63) / 64) return;
    const uint row = group.y * 64, column = group.x * 256;
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tx(
        (device half *)x, dextents<int32_t, 2>(int(args.z), int(args.x)));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tw(
        (device half *)weight, dextents<int32_t, 2>(int(args.z), int(args.w)));
    tensor<device float, dextents<int32_t, 2>, tensor_inline> to(
        out, dextents<int32_t, 2>(int(args.w), int(args.x)));
    constexpr auto descriptor = matmul2d_descriptor(
        64, 256, static_cast<int>(dynamic_extent), false, true, false,
        matmul2d_descriptor::mode::multiply);
    matmul2d<descriptor, execution_simdgroups<4>> op;
    if (args.x - row >= 64) {
        auto sx = tx.template slice<dynamic_extent, 64>(0, int(row));
        auto sw = tw.template slice<dynamic_extent, 256>(0, int(column));
        auto so = to.template slice<256, 64>(int(column), int(row));
        op.run(sx, sw, so);
    } else {
        auto sx = tx.slice(0, int(row));
        auto sw = tw.slice(0, int(column));
        auto so = to.slice(int(column), int(row));
        op.run(sx, sw, so);
    }
}

kernel void interleaved_silu(device const float *paired [[buffer(0)]],
                             device half *out [[buffer(1)]],
                             constant uint4 &args [[buffer(2)]],
                             uint gid [[thread_position_in_grid]]) {
    const ulong count = ulong(args.x) * args.y;
    if (ulong(gid) >= count) return;
    const uint row = gid / args.y, column = gid % args.y;
    const ulong gate = ulong(row) * args.w + (column / 128) * 256 + column % 128;
    const float g = paired[gate];
    out[gid] = half((g / (1.0f + exp(-g))) * paired[gate + 128]);
}
