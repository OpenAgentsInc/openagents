/* Independent native ABI/event tooling; no platform clock or SDK is read. */
#include "oscillator-clock.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <math.h>
static long checks;
#define CHECK(x) do {checks++;if(!(x)){fprintf(stderr,"line%d: %s\n",__LINE__,#x);exit(1);}}while(0)
static int same_clock(coax_t_ScSchedule a,coax_t_ScSchedule b){return a.now.epoch==b.now.epoch&&a.now.step==b.now.step&&a.origin==b.origin&&a.serial==b.serial&&a.remaining==b.remaining&&a.phase==b.phase&&a.holding==b.holding&&a.pending==b.pending;}
static int64_t bits(double d){int64_t b;memcpy(&b,&d,8);return b;}
static double word(uint64_t b){double d;memcpy(&d,&b,8);return d;}
static void put(FILE*f,int64_t n){uint64_t x=(uint64_t)n;unsigned char b[8];for(int i=0;i<8;i++)b[i]=(unsigned char)(x>>(8*i));CHECK(fwrite(b,1,8,f)==8);}
static void bounds(void){
 int64_t bad[]={INT64_MIN,-1,18001,INT64_MAX};coax_t_ScSimInstant at={1,25};
 for(unsigned i=0;i<4;i++){coax_t_ScSimInstant a={1,bad[i]};CHECK(!sc_sim_valid(a));CHECK(sc_sim_add(a,(coax_t_ScSimDelta){1}).status==COAX_ScStatus_range);CHECK(sc_sim_age(at,a).status==COAX_ScStatus_range);}
 for(unsigned i=0;i<4;i++){coax_t_ScSchedule s=sc_begin();s.now.step=bad[i];coax_t_ScResult r=sc_reset(s);CHECK(r.status==COAX_ScStatus_range&&same_clock(r.state,s));CHECK(sc_request(s,(coax_t_ScObservation){0}).status==COAX_ScStatus_range);CHECK(sc_accept(s,(coax_t_ScTicket){0}).status==COAX_ScStatus_range);CHECK(sc_select(s,1).status==COAX_ScStatus_range);CHECK(sc_sample(s,s.now).status==COAX_ScStatus_range);CHECK(sc_control(s,1,1).status==COAX_ScStatus_range);CHECK(sc_receipt(s,at,0).status==COAX_ScStatus_range);}
 coax_t_ScSteadyInstant t={1,9000000000000000000LL};CHECK(sc_steady_deadline(t,(coax_t_ScSteadyDelta){0}).status==0);CHECK(sc_steady_deadline(t,(coax_t_ScSteadyDelta){1}).status==COAX_ScStatus_range);
 int64_t nsbad[]={INT64_MIN,-1,9000000000000000001LL,INT64_MAX};for(unsigned i=0;i<4;i++){coax_t_ScSteadyInstant a={1,nsbad[i]};CHECK(!sc_steady_valid(a));CHECK(sc_steady_deadline(a,(coax_t_ScSteadyDelta){1}).status==COAX_ScStatus_range);CHECK(sc_steady_elapsed(t,a).status==COAX_ScStatus_range);CHECK(sc_steady_expiry(t,a).status==COAX_ScStatus_range);}
 uint64_t words[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(unsigned i=0;i<5;i++){double v=word(words[i]);CHECK(!sc_civil_valid((coax_t_ScCivilInstant){1,v}));CHECK(!sc_sdk_time(v,0,at));CHECK(!sc_sdk_time(0,v,at));}
 CHECK(sc_civil_valid((coax_t_ScCivilInstant){1,-1}));CHECK(sc_civil_valid((coax_t_ScCivilInstant){1000000,word(UINT64_C(0x7fefffffffffffff))}));
 for(int phase=7;phase<256;phase++){coax_t_ScSchedule s=sc_begin();s.phase=(uint8_t)phase;CHECK(!sc_schedule_valid(s));}
 coax_t_ScSimInstant end={1,18000};CHECK(sc_seconds((coax_t_ScSimDelta){18000}).seconds==36);CHECK(sc_sdk_time(100,136,end));double eps=ldexp(1,-26);CHECK(sc_sdk_time(0,36+eps,end));CHECK(!sc_sdk_time(0,36+2*eps,end));
}
static void refusals(void){
 coax_t_ScSchedule s=sc_begin();coax_t_ScResult q=sc_request(s,sc_observation(s.now));CHECK(q.status==0);s=q.state;
 for(int field=0;field<4;field++){coax_t_ScTicket t=q.ticket;if(field==0)t.epoch++;if(field==1)t.serial++;if(field==2)t.profile++;if(field==3)t.base_step++;coax_t_ScResult r=sc_accept(s,t);CHECK(r.status==COAX_ScStatus_stale&&same_clock(r.state,s));}
 s=sc_accept(s,q.ticket).state;s=sc_select(s,0).state;
 CHECK(sc_control(s,1,1).status==COAX_ScStatus_order);coax_t_ScResult early=sc_receipt(s,(coax_t_ScSimInstant){1,1},0);CHECK(early.status==COAX_ScStatus_order&&early.state.phase==COAX_ScPhase_halted&&early.state.now.step==0);
 s=sc_sample(s,s.now).state;CHECK(sc_control(s,0,1).status==COAX_ScStatus_order);CHECK(sc_control(s,1,0).status==COAX_ScStatus_order);s=sc_control(s,1,1).state;
 for(int fault=0;fault<5;fault++){coax_t_ScSimInstant after={1,1};int effect=0;if(fault==0)after.step=0;if(fault==1)after.step=2;if(fault==2)after.epoch=2;if(fault==3)effect=1;if(fault==4)after.step=INT64_MAX;coax_t_ScResult r=sc_receipt(s,after,effect);CHECK(r.status==COAX_ScStatus_effect&&r.state.phase==COAX_ScPhase_halted&&r.state.now.step==0&&r.state.remaining==8);CHECK(sc_sample(r.state,r.state.now).status==COAX_ScStatus_order);coax_t_ScResult z=sc_reset(r.state);CHECK(z.status==0&&z.state.now.epoch==2&&z.state.now.step==0);}
 coax_t_ScResult done=sc_receipt(s,(coax_t_ScSimInstant){1,1},0);CHECK(done.status==0&&done.state.now.step==1);coax_t_ScResult duplicate=sc_receipt(done.state,(coax_t_ScSimInstant){1,1},0);CHECK(duplicate.status==COAX_ScStatus_order&&duplicate.state.phase==COAX_ScPhase_halted&&duplicate.state.now.step==1);
 coax_t_ScOscResult o=sco_begin((coax_t_OsParams){1,2,.5},(coax_t_OsState){.5,0});coax_t_ScOscState p=o.state;p.clock=sc_select(p.clock,1).state;p.clock=sc_sample(p.clock,p.clock.now).state;
 coax_t_ScOscResult b=sco_control(p,INFINITY);CHECK(b.status==COAX_ScStatus_range&&b.plant_status==COAX_OsStatus_number&&same_clock(b.state.clock,p.clock)&&b.state.plant.position==.5);
 p=sco_control(p,0).state;p.plant=(coax_t_OsState){2,4};b=sco_advance(p,(coax_t_ScSimInstant){1,1},0);CHECK(b.status==COAX_ScStatus_effect&&b.plant_status==COAX_OsStatus_state&&b.state.clock.phase==COAX_ScPhase_halted&&b.state.clock.now.step==0&&b.state.plant.position==2&&b.state.plant.velocity==4);
}
static void run(FILE*f,FILE*json,int mode){
 coax_t_ScOscResult initial=sco_begin((coax_t_OsParams){1,2,.5},(coax_t_OsState){.5,0});CHECK(initial.status==0);coax_t_ScOscState s=initial.state;
 coax_t_ScTicket delayed={0};coax_t_ScObservation captured={.last_forward={1,-1},.qpos={1,-1}};int64_t origins[91];int requests=0,holds=0,refused=0,slot=-1;coax_t_ScSteadyInstant wall={1,0};double force=0;
 for(int step=0;step<18000;step++){
  if(step%25==0){
   if(mode==1&&step==25){coax_t_ScResult stale=sc_accept(s.clock,delayed);CHECK(stale.status==COAX_ScStatus_stale&&same_clock(stale.state,s.clock));s.clock=sc_refuse(s.clock).state;refused++;}
   int hold=step==0&&mode!=0;
   if(s.clock.remaining==0&&!(mode==3&&step==0)){
    captured=sc_observation(s.clock.now);CHECK(captured.last_forward.step==(step?step-1:0)&&captured.qpos.step==step);coax_t_ScResult q=sc_request(s.clock,captured);CHECK(q.status==0&&q.ticket.base_step==step);s.clock=q.state;origins[requests++]=q.ticket.base_step;
    coax_t_ScSteadyInstant deadline=sc_steady_deadline(wall,(coax_t_ScSteadyDelta){100}).instant;coax_t_ScSteadyInstant next=sc_steady_deadline(wall,(coax_t_ScSteadyDelta){1000000000000LL}).instant;CHECK(sc_steady_elapsed(wall,next).delta.ns==1000000000000LL);CHECK(sc_steady_expiry(deadline,next).expired);wall=next;
    CHECK(s.clock.now.step==step);
    if(mode==1&&step==0)delayed=q.ticket;else if(mode==2&&step==0){s.clock=sc_refuse(s.clock).state;refused++;}else{coax_t_ScResult accepted=sc_accept(s.clock,q.ticket);CHECK(accepted.status==0);s.clock=accepted.state;}
   }
   coax_t_ScResult select=sc_select(s.clock,hold);CHECK(select.status==0);slot=(int)select.slot;CHECK(slot==(hold?-1:(mode?(step/25-1)%8:(step/25)%8)));s.clock=select.state;if(hold){holds++;force=s.held_force;}else force=((slot%3)-1)*.25;
  }
  int64_t remaining=s.clock.remaining,pending=s.clock.pending,holding=s.clock.holding,serial=s.clock.serial,origin=s.clock.origin;
  coax_t_ScResult sample=sc_sample(s.clock,s.clock.now);CHECK(sample.status==0&&sample.state.phase==COAX_ScPhase_control&&sample.goal_update==(step%25==0));s.clock=sample.state;
  coax_t_ScOscResult controlled=sco_control(s,force);CHECK(controlled.status==0&&controlled.state.clock.phase==COAX_ScPhase_receipt&&controlled.state.held_force==force);s=controlled.state;
  coax_t_ScSimInstant after={1,step+1};coax_t_ScOscResult advanced=sco_advance(s,after,0);CHECK(advanced.status==0&&advanced.state.clock.now.step==step+1);s=advanced.state;CHECK(sc_schedule_valid(s.clock));CHECK(isfinite(s.plant.position)&&isfinite(s.plant.velocity));CHECK(sc_sdk_time(3,3+(step+1)*.002,s.clock.now));
  int64_t event[19]={mode,step,step+1,s.clock.now.epoch,serial,origin,remaining,slot,holding,pending,sample.goal_update,captured.last_forward.step,captured.qpos.step,step,s.clock.remaining,s.clock.phase,s.clock.holding,s.clock.pending,bits(s.held_force)};for(int i=0;i<19;i++)put(f,event[i]);
 }
 CHECK(s.clock.phase==COAX_ScPhase_finished&&s.clock.now.step==18000&&holds==(mode?1:0)&&refused==((mode==1||mode==2)?1:0));
 fprintf(json,"%s{\"mode\":%d,\"steps\":18000,\"rows\":720,\"requests\":%d,\"holds\":%d,\"refusals\":%d,\"origins\":[",mode?",\n":"",mode,requests,holds,refused);for(int i=0;i<requests;i++)fprintf(json,"%s%lld",i?",":"",(long long)origins[i]);fprintf(json,"],\"steady_ns\":%lld,\"remaining\":%lld,\"plant_finite\":true}",(long long)wall.ns,(long long)s.clock.remaining);
}
int main(int argc,char**argv){CHECK(argc==2);bounds();refusals();char p[4096];CHECK(snprintf(p,sizeof p,"%s.events.i64",argv[1])<(int)sizeof p);FILE*f=fopen(p,"wb");CHECK(f);CHECK(snprintf(p,sizeof p,"%s.summary.json",argv[1])<(int)sizeof p);FILE*j=fopen(p,"w");CHECK(j);fputs("{\"schema\":\"psionic-step-clock-native-v1\",\"scenarios\":[\n",j);for(int mode=0;mode<4;mode++)run(f,j,mode);fputs("\n]}\n",j);CHECK(!fclose(f));CHECK(!fclose(j));printf("step-clock native: %ld checks; 4 x18000 physical steps/720 rows; paused wall clock and fault receipts\n",checks);return 0;}
