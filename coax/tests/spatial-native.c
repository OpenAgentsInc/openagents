/* NEW M3 only. Independent volatile raw-coordinate IEEE oracle and real aggregate ABI. */
#define _POSIX_C_SOURCE 200809L
#include "spatial-types.h"
#include <assert.h>
#include <fenv.h>
#include <float.h>
#include <math.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#if defined(__SSE__)
#include <xmmintrin.h>
#endif
_Static_assert(sizeof(float)==4&&sizeof(double)==8&&FLT_RADIX==2&&FLT_MANT_DIG==24&&DBL_MANT_DIG==53,"IEEE carriers");
struct Raw32{float x,y,z;};struct Raw64{double x,y,z;};
struct RawArrays32{struct Raw32 p[2],v[2];};struct RawArrays64{struct Raw64 p[2],v[2];};
struct RawRecord{uint8_t lead;struct Raw32 point;struct Raw64 vector,points[2];uint16_t tail;};
struct RawView{void*data;int64_t count;};
#define LAYOUT(T,R) _Static_assert(sizeof(T)==sizeof(R),#T " size");_Static_assert(_Alignof(T)==_Alignof(R),#T " align")
#define OFFSET(T,R,F) _Static_assert(offsetof(T,F)==offsetof(R,F),#T " offset " #F)
LAYOUT(P32,struct Raw32);OFFSET(P32,struct Raw32,x);OFFSET(P32,struct Raw32,y);OFFSET(P32,struct Raw32,z);
LAYOUT(V32,struct Raw32);OFFSET(V32,struct Raw32,x);OFFSET(V32,struct Raw32,y);OFFSET(V32,struct Raw32,z);
LAYOUT(S32,struct Raw32);OFFSET(S32,struct Raw32,x);OFFSET(S32,struct Raw32,y);OFFSET(S32,struct Raw32,z);
LAYOUT(PV32,struct RawView);OFFSET(PV32,struct RawView,data);OFFSET(PV32,struct RawView,count);
LAYOUT(VV32,struct RawView);OFFSET(VV32,struct RawView,data);OFFSET(VV32,struct RawView,count);
LAYOUT(A32,struct RawArrays32);OFFSET(A32,struct RawArrays32,p);OFFSET(A32,struct RawArrays32,v);
static V32(*volatile pp32)(P32,P32)=SPN_pp32;
LAYOUT(P64,struct Raw64);OFFSET(P64,struct Raw64,x);OFFSET(P64,struct Raw64,y);OFFSET(P64,struct Raw64,z);
LAYOUT(V64,struct Raw64);OFFSET(V64,struct Raw64,x);OFFSET(V64,struct Raw64,y);OFFSET(V64,struct Raw64,z);
LAYOUT(S64,struct Raw64);OFFSET(S64,struct Raw64,x);OFFSET(S64,struct Raw64,y);OFFSET(S64,struct Raw64,z);
LAYOUT(PV64,struct RawView);OFFSET(PV64,struct RawView,data);OFFSET(PV64,struct RawView,count);
LAYOUT(VV64,struct RawView);OFFSET(VV64,struct RawView,data);OFFSET(VV64,struct RawView,count);
LAYOUT(A64,struct RawArrays64);OFFSET(A64,struct RawArrays64,p);OFFSET(A64,struct RawArrays64,v);
static V64(*volatile pp64)(P64,P64)=SPN_pp64;
LAYOUT(Record,struct RawRecord);OFFSET(Record,struct RawRecord,lead);OFFSET(Record,struct RawRecord,point);OFFSET(Record,struct RawRecord,vector);OFFSET(Record,struct RawRecord,points);OFFSET(Record,struct RawRecord,tail);

static uint64_t verified32,verified64;static int64_t trace[128],trap_id;static int trace_count,trace_fd=-1;
static void tick(int64_t id){
 assert(trace_count<128);trace[trace_count++]=id;
 if(trace_fd>=0){assert(write(trace_fd,&id,sizeof id)==(ssize_t)sizeof id);}
 if(id==trap_id){raise(SIGUSR1);_exit(97);}
}
float SPN_tick32(int64_t id){tick(id);return(float)id;}double SPN_tick64(int64_t id){tick(id);return(double)id;}
static uint32_t bits32(float x){uint32_t u;memcpy(&u,&x,4);return u;}static uint64_t bits64(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static float from32(uint32_t u){float x;memcpy(&x,&u,4);return x;}static double from64(uint64_t u){double x;memcpy(&x,&u,8);return x;}
static void get(FILE*f,void*p,size_t n){assert(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){assert(fwrite(p,1,n,f)==n);}
static void check32(FILE*out,float a,float b){assert((isnan(a)&&isnan(b))||bits32(a)==bits32(b));uint32_t u=isnan(a)?UINT32_C(0x7fc00000):bits32(a);put(out,&u,4);verified32++;}
static void check64(FILE*out,double a,double b){assert((isnan(a)&&isnan(b))||bits64(a)==bits64(b));uint64_t u=isnan(a)?UINT64_C(0x7ff8000000000000):bits64(a);put(out,&u,8);verified64++;}
static void ordered(const int64_t*ids,int n){assert(trace_count==n);for(int i=0;i<n;i++)assert(trace[i]==ids[i]);}
static void rows32(FILE*in,FILE*out,uint32_t count){for(uint32_t i=0;i<count;i++){
uint32_t words[7];get(in,words,sizeof words);float raw[7];for(int j=0;j<7;j++)raw[j]=from32(words[j]);volatile float x[3]={raw[0],raw[1],raw[2]},y[3]={raw[3],raw[4],raw[5]},scalar=raw[6];float scale=raw[6];
P32 p=SPN_point32(raw[0],raw[1],raw[2]),q=SPN_point32(raw[3],raw[4],raw[5]);V32 a=SPN_vector32(raw[0],raw[1],raw[2]),b=SPN_vector32(raw[3],raw[4],raw[5]);S32 sa=SPN_second32(raw[0],raw[1],raw[2]),sb=SPN_second32(raw[3],raw[4],raw[5]);
check32(out,p.x,raw[0]);check32(out,b.x,raw[3]);
check32(out,p.y,raw[1]);check32(out,b.y,raw[4]);
check32(out,p.z,raw[2]);check32(out,b.z,raw[5]);
{V32 got=pp32(p,q);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]-y[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{P32 got=SPN_pvadd32(p,b);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]+y[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{P32 got=SPN_vpadd32(a,q);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]+y[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{P32 got=SPN_pvsub32(p,b);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]-y[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{V32 got=SPN_vvadd32(a,b);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]+y[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{V32 got=SPN_vvsub32(a,b);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]-y[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{V32 got=SPN_vmul32(a,scale);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]*scalar;check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{V32 got=SPN_smul32(scale,a);float expected[3];for(int j=0;j<3;j++)expected[j]=scalar*x[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{V32 got=SPN_vdiv32(a,scale);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]/scalar;check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{V32 got=SPN_vneg32(a);float expected[3];for(int j=0;j<3;j++)expected[j]=-x[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
{S32 got=SPN_secondadd32(sa,sb);float expected[3];for(int j=0;j<3;j++)expected[j]=x[j]+y[j];check32(out,got.x,expected[0]);check32(out,got.y,expected[1]);check32(out,got.z,expected[2]);}
}}
static void containers32(FILE*out){A32 a={0};for(int i=0;i<2;i++){a.p.e[i]=SPN_point32(i+1,i+2,i+3);a.v.e[i]=SPN_vector32(i+4,i+5,i+6);}A32 b=SPN_arrays32(a);
for(int i=0;i<2;i++)check32(out,b.p.e[i].x,a.p.e[i].x);
for(int i=0;i<2;i++)check32(out,b.p.e[i].y,a.p.e[i].y);
for(int i=0;i<2;i++)check32(out,b.p.e[i].z,a.p.e[i].z);
for(int i=0;i<2;i++)check32(out,b.v.e[i].x,a.v.e[i].x);
for(int i=0;i<2;i++)check32(out,b.v.e[i].y,a.v.e[i].y);
for(int i=0;i<2;i++)check32(out,b.v.e[i].z,a.v.e[i].z);
V32 r=SPN_view32((PV32){a.p.e,2},(VV32){a.v.e,2});
{volatile float left=a.p.e[0].x-a.p.e[1].x,right=a.v.e[0].x+a.v.e[1].x;check32(out,r.x,left+right);}
{volatile float left=a.p.e[0].y-a.p.e[1].y,right=a.v.e[0].y+a.v.e[1].y;check32(out,r.y,left+right);}
{volatile float left=a.p.e[0].z-a.p.e[1].z,right=a.v.e[0].z+a.v.e[1].z;check32(out,r.z,left+right);}
P32 points[1]={SPN_point32(1,2,3)};P32 snapshot=SPN_alias32((PV32){points,1});
check32(out,snapshot.x,3);check32(out,points[0].x,101);
check32(out,snapshot.y,5);check32(out,points[0].y,202);
check32(out,snapshot.z,7);check32(out,points[0].z,303);
}
static void orders32(FILE*out){
{trace_count=0;V32 got=SPN_ordered_pp32();ordered((int64_t[]){11,12,13,21,22,23},6);check32(out,got.x,(float)(11-21));check32(out,got.y,(float)(12-22));check32(out,got.z,(float)(13-23));}
{trace_count=0;P32 got=SPN_ordered_pvadd32();ordered((int64_t[]){31,32,33,41,42,43},6);check32(out,got.x,(float)(31+41));check32(out,got.y,(float)(32+42));check32(out,got.z,(float)(33+43));}
{trace_count=0;P32 got=SPN_ordered_vpadd32();ordered((int64_t[]){51,52,53,61,62,63},6);check32(out,got.x,(float)(51+61));check32(out,got.y,(float)(52+62));check32(out,got.z,(float)(53+63));}
{trace_count=0;P32 got=SPN_ordered_pvsub32();ordered((int64_t[]){71,72,73,81,82,83},6);check32(out,got.x,(float)(71-81));check32(out,got.y,(float)(72-82));check32(out,got.z,(float)(73-83));}
{trace_count=0;V32 got=SPN_ordered_vvadd32();ordered((int64_t[]){91,92,93,101,102,103},6);check32(out,got.x,(float)(91+101));check32(out,got.y,(float)(92+102));check32(out,got.z,(float)(93+103));}
{trace_count=0;V32 got=SPN_ordered_vvsub32();ordered((int64_t[]){111,112,113,121,122,123},6);check32(out,got.x,(float)(111-121));check32(out,got.y,(float)(112-122));check32(out,got.z,(float)(113-123));}
{trace_count=0;V32 got=SPN_ordered_vmul32();ordered((int64_t[]){131,132,133,141},4);check32(out,got.x,(float)(131*141));check32(out,got.y,(float)(132*141));check32(out,got.z,(float)(133*141));}
{trace_count=0;V32 got=SPN_ordered_smul32();ordered((int64_t[]){151,161,162,163},4);check32(out,got.x,(float)(151*161));check32(out,got.y,(float)(151*162));check32(out,got.z,(float)(151*163));}
{trace_count=0;V32 got=SPN_ordered_vdiv32();ordered((int64_t[]){171,172,173,181},4);check32(out,got.x,(float)171/(float)181);check32(out,got.y,(float)172/(float)181);check32(out,got.z,(float)173/(float)181);}
{trace_count=0;V32 got=SPN_ordered_vneg32();ordered((int64_t[]){191,192,193},3);check32(out,got.x,(float)(-191));check32(out,got.y,(float)(-192));check32(out,got.z,(float)(-193));}
}
static void rows64(FILE*in,FILE*out,uint32_t count){for(uint32_t i=0;i<count;i++){
uint64_t words[7];get(in,words,sizeof words);double raw[7];for(int j=0;j<7;j++)raw[j]=from64(words[j]);volatile double x[3]={raw[0],raw[1],raw[2]},y[3]={raw[3],raw[4],raw[5]},scalar=raw[6];double scale=raw[6];
P64 p=SPN_point64(raw[0],raw[1],raw[2]),q=SPN_point64(raw[3],raw[4],raw[5]);V64 a=SPN_vector64(raw[0],raw[1],raw[2]),b=SPN_vector64(raw[3],raw[4],raw[5]);S64 sa=SPN_second64(raw[0],raw[1],raw[2]),sb=SPN_second64(raw[3],raw[4],raw[5]);
check64(out,p.x,raw[0]);check64(out,b.x,raw[3]);
check64(out,p.y,raw[1]);check64(out,b.y,raw[4]);
check64(out,p.z,raw[2]);check64(out,b.z,raw[5]);
{V64 got=pp64(p,q);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]-y[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{P64 got=SPN_pvadd64(p,b);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]+y[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{P64 got=SPN_vpadd64(a,q);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]+y[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{P64 got=SPN_pvsub64(p,b);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]-y[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{V64 got=SPN_vvadd64(a,b);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]+y[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{V64 got=SPN_vvsub64(a,b);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]-y[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{V64 got=SPN_vmul64(a,scale);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]*scalar;check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{V64 got=SPN_smul64(scale,a);double expected[3];for(int j=0;j<3;j++)expected[j]=scalar*x[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{V64 got=SPN_vdiv64(a,scale);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]/scalar;check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{V64 got=SPN_vneg64(a);double expected[3];for(int j=0;j<3;j++)expected[j]=-x[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
{S64 got=SPN_secondadd64(sa,sb);double expected[3];for(int j=0;j<3;j++)expected[j]=x[j]+y[j];check64(out,got.x,expected[0]);check64(out,got.y,expected[1]);check64(out,got.z,expected[2]);}
}}
static void containers64(FILE*out){A64 a={0};for(int i=0;i<2;i++){a.p.e[i]=SPN_point64(i+1,i+2,i+3);a.v.e[i]=SPN_vector64(i+4,i+5,i+6);}A64 b=SPN_arrays64(a);
for(int i=0;i<2;i++)check64(out,b.p.e[i].x,a.p.e[i].x);
for(int i=0;i<2;i++)check64(out,b.p.e[i].y,a.p.e[i].y);
for(int i=0;i<2;i++)check64(out,b.p.e[i].z,a.p.e[i].z);
for(int i=0;i<2;i++)check64(out,b.v.e[i].x,a.v.e[i].x);
for(int i=0;i<2;i++)check64(out,b.v.e[i].y,a.v.e[i].y);
for(int i=0;i<2;i++)check64(out,b.v.e[i].z,a.v.e[i].z);
V64 r=SPN_view64((PV64){a.p.e,2},(VV64){a.v.e,2});
{volatile double left=a.p.e[0].x-a.p.e[1].x,right=a.v.e[0].x+a.v.e[1].x;check64(out,r.x,left+right);}
{volatile double left=a.p.e[0].y-a.p.e[1].y,right=a.v.e[0].y+a.v.e[1].y;check64(out,r.y,left+right);}
{volatile double left=a.p.e[0].z-a.p.e[1].z,right=a.v.e[0].z+a.v.e[1].z;check64(out,r.z,left+right);}
P64 points[1]={SPN_point64(1,2,3)};P64 snapshot=SPN_alias64((PV64){points,1});
check64(out,snapshot.x,3);check64(out,points[0].x,101);
check64(out,snapshot.y,5);check64(out,points[0].y,202);
check64(out,snapshot.z,7);check64(out,points[0].z,303);
}
static void orders64(FILE*out){
{trace_count=0;V64 got=SPN_ordered_pp64();ordered((int64_t[]){11,12,13,21,22,23},6);check64(out,got.x,(double)(11-21));check64(out,got.y,(double)(12-22));check64(out,got.z,(double)(13-23));}
{trace_count=0;P64 got=SPN_ordered_pvadd64();ordered((int64_t[]){31,32,33,41,42,43},6);check64(out,got.x,(double)(31+41));check64(out,got.y,(double)(32+42));check64(out,got.z,(double)(33+43));}
{trace_count=0;P64 got=SPN_ordered_vpadd64();ordered((int64_t[]){51,52,53,61,62,63},6);check64(out,got.x,(double)(51+61));check64(out,got.y,(double)(52+62));check64(out,got.z,(double)(53+63));}
{trace_count=0;P64 got=SPN_ordered_pvsub64();ordered((int64_t[]){71,72,73,81,82,83},6);check64(out,got.x,(double)(71-81));check64(out,got.y,(double)(72-82));check64(out,got.z,(double)(73-83));}
{trace_count=0;V64 got=SPN_ordered_vvadd64();ordered((int64_t[]){91,92,93,101,102,103},6);check64(out,got.x,(double)(91+101));check64(out,got.y,(double)(92+102));check64(out,got.z,(double)(93+103));}
{trace_count=0;V64 got=SPN_ordered_vvsub64();ordered((int64_t[]){111,112,113,121,122,123},6);check64(out,got.x,(double)(111-121));check64(out,got.y,(double)(112-122));check64(out,got.z,(double)(113-123));}
{trace_count=0;V64 got=SPN_ordered_vmul64();ordered((int64_t[]){131,132,133,141},4);check64(out,got.x,(double)(131*141));check64(out,got.y,(double)(132*141));check64(out,got.z,(double)(133*141));}
{trace_count=0;V64 got=SPN_ordered_smul64();ordered((int64_t[]){151,161,162,163},4);check64(out,got.x,(double)(151*161));check64(out,got.y,(double)(151*162));check64(out,got.z,(double)(151*163));}
{trace_count=0;V64 got=SPN_ordered_vdiv64();ordered((int64_t[]){171,172,173,181},4);check64(out,got.x,(double)171/(double)181);check64(out,got.y,(double)172/(double)181);check64(out,got.z,(double)173/(double)181);}
{trace_count=0;V64 got=SPN_ordered_vneg64();ordered((int64_t[]){191,192,193},3);check64(out,got.x,(double)(-191));check64(out,got.y,(double)(-192));check64(out,got.z,(double)(-193));}
}

/* A foreign scalar effect deliberately interrupts evaluation. This is not a typed Coax trap oracle. */
static void trap_case(FILE*out,int which,int64_t stop,const int64_t*expected,int n){
 int pipefd[2];assert(pipe(pipefd)==0);pid_t child=fork();assert(child>=0);
 if(child==0){
  close(pipefd[0]);trace_fd=pipefd[1];trap_id=stop;trace_count=0;
  struct rlimit no_core={0,0};assert(setrlimit(RLIMIT_CORE,&no_core)==0);signal(SIGUSR1,SIG_DFL);
  sigset_t unblock;sigemptyset(&unblock);sigaddset(&unblock,SIGUSR1);assert(sigprocmask(SIG_UNBLOCK,&unblock,NULL)==0);
  switch(which){
case 0:(void)SPN_trap_left32();break;
case 1:(void)SPN_trap_right32();break;
case 2:(void)SPN_trap_scalar_left32();break;
case 3:(void)SPN_trap_vector_left32();break;
case 4:(void)SPN_trap_left64();break;
case 5:(void)SPN_trap_right64();break;
case 6:(void)SPN_trap_scalar_left64();break;
case 7:(void)SPN_trap_vector_left64();break;

  default:_exit(98);
  }_exit(99);
 }
 close(pipefd[1]);int64_t ids[16];size_t used=0;for(;;){ssize_t count=read(pipefd[0],(uint8_t*)ids+used,sizeof ids-used);assert(count>=0);if(!count)break;used+=(size_t)count;assert(used<sizeof ids);}close(pipefd[0]);int status;assert(waitpid(child,&status,0)==child);assert(WIFSIGNALED(status)&&WTERMSIG(status)==SIGUSR1);assert(used==(size_t)n*sizeof(int64_t));
 for(int i=0;i<n;i++){assert(ids[i]==expected[i]);int32_t id=(int32_t)ids[i];put(out,&id,4);}int32_t signal_value=SIGUSR1;put(out,&signal_value,4);
}
int main(int argc,char**argv){
 assert(argc==3&&fegetround()==FE_TONEAREST);
 uint64_t fpcr=0;
#if defined(__aarch64__)
 __asm__ volatile("mrs %0, fpcr":"=r"(fpcr));assert(!(fpcr&(UINT64_C(1)<<24)));
#endif
#if defined(__SSE__)
 assert(!(_mm_getcsr()&((1u<<15)|(1u<<6))));
#endif
 FILE*in=fopen(argv[1],"rb"),*out=fopen(argv[2],"wb");assert(in&&out);uint32_t n32,n64;get(in,&n32,4);get(in,&n64,4);assert(n32<=5000&&n64<=5000);
 rows32(in,out,n32);rows64(in,out,n64);assert(fgetc(in)==EOF);
 containers32(out);containers64(out);
 Record record={0};record.lead=7;record.tail=513;record.point=SPN_point32(-0.0f,2.5f,-3.25f);record.vector=SPN_vector64(1,-2,3);record.points.e[0]=SPN_point64(4,5,6);record.points.e[1]=SPN_point64(-7,-8,-9);Record copy=SPN_record(record);assert(copy.lead==record.lead&&copy.tail==record.tail);
check32(out,copy.point.x,record.point.x);check64(out,copy.vector.x,record.vector.x);for(int i=0;i<2;i++)check64(out,copy.points.e[i].x,record.points.e[i].x);
check32(out,copy.point.y,record.point.y);check64(out,copy.vector.y,record.vector.y);for(int i=0;i<2;i++)check64(out,copy.points.e[i].y,record.points.e[i].y);
check32(out,copy.point.z,record.point.z);check64(out,copy.vector.z,record.vector.z);for(int i=0;i<2;i++)check64(out,copy.points.e[i].z,record.points.e[i].z);
orders32(out);orders64(out);
trap_case(out,0,9002,(int64_t[]){9001,9002},2);
trap_case(out,1,9302,(int64_t[]){9201,9202,9203,9301,9302},5);
trap_case(out,2,9502,(int64_t[]){9401,9501,9502},3);
trap_case(out,3,9602,(int64_t[]){9601,9602},2);
trap_case(out,4,9002,(int64_t[]){9001,9002},2);
trap_case(out,5,9302,(int64_t[]){9201,9202,9203,9301,9302},5);
trap_case(out,6,9502,(int64_t[]){9401,9501,9502},3);
trap_case(out,7,9602,(int64_t[]){9601,9602},2);

 assert(verified32==39*(uint64_t)n32+54&&verified64==39*(uint64_t)n64+60);
 assert(!fclose(in)&&!fclose(out));printf("spatial native: %u f32 rows, %u f64 rows, %llu f32 + %llu f64 observations, 20 ordered aggregates, 8 interrupted-order cases, FPCR=%llu\n",n32,n64,(unsigned long long)verified32,(unsigned long long)verified64,(unsigned long long)fpcr);return 0;
}
