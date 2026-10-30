/* Effects only: observe actual borrowed read/write spans. */
#include "deepseek-front-v41-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} DvfSpan;
#define DVF_SPAN(v) (DvfSpan){(v).data,(v).count,sizeof(*(v).data)}
static _Bool overlap(DvfSpan a,DvfSpan b){return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static _Bool writes(DvfSpan *read,size_t nr,DvfSpan *write,size_t nw){
    for(size_t i=0;i<nw;i++){for(size_t j=0;j<nr;j++)if(overlap(write[i],read[j]))return 1;for(size_t j=0;j<i;j++)if(overlap(write[i],write[j]))return 1;}return 0;
}
coax_t_PtResult psionic_dvf_text(coax_v_u8 table,coax_string text,coax_v_s64 out,coax_v_s64 stage,coax_v_s64 symbols){
    DvfSpan read[]={DVF_SPAN(table),{text.data,text.count,1}},write[]={DVF_SPAN(out),DVF_SPAN(stage),DVF_SPAN(symbols)};
    return dvf_text(table,text,out,stage,symbols,writes(read,2,write,3));
}
coax_t_PtResult psionic_dvf_encode(coax_v_u8 table,coax_v_t_DfSegment segments,coax_v_s64 out,coax_v_s64 stage,coax_v_s64 temp,coax_v_u8 raw,coax_v_s64 symbols){
    DvfSpan read[]={DVF_SPAN(table),DVF_SPAN(segments)},write[]={DVF_SPAN(out),DVF_SPAN(stage),DVF_SPAN(temp),DVF_SPAN(raw),DVF_SPAN(symbols)};_Bool alias=writes(read,2,write,5);
    for(int64_t i=0;i<segments.count;i++){DvfSpan text={segments.data[i].text.data,segments.data[i].text.count,1};for(size_t j=0;j<5;j++)alias|=overlap(text,write[j]);}
    return dvf_encode(table,segments,out,stage,temp,raw,symbols,alias);
}
coax_t_PtResult psionic_dvf_units(coax_v_u8 table,coax_v_s64 ids,coax_v_s64 out,coax_v_s64 stage){
    DvfSpan read[]={DVF_SPAN(table),DVF_SPAN(ids)},write[]={DVF_SPAN(out),DVF_SPAN(stage)};return dvf_units(table,ids,out,stage,writes(read,2,write,2));
}
