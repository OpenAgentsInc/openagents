/* Test-only ABI around the Coax fixed-work self-verifier, for native and
   Wasm. The work record is pre-filled with 0xA5 to stand in for a stale or
   faulted workspace; after the call it must be all zero (cleared path) or
   untouched (refused views). Guard bytes on both sides must survive. */
#include "schnorr-self-verify.h"
#include <stdint.h>
#include <string.h>
#ifdef __wasm__
#define EXPORT(n) __attribute__((export_name(n)))
#else
#define EXPORT(n) __attribute__((visibility("default")))
#endif
/* One-shot loop over the staged API: at most three calls. */
static unsigned char region[sizeof(coax_t_SsvWork)+64] __attribute__((aligned(16)));
EXPORT("ssv_case") int ssv_case(const uint8_t *m,int64_t mn,const uint8_t *p,int64_t pn,const uint8_t *s,int64_t sn,int nostr,int overlap,int64_t work_count,int garbage_stage) {
  memset(region,0xA5,sizeof region);
  coax_t_SsvWork *work=(coax_t_SsvWork *)(region+32);
  /* A stale workspace starts at stage 0 unless the case faults the stage too. */
  if(!garbage_stage)work->stage=0;
  coax_v_u8 message={(uint8_t *)m,mn},pub={(uint8_t *)p,pn},sig={(uint8_t *)s,sn},r={(uint8_t *)s,sn<32?sn:32};
  coax_v_t_SsvWork view={work,work_count};
  uint64_t mask;int calls=0;
  do{mask=nostr?ssv_nostr_step(message,pub,r,sig,view,overlap!=0):ssv_step(message,pub,r,sig,view,overlap!=0);calls++;}while(mask==1&&calls<8);
  if(calls>3)return -3;
  for(int i=0;i<32;i++)if(region[i]!=0xA5||region[32+sizeof(coax_t_SsvWork)+i]!=0xA5)return -1;
  int zero=1,untouched=1;
  for(size_t i=0;i<sizeof(coax_t_SsvWork);i++){if(region[32+i]!=0)zero=0;if(region[32+i]!=0xA5&&(garbage_stage||i>=sizeof work->stage))untouched=0;}
  if(mask!=0&&mask!=UINT64_MAX)return -2;
  /* bit0 valid, bit1 cleared, bit2 untouched */
  return (mask==UINT64_MAX?1:0)|(zero?2:0)|(untouched?4:0);
}
EXPORT("old_verify") int old_verify(const uint8_t *m,int64_t mn,const uint8_t *p,int64_t pn,const uint8_t *s,int64_t sn) {
  coax_v_u8 message={(uint8_t *)m,mn},pub={(uint8_t *)p,pn},sig={(uint8_t *)s,sn};
  return schnorr_verify_vartime(message,pub,sig);
}
EXPORT("ssv_work_size") int ssv_work_size(void){return (int)sizeof(coax_t_SsvWork);}
