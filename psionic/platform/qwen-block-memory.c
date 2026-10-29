#include "qwen-block-memory.h"
#include "spans.h"
static bool floats(coax_v_f32 a,coax_v_f32 b) {return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);}
static bool bytes(coax_v_f32 a,coax_v_u8 b) {return psionic_spans_overlap(a.data,a.count,sizeof(float),b.data,b.count,sizeof(uint8_t));}
coax_t_QmResult psionic_qwen_begin(int64_t width,float eps,coax_v_f32 x,coax_v_f32 scale,coax_v_f32 out,coax_v_f32 work) {
    bool alias=floats(out,work)||floats(out,x)||floats(out,scale)||floats(work,x)||floats(work,scale);
    return qb_begin(width,eps,x,scale,out,work,alias);
}
coax_t_QmResult psionic_qwen_finish(coax_t_QmDtype type,int64_t width,int64_t hidden,float eps,coax_v_f32 x,coax_v_f32 att,coax_v_f32 scale,coax_v_u8 gate,coax_v_u8 up,coax_v_u8 down,coax_v_f32 out,coax_v_f32 work) {
    bool alias=floats(out,work)||floats(out,x)||floats(out,att)||floats(out,scale)||floats(work,x)||floats(work,att)||floats(work,scale)||bytes(out,gate)||bytes(out,up)||bytes(out,down)||bytes(work,gate)||bytes(work,up)||bytes(work,down);
    return qb_finish(type,width,hidden,eps,x,att,scale,gate,up,down,out,work,alias);
}
