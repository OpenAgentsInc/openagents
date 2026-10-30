/* Test-only secret-flow probe of the BIP-340 signer (#4828, #51f4), in the
   ctgrind style: memcheck treats the secret key and the auxiliary randomness
   as undefined, so every branch or memory address in the compiled signer
   that depends on them is reported. Only the published signature and the
   step status are declassified, plus, after each call, the values the signer
   publishes or decides on (P, R and the validity bits), as #4828's
   _declassify points do. A report left elsewhere is a key-dependent branch
   or address in the compiled signer.
   Run: coax/tests/nostr/schnorr_sign_secret_probe.sh */
#include "es.h"
#include <valgrind/memcheck.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static uint8_t key[32],aux[32],message[32],out[64];
static coax_t_SignWork work;
int main(int argc,char **argv) {
  key[31]=3;                                  /* BIP-340 vector 0's key */
  VALGRIND_MAKE_MEM_UNDEFINED(key,32);VALGRIND_MAKE_MEM_UNDEFINED(aux,32);
  if(argc>1){volatile int seen=0;if(key[31]==3)seen=1;printf("control %d\n",seen);return 0;}  /* a branch on the key: must be flagged */
  coax_v_u8 k={key,32},a={aux,32},m={message,32},o={out,64};
  coax_v_t_SignWork w={&work,1};
  int64_t status;int calls=0;
  do{
    status=schnorr_sign_step(k,a,m,w,o,false);calls++;VALGRIND_MAKE_MEM_DEFINED(&status,sizeof status);
    /* The values the signer publishes or decides on, declassified as #4828's
       _declassify points do: the public key P, R (the published signature's
       first half), and the key, nonce and equation validity bits. */
    VALGRIND_MAKE_MEM_DEFINED(&work.prep.e[0].public_key,sizeof work.prep.e[0].public_key);
    VALGRIND_MAKE_MEM_DEFINED(&work.prep.e[0].valid,sizeof work.prep.e[0].valid);
    VALGRIND_MAKE_MEM_DEFINED(&work.nonce.e[0].r,sizeof work.nonce.e[0].r);
    VALGRIND_MAKE_MEM_DEFINED(&work.nonce.e[0].valid,sizeof work.nonce.e[0].valid);
    VALGRIND_MAKE_MEM_DEFINED(&work.nonce.e[0].nonce_valid,sizeof work.nonce.e[0].nonce_valid);
    VALGRIND_MAKE_MEM_DEFINED(&work.math.e[0].valid,sizeof work.math.e[0].valid);
  }while(status==1&&calls<16);
  VALGRIND_MAKE_MEM_DEFINED(out,64);
  printf("calls %d status %lld sig %02x%02x..\n",calls,(long long)status,out[0],out[1]);
  return 0;
}
