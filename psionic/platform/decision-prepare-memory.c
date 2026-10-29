#include "decision-prepare-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} Span;
#define SPAN(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touches(Span a,Span b){return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool observes(Span *w,size_t nw,Span *r,size_t nr){
    for(size_t i=0;i<nw;i++){for(size_t j=i+1;j<nw;j++)if(touches(w[i],w[j]))return true;for(size_t j=0;j<nr;j++)if(touches(w[i],r[j]))return true;}return false;
}
coax_t_PrResult psionic_prepare(coax_v_u8 table,coax_t_PtEntry state,coax_t_PtQuestion question,coax_v_t_PtOption options,coax_v_s64 out,coax_t_PrWorkspace work){
    if(!pt_question_count(question.kind,options.count))return ps_prepare(table,state,question,options,out,work,true);
    Span w[]={SPAN(out),SPAN(work.prompt),SPAN(work.render_work),SPAN(work.candidate),SPAN(work.full),SPAN(work.token_stage),SPAN(work.normal),SPAN(work.scalars),SPAN(work.symbols)};
    Span r[]={SPAN(table),SPAN(state.value),SPAN(question.instructions.value),SPAN(options)};
    bool alias=observes(w,9,r,4);
    if(!alias)for(int64_t i=0;i<options.count;i++){
        Span x[]={SPAN(options.data[i].name),SPAN(options.data[i].description.value)};
        if(observes(w,9,x,2)){alias=true;break;}
    }
    return ps_prepare(table,state,question,options,out,work,alias);
}
