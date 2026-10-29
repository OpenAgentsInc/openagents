#include "delta-memory.h"
#include "spans.h"
static bool overlap(coax_v_f32 a,coax_v_f32 b) {
    return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);
}
coax_t_PdResult psionic_delta_step(coax_t_PdShape s,coax_v_f32 input,coax_v_f32 history,
        coax_v_f32 state,coax_v_f32 out,coax_v_f32 work) {
    bool alias=overlap(history,state)||overlap(history,out)||overlap(history,work)||
        overlap(state,out)||overlap(state,work)||overlap(out,work)||
        overlap(input,history)||overlap(input,state)||overlap(input,out)||overlap(input,work);
    return pd_step(s,input,history,state,out,work,alias);
}
