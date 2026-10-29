/* Owned native storage and original-file runner. Platform effects only. */
#include "gptoss-memory.h"
#include "gptoss-tiny.h"
#include <assert.h>
#include <fcntl.h>
#include <inttypes.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
static coax_v_f32 fv(void *p,int64_t n){return (coax_v_f32){p,n};}
static coax_v_s64 iv(void *p,int64_t n){return (coax_v_s64){p,n};}
static coax_v_u8 bv(void *p,int64_t n){return (coax_v_u8){p,n};}
static double now(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return t.tv_sec+t.tv_nsec*1e-9;}
static uint64_t rss(void){struct rusage r;assert(!getrusage(RUSAGE_SELF,&r));
#ifdef __APPLE__
return (uint64_t)r.ru_maxrss;
#else
return (uint64_t)r.ru_maxrss*1024;
#endif
}
static void *alloc(int64_t n,size_t width){assert(n>=0 && (uint64_t)n<=SIZE_MAX/width);void *p=calloc((size_t)(n?n:1),width);assert(p);return p;}
static uint8_t *load(const char *name,int mapped,size_t *size){int fd=open(name,O_RDONLY);assert(fd>=0);struct stat st;assert(!fstat(fd,&st));assert(st.st_size>0);*size=(size_t)st.st_size;uint8_t *p;
    if(mapped){p=mmap(NULL,*size,PROT_READ,MAP_PRIVATE,fd,0);assert(p!=MAP_FAILED);}else{p=alloc((int64_t)*size,1);size_t n=0;while(n<*size){ssize_t got=read(fd,p+n,*size-n);assert(got>0);n+=(size_t)got;}}assert(!close(fd));return p;
}
static void save(const char *dir,int step,const char *kind,const void *data,size_t n){char path[4096];assert(snprintf(path,sizeof path,"%s/step%d.%s",dir,step,kind)>0);FILE *f=fopen(path,"wb");assert(f);assert(fwrite(data,1,n,f)==n);assert(!fclose(f));}
static void close_to(const float *a,const float *b,int n){for(int i=0;i<n;i++)assert(isfinite(a[i]) && fabs((double)a[i]-b[i])<2e-5*(1+fabs(b[i])));}
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
static void generate(coax_t_GModel model,coax_t_GPlan plan,coax_t_GWork work,float *cache,coax_t_GOutput out,const char *dir){
    size_t bytes;uint8_t *table_data=load("psionic/fixtures/harmony/gptoss.hst",1,&bytes);coax_v_u8 table=bv(table_data,(int64_t)bytes);assert(hs_admit(table));
    int64_t ids[16384],stage[16384],symbols[1024];uint8_t text[65536],text_work[65536];coax_t_HmRecord records[64],draft[64];
    coax_t_HmMessage messages[2]={{.role=COAX_HmRole_system,.body=str("You are ChatGPT. Respond on the final channel.")},{.role=COAX_HmRole_user,.body=str("Say hi.")}};
    coax_t_HmResult rendered=hm_render_checked(table,(coax_v_t_HmMessage){messages,2},1,iv(ids,16384),iv(stage,16384),iv(symbols,1024),0);assert(!rendered.status);
    int64_t prefix=rendered.written,total=prefix;double begin=now();
    memset(cache,0,(size_t)plan.cache*4);for(int64_t i=0;i<prefix;i++){coax_t_GStep r=psionic_gptoss_forward(model,ids[i],i,g_macs(model.shape,i),fv(cache,plan.cache),out,work);assert(!r.status);}
    double prefill=now()-begin;double *prob=alloc(model.shape.vocab,8),*scratch=alloc(model.shape.vocab,8);int64_t *order=alloc(model.shape.vocab,8),*temporary=alloc(model.shape.vocab,8);
    coax_t_GSampleWork sample={{scratch,model.shape.vocab},{order,model.shape.vocab},{temporary,model.shape.vocab}};coax_t_GGeneration state={.random={45001,0},.remaining=64};coax_t_GSampling options={.temperature=0.7,.top_k=40,.top_p=0.9};
    do{coax_t_GEmission e=psionic_gptoss_emit(fv(out.logits.data,model.shape.vocab),bv(NULL,0),options,state,0,(coax_v_f64){prob,model.shape.vocab},sample);assert(!e.status);state=e.state;ids[total++]=e.token;
        if(state.stop!=COAX_GStop_running)break;
        coax_t_GStep r=psionic_gptoss_forward(model,e.token,total-1,g_macs(model.shape,total-1),fv(cache,plan.cache),out,work);assert(!r.status);
    }while(1);
    save(dir,0,"generation.i64",ids,(size_t)total*8);
    printf("generation prefix=%lld emitted=%lld stop=%u seed=45001 counter=%llu prefill_seconds=%.6f total_seconds=%.6f peak_rss_bytes=%" PRIu64 "\n",(long long)prefix,(long long)(total-prefix),state.stop,(unsigned long long)state.random.counter,prefill,now()-begin,rss());
    coax_t_HmResult parsed=hm_parse_checked(table,iv(ids,total),0,(coax_v_t_HmRecord){records,64},(coax_v_t_HmRecord){draft,64},bv(text,65536),bv(text_work,65536),0);
    assert(!parsed.status && state.stop==COAX_GStop_turn);
    for(int64_t i=0;i<parsed.written;i++){coax_t_HmRecord r=records[i];if(r.role==COAX_HmRole_assistant && r.channel==COAX_HmChannel_final)printf("final: %.*s\n",(int)(r.body_end-r.body_begin),text+r.body_begin);}
    free(prob);free(scratch);free(order);free(temporary);assert(!munmap(table_data,bytes));
}
static void numerical_checks(void){
    float live[3],backup[3];uint32_t raw[]={0x80000000,0x00000001,0x7fa12345};memcpy(live,raw,sizeof raw);assert(!psionic_gptoss_snapshot(fv(live,3),fv(backup,3)).status && !memcmp(raw,backup,sizeof raw));assert(psionic_gptoss_snapshot(fv(live,3),fv(live,3)).status==COAX_GStatus_alias);

    float x[]={0,3,1};uint8_t mask[]={1,1,1};double prob[3],stage[3];int64_t order[3],tmp[3];coax_t_GSampleWork sw={{stage,3},{order,3},{tmp,3}};coax_t_GGeneration state={.random={7,0},.remaining=3};coax_t_GSampling options={.temperature=0,.top_p=1};
    coax_t_GEmission e=psionic_gptoss_emit(fv(x,3),bv(mask,3),options,state,0,(coax_v_f64){prob,3},sw);assert(!e.status && e.token==1 && e.state.random.counter==1);
    sw.stage=(coax_v_f64){prob,3};assert(psionic_gptoss_emit(fv(x,3),bv(mask,3),options,state,0,(coax_v_f64){prob,3},sw).status==COAX_GStatus_alias);sw.stage=(coax_v_f64){stage,3};
    prob[0]=-7;x[1]=INFINITY;assert(psionic_gptoss_emit(fv(x,3),bv(mask,3),options,state,0,(coax_v_f64){prob,3},sw).status==COAX_GStatus_number && prob[0]==-7);x[1]=NAN;assert(psionic_gptoss_emit(fv(x,3),bv(mask,3),options,state,0,(coax_v_f64){prob,3},sw).status==COAX_GStatus_number);
    double p[]={.5,.25,.25},q[]={.25,.5,.25},residual[3],scratch[3];int counts[3]={0};coax_t_GSeed seed={47001,0};
    for(int i=0;i<65536;i++){coax_t_GDraw proposal=gs_draw(seed),accept=gs_draw(proposal.next),correct=gs_draw(accept.next);seed=correct.next;coax_t_GPick d=gs_pick((coax_v_f64){q,3},proposal.value);assert(!d.status);coax_t_GSpeculation r=gx_decide((coax_v_f64){p,3},(coax_v_f64){q,3},d.token,accept.value,correct.value,0,(coax_v_f64){residual,3},(coax_v_f64){scratch,3},0);assert(!r.status);counts[r.token]++;}
    assert(abs(counts[0]-32768)<1024 && abs(counts[1]-16384)<1024 && abs(counts[2]-16384)<1024 && seed.counter==196608);printf("sampled speculative counts: %d %d %d; target 32768 16384 16384; bound 1024; counter=196608\n",counts[0],counts[1],counts[2]);
    int64_t n=201088;coax_string *pieces=alloc(n,sizeof *pieces);uint8_t *bits=alloc(n+1,1),*bits_stage=alloc(n,1);coax_string alternatives[]={str("{}")};pieces[0]=str("{");pieces[199998]=str("{");bits[n]=7;
    coax_t_GResult m=psionic_gptoss_mask((coax_v_string){alternatives,1},str(""),(coax_v_string){pieces,n},bv(bits,n+1),bv(bits_stage,n));assert(!m.status && bits[0]==1 && bits[199998]==0 && bits[200012]==0 && bits[n]==7);
    m=psionic_gptoss_mask((coax_v_string){alternatives,1},str("{}"),(coax_v_string){pieces,n},bv(bits,n+1),bv(bits_stage,n));assert(!m.status && bits[200012]==1 && bits[200013]==0 && bits[n]==7);
    assert(psionic_gptoss_mask((coax_v_string){alternatives,1},str("{}"),(coax_v_string){pieces,n},bv(bits,n+1),bv(bits,n)).status==COAX_GStatus_alias);free(pieces);free(bits);free(bits_stage);
}
int main(int argc,char **argv){
    numerical_checks();
    assert(argc==3 || argc==5);int generating=!strcmp(argv[1],"generate");int original=generating || !strcmp(argv[1],"original");uint8_t target=original?(uint8_t)strtoul(argv[2],NULL,10):0;const char *output_dir=original?argv[4]:NULL;
    coax_t_GFile files[16];void *maps[16]={0};size_t sizes[16]={0};coax_t_GBlock blocks[36],draft[36];coax_t_GModel model;int nf=1;
    double start=now();
    if(original){nf=(int)g_original_files(target);assert(nf>0);for(int i=0;i<nf;i++){coax_string name=g_original_filename(target,i);char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[3],(int)name.count,name.data)>0);maps[i]=load(path,1,&sizes[i]);files[i]=(coax_t_GFile){name,bv(maps[i],(int64_t)sizes[i])};}
        printf("authenticating target=%u files=%d\n",target,nf);fflush(stdout);
        coax_t_GResult r=psionic_gptoss_bind(target,generating?256:128,(coax_v_t_GFile){files,nf},(coax_v_t_GBlock){blocks,36},(coax_v_t_GBlock){draft,36},(coax_v_t_GModel){&model,1});
        if(r.status){fprintf(stderr,"original binding refused: %u\n",r.status);return 2;}
    }else{
        maps[0]=load(argv[2],0,&sizes[0]);files[0]=(coax_t_GFile){{"tiny.safetensors",16},bv(maps[0],(int64_t)sizes[0])};
        coax_t_GProfile profile={.shape=tiny_shape,.pins={tiny_artifacts,1},.specs={tiny_specs,33}};
        coax_t_GResult r=g_bind_profile(profile,(coax_v_t_GFile){files,1},(coax_v_t_GBlock){blocks,36},(coax_v_t_GBlock){draft,36},(coax_v_t_GModel){&model,1},0);assert(r.status==0);
    }
    double bind_seconds=now()-start;printf("bound %.3f seconds; peak_rss_bytes=%" PRIu64 "\n",bind_seconds,rss());fflush(stdout);
    coax_t_GPlan p=g_plan(model.shape);assert(p.status==0);int64_t trace_n=p.begin.e[13]-p.begin.e[12];
    coax_t_GWork w={{alloc(p.floats,4),p.floats},{alloc(p.scores,8),p.scores},{alloc(p.sums,sizeof(coax_t_PsionicAccumulator)),p.sums},{alloc(p.routes,8),p.routes}};
    float *cache=alloc(p.cache+1,4),*logits=alloc(model.shape.vocab+1,4),*trace=alloc(trace_n+1,4),*weights=alloc(p.routes+1,4);int64_t *routes=alloc(p.routes+1,8);
    cache[p.cache]=-7;logits[model.shape.vocab]=-9;trace[trace_n]=-11;routes[p.routes]=-13;weights[p.routes]=-15;
    coax_t_GOutput out={fv(logits,model.shape.vocab+1),fv(trace,trace_n+1),iv(routes,p.routes+1),fv(weights,p.routes+1)};
    if(generating){generate(model,p,w,cache,out,output_dir);return 0;}
    int64_t original_tokens[]={200006,173781};const int64_t *tokens=original?original_tokens:tiny_tokens;int steps=original?2:4;
    for(int t=0;t<steps;t++){
        start=now();int64_t macs=g_macs(model.shape,t);coax_t_GStep r=psionic_gptoss_forward(model,tokens[t],t,macs,fv(cache,p.cache+1),out,w);
        if(r.status){fprintf(stderr,"forward refused at step %d: %u\n",t,r.status);return 3;}
        assert(r.written==model.shape.vocab && r.next==t+1 && r.macs==macs && cache[p.cache]==-7 && logits[model.shape.vocab]==-9 && trace[trace_n]==-11 && routes[p.routes]==-13 && weights[p.routes]==-15);
        printf("step=%d seconds=%.6f macs=%" PRId64 " peak_rss_bytes=%" PRIu64 "\n",t,now()-start,macs,rss());fflush(stdout);
        if(original){save(output_dir,t,"logits.f32",logits,(size_t)model.shape.vocab*4);save(output_dir,t,"trace.f32",trace,(size_t)trace_n*4);save(output_dir,t,"routes.i64",routes,(size_t)p.routes*8);save(output_dir,t,"weights.f32",weights,(size_t)p.routes*4);}
        else{close_to(logits,tiny_logits[t],16);close_to(trace,tiny_trace[t],128);close_to(weights,tiny_weights[t],4);assert(!memcmp(routes,tiny_routes[t],4*8));}
    }
    float *replay=alloc(p.cache,4);float *expected=alloc(model.shape.vocab,4);memcpy(expected,logits,(size_t)model.shape.vocab*4);
    for(int t=0;t<steps;t++){coax_t_GStep r=psionic_gptoss_forward(model,tokens[t],t,g_macs(model.shape,t),fv(replay,p.cache),out,w);assert(!r.status);}
    assert(!memcmp(expected,logits,(size_t)model.shape.vocab*4) && !memcmp(replay,cache,(size_t)p.cache*4));free(replay);free(expected);puts("fresh full-prefix replay equals retained cached step exactly");
    /* Refusals preserve live cache and every public output. */
    float *before=alloc(p.cache+1,4);memcpy(before,cache,(size_t)(p.cache+1)*4);float saved=logits[0];
    assert(psionic_gptoss_forward(model,-1,steps,INT64_MAX,fv(cache,p.cache+1),out,w).status==COAX_GStatus_token);
    assert(psionic_gptoss_forward(model,0,steps,0,fv(cache,p.cache+1),out,w).status==COAX_GStatus_budget);
    coax_t_GOutput alias=out;alias.logits=w.floats;assert(psionic_gptoss_forward(model,0,steps,INT64_MAX,fv(cache,p.cache+1),alias,w).status==COAX_GStatus_alias);
    if(!original){uint8_t *head=model.p.e[2].data;uint8_t a=head[0],b=head[1];head[0]=0x80;head[1]=0x7f;
        assert(psionic_gptoss_forward(model,0,steps,INT64_MAX,fv(cache,p.cache+1),out,w).status==COAX_GStatus_number);head[0]=a;head[1]=b;
    }
    assert(!memcmp(before,cache,(size_t)(p.cache+1)*4) && logits[0]==saved);
    printf("model passes; peak_rss_bytes=%" PRIu64 "\n",rss());
    free(before);free(cache);free(logits);free(trace);free(routes);free(weights);free(w.floats.data);free(w.scores.data);free(w.sums.data);free(w.routes.data);
    for(int i=0;i<nf;i++){if(original)assert(!munmap(maps[i],sizes[i]));else free(maps[i]);}return 0;
}
