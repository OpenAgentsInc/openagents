#include "scrypt_romix.h"
#include <stdint.h>
static coax_t_RomixState state;
static uint8_t input[1024], memory[16384], output[1025];
uint8_t *rm_input(void) { return input; }
uint8_t *rm_output(void) { return output; }
int rm_start(int r,int n) {
  if(r<1 || r>8 || n<2 || n>16) return 0;
  return romix_start((coax_v_t_RomixState){&state,1},(coax_v_u8){input,128*r},(coax_v_u8){memory,n*128*r},r,n,0);
}
int rm_step(int budget) {return romix_step((coax_v_t_RomixState){&state,1},(coax_v_u8){memory,state.n*128*state.r},budget,0);}
int rm_phase(void) {return (int)state.phase;}
int rm_index(void) {return (int)state.index;}
int rm_finish(void) {
  int n=(int)state.r*128;output[n]=165;
  return romix_finish((coax_v_t_RomixState){&state,1},(coax_v_u8){output,n+1},0) && output[n]==165;
}
int rm_refusal(int mode) {
  return !romix_start((coax_v_t_RomixState){&state,1},(coax_v_u8){input,128},
      (coax_v_u8){memory,mode==0?255:256},1,mode==1?3:2,mode==2);
}
