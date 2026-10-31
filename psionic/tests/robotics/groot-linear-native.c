/* Firm reason: 2 (independent numerical and ABI oracle). Synthetic asymmetric
   original-layout matrices, separately calculated transpose/dot/narrow/bias
   and ReLU. This does not load GR00T weights or implement a robot controller. */
#include "groot-linear.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static unsigned cases;
static coax_v_u8 bytes(uint8_t *p,int64_t n){return(coax_v_u8){p,n};}
static coax_v_f32 floats(float *p,int64_t n){return(coax_v_f32){p,n};}
static uint32_t bits(float x){uint32_t u;memcpy(&u,&x,4);return u;}
static float number(uint32_t u){float x;memcpy(&x,&u,4);return x;}
static int width(int dtype){return dtype==COAX_QmDtype_bf16?2:4;}
static void word(int dtype,uint8_t *raw,unsigned i,uint32_t u){
 unsigned n=width(dtype),p=i*n;if(n==2)u>>=16;for(unsigned b=0;b<n;b++)raw[p+b]=(uint8_t)(u>>(8*b));
}
static uint32_t read_word(int dtype,const uint8_t *raw,unsigned i){
 unsigned n=width(dtype),p=i*n;uint32_t u=0;for(unsigned b=0;b<n;b++)u|=(uint32_t)raw[p+b]<<(8*b);return n==2?u<<16:u;
}
static void encode(int dtype,uint8_t *raw,const float *values,unsigned n){for(unsigned i=0;i<n;i++)word(dtype,raw,i,bits(values[i]));}
static void sentinel(float *p,unsigned n){for(unsigned i=0;i<n;i++)p[i]=-713.25f;}
static void untouched(const float *p,unsigned n){for(unsigned i=0;i<n;i++)assert(p[i]==-713.25f);}
static void result(coax_t_QmResult r,int status,int64_t written){assert(r.status==status&&r.written==written);}

/* A Neumaier accumulator is independent of the product's TwoSum formulation.
   For these small exactly encoded matrices both specify the same rounded dot. */
static float dot(int dtype,const uint8_t *source,unsigned inputs,unsigned outputs,unsigned out,const float *x){
 double sum=0,correction=0;
 for(unsigned i=0;i<inputs;i++){
  double term=(double)number(read_word(dtype,source,i*outputs+out))*(double)x[i],next=sum+term;
  if(fabs(sum)>=fabs(term))correction+=(sum-next)+term;else correction+=(term-next)+sum;sum=next;
 }
 return(float)(sum+correction);
}
static void linear_ref(int dtype,const uint8_t *source,unsigned inputs,unsigned outputs,const float *bias,const float *x,float *out){
 for(unsigned o=0;o<outputs;o++){float narrowed=dot(dtype,source,inputs,outputs,o,x);out[o]=(float)((double)narrowed+(double)bias[o]);}
}
static void pack_ok(int dtype,unsigned inputs,unsigned outputs,uint8_t *raw,uint8_t *packed){
 unsigned n=inputs*outputs*width(dtype);uint8_t original[128];assert(n<=sizeof original);memcpy(original,raw,n);memset(packed,0xa5,n+3);
 result(gl_pack(dtype,inputs,outputs,bytes(raw,n),bytes(packed,n+3),0),COAX_QmStatus_ok,n);
 assert(!memcmp(raw,original,n));
 for(unsigned i=0;i<inputs;i++)for(unsigned o=0;o<outputs;o++)
  assert(read_word(dtype,packed,o*inputs+i)==read_word(dtype,raw,i*outputs+o));
 for(unsigned i=n;i<n+3;i++)assert(packed[i]==0xa5);cases++;
}

static void normal_cases(int dtype){
 uint8_t source[128],packed[131],raw_bias[64];
 const float w[12]={1,-2,.5f,3, -.25f,4,-1,2, 2,.75f,5,-3};
 const float b[4]={.25f,-.5f,1,-2};float x[3]={2,-1,.5f},bias[5],out[5],work[9],want[4];
 encode(dtype,source,w,12);pack_ok(dtype,3,4,source,packed);
 encode(dtype,raw_bias,b,4);sentinel(bias,5);
 result(gl_bias(dtype,4,bytes(raw_bias,4*width(dtype)),floats(bias,5),0),COAX_QmStatus_ok,4);
 for(unsigned i=0;i<4;i++)assert(bits(bias[i])==read_word(dtype,raw_bias,i));assert(bias[4]==-713.25f);cases++;
 float original_x[3],original_bias[4];uint8_t original_packed[128];memcpy(original_x,x,sizeof x);memcpy(original_bias,bias,sizeof original_bias);memcpy(original_packed,packed,12*width(dtype));
 linear_ref(dtype,source,3,4,bias,x,want);
 sentinel(out,5);sentinel(work,9);
 result(gl_linear(dtype,3,4,bytes(packed,12*width(dtype)),floats(bias,4),floats(x,3),floats(out,5),floats(work,9),0),COAX_QmStatus_ok,4);
 for(unsigned i=0;i<4;i++)assert(bits(out[i])==bits(want[i]));
 assert(!memcmp(original_x,x,sizeof x)&&!memcmp(original_bias,bias,sizeof original_bias)&&!memcmp(original_packed,packed,12*width(dtype)));
 assert(out[4]==-713.25f&&work[8]==-713.25f);cases++;

 /* Exact tie-to-even after the dot is observably before bias addition. */
 const float tie_w[2]={1,0x1p-24f},tie_x[2]={1,1};float tie_b[1]={-1};
 encode(dtype,source,tie_w,2);pack_ok(dtype,2,1,source,packed);sentinel(out,5);sentinel(work,9);
 result(gl_linear(dtype,2,1,bytes(packed,2*width(dtype)),floats(tie_b,1),floats((float*)tie_x,2),floats(out,5),floats(work,9),0),COAX_QmStatus_ok,1);
 assert(bits(out[0])==0&&out[0]!=(float)0x1p-24);untouched(out+1,4);cases++;

 /* Cancellation loses the unit term in a naive F64 accumulation. */
 const float cancel_w[3]={0x1p100f,1,-0x1p100f},cancel_x[3]={1,1,1};float cancel_b[1]={.25f};
 encode(dtype,source,cancel_w,3);pack_ok(dtype,3,1,source,packed);sentinel(out,5);
 result(gl_linear(dtype,3,1,bytes(packed,3*width(dtype)),floats(cancel_b,1),floats((float*)cancel_x,3),floats(out,5),floats(work,9),0),COAX_QmStatus_ok,1);
 assert(out[0]==1.25f&&dot(dtype,source,3,1,0,cancel_x)==1);untouched(out+1,4);cases++;

 /* Word movement and bias decoding preserve signed zero, not only equality. */
 const float zeros[6]={0,-0.0f,1,-0.0f,-2,0};encode(dtype,source,zeros,6);pack_ok(dtype,2,3,source,packed);
 encode(dtype,raw_bias,zeros,6);sentinel(out,5);
 result(gl_bias(dtype,3,bytes(raw_bias,3*width(dtype)),floats(out,5),0),COAX_QmStatus_ok,3);
 assert(bits(out[0])==0&&bits(out[1])==0x80000000&&bits(out[2])==0x3f800000);untouched(out+3,2);cases++;
}

static void refusals(int dtype){
 uint8_t raw[64],packed[67],raw_bias[16];float out[5],work[12],bias[3]={0,0,0},x[2]={1,1};
 const float values[6]={1,2,3,4,5,6};encode(dtype,raw,values,6);unsigned n=6*width(dtype);
 int64_t invalid[]={-1,0,32769,INT64_MAX};
 for(unsigned i=0;i<4;i++){
  memset(packed,0xa5,sizeof packed);sentinel(out,5);
  result(gl_pack(dtype,invalid[i],3,bytes(raw,n),bytes(packed,67),0),COAX_QmStatus_shape,0);
  result(gl_pack(dtype,2,invalid[i],bytes(raw,n),bytes(packed,67),0),COAX_QmStatus_shape,0);
  result(gl_bias(dtype,invalid[i],bytes(raw,n),floats(out,5),0),COAX_QmStatus_shape,0);
  result(gl_linear(dtype,invalid[i],3,bytes(raw,n),floats(bias,3),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_shape,0);
  result(gl_linear(dtype,2,invalid[i],bytes(raw,n),floats(bias,3),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_shape,0);
  result(gl_state(dtype,invalid[i],2,3,bytes(raw,n),floats(bias,2),bytes(raw,n),floats(bias,3),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_shape,0);
  result(gl_state(dtype,2,invalid[i],3,bytes(raw,n),floats(bias,2),bytes(raw,n),floats(bias,3),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_shape,0);
  result(gl_state(dtype,2,2,invalid[i],bytes(raw,n),floats(bias,2),bytes(raw,n),floats(bias,3),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_shape,0);
  for(unsigned j=0;j<67;j++)assert(packed[j]==0xa5);untouched(out,5);cases++;
 }
 result(gl_pack(dtype,32768,32768,bytes(raw,0),bytes(packed,0),0),COAX_QmStatus_capacity,0);cases++;
 result(gl_linear(dtype,32768,32768,bytes(raw,0),floats(bias,0),floats(x,0),floats(out,0),floats(work,0),0),COAX_QmStatus_capacity,0);cases++;
 result(gl_state(dtype,32768,32768,32768,bytes(raw,0),floats(bias,0),bytes(raw,0),floats(bias,0),floats(x,0),floats(out,0),floats(work,0),0),COAX_QmStatus_capacity,0);cases++;
 for(int mode=0;mode<3;mode++){
  memset(packed,0xa5,sizeof packed);
  result(gl_pack(dtype,2,3,bytes(raw,mode==0?n-1:n),bytes(packed,mode==1?n-1:67),mode==2),mode==2?COAX_QmStatus_alias:COAX_QmStatus_capacity,0);
  for(unsigned j=0;j<67;j++)assert(packed[j]==0xa5);cases++;
 }
 uint8_t saved_raw[64];memcpy(saved_raw,raw,sizeof raw);
 result(gl_pack(dtype,2,3,bytes(raw,n),bytes(raw,n),1),COAX_QmStatus_alias,0);assert(!memcmp(saved_raw,raw,sizeof raw));cases++;
 encode(dtype,raw_bias,values,3);
 for(int mode=0;mode<3;mode++){
  sentinel(out,5);result(gl_bias(dtype,3,bytes(raw_bias,mode==0?3*width(dtype)-1:3*width(dtype)),floats(out,mode==1?2:5),mode==2),mode==2?COAX_QmStatus_alias:COAX_QmStatus_capacity,0);untouched(out,5);cases++;
 }
 const uint32_t bad[]={0x7f800000,0xff800000,0x7fc10000};
 for(unsigned b=0;b<3;b++)for(unsigned position=0;position<6;position++){
  encode(dtype,raw,values,6);word(dtype,raw,position,bad[b]);memset(packed,0xa5,sizeof packed);
  result(gl_pack(dtype,2,3,bytes(raw,n),bytes(packed,67),0),COAX_QmStatus_number,0);
  for(unsigned j=0;j<67;j++)assert(packed[j]==0xa5);cases++;
 }
 for(unsigned b=0;b<3;b++)for(unsigned position=0;position<3;position++){
  encode(dtype,raw_bias,values,3);word(dtype,raw_bias,position,bad[b]);sentinel(out,5);
  result(gl_bias(dtype,3,bytes(raw_bias,3*width(dtype)),floats(out,5),0),COAX_QmStatus_number,0);untouched(out,5);cases++;
 }
 encode(dtype,raw,values,6);pack_ok(dtype,2,3,raw,packed);
 for(int mode=0;mode<7;mode++){
  sentinel(out,5);sentinel(work,12);
  result(gl_linear(dtype,2,3,bytes(packed,mode==0?n-1:n),floats(bias,mode==1?2:3),floats(x,mode==2?1:2),floats(out,mode==3?2:5),floats(work,mode==4?5:12),mode==5),mode==5?COAX_QmStatus_alias:mode==6?COAX_QmStatus_ok:COAX_QmStatus_capacity,mode==6?3:0);
  if(mode!=6)untouched(out,5);cases++;
 }
 for(unsigned position=0;position<3;position++){
  bias[position]=INFINITY;sentinel(out,5);
  result(gl_linear(dtype,2,3,bytes(packed,n),floats(bias,3),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_number,0);untouched(out,5);bias[position]=0;cases++;
 }
 for(unsigned position=0;position<6;position++){
  uint8_t saved[67];memcpy(saved,packed,sizeof saved);word(dtype,packed,position,0x7fc10000);sentinel(out,5);
  result(gl_linear(dtype,2,3,bytes(packed,n),floats(bias,3),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_number,0);untouched(out,5);memcpy(packed,saved,sizeof saved);cases++;
 }
 for(unsigned position=0;position<2;position++){
  x[position]=NAN;sentinel(out,5);
  result(gl_linear(dtype,2,3,bytes(packed,n),floats(bias,3),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_number,0);untouched(out,5);x[position]=1;cases++;
 }
 /* Projection overflow must refuse even if a subsequent bias could cancel. */
 const float huge[2]={FLT_MAX,FLT_MAX};encode(dtype,raw,huge,2);pack_ok(dtype,2,1,raw,packed);bias[0]=-FLT_MAX;sentinel(out,5);
 result(gl_linear(dtype,2,1,bytes(packed,2*width(dtype)),floats(bias,1),floats(x,2),floats(out,5),floats(work,12),0),COAX_QmStatus_arithmetic,0);untouched(out,5);cases++;
 const float unit[1]={1};encode(dtype,raw,unit,1);pack_ok(dtype,1,1,raw,packed);x[0]=FLT_MAX;bias[0]=FLT_MAX;sentinel(out,5);
 result(gl_linear(dtype,1,1,bytes(packed,width(dtype)),floats(bias,1),floats(x,1),floats(out,5),floats(work,12),0),COAX_QmStatus_arithmetic,0);untouched(out,5);cases++;
}

static void state_cases(int dtype){
 const float first[6]={1,-2,-1,.5f,2,1},second[6]={1,2,-1,-3,.5f,4},x[3]={1,-2,-1};
 float b1[2]={-.5f,.25f},b2[3]={1,-2,.5f},out[4],work[16],hidden[2],want[3];uint8_t s1[64],s2[64],p1[67],p2[67];
 encode(dtype,s1,first,6);encode(dtype,s2,second,6);pack_ok(dtype,3,2,s1,p1);pack_ok(dtype,2,3,s2,p2);
 linear_ref(dtype,s1,3,2,b1,x,hidden);for(unsigned i=0;i<2;i++)if(hidden[i]<0)hidden[i]=0;linear_ref(dtype,s2,2,3,b2,hidden,want);
 sentinel(out,4);sentinel(work,16);
 result(gl_state(dtype,3,2,3,bytes(p1,6*width(dtype)),floats(b1,2),bytes(p2,6*width(dtype)),floats(b2,3),floats((float*)x,3),floats(out,4),floats(work,16),0),COAX_QmStatus_ok,3);
 for(unsigned i=0;i<3;i++)assert(bits(out[i])==bits(want[i]));assert(out[3]==-713.25f&&work[15]==-713.25f);cases++;
 for(int mode=0;mode<8;mode++){
  sentinel(out,4);result(gl_state(dtype,3,2,3,bytes(p1,mode==0?6*width(dtype)-1:6*width(dtype)),floats(b1,mode==1?1:2),bytes(p2,mode==2?6*width(dtype)-1:6*width(dtype)),floats(b2,mode==3?2:3),floats((float*)x,mode==4?2:3),floats(out,mode==5?2:4),floats(work,mode==6?14:16),mode==7),mode==7?COAX_QmStatus_alias:COAX_QmStatus_capacity,0);untouched(out,4);cases++;
 }
 for(unsigned layer=0;layer<2;layer++)for(unsigned where=0;where<6;where++){
  uint8_t saved[64],*selected=layer==0?p1:p2;memcpy(saved,selected,6*width(dtype));word(dtype,selected,where,0x7fc10000);sentinel(out,4);
  result(gl_state(dtype,3,2,3,bytes(p1,6*width(dtype)),floats(b1,2),bytes(p2,6*width(dtype)),floats(b2,3),floats((float*)x,3),floats(out,4),floats(work,16),0),COAX_QmStatus_number,0);untouched(out,4);cases++;
  memcpy(selected,saved,6*width(dtype));
 }
 /* The second layer can fail after a fully computed first layer. */
 const float identity[4]={1,0,0,1},last[4]={1,FLT_MAX,1,FLT_MAX},ones[2]={1,1};float zeros[2]={0,0};
 encode(dtype,s1,identity,4);encode(dtype,s2,last,4);pack_ok(dtype,2,2,s1,p1);pack_ok(dtype,2,2,s2,p2);sentinel(out,4);
 result(gl_state(dtype,2,2,2,bytes(p1,4*width(dtype)),floats(zeros,2),bytes(p2,4*width(dtype)),floats(zeros,2),floats((float*)ones,2),floats(out,4),floats(work,16),0),COAX_QmStatus_arithmetic,0);untouched(out,4);cases++;
 const float last_bias_fail[4]={1,0,1,FLT_MAX};float overflow_bias[2]={0,FLT_MAX};
 encode(dtype,s2,last_bias_fail,4);pack_ok(dtype,2,2,s2,p2);sentinel(out,4);
 result(gl_state(dtype,2,2,2,bytes(p1,4*width(dtype)),floats(zeros,2),bytes(p2,4*width(dtype)),floats(overflow_bias,2),floats((float*)ones,2),floats(out,4),floats(work,16),0),COAX_QmStatus_arithmetic,0);untouched(out,4);cases++;
}

static void metadata_cases(void){
 const int64_t shapes[][2]={{1,1},{2,3},{3,2},{1,32768},{32768,1},{32768,32768},{1024,1536}};
 for(unsigned i=0;i<sizeof shapes/sizeof shapes[0];i++){
  int64_t h=shapes[i][0],o=shapes[i][1];coax_t_GlLayout l=gl_state_layout(h,o);
  assert(l.status==COAX_QmStatus_ok&&l.first==h&&l.stage==3*h&&l.second==3*h+o&&l.total==3*(h+o));cases++;
 }
 const int64_t bad[]={-1,0,32769,INT64_MAX};
 for(unsigned i=0;i<4;i++){
  assert(gl_state_layout(bad[i],1).status==COAX_QmStatus_shape&&gl_state_layout(1,bad[i]).status==COAX_QmStatus_shape);cases++;
 }
 for(int dtype=0;dtype<2;dtype++){
  const int64_t in=132,h=1024,o=1536,base[]={132*1024*width(dtype),1024,1024*1536*width(dtype),1536,132,1536,3*(1024+1536)};
  assert(gl_state_admit(dtype,in,h,o,base[0],base[1],base[2],base[3],base[4],base[5],base[6],0)==COAX_QmStatus_ok);cases++;
  assert(gl_state_admit(dtype,in,h,o,base[0],base[1],base[2],base[3],base[4],base[5],base[6],1)==COAX_QmStatus_alias);cases++;
  for(unsigned field=0;field<7;field++)for(unsigned mode=0;mode<3;mode++){
   int64_t count[7];memcpy(count,base,sizeof count);count[field]=mode==0?count[field]-1:mode==1?count[field]+1:-1;
   int expected=mode==1&&field>=5?COAX_QmStatus_ok:COAX_QmStatus_capacity;
   assert(gl_state_admit(dtype,in,h,o,count[0],count[1],count[2],count[3],count[4],count[5],count[6],0)==expected);cases++;
  }
  assert(gl_state_admit(dtype,in,h,o,base[0]-1,base[1],base[2],base[3],base[4],base[5],base[6],1)==COAX_QmStatus_capacity);cases++;
 }
 assert(gl_state_admit(2,132,1024,1536,0,0,0,0,0,0,0,1)==COAX_QmStatus_shape);cases++;
}

int main(void){
 metadata_cases();
 for(int dtype=0;dtype<2;dtype++){normal_cases(dtype);refusals(dtype);state_cases(dtype);}
 uint8_t raw[8]={0},packed[8]={0xa5};float value[1]={0},out[1]={-713.25f},work[8]={0};
 result(gl_pack(2,1,1,bytes(raw,4),bytes(packed,8),0),COAX_QmStatus_shape,0);
 result(gl_bias(2,1,bytes(raw,4),floats(out,1),0),COAX_QmStatus_shape,0);
 result(gl_linear(2,1,1,bytes(raw,4),floats(value,1),floats(value,1),floats(out,1),floats(work,8),0),COAX_QmStatus_shape,0);untouched(out,1);cases++;
 printf("GR00T linear native: %u independent transpose/BF16/F32/narrow/bias/ReLU/refusal cases passed\n",cases);return 0;
}
