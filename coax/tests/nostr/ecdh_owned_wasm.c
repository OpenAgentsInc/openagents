#include "ecdh_owned.h"
#include <stdint.h>
__attribute__((export_name("owned_test"))) uint32_t owned_test(uint32_t key,uint32_t peer,uint32_t output){
 uint64_t h=coax_ecdh_owned_acquire();if(!h)return 255;
 uint32_t result=coax_ecdh_owned_start(h,(const uint8_t *)(uintptr_t)key,32,(const uint8_t *)(uintptr_t)peer,32);
 if(result==COAX_ECDH_OWNED_PENDING)result=coax_ecdh_owned_collect(h,(uint8_t *)(uintptr_t)output,32);
 if(coax_ecdh_owned_close(h)!=0)return 254;return result;
}
