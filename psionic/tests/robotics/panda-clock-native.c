/* New paired-state ABI checks only; no SDK or controller/model arithmetic. */
#include "panda-clock.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <limits.h>
static int n;
#define CHECK(x) do{n++;if(!(x)){fprintf(stderr,"paired clock line%d: %s\n",__LINE__,#x);exit(1);}}while(0)
static uint64_t bits(double x){uint64_t w;memcpy(&w,&x,8);return w;}
int main(void){
 coax_t_PkResult b=pk_begin(.25,0);CHECK(!b.status&&pk_valid(b.state));coax_t_PkState s=b.state;coax_t_PkResult q=pk_request(s,0);CHECK(!q.status);uint32_t words[112]={0};coax_t_PlAction out[8]={0},work[8]={0};for(int i=0;i<8;i++)out[i].v.e[0]=7;
 for(int field=0;field<4;field++){coax_t_PlTag tag=q.tag;if(field==0)tag.epoch++;if(field==1)tag.serial++;if(field==2)tag.profile++;if(field==3)tag.tick=INT64_MAX;coax_t_PkResult r=pk_accept(q.state,tag,0,(coax_v_u32){words,112},(coax_v_t_PlAction){out,8},(coax_v_t_PlAction){work,8},0);CHECK(r.status==COAX_PkStatus_stale&&r.state.clock.pending&&r.state.supervisor.phase==COAX_PlPhase_waiting);for(int i=0;i<8;i++)CHECK(out[i].v.e[0]==7);}
 for(int i=0;i<112;i++){words[i]=UINT32_C(0x7fc00001);coax_t_PkResult r=pk_accept(q.state,q.tag,0,(coax_v_u32){words,112},(coax_v_t_PlAction){out,8},(coax_v_t_PlAction){work,8},0);CHECK(r.status==COAX_PkStatus_command&&r.pl_status==COAX_PlStatus_number&&r.state.clock.pending);for(int j=0;j<8;j++)CHECK(out[j].v.e[0]==7);words[i]=0;}
 s=pk_accept(q.state,q.tag,0,(coax_v_u32){words,112},(coax_v_t_PlAction){out,8},(coax_v_t_PlAction){work,8},0).state;s=pk_select(s,0).state;coax_t_PkState selected=s;
 coax_t_PkResult early=pk_sample(s,0);CHECK(early.status==COAX_PkStatus_order&&early.state.clock.phase==COAX_ScPhase_halted&&early.state.supervisor.phase==COAX_PlPhase_halted);
 s=pk_forward(s,0).state;CHECK(s.fresh);s=pk_sample(s,0).state;CHECK(!s.fresh);s=pk_control(s,0).state;
 int64_t ticks[]={-1,720,INT64_MIN,INT64_MAX};for(unsigned i=0;i<4;i++){coax_t_PkResult r=pk_receipt(s,ticks[i],1,bits(.252),0);CHECK(r.status==COAX_PkStatus_clock&&r.state.clock.now.step==0&&r.state.clock.phase==COAX_ScPhase_halted&&r.state.supervisor.phase==COAX_PlPhase_halted);}
 uint64_t bad[]={UINT64_C(0x7ff8000000000001),UINT64_C(0x7ff0000000000001),UINT64_C(0xfff0000000000000)};for(int i=0;i<3;i++)CHECK(pk_receipt(s,0,1,bad[i],0).status==COAX_PkStatus_clock);
 coax_t_PkResult good=pk_receipt(s,0,1,bits(.252),0);CHECK(!good.status&&good.state.clock.now.step==1&&good.state.supervisor.tick==0);coax_t_PkResult duplicate=pk_receipt(good.state,0,1,bits(.252),0);CHECK(duplicate.status==COAX_PkStatus_order&&duplicate.state.clock.now.step==1&&duplicate.state.clock.phase==COAX_ScPhase_halted);
 CHECK(pk_reset(selected,.5,1).status==COAX_PkStatus_effect);coax_t_PkResult reset=pk_reset(selected,.5,0);CHECK(!reset.status&&reset.state.clock.now.epoch==2&&reset.state.clock.now.step==0&&reset.state.last_forward.step==0&&pk_valid(reset.state));
 coax_t_PkResult stop=pk_finish(pk_begin(0,0).state,1);CHECK(!stop.status&&stop.state.supervisor.success&&stop.state.clock.phase==COAX_ScPhase_finished&&pk_valid(stop.state));
 printf("Panda paired clock: %d ABI/atomic-publication/fault checks; no SDK/model\n",n);return 0;
}
