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
/* Native large-vector helper uses the generated state ABI, not a mirror. */
#include <stdlib.h>
int rm_large(void){
 const int n=64*1024*1024;uint8_t *large=calloc((size_t)n,1);if(!large)return 0;
 int ok=romix_start((coax_v_t_RomixState){&state,1},(coax_v_u8){input,1024},(coax_v_u8){large,n},8,65536,0);
 int steps=0;while(ok&&state.phase!=3&&steps<2048){ok=romix_step((coax_v_t_RomixState){&state,1},(coax_v_u8){large,n},64,0);steps++;}
 if(state.phase!=3)ok=0;
 if(ok)ok=romix_finish((coax_v_t_RomixState){&state,1},(coax_v_u8){output,1024},0);
 free(large);return ok;
}
