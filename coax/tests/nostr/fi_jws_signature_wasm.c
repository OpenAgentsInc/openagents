#include "generated.c"
#include <stdint.h>
__attribute__((export_name("verify")))
int verify(uint32_t token,uint32_t token_n,uint32_t modulus,uint32_t modulus_n) {
 coax_t_FiSignatureCheck r=fi_jws_signature((coax_v_u8){(uint8_t *)(uintptr_t)token,token_n},(coax_v_u8){(uint8_t *)(uintptr_t)modulus,modulus_n});
 return r.verified?1:0;
}
__attribute__((export_name("rsa")))
int rsa(uint32_t np,uint32_t nn,uint32_t sp,uint32_t sn,uint32_t mp,uint32_t mn) {
 return rsa_verify_sha256((coax_v_u8){(uint8_t *)(uintptr_t)np,nn},(coax_v_u8){(uint8_t *)(uintptr_t)sp,sn},(coax_v_u8){(uint8_t *)(uintptr_t)mp,mn})?1:0;
}
