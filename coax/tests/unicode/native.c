// Explicit conformance runner and actual caller boundary tests, not runtime code.
#define _POSIX_C_SOURCE 200809L
#include "nfkc.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
static uint8_t input[1040],original[1024],output[73744],expected[73728];
static uint32_t scratch[18434];
static uint32_t word(FILE *f){uint8_t b[4];assert(fread(b,1,4,f)==4);return(uint32_t)b[0]|(uint32_t)b[1]<<8|(uint32_t)b[2]<<16|(uint32_t)b[3]<<24;}
static void boundaries(void){
 coax_t_UnResult empty=un_nfkc((coax_string){NULL,0},(coax_v_u32){NULL,0},(coax_v_u8){NULL,0},0);assert(empty.error==0&&empty.written==0);
 uint8_t tiny[3]={165,65,165};uint32_t work[3]={165,0,165};
 coax_string text={(const char *)tiny+1,1};coax_v_u32 space={work+1,1};coax_v_u8 out={tiny+1,1};
 coax_t_UnResult r=un_nfkc(text,space,out,1);assert(r.error==4&&r.written==0&&tiny[1]==65);
 const int64_t bad_input[3]={-1,1025,INT64_MAX},bad_scratch[3]={-1,18433,INT64_MAX},bad_output[3]={-1,73729,INT64_MAX};
 for(int i=0;i<3;i++){
   r=un_nfkc((coax_string){text.data,bad_input[i]},space,out,0);assert(r.error==2&&r.written==0);
   r=un_nfkc(text,(coax_v_u32){space.data,bad_scratch[i]},out,0);assert(r.error==2&&r.written==0);
   r=un_nfkc(text,space,(coax_v_u8){out.data,bad_output[i]},0);assert(r.error==2&&r.written==0);
 }
 uint8_t malformed[4]={244,144,128,128};uint8_t destination[4]={165,165,165,165};
 r=un_nfkc((coax_string){(const char*)malformed,4},space,(coax_v_u8){destination,4},0);assert(r.error==1&&r.written==0);
 for(int i=0;i<4;i++)assert(destination[i]==165);
 uint8_t maximum[1024];memset(maximum,65,sizeof maximum);
 r=un_nfkc((coax_string){(const char*)maximum,1024},(coax_v_u32){scratch+1,18432},(coax_v_u8){output+8,1024},0);
 assert(r.error==0&&r.written==1024);for(int i=0;i<1024;i++)assert(output[8+i]==65);
 memset(output,165,1040);
 r=un_nfkc((coax_string){(const char*)maximum,1024},(coax_v_u32){scratch+1,18432},(coax_v_u8){output+8,1023},0);
 assert(r.error==3&&r.written==0);for(int i=0;i<1040;i++)assert(output[i]==165);
 assert(tiny[0]==165&&tiny[1]==65&&tiny[2]==165&&work[0]==165&&work[2]==165);
}
int main(int argc,char **argv){
 boundaries();
 if(argc==1){puts("native boundary checks passed");return 0;}
 assert(argc==2);FILE *f=fopen(argv[1],"rb");assert(f);assert(word(f)==0x314b464e);uint32_t count=word(f);
 for(uint32_t row=0;row<count;row++){
   uint32_t n=word(f),m=word(f);assert(n<=1024&&m<=73728);
   memset(input,165,n+16);memset(output,165,m+16);scratch[0]=scratch[18433]=0xa5a5a5a5;
   assert(fread(input+8,1,n,f)==n&&fread(expected,1,m,f)==m);memcpy(original,input+8,n);
   struct timespec begun,ended;if(row>=count-3)assert(clock_gettime(CLOCK_MONOTONIC,&begun)==0);
   coax_t_UnResult r=un_nfkc((coax_string){(const char*)input+8,n},(coax_v_u32){scratch+1,18432},(coax_v_u8){output+8,m},0);
   if(row>=count-3){assert(clock_gettime(CLOCK_MONOTONIC,&ended)==0);long long ns=(ended.tv_sec-begun.tv_sec)*1000000000LL+ended.tv_nsec-begun.tv_nsec;printf("boundary_timing %u %lld\n",row-(count-3),ns);}
   if(r.error||r.written!=m||memcmp(output+8,expected,m)){fprintf(stderr,"NFKC mismatch row %u, error %lld, bytes %lld expected %u\n",row,(long long)r.error,(long long)r.written,m);return 1;}
   assert(!memcmp(input+8,original,n));
   for(int i=0;i<8;i++)assert(input[i]==165&&input[8+n+i]==165&&output[i]==165&&output[8+m+i]==165);
   assert(scratch[0]==0xa5a5a5a5&&scratch[18433]==0xa5a5a5a5);
 }
 assert(fgetc(f)==EOF);fclose(f);printf("%u full corpus/scalar cases passed\n",count);
}
