/* Test-only transport. An ok result is not event verification or admission. */
#include "store-rewrite.h"
#include <stdint.h>
#include <stdbool.h>
__attribute__((export_name("rewrite"))) int rewrite(uint32_t old,int64_t old_n,uint32_t text,int64_t text_n,uint32_t flags,int64_t flag_n,uint32_t out,int64_t out_n,int overlap) {
 return store_rewrite_test((coax_v_u8){(uint8_t *)(uintptr_t)old,old_n},(coax_string){(char *)(uintptr_t)text,text_n},(coax_v_bool){(bool *)(uintptr_t)flags,flag_n},(coax_v_u8){(uint8_t *)(uintptr_t)out,out_n},overlap!=0).ok;
}
