#include "cuda-host.h"
#include <assert.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char **argv){assert(argc==2);FILE *f=fopen(argv[1],"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);rewind(f);char *source=malloc((size_t)n);assert(source && fread(source,1,(size_t)n,f)==(size_t)n);fclose(f);PcDeviceInfo info;assert(!pc_open(&info));PcModule m;double secs;assert(!pc_compile(source,(size_t)n,argv[1],&m,&secs));void *driver=dlopen("libcuda.so.1",RTLD_NOW);assert(driver);int (*attribute)(int*,int,void*);*(void**)&attribute=dlsym(driver,"cuFuncGetAttribute");assert(attribute);const char *names[]={"gk_batch_mx_part","gk_batch_bf_part","gk_batch_scores","gk_batch_context_part","gk_mx_part","gk_bf_part"};for(int i=0;i<6;i++){PcKernel k;assert(!pc_kernel(m,names[i],&k));int regs,local,shared,maximum;assert(!attribute(&regs,4,k) && !attribute(&local,3,k) && !attribute(&shared,1,k) && !attribute(&maximum,0,k));printf("%s registers_per_thread=%d local_bytes_per_thread=%d shared_bytes_per_block=%d maximum_block_threads=%d\n",names[i],regs,local,shared,maximum);}pc_unload(m);free(source);return 0;}
