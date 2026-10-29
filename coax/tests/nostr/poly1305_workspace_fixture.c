/* Test-only host: the workspace is wiped after both success and refusal. */
#include "poly-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[32],message[1048576],output[17];
static coax_t_PolyWorkspace work;
uint8_t *poly_key(void){return key;}
uint8_t *poly_message(void){return message;}
uint8_t *poly_output(void){return output;}
static int wipe(void){
 if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;
 const uint8_t *p=(const uint8_t *)&work;
 for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;
 return 1;
}
int poly_run(int n){
 memset(&work,0xa5,sizeof work);output[16]=165;
 int ok=poly_workspace((coax_v_u8){key,32},(coax_v_u8){message,n},(coax_v_t_PolyWorkspace){&work,1},(coax_v_u8){output,17},0);
 return wipe() && ok && output[16]==165;
}
int poly_refusal(int mode){
 memset(&work,0xa5,sizeof work);memset(output,165,sizeof output);
 int ok=poly_workspace((coax_v_u8){key,mode==0?31:32},(coax_v_u8){message,mode==1?-1:16},(coax_v_t_PolyWorkspace){&work,1},(coax_v_u8){output,mode==2?15:17},mode==3);
 if(!wipe() || ok)return 0;
 for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;
 return 1;
}
