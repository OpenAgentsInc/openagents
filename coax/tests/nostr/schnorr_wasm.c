/* Test-only byte-view ABI. Verification logic stays in Coax. */
#include "schnorr.h"
#include <stdint.h>
__attribute__((export_name("verify"))) uint32_t verify(uint32_t m,int mn,uint32_t p,int pn,uint32_t s,int sn,int nostr){
 coax_v_u8 message={(uint8_t*)(uintptr_t)m,mn},pub={(uint8_t*)(uintptr_t)p,pn},sig={(uint8_t*)(uintptr_t)s,sn};
 return nostr?nostr_verify_vartime(message,pub,sig):schnorr_verify_vartime(message,pub,sig);
}
