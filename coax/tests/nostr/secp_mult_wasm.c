/* Test-only flat memory, arithmetic and validation are Coax. */
#include "secp_mult.h"
#include <stdint.h>
#define E(n) __attribute__((export_name(n)))
E("multiply") uint32_t multiply(uint32_t a,uint32_t p,uint32_t b,uint32_t out){
 coax_t_SgResult r=ecmult_vartime(*(coax_t_ScScalar*)(uintptr_t)a,*(coax_t_SgPoint*)(uintptr_t)p,*(coax_t_ScScalar*)(uintptr_t)b);
 *(coax_t_SgPoint*)(uintptr_t)out=r.value;return r.ok?0:1;
}
E("wnaf") int wnaf(uint32_t a,int width,uint32_t out){
 coax_t_SmWnaf r=sm_wnaf_vartime(*(coax_t_ScScalar*)(uintptr_t)a,width);
 for(int i=0;i<257;i++)((int64_t*)(uintptr_t)out)[i]=r.digits.e[i];return r.ok?(int)r.length:-1;
}
