/* Address observations only; all numerical and admission behavior is Coax.
   A written view (parameters, moments, compute copy) may overlap nothing. */
#include "adamw-memory.h"
#include "spans.h"
coax_t_AwResult psionic_aw_step(coax_v_t_AwGroup groups,coax_t_AwPolicy policy,coax_t_AwState state,coax_v_f64 params,coax_v_f64 grads,coax_v_f64 m,coax_v_f64 v,coax_v_f32 compute){
 const void *p[]={params.data,m.data,v.data,compute.data,grads.data,groups.data};int64_t n[]={params.count,m.count,v.count,compute.count,grads.count,groups.count};size_t w[]={8,8,8,4,8,sizeof(coax_t_AwGroup)};_Bool alias=0;
 for(int i=0;i<4;i++)for(int j=i+1;j<6;j++)alias|=psionic_spans_overlap(p[i],n[i],w[i],p[j],n[j],w[j]);
 return aw_step(groups,policy,state,params,grads,m,v,compute,alias);
}
