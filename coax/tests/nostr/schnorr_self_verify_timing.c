/* Native timing acquisition for #89db. Public fixture data only. Class 0 is
   official BIP-340 vector 0's valid signature; class 1 is a seeded random
   64-byte candidate for the same message and key (almost always invalid). */
#define _GNU_SOURCE
#include "schnorr_self_verify_timing.h"
#include <time.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>
static volatile uint64_t observed;
static uint64_t seed=20261029;
static uint64_t random_word(void){seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;return seed;}
static uint64_t ticks(void){struct timespec t;if(clock_gettime(CLOCK_MONOTONIC_RAW,&t))abort();return(uint64_t)t.tv_sec*UINT64_C(1000000000)+(uint64_t)t.tv_nsec;}
static void little(uint8_t*out,uint64_t value,unsigned bytes){for(unsigned i=0;i<bytes;i++)out[i]=(uint8_t)(value>>(i*8));}
static const uint8_t KEY[32]={249,48,138,1,146,88,195,16,73,52,79,133,248,157,82,41,181,49,200,69,131,111,153,176,134,1,241,19,188,224,54,249};
static const uint8_t SIG[64]={233,7,131,31,128,132,141,16,105,165,55,27,64,36,16,54,75,223,28,95,131,7,176,8,76,85,241,206,45,202,130,21,37,246,106,74,133,234,139,113,228,130,167,79,56,45,44,229,235,238,232,253,178,23,47,71,125,244,144,13,49,5,54,192};
static uint8_t message[32],key[32];static coax_t_SsvWork work;static int which;
static uint64_t call(uint8_t*candidate){
 coax_v_u8 m={message,32},k={key,32},c={candidate,64};
 if(which==0){coax_v_t_SsvWork w={&work,1};uint64_t r;do r=ssv_step(m,k,c,w,false);while(r==1);return r;}
 if(which==1)return ssvt_control_fixed(c);
 if(which==2)return ssvt_control_branch(c);
 return schnorr_fast_verify_vartime(m,k,c);
}
int main(int argc,char**argv){
 if(argc!=4)return 2;
 which=atoi(argv[1]);if(which<0||which>3)return 2;
 unsigned count=(unsigned)strtoul(argv[3],0,10);if(count<4||count>2000000||count%2)return 2;
 memcpy(key,KEY,32);
 uint8_t*classes=malloc(count);uint8_t*inputs=malloc((size_t)count*64);uint64_t*elapsed=malloc(count*sizeof(*elapsed));
 if(!classes||!inputs||!elapsed)return 3;
 for(unsigned i=0;i<count;i+=2){classes[i]=(uint8_t)(random_word()&1);classes[i+1]=1-classes[i];}
 for(unsigned i=0;i<count;i++){if(classes[i])for(unsigned j=0;j<64;j++)inputs[(size_t)i*64+j]=(uint8_t)random_word();else memcpy(inputs+(size_t)i*64,SIG,64);}
 /* sanity: the fixed class verifies, in this binary, under both verifiers */
 {int saved=which;which=0;uint8_t s[64];memcpy(s,SIG,64);if(call(s)!=UINT64_MAX)return 7;which=3;if(call(s)!=1)return 7;which=saved;}
 for(unsigned i=0;i<2000;i++)observed=call(inputs+(size_t)(i%count)*64);
 struct timespec resolution;if(clock_getres(CLOCK_MONOTONIC_RAW,&resolution))return 4;
 uint64_t calibration=UINT64_MAX;
 for(unsigned i=0;i<10000;i++){uint64_t a=ticks(),b=ticks();if(b-a<calibration)calibration=b-a;}
 uint64_t start=ticks();
 for(unsigned i=0;i<count;i++){uint64_t a=ticks();uint64_t r=call(inputs+(size_t)i*64);uint64_t b=ticks();observed=r;elapsed[i]=b-a;}
 uint64_t duration=ticks()-start;coax_t_CtStats stats={0};
 for(unsigned i=0;i<count;i++)if(!ct_observe((coax_v_t_CtStats){&stats,1},classes[i],(double)elapsed[i]))return 5;
 coax_t_CtResult r=ct_result(stats);FILE*out=fopen(argv[2],"wb");if(!out)return 6;
 uint8_t header[12]={'C','T','0','1'};little(header+4,count,8);if(fwrite(header,1,12,out)!=12)return 6;
 for(unsigned i=0;i<count;i++){uint8_t record[9];record[0]=classes[i];little(record+1,elapsed[i],8);if(fwrite(record,1,9,out)!=9)return 6;}
 if(fclose(out))return 6;
 printf("{\"fixture\":%d,\"status\":%lld,\"n0\":%lld,\"n1\":%lld,\"mean0\":%.17g,\"mean1\":%.17g,\"variance0\":%.17g,\"variance1\":%.17g,\"t\":%.17g,\"degenerate\":%s,\"clock_resolution_ns\":%lld,\"minimum_clock_pair_ns\":%llu,\"acquisition_ns\":%llu,\"warmup\":2000}\n",which,(long long)r.status,(long long)r.n0,(long long)r.n1,r.mean0,r.mean1,r.variance0,r.variance1,r.t,r.degenerate?"true":"false",(long long)resolution.tv_sec*1000000000+resolution.tv_nsec,(unsigned long long)calibration,(unsigned long long)duration);
 free(classes);free(inputs);free(elapsed);return 0;
}
