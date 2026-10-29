/* Host pointer validation and observable wiping; Coax owns chunk policy. */
#include "zeroize_large.h"
#include <stddef.h>
#include <stdint.h>

extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);

static int overlaps(const void *a,uint64_t an,const void *b,uint64_t bn){
  if(!a || !b || !an || !bn)return 0;
  uintptr_t x=(uintptr_t)a,y=(uintptr_t)b;
  return x<=y?(uint64_t)(y-x)<an:(uint64_t)(x-y)<bn;
}
uint32_t coax_large_wipe_begin(coax_large_wipe *scope,void *buffer,int64_t length){
  if(!scope || scope->active)return 5;
  coax_t_ZeroLargeChange change=zero_large_begin(length,buffer!=NULL);
  if(change.error)return change.error;
  if(length>0 && (uint64_t)length>UINTPTR_MAX-(uintptr_t)buffer)return 2;
  if(overlaps(scope,sizeof *scope,buffer,(uint64_t)length))return 4;
  scope->policy=change.state;scope->buffer=buffer;scope->active=1;
  return 0;
}
uint32_t coax_large_wipe_step(coax_large_wipe *scope){
  if(!scope || !scope->active)return 5;
  if(zero_large_complete(scope->policy))return 0;
  coax_t_ZeroLargeSpan part=zero_large_next(scope->policy);
  if(part.error)return part.error;
  uint32_t status=coax_secure_zero(scope->buffer+part.offset,part.length,0,part.length);
  coax_t_ZeroLargeChange change=zero_large_ack(scope->policy,part.offset,part.length,status);
  if(change.error)return change.error;
  scope->policy=change.state;
  return zero_large_complete(scope->policy)?0:9;
}
uint32_t coax_large_wipe_reset(coax_large_wipe *scope){
  if(!scope || !scope->active || !zero_large_complete(scope->policy))return 5;
  scope->policy=(coax_t_ZeroLarge){0};scope->buffer=NULL;scope->active=0;
  return 0;
}
