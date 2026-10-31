/* Chapter 113 native evidence: the generated C library against the
   independent Python goldens, decode/full agreement, central differences for
   every parameter, a short training run and refusals. */
#include "decoder.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#include "decoder-goldens.h"
#include "spans.h"
static const coax_t_TdShape S={5,4,2,1,2,3,2,4};
static int64_t tokens[4]={1,3,0,2};
static double p[204],w[384],dw[384],cache[384],grads[204],logits[20];
static coax_v_f64 v(double *x,int64_t n){return (coax_v_f64){x,n};}
static coax_t_TdResult forward(void){coax_v_f64 a=v(w,384),b=v(logits,20);return td_forward(S,v(p,204),(coax_v_s64){tokens,4},a,b,psionic_spans_overlap(a.data,a.count,8,b.data,b.count,8));}
int main(void){
 for(int i=0;i<204;i++)p[i]=0.5*sin(0.7*i+0.3);
 coax_t_TdResult r=forward();assert(!r.status && r.written==20 && fabs(r.loss-DECODER_LOSS)<1e-12);
 double worst=0;for(int i=0;i<20;i++){double d=fabs(logits[i]-DECODER_LOGITS[i]);if(d>worst)worst=d;}assert(worst<1e-12);
 double full[20];memcpy(full,logits,sizeof full);double step[5];int64_t filled=0;
 for(int t=0;t<4;t++){r=td_decode(S,v(p,204),v(cache,384),filled,tokens[t],v(step,5),0);assert(!r.status && r.written==t+1);filled=r.written;assert(!memcmp(step,full+5*t,sizeof step));}
 r=td_gradients(S,v(p,204),(coax_v_s64){tokens,4},v(w,384),v(dw,384),v(grads,204),0);assert(!r.status && r.written==204);
 double gap=0,h=1e-5;for(int i=0;i<204;i++){double keep=p[i];p[i]=keep+h;double up=forward().loss;p[i]=keep-h;double down=forward().loss;p[i]=keep;double e=fabs(grads[i]-(up-down)/(2*h))/(1+fabs(grads[i]));if(e>gap)gap=e;}assert(gap<1e-8);
 double first=0,last=0;for(int i=0;i<204;i++)p[i]*=0.4;
 for(int s=0;s<60;s++){r=td_gradients(S,v(p,204),(coax_v_s64){tokens,4},v(w,384),v(dw,384),v(grads,204),0);assert(!r.status);if(!s)first=r.loss;last=r.loss;for(int i=0;i<204;i++)p[i]-=0.5*grads[i];}
 assert(first>1 && last<0.05*first);
 assert(td_forward(S,v(p,204),(coax_v_s64){tokens,4},v(w,384),v(w,20),psionic_spans_overlap(w,384,8,w,20,8)).status==COAX_TdStatus_alias);
 double saved=p[7];p[7]=NAN;assert(forward().status==COAX_TdStatus_number);p[7]=INFINITY;assert(forward().status==COAX_TdStatus_number);p[7]=saved;
 tokens[1]=5;assert(forward().status==COAX_TdStatus_token);tokens[1]=3;
 printf("decoder native: 20 logits within %.3g of Python, decode equals full, 204 central differences within %.3g, loss %.4g -> %.4g\n",worst,gap,first,last);
 return 0;}
