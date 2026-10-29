/* Pointer observation and secure-zero effects; arithmetic remains in Coax. */
#include "ecdh_host.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
#ifdef COAX_ECDH_TEST_EFFECT
extern uint32_t coax_ecdh_test_zero(void *,int64_t,int64_t,int64_t);
#define ECDH_ZERO coax_ecdh_test_zero
#else
#define ECDH_ZERO coax_secure_zero
#endif
static int valid_range(const void *p,uint64_t n){
 return p && n && (uintptr_t)p<=UINTPTR_MAX-(n-1);
}
static int overlaps(const void *a,uint64_t an,const void *b,uint64_t bn){
 uintptr_t x=(uintptr_t)a,y=(uintptr_t)b;return x<=y?(uint64_t)(y-x)<an:(uint64_t)(x-y)<bn;
}
uint32_t coax_ecdh_cleanup(coax_t_EcdhScratch *work,uint8_t *output,int64_t capacity){
 if(!valid_range(work,sizeof *work) || !valid_range(output,32) || capacity<32 || ((uintptr_t)work%_Alignof(coax_t_EcdhScratch)) || overlaps(work,sizeof *work,output,32))return COAX_ECDH_METADATA;
 uint32_t a=ECDH_ZERO(work,sizeof *work,0,sizeof *work);uint32_t b=ECDH_ZERO(output,32,0,32);
 return (a||b)?COAX_ECDH_CLEANUP_PENDING:0;
}
uint32_t coax_ecdh_raw(const uint8_t *secret,int64_t secret_len,const uint8_t *peer,int64_t peer_len,uint8_t *output,int64_t capacity,coax_t_EcdhScratch *work){
 int present=valid_range(secret,32)&&valid_range(peer,32)&&valid_range(output,32)&&valid_range(work,sizeof *work);
 int aligned=present && ((uintptr_t)work%_Alignof(coax_t_EcdhScratch))==0;
 int alias=!present || overlaps(work,sizeof *work,secret,32)||overlaps(work,sizeof *work,peer,32)||overlaps(work,sizeof *work,output,32)||overlaps(output,32,secret,32)||overlaps(output,32,peer,32)||overlaps(secret,32,peer,32);
 uint32_t admission=ecdh_admit(secret_len,peer_len,capacity,present,aligned,alias);
 if(admission)return admission;
 uint32_t status=ecdh_stage_bridge((coax_v_u8){(uint8_t *)secret,32},(coax_v_u8){(uint8_t *)peer,32},(coax_v_t_EcdhScratch){work,1});
 if(!status)memcpy(output,work->staged.e,32);
 if(ECDH_ZERO(work,sizeof *work,0,sizeof *work)){
  if(!status)(void)ECDH_ZERO(output,32,0,32);
  return COAX_ECDH_CLEANUP_PENDING;
 }
 return status;
}
