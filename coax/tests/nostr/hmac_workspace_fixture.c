#include "hmac-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[1024],input[4096],output[33];static coax_t_HmacOwned work;
uint8_t *hm_key(void){return key;}uint8_t *hm_input(void){return input;}uint8_t *hm_output(void){return output;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int hm_run(int k,int n){
 if(k<0||k>1024||n<0||n>4096)return 0;memset(&work,165,sizeof work);output[32]=165;
 int ok=hmac_owned((coax_v_u8){key,k},(coax_v_u8){input,n},(coax_v_t_HmacOwned){&work,1},(coax_v_u8){output,33},0);
 return wipe()&&ok&&output[32]==165;
}
int hm_refusal(void){
 memset(&work,165,sizeof work);memset(output,165,sizeof output);
 int ok=hmac_owned((coax_v_u8){key,32},(coax_v_u8){input,1},(coax_v_t_HmacOwned){&work,1},(coax_v_u8){output,31},0);
 if(ok||!wipe())return 0;for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;return 1;
}
