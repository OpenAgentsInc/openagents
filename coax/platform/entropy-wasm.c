/* Scalar wasm ABI over the same Coax policy; no RNG or policy fallback here. */
#include "entropy.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
EXPORT("entropy_admit") uint32_t face_entropy_admit(int64_t n,int64_t cap,int64_t scratch,uint32_t valid,uint32_t overlap) {
  return entropy_admit(n,cap,scratch,valid!=0,overlap!=0);
}
EXPORT("entropy_chunk_bytes") int64_t face_entropy_chunk_bytes(void){return entropy_chunk_bytes();}
EXPORT("entropy_full_chunks") int64_t face_entropy_full_chunks(int64_t n){return entropy_full_chunks(n);}
EXPORT("entropy_tail") int64_t face_entropy_tail(int64_t n){return entropy_tail(n);}
EXPORT("entropy_finish") uint32_t face_entropy_finish(uint32_t out,uint32_t scratch,int64_t cap,int64_t scratch_cap,int64_t n,uint32_t valid,uint32_t overlap,uint32_t available,uint32_t ok,int64_t written) {
  coax_v_u8 a={(uint8_t *)(uintptr_t)out,cap},b={(uint8_t *)(uintptr_t)scratch,scratch_cap};
  return entropy_finish(n,a,b,valid!=0,overlap!=0,available!=0,ok!=0,written).error;
}
