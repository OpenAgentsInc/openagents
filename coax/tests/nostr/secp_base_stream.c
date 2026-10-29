/* Test-only bounded representation/I/O. All arithmetic lives in Coax. */
#include "secp_base.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
int main(int argc,char**argv){
 if(argc!=2)return 1;
 int variant=atoi(argv[1]);if(variant!=0&&variant!=1)return 1;
 uint8_t in[32],out[96];unsigned count=0;size_t got;
 while((got=fread(in,1,sizeof(in),stdin))==sizeof(in)){
  if(count++>=100000)return 2;
  coax_t_ScScalar scalar=sc_reduce_bytes((coax_v_u8){in,32}).value;
  coax_t_SgPoint point=variant?smb_multiply_native(scalar):smb_multiply_web(scalar);
  if(!sf_store(point.x,(coax_v_u8){out,32})||!sf_store(point.y,(coax_v_u8){out+32,32})||!sf_store(point.z,(coax_v_u8){out+64,32}))return 3;
  if(fwrite(out,1,sizeof(out),stdout)!=sizeof(out))return 4;
 }
 if(got||ferror(stdin)||ferror(stdout))return 5;
 fprintf(stderr,"%u fixed-base multiplications\n",count);return 0;
}
