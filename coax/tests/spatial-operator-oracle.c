/* Independent fixed-coordinate scalar oracle for actual mutated Coax C. */
#include "operators.h"
#include <assert.h>
#include <float.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
_Static_assert(sizeof(float)==4 && sizeof(double)==8 && FLT_RADIX==2 && FLT_MANT_DIG==24 && DBL_MANT_DIG==53,"binary32/binary64 witness profile");
static void expected(double out[30]) {
 const double p[]={10,20,30},q[]={1,2,3},v[]={8,16,32},w[]={1,2,4};
 for(int a=0;a<3;a++) {out[a]=p[a]+v[a];out[3+a]=p[a]-v[a];out[6+a]=v[a]+w[a];out[9+a]=v[a]-w[a];out[12+a]=v[a]*2;out[15+a]=v[a]/2;out[18+a]=p[a]-q[a];out[21+a]=v[a]+p[a];out[24+a]=2*v[a];out[27+a]=-v[a];}
}
int main(int argc,char **argv) {
 assert(argc==3);int width=atoi(argv[1]),segment=atoi(argv[2]);assert((width==0&&segment==-1)||((width==32||width==64)&&segment>=0&&segment<6));
 float got32[30];double got64[30],base[30];sm_sample32((coax_v_f32){got32,30});sm_sample64((coax_v_f64){got64,30});expected(base);
 int differences=0;
 for(int i=0;i<30;i++) {
  double e32=base[i],e64=base[i];if(segment>=0&&i/3==segment) {int paired=segment^1;if(width==32)e32=base[paired*3+i%3];else e64=base[paired*3+i%3];}
  float f=(float)e32;assert(memcmp(got32+i,&f,sizeof f)==0);assert(memcmp(got64+i,&e64,sizeof e64)==0);
  differences+=(got32[i]!=(float)base[i]);differences+=(got64[i]!=base[i]);
 }
 assert(differences==(width?3:0));assert(fwrite(got32,sizeof(float),30,stdout)==30);assert(fwrite(got64,sizeof(double),30,stdout)==30);return fflush(stdout)!=0;
}
