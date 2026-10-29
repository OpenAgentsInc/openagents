/* Test-only transport; admission stays in Coax. */
#include "store-admit.h"
#include <stdint.h>
__attribute__((export_name("admit"))) uint32_t admit(uint32_t text,int64_t length,uint32_t scratch,int64_t cap,uint32_t out,int64_t outcap,int overlap){
 return ns_admit_one((coax_string){(const char *)(uintptr_t)text,length},(coax_v_u8){(uint8_t *)(uintptr_t)scratch,cap},(coax_v_u8){(uint8_t *)(uintptr_t)out,outcap},overlap!=0).status;
}
