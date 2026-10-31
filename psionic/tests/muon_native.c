#include "muon-memory.h"
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "muon-oracle.h"
static double worst;
static void near(double a,double b){double d=fabs(a-b)/(1+fabs(b));assert(d<=1e-12);if(d>worst)worst=d;}
static int load(const long long *gi,const double *gs,int count,coax_t_McGroup *out){
 for(int i=0;i<count;i++){out[i]=(coax_t_McGroup){gi[6*i]?COAX_McKind_muon:COAX_McKind_adamw,gi[6*i+1],gi[6*i+2],gi[6*i+3],gi[6*i+4],gs[6*i],gs[6*i+1],gs[6*i+2],gs[6*i+3],gs[6*i+4],gs[6*i+5],gi[6*i+5]};}
 return count;}
/* Runs one oracle case from its start, resuming into fresh buffers at cut. */
static void run(coax_t_McGroup *g,int ng,int n,int sn,int steps,const double *start,const double *grads,const double *ps,const double *ms,const double *ss,int cut){
 static double p[2][320],m[2][320],s[2][160],w[256];memset(m,0,sizeof m);memset(s,0,sizeof s);memcpy(p[0],start,n*sizeof(double));int at=0;coax_t_McState st={0,0};
 for(int k=0;k<steps;k++){
  if(k==cut){memcpy(p[1],p[0],sizeof p[0]);memcpy(m[1],m[0],sizeof m[0]);memcpy(s[1],s[0],sizeof s[0]);at=1;}
  double x[320];memcpy(x,grads+k*n,n*sizeof(double));
  coax_t_McResult r=psionic_mc_step((coax_v_t_McGroup){g,ng},(coax_t_McPolicy){COAX_McOverflow_refuse},st,(coax_v_f64){p[at],n},(coax_v_f64){x,n},(coax_v_f64){m[at],n},(coax_v_f64){s[at],sn},(coax_v_f64){w,256});
  assert(r.status==COAX_McStatus_ok && r.state.step==k+1 && r.written==n);st=r.state;
  for(int i=0;i<n;i++){near(p[at][i],ps[k*n+i]);near(m[at][i],ms[k*n+i]);assert(x[i]==grads[k*n+i]);}
  for(int i=0;i<sn;i++)near(s[at][i],ss[k*sn+i]);
 }
}
int main(void){
 coax_t_McGroup a[16],b[16];int na=load(mc_nanochat_groups,mc_nanochat_settings,mc_nanochat_groups_count,a),nb=load(mc_muon_groups,mc_muon_settings,mc_muon_groups_count,b);
 for(int cut=-1;cut<mc_nanochat_steps;cut++)run(a,na,mc_nanochat_n,mc_nanochat_second,mc_nanochat_steps,mc_nanochat_start,mc_nanochat_grads,mc_nanochat_params,mc_nanochat_m,mc_nanochat_s,cut);
 for(int cut=-1;cut<mc_muon_steps;cut++)run(b,nb,mc_muon_n,mc_muon_second,mc_muon_steps,mc_muon_start,mc_muon_grads,mc_muon_params,mc_muon_m,mc_muon_s,cut);
 /* Actual aliases refuse and write nothing. */
 coax_t_McGroup g[2]={{COAX_McKind_adamw,0,2,0,0,0.5,0.5,0.5,0.5,0.5,0,0},{COAX_McKind_muon,2,4,2,2,0.5,0,0.5,0,0.5,0.5,5}};
 double p[8]={1,2,1,-1,2,0.5,0,0},m[6]={0},s[4]={0},x[6]={1,-1,0.5,1,-2,3},w[24];coax_t_McState st={3,1};coax_t_McPolicy W={COAX_McOverflow_refuse},S={COAX_McOverflow_skip};
 coax_t_McResult r=psionic_mc_step((coax_v_t_McGroup){g,2},W,st,(coax_v_f64){p,6},(coax_v_f64){p+1,6},(coax_v_f64){m,6},(coax_v_f64){s,4},(coax_v_f64){w,24});assert(r.status==COAX_McStatus_alias && r.state.step==3);
 r=psionic_mc_step((coax_v_t_McGroup){g,2},W,st,(coax_v_f64){p,6},(coax_v_f64){x,6},(coax_v_f64){m,6},(coax_v_f64){m+2,4},(coax_v_f64){w,24});assert(r.status==COAX_McStatus_alias);
 r=psionic_mc_step((coax_v_t_McGroup){g,2},W,st,(coax_v_f64){p,6},(coax_v_f64){x,6},(coax_v_f64){m,6},(coax_v_f64){s,4},(coax_v_f64){p+5,24});assert(r.status==COAX_McStatus_alias);
 r=psionic_mc_step((coax_v_t_McGroup){g,2},W,st,(coax_v_f64){p,6},(coax_v_f64){x,6},(coax_v_f64){m,6},(coax_v_f64){s,4},(coax_v_f64){x,24});assert(r.status==COAX_McStatus_alias);
 assert(p[0]==1 && p[1]==2 && p[5]==0.5 && m[0]==0 && s[1]==0);
 /* Native NaN and infinity: refused, or skipped with only the skip counter moving. */
 uint64_t bits=UINT64_C(0x7ff8000000000000);memcpy(x+4,&bits,8);
 r=psionic_mc_step((coax_v_t_McGroup){g,2},W,st,(coax_v_f64){p,6},(coax_v_f64){x,6},(coax_v_f64){m,6},(coax_v_f64){s,4},(coax_v_f64){w,24});assert(r.status==COAX_McStatus_nonfinite && r.state.step==3 && r.state.skipped==1);
 x[4]=INFINITY;r=psionic_mc_step((coax_v_t_McGroup){g,2},S,st,(coax_v_f64){p,6},(coax_v_f64){x,6},(coax_v_f64){m,6},(coax_v_f64){s,4},(coax_v_f64){w,24});assert(r.status==COAX_McStatus_skipped && r.state.step==3 && r.state.skipped==2);
 /* A Muon overflow past binary64 in the norm is refused before any sqrt. */
 x[4]=1e300;x[3]=-1e300;r=psionic_mc_step((coax_v_t_McGroup){g,2},W,st,(coax_v_f64){p,6},(coax_v_f64){x,6},(coax_v_f64){m,6},(coax_v_f64){s,4},(coax_v_f64){w,24});assert(r.status==COAX_McStatus_overflow);
 assert(p[0]==1 && p[1]==2 && p[2]==1 && p[5]==0.5 && m[0]==0 && m[5]==0 && s[0]==0 && s[3]==0);
 printf("MuonAdamW: 2 cases, %d resume points, every step within 1e-12 of the independent reference (max %.3g); aliases, NaN and overflow refuse\n",mc_nanochat_steps+mc_muon_steps+2,worst);
 return 0;}
