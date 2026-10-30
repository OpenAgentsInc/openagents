/* Native address observations only. All numerical and policy work is Coax. */
#include "deepseek-sparse-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} DaSpan;
#define DA_SPAN(x) (DaSpan){(x).data,(x).count,sizeof(*(x).data)}
static _Bool da_alias(DaSpan *write,size_t nw,DaSpan *read,size_t nr) {
    for(size_t i=0;i<nw;i++) {
        for(size_t j=0;j<i;j++)if(psionic_spans_overlap(write[i].data,write[i].count,write[i].width,write[j].data,write[j].count,write[j].width))return 1;
        for(size_t j=0;j<nr;j++)if(psionic_spans_overlap(write[i].data,write[i].count,write[i].width,read[j].data,read[j].count,read[j].width))return 1;
    }return 0;
}
coax_t_DaSelection psionic_da_select(coax_v_f32 scores,int64_t visible,coax_v_u8 mask,int64_t keep,coax_v_s64 out,coax_v_s64 heap) {
    DaSpan w[]={DA_SPAN(out),DA_SPAN(heap)},r[]={DA_SPAN(scores),DA_SPAN(mask)};return da_select(scores,visible,mask,keep,out,heap,da_alias(w,2,r,2));
}
coax_t_DaSelection psionic_da_candidates(coax_v_f32 scores,int64_t visible,int64_t block,int64_t keep,coax_v_u8 out,coax_v_u8 stage,coax_v_f32 blocks,coax_v_s64 heap) {
    DaSpan w[]={DA_SPAN(out),DA_SPAN(stage),DA_SPAN(blocks),DA_SPAN(heap)},r[]={DA_SPAN(scores)};return da_candidates(scores,visible,block,keep,out,stage,blocks,heap,da_alias(w,4,r,1));
}
coax_t_DkResult psionic_da_norm(coax_v_u16 input,coax_v_u16 weight,coax_v_u16 out,coax_v_u16 stage) {
    DaSpan w[]={DA_SPAN(out),DA_SPAN(stage)},r[]={DA_SPAN(input),DA_SPAN(weight)};return da_norm(input,weight,out,stage,da_alias(w,2,r,2));
}
coax_t_DaResult psionic_da_pool(int64_t ratio,int64_t position,coax_v_f32 kv,coax_v_f32 gate,coax_v_f32 old_kv,coax_v_f32 old_gate,coax_v_u16 norm,coax_v_u16 latent,coax_v_f32 next_kv,coax_v_f32 next_gate,coax_t_DaPoolWork work) {
    DaSpan w[]={DA_SPAN(latent),DA_SPAN(next_kv),DA_SPAN(next_gate),DA_SPAN(work.floats),DA_SPAN(work.bits)},r[]={DA_SPAN(kv),DA_SPAN(gate),DA_SPAN(old_kv),DA_SPAN(old_gate),DA_SPAN(norm)};
    return da_pool(ratio,position,kv,gate,old_kv,old_gate,norm,latent,next_kv,next_gate,work,da_alias(w,5,r,5));
}
coax_t_DaResult psionic_da_pool_chunk(int64_t ratio,int64_t position,int64_t chunk,int64_t width,coax_v_f32 kv,coax_v_f32 gate,coax_v_f32 old_kv,coax_v_f32 old_gate,coax_v_u16 norm,coax_v_u16 latent,coax_v_f32 next_kv,coax_v_f32 next_gate,coax_t_DaPoolWork work) {
    DaSpan w[]={DA_SPAN(latent),DA_SPAN(next_kv),DA_SPAN(next_gate),DA_SPAN(work.floats),DA_SPAN(work.bits)},r[]={DA_SPAN(kv),DA_SPAN(gate),DA_SPAN(old_kv),DA_SPAN(old_gate),DA_SPAN(norm)};
    return da_pool_chunk(ratio,position,chunk,width,kv,gate,old_kv,old_gate,norm,latent,next_kv,next_gate,work,da_alias(w,5,r,5));
}
coax_t_DkResult psionic_da_rotate(_Bool compressed,int64_t position,int64_t heads,int64_t width,int64_t rope,_Bool inverse,coax_v_u16 input,coax_v_u16 out,coax_v_u16 stage) {
    DaSpan w[]={DA_SPAN(out),DA_SPAN(stage)},r[]={DA_SPAN(input)};return da_rotate(compressed,position,heads,width,rope,inverse,input,out,stage,da_alias(w,2,r,1));
}
coax_t_DkResult psionic_da_store(uint8_t kind,int64_t rows,int64_t width,int64_t row,coax_v_u16 input,coax_t_DkPacked out,coax_v_u8 stage) {
    DaSpan w[]={DA_SPAN(out.data),DA_SPAN(out.scales),DA_SPAN(stage)},r[]={DA_SPAN(input)};return da_store(kind,rows,width,row,input,out,stage,da_alias(w,3,r,1));
}
coax_t_DkResult psionic_da_index(int64_t heads,int64_t width,int64_t rows,int64_t visible,coax_t_DkPacked query,coax_t_DkPacked keys,coax_v_u16 weights,coax_v_f32 out,coax_v_f32 stage) {
    DaSpan w[]={DA_SPAN(out),DA_SPAN(stage)},r[]={DA_SPAN(query.data),DA_SPAN(query.scales),DA_SPAN(keys.data),DA_SPAN(keys.scales),DA_SPAN(weights)};return da_index(heads,width,rows,visible,query,keys,weights,out,stage,da_alias(w,2,r,5));
}
coax_t_DkResult psionic_da_attend(coax_t_DaAttention shape,int64_t position,coax_v_u16 query,coax_t_DkPacked window,int64_t rows,coax_t_DkPacked compressed,coax_v_s64 ids,coax_v_f32 sink,coax_v_u16 out,coax_t_DaAttentionWork work) {
    DaSpan w[]={DA_SPAN(out),DA_SPAN(work.bits),DA_SPAN(work.scores)},r[]={DA_SPAN(query),DA_SPAN(window.data),DA_SPAN(window.scales),DA_SPAN(compressed.data),DA_SPAN(compressed.scales),DA_SPAN(ids),DA_SPAN(sink)};
    return da_attend(shape,position,query,window,rows,compressed,ids,sink,out,work,da_alias(w,3,r,7));
}
