/* Chapter 175: continuous batching of the original gpt-oss-20b on one device.
   Firm reason: 5 (thin harness: the CUDA driver, clocks and file mapping are host effects;
   every schedule, region, program, admission and verdict comes from Coax).
   Chapter 175's scheduler (cb_step) gives each step its rows; Coax binds
   regions (cbd_bind), lists segments and IDs (cbd_segments), admits the step
   (cbd_admit) and builds its programs (cbd_head, cbd_layer, cbd_tail). This
   harness uploads the IDs, executes the programs, reads control words and
   clocks, and prints what Coax decided.
   Usage: gptoss_batch CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS [HI_WORD LIST_WORD] */
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
#define TRY(x) do{PcStatus code_=(x);if(code_){fprintf(stderr,"%s: platform=%d %s %s\n",#x,code_,pc_log(),s?pg_error(s):"");exit(2);}}while(0)
#define REQS 8
#define OUTMAX 512
static int64_t ns(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return (int64_t)t.tv_sec*1000000000+t.tv_nsec;}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_string str(const char *p){return(coax_string){p,(int64_t)strlen(p)};}
static void *load(const char *path,size_t *n){int fd=open(path,O_RDONLY);assert(fd>=0);struct stat st;assert(!fstat(fd,&st) && st.st_size>0);*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);assert(!close(fd));return p;}
static double load1(void){double l=-1;FILE *f=fopen("/proc/loadavg","r");if(f){if(fscanf(f,"%lf",&l)!=1)l=-1;fclose(f);}return l;}
static PgSession *s;static coax_t_GShape shape;static coax_t_GDPlan plan;static coax_t_GDParam params[512];static int64_t nparams;
static coax_t_GDOp *ops;static coax_t_GDArg *args;
static int64_t prompts[REQS*1024],first[REQS],plen[REQS],limit[REQS];
static int64_t alone[REQS][OUTMAX],alone_n[REQS];
static int64_t render(coax_v_u8 table,const char *user,int64_t *input){
    uint8_t system[1024],work[1024];coax_t_PtResult sys=hm_system_checked(0,bv(system,1024),bv(work,1024),0);assert(!sys.status);
    coax_t_HmMessage m[2]={{.role=COAX_HmRole_system,.body={(const char*)system,sys.written}},{.role=COAX_HmRole_user,.body=str(user)}};
    static int64_t stage[16384],symbols[1024],input2[16384];coax_t_HmResult r=hm_render_checked(table,(coax_v_t_HmMessage){m,2},1,iv(input2,16384),iv(stage,16384),iv(symbols,1024),0);assert(!r.status && r.written<=1024);
    memcpy(input,input2,(size_t)r.written*8);return r.written;
}
/* Chapter 70's path, one request alone: prefill, then one replayed step a token. */
static void solo(int i){
    int64_t control[8];TRY(pg_reset(s));TRY(pg_prefill(s,prompts+first[i],plen[i],0,1,limit[i],control));assert(!control[0]);alone_n[i]=0;alone[i][alone_n[i]++]=control[1];
    while(!control[4]){TRY(pg_step(s,-1,control[2],1,limit[i],1,control));assert(!control[0] && alone_n[i]<OUTMAX);alone[i][alone_n[i]++]=control[1];}
}
static void run_program(coax_t_GDProgram p,const char *what){if(p.status){fprintf(stderr,"Coax refused the %s program: status=%u\n",what,p.status);exit(3);}TRY(pg_run(s,ops,args,p));}
typedef struct {int64_t ids[REQS][OUTMAX],n[REQS],first_ns[REQS],finish_ns[REQS],steps,rows,ns;int same;} Batch;
/* One batched run of every request at concurrency c. */
static void batched(int c,Batch *b){
    coax_t_CbConfig cfg={.slots=c,.tokens=plan.batch,.kv=c*shape.capacity,.state=0,.clients=4,.steps=8192};for(int k=0;k<4;k++)cfg.weight.e[k]=1;
    coax_t_CbRequest reqs[REQS];for(int i=0;i<REQS;i++)reqs[i]=(coax_t_CbRequest){.client=i%4,.arrive=0,.prompt=plen[i],.output=alone_n[i]};
    coax_t_CbState st=cb_start(cfg,(coax_v_t_CbRequest){reqs,REQS});if(st.status){fprintf(stderr,"scheduler refused: %u\n",st.status);exit(3);}
    int64_t region[REQS],last[REQS],ids[1024],zero[8]={0};coax_t_CbdSeg segs[16];memset(b,0,sizeof *b);for(int i=0;i<REQS;i++){region[i]=-1;last[i]=0;}
    int64_t t0=ns();
    while(st.done<st.n){
        st=cb_step(st);if(st.nrows==0)continue;
        if(!cbd_bind(st,iv(region,REQS),cbd_regions(plan))){fputs("no free region\n",stderr);exit(3);}
        int64_t n=cbd_segments(st,iv(region,REQS),iv(prompts,REQS*1024),iv(first,REQS),iv(last,REQS),(coax_v_t_CbdSeg){segs,16},iv(ids,1024));
        coax_v_t_CbdSeg sv={segs,st.nrows};coax_t_CbdStep admitted=cbd_admit(shape,plan,sv,iv(ids,n));
        if(n<1 || admitted.status){fprintf(stderr,"step %lld refused: rows=%lld status=%u\n",(long long)st.step,(long long)n,admitted.status);exit(3);}
        TRY(pg_write(s,26,ids,(size_t)n*8));TRY(pg_write(s,24,zero,64));
        coax_v_t_GDParam pv={params,nparams};coax_v_t_GDOp ov={ops,plan.max_ops};coax_v_t_GDArg av={args,plan.max_args};
        run_program(cbd_head(shape,plan,pv,sv,n,ov,av),"head");
        for(int64_t l=0;l<shape.layers;l++)run_program(cbd_layer(shape,plan,pv,sv,n,l,ov,av),"layer");
        run_program(cbd_tail(shape,plan,pv,sv,ov,av),"tail");
        int64_t control[8];TRY(pg_read(s,24,control,64));if(control[0]){fprintf(stderr,"step %lld: arithmetic flags, control %lld\n",(long long)st.step,(long long)control[0]);exit(3);}
        int64_t now=ns()-t0;b->steps++;b->rows+=n;
        for(int64_t k=0;k<st.nrows;k++){coax_t_CbRow r=st.rows.e[k];if(!r.emit)continue;int64_t i=r.req;
            TRY(pg_fetch(s,1,(size_t)cbd_control_offset(plan,region[i]),control,64));
            if(control[0] || control[2]!=r.at+r.count){fprintf(stderr,"request %lld: control %lld position %lld, expected %lld\n",(long long)i,(long long)control[0],(long long)control[2],(long long)(r.at+r.count));exit(3);}
            assert(b->n[i]<OUTMAX);if(b->n[i]==0)b->first_ns[i]=now;b->ids[i][b->n[i]++]=control[1];last[i]=control[1];}
        for(int i=0;i<REQS;i++)if(st.fate.e[i].finish==st.step-1)b->finish_ns[i]=now;
    }
    b->ns=ns()-t0;if(st.status){fprintf(stderr,"scheduler ended with status %u\n",st.status);exit(3);}
    b->same=1;for(int i=0;i<REQS;i++)b->same=b->same && cbd_same_tokens(iv(b->ids[i],b->n[i]),iv(alone[i],alone_n[i]));
}
int main(int argc,char **argv){
    if(argc!=7 && argc!=9){fputs("usage: gptoss_batch CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS [HI_WORD LIST_WORD]\n",stderr);return 2;}
    int runs=atoi(argv[6]);if(runs<5 || runs>64){fputs("RUNS must be 5 to 64\n",stderr);return 2;}
    PcDeviceInfo info;PcStatus code=pc_open(&info);if(code==PC_ABSENT){puts("SKIP: no CUDA device; no CPU fallback");return 77;}TRY(code);
    char device[256];snprintf(device,sizeof device,"%s sm_%d%d %zu bytes",info.name,info.major,info.minor,info.total_bytes);printf("device: %s\n",device);
    size_t source_bytes;const char *source=load(argv[1],&source_bytes);
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});assert(!profile.status);
    coax_t_GFile files[3];void *maps[3];size_t sizes[3];
    for(int i=0;i<3;i++){char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[2],(int)pins[i].name.count,pins[i].name.data)>0);maps[i]=load(path,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    coax_t_GResult why;int64_t begin=ns();TRY(pg_open(profile,(coax_v_t_GFile){files,3},source,source_bytes,17179869184,&s,&why));
    if(why.status){fprintf(stderr,"device admission status=%u\n",why.status);return 3;}assert(s);
    printf("original 20b authenticated and resident in %.3f s\n",(ns()-begin)*1e-9);
    shape=pg_shape(s);why=gd_params(profile,(coax_v_t_GDParam){params,512});assert(!why.status);nparams=why.written;
    plan=cbd_with_regions(pg_plan(s),shape,REQS);if(plan.status){fprintf(stderr,"regions refused: %u\n",plan.status);return 3;}TRY(pg_adopt(s,plan));
    ops=calloc((size_t)plan.max_ops,sizeof *ops);args=calloc((size_t)plan.max_args,sizeof *args);assert(ops && args);
    size_t free_bytes,total_bytes;TRY(pc_memory(&free_bytes,&total_bytes));
    printf("regions: %lld of %lld bytes; batch %lld rows; device bytes in use=%zu of %zu\n",(long long)cbd_regions(plan),(long long)cbd_region_bytes(plan),(long long)plan.batch,total_bytes-free_bytes,total_bytes);
    /* The workload: the two fixture prompts (limit 256, as chapter 71 checks
       them) and six prompts of chapter 71's IDs, 96 to 416 long (limit 64). */
    size_t table_bytes;void *table_data=load("psionic/fixtures/harmony/gptoss.hst",&table_bytes);coax_v_u8 table=bv(table_data,(int64_t)table_bytes);assert(hs_admit(table));
    const char *fixture[2]={"Say hi.","List the integers from 1 to 100, separated by commas."};
    for(int i=0;i<REQS;i++){first[i]=i*1024;
        if(i<2){plen[i]=render(table,fixture[i],prompts+first[i]);limit[i]=256;}
        else {plen[i]=32+64*(i-1);limit[i]=64;for(int64_t t=0;t<plen[i];t++)prompts[first[i]+t]=tg_id((t+97*i)%1024);}}
    /* Alone first: the reference each batched request must equal. */
    begin=ns();int64_t total=0;
    for(int i=0;i<REQS;i++){solo(i);total+=alone_n[i];printf("alone request=%d prompt=%lld ids=%lld word=%016llx\n",i,(long long)plen[i],(long long)alone_n[i],(unsigned long long)tg_word((coax_string){(const char*)alone[i],alone_n[i]*8}));}
    printf("alone: %lld output IDs in %.3f s\n",(long long)total,(ns()-begin)*1e-9);
    int fixtures_ok=1;
    if(argc==9){for(int p=0;p<2;p++)fixtures_ok=fixtures_ok && tg_word((coax_string){(const char*)alone[p],alone_n[p]*8})==strtoull(argv[7+p],NULL,16);
        printf("fixture prompts alone %s\n",fixtures_ok?"equal the reference words":"DIFFER from the reference words");}
    int levels[3]={1,4,8};static Batch b;int all_same=1;
    for(int c=0;c<3;c++){
        int64_t run_ns[64],agg[64];coax_t_CbdRun reports[64];
        for(int r=-1;r<runs;r++){
            double l=load1();batched(levels[c],&b);all_same=all_same && b.same;
            int64_t lat[REQS],ft[REQS];for(int i=0;i<REQS;i++){lat[i]=b.finish_ns[i];ft[i]=b.first_ns[i];}
            coax_t_CbdRun rep=cbd_run_report(iv(lat,REQS),iv(ft,REQS),total,b.ns);
            printf("concurrency=%d run=%d steps=%lld rows=%lld ns=%lld tok_s=%.3f latency50_ns=%lld latency95_ns=%lld first50_ns=%lld first95_ns=%lld same=%s load1=%.2f\n",levels[c],r,(long long)b.steps,(long long)b.rows,(long long)rep.ns,total*1e9/(double)rep.ns,
                (long long)rep.latency50,(long long)rep.latency95,(long long)rep.first50,(long long)rep.first95,b.same?"yes":"NO",l);fflush(stdout);
            if(!b.same)for(int i=0;i<REQS;i++)if(!cbd_same_tokens(iv(b.ids[i],b.n[i]),iv(alone[i],alone_n[i])))printf("  request %d differs: %lld IDs against %lld alone\n",i,(long long)b.n[i],(long long)alone_n[i]);
            if(r>=0){run_ns[r]=rep.ns;reports[r]=rep;agg[r]=rep.latency50;}
        }
        coax_t_CmStats st=cm_welford(iv(run_ns,runs));double m50=0,m95=0,f50=0,f95=0;for(int r=0;r<runs;r++){m50+=reports[r].latency50;m95+=reports[r].latency95;f50+=reports[r].first50;f95+=reports[r].first95;}
        printf("concurrency %d: n=%lld mean_ns=%.0f sd_ns=%.0f tok_s=%.3f latency P50 %.3f s P95 %.3f s first token P50 %.3f s P95 %.3f s\n",levels[c],(long long)st.n,st.mean,st.sd,total*1e9/st.mean,m50/runs*1e-9,m95/runs*1e-9,f50/runs*1e-9,f95/runs*1e-9);
        (void)agg;
        coax_t_TgBind bind={.build=tg_word(str(argv[3])),.commit=tg_word(str(argv[4])),.machine=tg_word(str(argv[5])),.device=tg_word(str(device)),.warmup=1,
            .batch={.concurrency=levels[c],.requests=REQS,.runs=runs,.same=all_same && fixtures_ok && argc==9,.build=tg_word(str(argv[3])),.machine=tg_word(str(argv[5]))}};
        coax_t_TgWork work=tg_work();work.concurrency=levels[c];coax_string why2=tg_reason_name(tg_concurrency(work,bind));
        printf("chapter 71's gate at concurrency %d: %.*s\n",levels[c],(int)why2.count,why2.data);
        static int64_t base[64];static int nbase;if(c==0){memcpy(base,run_ns,sizeof base);nbase=runs;}
        else {coax_t_CmSide side={.work={.file=tg_word(str("gpt-oss-20b@6cee5e81ee83917806bbde320786a8fb61efebee")),.weights=COAX_CmFormat_mxfp4,.activations=COAX_CmFormat_f32,.accumulate=COAX_CmFormat_f32,.rows=REQS,.cols=1,.tokens=total},.engine=tg_word(str("cbd-gptoss20b-v1")),.build=tg_word(str(argv[3])),.machine=tg_word(str(argv[5])),.warmup=1};
            coax_t_CmCall call=cm_compare(side,st,side,cm_welford(iv(base,nbase)));static const char *verdicts[5]={"refused","faster","slower","the same within two standard errors","too noisy to rank"};const char *v=verdicts[call.verdict];
            printf("concurrency %d against 1 (chapter 34): ratio %.4f, se %.0f ns: %s\n",levels[c],call.ratio,call.se,v);}
    }
    printf("batched IDs %s\n",all_same?"equal every request's IDs alone in every run":"DIFFER from a request's IDs alone");
    printf("bound: build=%s commit=%s machine=%s\n",argv[3],argv[4],argv[5]);
    pg_close(s);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);munmap(table_data,table_bytes);munmap((void*)source,source_bytes);
    return all_same && fixtures_ok?0:1;
}
