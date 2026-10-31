#include "adamw-memory.h"
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "adamw-oracle.h"
static double worst;
static void near(double a,double b){double d=fabs(a-b);assert(d<=1e-15*(1+fabs(b)));if(d>worst)worst=d;}
/* Runs one oracle case from its start, resuming into fresh buffers at cut. */
static void run(coax_t_AwGroup *g,int64_t ng,coax_t_AwCompute compute,int n,int steps,const double *start,const double *grads,const double *ps,const double *ms,const double *vs,const double *cs,int cut){
 double p[2][16],m[2][16],v[2][16];float c[16];memset(m,0,sizeof m);memset(v,0,sizeof v);memcpy(p[0],start,n*sizeof(double));int at=0;coax_t_AwState s={0,0};
 for(int k=0;k<steps;k++){
  if(k==cut){memcpy(p[1],p[0],sizeof p[0]);memcpy(m[1],m[0],sizeof m[0]);memcpy(v[1],v[0],sizeof v[0]);at=1;}
  double x[16];memcpy(x,grads+k*n,n*sizeof(double));
  coax_t_AwResult r=psionic_aw_step((coax_v_t_AwGroup){g,ng},(coax_t_AwPolicy){compute,COAX_AwOverflow_refuse},s,(coax_v_f64){p[at],n},(coax_v_f64){x,n},(coax_v_f64){m[at],n},(coax_v_f64){v[at],n},(coax_v_f32){c,cs?n:0});
  assert(r.status==COAX_AwStatus_ok && r.state.step==k+1 && r.written==n);s=r.state;
  for(int i=0;i<n;i++){near(p[at][i],ps[k*n+i]);near(m[at][i],ms[k*n+i]);near(v[at][i],vs[k*n+i]);if(cs)assert((double)c[i]==cs[k*n+i]);}
 }
}
int main(void){
 coax_t_AwGroup two[2]={{0,8,0.01,0.9,0.999,1e-8,0.1},{8,4,0.05,0.8,0.95,1e-6,0}},narrow[1]={{0,3,0.125,0.5,0.75,0.001,0.25}},one[1]={{0,1,0.3,0,0.5,0.01,0.5}};
 for(int cut=-1;cut<5;cut++)run(two,2,COAX_AwCompute_binary64,12,5,aw_matrix_start,aw_matrix_grads,aw_matrix_params,aw_matrix_m,aw_matrix_v,NULL,cut);
 run(narrow,1,COAX_AwCompute_binary32,3,3,aw_narrow_start,aw_narrow_grads,aw_narrow_params,aw_narrow_m,aw_narrow_v,aw_narrow_compute,-1);
 run(one,1,COAX_AwCompute_binary64,1,4,aw_scalar_start,aw_scalar_grads,aw_scalar_params,aw_scalar_m,aw_scalar_v,NULL,2);
 /* Actual aliases refuse and write nothing. */
 double p[4]={1,2,3,4},m[4]={0},v[4]={0},x[4]={1,1,1,1};float c[4]={-1,-1,-1,-1};coax_t_AwGroup g[1]={{0,2,0.5,0.5,0.5,0.5,0}};coax_t_AwState s={3,1};coax_t_AwPolicy W={COAX_AwCompute_binary64,COAX_AwOverflow_refuse};
 coax_t_AwResult r=psionic_aw_step((coax_v_t_AwGroup){g,1},W,s,(coax_v_f64){p,2},(coax_v_f64){p+1,2},(coax_v_f64){m,2},(coax_v_f64){v,2},(coax_v_f32){c,0});assert(r.status==COAX_AwStatus_alias && r.state.step==3);
 r=psionic_aw_step((coax_v_t_AwGroup){g,1},W,s,(coax_v_f64){p,2},(coax_v_f64){x,2},(coax_v_f64){m,2},(coax_v_f64){m+1,2},(coax_v_f32){c,0});assert(r.status==COAX_AwStatus_alias);
 r=psionic_aw_step((coax_v_t_AwGroup){g,1},(coax_t_AwPolicy){COAX_AwCompute_binary32,COAX_AwOverflow_refuse},s,(coax_v_f64){p,2},(coax_v_f64){x,2},(coax_v_f64){m,2},(coax_v_f64){v,2},(coax_v_f32){(float *)(p+1),2});assert(r.status==COAX_AwStatus_alias);
 assert(p[0]==1 && p[1]==2 && p[2]==3 && m[0]==0 && v[1]==0);
 /* Native NaN and infinity: refused, or skipped with only the skip counter moving. */
 uint64_t bits=UINT64_C(0x7ff8000000000000);memcpy(x+1,&bits,8);
 r=psionic_aw_step((coax_v_t_AwGroup){g,1},W,s,(coax_v_f64){p,2},(coax_v_f64){x,2},(coax_v_f64){m,2},(coax_v_f64){v,2},(coax_v_f32){c,0});assert(r.status==COAX_AwStatus_nonfinite && r.state.step==3 && r.state.skipped==1);
 x[1]=INFINITY;r=psionic_aw_step((coax_v_t_AwGroup){g,1},(coax_t_AwPolicy){COAX_AwCompute_binary64,COAX_AwOverflow_skip},s,(coax_v_f64){p,2},(coax_v_f64){x,2},(coax_v_f64){m,2},(coax_v_f64){v,2},(coax_v_f32){c,0});assert(r.status==COAX_AwStatus_skipped && r.state.step==3 && r.state.skipped==2);
 assert(p[0]==1 && p[1]==2 && m[0]==0 && m[1]==0 && v[0]==0 && v[1]==0 && c[0]==-1);
 printf("AdamW: 3 cases, 6 resume points, every step within 1e-15 of the independent reference (max %.3g); aliases and NaN refuse\n",worst);
 return 0;}
