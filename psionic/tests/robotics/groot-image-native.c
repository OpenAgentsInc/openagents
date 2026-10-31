/* Firm reason2: independent original floating coefficient formula and scalar
   integer resize/normalization oracle. No OpenCV runtime dependency. */
#include "groot-image.h"
#include <assert.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct {int lo,hi,a,b;} Axis;
static unsigned cases;
static coax_v_u8 bytes(uint8_t *p,int64_t n){return(coax_v_u8){p,n};}
static coax_v_f32 floats(float *p,int64_t n){return(coax_v_f32){p,n};}
static uint32_t bits(float v){uint32_t b;memcpy(&b,&v,4);return b;}
static Axis reference_axis(int d){
  double inv=256.0/243.0,scale=1.0/inv;int lo=(int)floor(d*scale);
  float frac=(float)((d+1)-(lo+1)*inv);if(frac<=0)frac=0;else frac-=(float)floor(frac);
  int a=(int)lrintf((1.0f-frac)*2048.0f),b=(int)lrintf(frac*2048.0f),hi=lo+1;
  if(hi>242)hi=242;return(Axis){lo,hi,a,b};
}
static uint8_t sample(const uint8_t *raw,int y,int x,int c){return raw[((255-(6+y))*256+255-(6+x))*3+c];}
static float reference(const uint8_t *raw,Axis y,Axis x,int c){
  int top=sample(raw,y.lo,x.lo,c)*x.a+sample(raw,y.lo,x.hi,c)*x.b;
  int bottom=sample(raw,y.hi,x.lo,c)*x.a+sample(raw,y.hi,x.hi,c)*x.b;
  int sum=(y.a*(top/16))/65536+(y.b*(bottom/16))/65536;
  int value=(sum+2)/4;float centered=(float)value-127.5f;return centered/127.5f;
}
static void pattern(uint8_t *raw,int kind){
  uint32_t state=kind==4?113:114;
  for(int i=0;i<196608;i++)switch(kind){
    case 0:raw[i]=(uint8_t)((i*17+(i/768)*37)%256);break;
    case 1:raw[i]=0;break;case 2:raw[i]=255;break;
    case 3:raw[i]=(uint8_t)(((i/3+i/768)%2)*255);break;
    case 4:case 5:state=1664525u*state+1013904223u;raw[i]=(uint8_t)(state>>24);break;
    case 6:raw[i]=(uint8_t)((i/3)%256);break;default:raw[i]=(uint8_t)((i/768)%256);break;
  }
}
static void sentinel(float *out,float *work){for(int i=0;i<196609;i++){out[i]=7;work[i]=9;}}
static void untouched(float *out){for(int i=0;i<196609;i++)assert(out[i]==7);}
int main(void){
  uint8_t *raw=malloc(196609);float *out=malloc(196609*sizeof(float)),*work=malloc(196609*sizeof(float));assert(raw&&out&&work);
  Axis axes[256];for(int i=0;i<256;i++)axes[i]=reference_axis(i);
  for(int kind=0;kind<8;kind++){
    pattern(raw,kind);sentinel(out,work);coax_t_GiRead r=gi_preprocess(0,256,bytes(raw,196608),floats(out,196608),floats(work,196608),0);
    assert(r.status==0&&r.written==196608&&out[196608]==7&&work[196608]==9);
    for(int c=0;c<3;c++)for(int y=0;y<256;y++)for(int x=0;x<256;x++){assert(bits(out[c*65536+y*256+x])==bits(reference(raw,axes[y],axes[x],c)));cases++;}
    const int firsts[]={0,1,128,255},rows[]={1,17,64,1};
    for(int slice=0;slice<4;slice++){
      int first=firsts[slice],n=rows[slice],need=3*n*256;sentinel(out,work);
      r=gi_preprocess(first,n,bytes(raw,196608),floats(out,need),floats(work,need),0);assert(r.status==0&&r.written==need);
      for(int c=0;c<3;c++)for(int y=0;y<n;y++)for(int x=0;x<256;x++){assert(bits(out[c*n*256+y*256+x])==bits(reference(raw,axes[first+y],axes[x],c)));cases++;}
      for(int i=need;i<196609;i++){assert(out[i]==7&&work[i]==9);}
    }
  }
  const int64_t starts[]={INT64_MIN,-1,0,1,255,256,INT64_MAX},counts[]={INT64_MIN,-1,0,1,255,256,257,INT64_MAX};
  for(unsigned a=0;a<sizeof starts/sizeof *starts;a++)for(unsigned b=0;b<sizeof counts/sizeof *counts;b++){
    int64_t first=starts[a],n=counts[b];int valid=first>=0&&n>=1&&n<=256&&first<=256-n;if(valid)continue;
    sentinel(out,work);coax_t_GiRead r=gi_preprocess(first,n,bytes(raw,196608),floats(out,196609),floats(work,196609),0);assert(r.status==1&&r.written==0);untouched(out);cases++;
  }
  for(int which=0;which<8;which++){
    sentinel(out,work);int64_t input=which==0?-1:which==1?0:which==2?196607:which==3?196609:196608;
    int64_t capacity=which==4?-1:which==5?196607:196608,scratch=which==6?196607:196608;
    coax_t_GiRead r=gi_preprocess(0,256,bytes(raw,input),floats(out,capacity),floats(work,scratch),which==7);assert(r.status==(which==7?2:1)&&r.written==0);untouched(out);cases++;
  }
  free(raw);free(out);free(work);printf("GR00T image: %u independent native cases passed\n",cases);return 0;
}
