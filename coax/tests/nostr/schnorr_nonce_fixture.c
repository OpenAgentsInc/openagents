#include "nonce-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[32],pub[32],aux[32],message[1048576],output[33];static coax_t_SnNonce work;
uint8_t *sn_key(void){return key;}uint8_t *sn_pub(void){return pub;}uint8_t *sn_aux(void){return aux;}uint8_t *sn_message(void){return message;}uint8_t *sn_output(void){return output;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int sn_run(int n){if(n<0||n>1048576)return 0;memset(&work,165,sizeof work);output[32]=165;int ok=sn_nonce((coax_v_u8){key,32},(coax_v_u8){pub,32},(coax_v_u8){aux,32},(coax_v_u8){message,n},(coax_v_t_SnNonce){&work,1},(coax_v_u8){output,33},0);return wipe()&&ok&&output[32]==165;}
int sn_refusal(int mode){memset(&work,165,sizeof work);memset(output,165,sizeof output);int ok=sn_nonce((coax_v_u8){key,mode==0?31:32},(coax_v_u8){pub,mode==1?31:32},(coax_v_u8){aux,mode==2?31:32},(coax_v_u8){message,1},(coax_v_t_SnNonce){&work,1},(coax_v_u8){output,mode==3?31:33},mode==4);if(ok||!wipe())return 0;for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;return 1;}
