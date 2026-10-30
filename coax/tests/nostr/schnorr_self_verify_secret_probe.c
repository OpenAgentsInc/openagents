/* Test-only secret-flow probe for the fixed-work self-verifier (#89db), in
   the ctgrind style: memcheck treats the unpublished candidate R||s as
   undefined, so any branch or memory address in the compiled code that
   depends on it is reported. Only the final validity mask is declassified,
   since publication is the one permitted observation. The variable-time
   verifier, given the same undefined candidate, is the positive control.
   Run: coax/tests/nostr/schnorr_self_verify_secret_probe.sh */
#include "schnorr-self-verify.h"
#include <valgrind/memcheck.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static uint8_t key[32]={249,48,138,1,146,88,195,16,73,52,79,133,248,157,82,41,181,49,200,69,131,111,153,176,134,1,241,19,188,224,54,249},sig[64]={233,7,131,31,128,132,141,16,105,165,55,27,64,36,16,54,75,223,28,95,131,7,176,8,76,85,241,206,45,202,130,21,37,246,106,74,133,234,139,113,228,130,167,79,56,45,44,229,235,238,232,253,178,23,47,71,125,244,144,13,49,5,54,192},message[32];
static coax_t_SsvWork work;
int main(int argc,char **argv) {
  int control=argc>1&&!strcmp(argv[1],"control");
  uint8_t candidate[64];memcpy(candidate,sig,64);
  if(argc>2)candidate[40]^=1;              /* an invalid candidate: same work */
  VALGRIND_MAKE_MEM_UNDEFINED(candidate,64);
  coax_v_u8 m={message,32},p={key,32},s={candidate,64};
  if(control){
    int ok=schnorr_verify_vartime(m,p,s);
    VALGRIND_MAKE_MEM_DEFINED(&ok,sizeof ok);printf("control %d\n",ok);return 0;
  }
  coax_v_t_SsvWork view={&work,1};uint64_t mask;int calls=0;
  for(;;){
    mask=ssv_step(m,p,s,view,false);calls++;
    VALGRIND_MAKE_MEM_DEFINED(&mask,sizeof mask);   /* stage status, then the published decision */
    if(mask!=1||calls>3)break;
  }
  printf("ssv calls %d valid %d\n",calls,mask==UINT64_MAX);
  return 0;
}
