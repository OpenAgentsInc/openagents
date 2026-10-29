#include "xchacha20_poly1305.h"
#include <stdint.h>
static uint8_t key[32],nonce[24],aad[1024],input[4112],output[4113];
uint8_t *aead_key(void) { return key; }
uint8_t *aead_nonce(void) { return nonce; }
uint8_t *aead_aad(void) { return aad; }
uint8_t *aead_input(void) { return input; }
uint8_t *aead_output(void) { return output; }
int aead_seal(int an,int n) {
  output[n+16]=165;
  return xcp_seal((coax_v_u8){key,32},(coax_v_u8){nonce,24},(coax_v_u8){aad,an},(coax_v_u8){input,n},(coax_v_u8){output,n+17},0) && output[n+16]==165;
}
int aead_open(int an,int n) {
  output[n-16]=165;
  return xcp_open((coax_v_u8){key,32},(coax_v_u8){nonce,24},(coax_v_u8){aad,an},(coax_v_u8){input,n},(coax_v_u8){output,n-15},0) && output[n-16]==165;
}
int aead_tamper(int an,int n,int at) {
  for(int i=0;i<4113;i++) output[i]=165;
  input[at]^=1;
  int ok=xcp_open((coax_v_u8){key,32},(coax_v_u8){nonce,24},(coax_v_u8){aad,an},(coax_v_u8){input,n},(coax_v_u8){output,n-16},0);
  input[at]^=1;
  if(ok) return 0;
  for(int i=0;i<4113;i++) if(output[i]!=165) return 0;
  return 1;
}
