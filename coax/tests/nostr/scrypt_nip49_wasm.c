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
