#include "zeroize_large.h"
#include <stdint.h>
static uint8_t memory[1048595];
static coax_large_wipe scope;
int wipe_fixture(void){
  for(int i=0;i<1048595;i++)memory[i]=165;
  if(coax_large_wipe_begin(&scope,memory+1,1048593)!=0)return 1;
  if(coax_large_wipe_step(&scope)!=9)return 2;
  for(int i=0;i<1048576;i++)if(memory[1+i]!=0)return 3;
  if(memory[1+1048576]!=165)return 4;
  if(coax_large_wipe_step(&scope)!=0)return 5;
  for(int i=0;i<1048593;i++)if(memory[1+i]!=0)return 6;
  if(memory[0]!=165 || memory[1048594]!=165)return 7;
  return coax_large_wipe_reset(&scope);
}
