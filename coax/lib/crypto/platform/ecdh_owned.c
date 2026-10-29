/* Experimental private one-slot host. Coax owns the public transition policy. */
#include "ecdh_owned.h"
#include "ecdh.h"
#include "ecdh_lifecycle.h"
#include <stdatomic.h>
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
#ifdef COAX_ECDH_OWNED_TEST_EFFECT
extern uint32_t coax_ecdh_owned_test_zero(void *,int64_t,int64_t,int64_t);
#define OWNED_ZERO coax_ecdh_owned_test_zero
#else
#define OWNED_ZERO coax_secure_zero
#endif
enum { FREE=0, READY=1, RUNNING=2, CLEANING=3, OUTPUT=4, DONE=5 };
enum { WORK=1, STAGE=2, OUT=4 };
typedef struct {
 coax_t_ElSlot policy;
 coax_t_EcdhScratch work;
 uint8_t staged[32];
 uint8_t *output;
 const uint8_t *secret, *peer;
 uint32_t outcome;
} OwnedSlot;
static OwnedSlot slot;
static atomic_flag mutex=ATOMIC_FLAG_INIT;
static int lock_slot(void){return !atomic_flag_test_and_set_explicit(&mutex,memory_order_acquire);}
static void unlock_slot(void){atomic_flag_clear_explicit(&mutex,memory_order_release);}
static int span(const void *p,uint64_t n){return p && n && (uintptr_t)p<=UINTPTR_MAX-(n-1);}
static int overlap(const void *a,uint64_t an,const void *b,uint64_t bn){uintptr_t x=(uintptr_t)a,y=(uintptr_t)b;return x<=y?(uint64_t)(y-x)<an:(uint64_t)(x-y)<bn;}
static int same(uint64_t h){return h && h==slot.policy.generation && slot.policy.phase!=FREE;}
static uint32_t incomplete(void){return COAX_ECDH_OWNED_PENDING;}
/* Acknowledgement follows the actual complete region wipe; failure switches
 * the policy to cancellation and retains every unacknowledged region. */
static uint32_t drain(uint64_t h){
 while(slot.policy.phase==CLEANING){
  uint32_t region=slot.policy.dirty&WORK?WORK:slot.policy.dirty&STAGE?STAGE:OUT;
  void *p=region==WORK?(void *)&slot.work:region==STAGE?(void *)slot.staged:(void *)slot.output;
  int64_t n=region==WORK?(int64_t)sizeof slot.work:32;
  if(region==OUT && slot.policy.reason==0)break;
  uint32_t wiped=OWNED_ZERO(p,n,0,n);
  coax_t_ElChange change=el_wipe(slot.policy,h,region,wiped==0);
  if(change.status!=0 && change.status!=2)return incomplete();
  slot.policy=change.slot;
  if(wiped){slot.outcome=COAX_ECDH_OWNED_EFFECT;return incomplete();}
 }
 if(slot.policy.phase==DONE)return slot.outcome;
 return incomplete();
}
uint64_t coax_ecdh_owned_acquire(void){
 if(!lock_slot())return 0;
 coax_t_ElChange c=el_open(slot.policy);uint64_t h=0;
 if(c.status==0){slot.policy=c.slot;slot.outcome=0;slot.output=0;slot.secret=0;slot.peer=0;h=c.slot.generation;}
 unlock_slot();return h;
}
uint32_t coax_ecdh_owned_start(uint64_t h,const uint8_t *secret,int64_t sn,const uint8_t *peer,int64_t pn){
 if(!lock_slot())return COAX_ECDH_OWNED_BUSY;
 if(!same(h)){unlock_slot();return COAX_ECDH_OWNED_INVALID;}
 if(slot.policy.phase!=READY){unlock_slot();return incomplete();}
 if(sn!=32 || pn!=32 || !span(secret,32) || !span(peer,32) || overlap(secret,32,peer,32) || overlap(secret,32,&slot,sizeof slot) || overlap(peer,32,&slot,sizeof slot) || overlap(secret,32,&mutex,sizeof mutex) || overlap(peer,32,&mutex,sizeof mutex)){
  unlock_slot();return COAX_ECDH_OWNED_INVALID;
 }
 slot.secret=secret;slot.peer=peer;
 coax_t_ElChange c=el_begin(slot.policy,h);if(c.status){unlock_slot();return incomplete();}slot.policy=c.slot;
 uint32_t result=ecdh_stage_bridge((coax_v_u8){(uint8_t *)secret,32},(coax_v_u8){(uint8_t *)peer,32},(coax_v_t_EcdhScratch){&slot.work,1});
 if(!result)memcpy(slot.staged,slot.work.staged.e,32);
 slot.outcome=result;
 c=el_stop(slot.policy,h,result?1:0);if(c.status!=2){unlock_slot();return incomplete();}slot.policy=c.slot;
 uint32_t answer=drain(h);unlock_slot();return answer;
}
uint32_t coax_ecdh_owned_collect(uint64_t h,uint8_t *output,int64_t capacity){
 if(!lock_slot())return COAX_ECDH_OWNED_BUSY;
 if(!same(h)){unlock_slot();return COAX_ECDH_OWNED_INVALID;}
 if(slot.policy.phase!=OUTPUT || capacity<32 || !span(output,32) || overlap(output,32,&slot,sizeof slot) || overlap(output,32,&mutex,sizeof mutex) || overlap(output,32,slot.secret,32) || overlap(output,32,slot.peer,32)){
  unlock_slot();return incomplete();
 }
 /* Register output before touching its first byte. */
 slot.output=output;coax_t_ElChange c=el_copy(slot.policy,h);if(c.status!=2){unlock_slot();return incomplete();}slot.policy=c.slot;
 memcpy(output,slot.staged,32);
 uint32_t answer=drain(h);
 if(slot.policy.phase==CLEANING && slot.policy.reason==0 && slot.policy.dirty==OUT){
  c=el_publish(slot.policy,h);if(c.status==0){slot.policy=c.slot;answer=COAX_ECDH_OWNED_OK;}
 }
 unlock_slot();return answer;
}
uint32_t coax_ecdh_owned_retry(uint64_t h){
 if(!lock_slot())return COAX_ECDH_OWNED_BUSY;if(!same(h)){unlock_slot();return COAX_ECDH_OWNED_INVALID;}
 uint32_t answer=drain(h);unlock_slot();return answer;
}
uint32_t coax_ecdh_owned_cancel(uint64_t h){
 if(!lock_slot())return COAX_ECDH_OWNED_BUSY;if(!same(h)){unlock_slot();return COAX_ECDH_OWNED_INVALID;}
 if(slot.policy.phase==READY){coax_t_ElChange c=el_close(slot.policy,h);if(c.status){unlock_slot();return incomplete();}slot.policy=c.slot;unlock_slot();return COAX_ECDH_OWNED_CANCELLED;}
 if(slot.policy.phase==DONE){uint32_t result=slot.outcome;unlock_slot();return result;}
 coax_t_ElChange c=el_cancel(slot.policy,h);if(c.status!=2){unlock_slot();return incomplete();}slot.policy=c.slot;slot.outcome=COAX_ECDH_OWNED_CANCELLED;
 uint32_t result=drain(h);unlock_slot();return result;
}
uint32_t coax_ecdh_owned_close(uint64_t h){
 if(!lock_slot())return COAX_ECDH_OWNED_BUSY;if(!same(h)){unlock_slot();return COAX_ECDH_OWNED_INVALID;}
 coax_t_ElChange c=el_close(slot.policy,h);if(c.status){unlock_slot();return incomplete();}
 slot.policy=c.slot;slot.output=0;slot.secret=0;slot.peer=0;slot.outcome=0;unlock_slot();return 0;
}
