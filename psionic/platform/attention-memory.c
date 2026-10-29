#include "attention-memory.h"
#include "spans.h"
static bool overlap(coax_v_f32 a, coax_v_f32 b) {
    return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);
}
coax_t_PaResult psionic_attention_step(coax_t_PaShape s, coax_v_f32 w, coax_v_f32 rope,
        coax_v_f32 x, int64_t position, coax_v_f32 cache, coax_v_f32 out) {
    bool alias=overlap(cache,out)||overlap(cache,w)||overlap(cache,rope)||overlap(cache,x)||
        overlap(out,w)||overlap(out,rope)||overlap(out,x);
    return pa_step(s,w,rope,x,position,cache,out,alias);
}
coax_t_PaResult psionic_attention_forward(coax_t_PaShape s, coax_v_f32 w, coax_v_f32 rope,
        coax_v_f32 x, int64_t steps, coax_v_f32 out) {
    bool alias=overlap(out,w)||overlap(out,rope)||overlap(out,x);
    return pa_forward(s,w,rope,x,steps,out,alias);
}
