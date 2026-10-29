#include "sha-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t input[16384],output[33];static coax_t_ShaOwned work;
uint8_t *so_input(void){return input;}uint8_t *so_output(void){return output;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int so_run(int n,int chunk){
 if(n<0||n>16384||chunk<1)return 0;memset(&work,165,sizeof work);output[32]=165;
 coax_v_t_ShaOwned w={&work,1};if(!so_init(w)){wipe();return 0;}
 for(int i=0;i<n;){int m=n-i;if(m>chunk)m=chunk;if(!so_update(w,(coax_v_u8){input+i,m},0)){wipe();return 0;}i+=m;}
 int ok=so_finish(w,(coax_v_u8){output,33},0);return wipe()&&ok&&output[32]==165;
}
int so_refusal(void){
 memset(&work,165,sizeof work);memset(output,165,sizeof output);coax_v_t_ShaOwned w={&work,1};
 if(!so_init(w)||so_update(w,(coax_v_u8){input,1},1)||so_finish(w,(coax_v_u8){output,31},0)||so_finish(w,(coax_v_u8){output,32},1))return 0;
 for(size_t i=0;i<sizeof output;i++)if(output[i]!=165)return 0;return wipe();
}
