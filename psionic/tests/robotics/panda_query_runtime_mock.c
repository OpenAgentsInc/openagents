/* NEW effect-order tests for the optional owned-query runtime bridge.
   The generated Coax functions are real. Only SDK effects are mocked. */
#include <dlfcn.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define pk_forward mock_real_pk_forward
#define pk_sample mock_real_pk_sample
#define pk_control mock_real_pk_control
#define pqs_select mock_real_pqs_select
#define ps_sample mock_real_ps_sample
#define pqn_model mock_real_pqn_model
#define pqn_asset_path mock_real_pqn_asset_path
#include "panda-session.h"
#undef pk_forward
#undef pk_sample
#undef pk_control
#undef pqs_select
#undef ps_sample
#undef pqn_model
#undef pqn_asset_path
#include "panda-env.h"

static size_t assertions;
#define NEED(x) do { assertions++; if (!(x)) { fprintf(stderr,"query runtime mock line %d: %s\n",__LINE__,#x); exit(2); } } while (0)
enum { FORWARD=1, CLOCK_FORWARD, SAMPLE, ROBOT, SELECT, CLOCK_SAMPLE, CONTROL, FAULT, LOOKUP, MODEL, ASSET };
static int events[32],count,which,selected_kind,model_calls,asset_calls;
static coax_t_PkResult last_clock;
static coax_t_PqsResult last_selection;
static coax_t_PcSample selected;
static void event(int code) { NEED(count<32); events[count++]=code; }
static void reset(int test) { which=test;count=0;selected_kind=-1;model_calls=asset_calls=0;memset(&last_clock,0,sizeof last_clock);memset(&last_selection,0,sizeof last_selection);memset(&selected,0,sizeof selected); }
static void trace(const int *expected,size_t n) { NEED(count==(int)n);for(size_t i=0;i<n;i++)NEED(events[i]==expected[i]); }
struct PandaEnv {int unused;};
int panda_env_forward(PandaEnv *env) { NEED(env!=NULL);event(FORWARD);return which!=1; }
int panda_env_sample(PandaEnv *env,PandaTelemetry *out) { NEED(env&&out);event(SAMPLE);memset(out,0,sizeof *out);if(which==6)out->book_pos[2]=NAN;return which!=2; }
int panda_env_robot_state(PandaEnv *env,PandaRobotStateV1 *out) { NEED(env&&out);event(ROBOT);memset(out,0,sizeof *out);if(which==4)out->q[0]=NAN;if(which==5)out->qd[0]=21;if(which==12)out->qd[8]=NAN;return which!=3; }
coax_t_PkResult pk_forward(coax_t_PkState s,int64_t effect) { event(CLOCK_FORWARD);last_clock=mock_real_pk_forward(s,effect);return last_clock; }
coax_t_PkResult pk_sample(coax_t_PkState s,int64_t effect) { event(CLOCK_SAMPLE);last_clock=mock_real_pk_sample(s,which==7?1:effect);return last_clock; }
coax_t_PkResult pk_control(coax_t_PkState s,int64_t substep) { event(CONTROL);last_clock=mock_real_pk_control(s,substep);return last_clock; }
coax_t_PqsResult pqs_select(int64_t kind,coax_v_u64 sdk,coax_v_u64 q,coax_v_u64 v,int64_t se,int64_t re,coax_v_t_PcSample out) {
 event(SELECT);selected_kind=(int)kind;NEED(sdk.count==173&&q.count==9&&v.count==9&&out.count==1);
 last_selection=mock_real_pqs_select(kind,sdk,q,v,se,re,out);if(!last_selection.code)selected=out.data[0];return last_selection;
}
coax_t_PcSample ps_sample(int64_t scenario,int64_t tick,int64_t substep,coax_t_PcSample in) { event(FAULT);NEED(tick==0&&substep==0);return mock_real_ps_sample(scenario,tick,substep,in); }
coax_t_PqnStatus pqn_model(coax_v_u8 x,coax_v_u8 i,coax_v_u8 sdk,int64_t effect) { event(MODEL);model_calls++;return mock_real_pqn_model(x,i,sdk,effect); }
coax_string pqn_asset_path(coax_v_u8 xml,int64_t index) { event(ASSET);asset_calls++;return mock_real_pqn_asset_path(xml,index); }
static void *mock_dlsym(void *handle,const char *name) { (void)handle;NEED(!strcmp(name,"mj_loadXML"));event(LOOKUP);return NULL; }
static int mock_dladdr(const void *address,Dl_info *info) { (void)address;(void)info;NEED(0);return 0; }
#define dlsym mock_dlsym
#define dladdr mock_dladdr
#include "panda_query_runtime.h"
#undef dlsym
#undef dladdr

/* Compare logical fields; structure assignment does not promise padding. */
static void same_clock(coax_t_PkState a,coax_t_PkState b) {
#define SUP(f) NEED(a.supervisor.f==b.supervisor.f)
 SUP(phase);SUP(epoch);SUP(serial);SUP(tick);SUP(origin);SUP(index);SUP(refusals);SUP(fallbacks);SUP(holding);SUP(resume_waiting);SUP(success);
#undef SUP
#define CLK(f) NEED(a.clock.f==b.clock.f)
 CLK(now.epoch);CLK(now.step);CLK(origin);CLK(serial);CLK(remaining);CLK(phase);CLK(holding);CLK(pending);
#undef CLK
 NEED(a.last_forward.epoch==b.last_forward.epoch&&a.last_forward.step==b.last_forward.step&&a.fresh==b.fresh&&a.initial_seconds==b.initial_seconds);
}
int main(void) {
 const int complete[]={FORWARD,CLOCK_FORWARD,SAMPLE,ROBOT,SELECT,CLOCK_SAMPLE,CONTROL,FAULT};
 const int before_control[]={FORWARD,CLOCK_FORWARD,SAMPLE,ROBOT,SELECT,CLOCK_SAMPLE};
 const int control_refusal[]={FORWARD,CLOCK_FORWARD,SAMPLE,ROBOT,SELECT,CLOCK_SAMPLE,CONTROL};
 const int forward_refusal[]={FORWARD,CLOCK_FORWARD};
 PandaEnv env={0};
 for(int test=0;test<=12;test++) {
  reset(test);coax_t_PkResult begin=pk_begin(0,0);NEED(!begin.status);coax_t_PkResult hold=pk_select(begin.state,1);NEED(!hold.status);coax_t_PkState clock=hold.state;
  if(test==10)clock.supervisor.epoch=0;if(test==11)clock.fresh=1;
  PandaTelemetry telemetry;memset(&telemetry,0xA5,sizeof telemetry);coax_t_PcSample out,poison;memset(&out,0xA5,sizeof out);memcpy(&poison,&out,sizeof poison);
  int ok=panda_query_control_v1(&env,&telemetry,&clock,test==9?4:0,test==8?26:0,&out);int success=test==0||test==9;NEED(ok==success);same_clock(clock,last_clock.state);
  if(success) {
   trace(complete,sizeof complete/sizeof *complete);NEED(selected_kind==1&&!last_selection.code&&!last_clock.status);
   coax_t_PcSample expected=selected;if(test==9)for(int j=0;j<7;j++)expected.bias.e[j]=500;
   NEED(!memcmp(&out,&expected,sizeof out));NEED(fabs(out.position.e[0]+0.662)<2e-12&&fabs(out.position.e[1])<2e-12&&fabs(out.position.e[2]-1.7415)<2e-12);
  } else {
   NEED(!memcmp(&out,&poison,sizeof out));NEED(last_clock.status!=0);
   if(test==1||test==10||test==11)trace(forward_refusal,sizeof forward_refusal/sizeof *forward_refusal);
   else if(test==8)trace(control_refusal,sizeof control_refusal/sizeof *control_refusal);
   else trace(before_control,sizeof before_control/sizeof *before_control);
   if(test>=2&&test<=7)NEED(selected_kind==1);
   if(test==2||test==3)NEED(last_selection.code==COAX_PqsCode_effect);
   if(test==4||test==6||test==12)NEED(last_selection.code==COAX_PqsCode_number);
   if(test==5)NEED(last_selection.code==COAX_PqsCode_sample&&last_selection.detail==COAX_PcStatus_range);
   if(test==7||test==8)NEED(last_selection.code==COAX_PqsCode_ok);
  }
 }
 /* A bad mode returns before even locating the SDK. A failed actual-file
    observation reaches the Coax model gate and never extracts asset paths. */
 reset(0);coax_t_PqnDescriptorV1 descriptor,old;memset(&descriptor,0xA5,sizeof descriptor);memcpy(&old,&descriptor,sizeof old);
 NEED(!panda_query_preflight_v1("owned",0,NULL,&descriptor));NEED(count==0&&!memcmp(&old,&descriptor,sizeof old));
 char *args[10]={0};args[9]="/__psionic_mock_nonexistent_model__";
 NEED(!panda_query_preflight_v1("owned-shadow-v1",10,args,&descriptor));NEED(model_calls==1&&asset_calls==0&&!memcmp(&old,&descriptor,sizeof old));
 const int rejected_model[]={LOOKUP,MODEL};trace(rejected_model,2);
 printf("{\"passed\":true,\"control_cases\":13,\"preflight_cases\":2,\"assertions\":%zu,\"SDK_calls\":0,\"NN_calls\":0,\"steps\":0}\n",assertions);return 0;
}
