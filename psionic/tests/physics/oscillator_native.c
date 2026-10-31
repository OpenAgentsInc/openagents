/* Firm reason2: independent analytic solver oracle and C ABI exercise, never shipped. */
#include "oscillator.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static long cases;
#define CHECK(x) do {++cases;if(!(x)){fprintf(stderr,"line %d: %s\n",__LINE__,#x);exit(1);}} while(0)
static uint64_t bits(double x){uint64_t b;memcpy(&b,&x,8);return b;}
static double word(uint64_t b){double x;memcpy(&x,&b,8);return x;}
static FILE *create(const char *prefix,const char *suffix){char p[4096];CHECK(snprintf(p,sizeof p,"%s.%s",prefix,suffix)>0);FILE*f=fopen(p,"wb");CHECK(f);return f;}
static void values(FILE*f,const double*x,size_t n){CHECK(fwrite(x,8,n,f)==n);}
static coax_t_OsState analytic(coax_t_OsParams p,coax_t_OsState s,double u,double t){double r=p.damping/2,w=sqrt(p.spring-r*r),eq=u/p.spring,a=s.position-eq,b=(s.velocity+r*a)/w,decay=exp(-r*t),co=cos(w*t),si=sin(w*t);return(coax_t_OsState){.position=eq+decay*(a*co+b*si),.velocity=decay*(s.velocity*co-(r*s.velocity+p.spring*a)/w*si)};}
int main(int argc,char**argv){CHECK(argc==2);const char*prefix=argv[1];FILE*steps=create(prefix,"steps.f64");double worst=0;
 const double ks[]={.5,.75,1,2,3,4},cs[]={.1,.5,1},xs[]={-1,0,1},vs[]={-3,-1,0,1,3},us[]={-2,-.5,0,.5,2},hs[]={.0009765625,.01,.025,.05};
 for(size_t k=0;k<6;k++)for(size_t c=0;c<3;c++)for(size_t x=0;x<3;x++)for(size_t v=0;v<5;v++)for(size_t u=0;u<5;u++)for(size_t h=0;h<4;h++){
  coax_t_OsParams p={.mass=1,.spring=ks[k],.damping=cs[c]};coax_t_OsState s={.position=xs[x],.velocity=vs[v]};coax_t_OsStep got=os_step(p,s,us[u],hs[h]);CHECK(got.status==0);coax_t_OsState want=analytic(p,s,us[u],hs[h]);double error=fmax(fabs(got.state.position-want.position),fabs(got.state.velocity-want.velocity));CHECK(error<=1e-5);worst=fmax(worst,error);values(steps,&got.state.position,1);values(steps,&got.state.velocity,1);
 }
 CHECK(fclose(steps)==0);
 coax_t_OsParams p={.mass=1,.spring=2,.damping=.5};coax_t_OsState initial={.position=.75,.velocity=-.25};double errors[3];
 for(int level=0;level<3;level++){double h=.05/(1<<level);coax_t_OsState s=initial;for(int i=0;i<20*(1<<level);i++){coax_t_OsStep r=os_step(p,s,.5,h);CHECK(r.status==0);s=r.state;}coax_t_OsState exact=analytic(p,initial,.5,1);errors[level]=hypot(s.position-exact.position,s.velocity-exact.velocity);}
 double ratio1=errors[0]/errors[1],ratio2=errors[1]/errors[2];CHECK(ratio1>=12&&ratio1<=20&&ratio2>=12&&ratio2<=20);
 for(int k=0;k<6;k++)for(int c=0;c<3;c++){p.spring=ks[k];p.damping=cs[c];coax_t_OsState s=initial;double before=.5*s.velocity*s.velocity+.5*p.spring*s.position*s.position;for(int i=0;i<128;i++){coax_t_OsStep r=os_step(p,s,0,.05);CHECK(r.status==0);s=r.state;double after=.5*s.velocity*s.velocity+.5*p.spring*s.position*s.position;CHECK(after<=before+1e-12);before=after;}}
 const uint64_t nonfinite[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000000123)};
 for(int field=0;field<7;field++)for(int v=0;v<3;v++){p=(coax_t_OsParams){.mass=1,.spring=2,.damping=.5};coax_t_OsState s=initial;double u=0,h=.05,bad=word(nonfinite[v]);switch(field){case 0:p.mass=bad;break;case 1:p.spring=bad;break;case 2:p.damping=bad;break;case 3:s.position=bad;break;case 4:s.velocity=bad;break;case 5:u=bad;break;case 6:h=bad;break;}coax_t_OsStep r=os_step(p,s,u,h);CHECK(r.status==1&&bits(r.state.position)==bits(s.position)&&bits(r.state.velocity)==bits(s.velocity));}
 p=(coax_t_OsParams){.mass=1,.spring=2,.damping=.5};double force[129]={0};coax_t_OsState out[130];for(int i=0;i<130;i++)out[i]=(coax_t_OsState){7,9};
 CHECK(os_rollout(p,initial,(coax_v_f64){force,-1},.05,(coax_v_t_OsState){out,130},0).status==6);CHECK(os_rollout(p,initial,(coax_v_f64){force,129},.05,(coax_v_t_OsState){out,130},0).status==6);
 CHECK(os_rollout(p,initial,(coax_v_f64){force,128},.05,(coax_v_t_OsState){out,128},0).status==7);CHECK(os_rollout(p,initial,(coax_v_f64){force,1},.05,(coax_v_t_OsState){out,130},1).status==8);
 force[127]=2.001;CHECK(os_rollout(p,initial,(coax_v_f64){force,128},.05,(coax_v_t_OsState){out,130},0).status==4);for(int i=0;i<130;i++)CHECK(out[i].position==7&&out[i].velocity==9);force[127]=0;coax_t_OsRun run=os_rollout(p,initial,(coax_v_f64){force,128},.05,(coax_v_t_OsState){out,130},0);CHECK(run.status==0&&run.written==129&&out[129].position==7&&out[129].velocity==9);
 coax_t_OeData data=oe_data();CHECK(data.status==0&&data.rows==1280);FILE*f=create(prefix,"data.f64");values(f,data.x.e,6400);values(f,data.y.e,2560);CHECK(fclose(f)==0);f=create(prefix,"groups.i64");CHECK(fwrite(data.groups.e,8,1280,f)==1280);CHECK(fclose(f)==0);
 coax_t_OeFit fit=oe_fit(data,256,256);CHECK(fit.status==0&&fit.training_rows==256&&fit.epochs==256&&fit.steps==8192);f=create(prefix,"fit.f64");values(f,fit.mean.e,5);values(f,fit.sd.e,5);values(f,fit.target_mean.e,2);values(f,fit.target_sd.e,2);values(f,fit.affine.e,12);values(f,fit.mlp.e,114);CHECK(fclose(f)==0);
 f=create(prefix,"metrics.json");fprintf(f,"[\n");for(int split=0;split<3;split++)for(int model=0;model<4;model++){coax_t_OeMetric m=oe_evaluate(fit,model,split);CHECK(m.status==0);fprintf(f,"%s{\"split\":%d,\"model\":%d,\"episodes\":%lld,\"completed\":%lld,\"one_rows\":%lld,\"roll_rows\":%lld,\"one_refused\":%lld,\"roll_refused\":%lld,\"first_episode\":%lld,\"first_tick\":%lld,\"one_q\":%.17g,\"one_v\":%.17g,\"roll_q\":%.17g,\"roll_v\":%.17g,\"max_q\":%.17g,\"max_v\":%.17g}",(split||model)?",\n":"",split,model,(long long)m.episodes,(long long)m.completed,(long long)m.one_rows,(long long)m.roll_rows,(long long)m.one_refused,(long long)m.roll_refused,(long long)m.first_episode,(long long)m.first_tick,m.one_q,m.one_v,m.roll_q,m.roll_v,m.max_q,m.max_v);}fprintf(f,"\n]\n");CHECK(fclose(f)==0);
 f=create(prefix,"native.json");fprintf(f,"{\"cases\":%ld,\"analytic_max_absolute\":%.17g,\"refinement_ratios\":[%.17g,%.17g],\"refinement_errors\":[%.17g,%.17g,%.17g],\"training_updates\":8192}\n",cases,worst,ratio1,ratio2,errors[0],errors[1],errors[2]);CHECK(fclose(f)==0);printf("oscillator native: %ld assertions; analytic max %.9g; refinement %.9g %.9g; four original trained heads\n",cases,worst,ratio1,ratio2);return 0;
}
