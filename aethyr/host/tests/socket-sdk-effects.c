// Firm reason: 5 (thin C ABI projection of the shared native worker SDK; Coax owns the workload).
#include "SocketEffects.h"
#include "metal-sockets.h"
#include <string.h>
static MNSocketEvent event;
int64_t hse_open(int64_t cap){return(int64_t)(uintptr_t)mn_open(cap);}
bool hse_start(int64_t context,int64_t id,int64_t generation,int64_t engine,coax_v_u8 url){return mn_start((void*)(uintptr_t)context,(MNSocketOwner){id,generation,engine},url.data,(size_t)url.count);}
bool hse_send(int64_t context,int64_t id,int64_t generation,int64_t engine,coax_v_u8 text){return mn_send((void*)(uintptr_t)context,(MNSocketOwner){id,generation,engine},text.data,(size_t)text.count);}
bool hse_next(int64_t context){return mn_next((void*)(uintptr_t)context,&event);}
int64_t hse_field(int64_t field){switch(field){case 0:return event.owner.id;case 1:return event.owner.generation;case 2:return event.owner.engine;case 3:return event.event;case 4:return(int64_t)event.count;}return -1;}
int64_t hse_bytes(coax_v_u8 out){if(event.count>(uint64_t)out.count)return -1;if(event.count)memcpy(out.data,event.bytes,event.count);return(int64_t)event.count;}
int64_t hse_token(void){return(int64_t)(uintptr_t)event.token;}
void hse_done(int64_t token){mn_done((void*)(uintptr_t)token);}
void hse_close(int64_t context){mn_close((void*)(uintptr_t)context);}
int64_t hse_info(int64_t context,int64_t field){return mn_info((void*)(uintptr_t)context,(int32_t)field);}
int64_t hse_pool(int64_t field){return mn_pool_info((int32_t)field);}
void hse_shutdown(void){mn_shutdown();}
