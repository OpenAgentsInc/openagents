/* Flat test ABI for generated Coax header projection. */
#include "generated.c"
#include <stdint.h>
__attribute__((export_name("parse")))
int parse(uint32_t input,uint32_t n,uint32_t output) {
 coax_t_FiProtected h=fi_protected_parse((coax_string){(const char *)(uintptr_t)input,n});
 if(!h.ok)return 0;
 uint8_t *p=(uint8_t *)(uintptr_t)output;
 for(int i=0;i<h.kid_count;i++)p[i]=h.kid.e[i];
 p[128]=(uint8_t)h.kid_count;return (int)h.typ;
}
