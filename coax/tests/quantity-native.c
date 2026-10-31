/* Independent raw-C IEEE oracle and transparent quantity ABI checks. */
#include "quantity-native.h"
#include <assert.h>
#include <fenv.h>
#include <float.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

_Static_assert(sizeof(float)==4&&sizeof(double)==8,"fixed float carrier widths");
_Static_assert(FLT_RADIX==2&&FLT_MANT_DIG==24&&DBL_MANT_DIG==53,"binary IEEE carrier precision");
struct RawNested {uint8_t prefix;float distance;double elapsed;uint16_t tail;};
struct RawArrays {float distances[3];double durations[2];};
_Static_assert(sizeof(coax_t_QN_Nested)==sizeof(struct RawNested),"nested carrier size");
_Static_assert(_Alignof(coax_t_QN_Nested)==_Alignof(struct RawNested),"nested carrier alignment");
_Static_assert(offsetof(coax_t_QN_Nested,prefix)==offsetof(struct RawNested,prefix),"prefix offset");
_Static_assert(offsetof(coax_t_QN_Nested,distance)==offsetof(struct RawNested,distance),"quantity f32 offset");
_Static_assert(offsetof(coax_t_QN_Nested,elapsed)==offsetof(struct RawNested,elapsed),"quantity f64 offset");
_Static_assert(offsetof(coax_t_QN_Nested,tail)==offsetof(struct RawNested,tail),"tail offset");
_Static_assert(sizeof(coax_t_QN_Arrays)==sizeof(struct RawArrays),"nested arrays size");
_Static_assert(_Alignof(coax_t_QN_Arrays)==_Alignof(struct RawArrays),"nested arrays alignment");
_Static_assert(offsetof(coax_t_QN_Arrays,distances)==offsetof(struct RawArrays,distances),"array f32 offset");
_Static_assert(offsetof(coax_t_QN_Arrays,durations)==offsetof(struct RawArrays,durations),"array f64 offset");
_Static_assert(_Generic(QN_add32(0.0f,0.0f),float:1,default:0),"export f32 result is raw float");
_Static_assert(_Generic(QN_add64(0.0,0.0),double:1,default:0),"export f64 result is raw double");

static float (*const abi32)(float,float)=QN_add32;
static double (*const abi64)(double,double)=QN_add64;
static float (*const array_abi)(coax_a3_f32)=QN_array_sum;
static float (*const view_abi)(coax_v_f32)=QN_view_sum;
static int trace[16],trace_count;
float QN_tick(int64_t id){assert(trace_count<16);trace[trace_count++]=(int)id;return (float)(id+1);}
float QN_scalar_tick(int64_t id){assert(trace_count<16);trace[trace_count++]=(int)id;return (float)(id+1);}
float QN_echo(float value){return value;}
static uint32_t bits32(float x){uint32_t u;memcpy(&u,&x,4);return u;}
static uint64_t bits64(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static float from32(uint32_t u){float x;memcpy(&x,&u,4);return x;}
static double from64(uint64_t u){double x;memcpy(&x,&u,8);return x;}
static uint64_t verified;
static void get(FILE *f,void *p,size_t n){assert(fread(p,1,n,f)==n);}
static void put(FILE *f,const void *p,size_t n){assert(fwrite(p,1,n,f)==n);}
static void check32(FILE *out,float got,float expected){
 assert((isnan(got)&&isnan(expected))||bits32(got)==bits32(expected));
 uint32_t canonical=isnan(got)?UINT32_C(0x7fc00000):bits32(got);put(out,&canonical,4);verified++;
}
static void check64(FILE *out,double got,double expected){
 assert((isnan(got)&&isnan(expected))||bits64(got)==bits64(expected));
 uint64_t canonical=isnan(got)?UINT64_C(0x7ff8000000000000):bits64(got);put(out,&canonical,8);verified++;
}
static uint32_t comparisons32(float a,float b){return (a==b?1u:0u)|(a!=b?2u:0u)|(a<b?4u:0u)|(a<=b?8u:0u)|(a>b?16u:0u)|(a>=b?32u:0u);}
static uint32_t comparisons64(double a,double b){return (a==b?1u:0u)|(a!=b?2u:0u)|(a<b?4u:0u)|(a<=b?8u:0u)|(a>b?16u:0u)|(a>=b?32u:0u);}
int main(int argc,char **argv){
 assert(argc==3&&fegetround()==FE_TONEAREST);FILE *in=fopen(argv[1],"rb"),*out=fopen(argv[2],"wb");assert(in&&out);
 uint32_t n32,n64;get(in,&n32,4);get(in,&n64,4);assert(n32<=5000&&n64<=5000);
 for(uint32_t i=0;i<n32;i++){
  uint32_t u[3];get(in,u,sizeof(u));float a=from32(u[0]),b=from32(u[1]),c=from32(u[2]);volatile float ra=a,rb=b,rc=c;
  check32(out,abi32(a,b),ra+rb);check32(out,QN_sub32(a,b),ra-rb);
  check32(out,QN_mul32(a,c),ra*rc);check32(out,QN_left32(c,a),rc*ra);check32(out,QN_div32(a,c),ra/rc);
  check32(out,QN_half_to_m32(a),ldexpf(ra,-1));check32(out,QN_m_to_half32(a),ldexpf(ra,1));
  check32(out,QN_small_to_big32(a),ldexpf(ra,-40));check32(out,QN_big_to_small32(a),ldexpf(ra,40));
  check32(out,QN_project32(a),a);check32(out,QN_make32(a),a);check32(out,QN_foreign32(a),a);
  volatile float sum=ra+rb,product=sum*rc;check32(out,QN_pure32(a,b,c),product-rb);
  uint32_t mask=QN_compare32(a,b);assert(mask==comparisons32(a,b));put(out,&mask,4);verified++;
  coax_a3_f32 ar={{a,b,c}};volatile float ab=ra+rb;check32(out,array_abi(ar),ab+rc);
  float raw[3]={a,b,c};volatile float vs=0.0f;for(int j=0;j<3;j++)vs=vs+raw[j];check32(out,view_abi((coax_v_f32){raw,3}),vs);
 }
 for(uint32_t i=0;i<n64;i++){
  uint64_t u[3];get(in,u,sizeof(u));double a=from64(u[0]),b=from64(u[1]),c=from64(u[2]);volatile double ra=a,rb=b,rc=c;
  check64(out,abi64(a,b),ra+rb);check64(out,QN_sub64(a,b),ra-rb);
  check64(out,QN_mul64(a,c),ra*rc);check64(out,QN_left64(c,a),rc*ra);check64(out,QN_div64(a,c),ra/rc);
  check64(out,QN_half_to_m64(a),ldexp(ra,-1));check64(out,QN_m_to_half64(a),ldexp(ra,1));
  check64(out,QN_small_to_big64(a),ldexp(ra,-40));check64(out,QN_big_to_small64(a),ldexp(ra,40));
  check64(out,QN_project64(a),a);check64(out,QN_make64(a),a);
  volatile double sum=ra+rb,product=sum*rc;check64(out,QN_pure64(a,b,c),product-rb);
  uint32_t mask=QN_compare64(a,b);assert(mask==comparisons64(a,b));put(out,&mask,4);verified++;
 }
 assert(fgetc(in)==EOF);coax_t_QN_Nested v={7,-0.0f,3.25,513};coax_t_QN_Nested r=QN_nested(v);
 assert(r.prefix==v.prefix&&r.tail==v.tail);check32(out,r.distance,v.distance);check64(out,r.elapsed,v.elapsed);
 coax_t_QN_Arrays arrays={{{1.0f,-2.0f,0.5f}},{{3.0,-4.0}}},copy=QN_arrays(arrays);
 for(int i=0;i<3;i++)check32(out,copy.distances.e[i],arrays.distances.e[i]);for(int i=0;i<2;i++)check64(out,copy.durations.e[i],arrays.durations.e[i]);
 check32(out,view_abi((coax_v_f32){NULL,0}),0.0f);
 trace_count=0;check32(out,QN_ordered(),28.0f);assert(trace_count==3&&trace[0]==1&&trace[1]==2&&trace[2]==3);
 trace_count=0;check32(out,QN_ordered_left(),10.0f);assert(trace_count==3&&trace[0]==1&&trace[1]==2&&trace[2]==3);
 trace_count=0;check32(out,QN_convert_once(),10.0f);assert(trace_count==1&&trace[0]==4);
 assert(!fclose(in)&&!fclose(out));printf("quantity native: %u f32 cases, %u f64 cases, %llu comparisons, transparent scalar/nested/array/view ABI and ordered single evaluation\n",n32,n64,(unsigned long long)verified);
 return 0;
}
