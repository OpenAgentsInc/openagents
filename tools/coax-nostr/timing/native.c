/* Acquisition, platform observations and file effects only. Policy/statistics
 * and measured public fixtures are generated Coax. No production keys. */
#define _GNU_SOURCE
#include "timing.c"
#include <time.h>
#include <errno.h>
#if defined(__linux__)
#include <sched.h>
#endif

typedef uint64_t (*fixture)(uint64_t,coax_v_u64);
static fixture volatile selected;
static volatile uint64_t observed;
static uint64_t seed=20261028;
static uint64_t random_word(void){seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;return seed;}
static uint64_t ticks(void){struct timespec t;if(clock_gettime(CLOCK_MONOTONIC_RAW,&t))abort();return(uint64_t)t.tv_sec*1000000000+(uint64_t)t.tv_nsec;}
static void little(uint8_t*out,uint64_t value,unsigned bytes){for(unsigned i=0;i<bytes;i++)out[i]=(uint8_t)(value>>(i*8));}

int main(int argc,char**argv){
 if(argc!=3)return 2;
 const unsigned count=1000000,table_count=1048576;
 int which=atoi(argv[1]),affinity=-1;
 if(which<0||which>2)return 2;
 #if defined(__linux__)
 cpu_set_t allowed,one;CPU_ZERO(&allowed);
 if(!sched_getaffinity(0,sizeof(allowed),&allowed))for(int i=0;i<CPU_SETSIZE;i++)if(CPU_ISSET(i,&allowed)){
  CPU_ZERO(&one);CPU_SET(i,&one);if(!sched_setaffinity(0,sizeof(one),&one))affinity=i;break;
 }
 #endif
 uint8_t*classes=malloc(count);uint64_t*inputs=malloc(count*sizeof(uint64_t));
 uint64_t*elapsed=malloc(count*sizeof(uint64_t)),*table=malloc(table_count*sizeof(uint64_t));
 if(!classes||!inputs||!elapsed||!table)return 3;
 // Balanced order randomized independently of fixture input; preparation is
 // outside measurement. Fixed class0 always receives zero, class1 random.
 for(unsigned i=0;i<count;i+=2){classes[i]=(uint8_t)(random_word()&1);classes[i+1]=1-classes[i];}
 for(unsigned i=0;i<count;i++)inputs[i]=classes[i]?random_word():0;
 table[0]=0;for(unsigned i=1;i<table_count;i++)table[i]=1+(random_word()%(table_count-1));
 coax_v_u64 span={table,table_count};
 selected=which==0?ct_fixed:which==1?ct_branch:ct_index;
 for(unsigned i=0;i<20000;i++)observed=selected(inputs[i],span);
 struct timespec resolution;if(clock_getres(CLOCK_MONOTONIC_RAW,&resolution))return 4;
 uint64_t calibration=UINT64_MAX;
 for(unsigned i=0;i<10000;i++){uint64_t a=ticks(),b=ticks();if(b-a<calibration)calibration=b-a;}
 uint64_t start=ticks();
 for(unsigned i=0;i<count;i++){
  uint64_t a=ticks();uint64_t result=selected(inputs[i],span);uint64_t b=ticks();
  observed=result;elapsed[i]=b-a;
 }
 uint64_t duration=ticks()-start;
 // Evaluate only after acquisition; classifier branches cannot condition the
 // following observation's instruction/data cache during collection.
 coax_t_CtStats stats={0};
 for(unsigned i=0;i<count;i++)if(!ct_observe((coax_v_t_CtStats){&stats,1},classes[i],(double)elapsed[i]))return 5;
 coax_t_CtResult r=ct_result(stats);
 FILE*out=fopen(argv[2],"wb");if(!out)return 6;
 uint8_t header[12]={'C','T','0','1'};little(header+4,count,8);
 if(fwrite(header,1,12,out)!=12)return 6;
 for(unsigned i=0;i<count;i++){uint8_t record[9];record[0]=classes[i];little(record+1,elapsed[i],8);if(fwrite(record,1,9,out)!=9)return 6;}
 if(fclose(out))return 6;
 printf("{\"fixture\":%d,\"status\":%lld,\"n0\":%lld,\"n1\":%lld,\"mean0\":%.17g,\"mean1\":%.17g,\"variance0\":%.17g,\"variance1\":%.17g,\"t\":%.17g,\"degenerate\":%s,\"clock_resolution_ns\":%lld,\"minimum_clock_pair_ns\":%llu,\"acquisition_ns\":%llu,\"affinity_cpu\":%d,\"warmup\":20000,\"raw_bytes\":9000012}\n",
 which,(long long)r.status,(long long)r.n0,(long long)r.n1,r.mean0,r.mean1,r.variance0,r.variance1,r.t,r.degenerate?"true":"false",
 (long long)resolution.tv_sec*1000000000+resolution.tv_nsec,(unsigned long long)calibration,(unsigned long long)duration,affinity);
 free(classes);free(inputs);free(elapsed);free(table);return 0;
}
