/* Chapter 102: choose the draft length by expected gain, on the original
   gpt-oss-20b. Coax drafts (gv_draft), builds every program (gv_save,
   gv_readout, gv_restore), decides what is committed (gv_more, gv_commit),
   fits the cost line and the acceptance model (dl_fit, dl_costs,
   dl_calibrate), chooses each block's length (dl_choose) and decides each
   comparison (sg_decide). This harness opens the session, executes those
   programs, reads a monotonic clock and prints what Coax decided.
   Usage:
     gptoss_draft_length cuda CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS
   Paths: target only; fixed k = 1, 2, 3, 4; adaptive k (0 to 4). */
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
#define MAXGEN 1024
#define KMAX 4
#define PATHS 6 /* 0 target only, 1..4 fixed k, 5 adaptive */
static int64_t ns(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return (int64_t)t.tv_sec*1000000000+t.tv_nsec;}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_f64 fv(double *p,int64_t n){return(coax_v_f64){p,n};}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_string str(const char *p){return(coax_string){p,(int64_t)strlen(p)};}
static void *load(const char *path,size_t *n){int fd=open(path,O_RDONLY);assert(fd>=0);struct stat st;assert(!fstat(fd,&st) && st.st_size>0);*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);assert(!close(fd));return p;}
static double load1(void){double l=-1;FILE *f=fopen("/proc/loadavg","r");if(f){if(fscanf(f,"%lf",&l)!=1)l=-1;fclose(f);}return l;}

/* chosen[k]: blocks of k drafts (chosen[0]: plain steps). Timing sums are
   filled only when `timed` is set: block_ns[k] over block_n[k] blocks of k
   drafts, step_ns over step_n plain steps. reached/kept count, per draft
   position, the blocks that proposed it after all earlier drafts matched,
   and those that kept it. */
typedef struct {int64_t ids[MAXGEN];int64_t n,blocks,drafted,accepted,chosen[KMAX+1];int64_t block_ns[KMAX+1],block_n[KMAX+1],step_ns,step_n,reached[KMAX],kept[KMAX];} Gen;
typedef struct {PgSession *s;coax_t_GShape shape;coax_t_GDParam params[3+15*36];int64_t nparams;coax_t_GDOp *ops;coax_t_GDArg *args;double accept[KMAX],cost[KMAX+1];} Ctx;
static void run(Ctx *c,coax_t_GDProgram p){if(p.status){fprintf(stderr,"Coax program refused: %u\n",p.status);exit(3);}TRY(pg_run(c->s,c->ops,c->args,p));}
static coax_v_t_GDOp opv(Ctx *c){return(coax_v_t_GDOp){c->ops,pg_plan(c->s).max_ops};}
static coax_v_t_GDArg argv_(Ctx *c){return(coax_v_t_GDArg){c->args,pg_plan(c->s).max_args};}

/* One generation. path 0: target only. 1..4: n-gram drafts of up to that
   many IDs. 5: n-gram drafts of up to 4, then dl_choose picks 0..drafted. */
static int64_t generate(Ctx *c,const int64_t *prompt,int64_t len,int64_t limit,int path,Gen *g){
    int64_t control[8];memset(g,0,sizeof *g);TRY(pg_reset(c->s));TRY(pg_prefill(c->s,prompt,len,0,1,limit,control));assert(!control[0]);g->ids[g->n++]=control[1];
    static int64_t history[MAXGEN+4096],scratch[MAXGEN+4096+32];memcpy(history,prompt,(size_t)len*8);
    int64_t begin=ns();
    while(!control[4]){
        int64_t t0=ns(),position=control[2],block[32],picks[32],out[32],*draft=block+1,drafted=0;block[0]=control[1];
        int64_t room=path==0?0:gv_room(c->shape,position,path<=KMAX?path:KMAX);
        if(room>0){for(int64_t i=0;i<g->n;i++)history[len+i]=g->ids[i];drafted=gv_draft(iv(history,len+g->n),3,room,iv(scratch,len+g->n+room),iv(draft,room));assert(drafted>=0);}
        if(path==PATHS-1 && drafted>0){coax_t_DlChoice d=dl_choose(fv(c->accept,drafted),fv(c->cost,drafted+1));if(d.status){fprintf(stderr,"choice refused: %u\n",d.status);exit(3);}drafted=d.k;}
        if(drafted==0){TRY(pg_step(c->s,-1,position,1,limit,1,control));assert(!control[0] && g->n<MAXGEN);g->ids[g->n++]=control[1];g->chosen[0]++;g->step_ns+=ns()-t0;g->step_n++;continue;}
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
        g->blocks++;g->drafted+=drafted;g->accepted+=b.accepted;g->chosen[drafted]++;
        for(int64_t i=0;i<drafted && i<=b.accepted;i++){g->reached[i]++;if(i<b.accepted)g->kept[i]++;}
        g->block_ns[drafted]+=ns()-t0;g->block_n[drafted]++;
    }
    return ns()-begin;
}
static int same(const Gen *a,const Gen *b){return a->n==b->n && !memcmp(a->ids,b->ids,(size_t)a->n*8);}
static const char *names[PATHS]={"target only","fixed k=1","fixed k=2","fixed k=3","fixed k=4","adaptive"};
static void verdict(const char *what,int all_same,int64_t *base,int64_t *spec,int runs){
    coax_t_SgDecision d=sg_decide(all_same,iv(base,runs),iv(spec,runs));coax_string v=sg_verdict_name(d.verdict),why=sg_reason_name(d.reason);
    printf("%s: mean %.0f ns against %.0f ns; saving %.4f of the first, se %.0f ns; %.*s (%.*s)\n",what,d.spec.mean,d.base.mean,d.gain,d.se,(int)v.count,v.data,(int)why.count,why.data);
}
int main(int argc,char **argv){
    if(argc!=8 || strcmp(argv[1],"cuda")){fputs("usage: gptoss_draft_length cuda CUDA ORIGINAL_DIR BUILD COMMIT MACHINE RUNS\n",stderr);return 2;}
    int runs=atoi(argv[7]);if(runs<1 || runs>64){fputs("RUNS must be 1 to 64\n",stderr);return 2;}
    PcDeviceInfo info;PcStatus code=pc_open(&info);if(code==PC_ABSENT){puts("SKIP: no CUDA device; no CPU fallback");return 77;}TRY(code);
    char device[256];snprintf(device,sizeof device,"%s sm_%d%d %zu bytes",info.name,info.major,info.minor,info.total_bytes);printf("device: %s\n",device);
    size_t source_bytes;const char *source=load(argv[2],&source_bytes);
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});assert(!profile.status);
    coax_t_GFile files[3];void *maps[3];size_t sizes[3];
    for(int i=0;i<3;i++){char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[3],(int)pins[i].name.count,pins[i].name.data)>0);maps[i]=load(path,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    PgSession *s=NULL;coax_t_GResult why;int64_t begin=ns();TRY(pg_open(profile,(coax_v_t_GFile){files,3},source,source_bytes,17179869184,&s,&why));
    if(why.status){fprintf(stderr,"device admission status=%u\n",why.status);return 3;}assert(s);
    printf("original 20b authenticated and resident in %.3f s\n",(ns()-begin)*1e-9);fflush(stdout);
    Ctx c={.s=s,.shape=pg_shape(s)};c.nparams=3+15*c.shape.layers;assert(!gd_params(profile,(coax_v_t_GDParam){c.params,c.nparams}).status);
    coax_t_GDPlan undo=gv_with_undo(pg_plan(s),c.shape,KMAX);if(undo.status){printf("speculation refused: status=%u\n",undo.status);return 3;}TRY(pg_adopt(s,undo));
    c.ops=calloc((size_t)undo.max_ops,sizeof *c.ops);c.args=calloc((size_t)undo.max_args,sizeof *c.args);assert(c.ops && c.args);
    size_t table_bytes;void *table_data=load("psionic/fixtures/harmony/gptoss.hst",&table_bytes);coax_v_u8 table=bv(table_data,(int64_t)table_bytes);assert(hs_admit(table));
    const char *texts[2]={"Say hi.","List the integers from 1 to 100, separated by commas."};int64_t prompt[2][1024],len[2];
    for(int p=0;p<2;p++){uint8_t system[1024],work[1024];coax_t_PtResult sys=hm_system_checked(0,bv(system,1024),bv(work,1024),0);assert(!sys.status);
        coax_t_HmMessage m[2]={{.role=COAX_HmRole_system,.body={(const char*)system,sys.written}},{.role=COAX_HmRole_user,.body=str(texts[p])}};
        static int64_t input[16384],stage[16384],symbols[1024];coax_t_HmResult r=hm_render_checked(table,(coax_v_t_HmMessage){m,2},1,iv(input,16384),iv(stage,16384),iv(symbols,1024),0);assert(!r.status && r.written<=1024);
        memcpy(prompt[p],input,(size_t)r.written*8);len[p]=r.written;}
    /* Calibration, which also warms up: target only and fixed k = 1..4 on
       both prompts, timing every plain step and every block. */
    static Gen base[2],g;int all_same=1;int64_t step_ns=0,step_n=0,block_ns[KMAX+1]={0},block_n[KMAX+1]={0},reached[KMAX]={0},kept[KMAX]={0};
    for(int p=0;p<2;p++){generate(&c,prompt[p],len[p],256,0,&base[p]);step_ns+=base[p].step_ns;step_n+=base[p].step_n;}
    for(int path=1;path<=KMAX;path++)for(int p=0;p<2;p++){generate(&c,prompt[p],len[p],256,path,&g);all_same=all_same && same(&base[p],&g);
        for(int k=1;k<=KMAX;k++){block_ns[k]+=g.block_ns[k];block_n[k]+=g.block_n[k];}
        if(path==KMAX)for(int i=0;i<KMAX;i++){reached[i]+=g.reached[i];kept[i]+=g.kept[i];}}
    double mean[KMAX];printf("calibration: plain step %.0f ns over %lld steps\n",(double)step_ns/(double)step_n,(long long)step_n);
    for(int k=1;k<=KMAX;k++){if(!block_n[k]){printf("calibration: no block of %d drafts\n",k);return 3;}mean[k-1]=(double)block_ns[k]/(double)block_n[k];
        printf("calibration: block of %d drafts %.0f ns over %lld blocks\n",k,mean[k-1],(long long)block_n[k]);}
    coax_t_DlLine line=dl_fit(fv(mean,KMAX));coax_t_DlStatus st=dl_costs((double)step_ns/(double)step_n,line,fv(c.cost,KMAX+1));
    if(st){printf("cost model refused: %u\n",st);return 3;}
    st=dl_calibrate(iv(reached,KMAX),iv(kept,KMAX),fv(c.accept,KMAX));if(st){printf("acceptance model refused: %u\n",st);return 3;}
    printf("cost line: c(k) = %.0f + %.0f k ns; costs",line.base,line.slope);for(int k=0;k<=KMAX;k++)printf(" %.0f",c.cost[k]);printf("\n");
    printf("acceptance at k=4 by position (kept/reached):");for(int i=0;i<KMAX;i++)printf(" %lld/%lld",(long long)kept[i],(long long)reached[i]);
    printf("; model");for(int i=0;i<KMAX;i++)printf(" %.4f",c.accept[i]);printf("\n");
    printf("with all 4 drafts available: k=%lld; certain drafts would choose k=%lld; never pays at any acceptance: %s\n",(long long)dl_choose(fv(c.accept,KMAX),fv(c.cost,KMAX+1)).k,
        (long long)dl_choose(fv((double[]){1,1,1,1},KMAX),fv(c.cost,KMAX+1)).k,dl_never(fv(c.cost,KMAX+1))?"yes":"no");
    /* Parity of the adaptive path, untimed. */
    for(int p=0;p<2;p++){generate(&c,prompt[p],len[p],256,PATHS-1,&g);all_same=all_same && same(&base[p],&g);
        printf("adaptive %s: ids=%lld same=%s plain=%lld blocks by k:",p?"list":"hi",(long long)g.n,same(&base[p],&g)?"yes":"NO",(long long)g.chosen[0]);
        for(int k=1;k<=KMAX;k++)printf(" %lld",(long long)g.chosen[k]);printf(" accepted=%lld of %lld\n",(long long)g.accepted,(long long)g.drafted);}
    printf("greedy outputs on every path %s the target-only outputs; target-only words %016llx %016llx\n",all_same?"equal":"DIFFER from",
        (unsigned long long)tg_word((coax_string){(const char*)base[0].ids,base[0].n*8}),(unsigned long long)tg_word((coax_string){(const char*)base[1].ids,base[1].n*8}));fflush(stdout);
    /* Timed: RUNS runs; each runs both prompts on every path, the path
       order rotating by one each run. */
    static int64_t t[PATHS][64];int64_t tokens=base[0].n+base[1].n;
    for(int r=0;r<runs;r++){double l=load1();int64_t sum[PATHS]={0};
        for(int i=0;i<PATHS;i++){int path=(i+r)%PATHS;for(int p=0;p<2;p++){sum[path]+=generate(&c,prompt[p],len[p],256,path,&g);all_same=all_same && same(&base[p],&g);}}
        printf("run=%d",r);for(int path=0;path<PATHS;path++){t[path][r]=sum[path];printf(" %lld",(long long)sum[path]);}printf(" load1=%.2f\n",l);fflush(stdout);}
    printf("bound: build=%s commit=%s machine=%s device=%s\n",argv[4],argv[5],argv[6],device);
    int best=1;double m[PATHS];
    for(int path=0;path<PATHS;path++){double sum=0;for(int r=0;r<runs;r++)sum+=(double)t[path][r];m[path]=sum/runs;
        printf("%s: mean %.0f ns, %.2f tokens/s\n",names[path],m[path],tokens*1e9/m[path]);if(path>=1 && path<=KMAX && m[path]<m[best])best=path;}
    for(int path=1;path<PATHS;path++){char what[64];snprintf(what,sizeof what,"%s against target only",names[path]);verdict(what,all_same,t[0],t[path],runs);}
    {char what[64];snprintf(what,sizeof what,"adaptive against the best fixed (%s)",names[best]);verdict(what,all_same,t[best],t[PATHS-1],runs);}
    free(c.ops);free(c.args);pg_close(s);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);munmap(table_data,table_bytes);munmap((void*)source,source_bytes);
    return all_same?0:1;
}
