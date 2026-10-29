/* Test-only public fixture transport; arithmetic stays in Coax. */
#include "secp_mult_const.h"
#include <stdint.h>
#include <stdio.h>
int main(void){
  uint8_t input[96],out[96];unsigned count=0;size_t n;
  while((n=fread(input,1,sizeof(input),stdin))==sizeof(input)){
    if(count++>=100000)return 2;
    coax_t_ScLoad scalar=sc_reduce_bytes((coax_v_u8){input,32});
    coax_t_SecpFieldResult x=sf_load_strict((coax_v_u8){input+32,32});
    coax_t_SecpFieldResult y=sf_load_strict((coax_v_u8){input+64,32});
    if(!scalar.ok||!x.ok||!y.ok)return 3;
    coax_t_SgPoint peer={0};peer.x=x.value;peer.y=y.value;peer.z.limb.e[0]=1;
    coax_t_SgResult result=smc_multiply(scalar.value,peer);if(!result.ok)return 4;
    if(!sf_store(result.value.x,(coax_v_u8){out,32})||!sf_store(result.value.y,(coax_v_u8){out+32,32})||!sf_store(result.value.z,(coax_v_u8){out+64,32}))return 5;
    if(fwrite(out,1,sizeof(out),stdout)!=sizeof(out))return 6;
  }
  if(n||ferror(stdin)||ferror(stdout))return 7;
  fprintf(stderr,"%u variable-base multiplications\n",count);return 0;
}
