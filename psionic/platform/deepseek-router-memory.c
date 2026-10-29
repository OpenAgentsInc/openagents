/* Address observations only. Coax owns scores, selection, weights and records. */
#include "deepseek-router-memory.h"
#include "spans.h"
coax_t_DrRoute psionic_dr_route(coax_t_DrConfig c,coax_v_f32 x,coax_v_f32 bias,
        coax_v_f64 scores,coax_v_f64 ranked) {
    _Bool alias=psionic_spans_overlap(scores.data,scores.count,sizeof(double),ranked.data,ranked.count,sizeof(double));
    alias |= psionic_spans_overlap(scores.data,scores.count,sizeof(double),x.data,x.count,sizeof(float));
    alias |= psionic_spans_overlap(scores.data,scores.count,sizeof(double),bias.data,bias.count,sizeof(float));
    alias |= psionic_spans_overlap(ranked.data,ranked.count,sizeof(double),x.data,x.count,sizeof(float));
    alias |= psionic_spans_overlap(ranked.data,ranked.count,sizeof(double),bias.data,bias.count,sizeof(float));
    return dr_route(c,x,bias,scores,ranked,alias);
}
