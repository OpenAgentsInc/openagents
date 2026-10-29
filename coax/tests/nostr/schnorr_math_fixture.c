#include "math-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[32],nonce[32],challenge[32],output[32];static coax_t_SignMath work;static uint64_t valid;
uint8_t *sm_key(void){return key;}uint8_t *sm_nonce(void){return nonce;}uint8_t *sm_challenge(void){return challenge;}uint8_t *sm_output(void){return output;}uint64_t sm_valid(void){return valid;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int sm_run(int keyodd,int nonceodd){
 memset(&work,165,sizeof work);int ok=sm_equation((coax_v_u8){key,32},(coax_v_u8){nonce,32},(coax_v_u8){challenge,32},keyodd,nonceodd,(coax_v_t_SignMath){&work,1},0);valid=work.valid;
 if(ok)for(int i=0;i<8;i++){uint32_t v=work.result.limbs.e[7-i];for(int j=0;j<4;j++)output[i*4+j]=(uint8_t)(v>>((3-j)*8));}
 return wipe()&&ok;
}
int sm_refusal(void){memset(&work,165,sizeof work);int ok=sm_equation((coax_v_u8){key,31},(coax_v_u8){nonce,32},(coax_v_u8){challenge,32},0,0,(coax_v_t_SignMath){&work,1},0);return wipe()&&!ok;}
