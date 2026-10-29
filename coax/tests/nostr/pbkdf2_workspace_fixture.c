#include "pb-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t password[1024],salt[1024],output[1025];static coax_t_PbkdfOwned work;
uint8_t *pb_password(void){return password;}uint8_t *pb_salt(void){return salt;}uint8_t *pb_output(void){return output;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int pb_run(int pn,int sn,int n){
 if(pn<0||pn>1024||sn<0||sn>1024||n<1||n>1024)return 0;memset(&work,165,sizeof work);output[n]=165;
 int ok=pbkdf_owned((coax_v_u8){password,pn},(coax_v_u8){salt,sn},n,(coax_v_t_PbkdfOwned){&work,1},(coax_v_u8){output,n+1},0);
 return wipe()&&ok&&output[n]==165;
}
int pb_refusal(int mode){
 memset(&work,165,sizeof work);memset(output,165,sizeof output);
 int ok=pbkdf_owned((coax_v_u8){password,mode==0?1025:1},(coax_v_u8){salt,mode==1?1025:1},mode==2?0:mode==3?1025:32,(coax_v_t_PbkdfOwned){&work,1},(coax_v_u8){output,mode==4?31:1025},0);
 if(ok||!wipe())return 0;for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;return 1;
}
