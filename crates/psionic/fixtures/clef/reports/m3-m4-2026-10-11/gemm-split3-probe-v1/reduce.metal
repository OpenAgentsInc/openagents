// Compile this file with -fno-fast-math to retain the written f32 addition order.
#include <metal_stdlib>
using namespace metal;
kernel void clef_split3_reduce(device const float *p0 [[buffer(0)]],
    device const float *p1 [[buffer(1)]], device const float *p2 [[buffer(2)]],
    device float *out [[buffer(3)]], constant uint2 &args [[buffer(4)]],
    uint gid [[thread_position_in_grid]]) {
    if (gid >= args.x) return;
    float result = p0[gid] + p1[gid];
    result = result + p2[gid];
    if (args.y != 0) result = result + out[gid];
    out[gid] = result;
}
