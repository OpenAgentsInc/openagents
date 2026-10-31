/* Firm reason: 2 (independent oracle). C recomputes expected routes, ownership,
   deadlines and local-vs-shadow behavior, testing the real emitted ABI. */
#include "session.h"
#include <assert.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static coax_string text(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_t_GgsSession one(coax_t_GgsSession *s){return (coax_v_t_GgsSession){s,1};}
static coax_t_GgsInput input(int goal){return (coax_t_GgsInput){
 .observation={.help_requested=1,.goal=goal,.destination_available=1,.eligible=1},
 .epoch=1,.actor=0,.generation=2,.world_revision=3,.goal_revision=4,.intent=1,
 .configured=1,.authenticated=1,.boundary=1,.url=text("/realms/everglade?guard-ai=directions-shadow")};}
static void reply(coax_t_GgsSession *s,int seq,int64_t when,int action){
 uint8_t body[8192];char wire[8200];coax_string r=gge_fixture(action,(coax_v_u8){body,sizeof body});assert(r.count>0&&r.count<8192);
 int n=snprintf(wire,sizeof wire,"200\n%.*s",(int)r.count,r.data);assert(n>0&&(size_t)n<sizeof wire);
 ggs_receive(one(s),seq,when,(coax_string){wire,n});
}
static int64_t probe(coax_t_GgsSession *s,int field){return ggs_probe(one(s),field);}
int main(void){
 const int expected[]={1,2,3,4,1};unsigned cases=0;
 for(int goal=1;goal<=5;goal++)for(int action=0;action<5;action++)for(int edge=0;edge<3;edge++){
  struct {uint64_t pre;coax_t_GgsSession s;uint64_t post;} box={0};box.pre=box.post=UINT64_C(0x7531bdf02468ace9);
  coax_t_GgsInput i=input(goal);i.url=text("/realms/everglade?guard-ai=directions-shadow");ggs_step(one(&box.s),i,0);
  coax_t_GgsView v=ggs_view(one(&box.s),0);assert(v.visible&&v.local_action==expected[goal-1]&&v.local_intent==1&&!v.can_request);
  coax_t_GgsEffect e=ggs_effect(one(&box.s),0);assert(e.sequence==1&&e.intent==1&&e.flags==5&&e.timeout_ms==8000&&e.body.count>600);
  assert(e.url.count==17&&!memcmp(e.url.data,"/api/v1/systemone",17));ggs_sent(one(&box.s),1,1);
  int64_t when=edge==0?0:edge==1?7999:8000;reply(&box.s,1,when,action);ggs_step(one(&box.s),i,when);
  _Bool admitted=when<8000&&(action==0||action==expected[goal-1]);
  assert(probe(&box.s,203)==1&&probe(&box.s,204)==1&&probe(&box.s,205)==admitted&&probe(&box.s,206)==!admitted&&probe(&box.s,210)==1);
  assert(probe(&box.s,207)==(when<8000?action:-1));assert(probe(&box.s,208)==(admitted?action:-1));
  assert(probe(&box.s,215)==expected[goal-1]);v=ggs_view(one(&box.s),when);assert(v.visible==(when<4000)&&v.local_action==(when<4000?expected[goal-1]:0));
  reply(&box.s,1,when+1,action);reply(&box.s,1,when+2,action);assert(probe(&box.s,210)==1&&probe(&box.s,211)==1);
  assert(box.pre==UINT64_C(0x7531bdf02468ace9)&&box.post==box.pre);cases++;
 }
 for(int field=0;field<12;field++){
  coax_t_GgsSession s={0};coax_t_GgsInput i=input(2);i.url=text("?guard-ai=directions-shadow");ggs_step(one(&s),i,0);ggs_sent(one(&s),1,1);reply(&s,1,1,2);
  if(field==0)i.epoch++;if(field==1)i.actor++;if(field==2)i.generation++;if(field==3)i.world_revision++;if(field==4)i.goal_revision++;if(field==5)i.observation.goal=3;
  if(field==6)i.intent++;if(field==7)i.observation.eligible=0;if(field==8)i.observation.destination_available=0;if(field==9)i.suspended=1;if(field==10)i.blocked=1;if(field==11)i.observation.help_requested=0;
  ggs_step(one(&s),i,2);assert(probe(&s,205)==0&&probe(&s,206)==1&&probe(&s,210)==1&&probe(&s,212)==0);cases++;
 }
 for(int mode=0;mode<4;mode++){
  coax_t_GgsSession s={0};coax_t_GgsInput i=input(2);i.url=text(mode==0?"/realm":"?guard-ai=directions-shadow");if(mode==1)i.authenticated=0;if(mode==2)i.blocked=1;if(mode==3)i.intent=0;
  ggs_step(one(&s),i,0);assert(probe(&s,203)==0&&ggs_effect(one(&s),0).sequence==0);assert(ggs_view(one(&s),0).visible==(mode<2));cases++;
 }
 for(int owner=0;owner<3;owner++){
  coax_t_GgsSession s={0};coax_t_GgsInput i=input(2);i.url=text("?guard-ai=directions-shadow");ggs_step(one(&s),i,0);ggs_sent(one(&s),1,1);
  i.observation.hint_visible=1;i.hint_intent=owner;reply(&s,1,4500,2);ggs_step(one(&s),i,4501);
  assert(probe(&s,205)==(owner==1)&&probe(&s,206)==(owner!=1)&&!ggs_view(one(&s),4501).visible);cases++;
 }
 coax_t_GgsSession s={0};coax_t_GgsInput i=input(2);i.url=text("?guard-ai=directions-shadow");
 for(int n=0;n<7;n++){i.intent=n+1;i.epoch=n+1;ggs_step(one(&s),i,n*10000);if(n<6)ggs_sent(one(&s),n+1,0);assert(probe(&s,203)==(n<6?n+1:6));cases++;}
 assert(probe(&s,202)==7&&probe(&s,206)==6);
 {
  coax_t_GgsSession replaced={0};coax_t_GgsInput next=input(2);ggs_step(one(&replaced),next,0);ggs_sent(one(&replaced),1,1);
  next.intent=2;ggs_step(one(&replaced),next,1);assert(probe(&replaced,203)==1&&probe(&replaced,206)==1&&probe(&replaced,212)==0);
  reply(&replaced,1,2,2);assert(probe(&replaced,205)==0&&probe(&replaced,211)==1&&probe(&replaced,203)==1);cases++;
 }
 for(int count=-1;count<=2;count++)if(count!=1){
  coax_t_GgsSession slots[2]={0},saved[2]={0};coax_v_t_GgsSession span={slots,count};
  ggs_step(span,i,0);ggs_sent(span,1,0);ggs_receive(span,1,0,text("200\n{}"));assert(!ggs_pending(span,1)&&ggs_effect(span,0).sequence==0&&!ggs_view(span,0).offer&&ggs_probe(span,200)==-1&&!memcmp(slots,saved,sizeof slots));cases++;
 }
 printf("Directions session native: %u independent route/shadow/deadline/lifecycle/ABI cases passed\n",cases);return 0;
}
