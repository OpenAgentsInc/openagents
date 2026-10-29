#include "qwen-full-layer-memory.h"
#include "spans.h"
static bool floats(coax_v_f32 a,coax_v_f32 b) {return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);}
static bool bytes(coax_v_f32 a,coax_v_u8 b) {return psionic_spans_overlap(a.data,a.count,sizeof(float),b.data,b.count,sizeof(uint8_t));}
coax_t_QhResult psionic_qwen_full_step(coax_t_QhShape s,coax_t_QhWeights w,coax_v_f32 x,coax_v_f32 rope,int64_t position,coax_v_f32 cache,coax_v_f32 out,coax_v_f32 work) {
    coax_v_f32 writable[]={cache,out,work};
    coax_v_f32 readable[]={x,rope,w.input_norm,w.post_norm,w.q_norm,w.k_norm};
    coax_v_u8 matrices[]={w.q,w.k,w.v,w.mixer_out,w.gate,w.up,w.down};bool alias=false;
    for(size_t i=0;i<3;i++) {
        for(size_t j=i+1;j<3;j++)alias=alias||floats(writable[i],writable[j]);
        for(size_t j=0;j<6;j++)alias=alias||floats(writable[i],readable[j]);
        for(size_t j=0;j<7;j++)alias=alias||bytes(writable[i],matrices[j]);
    }
    return qh_step(s,w,x,rope,position,cache,out,work,alias);
}
