/* Gaussian API transport and boundary witnesses. No C density/RNG/exp oracle. */
#include "locomotion-gaussian.h"
#include <float.h>
#include <inttypes.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

_Static_assert(sizeof(double)==8 && DBL_MANT_DIG==53,"IEEE F64");
_Static_assert(sizeof(coax_t_CrSeed)==16 && sizeof(coax_t_RgpAction)==8,"seed/action ABI");
_Static_assert(sizeof(coax_t_RgpDensity)==32 && sizeof(coax_t_RgpValue)==16 && sizeof(coax_t_RgpDraw)==32,"Gaussian ABI");
_Static_assert(sizeof(coax_t_RlrOutput)==56,"ratio ABI");
_Static_assert(offsetof(coax_t_RgpDraw,next)==16 && offsetof(coax_t_RlrOutput,objective)==16,"nested offsets");

static uint64_t assertions,refusals,aliases,ieee_patterns,probe_calls;
#define NEED(x) do{assertions++;if(!(x)){fprintf(stderr,"Gaussian native:%d: %s\n",__LINE__,#x);exit(1);}}while(0)
static const size_t WIDTH[4]={32,16,32,56};
static const int INPUTS[4]={3,4,2,4};
typedef struct {uint64_t before;unsigned char bytes[168];uint64_t after;} Guard;
static double number(uint64_t bits){double x;memcpy(&x,&bits,8);return x;}
static uint64_t word(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static void poison(Guard*g){memset(g,0x3c,sizeof *g);g->before=UINT64_C(0x3141592653589793);g->after=UINT64_C(0x2718281828459045);}
static uint8_t invoke(int kind,int typed,coax_t_CrSeed seed,const double p[4],void*out,int64_t count){
  coax_t_RgpAction mean={p[0]},action={p[2]};coax_t_RgaReward advantage={p[2]};
  if(kind==0)return typed?rgp_density(mean,p[1],action,(coax_v_t_RgpDensity){out,count}):rgp_density_raw(p[0],p[1],p[2],(coax_v_t_RgpDensity){out,count});
  if(kind==1)return typed?rgp_from_uniforms(mean,p[1],p[2],p[3],(coax_v_t_RgpValue){out,count}):rgp_from_uniforms_raw(p[0],p[1],p[2],p[3],(coax_v_t_RgpValue){out,count});
  if(kind==2)return typed?rgp_sample(seed,mean,p[1],(coax_v_t_RgpDraw){out,count}):rgp_sample_raw(seed,p[0],p[1],(coax_v_t_RgpDraw){out,count});
  return typed?rlr_surrogate(p[0],p[1],advantage,p[3],(coax_v_t_RlrOutput){out,count}):rlr_surrogate_raw(p[0],p[1],p[2],p[3],(coax_v_t_RlrOutput){out,count});
}
static void defaults(int kind,double p[4]){
  p[0]=.125;p[1]=-.25;p[2]=.25;p[3]=.75;
  if(kind==3){p[0]=.125;p[1]=0;p[2]=-.75;p[3]=.25;}
}
static void output_finite(int kind,const void*p){
  double words[6];size_t n=kind==0?4:kind==3?6:2;memcpy(words,p,n*8);
  for(size_t i=0;i<n;i++)NEED(isfinite(words[i]));
  if(kind==3){coax_t_RlrOutput x;memcpy(&x,p,sizeof x);NEED(x.objective.clipped_active==0||x.objective.clipped_active==1);}
}
static uint8_t checked(int kind,coax_t_CrSeed seed,const double p[4],unsigned char *payload){
  Guard a,b,saved;poison(&a);poison(&b);saved=a;double inputs[4];memcpy(inputs,p,sizeof inputs);coax_t_CrSeed old=seed;
  uint8_t x=invoke(kind,0,seed,p,a.bytes,1),y=invoke(kind,1,seed,p,b.bytes,1);
  NEED(x==y && x<=(kind==2?4:3));NEED(!memcmp(&a,&b,sizeof a));NEED(!memcmp(inputs,p,sizeof inputs)&&!memcmp(&old,&seed,sizeof seed));
  if(x){NEED(!memcmp(&a,&saved,sizeof a));memset(payload,0,WIDTH[kind]);}
  else {NEED(a.before==saved.before&&a.after==saved.after);NEED(!memcmp(a.bytes+WIDTH[kind],saved.bytes+WIDTH[kind],sizeof a.bytes-WIDTH[kind]));output_finite(kind,a.bytes);memcpy(payload,a.bytes,WIDTH[kind]);}
  return x;
}
static void refuse(int kind,uint8_t expected,coax_t_CrSeed seed,const double p[4],int64_t count){
  for(int typed=0;typed<2;typed++){
    Guard a,saved;poison(&a);saved=a;NEED(invoke(kind,typed,seed,p,a.bytes,count)==expected);NEED(!memcmp(&a,&saved,sizeof a));refusals++;
  }
}
static void admission(void){
  const int64_t badcount[]={INT64_MIN,-1,0,2,INT64_MAX};coax_t_CrSeed seed={7,0};double p[4];
  for(int k=0;k<4;k++)for(size_t j=0;j<5;j++){defaults(k,p);refuse(k,1,seed,p,badcount[j]);}
  const uint64_t bad[]={UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000000001),UINT64_C(0x7ff0000000000001),UINT64_C(0xfff8000000000013)};
  for(int k=0;k<4;k++)for(int lane=0;lane<INPUTS[k];lane++)for(size_t j=0;j<5;j++){
    defaults(k,p);p[lane]=number(bad[j]);refuse(k,2,seed,p,1);ieee_patterns++;
  }
  for(int k=0;k<3;k++){
    defaults(k,p);p[0]=nextafter(16.,INFINITY);refuse(k,3,seed,p,1);p[0]=-nextafter(16.,INFINITY);refuse(k,3,seed,p,1);
    defaults(k,p);p[1]=nextafter(-4.,-INFINITY);refuse(k,3,seed,p,1);p[1]=nextafter(2.,INFINITY);refuse(k,3,seed,p,1);
  }
  defaults(0,p);p[2]=nextafter(128.,INFINITY);refuse(0,3,seed,p,1);p[2]=-nextafter(128.,INFINITY);refuse(0,3,seed,p,1);
  for(int lane=2;lane<4;lane++){
    defaults(1,p);p[lane]=nextafter(0.,-INFINITY);refuse(1,3,seed,p,1);p[lane]=1.;refuse(1,3,seed,p,1);p[lane]=nextafter(1.,INFINITY);refuse(1,3,seed,p,1);
  }
  defaults(2,p);seed.counter=UINT64_MAX-1;refuse(2,4,seed,p,1);seed.counter=UINT64_MAX;refuse(2,4,seed,p,1);
  p[0]=number(bad[2]);refuse(2,2,seed,p,1);defaults(2,p);p[1]=3;refuse(2,3,seed,p,1);seed.counter=0;
  defaults(3,p);p[2]=nextafter(1e9,INFINITY);refuse(3,3,seed,p,1);p[2]=-nextafter(1e9,INFINITY);refuse(3,3,seed,p,1);
  defaults(3,p);p[3]=nextafter(0.,-INFINITY);refuse(3,3,seed,p,1);p[3]=nextafter(1.,INFINITY);refuse(3,3,seed,p,1);
  defaults(3,p);p[0]=nextafter(-0x1.bb9d3beb8c86bp+3,-INFINITY);p[1]=0;refuse(3,3,seed,p,1);
  p[0]=nextafter(0x1.62e42fefa39efp+1,INFINITY);refuse(3,3,seed,p,1);
  p[0]=DBL_MAX;p[1]=-DBL_MAX;refuse(3,3,seed,p,1);p[0]=-DBL_MAX;p[1]=DBL_MAX;refuse(3,3,seed,p,1);
  /* Global finite admission precedes domain and counter decisions. */
  defaults(0,p);p[0]=17;p[2]=number(bad[2]);refuse(0,2,seed,p,1);
  defaults(1,p);p[2]=0;p[3]=number(bad[2]);refuse(1,2,seed,p,1);p[3]=1;refuse(1,3,seed,p,1);
  defaults(3,p);p[2]=nextafter(1e9,INFINITY);p[3]=number(bad[2]);refuse(3,2,seed,p,1);
  for(int k=0;k<4;k++){for(int j=0;j<4;j++)p[j]=number(bad[2]);refuse(k,1,seed,p,0);}
}
static void endpoints(void){
  double p[4];unsigned char out[56];coax_t_CrSeed seed={7,0};
  for(int sign=-1;sign<=1;sign+=2){defaults(0,p);p[0]=sign*16.;p[2]=sign*128.;NEED(!checked(0,seed,p,out));}
  for(int k=0;k<3;k++)for(int edge=0;edge<2;edge++){defaults(k,p);p[1]=edge?2.:-4.;NEED(!checked(k,seed,p,out));}
  for(int sign=-1;sign<=1;sign+=2){defaults(0,p);p[0]=sign<0?-0.:0.;p[2]=sign<0?0.:-0.;NEED(!checked(0,seed,p,out));coax_t_RgpDensity d;memcpy(&d,out,sizeof d);NEED(word(d.score_mean)==0&&d.score_logstd==-1);}
  for(int sign=-1;sign<=1;sign+=2){defaults(1,p);p[0]=sign<0?-0.:.125;p[2]=sign<0?-0.:0.;p[3]=nextafter(1.,0.);NEED(!checked(1,seed,p,out));coax_t_RgpValue v;memcpy(&v,out,sizeof v);NEED(word(v.action.value)==word(p[0])&&word(v.z)==0);}
  defaults(1,p);p[2]=number(1);p[3]=.5;NEED(!checked(1,seed,p,out));
  defaults(1,p);p[2]=nextafter(1.,0.);p[3]=nextafter(1.,0.);NEED(!checked(1,seed,p,out));
  defaults(2,p);seed.counter=UINT64_MAX-2;NEED(!checked(2,seed,p,out));coax_t_RgpDraw d;memcpy(&d,out,sizeof d);NEED(d.next.seed==7&&d.next.counter==UINT64_MAX);seed.counter=0;
  defaults(3,p);p[0]=p[1]=DBL_MAX;NEED(!checked(3,seed,p,out));p[0]=p[1]=-DBL_MAX;NEED(!checked(3,seed,p,out));
  for(int sign=-1;sign<=1;sign+=2)for(int e=0;e<2;e++){defaults(3,p);p[0]=p[1]=0;p[2]=sign*1e9;p[3]=(double)e;NEED(!checked(3,seed,p,out));}
  /* Inward endpoint calls are observations: backend exp may be outside rho. */
  double ends[]={-0x1.bb9d3beb8c86bp+3,0x1.62e42fefa39efp+1};
  for(size_t i=0;i<2;i++){defaults(3,p);p[0]=ends[i];p[1]=0;uint8_t s=checked(3,seed,p,out);NEED(s==0||s==3);}
}
static void aliasing(void){
  const int counts[]={3,4,3,4};
  for(int k=0;k<4;k++)for(int typed=0;typed<2;typed++)for(int shift=0;shift<counts[k];shift++){
    union {uint64_t align;unsigned char bytes[160];} u;memset(&u,0x3c,sizeof u);double p[4];defaults(k,p);coax_t_CrSeed seed={7,3};
    if(k==2){memcpy(u.bytes,&seed,16);memcpy(u.bytes+16,p,16);}else memcpy(u.bytes,p,INPUTS[k]*8);
    unsigned char before[sizeof u],want[56];memcpy(before,&u,sizeof u);Guard base;poison(&base);
    NEED(!invoke(k,typed,seed,p,base.bytes,1));memcpy(want,base.bytes,WIDTH[k]);
    double actual[4]={0};coax_t_CrSeed state=seed;
    if(k==2){memcpy(&state,u.bytes,16);memcpy(actual,u.bytes+16,16);}else memcpy(actual,u.bytes,INPUTS[k]*8);
    NEED(!invoke(k,typed,state,actual,u.bytes+shift*8,1));NEED(!memcmp(u.bytes+shift*8,want,WIDTH[k]));
    for(size_t b=0;b<sizeof u;b++)if(b<(size_t)shift*8||b>=(size_t)shift*8+WIDTH[k])NEED(u.bytes[b]==before[b]);aliases++;
    memcpy(&u,before,sizeof u);if(k==2){state.counter=UINT64_MAX;memcpy(u.bytes,&state,16);}else{double nan=number(UINT64_C(0x7ff8000000000001));memcpy(u.bytes,&nan,8);}
    memcpy(before,&u,sizeof u);memset(actual,0,sizeof actual);if(k==2){memcpy(&state,u.bytes,16);memcpy(actual,u.bytes+16,16);}else memcpy(actual,u.bytes,INPUTS[k]*8);
    NEED(invoke(k,typed,state,actual,u.bytes+shift*8,1)==(k==2?4:2));NEED(!memcmp(&u,before,sizeof u));aliases++;
  }
}
static void *read_file(const char*name,size_t*n){FILE*f=fopen(name,"rb");NEED(f);NEED(!fseek(f,0,SEEK_END));long z=ftell(f);NEED(z>=0&&z<=8*1024*1024);rewind(f);void*p=malloc((size_t)z+1);NEED(p);NEED(fread(p,1,(size_t)z,f)==(size_t)z&&!fclose(f));*n=(size_t)z;return p;}
static void write_file(const char*dir,const char*name,const void*bytes,size_t n){char p[4096];NEED(snprintf(p,sizeof p,"%s/%s",dir,name)<(int)sizeof p);FILE*f=fopen(p,"wb");NEED(f);NEED(fwrite(bytes,1,n,f)==n&&!fclose(f));}
int main(int argc,char**argv){
  NEED(argc==8);uint16_t endian=1;NEED(*(unsigned char*)&endian==1);
#if defined(__aarch64__)
  uint64_t fpcr;__asm__ volatile("mrs %0, fpcr":"=r"(fpcr));NEED(fpcr==0);
#endif
  admission();endpoints();aliasing();size_t sizes[6];unsigned char*inputs[6];for(int i=0;i<6;i++)inputs[i]=read_file(argv[i+1],sizes+i);
  NEED(sizes[0]%24==0&&sizes[1]%32==0&&sizes[2]%32==0&&sizes[3]%32==0&&sizes[4]==sizes[3]/32&&sizes[5]==sizes[4]*8);
  size_t counts[]={sizes[0]/24,sizes[1]/32,sizes[2]/32,sizes[3]/32};const char*names[]={"density-actual.bin","uniform-actual.bin","seeded-actual.bin","ratio-actual.bin"};
  double*probes=calloc(counts[3]?counts[3]:1,8);NEED(probes);
  for(int k=0;k<4;k++){
    NEED(counts[k]>0&&counts[k]<=8192);size_t stride=WIDTH[k]+8;unsigned char*records=calloc(counts[k],stride);NEED(records);
    for(size_t i=0;i<counts[k];i++){
      double p[4]={0};coax_t_CrSeed state={0};if(k==2){memcpy(&state,inputs[k]+i*32,16);memcpy(p,inputs[k]+i*32+16,16);}else memcpy(p,inputs[k]+i*(k==0?24:32),INPUTS[k]*8);
      if(k==3){NEED(inputs[4][i]<=1);if(inputs[4][i]){double dlog;memcpy(&dlog,inputs[5]+i*8,8);double o[3]={1001,1002,1003};NEED(!rlr_exp_probe(dlog,(coax_v_f64){o+1,1}));NEED(isfinite(o[1])&&o[0]==1001&&o[2]==1003);probes[i]=o[1];probe_calls++;}}
      uint8_t status=checked(k,state,p,records+i*stride+8);int64_t wire=status;memcpy(records+i*stride,&wire,8);
      if(k==3&&status==0){coax_t_RlrOutput o;memcpy(&o,records+i*stride+8,sizeof o);NEED(inputs[4][i]==1&&word(o.ratio)==word(probes[i]));}
    }
    write_file(argv[7],names[k],records,counts[k]*stride);free(records);
  }
  write_file(argv[7],"ratio-exp.bin",probes,counts[3]*8);free(probes);for(int i=0;i<6;i++)free(inputs[i]);
  printf("{\"assertions\":%"PRIu64",\"refusal_queries\":%"PRIu64",\"alias_observations\":%"PRIu64",\"IEEE_patterns\":%"PRIu64",\"probe_calls\":%"PRIu64",\"density_rows\":%zu,\"uniform_rows\":%zu,\"seeded_rows\":%zu,\"ratio_rows\":%zu,\"fpcr_zero\":true}\n",assertions,refusals,aliases,ieee_patterns,probe_calls,counts[0],counts[1],counts[2],counts[3]);
  return 0;
}
