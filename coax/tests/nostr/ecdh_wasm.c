/* Test-only byte transport; no host cryptographic arithmetic. */
#include "ecdh.h"
#include <stdint.h>
__attribute__((export_name("ecdh_test"))) int ecdh_test(uint32_t key,uint32_t peer,uint32_t work){
 return (int)ecdh_stage((coax_v_u8){(uint8_t *)(uintptr_t)key,32},(coax_v_u8){(uint8_t *)(uintptr_t)peer,32},(coax_v_t_EcdhScratch){(coax_t_EcdhScratch *)(uintptr_t)work,1});
}
