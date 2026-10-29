#include "hchacha20.h"
#include <stdint.h>
static uint8_t key[32], nonce[16], output[33];
uint8_t *hc_key(void) { return key; }
uint8_t *hc_nonce(void) { return nonce; }
uint8_t *hc_output(void) { return output; }
int hc_run(void) {
  output[32]=165;
  coax_t_ChaChaResult r=hchacha20_checked((coax_v_u8){key,32},(coax_v_u8){nonce,16},(coax_v_u8){output,33},0);
  return r.error==0 && r.written==32 && output[32]==165;
}
int hc_refusal(int mode) {
  for(int i=0;i<33;i++) output[i]=165;
  coax_t_ChaChaResult r=hchacha20_checked((coax_v_u8){key,mode==0?31:32},
      (coax_v_u8){nonce,mode==1?15:16},(coax_v_u8){output,mode==2?31:33},mode==3);
  if(!r.error || r.written) return 0;
  for(int i=0;i<33;i++) if(output[i]!=165) return 0;
  return 1;
}
