/* Test-only byte transport; no host cryptographic arithmetic. */
#include "ecdh.h"
#include <stdint.h>
#include <stddef.h>
_Static_assert(sizeof(coax_t_EcdhScratch)==472, "unexpected workspace size");
_Static_assert(offsetof(coax_t_EcdhScratch,staged)==440, "unexpected staging offset");
__attribute__((export_name("ecdh_test"))) int ecdh_test(uint32_t key,uint32_t peer,uint32_t work){
 return (int)ecdh_stage((coax_v_u8){(uint8_t *)(uintptr_t)key,32},(coax_v_u8){(uint8_t *)(uintptr_t)peer,32},(coax_v_t_EcdhScratch){(coax_t_EcdhScratch *)(uintptr_t)work,1});
}
