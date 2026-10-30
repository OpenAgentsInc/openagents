/* Test-only secret-flow probe of the BIP-340 signer (#4828, #51f4), in the
   ctgrind style: memcheck treats the secret key and the auxiliary randomness
   as undefined, so every branch or memory address in the compiled signer
   that depends on them is reported. Only the published signature and the
   step status are declassified. It maps where key-derived values (the
   public key, the nonce point R) are used before an explicit declassify;
   each report names its source line.
   Run: coax/tests/nostr/schnorr_sign_secret_probe.sh */
#include "es.h"
#include <valgrind/memcheck.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static uint8_t key[32],aux[32],message[32],out[64];
static coax_t_SignWork work;
int main(void) {
  key[31]=3;                                  /* BIP-340 vector 0's key */
  VALGRIND_MAKE_MEM_UNDEFINED(key,32);VALGRIND_MAKE_MEM_UNDEFINED(aux,32);
  coax_v_u8 k={key,32},a={aux,32},m={message,32},o={out,64};
  coax_v_t_SignWork w={&work,1};
  int64_t status;int calls=0;
  do{status=schnorr_sign_step(k,a,m,w,o,false);calls++;VALGRIND_MAKE_MEM_DEFINED(&status,sizeof status);}while(status==1&&calls<16);
  VALGRIND_MAKE_MEM_DEFINED(out,64);
  printf("calls %d status %lld sig %02x%02x..\n",calls,(long long)status,out[0],out[1]);
  return 0;
}
