/* Address observations only; all numerical and admission behavior is Coax. */
#include "byte-lm-memory.h"
#include "spans.h"
coax_t_BgFit psionic_bg_fit(coax_v_u8 train,int64_t steps,double rate,coax_v_f64 w,coax_v_f64 stage,coax_v_f64 trace){
 const void *p[]={w.data,stage.data,trace.data};int64_t n[]={w.count,stage.count,trace.count};_Bool alias=0;
 for(int i=0;i<3;i++){alias|=psionic_spans_overlap(p[i],n[i],8,train.data,train.count,1);for(int j=0;j<i;j++)alias|=psionic_spans_overlap(p[i],n[i],8,p[j],n[j],8);}
 return bg_fit(train,steps,rate,w,stage,trace,alias);
}
coax_t_BgGeneration psionic_bg_generate(coax_v_f64 w,coax_t_CrSeed seed,uint8_t start,int64_t length,coax_v_u8 out){return bg_generate(w,seed,start,length,out,psionic_spans_overlap(w.data,w.count,8,out.data,out.count,1));}
