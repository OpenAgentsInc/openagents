/* Flat test-only Wasm ABI; no crypto or policy lives in this face. */
#include "sha256.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p,uint32_t n){return (coax_v_u8){(uint8_t *)(uintptr_t)p,n};}
EXPORT("sha_hash") uint32_t test_hash(uint32_t p,uint32_t n,uint32_t out){return sha256_hash(bytes(p,n),bytes(out,32));}
EXPORT("sha_stream") uint32_t test_stream(uint32_t p,uint32_t n,uint32_t chunk,uint32_t out){
  uint8_t state[112];coax_v_u8 s={state,112};if(!chunk)return 2;
  uint32_t status=sha256_init(s);if(status)return status;
  for(uint32_t at=0;at<n;){uint32_t take=n-at<chunk?n-at:chunk;status=sha256_update(s,bytes(p+at,take));if(status)return status;at+=take;}
  return sha256_final(s,bytes(out,32));
}
EXPORT("sha_tag") uint32_t test_tag(int32_t tag,uint32_t p,uint32_t n,uint32_t out){return sha256_bip340(tag,bytes(p,n),bytes(out,32));}
