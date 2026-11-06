// Firm reason: 1 (browser glue: concrete queue storage and scalar Coax projections).
#include "socket-carrier.h"
#include <stdbool.h>
#define SOCKET_EXPORT(name) __attribute__((export_name(name)))
static coax_t_HostSocketRetention queues;
static coax_t_HostSocketRetained grant;
static coax_v_t_HostSocketRetention view(void){return(coax_v_t_HostSocketRetention){&queues,1};}
SOCKET_EXPORT("socket_queue_open") uint32_t socket_queue_open(int64_t capacity){queues=hsr_open(capacity);return queues.valid;}
SOCKET_EXPORT("socket_queue_offer") uint32_t socket_queue_offer(uint32_t output,int64_t bytes){grant=hsr_offer(view(),output!=0,bytes);return grant.accepted;}
SOCKET_EXPORT("socket_queue_grant") int64_t socket_queue_grant(int32_t field){switch(field){case 0:return grant.lease.slot;case 1:return grant.lease.serial;case 2:return grant.lease.epoch;case 3:return grant.error;case 4:return grant.close;default:return -1;}}
SOCKET_EXPORT("socket_queue_release") uint32_t socket_queue_release(uint32_t output,int64_t slot,int64_t serial,int64_t epoch){return hsr_release(view(),output!=0,(coax_t_HostInboxLease){.slot=slot,.serial=serial,.epoch=epoch});}
SOCKET_EXPORT("socket_queue_info") int64_t socket_queue_info(uint32_t output,int64_t field){return hsr_info(view(),output!=0,field);}
SOCKET_EXPORT("socket_queue_reset") uint32_t socket_queue_reset(void){return hsr_reset(view());}
SOCKET_EXPORT("socket_queue_span") uint32_t socket_queue_span(uint64_t address,uint64_t capacity,uint64_t bytes,uint64_t limit){return hmp_span(address,capacity,bytes,limit,true);}
SOCKET_EXPORT("socket_queue_plan") int64_t socket_queue_plan(int32_t field){coax_t_HostSocketQueuePlan p=hsq_plan(hsr_info(view(),false,2));switch(field){case 0:return p.workers;case 1:return p.events;default:return -1;}}
SOCKET_EXPORT("socket_queue_failure") int64_t socket_queue_failure(int32_t field){coax_t_HostSocketSend p=hsq_failure();switch(field){case 0:return p.error;case 1:return p.close;default:return -1;}}
SOCKET_EXPORT("socket_queue_continue") uint32_t socket_queue_continue(int64_t handled,int64_t started,int64_t now){return htd_continue(handled,started,now);}
