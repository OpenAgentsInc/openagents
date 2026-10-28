/* Scalar test transport only; canonicalization and all policy remain Coax. */
#include "jcs.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static int64_t last_written;
EXPORT("canonicalize") uint32_t canonicalize(uint32_t input,int32_t n,uint32_t out,int32_t capacity){
  coax_string s={(const uint8_t *)(uintptr_t)input,n};coax_v_u8 o={(uint8_t *)(uintptr_t)out,capacity};
  coax_t_JcsResult r=jcs_canonicalize(s,o);last_written=r.written;return r.error;
}
EXPORT("written") int64_t written(void){return last_written;}
