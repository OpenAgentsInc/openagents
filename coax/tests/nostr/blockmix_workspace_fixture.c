#include "block-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t input[1024],output[1025];static coax_t_BlockmixWorkspace work;
uint8_t *bm_input(void){return input;}uint8_t *bm_output(void){return output;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int bm_run(int r){memset(&work,165,sizeof work);output[r*128]=165;int ok=blockmix_owned((coax_v_u8){input,r*128},(coax_v_t_BlockmixWorkspace){&work,1},(coax_v_u8){output,r*128+1},r,0);return wipe()&&ok&&output[r*128]==165;}
int bm_refusal(int mode){memset(&work,165,sizeof work);memset(output,165,sizeof output);int ok=blockmix_owned((coax_v_u8){input,mode==0?127:128},(coax_v_t_BlockmixWorkspace){&work,1},(coax_v_u8){output,mode==1?127:1025},mode==2?0:mode==3?9:1,mode==4);if(!wipe()||ok)return 0;for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;return 1;}
