/* Test-only bounded byte stream, not a product host or crypto implementation. */
#include "secp_mult.h"
#include <stdio.h>
#include <stdint.h>
int main(void){
 uint8_t in[160],out[97];unsigned count=0;size_t got;
 while((got=fread(in,1,sizeof(in),stdin))==sizeof(in)){
  if(count++>=100000)return 2;
  coax_t_ScScalar a=sc_reduce_bytes((coax_v_u8){in,32}).value,b=sc_reduce_bytes((coax_v_u8){in+32,32}).value;
  coax_t_SgPoint p={sf_load((coax_v_u8){in+64,32}).value,sf_load((coax_v_u8){in+96,32}).value,sf_load((coax_v_u8){in+128,32}).value};
  coax_t_SgResult r=ecmult_vartime(a,p,b);out[0]=r.ok?1:0;
  if(!sf_store(r.value.x,(coax_v_u8){out+1,32})||!sf_store(r.value.y,(coax_v_u8){out+33,32})||!sf_store(r.value.z,(coax_v_u8){out+65,32}))return 3;
  if(fwrite(out,1,sizeof(out),stdout)!=sizeof(out))return 4;
 }
 if(got||ferror(stdin)||ferror(stdout))return 5;
 fprintf(stderr,"%u public dual multiplications\n",count);return 0;
}
