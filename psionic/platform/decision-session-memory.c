#include "decision-session-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} Span;
#define SPAN(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touches(Span a,Span b){return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool observes(Span *w,size_t nw,Span *r,size_t nr){
    for(size_t i=0;i<nw;i++){for(size_t j=i+1;j<nw;j++)if(touches(w[i],w[j]))return true;for(size_t j=0;j<nr;j++)if(touches(w[i],r[j]))return true;}return false;
}
coax_t_PsSession psionic_session_open(uint8_t kind,int64_t capacity,coax_v_u8 config,coax_v_u8 tokenizer,coax_v_u8 readout,coax_v_u8 index,coax_v_t_PaFile files,coax_v_t_P5Spec specs,coax_v_u8 table,coax_t_PsWorkspace memory){
    coax_t_PsTarget target=ps_target(kind,capacity);
    if(target.status!=COAX_PsStatus_ok || files.count!=target.shards || specs.count!=target.tensors)
        return ps_open(kind,capacity,config,tokenizer,readout,index,files,specs,table,memory,true);
    Span w[]={SPAN(memory.inventory),SPAN(memory.draft),SPAN(memory.layers),SPAN(memory.params),SPAN(memory.stage),SPAN(memory.layer_work),SPAN(memory.norm),SPAN(memory.norm_work)};
    Span r[]={SPAN(config),SPAN(tokenizer),SPAN(readout),SPAN(index),SPAN(files),SPAN(specs),SPAN(table)};
    bool alias=observes(w,8,r,7);
    if(!alias)for(int64_t i=0;i<files.count;i++){Span x[]={SPAN(files.data[i].data)};if(observes(w,8,x,1)){alias=true;break;}}
    if(!alias)for(int64_t i=0;i<specs.count;i++){Span x[]={SPAN(specs.data[i].name)};if(observes(w,8,x,1)){alias=true;break;}}
    return ps_open(kind,capacity,config,tokenizer,readout,index,files,specs,table,memory,alias);
}
