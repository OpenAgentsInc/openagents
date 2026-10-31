/* Independent direct F64 reference for the changed floating-origin boundary. */
#include "spacecam-native.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
_Static_assert(sizeof(double)==8 && DBL_MANT_DIG==53 && FLT_RADIX==2,"F64 profile");
_Static_assert(sizeof(coax_t_D3)==24 && _Alignof(coax_t_D3)==8,"unchanged D3 layout");
_Static_assert(offsetof(coax_t_D3,x)==0 && offsetof(coax_t_D3,y)==8 && offsetof(coax_t_D3,z)==16,"unchanged D3 fields");
_Static_assert(sizeof(coax_t_SpaceCam)==80,"unchanged camera layout");
static uint64_t seed=UINT64_C(0x6cf83adf7135e290);
static uint64_t next(void){seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;return seed;}
static double number(uint64_t u){double d;memcpy(&d,&u,8);return d;}
static uint64_t word(double d){uint64_t u;memcpy(&u,&d,8);return u;}
static uint64_t random_finite(void){uint64_t u=next();if((u&UINT64_C(0x7ff0000000000000))==UINT64_C(0x7ff0000000000000))u^=UINT64_C(0x0010000000000000);return u;}
static void observe(double actual,double point,double center){
 volatile double p=point,c=center;double expected=p-c;
 if(isnan(expected)){assert(isnan(actual));actual=number(UINT64_C(0x7ff8000000000000));}
 else assert(word(actual)==word(expected));
 uint64_t u=word(actual);assert(fwrite(&u,8,1,stdout)==1);
}
int main(void){
 const uint64_t special[16]={0,UINT64_C(0x8000000000000000),UINT64_C(0x3ff0000000000000),UINT64_C(0xbff0000000000000),1,UINT64_C(0x8000000000000001),UINT64_C(0x0010000000000000),UINT64_C(0x8010000000000000),UINT64_C(0x7fefffffffffffff),UINT64_C(0xffefffffffffffff),UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),UINT64_C(0x7ff8000000001234),UINT64_C(0xfff8000000004321),UINT64_C(0x4266a51b25db0000),UINT64_C(0xc266a51b25db0000)};
 for(int i=0;i<512;i++){
  double c[3],p[3];for(int axis=0;axis<3;axis++){c[axis]=number(i<256?special[(i/16+axis)%16]:random_finite());p[axis]=number(i<256?special[(i+3*axis)%16]:random_finite());}
  coax_t_D3 center={c[0],c[1],c[2]},point={p[0],p[1],p[2]};coax_t_SpaceCam camera={0};camera.center=center;coax_t_D3 got=SCN_relative(camera,point);
  observe(got.x,p[0],c[0]);observe(got.y,p[1],c[1]);observe(got.z,p[2],c[2]);
 }
 return fflush(stdout)!=0;
}
