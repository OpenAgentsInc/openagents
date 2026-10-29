/* Native range observation only. Model admission, arithmetic and state are Coax. */
#include "gptoss-memory.h"
#include "spans.h"
typedef struct{const void *data;int64_t count;size_t width;} Span;
#define S(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touches(Span a,Span b){return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool observes(Span *w,int nw,Span *r,int nr){for(int i=0;i<nw;i++){for(int j=0;j<i;j++)if(touches(w[i],w[j]))return true;for(int j=0;j<nr;j++)if(touches(w[i],r[j]))return true;}return false;}
coax_t_GStep psionic_gptoss_forward(coax_t_GModel m,int64_t token,int64_t position,int64_t budget,coax_v_f32 cache,coax_t_GOutput out,coax_t_GWork work){
    Span w[]={S(cache),S(out.logits),S(out.trace),S(out.routes),S(out.weights),S(work.floats),S(work.scores),S(work.sums),S(work.routes)};
    Span r[]={S(m.blocks),S(m.p.e[0]),S(m.p.e[1]),S(m.p.e[2])};bool alias=observes(w,9,r,4);
    if(m.blocks.count<0 || m.blocks.count>36)alias=true;
    if(!alias)for(int64_t l=0;l<m.blocks.count;l++)for(int j=0;j<15;j++){Span p[]={S(m.blocks.data[l].p.e[j])};if(observes(w,9,p,1))alias=true;}
    return g_forward(m,token,position,budget,cache,out,work,alias);
}
coax_t_GResult psionic_gptoss_bind(uint8_t target,int64_t capacity,coax_v_t_GFile files,coax_v_t_GBlock blocks,coax_v_t_GBlock draft,coax_v_t_GModel out){
    Span w[]={S(blocks),S(draft),S(out)},r[]={S(files)};bool alias=observes(w,3,r,1);
    if(files.count<0 || files.count>16)alias=true;
    if(!alias)for(int64_t i=0;i<files.count;i++){Span p[]={S(files.data[i].name),S(files.data[i].data)};if(observes(w,3,p,2))alias=true;}
    return g_bind_original(target,capacity,files,blocks,draft,out,alias);
}
coax_t_GEmission psionic_gptoss_emit(coax_v_f32 logits,coax_v_u8 mask,coax_t_GSampling options,coax_t_GGeneration state,bool cancel,coax_v_f64 probabilities,coax_t_GSampleWork work){
    Span w[]={S(probabilities),S(work.stage),S(work.order),S(work.temporary)},r[]={S(logits),S(mask)};
    return gg_emit(logits,mask,options,state,cancel,probabilities,work,observes(w,4,r,2));
}
coax_t_GResult psionic_gptoss_mask(coax_v_string alternatives,coax_string prefix,coax_v_string pieces,coax_v_u8 out,coax_v_u8 stage){
    Span w[]={S(out),S(stage)},r[]={S(alternatives),S(prefix),S(pieces)};bool alias=observes(w,2,r,3);
    if(alternatives.count<0 || alternatives.count>16 || pieces.count<0 || pieces.count>201088)alias=true;
    if(!alias){for(int64_t i=0;i<alternatives.count;i++){Span p[]={S(alternatives.data[i])};if(observes(w,2,p,1))alias=true;}for(int64_t i=0;i<pieces.count;i++){Span p[]={S(pieces.data[i])};if(observes(w,2,p,1))alias=true;}}
    return gt_mask(alternatives,prefix,pieces,out,stage,alias);
}
coax_t_GResult psionic_gptoss_display(coax_t_HmRecord record,coax_string bytes,coax_v_u8 out){Span a=S(bytes),b=S(out);return gt_display(record,bytes,out,touches(a,b));}
coax_t_GSpecBlock psionic_gptoss_block(int64_t vocab,coax_v_f64 target,coax_v_f64 draft,coax_v_s64 proposals,coax_v_f64 uniforms,bool greedy,coax_v_s64 out,coax_v_s64 stage,coax_v_f64 residual,coax_v_f64 scratch){
    Span w[]={S(out),S(stage),S(residual),S(scratch)},r[]={S(target),S(draft),S(proposals),S(uniforms)};
    return gx_block(vocab,target,draft,proposals,uniforms,greedy,out,stage,residual,scratch,observes(w,4,r,4));
}
coax_t_GResult psionic_gptoss_snapshot(coax_v_f32 input,coax_v_f32 out){Span a=S(input),b=S(out);return gx_snapshot(input,out,touches(a,b));}
