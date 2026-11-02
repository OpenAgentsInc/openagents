/* Chapter 78: device generation calls a tool, on the original gpt-oss-20b.
   Coax decides the route (dx_route: device or a typed refusal), masks the
   downloaded logits before every argument choice (dx_choose), checks the
   grant (dx_grant), sequences the request (dx_next: when the one tool effect
   may happen and when device state must be released) and audits the receipt
   (dx_audit). This harness only opens the resident session, moves bytes,
   reads a monotonic clock and prints what Coax decided. It links no CPU
   forward pass: every model layer runs on the device.
   Usage: gptoss_device_tools CUDA ORIGINAL_DIR REFERENCE_DIR [SEEDS]
   REFERENCE_DIR holds chapter 77's recorded CPU reference IDs
   (step0.call.i64, step0.resumed.i64).
   Exit status: 0 pass, 1 a check failed, 2 platform failure, 5 refused. */
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
#define CHECK(c,...) do{if(!(c)){printf("FAIL: " __VA_ARGS__);printf("\n");exit(1);}}while(0)
enum{LIMIT=64,MAXIDS=16384};
static int64_t ns(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return (int64_t)t.tv_sec*1000000000+t.tv_nsec;}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_string str(const char *p){return(coax_string){p,(int64_t)strlen(p)};}
static void *load(const char *path,size_t *n){int fd=open(path,O_RDONLY);if(fd<0){fprintf(stderr,"cannot open %s\n",path);exit(2);}struct stat st;assert(!fstat(fd,&st) && st.st_size>0);*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);assert(!close(fd));return p;}
static void refuse(coax_t_DxWhy why){coax_string n=dx_why_name(why);printf("refused: %.*s\n",(int)n.count,n.data);exit(5);}

/* Everything one request needs, owned by main. */
typedef struct {
    PgSession *s;coax_v_u8 table;int64_t vocab;coax_t_DxRoute route;coax_string *pieces;float *logits;uint8_t *mask,*mask_stage;double *prob,*stage;int64_t *order,*tmp;
    coax_t_GTool tools[2];_Bool granted[2];int64_t ntools;coax_t_DxReceipt receipt;int64_t transfer_ns;int64_t effects;
} Run;
typedef struct {int64_t ids[MAXIDS];int64_t n;int64_t call_ids[1024];int64_t call_n;uint8_t body[513];int64_t body_n;coax_t_DxGrant grant;coax_t_DxPhase phase;char final[256];} Trip;
/* Cancellation points: where the host's cancel decision arrives. */
typedef enum {NEVER,PREFILL,ARGUMENT3,CALLED,ANSWER2} Cancel;
static const char *PHASE[]={"idle","argument","called","granted","ran","answer","done","cancelled","refused"};
static coax_string ADD[]={{"{\"a\":2,\"b\":2}",13}};
static coax_string MUL[]={{"{\"a\":3}",7}};
static coax_t_HmMessage messages[4];
static void init_messages(void){
    messages[0]=(coax_t_HmMessage){.role=COAX_HmRole_system,.body=str("Use functions.add to add numbers. Its arguments are JSON with a and b. Respond on the final channel after the result.")};
    messages[1]=(coax_t_HmMessage){.role=COAX_HmRole_user,.body=str("What is 2 + 2? Use add.")};
    messages[2]=(coax_t_HmMessage){.role=COAX_HmRole_assistant,.channel=COAX_HmChannel_commentary,.ending=COAX_HmEnd_call,.recipient=str("functions.add"),.body=str("")};
}
static int64_t render(Run *r,int64_t count,_Bool completion,int64_t *ids){static int64_t stage[MAXIDS],symbols[1024];coax_t_HmResult h=hm_render_checked(r->table,(coax_v_t_HmMessage){messages,count},completion,iv(ids,MAXIDS),iv(stage,MAXIDS),iv(symbols,1024),0);assert(!h.status);return h.written;}
static coax_t_DxPhase next(coax_t_DxPhase phase,coax_t_DxEvent event,Run *r){coax_t_DxStep s=dx_next(phase,event);CHECK(!s.why,"phase %d refused event %d",(int)phase,(int)event);
    if(s.effect)r->effects++;if(s.release){TRY(pg_reset(r->s));s=dx_next(s.phase,COAX_DxEvent_release);CHECK(!s.why && s.phase==COAX_DxPhase_idle,"release refused");return COAX_DxPhase_idle;}return s.phase;}
static int always(void *context){(void)context;return 1;}
/* The explicit, measured transfer: the device's published f32 logits. */
static void download(Run *r){int64_t t=ns();TRY(pg_read(r->s,18,r->logits,(size_t)r->vocab*4));r->transfer_ns+=ns()-t;r->receipt.masked++;r->receipt.transferred+=r->vocab*4;}
/* One forced named-tool request. Returns the last phase reached before the
   request ended: idle after a release, or the phase a check stopped in. */
static void trip(Run *r,Trip *t,double temperature,uint64_t seed,Cancel cancel){
    memset(t,0,sizeof *t);coax_t_DxPhase phase=next(COAX_DxPhase_idle,COAX_DxEvent_begin,r);int64_t control[8];
    messages[2].body=str("");int64_t n=render(r,3,0,t->ids)-1; /* open the argument body */
    if(cancel==PREFILL){int interrupted;TRY(pg_prefill_poll(r->s,t->ids,n,0,0,LIMIT,control,always,NULL,&interrupted));CHECK(interrupted,"prefill not interrupted");
        t->phase=phase;next(phase,COAX_DxEvent_cancel,r);t->n=n;return;}
    TRY(pg_prefill(r->s,t->ids,n,0,0,LIMIT,control));CHECK(!control[0],"prefill control=%lld",(long long)control[0]);r->receipt.prefills++;
    coax_t_GGeneration state={.random={seed,0},.remaining=LIMIT};coax_t_GSampleWork work={{r->stage,r->vocab},{r->order,r->vocab},{r->tmp,r->vocab}};int64_t argument=0;
    while(state.stop==COAX_GStop_running){download(r);
        coax_t_GEmission e=dx_choose((coax_v_string){ADD,1},(coax_string){(const char*)t->body,t->body_n},(coax_v_string){r->pieces,r->vocab},(coax_v_f32){r->logits,r->vocab},bv(r->mask,r->vocab),bv(r->mask_stage,r->vocab),
            (coax_t_GSampling){.temperature=temperature,.top_p=1},state,cancel==ARGUMENT3 && argument==3,(coax_v_f64){r->prob,r->vocab},work);CHECK(!e.status,"dx_choose status=%u",e.status);state=e.state;
        if(state.stop==COAX_GStop_cancelled){t->phase=phase;t->n=n;next(phase,COAX_DxEvent_cancel,r);return;}
        t->ids[n++]=e.token;argument++;
        if(state.stop==COAX_GStop_running){phase=next(phase,COAX_DxEvent_token,r);coax_string p=r->pieces[e.token];CHECK(t->body_n+p.count<=512,"argument too long");memcpy(t->body+t->body_n,p.data,(size_t)p.count);t->body_n+=p.count;
            TRY(pg_step(r->s,e.token,n-1,0,LIMIT,1,control));CHECK(!control[0],"step control=%lld",(long long)control[0]);r->receipt.steps++;}
    }
    CHECK(state.stop==COAX_GStop_call,"argument ended with stop=%u",state.stop);phase=next(phase,COAX_DxEvent_call,r);t->call_n=n;CHECK(n<=1024,"call too long");memcpy(t->call_ids,t->ids,(size_t)n*8);
    static coax_t_HmRecord records[64],draft[64];static uint8_t decoded[65536],work_bytes[65536];
    coax_t_HmResult parsed=hm_parse_checked(r->table,iv(t->ids,n),0,(coax_v_t_HmRecord){records,64},(coax_v_t_HmRecord){draft,64},bv(decoded,65536),bv(work_bytes,65536),0);CHECK(!parsed.status,"the call does not parse: %u",parsed.status);
    if(cancel==CALLED){t->phase=phase;t->n=n;next(phase,COAX_DxEvent_cancel,r);return;}
    t->grant=dx_grant((coax_v_t_GTool){r->tools,r->ntools},(coax_v_bool){r->granted,r->ntools},records[parsed.written-1],(coax_string){(const char*)decoded,parsed.bytes});
    if(t->grant.why){t->phase=phase;t->n=n;next(phase,COAX_DxEvent_refuse,r);return;}
    phase=next(phase,COAX_DxEvent_grant,r);phase=next(phase,COAX_DxEvent_run,r);
    coax_t_GInteger sum=gt_add(2,2);CHECK(!sum.status && sum.value==4,"tool failed");
    phase=next(phase,COAX_DxEvent_resume,r);
    /* Resume: chapter 77's tool-role message, a fresh completion prefix, and
       the device's own greedy choice (no mask, so no logits transfer). */
    messages[2].body=(coax_string){(const char*)t->body,t->body_n};coax_t_GToolResponse response=gt_response(r->tools[t->grant.tool].name,str("4"));assert(!response.status);messages[3]=response.message;
    n=render(r,4,1,t->ids);TRY(pg_reset(r->s));TRY(pg_prefill(r->s,t->ids,n,0,1,LIMIT,control));CHECK(!control[0],"resume control=%lld",(long long)control[0]);r->receipt.prefills++;t->ids[n++]=control[1];int64_t answer=1;
    while(!control[4]){if(cancel==ANSWER2 && answer==2){t->phase=phase;t->n=n;next(phase,COAX_DxEvent_cancel,r);return;}
        phase=next(phase,COAX_DxEvent_token,r);TRY(pg_step(r->s,-1,control[2],1,LIMIT,1,control));CHECK(!control[0] && n<MAXIDS,"answer control=%lld",(long long)control[0]);r->receipt.steps++;t->ids[n++]=control[1];answer++;}
    CHECK(control[4]==1,"the answer ended with stop=%lld",(long long)control[4]);t->n=n;
    coax_t_HmResult parsed2=hm_parse_checked(r->table,iv(t->ids,n),0,(coax_v_t_HmRecord){records,64},(coax_v_t_HmRecord){draft,64},bv(decoded,65536),bv(work_bytes,65536),0);CHECK(!parsed2.status,"the answer does not parse");
    uint8_t visible[256];for(int64_t i=0;i<parsed2.written;i++){coax_t_GResult d=gt_display(records[i],(coax_string){(const char*)decoded,parsed2.bytes},bv(visible,255),0);if(!d.status){memcpy(t->final,visible,(size_t)d.written);t->final[d.written]=0;}}
    t->phase=phase;next(phase,COAX_DxEvent_turn,r);
}
static int64_t *reference(const char *dir,const char *name,int64_t *n){char path[4096];snprintf(path,sizeof path,"%s/%s",dir,name);size_t bytes;int64_t *p=load(path,&bytes);*n=(int64_t)(bytes/8);return p;}
/* The whole ID sequence, prompt and generated, against the recorded CPU reference. */
static void same(const char *label,const int64_t *ids,int64_t n,int64_t prompt,const int64_t *want,int64_t want_n,int quiet){
    int64_t d=dx_first_difference(iv((int64_t*)ids,n),iv((int64_t*)want,want_n));
    CHECK(d<0,"%s differs from the CPU reference at position %lld (prompt %lld)",label,(long long)d,(long long)prompt);if(!quiet)printf("%s: %lld prompt + %lld generated IDs equal the CPU reference\n",label,(long long)prompt,(long long)(n-prompt));}
int main(int argc,char **argv){
    if(argc!=4 && argc!=5){fputs("usage: gptoss_device_tools CUDA ORIGINAL_DIR REFERENCE_DIR [SEEDS]\n",stderr);return 2;}
    int seeds=argc==5?atoi(argv[4]):8;if(seeds<0 || seeds>64){fputs("SEEDS must be 0 to 64\n",stderr);return 2;}
    PcDeviceInfo info;PcStatus code=pc_open(&info);
    if(code==PC_ABSENT)refuse(dx_route((coax_t_DxDevice){.present=0}).why);TRY(code);
    printf("device: %s sm_%d%d %zu bytes\n",info.name,info.major,info.minor,info.total_bytes);
    size_t source_bytes;const char *source=load(argv[1],&source_bytes);
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});assert(!profile.status);
    coax_t_GFile files[3];void *maps[3];size_t sizes[3];
    for(int i=0;i<3;i++){char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[2],(int)pins[i].name.count,pins[i].name.data)>0);maps[i]=load(path,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    Run r={0};coax_t_GResult why;int64_t begin=ns();TRY(pg_open(profile,(coax_v_t_GFile){files,3},source,source_bytes,17179869184,&r.s,&why));
    r.route=dx_route((coax_t_DxDevice){.present=1,.resident=!why.status && r.s,.vocab=r.s?pg_shape(r.s).vocab:0,.limit=dx_limit()});if(r.route.why)refuse(r.route.why);
    printf("original 20b authenticated and resident in %.3f s\n",(ns()-begin)*1e-9);
    r.vocab=pg_shape(r.s).vocab;printf("route: device; logits transfer %lld bytes per masked choice; no CPU route exists\n",(long long)r.route.transfer);fflush(stdout);
    size_t table_bytes;void *table_data=load("psionic/fixtures/harmony/gptoss.hst",&table_bytes);r.table=bv(table_data,(int64_t)table_bytes);assert(hs_admit(r.table));
    int64_t v=r.vocab;r.pieces=calloc((size_t)v,sizeof *r.pieces);uint8_t *dictionary=malloc(8388608);int64_t used=0;uint8_t one[1024],one_work[1024];
    for(int64_t id=0;id<199998;id++){coax_t_PtResult p=hs_decode_checked(r.table,iv(&id,1),bv(one,1024),bv(one_work,1024),0);assert(!p.status && used+p.written<=8388608);memcpy(dictionary+used,one,(size_t)p.written);r.pieces[id]=(coax_string){(const char*)dictionary+used,p.written};used+=p.written;}
    r.logits=malloc((size_t)v*4);r.mask=malloc((size_t)v);r.mask_stage=malloc((size_t)v);r.prob=malloc((size_t)v*8);r.stage=malloc((size_t)v*8);r.order=malloc((size_t)v*8);r.tmp=malloc((size_t)v*8);
    r.tools[0]=(coax_t_GTool){str("functions.add"),{ADD,1}};r.tools[1]=(coax_t_GTool){str("functions.mul"),{MUL,1}};r.granted[0]=1;r.granted[1]=1;r.ntools=2;init_messages();
    int64_t call_n,resumed_n;int64_t *call=reference(argv[3],"step0.call.i64",&call_n),*resumed=reference(argv[3],"step0.resumed.i64",&resumed_n);
    static Trip t,again;
    /* 1. Greedy round trip, timed, against chapter 77's recorded CPU reference. */
    r.receipt=(coax_t_DxReceipt){0};r.transfer_ns=0;begin=ns();trip(&r,&t,0,48,NEVER);int64_t elapsed=ns()-begin;
    CHECK(!t.grant.why && t.grant.tool==0,"greedy call refused");
    printf("call: functions.add arguments=%.*s granted=yes result=4 effects=%lld\n",(int)t.body_n,t.body,(long long)r.effects);
    int64_t answer_prompt=render(&r,4,1,again.ids);messages[2].body=str("");int64_t call_prompt=render(&r,3,0,again.ids)-1;
    same("call",t.call_ids,t.call_n,call_prompt,call,call_n,0);same("answer",t.ids,t.n,answer_prompt,resumed,resumed_n,0);printf("final: %s\n",t.final);
    coax_t_DxReceipt receipt=r.receipt;coax_t_DxWhy audit=dx_audit(receipt,r.route);CHECK(!audit,"receipt audit refused: %u",audit);
    printf("receipt: prefills=%lld device_steps=%lld masked=%lld transferred=%lld bytes transfer_ns=%lld round_trip_ns=%lld audit=pass\n",
        (long long)receipt.prefills,(long long)receipt.steps,(long long)receipt.masked,(long long)receipt.transferred,(long long)r.transfer_ns,(long long)elapsed);
    /* 2. Refusals before any effect: the same call, ungranted, then undeclared. */
    int64_t effects=r.effects;r.granted[0]=0;trip(&r,&again,0,48,NEVER);CHECK(again.grant.why==COAX_DxWhy_ungranted && r.effects==effects,"ungranted call not refused");
    coax_string n1=dx_why_name(again.grant.why);printf("ungranted: refused (%.*s); effects=%lld\n",(int)n1.count,n1.data,(long long)r.effects);r.granted[0]=1;
    r.tools[0]=r.tools[1];r.ntools=1;trip(&r,&again,0,48,NEVER);CHECK(again.grant.why==COAX_DxWhy_unknown && r.effects==effects,"undeclared call not refused");
    coax_string n2=dx_why_name(again.grant.why);printf("undeclared: refused (%.*s); effects=%lld\n",(int)n2.count,n2.data,(long long)r.effects);
    r.tools[0]=(coax_t_GTool){str("functions.add"),{ADD,1}};r.ntools=2;
    /* 3. Seeded sampled runs: every call parses, matches the schema and is granted. */
    for(int i=1;i<=seeds;i++){trip(&r,&again,1.0,(uint64_t)i,NEVER);CHECK(!again.grant.why,"seed %d refused",i);
        printf("seed=%d temperature=1 argument_ids=%lld arguments=%.*s granted=yes final=%s\n",i,(long long)(again.call_n-call_prompt),(int)again.body_n,again.body,again.final);}
    /* 4. Cancellation at four points; each releases device state and the next request repeats the reference exactly. */
    const char *names[]={"","prefill","argument token 3","call parsed, before the grant","answer token 2"};
    for(int c=PREFILL;c<=ANSWER2;c++){effects=r.effects;trip(&r,&again,0,48,(Cancel)c);int64_t ran=r.effects-effects;
        trip(&r,&t,0,48,NEVER);same("after cancel, call",t.call_ids,t.call_n,call_prompt,call,call_n,1);same("after cancel, answer",t.ids,t.n,answer_prompt,resumed,resumed_n,1);
        printf("cancel at %s: cancelled in phase %s, tool effects %lld; device state released; next request identical\n",names[c],PHASE[again.phase],(long long)ran);
        CHECK(ran==(c==ANSWER2),"a cancelled request had a partial effect");}
    size_t free_bytes,total_bytes;TRY(pc_memory(&free_bytes,&total_bytes));printf("device bytes in use=%zu of %zu\n",total_bytes-free_bytes,total_bytes);
    pg_close(r.s);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);
    puts("device tool round trips pass");return 0;
}
