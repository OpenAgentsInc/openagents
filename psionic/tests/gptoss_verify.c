/* Chapter 76: verify a draft block in one batched device pass.
   Coax drafts (gv_draft), builds every program (gv_save, gv_readout,
   gv_restore), decides what is committed (gv_more, gv_commit), sets the
   control words (gv_control) and decides whether speculation is on
   (sg_decide). This harness opens the session, executes those programs,
   reads a monotonic clock and prints what Coax decided.
   Usage:
     gptoss_verify cpu TINY_MODEL
       The tiny owned model on the CPU (the same Coax kernels compiled to C):
       oracle, wrong and n-gram drafts; greedy parity and cache equality.
     gptoss_verify cuda CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS K
       The original gpt-oss-20b: parity and cache checks on the two fixture
       prompts, then one warm-up and RUNS timed runs of each path. */
#define _POSIX_C_SOURCE 200809L
#include "gptoss-device.h"
#ifdef PSIONIC_DEVICE_TEST
#include "gptoss-tiny.h"
#endif
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
#define MAXGEN 1024
#define MAXCHECK 6
static int64_t ns(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return (int64_t)t.tv_sec*1000000000+t.tv_nsec;}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_string str(const char *p){return(coax_string){p,(int64_t)strlen(p)};}
static void *load(const char *path,size_t *n){int fd=open(path,O_RDONLY);assert(fd>=0);struct stat st;assert(!fstat(fd,&st) && st.st_size>0);*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);assert(!close(fd));return p;}
static double load1(void){double l=-1;FILE *f=fopen("/proc/loadavg","r");if(f){if(fscanf(f,"%lf",&l)!=1)l=-1;fclose(f);}return l;}

typedef enum {DRAFT_NGRAM,DRAFT_ORACLE,DRAFT_WRONG} Drafter;
typedef struct {int64_t ids[MAXGEN];int64_t n,blocks,drafted,accepted,rows,plain;} Gen;
/* Cache bytes recorded after a rollback, at a committed position, to be
   compared with the target-only run's cache at the same position. */
typedef struct {int64_t count,position[MAXCHECK],checked;float *cache[MAXCHECK];size_t bytes;int64_t from;} Checks;

typedef struct {PgSession *s;coax_t_GShape shape;coax_t_GDParam params[3+15*36];int64_t nparams;coax_t_GDOp *ops;coax_t_GDArg *args;int64_t k,width;} Ctx;
static void run(Ctx *c,coax_t_GDProgram p){if(p.status){fprintf(stderr,"Coax program refused: %u\n",p.status);exit(3);}TRY(pg_run(c->s,c->ops,c->args,p));}
static coax_v_t_GDOp opv(Ctx *c){return(coax_v_t_GDOp){c->ops,pg_plan(c->s).max_ops};}
static coax_v_t_GDArg argv_(Ctx *c){return(coax_v_t_GDArg){c->args,pg_plan(c->s).max_args};}

static void record(Ctx *c,Checks *k,int64_t position){
    if(!k || k->count>=MAXCHECK || position<k->from)return;
    k->cache[k->count]=malloc(k->bytes);assert(k->cache[k->count]);TRY(pg_read(c->s,16,k->cache[k->count],k->bytes));k->position[k->count++]=position;
}
static void compare(Ctx *c,Checks *k,int64_t position,float *scratch){
    if(!k)return;for(int64_t i=0;i<k->count;i++)if(k->position[i]==position){TRY(pg_read(c->s,16,scratch,k->bytes));
        if(memcmp(scratch,k->cache[i],k->bytes)){printf("cache after rollback at position %lld DIFFERS from the target-only cache\n",(long long)position);exit(1);}k->checked++;}
}
/* Target-only greedy: one prefill, then one step per ID. */
static int64_t baseline(Ctx *c,const int64_t *prompt,int64_t len,int64_t limit,Gen *g,Checks *k,float *scratch){
    int64_t control[8];memset(g,0,sizeof *g);TRY(pg_reset(c->s));TRY(pg_prefill(c->s,prompt,len,0,1,limit,control));assert(!control[0]);g->ids[g->n++]=control[1];
    int64_t begin=ns();
    while(!control[4]){compare(c,k,control[2],scratch);TRY(pg_step(c->s,-1,control[2],1,limit,1,control));assert(!control[0] && g->n<MAXGEN);g->ids[g->n++]=control[1];}
    return ns()-begin;
}
/* Speculative greedy: draft, save, verify the block in one prefill, read
   rows until the first mismatch, commit, restore the speculative rows. */
static int64_t speculate(Ctx *c,const int64_t *prompt,int64_t len,int64_t limit,Drafter drafter,const Gen *reference,Gen *g,Checks *k){
    int64_t control[8];memset(g,0,sizeof *g);TRY(pg_reset(c->s));TRY(pg_prefill(c->s,prompt,len,0,1,limit,control));assert(!control[0]);g->ids[g->n++]=control[1];
    static int64_t history[MAXGEN+4096],scratch[MAXGEN+4096+32];memcpy(history,prompt,(size_t)len*8);
    int64_t begin=ns();
    while(!control[4]){
        int64_t position=control[2],block[32],picks[32],out[32],*draft=block+1,room=gv_room(c->shape,position,c->k),drafted=0;block[0]=control[1];
        if(drafter==DRAFT_NGRAM && room>0){for(int64_t i=0;i<g->n;i++)history[len+i]=g->ids[i];drafted=gv_draft(iv(history,len+g->n),c->width,room,iv(scratch,len+g->n+room),iv(draft,room));assert(drafted>=0);}
        else if(room>0){/* Test drafters: the target-only answer, or one off it. */
            for(;drafted<room && g->n+drafted<reference->n;drafted++){int64_t id=reference->ids[g->n+drafted];draft[drafted]=drafter==DRAFT_ORACLE?id:(id+1)%c->shape.vocab;}}
        if(drafted==0){TRY(pg_step(c->s,-1,position,1,limit,1,control));assert(!control[0] && g->n<MAXGEN);g->ids[g->n++]=control[1];g->plain++;continue;}
        run(c,gv_save(c->shape,pg_plan(c->s),position,drafted+1,opv(c),argv_(c)));
        TRY(pg_prefill(c->s,block,drafted+1,position,0,limit,control));assert(!control[0] && control[2]==position+drafted+1);
        int64_t rows=0;
        for(;;){run(c,gv_readout(c->shape,pg_plan(c->s),(coax_v_t_GDParam){c->params,c->nparams},rows,drafted+1,limit,opv(c),argv_(c)));
            TRY(pg_read(c->s,24,control,64));assert(!control[0]);picks[rows]=control[1];
            if(!gv_more(iv(draft,drafted),iv(picks,rows+1),rows++))break;}
        coax_t_GVBlock b=gv_commit(c->shape.vocab,iv(draft,drafted),iv(picks,rows),rows,g->n,limit,iv(out,32));if(b.status){fprintf(stderr,"block refused: %u\n",b.status);exit(3);}
        run(c,gv_restore(c->shape,pg_plan(c->s),position,drafted+1,b.kept,opv(c),argv_(c)));
        TRY(pg_read(c->s,24,control,64));if(gv_control(iv(control,8),position,b,g->n)){fprintf(stderr,"control refused\n");exit(3);}TRY(pg_write(c->s,24,control,64));
        assert(g->n+b.written<=MAXGEN);for(int64_t i=0;i<b.written;i++)g->ids[g->n++]=out[i];
        g->blocks++;g->drafted+=drafted;g->accepted+=b.accepted;g->rows+=rows;
        if(b.kept<drafted+1 && !control[4])record(c,k,position+b.kept);
    }
    return ns()-begin;
}
static int same(const Gen *a,const Gen *b){return a->n==b->n && !memcmp(a->ids,b->ids,(size_t)a->n*8);}
static void report(const char *name,const Gen *base,const Gen *g,int64_t base_ns,int64_t spec_ns){
    printf("%s: ids=%lld same=%s blocks=%lld drafted=%lld accepted=%lld rows=%lld plain_steps=%lld base_ns=%lld spec_ns=%lld\n",name,(long long)g->n,same(base,g)?"yes":"NO",
        (long long)g->blocks,(long long)g->drafted,(long long)g->accepted,(long long)g->rows,(long long)g->plain,(long long)base_ns,(long long)spec_ns);fflush(stdout);
}
static Ctx open_ctx(PgSession *s,coax_t_GProfile profile,int64_t k,int64_t width){
    Ctx c={.s=s,.shape=pg_shape(s),.k=k,.width=width};c.nparams=3+15*c.shape.layers;assert(!gd_params(profile,(coax_v_t_GDParam){c.params,c.nparams}).status);
    coax_t_GDPlan undo=gv_with_undo(pg_plan(s),c.shape,k);if(undo.status){printf("speculation refused for k=%lld: status=%u\n",(long long)k,undo.status);exit(3);}TRY(pg_adopt(s,undo));
    c.ops=calloc((size_t)undo.max_ops,sizeof *c.ops);c.args=calloc((size_t)undo.max_args,sizeof *c.args);assert(c.ops && c.args);return c;
}
#ifdef PSIONIC_DEVICE_TEST
static int tiny(const char *file){
    size_t bytes;void *data=load(file,&bytes);coax_t_GFile files[]={{str("tiny.safetensors"),bv(data,(int64_t)bytes)}};coax_t_GProfile p={.shape=tiny_shape,.pins={tiny_artifacts,1},.specs={tiny_specs,33}};
    PgSession *s=NULL;coax_t_GResult why;TRY(pg_open_cpu(p,(coax_v_t_GFile){files,1},&s,&why));assert(s && !why.status);
    coax_t_GDPlan plain=pg_plan(s);printf("tiny model, CPU: window %lld, so k=2 is %s\n",(long long)tiny_shape.window,gv_with_undo(plain,tiny_shape,2).status==COAX_GStatus_shape?"refused (k + 1 > window)":"admitted");
    Ctx c=open_ctx(s,p,1,2);size_t cb=(size_t)pg_plan(s).counts.e[16]*4;float *scratch=malloc(cb);assert(scratch);
    const char *names[3]={"n-gram","oracle","wrong"};int64_t prompts[3][2]={{1,3},{7,2},{3,3}};int ok=1;
    for(int pr=0;pr<3;pr++)for(int d=0;d<3;d++){
        Gen base,g;Checks k={.bytes=cb};baseline(&c,prompts[pr],2,6,&base,NULL,scratch);
        speculate(&c,prompts[pr],2,6,(Drafter)d,&base,&g,&k);baseline(&c,prompts[pr],2,6,&base,&k,scratch);
        printf("prompt %lld %lld, %s drafts: ids",(long long)prompts[pr][0],(long long)prompts[pr][1],names[d]);for(int64_t i=0;i<g.n;i++)printf(" %lld",(long long)g.ids[i]);
        printf("; blocks=%lld accepted=%lld; same as target-only: %s; caches after rollback equal target-only: %lld of %lld\n",(long long)g.blocks,(long long)g.accepted,same(&base,&g)?"yes":"NO",(long long)k.checked,(long long)k.count);
        ok=ok && same(&base,&g) && k.checked==k.count;for(int64_t i=0;i<k.count;i++)free(k.cache[i]);
    }
    free(scratch);free(c.ops);free(c.args);pg_close(s);munmap(data,bytes);puts(ok?"tiny verification checks pass":"tiny verification checks FAIL");return ok?0:1;
}
#endif
static int original(int argc,char **argv){
    if(argc!=9){fputs("usage: gptoss_verify cuda CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS K\n",stderr);return 2;}
    int runs=atoi(argv[7]);int64_t k=atoll(argv[8]);if(runs<1 || runs>64){fputs("RUNS must be 1 to 64\n",stderr);return 2;}
    PcDeviceInfo info;PcStatus code=pc_open(&info);if(code==PC_ABSENT){puts("SKIP: no CUDA device; no CPU fallback");return 77;}TRY(code);
    char device[256];snprintf(device,sizeof device,"%s sm_%d%d %zu bytes",info.name,info.major,info.minor,info.total_bytes);printf("device: %s\n",device);
    size_t source_bytes;const char *source=load(argv[2],&source_bytes);
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});assert(!profile.status);
    coax_t_GFile files[3];void *maps[3];size_t sizes[3];
    for(int i=0;i<3;i++){char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[3],(int)pins[i].name.count,pins[i].name.data)>0);maps[i]=load(path,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    PgSession *s=NULL;coax_t_GResult why;int64_t begin=ns();TRY(pg_open(profile,(coax_v_t_GFile){files,3},source,source_bytes,17179869184,&s,&why));
    if(why.status){fprintf(stderr,"device admission status=%u\n",why.status);return 3;}assert(s);
    printf("original 20b authenticated and resident in %.3f s; prefill expert products: gpt-oss-cuda-f32-v1\n",(ns()-begin)*1e-9);fflush(stdout);
    Ctx c=open_ctx(s,profile,k,3);printf("speculation: prompt n-gram drafts (width 3), k=%lld, verified in one batched prefill per block\n",(long long)k);
    size_t table_bytes;void *table_data=load("psionic/fixtures/harmony/gptoss.hst",&table_bytes);coax_v_u8 table=bv(table_data,(int64_t)table_bytes);assert(hs_admit(table));
    const char *texts[2]={"Say hi.","List the integers from 1 to 100, separated by commas."};int64_t prompt[2][1024],len[2];
    for(int p=0;p<2;p++){uint8_t system[1024],work[1024];coax_t_PtResult sys=hm_system_checked(0,bv(system,1024),bv(work,1024),0);assert(!sys.status);
        coax_t_HmMessage m[2]={{.role=COAX_HmRole_system,.body={(const char*)system,sys.written}},{.role=COAX_HmRole_user,.body=str(texts[p])}};
        static int64_t input[16384],stage[16384],symbols[1024];coax_t_HmResult r=hm_render_checked(table,(coax_v_t_HmMessage){m,2},1,iv(input,16384),iv(stage,16384),iv(symbols,1024),0);assert(!r.status && r.written<=1024);
        memcpy(prompt[p],input,(size_t)r.written*8);len[p]=r.written;}
    size_t cb=(size_t)pg_plan(s).counts.e[16]*4;float *scratch=malloc(cb);assert(scratch);int all_same=1;static Gen base[2],g[2];
    /* Checks, untimed: parity on both prompts, and the cache after up to six
       rollbacks past the sliding window equal to the target-only cache. */
    int64_t checked=0,recorded=0;
    for(int p=0;p<2;p++){Checks k={.bytes=cb,.from=c.shape.window};baseline(&c,prompt[p],len[p],256,&base[p],NULL,scratch);
        speculate(&c,prompt[p],len[p],256,DRAFT_NGRAM,&base[p],&g[p],&k);baseline(&c,prompt[p],len[p],256,&base[p],&k,scratch);
        report(p?"check list":"check hi",&base[p],&g[p],0,0);all_same=all_same && same(&base[p],&g[p]);checked+=k.checked;recorded+=k.count;for(int64_t i=0;i<k.count;i++)free(k.cache[i]);}
    printf("greedy outputs with speculation %s the target-only outputs; target-only words %016llx %016llx\n",all_same?"equal":"DIFFER from",
        (unsigned long long)tg_word((coax_string){(const char*)base[0].ids,base[0].n*8}),(unsigned long long)tg_word((coax_string){(const char*)base[1].ids,base[1].n*8}));
    printf("cache bytes after rollback equal the target-only cache: %lld of %lld positions past the window\n",(long long)checked,(long long)recorded);fflush(stdout);
    if(checked!=recorded)all_same=0;
    /* Timed: one warm-up, then RUNS runs; each run does both prompts on both
       paths, alternating which path goes first. */
    int64_t tb[64],ts[64],tokens=base[0].n+base[1].n;
    for(int r=-1;r<runs;r++){int64_t a=0,b=0;double l=load1();
        for(int p=0;p<2;p++){Gen x,y;if(r%2){b+=speculate(&c,prompt[p],len[p],256,DRAFT_NGRAM,&base[p],&y,NULL);a+=baseline(&c,prompt[p],len[p],256,&x,NULL,scratch);}
            else {a+=baseline(&c,prompt[p],len[p],256,&x,NULL,scratch);b+=speculate(&c,prompt[p],len[p],256,DRAFT_NGRAM,&base[p],&y,NULL);}
            all_same=all_same && same(&base[p],&x) && same(&base[p],&y);if(r==-1)report(p?"warm-up list":"warm-up hi",&x,&y,0,0);}
        printf("run=%d base_ns=%lld spec_ns=%lld base_tok_s=%.3f spec_tok_s=%.3f load1=%.2f\n",r,(long long)a,(long long)b,tokens*1e9/(double)a,tokens*1e9/(double)b,l);fflush(stdout);
        if(r>=0){tb[r]=a;ts[r]=b;}}
    printf("bound: build=%s commit=%s machine=%s device=%s\n",argv[4],argv[5],argv[6],device);
    coax_t_SgDecision d=sg_decide(all_same,iv(tb,runs),iv(ts,runs));coax_string v=sg_verdict_name(d.verdict),why2=sg_reason_name(d.reason);
    printf("base: n=%lld mean_ns=%.0f sd_ns=%.0f; spec: mean_ns=%.0f sd_ns=%.0f; gain=%.4f se_ns=%.0f threshold=%d%%\n",(long long)d.base.n,d.base.mean,d.base.sd,d.spec.mean,d.spec.sd,d.gain,d.se,(int)5);
    printf("speculation: %.*s (%.*s)\n",(int)v.count,v.data,(int)why2.count,why2.data);
    free(scratch);free(c.ops);free(c.args);pg_close(s);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);munmap(table_data,table_bytes);munmap((void*)source,source_bytes);
    return d.verdict==COAX_SgVerdict_refused?5:0;
}
int main(int argc,char **argv){
#ifdef PSIONIC_DEVICE_TEST
    if(argc==3 && !strcmp(argv[1],"cpu"))return tiny(argv[2]);
#endif
    if(argc>=2 && !strcmp(argv[1],"cuda"))return original(argc,argv);
    fputs("usage: gptoss_verify cpu TINY_MODEL | cuda CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS K\n",stderr);return 2;
}
