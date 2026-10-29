#include "prepare-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[32],pub[32],normalized[32];static uint64_t valid;static coax_t_SignPrepare work;
uint8_t *sk_key(void){return key;}uint8_t *sk_public(void){return pub;}uint8_t *sk_normalized(void){return normalized;}uint64_t sk_valid(void){return valid;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int sk_run(void){memset(&work,165,sizeof work);int ok=sk_prepare((coax_v_u8){key,32},(coax_v_t_SignPrepare){&work,1},0);valid=work.valid;if(ok){memcpy(pub,work.public_key.e,32);memcpy(normalized,work.normalized.e,32);}return wipe()&&ok;}
int sk_refusal(void){memset(&work,165,sizeof work);int ok=sk_prepare((coax_v_u8){key,31},(coax_v_t_SignPrepare){&work,1},0);return wipe()&&!ok;}
