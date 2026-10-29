/* Only address-range observations. Coax decides policy and moves values. */
#include "tensor-view-memory.h"
#include "spans.h"
static bool fv_overlap(coax_v_f32 a,coax_v_f32 b){return psionic_f32_spans_overlap(a.data,a.count,b.data,b.count);}
static bool owner_overlap(coax_v_t_TvStorage a,coax_v_f32 b){return psionic_spans_overlap(a.data,a.count,sizeof *a.data,b.data,b.count,sizeof *b.data);}
coax_t_TvResult tv_gather(coax_v_t_TvStorage owner,coax_t_TvView view,coax_v_f32 data,coax_v_f32 out,coax_v_f32 scratch){
    bool overlap=fv_overlap(out,data) || fv_overlap(scratch,data) || fv_overlap(out,scratch) || owner_overlap(owner,out) || owner_overlap(owner,scratch);
    return tv_gather_checked(owner,view,data,out,scratch,overlap);
}
coax_t_TvResult tv_scatter(coax_v_t_TvStorage owner,coax_t_TvView view,coax_v_f32 source,coax_v_f32 data,coax_v_f32 scratch){
    bool overlap=fv_overlap(data,source) || fv_overlap(scratch,source) || fv_overlap(data,scratch) || owner_overlap(owner,source) || owner_overlap(owner,data) || owner_overlap(owner,scratch);
    return tv_scatter_checked(owner,view,source,data,scratch,overlap);
}
