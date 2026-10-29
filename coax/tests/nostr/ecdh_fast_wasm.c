#include "ecdh_fast_adapter.h"
#include <stdint.h>
__attribute__((export_name("field"))) void field(uint32_t op,uint32_t a,uint32_t b,uint32_t out){
 coax_t_Sf32 x=*(coax_t_Sf32 *)(uintptr_t)a,y=*(coax_t_Sf32 *)(uintptr_t)b;
 *(coax_t_Sf32 *)(uintptr_t)out=op==0?ef_test_add(x,y):op==1?ef_test_mul(x,y):ef_test_inverse(x);
}
