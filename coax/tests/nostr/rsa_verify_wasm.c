/* Test-only flat ABI: the verifier is generated from Coax. */
#include "rsa.h"
#include <stdint.h>
__attribute__((export_name("verify")))
uint32_t verify(uint32_t np, uint32_t nn, uint32_t sp, uint32_t sn, uint32_t mp, uint32_t mn) {
 return rsa_verify_sha256((coax_v_u8){(uint8_t*)(uintptr_t)np,nn},
                         (coax_v_u8){(uint8_t*)(uintptr_t)sp,sn},
                         (coax_v_u8){(uint8_t*)(uintptr_t)mp,mn}) ? 1 : 0;
}
