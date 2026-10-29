/* Observe nested native spans; admission, routing and arithmetic stay in Coax. */
#include "moe-memory.h"
#include "spans.h"
#define OVERLAPS(v) psionic_spans_overlap(out.data,out.count,sizeof(float), \
    (v).data,(v).count,sizeof(*(v).data))

coax_t_MoResult psionic_moe_forward(coax_v_f32 logits, int64_t k,
        coax_v_f32 x, coax_v_t_MoExpert experts, int64_t budget, coax_v_f32 out) {
    bool alias=OVERLAPS(logits)||OVERLAPS(x)||OVERLAPS(experts);
    /* Bound traversal of nested metadata; Coax chooses the shape refusal. */
    if(experts.count<0 || experts.count>128) alias=true;
    if(!alias)for(int64_t i=0;i<experts.count;i++) {
        if(OVERLAPS(experts.data[i].matrix)||OVERLAPS(experts.data[i].bias)){alias=true;break;}
    }
    return mo_forward(logits,k,x,experts,budget,out,alias);
}
