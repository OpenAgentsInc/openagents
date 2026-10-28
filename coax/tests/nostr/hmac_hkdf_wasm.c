/* Flat test-only Wasm bindings; cryptography and policy remain Coax. */
#include "hkdf.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p,uint32_t n){return(coax_v_u8){(uint8_t *)(uintptr_t)p,n};}
EXPORT("mac") uint32_t mac(uint32_t k,uint32_t kn,uint32_t m,uint32_t mn,uint32_t o){return hmac_sha256(bytes(k,kn),bytes(m,mn),bytes(o,32));}
EXPORT("extract") uint32_t extract(uint32_t s,uint32_t sn,uint32_t k,uint32_t kn,uint32_t o){return hkdf_extract(bytes(s,sn),bytes(k,kn),bytes(o,32));}
EXPORT("expand") uint32_t expand(uint32_t p,uint32_t pn,uint32_t i,uint32_t in,uint32_t o,int32_t n){return hkdf_expand(bytes(p,pn),bytes(i,in),bytes(o,8192),n);}
