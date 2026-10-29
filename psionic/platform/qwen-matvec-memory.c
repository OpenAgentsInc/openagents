#include "qwen-matvec-memory.h"
#include "spans.h"
static bool floats(coax_v_f32 a,coax_v_f32 b) {return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);}
static bool bytes(coax_v_f32 a,coax_v_u8 b) {return psionic_spans_overlap(a.data,a.count,sizeof(float),b.data,b.count,sizeof(uint8_t));}
coax_t_QmResult psionic_qwen_project(coax_t_QmDtype type,int64_t rows,int64_t cols,coax_v_u8 w,coax_v_f32 x,coax_v_f32 out,coax_v_f32 work) {
    bool alias=floats(out,work)||floats(out,x)||floats(work,x)||bytes(out,w)||bytes(work,w);
    return qm_project(type,rows,cols,w,x,out,work,alias);
}
