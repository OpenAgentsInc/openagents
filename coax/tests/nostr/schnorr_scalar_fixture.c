#include "scalar-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t key[32],output[33];static coax_t_SignScalar work;static uint64_t valid;
uint8_t *ss_input(void){return key;}uint8_t *ss_output(void){return output;}uint64_t ss_valid(void){return valid;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int ss_run(int odd){memset(&work,165,sizeof work);output[32]=165;coax_v_t_SignScalar w={&work,1};int ok=ss_key((coax_v_u8){key,32},w,0);valid=work.valid;if(ok)ok=ss_probe_even(w,odd,(coax_v_u8){output,33});return wipe()&&ok&&output[32]==165;}
int ss_refusal(void){memset(&work,165,sizeof work);int ok=ss_key((coax_v_u8){key,31},(coax_v_t_SignScalar){&work,1},0);return wipe()&&!ok;}
