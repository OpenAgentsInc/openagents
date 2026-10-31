/* New Jacobian output/atomic-admission ABI only. Independent numeric values
   come from retained SDK bytes and a stdlib matrix-FK finite-difference oracle. */
#include "panda-jacobian.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>
static size_t checks;
#define NEED(v) do{++checks;if(!(v)){fprintf(stderr,"Panda Jacobian native failure line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PjOutput)==42*8&&_Alignof(coax_t_PjOutput)==8,"42 F64 derivative words; no hidden unit payload");
_Static_assert(offsetof(coax_t_PjOutput,linear)==0&&offsetof(coax_t_PjOutput,angular)==21*8,"separate row-major21 derivative blocks");
static coax_t_PfkStatus(*const typed_abi)(coax_v_f64,coax_v_t_PjOutput)=pj_forward;
static double real(uint64_t word){double v;memcpy(&v,&word,8);return v;}
static void get(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static void refusal(double*q,int64_t n,int64_t outputs,coax_t_PfkStatus expected){
 coax_t_PjOutput out[2],before[2];memset(out,0xa5,sizeof out);memcpy(before,out,sizeof out);
 NEED(typed_abi((coax_v_f64){q,n},(coax_v_t_PjOutput){out,outputs})==expected);NEED(!memcmp(out,before,sizeof out));
 NEED(pj_forward_raw((coax_v_f64){q,n},(coax_v_t_PjOutput){out,outputs})==expected);NEED(!memcmp(out,before,sizeof out));
}
static void faults(void){
 double q[8]={0};for(int n=-1;n<=8;n++)if(n!=7)refusal(q,n,1,COAX_PfkStatus_shape);
 for(int n=-1;n<=2;n++)if(n!=1)refusal(q,7,n,COAX_PfkStatus_shape);
 const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int i=0;i<7;i++){
  for(size_t k=0;k<sizeof bad/sizeof*bad;k++){q[i]=real(bad[k]);refusal(q,7,1,COAX_PfkStatus_number);refusal(q,7,0,COAX_PfkStatus_shape);}
  q[i]=100.00000000000001;refusal(q,7,1,COAX_PfkStatus_range);q[i]=-100.00000000000001;refusal(q,7,1,COAX_PfkStatus_range);q[i]=0;
 }
 /* FK's first-coordinate admission precedence is propagated, never relabelled. */
 q[0]=101;q[1]=real(UINT64_C(0x7ff8000000000001));refusal(q,7,1,COAX_PfkStatus_range);
 q[0]=real(UINT64_C(0x7ff8000000000001));q[1]=101;refusal(q,7,1,COAX_PfkStatus_number);
}
int main(int argc,char**argv){
 if(argc!=3)return 2;FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx");NEED(input&&output);uint32_t magic,n;get(input,&magic,4);get(input,&n,4);NEED(magic==UINT32_C(0x314a4a50)&&n>0&&n<=4096);
 for(uint32_t i=0;i<n;i++){
  double q[7];get(input,q,sizeof q);coax_t_PjOutput a[2],b[1],poison;memset(a,0xa5,sizeof a);memcpy(&poison,&a[1],sizeof poison);
  NEED(pj_forward_raw((coax_v_f64){q,7},(coax_v_t_PjOutput){a,1})==COAX_PfkStatus_ok);
  NEED(typed_abi((coax_v_f64){q,7},(coax_v_t_PjOutput){b,1})==COAX_PfkStatus_ok);NEED(!memcmp(a,b,sizeof b));NEED(!memcmp(&a[1],&poison,sizeof poison));put(output,&a[0],sizeof a[0]);
 }
 NEED(fgetc(input)==EOF);faults();NEED(!fclose(input)&&!fclose(output));printf("{\"schema\":\"psionic-panda-jacobian-native-v1\",\"passed\":true,\"cases\":%u,\"assertions\":%zu,\"output_words_per_case\":42}\n",n,checks);return 0;
}
