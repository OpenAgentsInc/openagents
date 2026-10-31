/* Address observations only; all numerical and admission behavior is Coax.
   A written view (parameters, momentum, second moment, scratch) may overlap nothing. */
#include "muon-memory.h"
#include "spans.h"
coax_t_McResult psionic_mc_step(coax_v_t_McGroup groups,coax_t_McPolicy policy,coax_t_McState state,coax_v_f64 params,coax_v_f64 grads,coax_v_f64 m,coax_v_f64 second,coax_v_f64 work){
 const void *p[]={params.data,m.data,second.data,work.data,grads.data,groups.data};int64_t n[]={params.count,m.count,second.count,work.count,grads.count,groups.count};size_t w[]={8,8,8,8,8,sizeof(coax_t_McGroup)};_Bool alias=0;
 for(int i=0;i<4;i++)for(int j=i+1;j<6;j++)alias|=psionic_spans_overlap(p[i],n[i],w[i],p[j],n[j],w[j]);
 return mc_step(groups,policy,state,params,grads,m,second,work,alias);
}
