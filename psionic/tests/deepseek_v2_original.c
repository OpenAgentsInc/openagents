/* Opt-in qualification: mmap/I/O, allocation, clocks and execution scheduling.
 * Coax owns admission, all arithmetic, greedy selection, stop and decoding. */
#define _POSIX_C_SOURCE 200809L
#include "deepseek-v2-memory.h"
#include <assert.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#define V(a,n) {a,n}
static void *alloc(int64_t n,size_t s) {void *p=calloc((size_t)n,s);assert(p);return p;}
static double now(void) {struct timespec t;assert(clock_gettime(CLOCK_MONOTONIC,&t)==0);return (double)t.tv_sec+1e-9*(double)t.tv_nsec;}
static void put(FILE *f,const void *p,size_t size,size_t count) {assert(fwrite(p,size,count,f)==count);}
static coax_v_u8 mapping(const char *dir,coax_string name) {
    char path[4096];assert(snprintf(path,sizeof(path),"%s/%.*s",dir,(int)name.count,name.data)>0);
    int fd=open(path,O_RDONLY);assert(fd>=0);struct stat s;assert(fstat(fd,&s)==0 && s.st_size>0);
    void *p=mmap(NULL,(size_t)s.st_size,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);close(fd);return (coax_v_u8){p,s.st_size};
}
static FILE *output(const char *dir,const char *name,const char *mode) {
    char path[4096];assert(snprintf(path,sizeof(path),"%s/%s",dir,name)>0);FILE *f=fopen(path,mode);assert(f);return f;
}
static void run(coax_t_DvModel model,coax_t_MlMode mode,coax_v_u8 table,const char *dir) {
    const char *name=mode?"expanded":"compressed";char path[64];coax_t_DvShape s=model.shape;coax_t_DvPlan p=dv_plan(s,mode);assert(p.status==COAX_DsStatus_ok);
    float *cache=alloc(p.cache,sizeof(float)),*logits=alloc(s.vocab,sizeof(float)),*trace=alloc(2*s.layers*s.width,sizeof(float));
    coax_t_DrRoute *routes=alloc(s.layers,sizeof(*routes)),*draft=alloc(s.layers,sizeof(*draft));
    coax_t_DvWork work={.floats={alloc(p.floats,sizeof(float)),p.floats},.scores={alloc(p.scores,sizeof(double)),p.scores},.accumulators={alloc(p.accumulators,sizeof(coax_t_PsionicAccumulator)),p.accumulators},.routes={draft,s.layers}};
    coax_t_DvOutput out={.logits={logits,s.vocab},.trace={trace,2*s.layers*s.width},.routes={routes,s.layers}};
    int64_t prompt[8],answer[128];int64_t n=dv_test_hi((coax_v_s64){prompt,8});assert(n==8);int64_t pos=0,generated=0,token=prompt[0];
    snprintf(path,sizeof(path),"%s.bin",name);FILE *binary=output(dir,path,"wb");put(binary,"DV2RUN01",1,8);
    int64_t dims[]={s.vocab,2*s.layers*s.width,s.layers};put(binary,dims,sizeof(int64_t),3);
    double start=now(),prefill=0;coax_t_DvStop stop=COAX_DvStop_more;
    for (;;) {
        double begin=now();coax_t_DsResult result=psionic_dv_step(model,mode,token,pos,(coax_v_f32){cache,p.cache},out,work);
        if (result.status!=COAX_DsStatus_ok) {fprintf(stderr,"%s step %" PRId64 " status %d\n",name,pos,result.status);abort();}
        put(binary,&token,sizeof(token),1);put(binary,&pos,sizeof(pos),1);put(binary,logits,sizeof(float),(size_t)s.vocab);put(binary,trace,sizeof(float),(size_t)(2*s.layers*s.width));
        for (int64_t l=0;l<s.layers;l++) {put(binary,routes[l].ids.e,sizeof(int64_t),8);put(binary,routes[l].weights.e,sizeof(double),8);put(binary,&routes[l].margin,sizeof(double),1);}
        fflush(binary);fprintf(stderr,"%s position %" PRId64 " input %" PRId64 " %.3fs\n",name,pos,token,now()-begin);pos=result.next;
        if (pos<n) {token=prompt[pos];continue;}
        if (pos==n) prefill=now()-start;
        coax_t_DvDecision decision=dv_decide(out.logits,dv_original_eos(),generated,128,pos,s.attention.capacity,0);assert(decision.status==COAX_DsStatus_ok);
        stop=decision.stop;if (stop==COAX_DvStop_more || stop==COAX_DvStop_eos) answer[generated++]=decision.token;
        if (stop!=COAX_DvStop_more) break;token=decision.token;
    }
    fclose(binary);double total=now()-start;uint8_t decoded[65536],stage[65536];
    coax_t_DsResult decoded_result=psionic_dv_decode(table,(coax_v_s64){answer,generated},(coax_v_u8){decoded,65536},(coax_v_u8){stage,65536});assert(decoded_result.status==COAX_DsStatus_ok);
    snprintf(path,sizeof(path),"%s.txt",name);FILE *text=output(dir,path,"wb");put(text,decoded,1,(size_t)decoded_result.written);fclose(text);
    snprintf(path,sizeof(path),"%s.json",name);FILE *json=output(dir,path,"w");struct rusage usage;assert(getrusage(RUSAGE_SELF,&usage)==0);
    fprintf(json,"{\"mode\":\"%s\",\"capacity\":%" PRId64 ",\"stop\":%d,\"eos\":%s,\"steps\":%" PRId64 ",\"prompt\":[",name,s.attention.capacity,stop,stop==COAX_DvStop_eos?"true":"false",pos);
    for (int64_t i=0;i<n;i++) fprintf(json,"%s%" PRId64,i?",":"",prompt[i]);fprintf(json,"],\"generated\":[");
    for (int64_t i=0;i<generated;i++) fprintf(json,"%s%" PRId64,i?",":"",answer[i]);
    fprintf(json,"],\"prefill_seconds\":%.9f,\"total_seconds\":%.9f,\"peak_rss_kib\":%ld,\"cache_bytes\":%" PRId64 "}\n",prefill,total,usage.ru_maxrss,4*p.cache);fclose(json);
    fprintf(stderr,"%s finished stop=%d tokens=%" PRId64 " seconds=%.3f\n",name,stop,generated,total);
    assert(stop==COAX_DvStop_eos);
    free(cache);free(logits);free(trace);free(routes);free(draft);free(work.floats.data);free(work.scores.data);free(work.accumulators.data);
}
int main(int argc,char **argv) {
    assert(argc==4);coax_t_DsArtifact pins[7];coax_t_DvTensor *tensors=alloc(5291,sizeof(*tensors));int64_t payload[7];
    coax_t_DvProfile p=dv_original_profile(256,(coax_v_t_DsArtifact){pins,7},(coax_v_t_DvTensor){tensors,5291},(coax_v_s64){payload,7});assert(p.pins.count==7);
    coax_t_DsFile files[7];for (int i=0;i<7;i++) files[i]=(coax_t_DsFile){.name=pins[i].name,.data=mapping(argv[1],pins[i].name)};
    int64_t count=(p.shape.layers-p.shape.dense_layers)*p.shape.experts;coax_t_DvBlock *blocks=alloc(p.shape.layers,sizeof(*blocks)),*draft=alloc(p.shape.layers,sizeof(*draft));
    coax_t_DvExpert *experts=alloc(count,sizeof(*experts)),*de=alloc(count,sizeof(*de));coax_t_DvModel model={0};
    double begin=now();fprintf(stderr,"authenticating all original bytes\n");
    coax_t_DsResult r=psionic_dv_bind(p,(coax_v_t_DsFile){files,7},(coax_v_t_DvBlock){blocks,p.shape.layers},(coax_v_t_DvExpert){experts,count},(coax_v_t_DvBlock){draft,p.shape.layers},(coax_v_t_DvExpert){de,count},(coax_v_t_DvModel){&model,1});
    fprintf(stderr,"admission status %d in %.3fs\n",r.status,now()-begin);assert(r.status==COAX_DsStatus_ok);free(draft);free(de);
    coax_string table_name={"v2-token-bytes.bin",18};table_name.count=(int64_t)strlen(table_name.data);coax_v_u8 table=mapping(argv[2],table_name);assert(dv_text_admit(table));
    run(model,COAX_MlMode_compressed,table,argv[3]);run(model,COAX_MlMode_expanded,table,argv[3]);
    for (int i=0;i<7;i++) munmap(files[i].data.data,(size_t)files[i].data.count);munmap(table.data,(size_t)table.count);free(tensors);free(blocks);free(experts);return 0;
}
