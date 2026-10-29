/* Test-only flat-memory transport. No host arithmetic. */
#include "secp_base.h"
#include <stdint.h>
#define E(n) __attribute__((export_name(n)))
E("base_web") void base_web(uint32_t scalar,uint32_t out){
 *(coax_t_SgPoint*)(uintptr_t)out=smb_multiply_web(*(coax_t_ScScalar*)(uintptr_t)scalar);
}
E("base_native") void base_native(uint32_t scalar,uint32_t out){
 *(coax_t_SgPoint*)(uintptr_t)out=smb_multiply_native(*(coax_t_ScScalar*)(uintptr_t)scalar);
}
