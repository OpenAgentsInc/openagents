#include "qwen-prefix-memory.h"
#include "spans.h"
static bool overlap(coax_v_f32 a,coax_v_f32 b) {return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);}
coax_t_QpResult psionic_qwen_fork(coax_t_QpShape s,coax_a32_u8 id,coax_t_QpClaim claim,coax_v_f32 src,coax_v_f32 dst,coax_v_f32 work) {
    return qp_fork(s,id,claim,src,dst,work,overlap(src,dst)||overlap(src,work)||overlap(dst,work));
}
