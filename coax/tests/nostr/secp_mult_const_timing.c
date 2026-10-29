/* Native timing acquisition/observable output only. Public fixture data. */
#define _GNU_SOURCE
#include "secp_mult_const_timing.h"
#include <time.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
typedef coax_t_SgPoint (*base_fixture)(coax_t_ScScalar);
static base_fixture volatile selected;
static volatile uint64_t observed;
static uint64_t seed=20261029;
static uint64_t random_word(void){seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;return seed;}
static uint64_t ticks(void){struct timespec t;if(clock_gettime(CLOCK_MONOTONIC_RAW,&t))abort();return(uint64_t)t.tv_sec*UINT64_C(1000000000)+(uint64_t)t.tv_nsec;}
static void little(uint8_t*out,uint64_t value,unsigned bytes){for(unsigned i=0;i<bytes;i++)out[i]=(uint8_t)(value>>(i*8));}
int main(int argc,char**argv){
 if(argc!=3)return 2;
 int which=atoi(argv[1]);if(which<0||which>2)return 2;
 const unsigned count=1000000;
 uint8_t*classes=malloc(count);coax_t_ScScalar*inputs=calloc(count,sizeof(*inputs));uint64_t*elapsed=malloc(count*sizeof(*elapsed));
 if(!classes||!inputs||!elapsed)return 3;
 for(unsigned i=0;i<count;i+=2){classes[i]=(uint8_t)(random_word()&1);classes[i+1]=1-classes[i];}
 // All preparation is outside measurement. Fixed class0 is scalar zero;
 // class1 is a seeded256-bit public scalar, normalized inside the Coax API.
 for(unsigned i=0;i<count;i++)if(classes[i])for(unsigned j=0;j<8;j++)inputs[i].limbs.e[j]=(uint32_t)random_word();
 selected=which==0?smc_timing:which==1?smc_control_fixed:smc_control_branch;
 for(unsigned i=0;i<20000;i++){coax_t_SgPoint p=selected(inputs[i]);observed=p.x.limb.e[0]^p.y.limb.e[0]^p.z.limb.e[0];}
 struct timespec resolution;if(clock_getres(CLOCK_MONOTONIC_RAW,&resolution))return 4;
 uint64_t calibration=UINT64_MAX;
 for(unsigned i=0;i<10000;i++){uint64_t a=ticks(),b=ticks();if(b-a<calibration)calibration=b-a;}
 uint64_t start=ticks();
 for(unsigned i=0;i<count;i++){
  uint64_t a=ticks();coax_t_SgPoint p=selected(inputs[i]);uint64_t b=ticks();
  observed=p.x.limb.e[0]^p.y.limb.e[0]^p.z.limb.e[0];elapsed[i]=b-a;
 }
 uint64_t duration=ticks()-start;coax_t_CtStats stats={0};
 for(unsigned i=0;i<count;i++)if(!ct_observe((coax_v_t_CtStats){&stats,1},classes[i],(double)elapsed[i]))return 5;
 coax_t_CtResult r=ct_result(stats);FILE*out=fopen(argv[2],"wb");if(!out)return 6;
 uint8_t header[12]={'C','T','0','1'};little(header+4,count,8);if(fwrite(header,1,12,out)!=12)return 6;
 for(unsigned i=0;i<count;i++){uint8_t record[9];record[0]=classes[i];little(record+1,elapsed[i],8);if(fwrite(record,1,9,out)!=9)return 6;}
 if(fclose(out))return 6;
 printf("{\"fixture\":%d,\"status\":%lld,\"n0\":%lld,\"n1\":%lld,\"mean0\":%.17g,\"mean1\":%.17g,\"variance0\":%.17g,\"variance1\":%.17g,\"t\":%.17g,\"degenerate\":%s,\"clock_resolution_ns\":%lld,\"minimum_clock_pair_ns\":%llu,\"acquisition_ns\":%llu,\"warmup\":20000,\"raw_bytes\":9000012}\n",which,(long long)r.status,(long long)r.n0,(long long)r.n1,r.mean0,r.mean1,r.variance0,r.variance1,r.t,r.degenerate?"true":"false",(long long)resolution.tv_sec*1000000000+resolution.tv_nsec,(unsigned long long)calibration,(unsigned long long)duration);
 free(classes);free(inputs);free(elapsed);return 0;
}
