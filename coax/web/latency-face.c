/* Browser SDK scalar bridge to Coax. Each concrete socket instantiates its own
   module; no policy or matching lives in this C face. */
#include <stdint.h>
#include "latency-client.h"
#define EX(n) __attribute__((export_name(n)))
static coax_t_LatClient state;
static uint8_t inbox[4300],outbox[256];
static coax_v_t_LatClient view(void){return(coax_v_t_LatClient){&state,1};}
EX("lt_init") void lt_init(void){coax_default_t_LatClient(&state);}
EX("lt_inbox") uint32_t lt_inbox(void){return(uint32_t)(uintptr_t)inbox;}
EX("lt_outbox") uint32_t lt_outbox(void){return(uint32_t)(uintptr_t)outbox;}
EX("lt_before") int32_t lt_before(int32_t n,int64_t now){if(n<0||n>(int32_t)sizeof inbox)return 0;return(int32_t)ltc_before(view(),(coax_string){(const char*)inbox,n},now,(coax_v_u8){outbox,256});}
EX("lt_receive") void lt_receive(int32_t n,int64_t now){if(n>=0&&n<=(int32_t)sizeof inbox)ltc_receive(view(),(coax_string){(const char*)inbox,n},now);}
EX("lt_applied") void lt_applied(int64_t ack,int64_t tick,int64_t now,int64_t lag){ltc_applied(view(),ack,tick,now,lag);}
EX("lt_submit") int64_t lt_submit(int64_t now,int64_t lag){return ltc_submit(view(),now,lag);}
EX("lt_complete") void lt_complete(int64_t id,int64_t now){ltc_complete(view(),id,now);}
EX("lt_retire") void lt_retire(void){ltc_retire(view());}
EX("lt_value") int64_t lt_value(int32_t row,int32_t field){return ltc_value(view(),row,field);}
EX("lt_count") int32_t lt_count(void){return(int32_t)state.count;}
EX("lt_expired") int64_t lt_expired(void){return state.expired;}
EX("lt_refused") int64_t lt_refused(void){return state.refused;}
EX("lt_missing") int64_t lt_missing(void){return state.missing;}
