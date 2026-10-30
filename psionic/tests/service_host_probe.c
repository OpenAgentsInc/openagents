/* Host observations only: generated Coax does copy and ordered dot work. */
#define _POSIX_C_SOURCE 200809L
#include "serve.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
int main(void){
    int64_t n=16777216;float *x=malloc((size_t)n*4),*y=malloc((size_t)n*4);assert(x && y);for(int64_t i=0;i<n;i++)x[i]=y[i]=0.5f;
    double copy[5],compute[5];for(int r=0;r<5;r++){double t=now();assert(ls_probe_copy((coax_v_f32){x,n},(coax_v_f32){y,n}));copy[r]=now()-t;t=now();assert(ls_probe_dot((coax_v_f32){x,n},(coax_v_f32){y,n})==(double)n*0.25);compute[r]=now()-t;}
    printf("{\"schema\":\"psionic-host-probes-v1\",\"elements\":%lld,\"copy_bytes\":%lld,\"dot_ops\":%lld,\"copy_seconds\":[",(long long)n,(long long)n*8,(long long)n*2);
    for(int i=0;i<5;i++)printf("%s%.9f",i?",":"",copy[i]);printf("],\"dot_seconds\":[");for(int i=0;i<5;i++)printf("%s%.9f",i?",":"",compute[i]);puts("]}");free(x);free(y);
}
