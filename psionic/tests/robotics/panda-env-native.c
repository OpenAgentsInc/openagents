/* Firm reason2: independent libc/IEEE native oracle, no simulator dependency. */
#include "panda-env-generated.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
static unsigned long cases,words,observations;
#define CHECK(x) do{cases++;if(!(x)){fprintf(stderr,"panda-env native line%d\n",__LINE__);exit(1);}}while(0)
static coax_v_f64 d(double *p,int64_t n){return(coax_v_f64){p,n};}
static coax_v_f32 f(float *p,int64_t n){return(coax_v_f32){p,n};}
static uint32_t bits(float x){uint32_t b;memcpy(&b,&x,4);return b;}
static double from(uint64_t b){double x;memcpy(&x,&b,8);return x;}
static uint64_t rng=UINT64_C(169);
static double random_unit(void){rng=rng*UINT64_C(6364136223846793005)+1;return(double)(rng>>11)*0x1p-53;}
static void observation(double *p,double *q,double *finger,int after){
 float out[10],expected[8];for(int i=0;i<10;i++)out[i]=713.25f;
 double w=fmax(-1.,fmin(1.,q[0])),den=sqrt(1-w*w);
 for(int i=0;i<3;i++){expected[i]=(float)p[i];expected[3+i]=den==0?0.f:(float)(((q[1+i]*2)*acos(w))/den);}
 expected[6]=(float)finger[0];expected[7]=(float)finger[1];
 coax_t_PeRead r=pe_observation(d(p,3),d(q,4),d(finger,2),f(out,10),after,0);
 CHECK(r.status==0&&r.written==8&&r.derived_lag==(after?.002:0)&&r.quaternion_clamped==(q[0]>1||q[0]<-1));
 for(int i=0;i<8;i++){if(bits(out[i])!=bits(expected[i])){fprintf(stderr,"obs%lu axis%d actual%08x expected%08x w%.17g\n",observations,i,bits(out[i]),bits(expected[i]),q[0]);exit(1);}words++;cases++;}
 CHECK(out[8]==713.25f&&out[9]==713.25f);observations++;
}
int main(void){
 CHECK(pe_profile(23,21,9,0,45,.002));CHECK(!pe_profile(INT64_MIN,21,9,0,45,.002));CHECK(!pe_profile(23,INT64_MAX,9,0,45,.002));
 CHECK(pe_binding(INT64_MIN).count==0&&pe_binding(INT64_MAX).count==0);
 double p[3]={.1,.2,.3},q[4]={1,0,0,0},finger[2]={.03,-.03};
 for(int k=0;k<10001;k++){
  double norm=0;for(int i=0;i<4;i++){q[i]=2*random_unit()-1;norm+=q[i]*q[i];}norm=sqrt(norm);for(int i=0;i<4;i++)q[i]/=norm;
  for(int i=0;i<3;i++)p[i]=4*random_unit()-2;finger[0]=.04*random_unit();finger[1]=-.04*random_unit();observation(p,q,finger,k&1);
 }
 for(int k=0;k<=2000;k++){double w=(double)k/1000.-1;q[0]=w;q[1]=sqrt(fmax(0,1-w*w));q[2]=0;q[3]=-0.;observation(p,q,finger,k&1);}
 for(int sign=-1;sign<=1;sign+=2){q[0]=sign*1.00000001;q[1]=q[2]=q[3]=0;observation(p,q,finger,0);}
 q[0]=1;q[1]=q[2]=q[3]=0;
 const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000000001)};
 for(int kind=0;kind<3;kind++)for(int axis=0;axis<(kind==0?3:kind==1?4:2);axis++)for(int b=0;b<3;b++){
  double *array=kind==0?p:kind==1?q:finger;double saved=array[axis];array[axis]=from(bad[b]);float out[10],poison[10];for(int i=0;i<10;i++)out[i]=poison[i]=-71.25f;
  coax_t_PeRead r=pe_observation(d(p,3),d(q,4),d(finger,2),f(out,10),0,0);CHECK(r.status==COAX_PeStatus_number&&r.written==0&&!memcmp(out,poison,sizeof(out)));array[axis]=saved;
 }
 float out[10],poison[10];for(int i=0;i<10;i++)out[i]=poison[i]=91.5f;
 int64_t counts[]={INT64_MIN,-1,0,1,2,4,7,9,INT64_MAX};
 for(int field=0;field<4;field++)for(unsigned i=0;i<sizeof(counts)/sizeof(*counts);i++){
  int64_t n=counts[i];if((field==0&&n==3)||(field==1&&n==4)||(field==2&&n==2)||(field==3&&n>=8))continue;
  coax_t_PeRead r=pe_observation(d(p,field==0?n:3),d(q,field==1?n:4),d(finger,field==2?n:2),f(out,field==3?n:10),0,0);CHECK(r.status==COAX_PeStatus_shape&&!memcmp(out,poison,sizeof(out)));
 }
 CHECK(pe_observation(d(p,3),d(q,4),d(finger,2),f(out,10),0,1).status==COAX_PeStatus_overlap);CHECK(!memcmp(out,poison,sizeof(out)));
 double pos[3]={0},matrix[9]={1,0,0,0,1,0,0,0,1},size[3]={1,2,3},object[3]={0};
 for(int axis=0;axis<3;axis++)for(int side=-1;side<=1;side+=2){object[axis]=side*size[axis]-(axis==2&&side<0?.01:0);CHECK(!pe_success(d(pos,3),d(matrix,9),d(size,3),d(object,3)).success);object[axis]=0;}
 for(int k=0;k<10001;k++){
  double angle=random_unit()*6.283185307179586,c=cos(angle),s=sin(angle);matrix[0]=c;matrix[1]=-s;matrix[3]=s;matrix[4]=c;
  for(int i=0;i<3;i++){pos[i]=random_unit();size[i]=.01+random_unit();object[i]=pos[i]+(random_unit()*4-2)*size[i];}
  int expected=1;for(int i=0;i<3;i++){double extent=fabs(matrix[3*i]*size[0]+matrix[3*i+1]*size[1]+matrix[3*i+2]*size[2]);double lower=pos[i]-extent-(i==2?.01:0);if(!(object[i]>lower&&object[i]<pos[i]+extent))expected=0;}
  coax_t_PeSuccess r=pe_success(d(pos,3),d(matrix,9),d(size,3),d(object,3));CHECK(r.status==0&&r.success==expected);
 }
 printf("panda-env native: %lu assertions; %lu observations/%lu exact F32 words\n",cases,observations,words);return 0;
}
