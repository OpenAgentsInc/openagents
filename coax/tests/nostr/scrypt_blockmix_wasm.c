#include "scrypt_blockmix.h"
#include <stdint.h>
static uint8_t input[1024], output[1025];
uint8_t *bm_input(void) { return input; }
uint8_t *bm_output(void) { return output; }
int bm_run(int r) {
  output[r*128]=165;
  return scrypt_blockmix((coax_v_u8){input,r*128},(coax_v_u8){output,r*128+1},r,0) && output[r*128]==165;
}
int bm_refusal(int mode) {
  for(int i=0;i<1025;i++) output[i]=165;
  int ok=scrypt_blockmix((coax_v_u8){input,mode==0?127:128},
      (coax_v_u8){output,mode==1?127:1025},mode==2?0:mode==3?9:1,mode==4);
  if(ok) return 0;
  for(int i=0;i<1025;i++) if(output[i]!=165) return 0;
  return 1;
}
