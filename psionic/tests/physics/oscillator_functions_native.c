/* Small-plant transform qualification tooling; never shipped. */
#include "oscillator-functions.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include <string.h>
static unsigned long checks;
#define CHECK(x) do{if(!(x)){fprintf(stderr,"oscillator function line%d: %s\n",__LINE__,#x);abort();}checks++;}while(0)
static float values[256],point[32];static uint8_t flags[256];static double dots[256],grads[256],grad_dots[256],seed[32],direction[32],stage[256];
static coax_t_FxWork work(void){return(coax_t_FxWork){.values={values,256},.flags={flags,256},.dots={dots,256},.grads={grads,256},.grad_dots={grad_dots,256},.seed={seed,32},.direction={direction,32},.stage={stage,256},.point={point,32}};}
static void array(FILE*f,const double*x,int n){fputc('[',f);for(int i=0;i<n;i++){if(i)fputc(',',f);fprintf(f,"%.17g",x[i]);}fputc(']',f);}
static void floats(FILE*f,const float*x,int n){double a[128];CHECK(n<=128);for(int i=0;i<n;i++)a[i]=x[i];array(f,a,n);}
static void key(FILE*f,const char*k,const double*x,int n){fprintf(f,",\"%s\":",k);array(f,x,n);}
static void eval(coax_t_FxFunction f,float*x,int domain,double*out,int count){for(int i=0;i<12;i++)out[i]=1234567;CHECK(f.count==count);coax_t_FxRun r=fx_eval(f,(coax_v_f32){x,domain},(coax_v_f64){out,12},work(),0);CHECK(r.status==COAX_FxStatus_ok&&r.values==count);for(int i=count;i<12;i++)CHECK(out[i]==1234567);}
int main(int argc,char**argv){CHECK(argc==2);FILE*out=fopen(argv[1],"wx");CHECK(out);coax_t_FxBuild f[5];for(int i=0;i<5;i++){f[i]=of_function(i);CHECK(f[i].status==COAX_FxStatus_ok);}CHECK(of_function(5).status==COAX_FxStatus_operation&&of_function(255).status==COAX_FxStatus_operation);
 const double pc[3][2]={{.5,.1},{2.4,.55},{4,1}},dt[3]={.002,.01,.05},cases[5][3]={{-1.25,-.75,-1.5},{-.8,1.5,.25},{0,.2,1.75},{.8,-.4,.2},{1.25,.75,-.75}};float weights[3]={1,.25f,.05f};double tangent[22]={.4,-.3,.7},cot[3]={.4,-.2,.1};coax_t_OsState target={.2,0};int id=0,group=0;
 for(int p=0;p<3;p++)for(int h=0;h<3;h++,group++){
  float packed[5][22];
  for(int s=0;s<5;s++,id++){
   coax_t_OsParams params={1,pc[p][0],pc[p][1]};coax_t_OsState state={cases[s][0],cases[s][1]};coax_t_OfInput in=of_pack(params,state,cases[s][2],dt[h],target,(coax_v_f32){weights,3});CHECK(in.status==COAX_OsStatus_ok);memcpy(packed[s],in.values.e,sizeof packed[s]);double result[5][12];for(int mode=0;mode<5;mode++)eval(f[mode].function,in.values.e,mode==0||mode==2?16:22,result[mode],mode==0||mode==3?3:mode==1?1:9);
   double y[3],dy[3],dx[16],gg[22],hv[22];CHECK(fx_jvp(f[0].function,(coax_v_f32){in.values.e,16},(coax_v_f64){tangent,16},(coax_v_f64){y,3},(coax_v_f64){dy,3},work(),0).status==COAX_FxStatus_ok);CHECK(!memcmp(y,result[0],sizeof y));CHECK(fx_vjp(f[0].function,(coax_v_f32){in.values.e,16},(coax_v_f64){cot,3},(coax_v_f64){y,3},(coax_v_f64){dx,16},work(),0).status==COAX_FxStatus_ok);CHECK(!memcmp(y,result[0],sizeof y));CHECK(fx_hvp(f[1].function,(coax_v_f32){in.values.e,22},(coax_v_f64){tangent,22},(coax_v_f64){gg,22},(coax_v_f64){hv,22},work(),0).status==COAX_FxStatus_ok);CHECK(!memcmp(gg,result[3],3*sizeof(double)));
   coax_t_OsStep physical=os_step(params,state,cases[s][2],dt[h]);CHECK(physical.status==COAX_OsStatus_ok);double phys[2]={physical.state.position,physical.state.velocity},fd[4];for(int e=0;e<2;e++){double eps=e==0?.001:.0001;coax_t_OsStep a=os_step(params,state,cases[s][2]+eps,dt[h]),b=os_step(params,state,cases[s][2]-eps,dt[h]);CHECK(a.status==COAX_OsStatus_ok&&b.status==COAX_OsStatus_ok);fd[2*e]=(a.state.position-b.state.position)/(2*eps);fd[2*e+1]=(a.state.velocity-b.state.velocity)/(2*eps);}
   fprintf(out,"{\"kind\":\"point\",\"id\":%d,\"group\":%d,\"spring\":%.17g,\"damping\":%.17g,\"dt\":%.17g,\"initial\":",id,group,pc[p][0],pc[p][1],dt[h]);array(out,cases[s],3);fprintf(out,",\"packed\":");floats(out,in.values.e,22);key(out,"step",result[0],3);key(out,"cost",result[1],1);key(out,"jacobian",result[2],9);key(out,"gradient",result[3],3);key(out,"hessian",result[4],9);key(out,"jvp",dy,3);key(out,"vjp",dx,3);key(out,"hvp_gradient",gg,3);key(out,"hvp",hv,3);key(out,"physical",phys,2);key(out,"physical_force_fd",fd,4);fputs("}\n",out);
  }
  int64_t axes[22];float shared[22],rows[110];memcpy(shared,packed[0],sizeof shared);for(int j=0;j<22;j++){axes[j]=j<3?0:-1;if(j<3)shared[j]=NAN;}for(int s=0;s<5;s++)for(int j=0;j<22;j++)rows[s*22+j]=j<3?packed[s][j]:NAN;
  for(int mode=3;mode<5;mode++){coax_t_FxMapBuild map=fx_vmap(f[mode].function,(coax_v_s64){axes,22},5);CHECK(map.status==COAX_FxStatus_ok);double batch[46];for(int i=0;i<46;i++)batch[i]=1234567;CHECK(fx_vmap_apply(map.map,(coax_v_f32){shared,22},(coax_v_f32){rows,110},(coax_v_f64){batch,46},work(),0).status==COAX_FxStatus_ok);int count=5*(mode==3?3:9);CHECK(batch[count]==1234567);fprintf(out,"{\"kind\":\"batch\",\"group\":%d,\"mode\":%d,\"values\":",group,mode);array(out,batch,count);fputs("}\n",out);}
 }
 CHECK(id==45&&group==9);coax_t_OsParams p={1,2,.5};coax_t_OsState s={0};for(int lane=0;lane<3;lane++){float w[3]={1,.25f,.05f};w[lane]=NAN;CHECK(of_pack(p,s,0,.01,s,(coax_v_f32){w,3}).status==COAX_OsStatus_number);w[lane]=0;CHECK(of_pack(p,s,0,.01,s,(coax_v_f32){w,3}).status==COAX_OsStatus_ok);}CHECK(of_pack(p,s,0,.01,(coax_t_OsState){NAN,0},(coax_v_f32){weights,3}).status==COAX_OsStatus_number);CHECK(of_pack(p,s,0,.01,(coax_t_OsState){0,NAN},(coax_v_f32){weights,3}).status==COAX_OsStatus_number);
 CHECK(fclose(out)==0);fprintf(stderr,"oscillator function native: %lu checks\n",checks);return 0;
}
