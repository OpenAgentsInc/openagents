#include "nonce-point.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[32],pub[32],aux[32],message[1048576],r[32],hash[32];static uint64_t valid,odd;static coax_t_SignNoncePoint work;
uint8_t *np_key(void){return key;}uint8_t *np_pub(void){return pub;}uint8_t *np_aux(void){return aux;}uint8_t *np_message(void){return message;}uint8_t *np_r(void){return r;}uint8_t *np_hash(void){return hash;}uint64_t np_valid(void){return valid;}uint64_t np_odd(void){return odd;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int np_run(int n){if(n<0||n>1048576)return 0;memset(&work,165,sizeof work);int ok=snp_prepare((coax_v_u8){key,32},(coax_v_u8){pub,32},(coax_v_u8){aux,32},(coax_v_u8){message,n},(coax_v_t_SignNoncePoint){&work,1},0);valid=work.valid;odd=work.point.e[0].odd;if(ok){memcpy(r,work.r.e,32);memcpy(hash,work.hash.e,32);}return wipe()&&ok;}
int np_refusal(void){memset(&work,165,sizeof work);int ok=snp_prepare((coax_v_u8){key,31},(coax_v_u8){pub,32},(coax_v_u8){aux,32},(coax_v_u8){message,1},(coax_v_t_SignNoncePoint){&work,1},0);return wipe()&&!ok;}
