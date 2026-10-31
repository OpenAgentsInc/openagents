/* Exercise the actual host header with synthetic effects. No SDK/controller
   arithmetic or neural provider is initialized by this fixture. */
#include "panda-session.h"
#include "panda-env.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int fault,failed,requests,controls,renders;
struct PandaEnv {double initial[45];int step,last_forward,stage,resets;};
static PandaEnv *active;
#define MOCK(x) do{if(!(x)){fprintf(stderr,"mock effect order line%d: %s\n",__LINE__,#x);exit(3);}}while(0)
static int fail(int code){if(fault==code&&!failed){failed=1;return 1;}return 0;}
PandaEnv*panda_env_open(const char*x,const char*const names[24],char*err,size_t cap){(void)x;(void)names;(void)err;(void)cap;active=calloc(1,sizeof(*active));return active;}
void panda_env_close(PandaEnv*e){free(e);active=NULL;}
int panda_env_dimensions(PandaEnv*e,PandaDimensions*out){(void)e;*out=(PandaDimensions){23,21,9,0,45,.002};return 1;}
int panda_env_reset(PandaEnv*e,const double*state,size_t count){MOCK(count==45);memcpy(e->initial,state,360);e->step=e->last_forward=e->stage=0;e->resets++;return 1;}
int panda_env_forward(PandaEnv*e){if(fail(1))return 0;MOCK(e->stage==0);e->last_forward=e->step;e->stage=1;return 1;}
int panda_env_sample(PandaEnv*e,PandaTelemetry*t){if(e->stage==1&&fail(2))return 0;memset(t,0,sizeof(*t));t->time=e->initial[0]+e->step*.002;t->body_quat_wxyz[0]=1;for(int i=0;i<7;i++){t->torque_min[i]=i<5?-80:-12;t->torque_max[i]=i<5?80:12;}t->finger_lower[1]=-.04;t->finger_upper[0]=.04;t->site_mat[0]=t->site_mat[4]=t->site_mat[8]=1;t->target_mat[0]=t->target_mat[4]=t->target_mat[8]=1;t->target_size[0]=t->target_size[1]=t->target_size[2]=.1;t->book_pos[0]=10;if(e->stage==1)e->stage=2;return 1;}
int panda_env_render(PandaEnv*e,int camera,unsigned char*rgb,size_t count){if(fail(6))return 0;MOCK(e->stage==0&&e->last_forward==(e->step?e->step-1:0)&&camera>=0&&camera<2&&count==196608);memset(rgb,0,count);renders++;return 1;}
int panda_env_step(PandaEnv*e,const double*control,size_t count){(void)control;MOCK(count==9&&((e->resets==1&&e->stage==3)||(e->resets>1&&e->stage==1)));if(fail(4))return 0;e->step++;e->stage=4;return 1;}
int panda_env_state(PandaEnv*e,double*state,size_t count){MOCK(count==45&&e->stage==4);memcpy(state,e->initial,360);state[0]=e->initial[0]+e->step*.002;if(fail(5))state[0]+=.002;e->stage=0;return 1;}
static coax_t_PcStatus fixture_reset(coax_v_t_PcState state,coax_t_PcSample sample,coax_v_f64 home){(void)sample;(void)home;memset(state.data,0,sizeof(coax_t_PcState));return COAX_PcStatus_ok;}
static coax_t_PcOutput fixture_control(coax_v_t_PcState state,coax_t_PcSample sample,coax_v_f64 action,int64_t sub){(void)state;(void)sample;(void)action;MOCK(active->stage==2&&sub==active->step%25);coax_t_PcOutput out={0};if(fail(7)){out.status=COAX_PcStatus_shape;return out;}active->stage=3;controls++;return out;}
#define pc_reset fixture_reset
#define ps_control fixture_control
#include "panda_loop_effects.h"
#undef pc_reset
#undef ps_control
static int fixture_commands(void*context,const uint8_t*a,const uint8_t*b,const float*s,const float*noise,float*out,PandaPolicyTiming*t){(void)context;(void)a;(void)b;(void)s;(void)noise;MOCK(active->stage==0&&active->last_forward==(active->step?active->step-1:0));if(fail(3))return 0;memset(out,0,448);memset(t,0,sizeof(*t));t->tokens=152;requests++;return 1;}
static const char*fixture_error(const void*c){(void)c;return "declared synthetic source fault";}
int main(int argc,char**argv){
 if(argc!=5)return 2;fault=atoi(argv[4]);char*args[]={"fixture","unused","unused","unused","unused",argv[1],"fixture", "10",argv[3],argv[2]};float noise[5280]={0};int context=0;PandaLoopProvider p={&context,fixture_commands,fixture_error,"panda-clock-synthetic-host-seam-v1","psionic-panda-clock-synthetic-host-v1",0};
 int status=panda_loop_cases(10,args,&p,(unsigned char*)noise,sizeof noise,0);MOCK(!status&&controls==250&&requests==(atoi(argv[3])==2?3:2)&&renders==requests*2);printf("Panda clock host seam:250controls,%dsyntheticrequests; actual header order/replay; no SDK/NN\n",requests);return 0;
}
