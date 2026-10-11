#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

// Exactly the current Q4_K fma-to-half weight arithmetic.
inline half q4_weight(device const uchar *src, ulong i) {
    device const uchar *b = src + (i / 256) * 144;
    uint e = uint(i % 256), j = e / 64, r = e % 64, s = 2 * j + (r >= 32);
    float d = float(as_type<half>(ushort(ushort(b[0]) | (ushort(b[1]) << 8))));
    float dm = float(as_type<half>(ushort(ushort(b[2]) | (ushort(b[3]) << 8))));
    device const uchar *scales = b + 4;
    uchar sc = s < 4 ? (scales[s] & 63) : ((scales[s + 4] & 15) | ((scales[s - 4] >> 6) << 4));
    uchar mn = s < 4 ? (scales[s + 4] & 63) : ((scales[s + 4] >> 4) | ((scales[s] >> 6) << 4));
    uchar packed = b[16 + 32 * j + r % 32];
    uint q = r >= 32 ? packed >> 4 : packed & 15;
    return half(fma(d * float(sc), float(q), -(dm * float(mn))));
}

kernel void verify_decode(device const uchar *q4 [[buffer(0)]], device half *out [[buffer(1)]],
                          constant uint &count [[buffer(2)]], uint i [[thread_position_in_grid]]) {
    if (i < count) out[i] = q4_weight(q4, i);
}

// K=256 allows the packed decoder to consume complete superblocks. One 32 KiB
// threadgroup weight tile is reused; no full expanded-weight allocation is used.
// The expanded-f16 variant has the identical K loop and separates changed
// reduction order from any decoder error. Host requires K % 256 == 0.
template<bool DECODE>
inline void tiled_product(device const half *x, device const uchar *q4,
                          device const half *expanded, device float *out,
                          uint3 nmk, uint2 group, uint tid, threadgroup half *tile) {
    const int n = int(nmk.x), m = int(nmk.y), k = int(nmk.z);
    const int nr = int(group.y) * 64, mc = int(group.x) * 64;
    tensor<device half, dextents<int, 2>, tensor_inline> tx((device half *)x, dextents<int, 2>(k,n));
    tensor<threadgroup half, extents<int,256,64>, tensor_inline> tw(tile, extents<int,256,64>());
    tensor<device float, dextents<int, 2>, tensor_inline> to(out, dextents<int,2>(m,n));
    constexpr auto desc = matmul2d_descriptor(64,64,256,false,true,false,matmul2d_descriptor::mode::multiply_accumulate);
    matmul2d<desc, execution_simdgroups<4>> op;
    auto first = tx.slice(0,nr);
    auto result = op.get_destination_cooperative_tensor<decltype(first),decltype(tw),float>();
    #pragma unroll full
    for (uint16_t i=0;i<result.get_capacity();++i) if(result.is_valid_element(i)) result[i]=0.0f;
    for(int k0=0;k0<k;k0+=256) {
        for(uint at=tid;at<64*256;at+=128) {
            int row=mc+int(at/256), col=k0+int(at%256);
            tile[at] = row < m ? (DECODE ? q4_weight(q4,ulong(row)*k+col) : expanded[ulong(row)*k+col]) : half(0);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        auto a = tx.slice(k0,nr);
        op.run(a,tw,result);
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    result.store(to.slice(mc,nr));
}

kernel void q4_tile_f16(device const half *x [[buffer(0)]], device const uchar *q4 [[buffer(1)]],
                        device const half *expanded [[buffer(2)]], device float *out [[buffer(3)]],
                        constant uint3 &nmk [[buffer(4)]], uint2 group [[threadgroup_position_in_grid]],
                        uint tid [[thread_position_in_threadgroup]]) {
    threadgroup half tile[64*256]; tiled_product<true>(x,q4,expanded,out,nmk,group,tid,tile);
}
kernel void expanded_tile_f16(device const half *x [[buffer(0)]], device const uchar *q4 [[buffer(1)]],
                              device const half *expanded [[buffer(2)]], device float *out [[buffer(3)]],
                              constant uint3 &nmk [[buffer(4)]], uint2 group [[threadgroup_position_in_grid]],
                              uint tid [[thread_position_in_threadgroup]]) {
    threadgroup half tile[64*256]; tiled_product<false>(x,q4,expanded,out,nmk,group,tid,tile);
}

// Activation sums are a separate timed dispatch. One SIMD group per group of
// 32 K values; every value is an existing f16 activation widened exactly to f32.
kernel void activation_sums(device const half *x [[buffer(0)]], device float *sums [[buffer(1)]],
                            constant uint2 &nk [[buffer(2)]], uint group [[threadgroup_position_in_grid]],
                            uint lane [[thread_index_in_simdgroup]]) {
    if(group < nk.x*(nk.y/32)) {
        float total=simd_sum(float(x[ulong(group)*32+lane]));
        if(lane==0) sums[group]=total;
    }
}

// Q4_K represented as raw uint4 plus f32 per-32-value scale and offset planes.
// This does NOT reproduce per-weight f16 rounding. It is a numerical candidate,
// with every output compared against the frozen expanded-f16 implementation.
// Native API support does not establish a faster hardware instruction path.
kernel void native_uint4_affine(device const half *x [[buffer(0)]], device const uchar *q [[buffer(1)]],
                                device const float *scales [[buffer(2)]], device const float *offsets [[buffer(3)]],
                                device const float *sums [[buffer(4)]], device float *out [[buffer(5)]],
                                constant uint3 &nmk [[buffer(6)]], uint2 group [[threadgroup_position_in_grid]]) {
    const int n=int(nmk.x),m=int(nmk.y),k=int(nmk.z),nr=int(group.y)*64,mc=int(group.x)*64,groups=k/32;
    tensor<device half,dextents<int,2>,tensor_inline> tx((device half *)x,dextents<int,2>(k,n));
    tensor<device uint4b_format,dextents<int,2>,tensor_inline> tq((device uchar *)q,dextents<int,2>(k,m));
    tensor<device float,dextents<int,2>,tensor_inline> to(out,dextents<int,2>(m,n));
    constexpr auto desc=matmul2d_descriptor(64,64,32,false,true,false,matmul2d_descriptor::mode::multiply);
    matmul2d<desc,execution_simdgroups<4>> op;
    auto a0=tx.slice(0,nr);auto b0=tq.slice(0,mc);
    auto partial=op.get_destination_cooperative_tensor<decltype(a0),decltype(b0),float>();
    auto result=op.get_destination_cooperative_tensor<decltype(a0),decltype(b0),float>();
    #pragma unroll full
    for(uint16_t i=0;i<result.get_capacity();++i)if(result.is_valid_element(i))result[i]=0.0f;
    for(int g=0;g<groups;++g){
        auto a=tx.slice(g*32,nr);auto b=tq.slice(g*32,mc);op.run(a,b,partial);
        #pragma unroll full
        for(uint16_t i=0;i<result.get_capacity();++i)if(result.is_valid_element(i)){
            auto xy=result.get_multidimensional_index(i);int row=nr+int(xy[1]),col=mc+int(xy[0]);
            if(row<n&&col<m){ulong at=ulong(col)*groups+g;
                float term=fma(scales[at],partial[i],offsets[at]*sums[ulong(row)*groups+g]);
                result[i]+=term;
            }
        }
    }
    result.store(to.slice(mc,nr));
}

// Raw uint4 capability and throughput control. Host uses scale=1/offset=0
// fixtures for accuracy checks; arbitrary real Q4_K nibbles are timing-only.
kernel void native_uint4_raw(device const half *x [[buffer(0)]],device const uchar *q [[buffer(1)]],
                             device float *out [[buffer(2)]],constant uint3 &nmk [[buffer(3)]],
                             uint2 group [[threadgroup_position_in_grid]]){
    tensor<device half,dextents<int,2>,tensor_inline> a((device half *)x,dextents<int,2>(nmk.z,nmk.x));
    tensor<device uint4b_format,dextents<int,2>,tensor_inline> b((device uchar *)q,dextents<int,2>(nmk.z,nmk.y));
    tensor<device float,dextents<int,2>,tensor_inline> c(out,dextents<int,2>(nmk.y,nmk.x));
    constexpr auto desc=matmul2d_descriptor(64,64,static_cast<int>(dynamic_extent),false,true,false);
    matmul2d<desc,execution_simdgroups<4>> op;
    auto aa=a.slice(0,int(group.y)*64);auto bb=b.slice(0,int(group.x)*64);auto cc=c.slice(int(group.x)*64,int(group.y)*64);
    op.run(aa,bb,cc);
}

kernel void probe_add_residual(device float *out [[buffer(0)]],device const float *residual [[buffer(1)]],
                               constant uint &count [[buffer(2)]],uint i [[thread_position_in_grid]]){
    if(i<count)out[i]+=residual[i];
}
