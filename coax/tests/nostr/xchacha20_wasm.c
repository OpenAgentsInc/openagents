#include "xchacha20.h"
#include <stdint.h>
static uint8_t key[32],nonce[24],input[1024],output[1025];
uint8_t *xc_key(void) { return key; }
uint8_t *xc_nonce(void) { return nonce; }
uint8_t *xc_input(void) { return input; }
uint8_t *xc_output(void) { return output; }
int xc_run(int n,uint32_t counter) {
  output[n]=165;
  coax_t_ChaChaResult r=xchacha20_xor_checked((coax_v_u8){key,32},(coax_v_u8){nonce,24},counter,(coax_v_u8){input,n},(coax_v_u8){output,n+1},0);
  return r.error==0 && r.written==n && output[n]==165;
}
int xc_refusal(int mode) {
  for(int i=0;i<1025;i++) output[i]=165;
  coax_t_ChaChaResult r=xchacha20_xor_checked((coax_v_u8){key,mode==0?31:32},
     (coax_v_u8){nonce,mode==1?23:24},mode==4?UINT32_MAX:0,
     (coax_v_u8){input,65},(coax_v_u8){output,mode==2?64:1025},mode==3);
  if(!r.error || r.written) return 0;
  for(int i=0;i<1025;i++) if(output[i]!=165) return 0;
  return 1;
}
