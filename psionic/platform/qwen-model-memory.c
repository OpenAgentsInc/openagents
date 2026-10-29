#include "qwen-model-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} Span;
#define SPAN(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touches(Span a,Span b) {return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool observes(Span *w,size_t nw,Span *r,size_t nr) {
    for(size_t i=0;i<nw;i++) {for(size_t j=i+1;j<nw;j++)if(touches(w[i],w[j]))return true;for(size_t j=0;j<nr;j++)if(touches(w[i],r[j]))return true;}return false;
}
coax_t_QeResult psionic_qwen_model_block(coax_t_QeShape shape,coax_v_t_QeLayer layers,coax_v_f32 input,coax_v_f32 rope,int64_t start,int64_t count,coax_v_f32 state,coax_v_f32 out,coax_v_f32 work) {
    Span w[]={SPAN(state),SPAN(out),SPAN(work)},r[]={SPAN(layers),SPAN(input),SPAN(rope)};
    bool alias=observes(w,3,r,3);
    /* No nested observations until the descriptor count is bounded. */
    if(layers.count<0 || layers.count>64) return qe_block(shape,layers,input,rope,start,count,state,out,work,true);
    if(!alias) for(int64_t i=0;i<layers.count;i++) {
        coax_t_QeLayer l=layers.data[i];
        if(l.kind==PSIONIC_QE_LINEAR) {
            coax_t_QlWeights v=l.linear;
            Span read[]={SPAN(v.input_norm),SPAN(v.post_norm),SPAN(v.conv),SPAN(v.a_log),SPAN(v.dt),SPAN(v.delta_norm),SPAN(v.qkv),SPAN(v.z),SPAN(v.a),SPAN(v.b),SPAN(v.mixer_out),SPAN(v.gate),SPAN(v.up),SPAN(v.down)};
            alias=observes(w,3,read,sizeof(read)/sizeof(*read));
        } else if(l.kind==PSIONIC_QE_FULL) {
            coax_t_QhWeights v=l.full;
            Span read[]={SPAN(v.input_norm),SPAN(v.post_norm),SPAN(v.q_norm),SPAN(v.k_norm),SPAN(v.q),SPAN(v.k),SPAN(v.v),SPAN(v.mixer_out),SPAN(v.gate),SPAN(v.up),SPAN(v.down)};
            alias=observes(w,3,read,sizeof(read)/sizeof(*read));
        }
        if(alias)break;
    }
    return qe_block(shape,layers,input,rope,start,count,state,out,work,alias);
}
