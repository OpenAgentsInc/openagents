/* New derived-quantity corpus only; independent volatile raw-C IEEE oracle. */
#include "derived-native.h"
#include <assert.h>
#include <fenv.h>
#include <float.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
_Static_assert(sizeof(float)==4&&sizeof(double)==8&&FLT_RADIX==2&&FLT_MANT_DIG==24&&DBL_MANT_DIG==53,"IEEE binary carriers");
struct RawNested{uint8_t prefix;float product;double speed;float ratio;uint16_t tail;};
_Static_assert(sizeof(coax_t_QD_Nested)==sizeof(struct RawNested),"derived nested size");
_Static_assert(_Alignof(coax_t_QD_Nested)==_Alignof(struct RawNested),"derived nested alignment");
#define OFFSET(field) _Static_assert(offsetof(coax_t_QD_Nested,field)==offsetof(struct RawNested,field),"derived nested " #field)
OFFSET(prefix);OFFSET(product);OFFSET(speed);OFFSET(ratio);OFFSET(tail);
_Static_assert(_Generic(QD_product32(0,0),float:1,default:0),"derived32 erases to float");
_Static_assert(_Generic(QD_product64(0,0),double:1,default:0),"derived64 erases to double");
static float(*const product32)(float,float)=QD_product32;
static float(*const quotient32)(float,float)=QD_quotient32;
static float(*const from32abi)(float)=QD_from_named32;
static double(*const product64)(double,double)=QD_product64;
static double(*const quotient64)(double,double)=QD_quotient64;
static double(*const from64abi)(double)=QD_from_named64;
static int trace[16],trace_count;
static float tick(int64_t id){assert(trace_count<16);trace[trace_count++]=(int)id;return(float)(id+1);}
float QD_metre_tick(int64_t id){return tick(id);}float QD_second_tick(int64_t id){return tick(id);}
float QD_half_tick(int64_t id){return tick(id);}float QD_named_tick(int64_t id){return tick(id);}
static uint32_t bits32(float x){uint32_t u;memcpy(&u,&x,4);return u;}
static uint64_t bits64(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static float value32(uint32_t u){float x;memcpy(&x,&u,4);return x;}
static double value64(uint64_t u){double x;memcpy(&x,&u,8);return x;}
static uint64_t verified;
static void get(FILE*f,void*p,size_t n){assert(fread(p,1,n,f)==n);}static void put(FILE*f,const void*p,size_t n){assert(fwrite(p,1,n,f)==n);}
static void check32(FILE*out,float got,float expected){assert((isnan(got)&&isnan(expected))||bits32(got)==bits32(expected));uint32_t x=isnan(got)?UINT32_C(0x7fc00000):bits32(got);put(out,&x,4);verified++;}
static void check64(FILE*out,double got,double expected){assert((isnan(got)&&isnan(expected))||bits64(got)==bits64(expected));uint64_t x=isnan(got)?UINT64_C(0x7ff8000000000000):bits64(got);put(out,&x,8);verified++;}
static void ordered(const int *ids,int n){assert(trace_count==n);for(int i=0;i<n;i++)assert(trace[i]==ids[i]);}
int main(int argc,char**argv){
 assert(argc==3&&fegetround()==FE_TONEAREST);
 #if defined(__aarch64__)
 uint64_t fpcr;__asm__ volatile("mrs %0, fpcr":"=r"(fpcr));assert(!(fpcr&(UINT64_C(1)<<24)));
 #else
 uint64_t fpcr=0;
 #endif
 FILE*in=fopen(argv[1],"rb"),*out=fopen(argv[2],"wb");assert(in&&out);uint32_t n32,n64;get(in,&n32,4);get(in,&n64,4);assert(n32<=5000&&n64<=5000);
 for(uint32_t i=0;i<n32;i++){
  uint32_t words[2];get(in,words,sizeof(words));float a=value32(words[0]),b=value32(words[1]);volatile float x=a,y=b,q=x/y;
  check32(out,product32(a,b),x*y);check32(out,quotient32(a,b),q);check32(out,QD_one32(a,b),q);
  check32(out,QD_to_named32(a,b),q);check32(out,from32abi(a),a);check32(out,QD_half_form32(a,b),q);
  check32(out,QD_half_one32(a,b),q*0.5f);check32(out,QD_cancel32(a,b),q*y);
 }
 for(uint32_t i=0;i<n64;i++){
  uint64_t words[2];get(in,words,sizeof(words));double a=value64(words[0]),b=value64(words[1]);volatile double x=a,y=b,q=x/y;
  check64(out,product64(a,b),x*y);check64(out,quotient64(a,b),q);check64(out,QD_one64(a,b),q);
  check64(out,QD_to_named64(a,b),q);check64(out,from64abi(a),a);check64(out,QD_half_form64(a,b),q);
  check64(out,QD_half_one64(a,b),q*0.5);check64(out,QD_cancel64(a,b),q*y);
 }
 assert(fgetc(in)==EOF);coax_t_QD_Nested a={7,-0.0f,3.25,-2.5f,513},r=QD_nested(a);assert(r.prefix==a.prefix&&r.tail==a.tail);
 check32(out,r.product,a.product);check64(out,r.speed,a.speed);check32(out,r.ratio,a.ratio);
 trace_count=0;check32(out,QD_ordered_product(),6.0f);ordered((int[]){1,2},2);
 volatile float four=4,five=5,six=6,seven=7,eight=8,nine=9,ten=10;
 trace_count=0;check32(out,QD_ordered_quotient(),four/five);ordered((int[]){3,4},2);
 volatile float quotient=six/seven;trace_count=0;check32(out,QD_ordered_cancel(),quotient*eight);ordered((int[]){5,6,7},3);
 quotient=nine/ten;trace_count=0;check32(out,QD_ordered_half(),quotient*0.5f);ordered((int[]){8,9},2);
 trace_count=0;check32(out,QD_ordered_from_named(),11.0f);ordered((int[]){10},1);
 trace_count=0;check32(out,QD_ordered_reverse_product(),156.0f);ordered((int[]){11,12},2);
 assert(!fclose(in)&&!fclose(out));printf("derived quantity native: %u f32, %u f64, %llu observations, FPCR=%llu\n",n32,n64,(unsigned long long)verified,(unsigned long long)fpcr);return 0;
}
