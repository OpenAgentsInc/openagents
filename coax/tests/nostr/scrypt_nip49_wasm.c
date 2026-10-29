#include "scrypt_nip49.h"
#include <stdint.h>
static coax_t_Nip49Scrypt state;
static uint8_t password[1024],salt[16],memory[65536],output[33];
uint8_t *sk_password(void){return password;}
uint8_t *sk_salt(void){return salt;}
uint8_t *sk_output(void){return output;}
int sk_start(int pn,int log_n){
  if(log_n<1 || log_n>6)return 0;
  return nip49_scrypt_start((coax_v_t_Nip49Scrypt){&state,1},(coax_v_u8){password,pn},(coax_v_u8){salt,16},(coax_v_u8){memory,(1<<log_n)*1024},log_n,0);
}
int sk_step(int budget){return nip49_scrypt_step((coax_v_t_Nip49Scrypt){&state,1},(coax_v_u8){memory,state.romix.e[0].n*1024},budget,0);}
int sk_phase(void){return (int)state.romix.e[0].phase;}
int sk_finish(void){output[32]=165;return nip49_scrypt_finish((coax_v_t_Nip49Scrypt){&state,1},(coax_v_u8){output,33},0) && output[32]==165;}
/* Native large-vector harness uses the generated state ABI rather than a
 * duplicated host-language struct layout. Test inputs here are public. */
#include <stdlib.h>
int sk_large(void){
  const int n=64*1024*1024;uint8_t *large=calloc((size_t)n,1);if(!large)return 0;
  int ok=nip49_scrypt_start((coax_v_t_Nip49Scrypt){&state,1},(coax_v_u8){password,5},(coax_v_u8){salt,16},(coax_v_u8){large,n},16,0);
  int steps=0;while(ok&&state.romix.e[0].phase!=3&&steps<2048){ok=nip49_scrypt_step((coax_v_t_Nip49Scrypt){&state,1},(coax_v_u8){large,n},64,0);steps++;}
  if(state.romix.e[0].phase!=3)ok=0;
  if(ok)ok=nip49_scrypt_finish((coax_v_t_Nip49Scrypt){&state,1},(coax_v_u8){output,32},0);
  free(large);return ok;
}
