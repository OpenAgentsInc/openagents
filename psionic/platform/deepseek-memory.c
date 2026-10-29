/* Address observations only. Coax owns shapes, policy, arithmetic and publication. */
#include "deepseek-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} Span;
static bool hit(Span a,Span b){return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool writable(Span *w,size_t n,Span *r,size_t m){
    for(size_t i=0;i<n;i++){
        if(hit(w[i],(Span){0,0,1}))return true;
        for(size_t j=0;j<i;j++)if(hit(w[i],w[j]))return true;
        for(size_t j=0;j<m;j++)if(hit(w[i],r[j]))return true;
    }return false;
}
#define SP(v) (Span){(v).data,(v).count,sizeof(*(v).data)}
coax_t_DsResult psionic_ds_step(coax_t_DsModel m,int64_t t,int64_t pos,coax_v_f32 cache,coax_t_DsOutput out,coax_t_DsWork work){
    Span w[]={SP(cache),SP(out.logits),SP(out.trace),SP(work.floats),SP(work.scores)};
    Span r[4+12*28];size_t n=0;r[n++]=SP(m.blocks);
    for(size_t i=0;i<3;i++)r[n++]=SP(m.p.e[i]);
    bool overlap=m.blocks.count<0||m.blocks.count>28;
    if(!overlap)for(int64_t l=0;l<m.blocks.count;l++)for(size_t j=0;j<12;j++)r[n++]=SP(m.blocks.data[l].p.e[j]);
    return ds_step(m,t,pos,cache,out,work,overlap||writable(w,5,r,n));
}
coax_t_DsResult psionic_ds_bind(int64_t capacity,coax_v_t_DsFile files,coax_v_t_DsBlock blocks,coax_v_t_DsBlock draft,coax_v_t_DsModel out){
    Span w[]={SP(blocks),SP(draft),SP(out)};Span r[11];size_t n=0;r[n++]=SP(files);bool overlap=files.count<0||files.count>5;
    if(!overlap)for(int64_t i=0;i<files.count;i++){r[n++]=SP(files.data[i].name);r[n++]=SP(files.data[i].data);}
    return ds_bind_original(capacity,files,blocks,draft,out,overlap||writable(w,3,r,n));
}
coax_t_PtResult psionic_ds_chat(coax_v_u8 table,coax_v_t_DsMessage messages,coax_v_s64 out,coax_v_s64 stage,coax_v_s64 temp,coax_v_u8 normal,coax_v_s64 scalars,coax_v_s64 symbols){
    Span w[]={SP(out),SP(stage),SP(temp),SP(normal),SP(scalars),SP(symbols)};Span r[66];size_t n=0;r[n++]=SP(table);r[n++]=SP(messages);bool overlap=messages.count<0||messages.count>64;
    if(!overlap)for(int64_t i=0;i<messages.count;i++)r[n++]=SP(messages.data[i].content);
    return ds_chat(table,messages,out,stage,temp,normal,scalars,symbols,overlap||writable(w,6,r,n));
}

coax_t_PtResult psionic_ds_encode(coax_v_u8 table,coax_string input,coax_v_s64 out,coax_v_s64 stage,coax_v_u8 normal,coax_v_s64 scalars,coax_v_s64 symbols){
    Span w[]={SP(out),SP(stage),SP(normal),SP(scalars),SP(symbols)};Span r[]={SP(table),SP(input)};
    return ds_encode(table,input,out,stage,normal,scalars,symbols,writable(w,5,r,2));
}
coax_t_PtResult psionic_ds_decode(coax_v_u8 table,coax_v_s64 tokens,coax_v_u8 out,coax_v_u8 stage){
    Span w[]={SP(out),SP(stage)};Span r[]={SP(table),SP(tokens)};
    return ds_decode(table,tokens,out,stage,writable(w,2,r,2));
}
