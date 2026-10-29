#include "zeroize_large.h"
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

int main(void){
  const int64_t n=64*1024*1024;
  uint8_t *memory=malloc((size_t)n+2);assert(memory);memset(memory,0xa5,(size_t)n+2);
  coax_large_wipe scope={0};
  assert(coax_large_wipe_begin(&scope,NULL,1)==3);
  assert(coax_large_wipe_begin(&scope,memory,-1)==1);
  assert(coax_large_wipe_begin(&scope,&scope,sizeof scope)==4);
  assert(coax_large_wipe_begin(&scope,(void *)(UINTPTR_MAX-1),4)==2);
  assert(coax_large_wipe_begin(&scope,memory+1,n)==0);
  assert(coax_large_wipe_begin(&scope,memory+1,n)==5);
  assert(coax_large_wipe_reset(&scope)==5);
  assert(coax_large_wipe_step(&scope)==9);
  for(int i=0;i<1048576;i++)assert(memory[1+i]==0);
  assert(memory[1+1048576]==0xa5);
  for(int i=1;i<63;i++)assert(coax_large_wipe_step(&scope)==9);
  assert(coax_large_wipe_step(&scope)==0);
  for(int64_t i=0;i<n;i++)assert(memory[1+i]==0);
  assert(memory[0]==0xa5 && memory[n+1]==0xa5);
  assert(coax_large_wipe_step(&scope)==0 && coax_large_wipe_reset(&scope)==0);
  assert(coax_large_wipe_step(&scope)==5);
  assert(coax_large_wipe_begin(&scope,NULL,0)==0);
  assert(coax_large_wipe_step(&scope)==0 && coax_large_wipe_reset(&scope)==0);
  assert(coax_large_wipe_begin(&scope,memory+1,9)==0);
  scope.policy.cursor=10;
  assert(coax_large_wipe_step(&scope)==5 && coax_large_wipe_reset(&scope)==5);
  free(memory);
}
