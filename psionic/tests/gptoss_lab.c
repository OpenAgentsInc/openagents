/* Opt-in original-model qualification. Only owned Coax arithmetic is executed. */
#define main gptoss_format_test_main
#include "gptoss_native.c"
#undef main
typedef struct {coax_t_GModel m;coax_t_GPlan p;coax_t_GWork w;coax_t_GOutput out;float *cache;double *prob,*stage;int64_t *order,*tmp;void *maps[16];size_t sizes[16];int nf;} Host;
static Host host(uint8_t target,const char *root){
    Host h={0};coax_t_GFile files[16];h.nf=(int)g_original_files(target);coax_t_GBlock *blocks=alloc(36,sizeof *blocks),*draft=alloc(36,sizeof *draft);double begin=now();
    for(int i=0;i<h.nf;i++){coax_string name=g_original_filename(target,i);char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",root,(int)name.count,name.data)>0);h.maps[i]=load(path,1,&h.sizes[i]);files[i]=(coax_t_GFile){name,bv(h.maps[i],(int64_t)h.sizes[i])};}
    coax_t_GResult b=psionic_gptoss_bind(target,256,(coax_v_t_GFile){files,h.nf},(coax_v_t_GBlock){blocks,36},(coax_v_t_GBlock){draft,36},(coax_v_t_GModel){&h.m,1});assert(!b.status);free(draft);
    h.p=g_plan(h.m.shape);assert(!h.p.status);h.cache=alloc(h.p.cache,4);h.w=(coax_t_GWork){{alloc(h.p.floats,4),h.p.floats},{alloc(h.p.scores,8),h.p.scores},{alloc(h.p.sums,sizeof(coax_t_PsionicAccumulator)),h.p.sums},{alloc(h.p.routes,8),h.p.routes}};
    h.out.logits=fv(alloc(h.m.shape.vocab,4),h.m.shape.vocab);h.prob=alloc(h.m.shape.vocab,8);h.stage=alloc(h.m.shape.vocab,8);h.order=alloc(h.m.shape.vocab,8);h.tmp=alloc(h.m.shape.vocab,8);
    printf("admitted target=%u bytes=%lld seconds=%.6f peak_rss_bytes=%" PRIu64 "\n",target,(long long)b.bytes,now()-begin,rss());fflush(stdout);return h;
}
static void step(Host *h,int64_t token,int64_t position){coax_t_GStep s=psionic_gptoss_forward(h->m,token,position,g_macs(h->m.shape,position),fv(h->cache,h->p.cache),h->out,h->w);if(s.status)fprintf(stderr,"step status=%u pos=%lld\n",s.status,(long long)position);assert(!s.status);}
static void distribution(Host *h,double *out){coax_t_GResult r=gs_distribution(h->out.logits,bv(NULL,0),0,0,1,(coax_v_f64){out,h->m.shape.vocab},(coax_v_f64){h->stage,h->m.shape.vocab},iv(h->order,h->m.shape.vocab),iv(h->tmp,h->m.shape.vocab),0);assert(!r.status);}
static void reset(Host *h,const int64_t *tokens,int64_t n){memset(h->cache,0,(size_t)h->p.cache*4);for(int64_t i=0;i<n;i++)step(h,tokens[i],i);}
static void snapshot(Host *h,float *from,float *to){assert(!psionic_gptoss_snapshot(fv(from,h->p.cache),fv(to,h->p.cache)).status);}
static coax_v_u8 table(void){size_t n;uint8_t *p=load("psionic/fixtures/harmony/gptoss.hst",1,&n);coax_v_u8 t=bv(p,(int64_t)n);assert(hs_admit(t));return t;}
static int64_t render(coax_v_u8 table,coax_t_HmMessage *messages,int64_t n,_Bool completion,int64_t *ids){int64_t stage[16384],symbols[1024];coax_t_HmResult r=hm_render_checked(table,(coax_v_t_HmMessage){messages,n},completion,iv(ids,16384),iv(stage,16384),iv(symbols,1024),0);assert(!r.status);return r.written;}
static void speculate(const char *small,const char *large,const char *dir){
    Host d=host(0,small),t=host(1,large);int64_t v=t.m.shape.vocab;assert(d.m.shape.vocab==v);coax_v_u8 tab=table();coax_t_HmMessage message={.role=COAX_HmRole_user,.body=str("Hi")};int64_t prefix[16384];int64_t n=render(tab,&message,1,1,prefix);double begin=now();reset(&d,prefix,n);reset(&t,prefix,n);printf("shared prefill tokens=%lld seconds=%.6f\n",(long long)n,now()-begin);fflush(stdout);
    float *dc=alloc(d.p.cache,4),*tc=alloc(t.p.cache,4),*dl=alloc(v,4),*tl=alloc(v,4),*db=alloc(d.p.cache,4),*tb=alloc(t.p.cache,4);snapshot(&d,d.cache,dc);snapshot(&t,t.cache,tc);memcpy(dl,d.out.logits.data,(size_t)v*4);memcpy(tl,t.out.logits.data,(size_t)v*4);
    enum{GOAL=6};int64_t baseline[GOAL];begin=now();for(int i=0;i<GOAL;i++){distribution(&t,t.prob);baseline[i]=gx_argmax((coax_v_f64){t.prob,v});step(&t,baseline[i],n+i);}double baseline_seconds=now()-begin;printf("target-alone tokens=%d seconds=%.6f\n",GOAL,baseline_seconds);save(dir,0,"target.i64",baseline,sizeof baseline);fflush(stdout);
    double *p=alloc(5*v,8),*q=alloc(4*v,8),*residual=alloc(v,8),*scratch=alloc(v,8);int64_t proposed[4],committed[5],staged[5],history[16384];double uniform[9]={0};
    for(int mode=0;mode<4;mode++){
        int maximum=mode==0?1:mode==1?2:4,ngram=mode==3;int emitted=0,accepted=0,attempted=0,blocks=0;memcpy(history,prefix,(size_t)n*8);snapshot(&d,dc,d.cache);snapshot(&t,tc,t.cache);memcpy(d.out.logits.data,dl,(size_t)v*4);memcpy(t.out.logits.data,tl,(size_t)v*4);begin=now();
        while(emitted<GOAL){int k=maximum;if(k>GOAL-emitted-1)k=GOAL-emitted-1;snapshot(&d,d.cache,db);snapshot(&t,t.cache,tb);
            if(ngram){int actual=0;for(int i=0;i<k;i++){int64_t guess=gx_ngram(iv(history,n+emitted+i),4);if(guess<0)break;proposed[i]=guess;history[n+emitted+i]=guess;memset(q+i*v,0,(size_t)v*8);q[i*v+guess]=1;actual++;}k=actual;}
            else for(int i=0;i<k;i++){distribution(&d,q+i*v);proposed[i]=gx_argmax((coax_v_f64){q+i*v,v});step(&d,proposed[i],n+emitted+i);}
            int count=1;
            if(k){for(int i=0;i<k;i++){distribution(&t,p+i*v);step(&t,proposed[i],n+emitted+i);}distribution(&t,p+k*v);
                coax_t_GSpecBlock b=psionic_gptoss_block(v,(coax_v_f64){p,(k+1)*v},(coax_v_f64){q,k*v},iv(proposed,k),(coax_v_f64){uniform,2*k+1},1,iv(committed,5),iv(staged,5),(coax_v_f64){residual,v},(coax_v_f64){scratch,v});assert(!b.status);count=(int)b.written;accepted+=(int)b.accepted;attempted+=k;blocks++;
                snapshot(&d,db,d.cache);snapshot(&t,tb,t.cache);
            }else{distribution(&t,t.prob);committed[0]=gx_argmax((coax_v_f64){t.prob,v});}
            for(int i=0;i<count;i++){assert(emitted<GOAL && committed[i]==baseline[emitted]);history[n+emitted]=committed[i];if(!ngram)step(&d,committed[i],n+emitted);step(&t,committed[i],n+emitted);emitted++;}
        }
        double seconds=now()-begin;printf("speculation mode=%s requested_draft=%d target_batch=1 accepted=%d attempted=%d blocks=%d tokens=%d seconds=%.6f baseline_seconds=%.6f speedup=%.6f greedy_exact=yes peak_rss_bytes=%" PRIu64 "\n",ngram?"ngram":"20b",maximum,accepted,attempted,blocks,GOAL,seconds,baseline_seconds,baseline_seconds/seconds,rss());fflush(stdout);
    }
}
static void tools(uint8_t target,const char *root,const char *dir){
    Host h=host(target,root);coax_v_u8 tab=table();int64_t v=h.m.shape.vocab;int64_t ids[16384];uint8_t body[513]={0},decoded[65536],decode_work[65536],visible[65536];coax_t_HmRecord records[64],draft[64];
    coax_string alternatives[]={str("{\"a\":2,\"b\":2}")};coax_t_GTool tool={str("functions.add"),{alternatives,1}};
    coax_t_HmMessage messages[4]={{.role=COAX_HmRole_system,.body=str("Use functions.add to add numbers. Its arguments are JSON with a and b. Respond on the final channel after the result.")},{.role=COAX_HmRole_user,.body=str("What is 2 + 2? Use add.")},{.role=COAX_HmRole_assistant,.channel=COAX_HmChannel_commentary,.ending=COAX_HmEnd_call,.recipient=str("functions.add"),.body=str("")}};
    int64_t n=render(tab,messages,3,0,ids)-1; /* named tool choice: open the argument body */
    reset(&h,ids,n);coax_string *pieces=alloc(v,sizeof *pieces);uint8_t *dictionary=alloc(8388608,1);int64_t used=0;uint8_t one[1024],one_work[1024];
    for(int64_t id=0;id<199998;id++){coax_t_PtResult r=hs_decode_checked(tab,iv(&id,1),bv(one,1024),bv(one_work,1024),0);assert(!r.status && used+r.written<=8388608);memcpy(dictionary+used,one,(size_t)r.written);pieces[id]=(coax_string){(const char*)dictionary+used,r.written};used+=r.written;}
    uint8_t *mask=alloc(v,1),*mask_stage=alloc(v,1);coax_t_GSampleWork sw={{h.stage,v},{h.order,v},{h.tmp,v}};coax_t_GGeneration state={.random={48,0},.remaining=64};int64_t body_n=0;
    while(state.stop==COAX_GStop_running){coax_t_GResult m=psionic_gptoss_mask((coax_v_string){alternatives,1},(coax_string){(const char*)body,body_n},(coax_v_string){pieces,v},bv(mask,v),bv(mask_stage,v));assert(!m.status);
        coax_t_GEmission e=psionic_gptoss_emit(h.out.logits,bv(mask,v),(coax_t_GSampling){.temperature=0,.top_p=1},state,0,(coax_v_f64){h.prob,v},sw);assert(!e.status);state=e.state;ids[n++]=e.token;
        if(state.stop==COAX_GStop_running){coax_string p=pieces[e.token];assert(body_n+p.count<=512);memcpy(body+body_n,p.data,(size_t)p.count);body_n+=p.count;step(&h,e.token,n-1);}
    }
    assert(state.stop==COAX_GStop_call);coax_t_HmResult parsed=hm_parse_checked(tab,iv(ids,n),0,(coax_v_t_HmRecord){records,64},(coax_v_t_HmRecord){draft,64},bv(decoded,65536),bv(decode_work,65536),0);assert(!parsed.status);
    coax_t_GCall authorized=gt_authorize(tool,records[parsed.written-1],(coax_string){(const char*)decoded,parsed.bytes},1);assert(!authorized.status && authorized.choice==0);assert(gt_add(2,2).value==4);
    printf("forced named tool choice=functions.add schema=root-object-enum arguments=%.*s result=4 host_grant=yes\n",(int)body_n,body);fflush(stdout);save(dir,0,"call.i64",ids,(size_t)n*8);
    messages[2].body=(coax_string){(const char*)body,body_n};coax_t_GToolResponse response=gt_response(tool.name,str("4"));assert(!response.status);messages[3]=response.message;n=render(tab,messages,4,1,ids);reset(&h,ids,n);state=(coax_t_GGeneration){.random={48,0},.remaining=64};
    while(state.stop==COAX_GStop_running){coax_t_GEmission e=psionic_gptoss_emit(h.out.logits,bv(NULL,0),(coax_t_GSampling){.temperature=0,.top_p=1},state,0,(coax_v_f64){h.prob,v},sw);assert(!e.status);state=e.state;ids[n++]=e.token;if(state.stop==COAX_GStop_running)step(&h,e.token,n-1);}
    assert(state.stop==COAX_GStop_turn);parsed=hm_parse_checked(tab,iv(ids,n),0,(coax_v_t_HmRecord){records,64},(coax_v_t_HmRecord){draft,64},bv(decoded,65536),bv(decode_work,65536),0);assert(!parsed.status);
    for(int64_t i=0;i<parsed.written;i++){coax_t_GResult r=psionic_gptoss_display(records[i],(coax_string){(const char*)decoded,parsed.bytes},bv(visible,65536));if(!r.status)printf("final: %.*s\n",(int)r.written,visible);}
    save(dir,0,"resumed.i64",ids,(size_t)n*8);printf("tool roundtrip passes; analysis excluded by Coax display gate; peak_rss_bytes=%" PRIu64 "\n",rss());
}
int main(int argc,char **argv){assert(argc==5);if(!strcmp(argv[1],"speculate"))speculate(argv[2],argv[3],argv[4]);else{assert(!strcmp(argv[1],"tools"));tools(0,argv[2],argv[4]);}return 0;}
