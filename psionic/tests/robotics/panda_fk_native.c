/* New FK ABI/atomic-admission harness. The numeric oracle is independent
   stdlib matrix algebra plus frozen installed-SDK records, never this file. */
#include "panda-fk.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>
static size_t checks;
#define NEED(v) do{++checks;if(!(v)){fprintf(stderr,"Panda FK native failure line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PfkPose)==128,"pose raw layout");
_Static_assert(offsetof(coax_t_PfkPose,position)==0&&offsetof(coax_t_PfkPose,quaternion)==24&&offsetof(coax_t_PfkPose,rotation)==56,"pose fields");
_Static_assert(sizeof(coax_t_PfkOutput)==208*8&&offsetof(coax_t_PfkOutput,grip)==192*8,"complete output has no hidden tags/padding");
_Static_assert(sizeof(coax_t_PfkNode)==72&&offsetof(coax_t_PfkNode,offset)==16&&offsetof(coax_t_PfkNode,fixed)==40,"source profile fields");
static double real(uint64_t word){double v;memcpy(&v,&word,8);return v;}
static void get(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static void refusal(double*q,int64_t n,int64_t outputs,coax_t_PfkStatus expected){
 coax_t_PfkOutput out[2],before[2];memset(out,0xa5,sizeof out);memcpy(before,out,sizeof out);
 NEED(pfk_forward((coax_v_f64){q,n},(coax_v_t_PfkOutput){out,outputs})==expected);NEED(!memcmp(before,out,sizeof out));
 NEED(pfk_forward_raw((coax_v_f64){q,n},(coax_v_t_PfkOutput){out,outputs})==expected);NEED(!memcmp(before,out,sizeof out));
}
static void faults(void){
 double q[8]={0};for(int n=-1;n<=8;n++)if(n!=7)refusal(q,n,1,COAX_PfkStatus_shape);
 for(int n=-1;n<=2;n++)if(n!=1)refusal(q,7,n,COAX_PfkStatus_shape);
 const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int i=0;i<7;i++){for(size_t k=0;k<sizeof bad/sizeof*bad;k++){q[i]=real(bad[k]);refusal(q,7,1,COAX_PfkStatus_number);}q[i]=100.00000000000001;refusal(q,7,1,COAX_PfkStatus_range);q[i]=-100.00000000000001;refusal(q,7,1,COAX_PfkStatus_range);q[i]=0;}
 for(int i=0;i<4;i++){
  coax_a4_u64 bits={{0}};coax_t_PfkQuaternion r=pfk_normalize_bits(bits);NEED(r.status==COAX_PfkStatus_rotation);
  for(size_t k=0;k<sizeof bad/sizeof*bad;k++){bits.e[i]=bad[k];r=pfk_normalize_bits(bits);NEED(r.status==COAX_PfkStatus_number);for(int j=0;j<4;j++)NEED(r.value.e[j]==0);}
  bits.e[i]=UINT64_C(0x4000000000000001);r=pfk_normalize_bits(bits);NEED(r.status==COAX_PfkStatus_range);
 }
}
int main(int argc,char**argv){
 if(argc!=4)return 2;FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx"),*profile=fopen(argv[3],"wbx");NEED(input&&output&&profile);uint32_t magic,n;get(input,&magic,4);get(input,&n,4);NEED(magic==UINT32_C(0x314b4650)&&n>0&&n<=4096);
 for(uint32_t i=0;i<n;i++){
  double q[7];get(input,q,sizeof q);coax_t_PfkOutput a[2],b[1],poison;memset(a,0xa5,sizeof a);memcpy(&poison,&a[1],sizeof poison);
  NEED(pfk_forward_raw((coax_v_f64){q,7},(coax_v_t_PfkOutput){a,1})==COAX_PfkStatus_ok);
  NEED(pfk_forward((coax_v_f64){q,7},(coax_v_t_PfkOutput){b,1})==COAX_PfkStatus_ok);NEED(!memcmp(a,b,sizeof b));NEED(!memcmp(&a[1],&poison,sizeof poison));put(output,&a[0],sizeof a[0]);
 }
 NEED(fgetc(input)==EOF);coax_a12_t_PfkNode nodes=pfk_profile();put(profile,&nodes,sizeof nodes);faults();NEED(!fclose(input)&&!fclose(output)&&!fclose(profile));
 printf("{\"schema\":\"psionic-panda-fk-native-v1\",\"passed\":true,\"cases\":%u,\"assertions\":%zu,\"output_words_per_case\":208}\n",n,checks);return 0;
}
