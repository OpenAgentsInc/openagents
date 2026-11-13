// Firm reason: 2 (independent CLOCK_PROCESS_CPUTIME_ID oracle for profiler units).
#include "../face.h"
#include <assert.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>
static uint64_t clock_ns(void) {struct timespec t;assert(clock_gettime(CLOCK_PROCESS_CPUTIME_ID,&t)==0);return (uint64_t)t.tv_sec*1000000000+t.tv_nsec;}
int main(void) {
  uint64_t cpu=clock_ns(),reported=editor_process_cpu_ns(getpid());
  volatile uint64_t sum=1;
  while(clock_ns()-cpu<200000000) {for(int i=0;i<10000;i++)sum=sum*6364136223846793005ULL+1;}
  uint64_t actual=clock_ns()-cpu,observed=editor_process_cpu_ns(getpid())-reported;
  double ratio=(double)observed/(double)actual;
  printf("process CPU clock: actual=%llu ns reported=%llu ns ratio=%.5f\n",(unsigned long long)actual,(unsigned long long)observed,ratio);
  assert(ratio>0.95 && ratio<1.05);
}
