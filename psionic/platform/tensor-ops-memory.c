/* Only native range observations; no tensor policy, arithmetic or gradients. */
#include "tensor-ops-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} ToSpan;
#define S(v) (ToSpan){(v).data,(v).count,sizeof *(v).data}
static bool observed(const ToSpan *reads,size_t nr,const ToSpan *writes,size_t nw){
    for(size_t i=0;i<nw;i++){
        for(size_t j=0;j<nr;j++)if(psionic_spans_overlap(writes[i].data,writes[i].count,writes[i].width,reads[j].data,reads[j].count,reads[j].width))return true;
        for(size_t j=0;j<i;j++)if(psionic_spans_overlap(writes[i].data,writes[i].count,writes[i].width,writes[j].data,writes[j].count,writes[j].width))return true;
    }return false;
}
coax_t_ToResult to_forward(coax_t_ToRequest r,coax_v_t_TvStorage owners,coax_v_t_TvView views,coax_v_f32 a,coax_v_f32 b,coax_v_s64 indices,coax_v_f32 out,coax_v_f32 scratch,coax_v_s64 aux,coax_v_s64 aux_stage){
    ToSpan reads[]={S(owners),S(views),S(a),S(b),S(indices)},writes[]={S(out),S(scratch),S(aux),S(aux_stage)};
    return to_forward_checked(r,owners,views,a,b,indices,out,scratch,aux,aux_stage,observed(reads,5,writes,4));
}
coax_t_ToGradient to_vjp(coax_t_ToRequest r,coax_v_t_TvStorage owners,coax_v_t_TvView views,coax_v_f32 a,coax_v_f32 b,coax_v_s64 indices,coax_v_f32 dy,coax_v_f32 ga,coax_v_f32 gb,coax_v_f32 sa,coax_v_f32 sb){
    ToSpan reads[]={S(owners),S(views),S(a),S(b),S(indices),S(dy)},writes[]={S(ga),S(gb),S(sa),S(sb)};
    return to_vjp_checked(r,owners,views,a,b,indices,dy,ga,gb,sa,sb,observed(reads,6,writes,4));
}
