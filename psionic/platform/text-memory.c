/* Address observations only: Coax owns all text, token and admission semantics. */
#include "text-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} Span;
#define SPAN(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touches(Span a,Span b) {return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool observes(Span *w,size_t nw,Span *r,size_t nr) {
    for(size_t i=0;i<nw;i++) {
        for(size_t j=i+1;j<nw;j++) if(touches(w[i],w[j])) return true;
        for(size_t j=0;j<nr;j++) if(touches(w[i],r[j])) return true;
    }return false;
}
coax_t_PtResult psionic_text_encode(coax_v_u8 table,coax_string input,
        coax_v_s64 output,coax_v_s64 stage,coax_v_u8 normal,coax_v_s64 scalars,coax_v_s64 symbols) {
    Span w[]={SPAN(output),SPAN(stage),SPAN(normal),SPAN(scalars),SPAN(symbols)},r[]={SPAN(table),SPAN(input)};
    return pt_encode_checked(table,input,output,stage,normal,scalars,symbols,observes(w,5,r,2));
}
coax_t_PtResult psionic_text_normalize(coax_v_u8 table,coax_string input,coax_v_u8 output,coax_v_s64 scalars) {
    Span w[]={SPAN(output),SPAN(scalars)},r[]={SPAN(table),SPAN(input)};
    return pt_normalize_checked(table,input,output,scalars,observes(w,2,r,2));
}
coax_t_PtPrompt psionic_text_render(coax_t_PtEntry state,coax_t_PtQuestion q,
        coax_v_t_PtOption options,coax_v_u8 output,coax_v_u8 scratch) {
    if(!pt_question_count(q.kind,options.count) || !pt_render_capacity(output.count,scratch.count)) return pt_render_checked(state,q,options,output,scratch,true);
    Span w[]={SPAN(output),SPAN(scratch)},r[]={SPAN(state.value),SPAN(q.instructions.value),SPAN(options)};
    bool overlap=observes(w,2,r,3);
    if(!overlap) for(int64_t i=0;i<options.count;i++) {
        Span nested[]={SPAN(options.data[i].name),SPAN(options.data[i].description.value)};
        overlap=overlap || observes(w,2,nested,2);
    }
    return pt_render_checked(state,q,options,output,scratch,overlap);
}
coax_t_PtSplit psionic_text_split(coax_v_s64 candidate,coax_v_s64 full,
        coax_v_s64 ends,coax_v_t_PtBranch output) {
    Span w[]={SPAN(output)},r[]={SPAN(candidate),SPAN(full),SPAN(ends)};
    return pt_split_checked(candidate,full,ends,output,observes(w,1,r,3));
}
