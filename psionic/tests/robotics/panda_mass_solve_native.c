/* Test effects/ABI only. Independent Gaussian/known-solution oracle is Python. */
#include "panda-mass-solve.h"
#include <stddef.h>
#include <stdint.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static size_t assertions;
#define NEED(v) do{++assertions;if(!(v)){fprintf(stderr,"Panda mass solve native line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PmsOutput)==72&&offsetof(coax_t_PmsOutput,slide)==56,"typed accelerations erase to nine F64 words");
_Static_assert(sizeof(coax_t_PmsRaw)==80&&offsetof(coax_t_PmsRaw,acceleration)==8,"raw solver status/value layout");
static double real(uint64_t bits){double value;memcpy(&value,&bits,8);return value;}
static void get(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static void refusal(double*x,int64_t q,int64_t s,int64_t h,int64_t f,int64_t outputs,coax_t_PmsStatus status){
 double original[18];memcpy(original,x,sizeof original);coax_t_PmsOutput out[2],before[2];memset(out,0xa5,sizeof out);memcpy(before,out,sizeof out);
 NEED(pms_forward((coax_v_f64){x,q},(coax_v_f64){x+7,s},(coax_v_f64){x+9,h},(coax_v_f64){x+16,f},(coax_v_t_PmsOutput){out,outputs})==status);NEED(!memcmp(before,out,sizeof out));
 NEED(pms_forward_raw((coax_v_f64){x,q},(coax_v_f64){x+7,s},(coax_v_f64){x+9,h},(coax_v_f64){x+16,f},(coax_v_t_PmsOutput){out,outputs})==status);NEED(!memcmp(before,out,sizeof out));NEED(!memcmp(original,x,sizeof original));
}
static coax_a81_f64 identity(void){coax_a81_f64 m={{0}};for(int i=0;i<9;i++)m.e[i*9+i]=1;return m;}
static void raw_refusal(coax_a81_f64 m,coax_a9_f64 f,coax_t_PmsStatus status){
 coax_t_PmsRaw r=pms_probe_raw(m,f);NEED(r.status==status);for(int i=0;i<9;i++){uint64_t bits;memcpy(&bits,r.acceleration.e+i,8);NEED(bits==0);}
}
static void faults(void){
 double x[18]={0};for(int n=-1;n<=8;n++)if(n!=7){refusal(x,n,2,7,2,1,COAX_PmsStatus_shape);refusal(x,7,2,n,2,1,COAX_PmsStatus_shape);}
 for(int n=-1;n<=3;n++)if(n!=2){refusal(x,7,n,7,2,1,COAX_PmsStatus_shape);refusal(x,7,2,7,n,1,COAX_PmsStatus_shape);}for(int n=-1;n<=2;n++)if(n!=1)refusal(x,7,2,7,2,n,COAX_PmsStatus_shape);
 const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int i=0;i<18;i++){
  for(size_t k=0;k<5;k++){x[i]=real(bad[k]);refusal(x,7,2,7,2,1,COAX_PmsStatus_number);refusal(x,7,2,7,2,0,COAX_PmsStatus_shape);}
  x[i]=i<7?100.00000000000001:(i<9?1.0000000000000002:10000.000000000002);refusal(x,7,2,7,2,1,COAX_PmsStatus_range);x[i]=-x[i];refusal(x,7,2,7,2,1,COAX_PmsStatus_range);x[i]=0;
 }
 memset(x,0,sizeof x);x[0]=101;x[9]=real(bad[0]);refusal(x,7,2,7,2,1,COAX_PmsStatus_range);
 memset(x,0,sizeof x);x[9]=10001;x[10]=real(bad[0]);refusal(x,7,2,7,2,1,COAX_PmsStatus_range);
 memset(x,0,sizeof x);x[15]=real(bad[0]);x[16]=10001;refusal(x,7,2,7,2,1,COAX_PmsStatus_number);
 memset(x,0,sizeof x);x[16]=10001;x[17]=real(bad[0]);refusal(x,7,2,7,2,1,COAX_PmsStatus_range);
 coax_a81_f64 m=identity();coax_a9_f64 f={{0}};
 for(int i=0;i<81;i++){for(size_t k=0;k<5;k++){coax_a81_f64 b=m;b.e[i]=real(bad[k]);raw_refusal(b,f,COAX_PmsStatus_number);}coax_a81_f64 b=m;b.e[i]=100000000.00000001;raw_refusal(b,f,COAX_PmsStatus_range);b.e[i]=-100000000.00000001;raw_refusal(b,f,COAX_PmsStatus_range);}
 for(int i=0;i<9;i++){for(size_t k=0;k<5;k++){f.e[i]=real(bad[k]);raw_refusal(m,f,COAX_PmsStatus_number);}f.e[i]=10000.000000000002;raw_refusal(m,f,COAX_PmsStatus_range);f.e[i]=0;coax_a81_f64 b=m;b.e[i*9+i]=nextafter(.25,0.);raw_refusal(b,f,COAX_PmsStatus_factor);b.e[i*9+i]=-1;raw_refusal(b,f,COAX_PmsStatus_factor);}
 for(int i=0;i<9;i++)for(int j=0;j<i;j++){coax_a81_f64 b=m;b.e[i*9+j]=.125;raw_refusal(b,f,COAX_PmsStatus_factor);}
}
int main(int argc,char**argv){
 if(argc!=3)return 2;FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx");NEED(input&&output);uint32_t magic,n,raw;get(input,&magic,4);get(input,&n,4);get(input,&raw,4);NEED(magic==UINT32_C(0x31534d50)&&n==129&&raw==7);
 for(uint32_t row=0;row<n;row++){
  double x[18],old[18];get(input,x,sizeof x);memcpy(old,x,sizeof x);coax_t_PmsOutput a[2],b[1],poison;memset(a,0xa5,sizeof a);memcpy(&poison,&a[1],sizeof poison);
  NEED(pms_forward((coax_v_f64){x,7},(coax_v_f64){x+7,2},(coax_v_f64){x+9,7},(coax_v_f64){x+16,2},(coax_v_t_PmsOutput){a,1})==COAX_PmsStatus_ok);
  NEED(pms_forward_raw((coax_v_f64){x,7},(coax_v_f64){x+7,2},(coax_v_f64){x+9,7},(coax_v_f64){x+16,2},(coax_v_t_PmsOutput){b,1})==COAX_PmsStatus_ok);
  NEED(!memcmp(a,b,sizeof b)&&!memcmp(&a[1],&poison,sizeof poison));NEED(!memcmp(x,old,sizeof x));for(int i=0;i<7;i++)NEED(isfinite(a[0].hinge.e[i]));for(int i=0;i<2;i++)NEED(isfinite(a[0].slide.e[i]));put(output,a,sizeof a[0]);
 }
 for(uint32_t row=0;row<raw;row++){coax_a81_f64 m;coax_a9_f64 f;get(input,&m,sizeof m);get(input,&f,sizeof f);coax_t_PmsRaw r=pms_probe_raw(m,f);NEED(r.status==COAX_PmsStatus_ok);for(int i=0;i<9;i++)NEED(isfinite(r.acceleration.e[i]));put(output,&r.acceleration,sizeof r.acceleration);}
 NEED(fgetc(input)==EOF);faults();NEED(!fclose(input)&&!fclose(output));printf("{\"schema\":\"psionic.panda-mass-solve.native.v1\",\"passed\":true,\"public_cases\":%u,\"raw_cases\":%u,\"assertions\":%zu,\"output_words\":1224}\n",n,raw,assertions);return 0;
}
