#include "qwen-rope-memory.h"
#include "spans.h"
coax_t_QwResult psionic_qwen_rope(int64_t start,int64_t count,coax_v_f32 out,coax_v_f32 work){
    return qw_rope(start,count,out,work,psionic_f32_spans_overlap(out.data,out.count,work.data,work.count));
}
