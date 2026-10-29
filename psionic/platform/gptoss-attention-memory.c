/* Address observation only. Shapes, arithmetic and publication are Coax. */
#include "gptoss-attention-memory.h"
#include "spans.h"
static bool conflict(coax_v_f32 *reads,int nr,coax_v_f32 *writes,int nw){
    bool alias=false;
    for(int i=0;i<nw;i++){
        for(int j=0;j<nr;j++)alias|=psionic_f32_spans_overlap(writes[i].data,writes[i].count,reads[j].data,reads[j].count);
        for(int j=0;j<i;j++)alias|=psionic_f32_spans_overlap(writes[i].data,writes[i].count,writes[j].data,writes[j].count);
    }
    return alias;
}
coax_t_GaResult psionic_gptoss_attention(coax_t_GaShape s,int64_t p,
        coax_v_f32 q,coax_v_f32 k,coax_v_f32 v,coax_v_f32 sinks,
        coax_v_f32 cache,coax_v_f32 out,coax_v_f32 work){
    coax_v_f32 reads[]={q,k,v,sinks},writes[]={cache,out,work};
    return ga_step(s,p,q,k,v,sinks,cache,out,work,conflict(reads,4,writes,3));
}
coax_t_GoResult psionic_gptoss_rotate(int64_t h,coax_v_f32 x,coax_v_f32 c,coax_v_f32 out,coax_v_f32 work){
    coax_v_f32 reads[]={x,c},writes[]={out,work};
    return go_rotate(h,x,c,out,work,conflict(reads,2,writes,2));
}
coax_t_GoResult psionic_gptoss_swiglu(coax_v_f32 x,coax_v_f32 out){
    return go_swiglu(x,out,conflict(&x,1,&out,1));
}
coax_t_GoResult psionic_gptoss_project(int64_t r,int64_t c,coax_v_f32 a,coax_v_f32 x,coax_v_f32 b,coax_v_f32 out,coax_v_f32 work){
    coax_v_f32 reads[]={a,x,b},writes[]={out,work};
    return go_project(r,c,a,x,b,out,work,conflict(reads,3,writes,2));
}
