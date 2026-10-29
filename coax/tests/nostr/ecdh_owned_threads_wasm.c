/* Test-only shared-memory contention probe; not a public artifact. */
#include "ecdh_owned.h"
#include <stdint.h>
#include <stdatomic.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static _Atomic uint32_t gates[3];
uint32_t coax_ecdh_owned_test_zero(void *p,int64_t capacity,int64_t offset,int64_t length){
 if(atomic_exchange_explicit(&gates[0],0,memory_order_seq_cst)){
  atomic_store_explicit(&gates[1],1,memory_order_seq_cst);
  while(!atomic_load_explicit(&gates[2],memory_order_seq_cst)){}
 }
 return coax_secure_zero(p,capacity,offset,length);
}
__attribute__((export_name("gate_address"))) uint32_t gate_address(void){return (uint32_t)(uintptr_t)gates;}
