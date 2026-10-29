/* Native address observations only; Coax owns all grammar and token decisions. */
#include "harmony-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} Span;
#define SPAN(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touches(Span a,Span b){return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool observes(Span *w,int nw,Span *r,int nr){
    for(int i=0;i<nw;i++){for(int j=0;j<i;j++)if(touches(w[i],w[j]))return true;for(int j=0;j<nr;j++)if(touches(w[i],r[j]))return true;}return false;
}
coax_t_PtResult psionic_harmony_encode(coax_v_u8 table,coax_string text,coax_v_s64 out,coax_v_s64 stage,coax_v_s64 symbols){
    Span w[]={SPAN(out),SPAN(stage),SPAN(symbols)},r[]={SPAN(table),SPAN(text)};return hs_encode_checked(table,text,out,stage,symbols,observes(w,3,r,2));
}
coax_t_PtResult psionic_harmony_decode(coax_v_u8 table,coax_v_s64 tokens,coax_v_u8 out,coax_v_u8 stage){
    Span w[]={SPAN(out),SPAN(stage)},r[]={SPAN(table),SPAN(tokens)};return hs_decode_checked(table,tokens,out,stage,observes(w,2,r,2));
}
coax_t_PtResult psionic_harmony_system(int64_t effort,coax_v_u8 out,coax_v_u8 work){
    Span w[]={SPAN(out),SPAN(work)};return hm_system_checked(effort,out,work,observes(w,2,NULL,0));
}
coax_t_HmResult psionic_harmony_render(coax_v_u8 table,coax_v_t_HmMessage messages,bool completion,coax_v_s64 out,coax_v_s64 stage,coax_v_s64 symbols){
    Span w[]={SPAN(out),SPAN(stage),SPAN(symbols)},r[]={SPAN(table),SPAN(messages)};
    bool alias=observes(w,3,r,2);
    if(!hm_message_count(messages.count))alias=true;
    if(!alias)for(int64_t i=0;i<messages.count;i++){
        Span nested[]={SPAN(messages.data[i].name),SPAN(messages.data[i].recipient),SPAN(messages.data[i].body)};
        if(observes(w,3,nested,3)){alias=true;break;}
    }
    return hm_render_checked(table,messages,completion,out,stage,symbols,alias);
}
coax_t_HmResult psionic_harmony_parse(coax_v_u8 table,coax_v_s64 tokens,bool completion,coax_v_t_HmRecord out,coax_v_t_HmRecord draft,coax_v_u8 bytes,coax_v_u8 work){
    Span w[]={SPAN(out),SPAN(draft),SPAN(bytes),SPAN(work)},r[]={SPAN(table),SPAN(tokens)};
    return hm_parse_checked(table,tokens,completion,out,draft,bytes,work,observes(w,4,r,2));
}
