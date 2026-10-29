/* Test-only host: erase owned state on both success and refusal. */
#include "xc-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[32],nonce[24],input[1024],output[1025],derived[32];
static coax_t_XcWorkspace work;
uint8_t *xc_key(void){return key;}uint8_t *xc_nonce(void){return nonce;}uint8_t *xc_input(void){return input;}uint8_t *xc_output(void){return output;}uint8_t *xc_subkey(void){return derived;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int xc_derive(void){memset(&work,0xa5,sizeof work);coax_t_ChaChaResult r=xc_hchacha((coax_v_u8){key,32},(coax_v_u8){nonce,16},(coax_v_t_XcWorkspace){&work,1},(coax_v_u8){derived,32},0);return wipe()&&r.error==0&&r.written==32;}
int xc_run(int n,uint32_t counter){
 memset(&work,0xa5,sizeof work);output[n]=165;
 coax_t_ChaChaResult r=xc_xor((coax_v_u8){key,32},(coax_v_u8){nonce,24},counter,(coax_v_u8){input,n},(coax_v_t_XcWorkspace){&work,1},(coax_v_u8){output,n+1},0);
 return wipe()&&r.error==0&&r.written==n&&output[n]==165;
}
int xc_refusal(int mode){
 memset(&work,0xa5,sizeof work);memset(output,165,sizeof output);
 coax_t_ChaChaResult r=xc_xor((coax_v_u8){key,mode==0?31:32},(coax_v_u8){nonce,mode==1?23:24},mode==4?UINT32_MAX:0,(coax_v_u8){input,65},(coax_v_t_XcWorkspace){&work,1},(coax_v_u8){output,mode==2?64:1025},mode==3);
 if(!wipe()||!r.error||r.written)return 0;for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;return 1;
}
