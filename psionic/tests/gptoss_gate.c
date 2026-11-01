/* Chapter 71: the throughput gate on the original gpt-oss-20b, opt-in.
   Coax owns the workload IDs (tg_id), the identity words (tg_word) and the
   verdict (tg_decide); this harness only opens the resident session, feeds
   the IDs, reads a monotonic clock and prints what Coax decided.
   Usage: gptoss_gate CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS [HI_WORD LIST_WORD [MMA_CUDA SLICES]]
   HI_WORD and LIST_WORD are the reference path's greedy-output words (hex) on
   the two fixture prompts; without them the outputs are printed, not checked.
   MMA_CUDA (`coax cuda` of gptoss/device-mma.coax) and SLICES turn on the
   tensor-core prefill expert products through pg_mma, which Coax admits. */
#define _POSIX_C_SOURCE 200809L
#include "gptoss-device.h"
#include <assert.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#define TRY(x) do{PcStatus code_=(x);if(code_){fprintf(stderr,"%s: platform=%d %s\n",#x,code_,pc_log());exit(2);}}while(0)
static int64_t ns(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return (int64_t)t.tv_sec*1000000000+t.tv_nsec;}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_string str(const char *p){return(coax_string){p,(int64_t)strlen(p)};}
static void *load(const char *path,size_t *n){int fd=open(path,O_RDONLY);assert(fd>=0);struct stat st;assert(!fstat(fd,&st) && st.st_size>0);*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);assert(!close(fd));return p;}
static double load1(void){double l=-1;FILE *f=fopen("/proc/loadavg","r");if(f){if(fscanf(f,"%lf",&l)!=1)l=-1;fclose(f);}return l;}
/* One fixture prompt, greedy to its stop or the limit. Returns the word of the
   generated IDs (8 little-endian bytes each, as tg_ids_word hashes). */
static uint64_t greedy(PgSession *s,coax_v_u8 table,const char *user,int64_t limit,int64_t *count){
    uint8_t system[1024],work[1024];coax_t_PtResult sys=hm_system_checked(0,bv(system,1024),bv(work,1024),0);assert(!sys.status);
    coax_t_HmMessage m[2]={{.role=COAX_HmRole_system,.body={(const char*)system,sys.written}},{.role=COAX_HmRole_user,.body=str(user)}};
    static int64_t input[16384],stage[16384],symbols[1024];coax_t_HmResult r=hm_render_checked(table,(coax_v_t_HmMessage){m,2},1,iv(input,16384),iv(stage,16384),iv(symbols,1024),0);assert(!r.status);
    int64_t control[8],out[512],n=0;TRY(pg_reset(s));TRY(pg_prefill(s,input,r.written,0,1,limit,control));assert(!control[0]);out[n++]=control[1];
    while(!control[4]){TRY(pg_step(s,-1,control[2],1,limit,1,control));assert(!control[0] && n<512);out[n++]=control[1];}
    *count=n;return tg_word((coax_string){(const char*)out,n*8});
}
int main(int argc,char **argv){
    if(argc!=7 && argc!=9 && argc!=11){fputs("usage: gptoss_gate CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS [HI_WORD LIST_WORD [MMA_CUDA SLICES]]\n",stderr);return 2;}
    int runs=atoi(argv[6]);if(runs<1 || runs>64){fputs("RUNS must be 1 to 64\n",stderr);return 2;}
    PcDeviceInfo info;PcStatus code=pc_open(&info);if(code==PC_ABSENT){puts("SKIP: no CUDA device; no CPU fallback");return 77;}TRY(code);
    char device[256];snprintf(device,sizeof device,"%s sm_%d%d %zu bytes",info.name,info.major,info.minor,info.total_bytes);printf("device: %s\n",device);
    size_t source_bytes;const char *source=load(argv[1],&source_bytes);
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});assert(!profile.status);
    coax_t_GFile files[3];void *maps[3];size_t sizes[3];
    for(int i=0;i<3;i++){char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[2],(int)pins[i].name.count,pins[i].name.data)>0);maps[i]=load(path,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    PgSession *s=NULL;coax_t_GResult why;int64_t begin=ns();TRY(pg_open(profile,(coax_v_t_GFile){files,3},source,source_bytes,17179869184,&s,&why));
    if(why.status){fprintf(stderr,"device admission status=%u\n",why.status);return 3;}assert(s);
    printf("original 20b authenticated and resident in %.3f s\n",(ns()-begin)*1e-9);fflush(stdout);
    if(argc==11){size_t mma_bytes;const char *mma=load(argv[9],&mma_bytes);coax_t_GResult admitted;TRY(pg_mma(s,mma,mma_bytes,atoll(argv[10]),&admitted));
        if(admitted.status){fprintf(stderr,"tensor-core products refused: status=%u\n",admitted.status);return 3;}printf("prefill expert products: MXFP4-BF16-MMA-v1, %lld slices\n",(long long)pg_plan(s).mma);munmap((void*)mma,mma_bytes);}
    else puts("prefill expert products: gpt-oss-cuda-f32-v1");
    /* Greedy outputs on the fixture prompts, before and after the timed runs. */
    size_t table_bytes;void *table_data=load("psionic/fixtures/harmony/gptoss.hst",&table_bytes);coax_v_u8 table=bv(table_data,(int64_t)table_bytes);assert(hs_admit(table));
    const char *prompts[2]={"Say hi.","List the integers from 1 to 100, separated by commas."};int64_t limits[2]={256,256};uint64_t words[2][2];int64_t counts[2][2];
    for(int p=0;p<2;p++){words[0][p]=greedy(s,table,prompts[p],limits[p],&counts[0][p]);printf("greedy prompt=%d ids=%lld word=%016llx\n",p,(long long)counts[0][p],(unsigned long long)words[0][p]);fflush(stdout);}
    /* The workload: Coax's IDs, one untimed warm-up, then RUNS timed runs. */
    int64_t ids[1152];for(int i=0;i<1152;i++)ids[i]=tg_id(i);
    coax_t_TgWork work={.model=tg_word(str("gpt-oss-20b@6cee5e81ee83917806bbde320786a8fb61efebee")),.corpus=tg_word((coax_string){(const char*)ids,1152*8}),.input=1024,.output=128,.context=profile.shape.capacity,.concurrency=1};
    int64_t prefill[64],decode[64],control[8];
    for(int r=-1;r<runs;r++){
        double load=load1();TRY(pg_reset(s));begin=ns();TRY(pg_prefill(s,ids,1024,0,0,128,control));assert(!control[0]);int64_t a=ns()-begin;
        begin=ns();for(int i=0;i<128;i++)TRY(pg_step(s,ids[1024+i],1024+i,0,128,0,control));TRY(pg_wait(s));int64_t b=ns()-begin;TRY(pg_read(s,24,control,64));assert(!control[0] && control[2]==1152);
        printf("run=%d prefill_ns=%lld decode_ns=%lld decode_tok_s=%.3f load1=%.2f\n",r,(long long)a,(long long)b,128e9/(double)b,load);fflush(stdout);
        if(r>=0){prefill[r]=a;decode[r]=b;}
    }
#ifdef PSIONIC_DEVICE_TEST
    /* Test builds only: one more prefill with a device-timed launch per kernel,
       to say where the time goes. Never part of the gate's runs. */
    TRY(pg_reset(s));pg_profile(s,1);TRY(pg_prefill(s,ids,1024,0,0,128,control));assert(!control[0]);pg_profile(s,0);fflush(stdout);
#endif
    for(int p=0;p<2;p++){words[1][p]=greedy(s,table,prompts[p],limits[p],&counts[1][p]);printf("greedy again prompt=%d ids=%lld word=%016llx\n",p,(long long)counts[1][p],(unsigned long long)words[1][p]);}
    int same=words[0][0]==words[1][0] && words[0][1]==words[1][1];
    if(argc>=9)same=same && words[0][0]==strtoull(argv[7],NULL,16) && words[0][1]==strtoull(argv[8],NULL,16);
    printf("greedy outputs %s\n",argc>=9?(same?"equal the reference words":"DIFFER from the reference words"):(same?"repeat exactly (no reference words given)":"DIFFER between repeats"));
    size_t free_bytes,total_bytes;TRY(pc_memory(&free_bytes,&total_bytes));printf("device bytes in use=%zu of %zu\n",total_bytes-free_bytes,total_bytes);
    coax_t_TgBind bind={.build=tg_word(str(argv[3])),.commit=tg_word(str(argv[4])),.machine=tg_word(str(argv[5])),.device=tg_word(str(device)),.warmup=1};
    printf("bound: build=%s commit=%s machine=%s\n",argv[3],argv[4],argv[5]);
    coax_t_TgGate g=tg_decide(work,bind,argc>=9 && same,iv(prefill,runs),iv(decode,runs));
    coax_t_TgGoal goals[2]={g.prefill,g.decode};const char *names[2]={"prefill","decode"};
    for(int i=0;i<2;i++){coax_string v=tg_verdict_name(goals[i].verdict),why2=tg_reason_name(goals[i].reason);printf("goal %s: n=%lld mean_ns=%.0f sd_ns=%.0f se_ns=%.0f limit_ns=%lld verdict=%.*s (%.*s)\n",names[i],(long long)goals[i].stats.n,goals[i].stats.mean,goals[i].stats.sd,goals[i].se,(long long)goals[i].limit,(int)v.count,v.data,(int)why2.count,why2.data);}
    coax_string v=tg_verdict_name(g.verdict),why3=tg_reason_name(g.reason);printf("gate: %.*s (%.*s)\n",(int)v.count,v.data,(int)why3.count,why3.data);
    pg_close(s);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);munmap(table_data,table_bytes);munmap((void*)source,source_bytes);
    return g.verdict==COAX_TgVerdict_pass?0:g.verdict==COAX_TgVerdict_fail?1:g.verdict==COAX_TgVerdict_noisy?4:5;
}
