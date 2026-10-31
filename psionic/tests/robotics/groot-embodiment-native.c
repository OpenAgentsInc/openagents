/* Firm reason: 2 (independent oracle). Admission, atomic writes and scalar
   source-statistic arithmetic; this does not run the learned model. */
#include "groot-embodiment.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static unsigned cases;
static const double sl[]={-.38999009132385254,-.28383004665374756,.4479505717754364,1.8810230493545532,-2.8866775035858154,-1.159900426864624,.0020664597395807505,-.0400138795375824};
static const double sh[]={.15302616357803345,.32915401458740234,1.2546923160552979,3.3035426139831543,2.7496531009674072,.6893712878227234,.04004855826497078,-.0017598443664610386};
static const double al[]={-.6348214149475098,-.7741071581840515,-.7633928656578064,-.09749999642372131,-.14820000529289246,-.2742857038974762,0};
static const double ah[]={.7714285850524902,.8464285731315613,.9375,.13928571343421936,.15964286029338837,.3246428668498993,1};
static coax_v_f32 span(float *p,int64_t n){return(coax_v_f32){p,n};}
static uint32_t bits(float v){uint32_t b;memcpy(&b,&v,4);return b;}
static float value(uint32_t b){float v;memcpy(&v,&b,4);return v;}
static void fill(float *p,int n,float v){for(int i=0;i<n;i++)p[i]=v;}
static void untouched(float *p,int n){for(int i=0;i<n;i++)assert(bits(p[i])==bits(-713.25f));}
static void result(coax_t_GeRead r,int status,int written,int clipped){assert(r.status==status&&r.written==written&&r.clipped==clipped);cases++;}
static float norm(int i,float x){
  if((double)x<sl[i])return -1;if((double)x>sh[i])return 1;
  float ratio=(float)(((double)x-sl[i])/(sh[i]-sl[i]));
  float twice=2.0f*ratio;return twice-1.0f;
}
static float action(int i,float x){
  if(x< -1)x=-1;if(x>1)x=1;
  float shifted=x+1.0f,unit=shifted/2.0f;
  double scaled=(double)unit*(ah[i]-al[i]);float y=(float)(scaled+al[i]);
  if(i==6){float twice=2.0f*y,center=twice-1.0f;return center>0?-1.0f:center<0?1.0f:value(0x80000000);}
  return y;
}
int main(void){
  float state[8]={0},model[5280]={0},out[137];
  const uint32_t bad[]={0x7f800000,0xff800000,0x7fc00000,0x7f800001,0xff800001,0xffc00001};
  for(unsigned b=0;b<sizeof bad/sizeof *bad;b++)for(int i=0;i<8;i++){
    fill(state,8,0);state[i]=value(bad[b]);fill(out,137,-713.25f);
    result(ge_normalize(span(state,8),span(out,137),0),2,0,0);untouched(out,137);
  }
  fill(state,8,0);
  for(unsigned b=0;b<sizeof bad/sizeof *bad;b++)for(int row=0;row<16;row++)for(int axis=0;axis<7;axis++){
    fill(model,5280,0);model[row*132+axis]=value(bad[b]);fill(out,137,-713.25f);
    result(ge_decode(span(model,5280),span(out,137),0),2,0,0);untouched(out,137);
  }
  for(int n=-1;n<=10;n++)for(int capacity=-1;capacity<=137;capacity++)for(int alias=0;alias<2;alias++){
    fill(out,137,-713.25f);int status=n!=8||capacity<132?1:alias?3:0;
    int clipped=4; /* zero is outside z,axis-angle-x and both finger bounds */
    result(ge_normalize(span(state,n),span(out,capacity),alias),status,status?0:132,status?0:clipped);
    if(status)untouched(out,137);else untouched(out+132,5);
  }
  const int64_t sizes[]={-1,0,5279,5280,5281,INT64_MAX};
  fill(model,5280,0);
  for(unsigned n=0;n<sizeof sizes/sizeof *sizes;n++)for(int capacity=110;capacity<=114;capacity++)for(int alias=0;alias<2;alias++){
    fill(out,137,-713.25f);int status=sizes[n]!=5280||capacity<112?1:alias?3:0;
    result(ge_decode(span(model,sizes[n]),span(out,capacity),alias),status,status?0:112,0);
    if(status)untouched(out,137);else untouched(out+112,25);
  }
  /* Every unconsumed coordinate may be NaN, including rows16..39. */
  fill(model,5280,value(0x7fc00001));
  for(int row=0;row<16;row++)for(int axis=0;axis<7;axis++)model[row*132+axis]=0;
  fill(out,137,-713.25f);result(ge_decode(span(model,5280),span(out,137),0),0,112,0);
  for(int row=0;row<16;row++)for(int axis=0;axis<7;axis++){assert(bits(out[row*7+axis])==bits(action(axis,0)));cases++;}untouched(out+112,25);
  for(int sample=0;sample<257;sample++){
    int clipped=0;
    for(int i=0;i<8;i++){
      state[i]=(float)(sl[i]+(sh[i]-sl[i])*((double)((sample*37+i*17)%281)-12.0)/256.0);
      clipped+=(double)state[i]<sl[i]||(double)state[i]>sh[i];
    }
    fill(out,137,-713.25f);result(ge_normalize(span(state,8),span(out,137),0),0,132,clipped);
    for(int i=0;i<132;i++){assert(bits(out[i])==bits(i<8?norm(i,state[i]):0));cases++;}untouched(out+132,5);
    clipped=0;for(int row=0;row<16;row++)for(int axis=0;axis<7;axis++){
      float x=(float)((sample*37+row*17+axis*5)%521-260)/128.0f;
      model[row*132+axis]=x;clipped+=x< -1||x>1;
    }
    fill(out,137,-713.25f);result(ge_decode(span(model,5280),span(out,137),0),0,112,clipped);
    for(int row=0;row<16;row++)for(int axis=0;axis<7;axis++){assert(bits(out[row*7+axis])==bits(action(axis,model[row*132+axis])));cases++;}untouched(out+112,25);
  }
  /* One-ULP neighbors of every original quantile, and every finite extreme. */
  for(int axis=0;axis<8;axis++)for(int edge=0;edge<2;edge++)for(int direction=-1;direction<=1;direction++){
    fill(state,8,0);float bound=(float)(edge?sh[axis]:sl[axis]);state[axis]=direction?nextafterf(bound,direction>0?INFINITY:-INFINITY):bound;
    fill(out,137,-713.25f);coax_t_GeRead r=ge_normalize(span(state,8),span(out,137),0);assert(r.status==0);assert(bits(out[axis])==bits(norm(axis,state[axis])));untouched(out+132,5);cases++;
  }
  for(int sign=0;sign<2;sign++){
    fill(state,8,sign?-FLT_MAX:FLT_MAX);fill(out,137,-713.25f);result(ge_normalize(span(state,8),span(out,137),0),0,132,8);
    for(int i=0;i<8;i++)assert(out[i]==(sign?-1:1));untouched(out+132,5);
  }
  printf("GR00T LIBERO embodiment: %u independent native cases passed\n",cases);return 0;
}
