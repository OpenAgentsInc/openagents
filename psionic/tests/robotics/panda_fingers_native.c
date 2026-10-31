/* New four-body branch ABI/admission proof. Numeric references are external. */
#include "panda-fingers.h"
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static size_t checks;
#define NEED(v) do{++checks;if(!(v)){fprintf(stderr,"Panda fingers native failure line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PfkOutput)==1664,"published arm ABI");
_Static_assert(sizeof(coax_t_PffOutput)==2176&&offsetof(coax_t_PffOutput,fingers)==1664,"separate272word output");
_Static_assert(sizeof(coax_t_PffNode)==104&&offsetof(coax_t_PffNode,axis)==72&&offsetof(coax_t_PffNode,reference)==96,"source table layout");
static double real(uint64_t w){double x;memcpy(&x,&w,8);return x;}
static void get(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static void refusal(double*q,int64_t nq,int64_t ns,int64_t no,coax_t_PfkStatus expected){
 coax_t_PffOutput a[2],old[2];double before[9];memcpy(before,q,sizeof before);memset(a,0xa5,sizeof a);memcpy(old,a,sizeof a);
 NEED(pff_forward((coax_v_f64){q,nq},(coax_v_f64){q+7,ns},(coax_v_t_PffOutput){a,no})==expected);NEED(!memcmp(old,a,sizeof a));NEED(!memcmp(before,q,sizeof before));
 NEED(pff_forward_raw((coax_v_f64){q,nq},(coax_v_f64){q+7,ns},(coax_v_t_PffOutput){a,no})==expected);NEED(!memcmp(old,a,sizeof a));NEED(!memcmp(before,q,sizeof before));
}
static void faults(void){
 double q[9]={0};for(int n=-1;n<=8;n++)if(n!=7)refusal(q,n,2,1,COAX_PfkStatus_shape);
 for(int n=-1;n<=3;n++)if(n!=2)refusal(q,7,n,1,COAX_PfkStatus_shape);
 for(int n=-1;n<=2;n++)if(n!=1)refusal(q,7,2,n,COAX_PfkStatus_shape);
 const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int i=0;i<9;i++){
  for(size_t k=0;k<sizeof bad/sizeof*bad;k++){q[i]=real(bad[k]);refusal(q,7,2,1,COAX_PfkStatus_number);}
  q[i]=i<7?100.00000000000001:1.0000000000000002;refusal(q,7,2,1,COAX_PfkStatus_range);
  q[i]=-q[i];refusal(q,7,2,1,COAX_PfkStatus_range);q[i]=0;
 }
 q[0]=101;q[7]=real(bad[0]);refusal(q,7,2,1,COAX_PfkStatus_range);
 q[0]=real(bad[0]);q[7]=2;refusal(q,7,2,1,COAX_PfkStatus_number);
 q[0]=101;q[1]=real(bad[0]);refusal(q,7,2,1,COAX_PfkStatus_range);
 q[0]=q[1]=0;q[7]=2;q[8]=real(bad[0]);refusal(q,7,2,1,COAX_PfkStatus_range);
 q[7]=real(bad[0]);q[8]=2;refusal(q,7,2,1,COAX_PfkStatus_number);
 q[0]=101;refusal(q,6,2,1,COAX_PfkStatus_shape);refusal(q,7,1,1,COAX_PfkStatus_shape);refusal(q,7,2,2,COAX_PfkStatus_shape);
}
int main(int argc,char**argv){
 if(argc!=4)return 2;FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx"),*profile=fopen(argv[3],"wbx");NEED(input&&output&&profile);uint32_t magic,n;get(input,&magic,4);get(input,&n,4);NEED(magic==UINT32_C(0x31464650)&&n>0&&n<=4096);
 for(uint32_t i=0;i<n;i++){
  double q[9],before[9];get(input,q,sizeof q);memcpy(before,q,sizeof q);coax_t_PffOutput a[2],b[1],poison;coax_t_PfkOutput arm[1];memset(a,0xa5,sizeof a);memcpy(&poison,&a[1],sizeof poison);
  NEED(pff_forward_raw((coax_v_f64){q,7},(coax_v_f64){q+7,2},(coax_v_t_PffOutput){a,1})==COAX_PfkStatus_ok);
  NEED(pff_forward((coax_v_f64){q,7},(coax_v_f64){q+7,2},(coax_v_t_PffOutput){b,1})==COAX_PfkStatus_ok);
  NEED(pfk_forward((coax_v_f64){q,7},(coax_v_t_PfkOutput){arm,1})==COAX_PfkStatus_ok);
  NEED(!memcmp(a,b,sizeof b)&&!memcmp(&a[0].arm,arm,sizeof arm));NEED(!memcmp(&a[1],&poison,sizeof poison));NEED(!memcmp(q,before,sizeof q));put(output,a,sizeof a[0]);
 }
 NEED(fgetc(input)==EOF);coax_a4_t_PffNode nodes=pff_profile();put(profile,&nodes,sizeof nodes);faults();NEED(!fclose(input)&&!fclose(output)&&!fclose(profile));
 printf("{\"schema\":\"psionic.panda-fingers.native.v1\",\"passed\":true,\"cases\":%u,\"assertions\":%zu,\"output_words_per_case\":272}\n",n,checks);return 0;
}
