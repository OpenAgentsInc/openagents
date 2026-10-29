#include "salsa20_8.h"
#include <stdint.h>

static uint32_t input[17], output[17];
uint32_t *salsa_input(void) { return input; }
uint32_t *salsa_output(void) { return output; }
int salsa_run(void) {
  return salsa20_8_core((coax_v_u32){input,16}, (coax_v_u32){output,16}, 0);
}
int salsa_refusal(int mode) {
  for (int i=0;i<17;i++) output[i]=0xa5a5a5a5;
  int ok=salsa20_8_core((coax_v_u32){input,mode==0?15:16},
                        (coax_v_u32){output,mode==1?15:17},mode==2);
  if (ok) return 0;
  for (int i=0;i<17;i++) if (output[i]!=0xa5a5a5a5) return 0;
  return 1;
}
int salsa_tail(void) {
  output[16]=0xa5a5a5a5;
  return salsa20_8_core((coax_v_u32){input,16},(coax_v_u32){output,17},0) && output[16]==0xa5a5a5a5;
}
