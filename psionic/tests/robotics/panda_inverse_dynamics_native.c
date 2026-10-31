/* New inverse-force ABI/admission harness. All numerical references are outside
   this file: original SDK facts and independently differentiated matrix metric. */
#include "panda-inverse-dynamics.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
static size_t checks;
#define NEED(x) do{++checks;if(!(x)){fprintf(stderr,"Panda inverse native failure line%d\n",__LINE__);exit(1);}}while(0)
_Static_assert(sizeof(coax_t_PidOutput)==72&&_Alignof(coax_t_PidOutput)==8&&offsetof(coax_t_PidOutput,slide)==56,"seven energy/rad plus two force coefficients erase to nine F64 words");
static coax_t_PfkStatus(*const typed)(coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_f64,coax_v_t_PidOutput)=pid_inverse;
static double real(uint64_t x){double y;memcpy(&y,&x,8);return y;}
static void read_bytes(FILE*f,void*p,size_t n){NEED(fread(p,1,n,f)==n);}static void write_bytes(FILE*f,const void*p,size_t n){NEED(fwrite(p,1,n,f)==n);}
static void refuse(double*q,double*v,double*a,const int64_t lens[7],coax_t_PfkStatus expected){
 coax_t_PidOutput out[2],old[2];double oq[10],ov[10],oa[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);memcpy(oa,a,sizeof oa);memset(out,0xa5,sizeof out);memcpy(old,out,sizeof old);
 NEED(typed((coax_v_f64){q,lens[0]},(coax_v_f64){q+7,lens[1]},(coax_v_f64){v,lens[2]},(coax_v_f64){v+7,lens[3]},(coax_v_f64){a,lens[4]},(coax_v_f64){a+7,lens[5]},(coax_v_t_PidOutput){out,lens[6]})==expected);NEED(!memcmp(out,old,sizeof out));NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov)&&!memcmp(a,oa,sizeof oa));
}
static void refuse_raw(double*q,double*v,double*a,const int64_t lens[4],coax_t_PfkStatus expected){
 coax_t_PidOutput out[2],old[2];double oq[10],ov[10],oa[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);memcpy(oa,a,sizeof oa);memset(out,0xa5,sizeof out);memcpy(old,out,sizeof old);
 NEED(pid_inverse_raw((coax_v_f64){q,lens[0]},(coax_v_f64){v,lens[1]},(coax_v_f64){a,lens[2]},(coax_v_t_PidOutput){out,lens[3]})==expected);NEED(!memcmp(out,old,sizeof out));NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov)&&!memcmp(a,oa,sizeof oa));
}
static void refused_both(double*q,double*v,double*a,coax_t_PfkStatus status){const int64_t t[]={7,2,7,2,7,2,1},r[]={9,9,9,1};refuse(q,v,a,t,status);refuse_raw(q,v,a,r,status);}
static void accepted(double*q,double*v,double*a){
 coax_t_PidOutput out[2],other;double oq[10],ov[10],oa[10];memcpy(oq,q,sizeof oq);memcpy(ov,v,sizeof ov);memcpy(oa,a,sizeof oa);memset(out,0xa5,sizeof out);coax_t_PidOutput tail=out[1];
 NEED(typed((coax_v_f64){q,7},(coax_v_f64){q+7,2},(coax_v_f64){v,7},(coax_v_f64){v+7,2},(coax_v_f64){a,7},(coax_v_f64){a+7,2},(coax_v_t_PidOutput){out,1})==COAX_PfkStatus_ok);NEED(pid_inverse_raw((coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_f64){a,9},(coax_v_t_PidOutput){&other,1})==COAX_PfkStatus_ok);NEED(!memcmp(out,&other,sizeof other)&&!memcmp(&out[1],&tail,sizeof tail));NEED(!memcmp(q,oq,sizeof oq)&&!memcmp(v,ov,sizeof ov)&&!memcmp(a,oa,sizeof oa));for(int i=0;i<7;i++)NEED(isfinite(out[0].hinge.e[i]));for(int i=0;i<2;i++)NEED(isfinite(out[0].slide.e[i]));
}
static void faults(void){
 double q[10]={0},v[10]={0},a[10]={0};int64_t t[]={7,2,7,2,7,2,1},r[]={9,9,9,1};
 for(int field=0;field<7;field++){int64_t good=t[field];for(int64_t n=-1;n<=good+1;n++)if(n!=good){t[field]=n;refuse(q,v,a,t,COAX_PfkStatus_shape);}t[field]=good;}
 for(int field=0;field<4;field++){int64_t good=r[field];for(int64_t n=-1;n<=good+1;n++)if(n!=good){r[field]=n;refuse_raw(q,v,a,r,COAX_PfkStatus_shape);}r[field]=good;}
 const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff0000000000001),UINT64_C(0x7ff8000000000001),UINT64_C(0xfff8000000000001)};
 for(int role=0;role<3;role++){double*values=role==0?q:role==1?v:a;for(int j=0;j<9;j++){double limit=role==0?(j<7?100:1):role==1?(j<7?20:2):(j<7?200:20);for(size_t k=0;k<5;k++){values[j]=real(bad[k]);refused_both(q,v,a,COAX_PfkStatus_number);}for(int sign=-1;sign<=1;sign+=2){values[j]=sign*nextafter(limit,INFINITY);refused_both(q,v,a,COAX_PfkStatus_range);values[j]=sign*limit;accepted(q,v,a);}values[j]=0;}}
 q[0]=101;v[0]=real(UINT64_C(0x7ff8000000000001));refused_both(q,v,a,COAX_PfkStatus_range);q[0]=0;q[8]=2;refused_both(q,v,a,COAX_PfkStatus_range);q[8]=0;refused_both(q,v,a,COAX_PfkStatus_number);v[0]=21;v[1]=real(UINT64_C(0x7ff8000000000001));refused_both(q,v,a,COAX_PfkStatus_range);v[0]=real(UINT64_C(0x7ff8000000000001));v[1]=21;refused_both(q,v,a,COAX_PfkStatus_number);v[0]=v[1]=0;v[7]=3;a[0]=real(UINT64_C(0x7ff8000000000001));refused_both(q,v,a,COAX_PfkStatus_range);v[7]=0;a[0]=201;a[7]=real(UINT64_C(0x7ff8000000000001));refused_both(q,v,a,COAX_PfkStatus_range);t[0]=6;refuse(q,v,a,t,COAX_PfkStatus_shape);r[3]=0;refuse_raw(q,v,a,r,COAX_PfkStatus_shape);
}
int main(int argc,char**argv){
 if(argc!=3)return 2;FILE*input=fopen(argv[1],"rb"),*output=fopen(argv[2],"wbx");NEED(input&&output);uint32_t header[2];read_bytes(input,header,sizeof header);NEED(header[0]==UINT32_C(0x31444950)&&header[1]==24);
 for(uint32_t n=0;n<header[1];n++){double values[27];read_bytes(input,values,sizeof values);double q[10]={0},v[10]={0},a[10]={0};memcpy(q,values,9*8);memcpy(v,values+9,9*8);memcpy(a,values+18,9*8);accepted(q,v,a);coax_t_PidOutput out;NEED(pid_inverse_raw((coax_v_f64){q,9},(coax_v_f64){v,9},(coax_v_f64){a,9},(coax_v_t_PidOutput){&out,1})==COAX_PfkStatus_ok);write_bytes(output,&out,sizeof out);}
 NEED(fgetc(input)==EOF);faults();NEED(!fclose(input)&&!fclose(output));printf("{\"schema\":\"psionic-panda-inverse-dynamics-native-v1\",\"passed\":true,\"cases\":24,\"words_per_case\":9,\"inclusive_boundaries\":54,\"assertions\":%zu}\n",checks);return 0;
}
