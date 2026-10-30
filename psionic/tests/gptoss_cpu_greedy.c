/* Chapter 45's CPU engine, greedy, on a prompt the GPU harness rendered:
   the reference for the Metal engine's token identity (run_gptoss_metal.sh,
   PSIONIC_CPU_GREEDY=1). One token per forward, single core: minutes.
   Usage: gptoss-cpu-greedy ORIGINAL_DIR PROMPT.i64 OUT.i64 */
#include "gptoss-memory.h"
#include <assert.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
static double now(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return t.tv_sec+t.tv_nsec*1e-9;}
static void *alloc(int64_t n,size_t width){void *p=calloc((size_t)(n?n:1),width);assert(p);return p;}
static uint8_t *load(const char *name,size_t *size){int fd=open(name,O_RDONLY);if(fd<0){perror(name);exit(2);}struct stat st;assert(!fstat(fd,&st) && st.st_size>0);*size=(size_t)st.st_size;uint8_t *p=mmap(NULL,*size,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);assert(!close(fd));return p;}
int main(int argc,char **argv){
    if(argc!=4){fprintf(stderr,"usage: gptoss-cpu-greedy ORIGINAL_DIR PROMPT.i64 OUT.i64\n");return 2;}
    coax_t_GFile files[16];void *maps[16];size_t sizes[16];int nf=(int)g_original_files(0);assert(nf>0 && nf<=16);
    for(int i=0;i<nf;i++){coax_string name=g_original_filename(0,i);char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[1],(int)name.count,name.data)>0);maps[i]=load(path,&sizes[i]);files[i]=(coax_t_GFile){name,(coax_v_u8){maps[i],(int64_t)sizes[i]}};}
    coax_t_GBlock blocks[36],draft[36];coax_t_GModel model;double start=now();
    coax_t_GResult bound=psionic_gptoss_bind(0,256,(coax_v_t_GFile){files,nf},(coax_v_t_GBlock){blocks,36},(coax_v_t_GBlock){draft,36},(coax_v_t_GModel){&model,1});
    if(bound.status){fprintf(stderr,"original binding refused: %u\n",bound.status);return 2;}
    printf("bound %.3f seconds\n",now()-start);fflush(stdout);
    size_t prompt_bytes;const int64_t *prompt=(const int64_t*)load(argv[2],&prompt_bytes);int64_t prefix=(int64_t)(prompt_bytes/8);
    coax_t_GPlan p=g_plan(model.shape);assert(p.status==0);int64_t trace_n=p.begin.e[13]-p.begin.e[12],vocab=model.shape.vocab;
    coax_t_GWork w={{alloc(p.floats,4),p.floats},{alloc(p.scores,8),p.scores},{alloc(p.sums,sizeof(coax_t_PsionicAccumulator)),p.sums},{alloc(p.routes,8),p.routes}};
    float *cache=alloc(p.cache,4),*logits=alloc(vocab,4),*trace=alloc(trace_n,4),*weights=alloc(p.routes,4);int64_t *routes=alloc(p.routes,8);
    coax_t_GOutput out={{logits,vocab},{trace,trace_n},{routes,p.routes},{weights,p.routes}};
    start=now();
    for(int64_t i=0;i<prefix;i++){coax_t_GStep r=psionic_gptoss_forward(model,prompt[i],i,g_macs(model.shape,i),(coax_v_f32){cache,p.cache},out,w);if(r.status){fprintf(stderr,"forward refused at %lld: %u\n",(long long)i,r.status);return 3;}}
    printf("prefix=%lld forwarded in %.1f s\n",(long long)prefix,now()-start);fflush(stdout);
    double *prob=alloc(vocab,8),*scratch=alloc(vocab,8);int64_t *order=alloc(vocab,8),*temporary=alloc(vocab,8),generated[256];int64_t count=0;
    coax_t_GSampleWork sample={{scratch,vocab},{order,vocab},{temporary,vocab}};coax_t_GGeneration state={.remaining=256};coax_t_GSampling greedy={.temperature=0,.top_k=0,.top_p=1};
    while(1){coax_t_GEmission e=psionic_gptoss_emit((coax_v_f32){logits,vocab},(coax_v_u8){NULL,0},greedy,state,0,(coax_v_f64){prob,vocab},sample);assert(!e.status);state=e.state;generated[count++]=e.token;
        if(state.stop!=COAX_GStop_running)break;
        int64_t at=prefix+count-1;coax_t_GStep r=psionic_gptoss_forward(model,e.token,at,g_macs(model.shape,at),(coax_v_f32){cache,p.cache},out,w);if(r.status){fprintf(stderr,"forward refused at %lld: %u\n",(long long)at,r.status);return 3;}}
    printf("greedy emitted=%lld stop=%u in %.1f s:",(long long)count,state.stop,now()-start);for(int64_t i=0;i<count;i++)printf(" %lld",(long long)generated[i]);printf("\n");
    FILE *f=fopen(argv[3],"wb");assert(f && fwrite(generated,8,(size_t)count,f)==(size_t)count);assert(!fclose(f));
    return 0;
}
