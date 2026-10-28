/* Platform effects and pointer observations only. Coax owns all admission,
 * copy registration, cleanup states and completion/error decisions. */
#include "zeroize.h"
#include <stdatomic.h>
#include <stddef.h>
#include <string.h>
_Static_assert(sizeof(((coax_t_ZeroScope *)0)->records.e)/sizeof(coax_t_ZeroRecord)==8,"scope slot ABI");

/* Volatile byte writes are observable C effects, including under LTO.
 * This does not erase registers, hidden compiler copies or OS snapshots. */
static uint32_t erase_range(volatile uint8_t *buffer,size_t count){
    for(size_t i=0;i<count;i++)buffer[i]=0;
    atomic_signal_fence(memory_order_seq_cst);
    return 0;
}
uint32_t coax_secure_zero(void *buffer,int64_t capacity,int64_t offset,int64_t length){
    uint32_t status=zero_admit(capacity,offset,length,buffer!=NULL);
    if(status)return status;
    if(!length)return 0; /* No arithmetic on a possibly null empty pointer. */
    return erase_range((volatile uint8_t *)buffer+(size_t)offset,(size_t)length);
}
static int overlap(const void *a,int64_t an,const void *b,int64_t bn){
    if(!a||!b||an<=0||bn<=0)return 0;
    uintptr_t x=(uintptr_t)a,y=(uintptr_t)b;
    return x<=y?(uint64_t)(y-x)<(uint64_t)an:(uint64_t)(x-y)<(uint64_t)bn;
}
uint32_t coax_secret_register(coax_secret_scope *scope,uint64_t id,void *buffer,int64_t capacity,int64_t secret_prefix){
    if(!scope)return 5;
    /* The policy shape is checked before reading its bounded pointer table. */
    if(!zero_scope_valid(scope->policy))return 5;
    int alias=0;
    for(int64_t i=0;i<scope->policy.count;i++)alias|=overlap(buffer,capacity,scope->buffers[i],scope->policy.records.e[i].capacity);
    coax_t_ZeroChange change=zero_scope_register(scope->policy,id,capacity,secret_prefix,buffer!=NULL,alias!=0);
    if(change.error)return change.error;
    scope->buffers[change.slot]=buffer;scope->policy=change.scope;
    return 0;
}
uint32_t coax_secret_copy(coax_secret_scope *scope,uint64_t source,uint64_t destination,int64_t length){
    if(!scope)return 5;
    int64_t from=zero_scope_slot(scope->policy,source),to=zero_scope_slot(scope->policy,destination);
    /* Unknown slots are passed as facts to Coax, never dereferenced here. */
    int alias=from>=0&&to>=0?overlap(scope->buffers[from],length,scope->buffers[to],length):0;
    coax_t_ZeroChange change=zero_scope_copy(scope->policy,source,destination,length,alias!=0);
    if(change.error)return change.error;
    /* Register the dirty destination before transfer, so partial copy failure
     * or a cooperative cancellation cannot lose its cleanup obligation. */
    scope->policy=change.scope;
    if(length)memcpy(scope->buffers[change.slot],scope->buffers[from],(size_t)length);
    return 0;
}
uint32_t coax_secret_close(coax_secret_scope *scope,uint32_t reason){
    if(!scope)return 5;
    coax_t_ZeroChange change=zero_scope_begin(scope->policy,reason);
    if(change.error)return change.error;
    scope->policy=change.scope;
    for(int64_t slot=0;slot<8;slot++){
        int64_t count=zero_scope_pending(scope->policy,slot);
        if(count){
            uint32_t status=coax_secure_zero(scope->buffers[slot],scope->policy.records.e[slot].capacity,0,count);
            change=zero_scope_ack(scope->policy,scope->policy.records.e[slot].id,status);
            scope->policy=change.scope;
        }
    }
    return zero_scope_status(scope->policy);
}
uint32_t coax_secret_reset(coax_secret_scope *scope){
    if(!scope)return 5;
    coax_t_ZeroChange change=zero_scope_reset(scope->policy);
    if(change.error)return change.error;
    scope->policy=change.scope;
    for(int i=0;i<8;i++)scope->buffers[i]=NULL;
    return 0;
}
