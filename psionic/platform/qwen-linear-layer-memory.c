#include "qwen-linear-layer-memory.h"
#include "spans.h"
static bool floats(coax_v_f32 a,coax_v_f32 b) {return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);}
static bool bytes(coax_v_f32 a,coax_v_u8 b) {return psionic_spans_overlap(a.data,a.count,sizeof(float),b.data,b.count,sizeof(uint8_t));}
coax_t_QlResult psionic_qwen_linear_step(coax_t_QlShape s,coax_t_QlWeights w,coax_v_f32 x,coax_v_f32 history,coax_v_f32 state,coax_v_f32 out,coax_v_f32 work) {
    coax_v_f32 writable[]={history,state,out,work};
    coax_v_f32 readable[]={x,w.input_norm,w.post_norm,w.conv,w.a_log,w.dt,w.delta_norm};
    coax_v_u8 matrices[]={w.qkv,w.z,w.a,w.b,w.mixer_out,w.gate,w.up,w.down};
    bool alias=false;
    for(size_t i=0;i<4;i++) {
        for(size_t j=i+1;j<4;j++)alias=alias||floats(writable[i],writable[j]);
        for(size_t j=0;j<7;j++)alias=alias||floats(writable[i],readable[j]);
        for(size_t j=0;j<8;j++)alias=alias||bytes(writable[i],matrices[j]);
    }
    return ql_step(s,w,x,history,state,out,work,alias);
}
