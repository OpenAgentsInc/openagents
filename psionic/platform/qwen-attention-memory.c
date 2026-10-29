#include "qwen-attention-memory.h"
#include "spans.h"
static bool overlap(coax_v_f32 a,coax_v_f32 b) {return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);}
static bool mask_overlap(coax_v_f32 a,coax_v_u8 b) {return psionic_spans_overlap(a.data,a.count,sizeof(float),b.data,b.count,sizeof(uint8_t));}
coax_t_QfResult psionic_qwen_prepare(coax_t_QfShape s,coax_v_f32 input,coax_v_f32 output) {
    return qf_prepare(s,input,output,overlap(input,output));
}
coax_t_QfResult psionic_qwen_attend(coax_t_QfShape s,int64_t n,coax_v_f32 input,coax_v_u8 mask,coax_v_f32 output,coax_v_f32 scratch) {
    bool alias=overlap(output,scratch)||overlap(output,input)||overlap(scratch,input)||mask_overlap(output,mask)||mask_overlap(scratch,mask);
    return qf_attend(s,n,input,mask,output,scratch,alias);
}
