/* Address observations only; all latent-attention/rotary equations are Coax. */
#include "deepseek-v2-math-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} MlSpan;
#define ML_SPAN(x) (MlSpan){(x).data,(x).count,sizeof(*(x).data)}
static _Bool ml_overlap(MlSpan a,MlSpan b) {return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static _Bool ml_writes(MlSpan *read,size_t nr,MlSpan *write,size_t nw) {
    for (size_t i=0;i<nw;i++) {
        for (size_t j=0;j<nr;j++) if (ml_overlap(write[i],read[j])) return 1;
        for (size_t j=0;j<i;j++) if (ml_overlap(write[i],write[j])) return 1;
    }return 0;
}
coax_t_MlResult psionic_ml_attention(coax_t_MlShape s,coax_t_MlMode mode,coax_v_u8 w,
        coax_v_f32 q,coax_v_f32 z,coax_v_f32 pe,coax_v_f32 history,int64_t pos,double scale,
        coax_v_f32 out,coax_v_f32 row,coax_v_f32 work,coax_v_f64 scores) {
    MlSpan reads[]={ML_SPAN(w),ML_SPAN(q),ML_SPAN(z),ML_SPAN(pe),ML_SPAN(history)};
    MlSpan writes[]={ML_SPAN(out),ML_SPAN(row),ML_SPAN(work),ML_SPAN(scores)};
    return ml_attention(s,mode,w,q,z,pe,history,pos,scale,out,row,work,scores,ml_writes(reads,5,writes,4));
}
coax_t_DvRotateResult psionic_dv_rotary(int64_t heads,int64_t nope,int64_t rope,
        coax_v_f32 q,coax_v_f32 k,int64_t pos,coax_v_f32 out,coax_v_f32 key,coax_v_f32 work) {
    MlSpan reads[]={ML_SPAN(q),ML_SPAN(k)},writes[]={ML_SPAN(out),ML_SPAN(key),ML_SPAN(work)};
    return dv_rotary(heads,nope,rope,q,k,pos,out,key,work,ml_writes(reads,2,writes,3));
}
coax_t_DsResult psionic_dv_yarn(int64_t rope,int64_t pos,coax_v_f32 out,coax_v_f32 stage) {
    MlSpan writes[]={ML_SPAN(out),ML_SPAN(stage)};return dv_yarn(rope,pos,out,stage,ml_writes(NULL,0,writes,2));
}
