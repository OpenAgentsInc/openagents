/* Scalar test-only transport; all algorithm/admission behavior remains Coax. */
#include "chacha20.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static int64_t last_written;
static coax_v_u8 bytes(uint32_t p,int32_t n){return(coax_v_u8){(uint8_t *)(uintptr_t)p,n};}
EXPORT("cc_block") uint32_t test_block(uint32_t key,int32_t kn,uint32_t nonce,int32_t nn,uint32_t counter,uint32_t out,int32_t capacity){
  coax_t_ChaChaResult r=chacha20_block(bytes(key,kn),bytes(nonce,nn),counter,bytes(out,capacity));last_written=r.written;return r.error;
}
EXPORT("cc_xor") uint32_t test_xor(uint32_t key,int32_t kn,uint32_t nonce,int32_t nn,uint32_t counter,uint32_t input,int32_t n,uint32_t out,int32_t capacity){
  coax_t_ChaChaResult r=chacha20_xor(bytes(key,kn),bytes(nonce,nn),counter,bytes(input,n),bytes(out,capacity));last_written=r.written;return r.error;
}
EXPORT("cc_written") int64_t written(void){return last_written;}
