/* Independent event oracle; product transitions remain in Coax. */
#include "panda-loop.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static int assertions;
#define CHECK(x) do{assertions++;assert(x);}while(0)
int main(void){
  for(int trial=0;trial<32;trial++){
    coax_t_PlState h=pl_reset_done(pl_new(),0).state;
    coax_t_PlAction actions[9]={0},work[8]={0};uint32_t words[112]={0};
    actions[8].v.e[0]=77;
    for(int i=0;i<112;i++){float v=i%7==6?(i%2?1.f:-1.f):(float)((i%9)-4)*.125f;memcpy(words+i,&v,4);}
    for(int tick=0;tick<720;tick++){
      if(tick%8==0){
        coax_t_PlResult r=pl_request(h);CHECK(!r.status&&r.tag.tick==tick&&r.tag.serial==tick/8+1);h=r.state;
        r=pl_accept(h,r.tag,tick,(coax_v_u32){words,112},(coax_v_t_PlAction){actions,9},(coax_v_t_PlAction){work,8},0);
        CHECK(!r.status&&r.state.phase==COAX_PlPhase_ready);h=r.state;
        for(int i=0;i<56;i++){uint32_t b;memcpy(&b,&actions[i/7].v.e[i%7],4);CHECK(b==words[i]);}CHECK(actions[8].v.e[0]==77);
      }
      coax_t_PlResult r=pl_choose(h,tick);CHECK(!r.status&&r.slot==tick%8&&!r.fallback&&r.state.phase==COAX_PlPhase_stepping);
      r=pl_commit(r.state,tick,25,0);CHECK(!r.status&&r.state.tick==tick+1&&r.state.index==tick%8+1);
      CHECK(r.state.phase==(tick==719?COAX_PlPhase_finished:tick%8==7?COAX_PlPhase_empty:COAX_PlPhase_ready));h=r.state;
    }
    CHECK(pl_request(h).status==COAX_PlStatus_order);coax_t_PlResult r=pl_reset(h);CHECK(!r.status&&r.state.epoch==2&&r.state.phase==COAX_PlPhase_resetting);
  }
  for(int fault=0;fault<9;fault++){
    coax_t_PlResult req=pl_request(pl_reset_done(pl_new(),0).state);coax_t_PlAction a[8],w[8];uint32_t x[112]={0};memset(a,0x42,sizeof(a));unsigned char before[sizeof(a)];memcpy(before,a,sizeof(a));
    coax_t_PlTag tag=req.tag;int now=0,n=112,cap=8,wcap=8,alias=0;
    if(fault==0)tag.epoch++;if(fault==1)tag.serial++;if(fault==2)tag.tick++;if(fault==3)tag.profile++;if(fault==4)now++;if(fault==5)n--;if(fault==6)cap--;if(fault==7)wcap--;if(fault==8)alias=1;
    coax_t_PlResult r=pl_accept(req.state,tag,now,(coax_v_u32){x,n},(coax_v_t_PlAction){a,cap},(coax_v_t_PlAction){w,wcap},alias);
    CHECK(r.status&&r.state.phase==COAX_PlPhase_waiting&&!memcmp(a,before,sizeof(a)));
  }
  for(int tick=0;tick<720;tick++){
    coax_t_PlState h=pl_reset_done(pl_new(),0).state;h.tick=h.origin=tick;
    coax_t_PlResult r=pl_request(h);coax_t_PlTag old=r.tag;r=pl_hold(r.state,tick);CHECK(!r.status&&r.fallback&&r.slot==-1);
    r=pl_commit(r.state,tick,25,0);CHECK(!r.status&&r.state.tick==tick+1&&r.state.fallbacks==1);
    if(tick<719){uint32_t x[112]={0};coax_t_PlAction a[8],w[8];CHECK(pl_accept(r.state,old,tick+1,(coax_v_u32){x,112},(coax_v_t_PlAction){a,8},(coax_v_t_PlAction){w,8},0).status==COAX_PlStatus_stale);}
  }
  printf("Panda supervisor independent native oracle: %d assertions passed\n",assertions);return 0;
}
