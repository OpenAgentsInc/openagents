// Experimental full-K specialization and persistent weight layout. Scratch only.
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

template <int STATIC_K, bool PRETRANSPOSED>
inline void projection(device const half *x, device const half *w, device float *out, uint3 nmk, uint3 group) {
    const int n=int(nmk.x), m=int(nmk.y), k=int(nmk.z);
    const int row=int(group.y)*64, col=int(group.x)*128;
    tensor<device half,dextents<int32_t,2>,tensor_inline> tx((device half*)x,dextents<int32_t,2>(k,n));
    tensor<device half,dextents<int32_t,2>,tensor_inline> tw((device half*)w,dextents<int32_t,2>(PRETRANSPOSED?m:k,PRETRANSPOSED?k:m));
    tensor<device float,dextents<int32_t,2>,tensor_inline> to(out,dextents<int32_t,2>(m,n));
    constexpr auto descriptor=matmul2d_descriptor(64,128,STATIC_K,false,!PRETRANSPOSED,false,matmul2d_descriptor::mode::multiply);
    matmul2d<descriptor,execution_simdgroups<4>> op;
    if(row+63<n && col+127<m) {
        auto sx=tx.template slice<dynamic_extent,64>(0,row);
        auto so=to.template slice<128,64>(col,row);
        if constexpr(PRETRANSPOSED) {
            auto sw=tw.template slice<128,dynamic_extent>(col,0);
            op.run(sx,sw,so);
        } else {
            auto sw=tw.template slice<dynamic_extent,128>(0,col);
            op.run(sx,sw,so);
        }
    } else {
        auto sx=tx.slice(0,row);auto so=to.slice(col,row);
        if constexpr(PRETRANSPOSED) {auto sw=tw.slice(col,0);op.run(sx,sw,so);}
        else {auto sw=tw.slice(0,col);op.run(sx,sw,so);}
    }
}
kernel void clef_gemm(device const half*x [[buffer(0)]],device const half*w [[buffer(1)]],device float*out [[buffer(2)]],constant uint3&dims [[buffer(3)]],uint3 group [[threadgroup_position_in_grid]]) {projection<static_cast<int>(dynamic_extent),false>(x,w,out,dims,group);}
kernel void static_gemm(device const half*x [[buffer(0)]],device const half*w [[buffer(1)]],device float*out [[buffer(2)]],constant uint3&dims [[buffer(3)]],uint3 group [[threadgroup_position_in_grid]]) {
    if(dims.z==4096) projection<4096,false>(x,w,out,dims,group);
    else if(dims.z==12288) projection<12288,false>(x,w,out,dims,group);
}
kernel void nn_gemm(device const half*x [[buffer(0)]],device const half*w [[buffer(1)]],device float*out [[buffer(2)]],constant uint3&dims [[buffer(3)]],uint3 group [[threadgroup_position_in_grid]]) {projection<static_cast<int>(dynamic_extent),true>(x,w,out,dims,group);}
kernel void probe_silu(device const float *gate [[buffer(0)]],device const float *up [[buffer(1)]],device half *out [[buffer(2)]],constant uint &count [[buffer(3)]],uint i [[thread_position_in_grid]]) {
    if(i<count){float g=gate[i];out[i]=half(g/(1.0f+exp(-g))*up[i]);}
}
