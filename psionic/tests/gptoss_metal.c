/* gpt-oss on an Apple GPU (psionic/platform/gptoss-metal.c): the owned tiny
   model, then the original 20b against chapter 45's recorded CPU vectors,
   greedy fixture prompts, a per-launch CPU repeat and timing.
   Usage: gptoss-metal tiny GENERATED_METAL TINY_MODEL
          gptoss-metal original GENERATED_METAL ORIGINAL_DIR OUT_DIR [REPEAT]
   Prints SKIP and exits 0 without a Metal device; there is no CPU fallback.
   PSIONIC_DEVICE_QUALIFY=1 repeats layers 0 and 1 and the readout on the CPU;
   PSIONIC_DEVICE_PROFILE=1 adds one prefill and one step timed launch by launch. */
#define _POSIX_C_SOURCE 200809L
#include "gptoss-device.h"
#include "gptoss-tiny.h"
#include "metal-host.h"
#include <assert.h>
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#define TRY(x) do{PcStatus code_=(x);if(code_){fprintf(stderr,"%s: platform=%d %s\n",#x,code_,pm_log());exit(2);}}while(0)
static double now(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return t.tv_sec+t.tv_nsec*1e-9;}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_string str(const char *p){return(coax_string){p,(int64_t)strlen(p)};}
static void *alloc(size_t n,size_t width){void *p=calloc(n?n:1,width);assert(p);return p;}
static void *load(const char *path,size_t *n){int fd=open(path,O_RDONLY);if(fd<0){perror(path);exit(2);}struct stat st;assert(!fstat(fd,&st) && st.st_size>0);*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);assert(!close(fd));return p;}
static void save(const char *dir,const char *name,const void *data,size_t n){char path[4096];assert(snprintf(path,sizeof path,"%s/%s",dir,name)>0);FILE *f=fopen(path,"wb");assert(f && fwrite(data,1,n,f)==n);assert(!fclose(f));}
static void compare(const float *got,const float *expected,int64_t n,const char *field){double largest=0;for(int64_t i=0;i<n;i++){double e=fabs((double)got[i]-expected[i])/(1+fabs(expected[i]));assert(isfinite(got[i]));if(e>largest)largest=e;}printf("%s max scaled error %.9g; fixed bound 0.0005\n",field,largest);assert(largest<0.0005);}
static int cancel_prompt(void *context){(*(int*)context)++;return 1;}
static int sort_double(const void *a,const void *b){double x=*(const double*)a,y=*(const double*)b;return (x>y)-(x<y);}

static void tiny(const char *source,size_t source_bytes,const char *file){
    size_t bytes;void *data=load(file,&bytes);coax_t_GFile files[]={{str("tiny.safetensors"),bv(data,(int64_t)bytes)}};coax_t_GProfile p={.shape=tiny_shape,.pins={tiny_artifacts,1},.specs={tiny_specs,33}};PgSession *s=NULL;coax_t_GResult why;
    {PgSession *refused=NULL;coax_t_GResult preflight;coax_t_GFile empty={0};TRY(pg_open(p,(coax_v_t_GFile){&empty,1},source,source_bytes,0,&refused,&preflight));assert(!refused && preflight.status==COAX_GStatus_budget);puts("zero-budget preflight refuses before reading invalid artifact bytes");}
    double begin=now();TRY(pg_open(p,(coax_v_t_GFile){files,1},source,source_bytes,1073741824,&s,&why));assert(s && !why.status);
    printf("tiny backend=Metal admission=%.6f s\n",now()-begin);coax_t_GDPlan plan=pg_plan(s);int64_t control[8];float logits[16],trace[128],weights[4];int64_t ids[4];
    for(int step=0;step<4;step++){TRY(pg_step(s,tiny_tokens[step],step,0,4,1,control));assert(control[0]==0 && control[2]==step+1);TRY(pg_read(s,18,logits,sizeof logits));TRY(pg_read(s,19,trace,sizeof trace));TRY(pg_read(s,20,ids,sizeof ids));TRY(pg_read(s,21,weights,sizeof weights));compare(logits,tiny_logits[step],16,"logits");compare(trace,tiny_trace[step],128,"trace");compare(weights,tiny_weights[step],4,"mixture");assert(!memcmp(ids,tiny_routes[step],sizeof ids));}
    size_t cache_bytes=(size_t)plan.counts.e[16]*4;float *before=alloc(cache_bytes,1),*after=alloc(cache_bytes,1);TRY(pg_read(s,16,before,cache_bytes));float saved[16];memcpy(saved,logits,sizeof saved);
    for(int split=1;split<=4;split++){
        TRY(pg_reset(s));TRY(pg_prefill(s,tiny_tokens,split,0,0,4,control));assert(!control[0] && control[2]==split);
        if(split<4){TRY(pg_prefill(s,tiny_tokens+split,4-split,split,0,4,control));assert(!control[0] && control[2]==4);}
        TRY(pg_read(s,18,logits,sizeof logits));TRY(pg_read(s,19,trace,sizeof trace));TRY(pg_read(s,20,ids,sizeof ids));TRY(pg_read(s,21,weights,sizeof weights));
        compare(logits,tiny_logits[3],16,"batch logits");compare(trace,tiny_trace[3],128,"batch trace");compare(weights,tiny_weights[3],4,"batch mixture");assert(!memcmp(ids,tiny_routes[3],sizeof ids));
        TRY(pg_read(s,16,after,cache_bytes));assert(!memcmp(before,after,cache_bytes));
    }
    int calls=0,interrupted=0;TRY(pg_prefill_poll(s,tiny_tokens,1,4,0,4,control,cancel_prompt,&calls,&interrupted));assert(interrupted && calls==1);
    TRY(pg_read(s,24,control,64));TRY(pg_read(s,16,after,cache_bytes));assert(control[2]==4 && !memcmp(before,after,cache_bytes));
    puts("cancelled prefill returns before the next chunk and preserves committed state");
    puts("batched prefill splits 1/3, 2/2, 3/1 and 4 preserve all serial cache bytes");
    int64_t bad_tokens[]={1,16};assert(pg_prefill(s,bad_tokens,2,4,0,4,control)==PC_ARGUMENT);
    TRY(pg_read(s,24,control,64));assert(!control[0] && control[2]==4);
    int64_t valid_tokens[]={1,2};TRY(pg_prefill(s,valid_tokens,2,2,0,4,control));assert(control[0]==COAX_GStatus_state && control[2]==4);
    TRY(pg_read(s,16,after,cache_bytes));TRY(pg_read(s,18,logits,sizeof logits));assert(!memcmp(before,after,cache_bytes) && !memcmp(saved,logits,sizeof saved));
    puts("invalid prompt IDs and mismatched sequential position preserve public state");
    coax_t_GDParam params[33];assert(!gd_params(p,(coax_v_t_GDParam){params,33}).status);uint16_t infinity=0x7f80;TRY(pg_patch(s,(size_t)params[2].device,&infinity,2));TRY(pg_step(s,1,4,0,4,1,control));assert(control[0]==COAX_GStatus_arithmetic && control[2]==4);
    TRY(pg_read(s,16,after,cache_bytes));TRY(pg_read(s,18,logits,sizeof logits));assert(!memcmp(before,after,cache_bytes) && !memcmp(saved,logits,sizeof saved));
    int64_t more[]={1,2};TRY(pg_prefill(s,more,2,4,0,4,control));assert(control[0]==COAX_GStatus_arithmetic && control[2]==4);
    TRY(pg_read(s,16,after,cache_bytes));TRY(pg_read(s,18,logits,sizeof logits));assert(!memcmp(before,after,cache_bytes) && !memcmp(saved,logits,sizeof saved));
    puts("late serial and batch head failure preserves the full public cache and logits");free(before);free(after);pg_close(s);assert(!munmap(data,bytes));
}

/* One greedy Harmony request with the Coax low-reasoning template; the
   prompt and generated IDs are saved for the CPU repeat. */
static void greedy(PgSession *s,coax_v_u8 table,const char *dir,const char *label,const char *prompt){
    int64_t input[16384],stage[16384],symbols[1024],control[8];uint8_t system[1024],system_work[1024];coax_t_PtResult sys=hm_system_checked(0,bv(system,1024),bv(system_work,1024),0);assert(!sys.status);
    coax_t_HmMessage messages[2]={{.role=COAX_HmRole_system,.body={(const char*)system,sys.written}},{.role=COAX_HmRole_user,.body=str(prompt)}};
    coax_t_HmResult rendered=hm_render_checked(table,(coax_v_t_HmMessage){messages,2},1,iv(input,16384),iv(stage,16384),iv(symbols,1024),0);assert(!rendered.status);int64_t prefix=rendered.written,total=prefix;
    TRY(pg_reset(s));double begin=now();TRY(pg_prefill(s,input,prefix,0,1,256,control));assert(!control[0]);double prefill=now()-begin;input[total++]=control[1];begin=now();
    while(!control[4]){TRY(pg_step(s,-1,control[2],1,256,1,control));assert(!control[0]);input[total++]=control[1];}
    double decode=now()-begin;
    printf("greedy %s prefix=%lld emitted=%lld stop=%lld prefill_seconds=%.6f decode_seconds=%.6f decode_tok_s=%.3f\n",label,(long long)prefix,(long long)(total-prefix),(long long)control[4],prefill,decode,(total-prefix-1)/decode);
    char name[256];snprintf(name,sizeof name,"%s.prompt.i64",label);save(dir,name,input,(size_t)prefix*8);snprintf(name,sizeof name,"%s.generated.i64",label);save(dir,name,input+prefix,(size_t)(total-prefix)*8);
    uint8_t bytes[65536],work[65536],visible[65536];coax_t_HmRecord records[64],draft[64];coax_t_HmResult parsed=hm_parse_checked(table,iv(input,total),0,(coax_v_t_HmRecord){records,64},(coax_v_t_HmRecord){draft,64},bv(bytes,65536),bv(work,65536),0);
    assert(control[4]==1 && !parsed.status);int finals=0;
    for(int64_t i=0;i<parsed.written;i++){coax_t_GResult r=gt_display(records[i],(coax_string){(const char*)bytes,parsed.bytes},bv(visible,65536),0);if(!r.status){printf("final %s: %.*s\n",label,(int)r.written,visible);finals++;}}
    assert(finals==1);fflush(stdout);
}

static void original(const char *source,size_t source_bytes,const char *root,const char *dir,int repeat){
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});assert(!profile.status);coax_t_GFile files[3];void *maps[3];size_t sizes[3];
    for(int i=0;i<3;i++){char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",root,(int)pins[i].name.count,pins[i].name.data)>0);maps[i]=load(path,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    PgSession *s=NULL;coax_t_GResult why;double begin=now();TRY(pg_open(profile,(coax_v_t_GFile){files,3},source,source_bytes,17179869184,&s,&why));if(why.status){fprintf(stderr,"device admission status=%u %s\n",why.status,s?pg_error(s):"");exit(3);}assert(s);coax_t_GDPlan plan=pg_plan(s);
    printf("original 20b admitted/authenticated/copied once in %.6f s; weight bytes=%lld workspace=%lld reserve=%d\n",now()-begin,(long long)plan.weights,(long long)plan.workspace,134217728);fflush(stdout);
    /* Chapter 45's two recorded CPU prefixes. */
    int64_t control[8],tokens[]={200006,173781};float *logits=alloc(201088,4),*trace=alloc(138240,4),*weights=alloc(96,4);int64_t ids[96];
    for(int step=0;step<2;step++){TRY(pg_reset(s));begin=now();TRY(pg_prefill(s,tokens,step+1,0,0,128,control));assert(!control[0]);printf("original step %d %.6f seconds\n",step,now()-begin);
        TRY(pg_read(s,18,logits,201088*4));TRY(pg_read(s,19,trace,138240*4));TRY(pg_read(s,20,ids,sizeof ids));TRY(pg_read(s,21,weights,96*4));char name[64];
        snprintf(name,sizeof name,"step%d.logits.f32",step);save(dir,name,logits,201088*4);snprintf(name,sizeof name,"step%d.trace.f32",step);save(dir,name,trace,138240*4);
        snprintf(name,sizeof name,"step%d.routes.i64",step);save(dir,name,ids,sizeof ids);snprintf(name,sizeof name,"step%d.weights.f32",step);save(dir,name,weights,96*4);fflush(stdout);}
    /* Serial steps and batched prefill agree to the byte across the 128-position window and the chunk boundary. */
    int64_t history[129];for(int i=0;i<129;i++)history[i]=1000+(i*7919)%190000;
    size_t cb=(size_t)plan.counts.e[16]*4;float *serial_cache=alloc(cb,1),*batch_cache=alloc(cb,1),*serial_logits=alloc(201088,4);
    TRY(pg_reset(s));for(int i=0;i<129;i++)TRY(pg_step(s,history[i],i,0,128,0,control));TRY(pg_read(s,16,serial_cache,cb));TRY(pg_read(s,18,serial_logits,201088*4));
    TRY(pg_reset(s));TRY(pg_prefill(s,history,129,0,0,128,control));assert(!control[0] && control[2]==129);TRY(pg_read(s,16,batch_cache,cb));TRY(pg_read(s,18,logits,201088*4));
    assert(!memcmp(serial_cache,batch_cache,cb) && !memcmp(serial_logits,logits,201088*4));puts("original 129-token varied prefix: complete serial/batch cache and logits bitwise equal across window/chunk boundary");free(serial_cache);free(batch_cache);free(serial_logits);
    size_t table_bytes;void *table_data=load("psionic/fixtures/harmony/gptoss.hst",&table_bytes);coax_v_u8 table=bv(table_data,(int64_t)table_bytes);assert(hs_admit(table));
    greedy(s,table,dir,"hi","Say hi.");
    greedy(s,table,dir,"sum","What is 2 + 2?");
    const char *profile_env=getenv("PSIONIC_DEVICE_PROFILE");
    if(profile_env && *profile_env=='1'){
        int64_t corpus[1024];for(int i=0;i<1024;i++)corpus[i]=1000+(i*7919)%190000;
        TRY(pg_reset(s));pg_profile(s,1);TRY(pg_prefill(s,corpus,1024,0,0,128,control));puts("profiled prefill, 1024 tokens (one command buffer per launch):");pg_profile(s,0);
        pg_profile(s,1);for(int i=0;i<16;i++)TRY(pg_step(s,corpus[i],1024+i,0,128,1,control));puts("profiled decode, 16 steps at position 1024:");pg_profile(s,0);
    }
    const char *qualify_env=getenv("PSIONIC_DEVICE_QUALIFY");
    if(qualify_env && *qualify_env=='1'){
        /* A decode step and a prompt chunk, each from the GPU's own state. */
        int64_t checked,exact;double worst,worst_wide;int wk,wwk;
        TRY(pg_reset(s));TRY(pg_prefill(s,history,9,0,0,128,control));assert(!control[0]);
        pg_qualify(s,1);TRY(pg_prefill(s,history+9,8,9,0,128,control));assert(!control[0]);pg_qualify_report(s,&checked,&exact,&worst,&wk,&worst_wide,&wwk);pg_qualify(s,0);
        coax_string a=wk>=0?gd_kernel_name(wk):str("none"),b=wwk>=0?gd_kernel_name(wwk):str("none");
        printf("qualified prompt chunk: %lld launches repeated on the CPU, %lld identical; largest relative difference %.3g in f32 (%.*s), %.3g in f64 against float-float (%.*s)\n",(long long)checked,(long long)exact,worst,(int)a.count,a.data,worst_wide,(int)b.count,b.data);
        pg_qualify(s,1);TRY(pg_step(s,history[17],17,1,128,1,control));assert(!control[0]);pg_qualify_report(s,&checked,&exact,&worst,&wk,&worst_wide,&wwk);pg_qualify(s,0);
        a=wk>=0?gd_kernel_name(wk):str("none");b=wwk>=0?gd_kernel_name(wwk):str("none");
        printf("qualified decode step: %lld launches repeated on the CPU, %lld identical; largest relative difference %.3g in f32 (%.*s), %.3g in f64 against float-float (%.*s)\n",(long long)checked,(long long)exact,worst,(int)a.count,a.data,worst_wide,(int)b.count,b.data);
        fflush(stdout);
    }
    /* Frozen diverse forced IDs measure the core: one warm-up, then REPEAT runs of 1,024 prompt / 128 decode tokens. */
    int64_t corpus[1024];for(int i=0;i<1024;i++)corpus[i]=1000+(i*7919)%190000;
    double *pre=alloc((size_t)repeat,8),*dec=alloc((size_t)repeat,8);
    for(int r=-1;r<repeat;r++){TRY(pg_reset(s));begin=now();TRY(pg_prefill(s,corpus,1024,0,0,128,control));assert(!control[0]);double pref=now()-begin;begin=now();for(int i=0;i<128;i++)TRY(pg_step(s,corpus[i],1024+i,0,128,0,control));double decode=now()-begin;TRY(pg_read(s,24,control,64));assert(!control[0]);
        printf("forced-token batched workload repeat=%d input=1024 output=128 prefill=%.6f prefill_tok_s=%.1f decode=%.6f tok_s=%.3f\n",r,pref,1024/pref,decode,128/decode);fflush(stdout);if(r>=0){pre[r]=pref;dec[r]=decode;}}
    if(repeat>0){qsort(pre,(size_t)repeat,8,sort_double);qsort(dec,(size_t)repeat,8,sort_double);printf("median prefill %.6f s (%.1f tok/s), median decode %.6f s (%.3f tok/s) over %d runs\n",pre[repeat/2],1024/pre[repeat/2],dec[repeat/2],128/dec[repeat/2],repeat);}
    PmDeviceInfo info;pm_memory(&info);printf("Metal memory allocated by this process: %.2f GiB of a %.2f GiB working set\n",info.allocated_bytes/1073741824.0,info.working_set_bytes/1073741824.0);
    pg_close(s);free(logits);free(trace);free(weights);free(pre);free(dec);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);munmap(table_data,table_bytes);
}
int main(int argc,char **argv){
    if(argc<4){fprintf(stderr,"usage: gptoss-metal tiny GENERATED_METAL TINY_MODEL | original GENERATED_METAL ORIGINAL_DIR OUT_DIR [REPEAT]\n");return 2;}
    PmDeviceInfo info;PmStatus code=pm_open(&info);if(code==PM_ABSENT){puts("SKIP: no Metal device; no CPU fallback");return 0;}if(code){fprintf(stderr,"%s\n",pm_log());return 2;}
    printf("device %s (Apple family %d) unified=%d working set %.1f GiB, largest buffer %.1f GiB\n",info.name,info.apple_family,info.unified_memory,info.working_set_bytes/1073741824.0,info.max_buffer_bytes/1073741824.0);
    size_t n;const char *source=load(argv[2],&n);
    if(!strcmp(argv[1],"tiny"))tiny(source,n,argv[3]);
    else{assert(!strcmp(argv[1],"original") && argc>=5);original(source,n,argv[3],argv[4],argc>5?atoi(argv[5]):5);}
    return 0;
}
