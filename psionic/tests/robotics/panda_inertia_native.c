/* NEW mass/gravity ABI and refusal checks. Numeric authority is independently
   retained SDK facts and matrix-FK kinetic/potential differences in tooling. */
#include "panda-inertia.h"
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
static size_t assertions;
#define NEED(x) do{++assertions;if(!(x)){fprintf(stderr,"Panda inertia failure line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PmOutput)==90*8&&_Alignof(coax_t_PmOutput)==8,"four typed blocks and two generalized force arrays erase to90F64");
_Static_assert(offsetof(coax_t_PmOutput,hinge_slide)==49*8&&offsetof(coax_t_PmOutput,slide_hinge)==63*8&&offsetof(coax_t_PmOutput,slide_slide)==77*8&&offsetof(coax_t_PmOutput,hinge_gravity)==81*8&&offsetof(coax_t_PmOutput,slide_gravity)==88*8,"independent block offsets");
_Static_assert(sizeof(coax_t_PmBody)==96&&sizeof(coax_t_PmProfile)==1632,"fixed inertial source table layout");
static coax_t_PfkStatus(*const typed)(coax_v_f64,coax_v_f64,coax_v_t_PmOutput)=pm_forward;
static double real(uint64_t v){double r;memcpy(&r,&v,8);return r;}
static void read_bytes(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void write_bytes(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static void flatten(const coax_t_PmOutput*p,double out[90]){
 for(int i=0;i<7;i++){for(int j=0;j<7;j++)out[i*9+j]=p->hinge_hinge.e[i*7+j];for(int j=0;j<2;j++){out[i*9+7+j]=p->hinge_slide.e[i*2+j];out[(j+7)*9+i]=p->slide_hinge.e[j*7+i];}out[81+i]=p->hinge_gravity.e[i];}
 for(int i=0;i<2;i++){for(int j=0;j<2;j++)out[(i+7)*9+j+7]=p->slide_slide.e[i*2+j];out[88+i]=p->slide_gravity.e[i];}
}
static void refuse(double*q,int64_t nq,double*s,int64_t ns,int64_t no,coax_t_PfkStatus expected){
 coax_t_PmOutput a[2],before[2];double oldq[8],olds[3];memcpy(oldq,q,sizeof oldq);memcpy(olds,s,sizeof olds);memset(a,0xa5,sizeof a);memcpy(before,a,sizeof a);
 NEED(typed((coax_v_f64){q,nq},(coax_v_f64){s,ns},(coax_v_t_PmOutput){a,no})==expected);NEED(!memcmp(a,before,sizeof a));
 NEED(pm_forward_raw((coax_v_f64){q,nq},(coax_v_f64){s,ns},(coax_v_t_PmOutput){a,no})==expected);NEED(!memcmp(a,before,sizeof a));NEED(!memcmp(q,oldq,sizeof oldq)&&!memcmp(s,olds,sizeof olds));
}
static void faults(void){
 double q[8]={0},s[3]={0};for(int n=-1;n<=8;n++)if(n!=7)refuse(q,n,s,2,1,COAX_PfkStatus_shape);for(int n=-1;n<=3;n++)if(n!=2)refuse(q,7,s,n,1,COAX_PfkStatus_shape);for(int n=-1;n<=2;n++)if(n!=1)refuse(q,7,s,2,n,COAX_PfkStatus_shape);
 const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int j=0;j<9;j++){double*word=j<7?q+j:s+(j-7);for(size_t k=0;k<5;k++){*word=real(bad[k]);refuse(q,7,s,2,1,COAX_PfkStatus_number);refuse(q,7,s,2,0,COAX_PfkStatus_shape);}*word=j<7?100.00000000000001:1.0000000000000002;refuse(q,7,s,2,1,COAX_PfkStatus_range);*word=-*word;refuse(q,7,s,2,1,COAX_PfkStatus_range);*word=0;}
 q[0]=101;s[0]=real(UINT64_C(0x7ff8000000000001));refuse(q,7,s,2,1,COAX_PfkStatus_range);q[0]=real(UINT64_C(0x7ff8000000000001));s[0]=2;refuse(q,7,s,2,1,COAX_PfkStatus_number);q[0]=0;s[0]=2;s[1]=real(UINT64_C(0x7ff8000000000001));refuse(q,7,s,2,1,COAX_PfkStatus_range);s[0]=real(UINT64_C(0x7ff8000000000001));s[1]=2;refuse(q,7,s,2,1,COAX_PfkStatus_number);refuse(q,6,s,2,1,COAX_PfkStatus_shape);refuse(q,7,s,1,1,COAX_PfkStatus_shape);
}
int main(int argc,char**argv){
 if(argc!=4)return 2;FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx"),*table=fopen(argv[3],"wbx");NEED(input&&output&&table);uint32_t magic,n;read_bytes(input,&magic,4);read_bytes(input,&n,4);NEED(magic==UINT32_C(0x314d4950)&&n==57);coax_t_PmProfile p=pm_profile();write_bytes(table,&p,sizeof p);NEED(!fclose(table));
 for(uint32_t k=0;k<n;k++){
  double q[9],old[9],flat[90];read_bytes(input,q,sizeof q);memcpy(old,q,sizeof q);coax_t_PmOutput a[2],b[1],poison;memset(a,0xa5,sizeof a);memcpy(&poison,&a[1],sizeof poison);
  NEED(typed((coax_v_f64){q,7},(coax_v_f64){q+7,2},(coax_v_t_PmOutput){a,1})==COAX_PfkStatus_ok);NEED(pm_forward_raw((coax_v_f64){q,7},(coax_v_f64){q+7,2},(coax_v_t_PmOutput){b,1})==COAX_PfkStatus_ok);NEED(!memcmp(a,b,sizeof b));NEED(!memcmp(&a[1],&poison,sizeof poison));NEED(!memcmp(old,q,sizeof q));flatten(a,flat);
  for(int i=0;i<90;i++)NEED(isfinite(flat[i]));for(int i=0;i<9;i++){NEED(flat[i*9+i]>=p.armature.e[i]);for(int j=0;j<9;j++)NEED(flat[i*9+j]==flat[j*9+i]);}NEED(flat[7*9+8]==0&&flat[8*9+7]==0);write_bytes(output,flat,sizeof flat);
 }
 NEED(fgetc(input)==EOF);faults();NEED(!fclose(input)&&!fclose(output));printf("{\"schema\":\"psionic-panda-inertia-native-v1\",\"passed\":true,\"cases\":%u,\"assertions\":%zu,\"words_per_case\":90}\n",n,assertions);return 0;
}
