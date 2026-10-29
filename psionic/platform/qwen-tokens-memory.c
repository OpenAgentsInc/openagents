#include "qwen-tokens-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} Span;
#define SPAN(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touches(Span a,Span b) {return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool observes(Span *w,size_t nw,Span *r,size_t nr) {
    for(size_t i=0;i<nw;i++){for(size_t j=i+1;j<nw;j++)if(touches(w[i],w[j]))return true;for(size_t j=0;j<nr;j++)if(touches(w[i],r[j]))return true;}return false;
}
coax_t_QxResult psionic_qwen_tokens(coax_t_QxModel model,coax_v_s64 tokens,coax_v_f32 rope,int64_t start,coax_v_f32 state,coax_t_QrKind kind,float temperature,coax_v_s64 slots,coax_v_f64 out,coax_v_f64 prob,coax_v_f32 work){
    Span w[]={SPAN(state),SPAN(out),SPAN(prob),SPAN(work)},r[]={SPAN(tokens),SPAN(rope),SPAN(slots),SPAN(model.layers),SPAN(model.embedding),SPAN(model.head),SPAN(model.final_norm)};
    bool alias=observes(w,4,r,7);
    if(model.layers.count<0 || model.layers.count>64)return qx_block(model,tokens,rope,start,state,kind,temperature,slots,out,prob,work,true);
    if(!alias) for(int64_t i=0;i<model.layers.count;i++){
        coax_t_QeLayer l=model.layers.data[i];
        if(l.kind==0){
            coax_t_QlWeights v=l.linear;Span nested[]={SPAN(v.input_norm),SPAN(v.post_norm),SPAN(v.conv),SPAN(v.a_log),SPAN(v.dt),SPAN(v.delta_norm),SPAN(v.qkv),SPAN(v.z),SPAN(v.a),SPAN(v.b),SPAN(v.mixer_out),SPAN(v.gate),SPAN(v.up),SPAN(v.down)};
            alias=observes(w,4,nested,sizeof(nested)/sizeof(*nested));
        }else if(l.kind==1){
            coax_t_QhWeights v=l.full;Span nested[]={SPAN(v.input_norm),SPAN(v.post_norm),SPAN(v.q_norm),SPAN(v.k_norm),SPAN(v.q),SPAN(v.k),SPAN(v.v),SPAN(v.mixer_out),SPAN(v.gate),SPAN(v.up),SPAN(v.down)};
            alias=observes(w,4,nested,sizeof(nested)/sizeof(*nested));
        }
        if(alias)break;
    }
    return qx_block(model,tokens,rope,start,state,kind,temperature,slots,out,prob,work,alias);
}
