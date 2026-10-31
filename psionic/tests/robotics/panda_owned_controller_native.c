/* NEW paired static controller witness. All controller arithmetic remains in
   the unchanged qualified Coax translation; this file supplies held fixtures. */
#include "panda-owned-sample.h"
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
_Static_assert(sizeof(coax_t_PcSample)==1152&&sizeof(coax_t_PcState)==176&&sizeof(coax_t_PcOutput)==96,"qualified controller ABI");
static size_t checks,events,failures,atomic_failures;static FILE*transcript;
#define NEED(x) do{++checks;if(!(x)){fprintf(stderr,"paired controller failure line%d\n",__LINE__);exit(1);}}while(0)
static void read_bytes(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(const void*p,size_t n){NEED(fwrite(p,1,n,transcript)==n);}
static void event(int kind,int sample,int pattern,int tick,int side,coax_t_PcOutput out,const coax_t_PcState*state,coax_t_PcStatus prep){
 int64_t meta[10]={kind,sample,pattern,tick,side,out.status,state->ready?1:0,out.torque_clipped,out.action_clipped,prep};double values[30];
 NEED(out.status<=COAX_PcStatus_limits);memcpy(values,out.torque.e,7*8);memcpy(values+7,out.gripper.e,2*8);memcpy(values+9,state->goal_position.e,3*8);memcpy(values+12,state->goal_rotation.e,9*8);memcpy(values+21,state->home.e,7*8);memcpy(values+28,state->grip.e,2*8);for(int i=0;i<30;i++)NEED(isfinite(values[i]));put(meta,sizeof meta);put(values,sizeof values);events++;if(out.status!=COAX_PcStatus_ok)failures++;
}
static coax_t_PcStatus reset(coax_t_PcState*state,coax_t_PcSample sample,double home[7]){
 unsigned char before[sizeof*state];memcpy(before,state,sizeof before);coax_t_PcStatus status=pc_reset((coax_v_t_PcState){state,1},sample,(coax_v_f64){home,7});if(status!=COAX_PcStatus_ok){NEED(!memcmp(before,state,sizeof before));atomic_failures++;}return status;
}
static coax_t_PcOutput step(coax_t_PcState*state,coax_t_PcSample sample,double*action,int64_t count,int new_goal){
 unsigned char before[sizeof*state];memcpy(before,state,sizeof before);coax_t_PcOutput out=pc_step((coax_v_t_PcState){state,1},sample,(coax_v_f64){action,count},new_goal!=0);if(out.status!=COAX_PcStatus_ok){NEED(!memcmp(before,state,sizeof before));atomic_failures++;}return out;
}
int main(int argc,char**argv){
 if(argc!=3)return 2;FILE*input=fopen(argv[1],"rb");transcript=fopen(argv[2],"wbx");NEED(input&&transcript);uint32_t header[4];read_bytes(input,header,sizeof header);NEED(header[0]==UINT32_C(0x31434f50)&&header[1]==15&&header[2]==3&&header[3]==25);double home[7],actions[3][7];coax_t_PcSample samples[2][15],original[2][15];read_bytes(input,home,sizeof home);read_bytes(input,actions,sizeof actions);read_bytes(input,samples,sizeof samples);NEED(fgetc(input)==EOF&&!fclose(input));memcpy(original,samples,sizeof samples);
 coax_a7_f64 source_home=pc_default_home();NEED(!memcmp(home,source_home.e,sizeof home));double sample_words[4320];memcpy(sample_words,samples,sizeof sample_words);for(int i=0;i<4320;i++)NEED(isfinite(sample_words[i]));for(int i=0;i<7;i++)NEED(isfinite(home[i]));for(int i=0;i<3;i++)for(int j=0;j<7;j++)NEED(isfinite(actions[i][j]));
 for(int sample=0;sample<15;sample++)for(int pattern=0;pattern<3;pattern++){
  coax_t_PcState state[2];memset(state,0,sizeof state);coax_t_PcStatus prep[2];for(int side=0;side<2;side++){prep[side]=reset(&state[side],samples[side][sample],home);coax_t_PcOutput out={0};out.status=prep[side];event(0,sample,pattern,-1,side,out,&state[side],prep[side]);}
  for(int tick=0;tick<25;tick++)for(int side=0;side<2;side++){coax_t_PcOutput out=step(&state[side],samples[side][sample],actions[pattern],7,tick==0);event(1,sample,pattern,tick,side,out,&state[side],prep[side]);}
 }
 for(int sample=0;sample<15;sample++)for(int fault=0;fault<5;fault++)for(int side=0;side<2;side++){
  coax_t_PcState state;memset(&state,0,sizeof state);coax_t_PcStatus prep=reset(&state,samples[side][sample],home);coax_t_PcSample current=samples[side][sample];double action[7]={0};coax_t_PcOutput out={0};int64_t count=7;
  if(fault<2)current.mass.e[0]=-1;if(fault==2){uint64_t bits=UINT64_C(0x7ff8000000000001);memcpy(action+2,&bits,8);}if(fault==3)count=6;if(fault==4)state.ready=0;
  if(fault==0)out.status=reset(&state,current,home);else out=step(&state,current,action,count,1);event(10+fault,sample,-1,-1,side,out,&state,prep);
 }
 NEED(!memcmp(samples,original,sizeof samples));NEED(events==2490);NEED(!fclose(transcript));printf("{\"schema\":\"psionic.panda-owned-controller.native.v1\",\"passed\":true,\"events\":%zu,\"refusals\":%zu,\"atomic_failed_calls\":%zu,\"assertions\":%zu,\"physics_steps\":0}\n",events,failures,atomic_failures,checks);return 0;
}
