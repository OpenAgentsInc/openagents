/* Address observations only. All stream/mix equations and publication are Coax. */
#include "deepseek-hyper-memory.h"
#include "spans.h"
static _Bool dh_overlap(coax_v_f32 a,coax_v_f32 b) {return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);}
static _Bool dh_alias(coax_v_f32 a,coax_v_f32 b,coax_v_f32 out,coax_v_f32 stage) {
    return dh_overlap(out,stage)||dh_overlap(out,a)||dh_overlap(out,b)||dh_overlap(stage,a)||dh_overlap(stage,b);
}
coax_t_DsResult psionic_dh_collapse(int64_t width,coax_v_f32 x,coax_t_DhCarry previous,coax_v_f32 out,coax_v_f32 stage) {
    return dh_collapse(width,x,previous,out,stage,dh_alias(x,(coax_v_f32){0},out,stage));
}
coax_t_DhMixResult psionic_dh_prepare(int64_t width,coax_v_f32 x,coax_t_DhCarry previous,
        coax_t_DhWeights w,coax_v_f32 out,coax_v_f32 stage) {
    return dh_prepare(width,x,previous,w,out,stage,dh_alias(x,w.projection,out,stage));
}
coax_t_DhFinish psionic_dh_finish(int64_t width,coax_v_f32 residual,coax_v_f32 value,
        coax_t_DhMix mix,coax_v_f32 out,coax_v_f32 stage) {
    return dh_finish(width,residual,value,mix,out,stage,dh_alias(residual,value,out,stage));
}
