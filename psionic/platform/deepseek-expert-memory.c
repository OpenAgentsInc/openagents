/* Native address observations only; Coax owns every format/math/policy decision. */
#include "deepseek-expert-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} DeSpan;
#define DE_SPAN(x) (DeSpan){(x).data,(x).count,sizeof(*(x).data)}
static _Bool de_hit(DeSpan a,DeSpan b) {return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static _Bool de_alias(DeSpan *write,size_t nw,DeSpan *read,size_t nr) {
    for (size_t i=0;i<nw;i++) {for (size_t j=0;j<i;j++) if (de_hit(write[i],write[j])) return 1;for (size_t j=0;j<nr;j++) if (de_hit(write[i],read[j])) return 1;}return 0;
}
static _Bool de_matrix_alias(DeSpan *write,size_t n,coax_t_DeMatrix m) {DeSpan read[]={DE_SPAN(m.data),DE_SPAN(m.scales)};return de_alias(write,n,read,2);}
static _Bool de_expert_alias(DeSpan *write,size_t n,coax_t_DeExpert e) {return de_matrix_alias(write,n,e.gate)||de_matrix_alias(write,n,e.up)||de_matrix_alias(write,n,e.down);}
coax_t_DeResult psionic_de_decode(coax_t_DeMatrix m,coax_v_f64 out,coax_v_f64 stage) {
    DeSpan write[]={DE_SPAN(out),DE_SPAN(stage)};return de_decode(m,out,stage,de_matrix_alias(write,2,m));
}
coax_t_DeResult psionic_de_quantize(coax_v_f32 x,coax_v_f64 out,coax_v_f64 stage) {
    DeSpan write[]={DE_SPAN(out),DE_SPAN(stage)},read[]={DE_SPAN(x)};return de_quantize(x,out,stage,de_alias(write,2,read,1));
}
coax_t_DeResult psionic_de_project(coax_t_DeMatrix m,coax_v_f32 x,coax_v_f32 out,coax_v_f64 quant,coax_v_f32 stage) {
    DeSpan write[]={DE_SPAN(out),DE_SPAN(quant),DE_SPAN(stage)},read[]={DE_SPAN(x)};return de_project(m,x,out,quant,stage,de_alias(write,3,read,1)||de_matrix_alias(write,3,m));
}
coax_t_DeResult psionic_de_activate(coax_v_f32 gate,coax_v_f32 up,float route,float limit,coax_v_f32 out,coax_v_f32 stage) {
    DeSpan write[]={DE_SPAN(out),DE_SPAN(stage)},read[]={DE_SPAN(gate),DE_SPAN(up)};return de_activate(gate,up,route,limit,out,stage,de_alias(write,2,read,2));
}
coax_t_DeResult psionic_de_expert(coax_t_DeExpert e,coax_v_f32 x,float route,float limit,coax_v_f32 out,coax_t_DeWork work) {
    DeSpan write[]={DE_SPAN(out),DE_SPAN(work.floats),DE_SPAN(work.wide)},read[]={DE_SPAN(x)};
    return de_expert(e,x,route,limit,out,work,de_alias(write,3,read,1)||de_expert_alias(write,3,e));
}
coax_t_DeResult psionic_de_mixture(coax_v_t_DeExpert e,coax_t_DeExpert shared,coax_v_f32 x,coax_v_s64 ids,coax_v_f32 weights,float limit,coax_v_f32 out,coax_t_DeWork work,coax_v_t_PsionicAccumulator acc) {
    DeSpan write[]={DE_SPAN(out),DE_SPAN(work.floats),DE_SPAN(work.wide),DE_SPAN(acc)},read[]={DE_SPAN(e),DE_SPAN(x),DE_SPAN(ids),DE_SPAN(weights)};
    _Bool alias=de_alias(write,4,read,4)||de_expert_alias(write,4,shared);for (int64_t i=0;i<e.count;i++) alias|=de_expert_alias(write,4,e.data[i]);
    return de_mixture(e,shared,x,ids,weights,limit,out,work,acc,alias);
}
