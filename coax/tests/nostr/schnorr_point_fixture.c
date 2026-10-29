#include "point-owned.h"
#include <stdint.h>
#include <string.h>
extern uint32_t coax_secure_zero(void *,int64_t,int64_t,int64_t);
static uint8_t scalar[32],output[33];static coax_t_SignPoint work;static uint64_t finite,odd;
uint8_t *sp_input(void){return scalar;}uint8_t *sp_output(void){return output;}uint64_t sp_finite(void){return finite;}uint64_t sp_odd(void){return odd;}
static int wipe(void){if(coax_secure_zero(&work,sizeof work,0,sizeof work))return 0;const uint8_t *p=(const uint8_t *)&work;for(size_t i=0;i<sizeof work;i++)if(p[i])return 0;return 1;}
int sp_run(void){
 memset(&work,165,sizeof work);output[32]=165;coax_t_ScLoad v=sc_reduce_bytes((coax_v_u8){scalar,32});
 int ok=v.ok&&sp_base(v.value,(coax_v_t_SignPoint){&work,1});finite=work.finite;odd=work.odd;
 if(ok)ok=sp_x((coax_v_t_SignPoint){&work,1},(coax_v_u8){output,33});return wipe()&&ok&&output[32]==165;
}
