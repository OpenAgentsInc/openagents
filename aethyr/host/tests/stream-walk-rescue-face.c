// Exact-build browser checkpoint effects; shared Coax admits compatibility.
#define SW_RESCUE 1
#define SW_PRESENTATION 1
#define SW_LOSS_POLICY 1
#define SW_HEADER "stream-walk-rescue.h"
#include "stream-walk-face.c"
SW_API("sw_rescue_check") int64_t sw_rescue_check(int64_t layout,int64_t bytes,int64_t claimed,int64_t tier,int64_t saved_tier){return rp_admit((coax_v_u8){configuration,512},1,layout,bytes,claimed,tier,saved_tier);}
SW_API("sw_rescue_save_allowed") int32_t sw_rescue_save_allowed(int64_t frames){return rp_save_allowed(frames);}
SW_API("sw_rescue_object") int64_t sw_rescue_object(int32_t row,int32_t field){
  if(row<0||row>=64)return -1;SwPhysical *p=&objects[row];
  switch(field){case 0:return p->occupied;case 1:return p->finished;case 2:return p->key.generation;case 3:return p->key.epoch;default:return -1;}
}
SW_API("sw_rescue_allocated") int32_t sw_rescue_allocated(int32_t row){
  if(row<0||row>=64)return 0;SwPhysical *p=&objects[row];coax_t_ResourceCost c=manifest.entries.e[row].plan.cost;
  return p->occupied&&p->plan&&p->source&&(!c.decoded||p->decoded)&&(!c.decode_scratch||p->work)&&(!c.upload_scratch||p->upload);
}
SW_API("sw_rescue_stop") void sw_rescue_stop(int64_t now){if(gl_lost(lv(),now)==1)gl_finish(lv(),now,loss_policy.serial,false);}
// Deliberate real module trap for this SDK test carrier's recovery qualifier.
SW_API("sw_rescue_trap") void sw_rescue_trap(void){__builtin_trap();}
SW_API("sw_host_rescue_digest") uintptr_t sw_host_rescue_digest(void){return sw_rescue_digest(sv(),(coax_v_u8){device_digest,64})?(uintptr_t)device_digest:0;}
