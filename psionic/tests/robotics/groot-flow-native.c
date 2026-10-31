/* Firm reason: 2 (independent oracle). Source-formula reference calculations
   for time embeddings and staged Euler actions; no learned policy is run. */
#include "groot-flow.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned cases;
static coax_v_f32 span(float *p,int64_t n){return(coax_v_f32){p,n};}
static uint32_t bits(float v){uint32_t b;memcpy(&b,&v,4);return b;}
static float value(uint32_t b){float v;memcpy(&v,&b,4);return v;}
static void sentinels(float *p,unsigned n){for(unsigned i=0;i<n;i++)p[i]=-713.25f;}
static void unchanged(const float *p,unsigned n){for(unsigned i=0;i<n;i++)assert(p[i]==-713.25f);}
static void result(coax_t_GfResult r,int status,int written){assert(r.status==status&&r.written==written);}
static void reference_time(int kind,int width,int bucket,float *out){
  int half=width/2;float logarithm=(float)log(10000.0),ratio=logarithm/(float)half;
  for(int i=0;i<half;i++){
    float exponent;
    if(kind==0)exponent=-(float)i*ratio;
    else {float product=-logarithm*(float)i;exponent=product/(float)(half-1);}
    float frequency=(float)exp((double)exponent),phase=(float)bucket*frequency;
    float sine=(float)sin((double)phase),cosine=(float)cos((double)phase);
    out[i]=kind?cosine:sine;out[half+i]=kind?sine:cosine;
  }
}
static int time_admission(unsigned kind,int64_t width,int64_t bucket,int64_t out,int64_t work,int alias){
  if(kind>1)return 2;
  if(width<2||width>4096||(width&1)||(kind==1&&width<4))return 3;
  if(bucket<0||bucket>999)return 1;
  if(out<width||work<width)return 4;
  return alias?5:0;
}
int main(int argc,char **argv){
  assert(argc==1||argc==2);float out[5281],work[5281],x[5280],v[5280],expected[4096];
  for(int step=-1;step<=4;step++){
    coax_t_GfSchedule s=gf_schedule(step);
    if(step<0||step>3)assert(s.status==1&&s.bucket==0&&s.dt==0);
    else assert(s.status==0&&s.bucket==step*250&&s.dt==0.25f);cases++;
  }
  const int64_t buckets[]={-1,0,1,249,250,500,750,999,1000,INT64_MIN,INT64_MAX};
  for(unsigned kind=0;kind<4;kind++)for(int64_t width=-2;width<=4098;width++)for(unsigned b=0;b<sizeof buckets/sizeof *buckets;b++){
    int expected_status=time_admission(kind,width,buckets[b],4096,4096,0);
    assert(gf_time_admit((uint8_t)kind,width,buckets[b],4096,4096,0)==expected_status);cases++;
  }
  const int widths[]={2,4,6,48,256,1536,4096};const int times[]={0,1,250,500,750,999};
  for(int kind=0;kind<2;kind++)for(unsigned n=0;n<sizeof widths/sizeof *widths;n++){
    int width=widths[n];if(kind&&width==2)continue;
    for(unsigned b=0;b<sizeof times/sizeof *times;b++){
      sentinels(out,5281);sentinels(work,5281);reference_time(kind,width,times[b],expected);
      result(gf_time((coax_t_GfTimeKind)kind,width,times[b],span(out,5281),span(work,5281),0),0,width);
      for(int i=0;i<width;i++){assert(bits(out[i])==bits(expected[i]));cases++;}
      unchanged(out+width,5281-width);unchanged(work+width,5281-width);
    }
  }
  for(int kind=0;kind<3;kind++)for(int which=0;which<5;which++){
    sentinels(out,5281);sentinels(work,5281);int width=which==0?3:4,bucket=which==1?1000:0;
    int outputs=which==2?3:4,scratch=which==3?3:4,alias=which==4;
    int status=time_admission(kind,width,bucket,outputs,scratch,alias);
    result(gf_time((coax_t_GfTimeKind)kind,width,bucket,span(out,outputs),span(work,scratch),alias),status,0);
    unchanged(out,5281);unchanged(work,5281);cases++;
  }
  for(int i=0;i<5280;i++){x[i]=(float)(i%37-18)/16.0f;v[i]=(float)(i%29-14)/8.0f;}
  for(int step=0;step<4;step++){
    sentinels(out,5281);sentinels(work,5281);
    result(gf_advance(step,span(x,5280),span(v,5280),span(out,5281),span(work,5281),0),0,5280);
    for(int i=0;i<5280;i++){float product=(float)((double)v[i]*0.25);float want=(float)((double)x[i]+(double)product);assert(bits(out[i])==bits(want));cases++;}
    unchanged(out+5280,1);unchanged(work+5280,1);
  }
  const uint32_t bad[]={0x7f800000,0xff800000,0x7fc00000,0x7f800001,0xff800001};
  for(unsigned b=0;b<sizeof bad/sizeof *bad;b++)for(int which=0;which<4;which++){
    float a[]={1,2},vel[]={1,2};if(which<2)a[which]=value(bad[b]);else vel[which-2]=value(bad[b]);
    sentinels(out,5281);sentinels(work,5281);result(gf_advance(0,span(a,2),span(vel,2),span(out,2),span(work,2),0),6,0);unchanged(out,5281);unchanged(work,5281);cases++;
  }
  for(int which=0;which<7;which++){
    sentinels(out,5281);sentinels(work,5281);int step=which==0?-1:0,n=which==1?0:2,nv=which==2?1:2,no=which==3?1:2,nw=which==4?1:2,alias=which==5;
    float a[]={1,FLT_MAX},vel[]={1,FLT_MAX};int status=which==0?1:which==1||which==2?3:which==3||which==4?4:which==5?5:7;
    result(gf_advance(step,span(a,n),span(vel,nv),span(out,no),span(work,nw),alias),status,0);unchanged(out,5281);cases++;
  }
  const uint32_t edge[]={0,0x80000000,1,0x80000001,0x00800003,0x80800003,0x3f800000,0xbf800000};
  for(unsigned a=0;a<sizeof edge/sizeof *edge;a++)for(unsigned b=0;b<sizeof edge/sizeof *edge;b++){
    float position=value(edge[a]),velocity=value(edge[b]),product=(float)((double)velocity*0.25),want=(float)((double)position+(double)product);
    result(gf_advance(3,span(&position,1),span(&velocity,1),span(out,1),span(work,1),0),0,1);assert(bits(out[0])==bits(want));cases++;
  }
  if(argc==2){
    FILE *file=fopen(argv[1],"wb");assert(file);const int stage[]={0,250,500,750};
    for(int kind=0;kind<2;kind++)for(int i=0;i<4;i++){
      int width=kind?256:1536;result(gf_time((coax_t_GfTimeKind)kind,width,stage[i],span(out,width),span(work,width),0),0,width);
      assert(fwrite(out,sizeof(float),(size_t)width,file)==(size_t)width);
    }assert(fclose(file)==0);
  }
  printf("GR00T flow/time native: %u independent formula, metadata, number, atomicity and ABI cases passed\n",cases);return 0;
}
