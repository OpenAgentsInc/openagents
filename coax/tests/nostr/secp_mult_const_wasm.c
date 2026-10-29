/* Test-only flat-memory transport; no host arithmetic. */
#include "secp_mult_const.h"
#include <stdint.h>
__attribute__((export_name("variable_base"))) int variable_base(uint32_t scalar,uint32_t peer,uint32_t out){
  coax_t_SgResult r=smc_multiply(*(coax_t_ScScalar*)(uintptr_t)scalar,*(coax_t_SgPoint*)(uintptr_t)peer);
  *(coax_t_SgPoint*)(uintptr_t)out=r.value;return r.ok;
}
