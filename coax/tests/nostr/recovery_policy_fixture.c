/* Independent bounded mock of storage + a trusted CAS checkpoint. This is a
 * test of the abstract policy, not an implementation or trusted provider. */
#include "recovery-policy.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#define CHECK(x) do { ++checks; if (!(x)) { fprintf(stderr,"recovery control failed at %d\n",__LINE__); return 1; } } while(0)

int recovery_policy_fixture(void) {
 int checks=0;
 const uint64_t generations[]={0,1,2,3,UINT64_MAX-1,UINT64_MAX};
 for (unsigned i=0;i<sizeof generations/sizeof generations[0];i++) {
  for (unsigned j=0;j<sizeof generations/sizeof generations[0];j++) {
   for (unsigned as=0;as<3;as++) for (unsigned bs=0;bs<3;bs++)
    for(unsigned ai=0;ai<2;ai++) for(unsigned bi=0;bi<2;bi++)
     for(unsigned ah=0;ah<2;ah++) for(unsigned bh=0;bh<2;bh++) {
      uint64_t ag=generations[i],bg=generations[j];
      int match=ag!=0 && as!=0 && ag==bg && as==bs && ai==bi && ah==bh;
      int next=ag!=0 && as!=0 && ag!=UINT64_MAX && bg==ag+1 && as==bs && ai==bi;
      CHECK(rf_match(ag,as,ai,ah,bg,bs,bi,bh)==match);
      CHECK(rf_next(ag,as,ai,ah,bg,bs,bi,bh)==next);
     }
  }
 }
 for(unsigned phase=0;phase<8;phase++) for(unsigned event=0;event<6;event++) {
  unsigned expected=0;
  if(phase==1 && event==1)expected=2;
  if(phase==2 && event==2)expected=3;
  if(phase==4 && event==3)expected=5;
  CHECK(rf_event(phase,event)==expected);
  CHECK(rf_ack(phase,2,1,11,33)==(phase==5));
 }
 /* All interruption points in the declared order. The separate checkpoint
  * simulator is intentionally trusted, atomic and monotonic by assumption. */
 for(unsigned stop=0;stop<=5;stop++) {
  unsigned phase=1, durable=0, committed=0, staged=0;
  if(stop>=1){phase=rf_event(phase,1);durable=1;}
  if(stop>=2)phase=rf_event(phase,2);
  if(stop>=3){CHECK(durable && phase==3);committed=1;}
  if(stop>=4)phase=rf_observe(phase,2,1,11,33);
  if(stop>=5){phase=rf_event(phase,3);staged=1;}
  CHECK(rf_ack(phase,2,1,11,33)==staged);
  /* Recovery receives a fresh checkpoint observation and reads an immutable
   * record. A missing record causes unavailability; it never falls back. */
  if(committed){
   CHECK(durable);
   CHECK(rf_match(2,1,11,33,2,1,11,33));
   CHECK(!rf_match(2,1,11,33,1,1,11,22)); /* stale/omitted tombstone */
   CHECK(!rf_match(2,1,11,33,2,1,11,34)); /* forged record */
   CHECK(!rf_match(2,1,11,33,2,2,11,33)); /* unexpected migration */
  } else CHECK(rf_match(1,1,11,22,1,1,11,22));
 }
 /* A observes its candidate then a competing writer advances the checkpoint
  * before A's public installation/acknowledgement: its fresh fenced check refuses. */
 CHECK(!rf_ack(5,3,1,11,44));
 CHECK(!rf_ack(5,1,1,11,22));
 CHECK(!rf_ack(5,0,0,0,0));
 /* Before and after a contended CAS, observing the old value is inconclusive;
  * a different winning checkpoint can never produce an acknowledgement. */
 CHECK(rf_observe(3,1,1,11,22)==3);
 CHECK(rf_observe(3,2,1,11,33)==4);
 CHECK(rf_observe(3,2,1,11,34)==0);
 CHECK(rf_observe(3,0,0,0,0)==0);
 CHECK(!rf_ack(rf_observe(3,1,1,11,22),1,1,11,22));
 CHECK(!rf_ack(rf_observe(3,2,1,11,34),2,1,11,34));
 CHECK(!rf_ack(rf_observe(2,2,1,11,33),2,1,11,33));
 printf("%d recovery policy mock assertions passed\n",checks);
 return 0;
}
#ifdef NATIVE_MAIN
int main(void){return recovery_policy_fixture();}
#endif
