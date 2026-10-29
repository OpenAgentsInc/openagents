#include "poly1305.h"
#include <stdint.h>
static uint8_t key[32],message[1048576],output[17];
uint8_t *poly_key(void) { return key; }
uint8_t *poly_message(void) { return message; }
uint8_t *poly_output(void) { return output; }
int poly_run(int n) {
  output[16]=165;
  return poly1305_checked((coax_v_u8){key,32},(coax_v_u8){message,n},(coax_v_u8){output,17},0) && output[16]==165;
}
int poly_refusal(int mode) {
  for(int i=0;i<17;i++) output[i]=165;
  int ok=poly1305_checked((coax_v_u8){key,mode==0?31:32},(coax_v_u8){message,mode==1?-1:16},
                          (coax_v_u8){output,mode==2?15:17},mode==3);
  if(ok) return 0;
  for(int i=0;i<17;i++) if(output[i]!=165) return 0;
  return 1;
}
