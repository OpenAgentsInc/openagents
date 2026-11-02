/* Chapter 99: a decode step with no host waits, on the original gpt-oss-20b.
   Coax builds the recorded step (gs_program), admits the options and decides
   the starting control words; chapter 53's nx_step is the host-sampled
   reference. This harness opens the session, records the program once,
   launches it once per token, counts host waits (pc_waits) and compares.
   Usage: gptoss_step CUDA ORIGINAL_DIR RUNS */
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
#define BATCH 16
static int64_t ns(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return (int64_t)t.tv_sec*1000000000+t.tv_nsec;}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_string str(const char *p){return(coax_string){p,(int64_t)strlen(p)};}
static void *load(const char *path,size_t *n){int fd=open(path,O_RDONLY);assert(fd>=0);struct stat st;assert(!fstat(fd,&st) && st.st_size>0);*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);assert(!close(fd));return p;}
static int64_t render(coax_v_u8 table,const char *user,int64_t *input){
    uint8_t system[1024],work[1024];coax_t_PtResult sys=hm_system_checked(0,bv(system,1024),bv(work,1024),0);assert(!sys.status);
    coax_t_HmMessage m[2]={{.role=COAX_HmRole_system,.body={(const char*)system,sys.written}},{.role=COAX_HmRole_user,.body=str(user)}};
    static int64_t stage[16384],symbols[1024];coax_t_HmResult r=hm_render_checked(table,(coax_v_t_HmMessage){m,2},1,iv(input,16384),iv(stage,16384),iv(symbols,1024),0);assert(!r.status);return r.written;
}
typedef struct {int64_t ids[512],n,decode_ns,step_waits,boundary_waits,launches;} Run;
static PgSession *s;static coax_t_GShape shape;static coax_t_GDParam params[512];static float *logits;static double *out,*stage;static int64_t *order,*temporary;
/* Chapter 70's path: one replayed step, then a wait to read the control words. */
static void host_greedy(const int64_t *input,int64_t n,int64_t limit,Run *r){
    int64_t control[8];TRY(pg_reset(s));TRY(pg_prefill(s,input,n,0,1,limit,control));assert(!control[0]);r->n=0;r->ids[r->n++]=control[1];
    uint64_t w0=pc_waits();int64_t t0=ns();
    while(!control[4]){TRY(pg_step(s,-1,control[2],1,limit,1,control));assert(!control[0] && r->n<512);r->ids[r->n++]=control[1];r->launches++;}
    r->decode_ns=ns()-t0;r->boundary_waits=(int64_t)(pc_waits()-w0);r->step_waits=0;
}
/* Chapter 53 on the host: download the logits every step and draw with nx_step. */
static void host_sampled(const int64_t *input,int64_t n,int64_t limit,coax_t_NxOptions o,uint64_t seed,Run *r){
    int64_t control[8],stops[2]={200002,200012};TRY(pg_reset(s));TRY(pg_prefill(s,input,n-1,0,0,limit,control));assert(!control[0]);
    coax_t_NxRun run={.random={.seed=seed,.counter=0},.emitted=0,.limit=limit};int64_t token=input[n-1],position=n-1;r->n=0;
    uint64_t w0=pc_waits();int64_t t0=ns();
    for(;;){TRY(pg_step(s,token,position,0,limit,1,control));assert(!control[0]);TRY(pg_read(s,18,logits,(size_t)shape.vocab*4));r->launches++;
        coax_t_NxEmission e=nx_step((coax_v_f32){logits,shape.vocab},(coax_v_u8){NULL,0},o,run,iv(stops,2),0,(coax_v_f64){out,shape.vocab},(coax_t_NxWork){.stage={stage,shape.vocab},.order={order,shape.vocab},.temporary={temporary,shape.vocab}},0);
        assert(!e.status && r->n<512);r->ids[r->n++]=e.token;run=e.run;position++;token=e.token;if(run.stop)break;}
    r->decode_ns=ns()-t0;r->boundary_waits=(int64_t)(pc_waits()-w0);r->step_waits=0;
}
/* Chapter 99: launch the recorded step BATCH times with no wait, then read the
   control words once. Steps after a stop change nothing (status 10). */
static void device_steps(PcProgram program,int64_t first_index,Run *r){
    int64_t control[8];uint64_t w0=pc_waits();int64_t t0=ns();
    for(;;){uint64_t before=pc_waits();for(int i=0;i<BATCH;i++){TRY(pg_launch(s,program));r->launches++;}r->step_waits+=(int64_t)(pc_waits()-before);
        TRY(pg_read(s,24,control,64));if(control[0]!=0 && control[0]!=10){fprintf(stderr,"device step refused: status=%lld\n",(long long)control[0]);exit(2);}if(control[4])break;}
    r->decode_ns=ns()-t0;r->boundary_waits=(int64_t)(pc_waits()-w0)-r->step_waits;
    static int64_t emitted[4096];assert(control[3]<=4096);TRY(pg_read(s,26,emitted,(size_t)control[3]*8));
    for(int64_t i=first_index;i<control[3];i++){assert(r->n<512);r->ids[r->n++]=emitted[i];}
}
static void device_greedy(PcProgram program,const int64_t *input,int64_t n,int64_t limit,Run *r){
    int64_t control[8];TRY(pg_reset(s));TRY(pg_prefill(s,input,n,0,1,limit,control));assert(!control[0]);r->n=0;r->ids[r->n++]=control[1];
    if(!control[4])device_steps(program,1,r);
}
static void device_sampled(PcProgram program,const int64_t *input,int64_t n,int64_t limit,Run *r){
    int64_t control[8];TRY(pg_reset(s));TRY(pg_prefill(s,input,n-1,0,0,limit,control));assert(!control[0]);
    coax_t_GResult c=gs_control(shape,input[n-1],n-1,iv(control,8));assert(!c.status);TRY(pg_write(s,24,control,64));r->n=0;device_steps(program,0,r);
}
static int same(const Run *a,const Run *b){return a->n==b->n && !memcmp(a->ids,b->ids,(size_t)a->n*8);}
static uint64_t word(const Run *r){return tg_word((coax_string){(const char*)r->ids,r->n*8});}
static PcProgram record(coax_t_NxOptions o,uint64_t seed,int64_t limit){
    coax_t_GDPlan plan=pg_plan(s);static coax_t_GDOp ops[4096];static coax_t_GDArg args[98304];assert(plan.max_ops<=4096 && plan.max_args<=98304);
    coax_t_GDProgram p=gs_program(shape,plan,(coax_v_t_GDParam){params,3+15*shape.layers},o,seed,limit,(coax_v_t_GDOp){ops,plan.max_ops},(coax_v_t_GDArg){args,plan.max_args});
    if(p.status){fprintf(stderr,"gs_program refused: %u\n",p.status);exit(3);}
    coax_string names[16];int64_t first=gd_kinds(),count=gs_kinds()-first;for(int64_t i=0;i<count;i++)names[i]=gs_kernel_name(first+i);
    PcProgram graph;TRY(pg_record(s,ops,args,p,names,first,count,&graph));printf("recorded step: %lld launches in one graph\n",(long long)p.ops);return graph;
}
static void show(const char *what,int prompt,const Run *a,const Run *b){
    printf("%s prompt=%d host ids=%lld word=%016llx | device ids=%lld word=%016llx | %s\n",what,prompt,(long long)a->n,(unsigned long long)word(a),(long long)b->n,(unsigned long long)word(b),same(a,b)?"equal":"DIFFER");
    printf("  host: %lld steps, %lld host waits; device: %lld launches, %lld host waits inside them, %lld at batch boundaries\n",(long long)a->launches,(long long)a->boundary_waits,(long long)b->launches,(long long)b->step_waits,(long long)b->boundary_waits);
}
int main(int argc,char **argv){
    if(argc!=4){fputs("usage: gptoss_step CUDA ORIGINAL_DIR RUNS\n",stderr);return 2;}
    int runs=atoi(argv[3]);if(runs<1 || runs>64){fputs("RUNS must be 1 to 64\n",stderr);return 2;}
    PcDeviceInfo info;PcStatus code=pc_open(&info);if(code==PC_ABSENT){puts("SKIP: no CUDA device; no CPU fallback");return 77;}TRY(code);
    printf("device: %s sm_%d%d\n",info.name,info.major,info.minor);
    size_t source_bytes;const char *source=load(argv[1],&source_bytes);
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});assert(!profile.status);
    coax_t_GFile files[3];void *maps[3];size_t sizes[3];
    for(int i=0;i<3;i++){char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[2],(int)pins[i].name.count,pins[i].name.data)>0);maps[i]=load(path,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    coax_t_GResult why;TRY(pg_open(profile,(coax_v_t_GFile){files,3},source,source_bytes,17179869184,&s,&why));if(why.status){fprintf(stderr,"device admission status=%u\n",why.status);return 3;}
    shape=pg_shape(s);why=gd_params(profile,(coax_v_t_GDParam){params,512});assert(!why.status);
    logits=malloc((size_t)shape.vocab*4);out=malloc((size_t)shape.vocab*8);stage=malloc((size_t)shape.vocab*8);order=malloc((size_t)shape.vocab*8);temporary=malloc((size_t)shape.vocab*8);assert(logits && out && stage && order && temporary);
    size_t table_bytes;void *table_data=load("psionic/fixtures/harmony/gptoss.hst",&table_bytes);coax_v_u8 table=bv(table_data,(int64_t)table_bytes);assert(hs_admit(table));
    const char *prompts[2]={"Say hi.","List the integers from 1 to 100, separated by commas."};static int64_t inputs[2][16384];int64_t counts[2];
    for(int p=0;p<2;p++)counts[p]=render(table,prompts[p],inputs[p]);
    coax_t_NxOptions greedy={.temperature=0,.top_k=0,.top_p=1},sampled={.temperature=1,.top_k=64,.top_p=0.95};uint64_t seed=45001;int64_t limit=256;
    PcProgram gg=record(greedy,0,limit),sg=record(sampled,seed,limit);
    int ok=1;
    for(int p=0;p<2;p++){
        Run a={0},b={0},c={0},d={0};
        host_greedy(inputs[p],counts[p],limit,&a);device_greedy(gg,inputs[p],counts[p],limit,&b);show("greedy",p,&a,&b);ok&=same(&a,&b);
        host_sampled(inputs[p],counts[p],limit,sampled,seed,&c);device_sampled(sg,inputs[p],counts[p],limit,&d);show("seeded",p,&c,&d);ok&=same(&c,&d);
        /* The last draw's order and weights, read back from the device, against nx_distribution on its logits. */
        coax_t_GSLayout lay=gs_layout(pg_plan(s),shape.vocab,64);size_t bytes=(size_t)(lay.by_id.offset+8*64);int64_t *scratch=malloc(bytes);assert(scratch);
        TRY(pg_read(s,18,logits,(size_t)shape.vocab*4));TRY(pg_read(s,40,scratch,bytes));
        coax_t_NxResult dist=nx_distribution((coax_v_f32){logits,shape.vocab},(coax_v_u8){NULL,0},sampled,(coax_v_f64){out,shape.vocab},(coax_t_NxWork){.stage={stage,shape.vocab},.order={order,shape.vocab},.temporary={temporary,shape.vocab}},0);assert(!dist.status);
        int64_t mismatched=0;const int64_t *ids=scratch+lay.final*64;const double *weights=(const double*)(scratch+lay.weights.offset/8);
        for(int64_t i=0;i<dist.kept;i++){uint64_t x,y;memcpy(&x,&weights[i],8);memcpy(&y,&out[ids[i]],8);if(x!=y || ids[i]!=order[i])mismatched++;}
        printf("  last draw: %lld kept IDs; device order and weight bits differ from nx_distribution at %lld\n",(long long)dist.kept,(long long)mismatched);ok&=mismatched==0;free(scratch);
    }
    /* Timing: the list prompt, one warm-up and RUNS timed runs per path. */
    int64_t times[4][64],steps[4]={0},decoded[4]={0};const char *names[4]={"host greedy (replay, wait per token)","device greedy (recorded step)","host sampled (logits read, nx_step)","device sampled (recorded step)"};
    for(int r=-1;r<runs;r++){Run x[4]={{0}};host_greedy(inputs[1],counts[1],limit,&x[0]);device_greedy(gg,inputs[1],counts[1],limit,&x[1]);host_sampled(inputs[1],counts[1],limit,sampled,seed,&x[2]);device_sampled(sg,inputs[1],counts[1],limit,&x[3]);
        if(r>=0)for(int i=0;i<4;i++){times[i][r]=x[i].decode_ns;steps[i]=x[i].launches;decoded[i]=i<2?x[i].n-1:x[i].n;}}
    for(int i=0;i<4;i++){coax_t_CmStats st=cm_two_pass(iv(times[i],runs));
        printf("%-38s tokens=%lld launches=%lld n=%lld mean_ns=%.0f sd_ns=%.0f tokens_per_s=%.2f\n",names[i],(long long)decoded[i],(long long)steps[i],(long long)st.n,st.mean,st.sd,(double)decoded[i]*1e9/st.mean);}
    puts(ok?"all comparisons equal":"COMPARISONS DIFFER");
    pc_program_free(gg);pc_program_free(sg);pg_close(s);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);munmap(table_data,table_bytes);munmap((void*)source,source_bytes);
    return ok?0:1;
}
