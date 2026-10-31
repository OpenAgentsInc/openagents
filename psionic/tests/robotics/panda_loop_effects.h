/* Shared test-loop effects, extracted without changing Coax decisions. */
#ifndef PSIONIC_PANDA_LOOP_EFFECTS_H
#define PSIONIC_PANDA_LOOP_EFFECTS_H
#include "panda-session.h"
#include "panda-env.h"
#include "panda_loop_provider.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#define REQUIRE(x) do{if(!(x)){fprintf(stderr,"Panda evaluation failed %s:%d: %s\n",__FILE__,__LINE__,#x);exit(1);}}while(0)
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
static void path(char out[4096],const char *a,const char *b){REQUIRE(snprintf(out,4096,"%s/%s",a,b)<4096);}
static unsigned char *read_file(const char *name,size_t *size){FILE *f=fopen(name,"rb");REQUIRE(f);REQUIRE(!fseek(f,0,SEEK_END));long n=ftell(f);REQUIRE(n>=0);rewind(f);unsigned char *p=malloc((size_t)n+1);REQUIRE(p);REQUIRE(fread(p,1,n,f)==(size_t)n);REQUIRE(!fclose(f));p[n]=0;*size=n;return p;}
static void dump(const char *name,const void *data,size_t size){FILE *f=fopen(name,"wb");REQUIRE(f&&fwrite(data,1,size,f)==size&&!fclose(f));}
static void hash(const void *data,size_t n,unsigned char out[32]){REQUIRE(!ps_digest((coax_v_u8){(unsigned char *)data,n},(coax_v_u8){out,32},0));}
static void hex(FILE *f,const unsigned char *p,size_t n){for(size_t i=0;i<n;i++)fprintf(f,"%02x",p[i]);}
static coax_t_PcSample sample(const PandaTelemetry *t){
  coax_t_PcSample s={0};
  memcpy(s.position.e,t->site_pos,sizeof(t->site_pos));memcpy(s.rotation.e,t->site_mat,sizeof(t->site_mat));
  memcpy(s.q.e,t->q,sizeof(t->q));memcpy(s.qd.e,t->qd,sizeof(t->qd));memcpy(s.mass.e,t->mass,sizeof(t->mass));
  memcpy(s.jp.e,t->jp,sizeof(t->jp));memcpy(s.jr.e,t->jr,sizeof(t->jr));memcpy(s.velocity.e,t->velocity,sizeof(t->velocity));
  memcpy(s.bias.e,t->bias,sizeof(t->bias));memcpy(s.lower.e,t->torque_min,sizeof(t->torque_min));memcpy(s.upper.e,t->torque_max,sizeof(t->torque_max));return s;
}
static coax_t_PeSuccess success(const PandaTelemetry *t){return pe_success((coax_v_f64){(double *)t->target_pos,3},(coax_v_f64){(double *)t->target_mat,9},(coax_v_f64){(double *)t->target_size,3},(coax_v_f64){(double *)t->book_pos,3});}
static void chain(unsigned char digest[32],const unsigned char record[512]){unsigned char next[32];REQUIRE(!pr_chain((coax_v_u8){digest,32},(coax_v_u8){(unsigned char *)record,512},(coax_v_u8){next,32},0));memcpy(digest,next,32);}
static int replay(PandaEnv *env,const double initial[45],const char *trace,int count,const unsigned char seed[32],const unsigned char expected[32]){
  REQUIRE(panda_env_reset(env,initial,45));FILE *f=fopen(trace,"rb");REQUIRE(f);unsigned char digest[32],header[32];REQUIRE(fread(header,1,32,f)==32&&!memcmp(seed,header,32));memcpy(digest,seed,32);
  for(int i=0;i<count;i++){
    unsigned char record[512],actual[512];REQUIRE(fread(record,1,512,f)==512);coax_t_PrRead r=pr_decode((coax_v_u8){record,512});REQUIRE(!r.status);uint64_t start_word;memcpy(&start_word,initial,8);REQUIRE(ps_time(start_word,r.sample.state.e[0],r.sample.tick,r.sample.substep));
    double ctrl[9],state[45];memcpy(ctrl,r.sample.control.e,sizeof(ctrl));REQUIRE(panda_env_forward(env)&&panda_env_step(env,ctrl,9)&&panda_env_state(env,state,45));
    if(memcmp(state,r.sample.state.e,sizeof(state))){fprintf(stderr,"Physics replay mismatch at substep %d\n",i);fclose(f);return 0;}
    memcpy(r.sample.state.e,state,sizeof(state));REQUIRE(!pr_encode(r.sample,(coax_v_u8){actual,512},0));chain(digest,actual);
  }
  unsigned char tail[32];REQUIRE(fread(tail,1,32,f)==32&&fgetc(f)==EOF&&!fclose(f));return !memcmp(tail,expected,32)&&!memcmp(digest,expected,32);
}
static int panda_loop_cases(int argc,char**argv,const PandaLoopProvider*provider,const unsigned char*noise_raw,size_t noise_bytes,double open_seconds){
 REQUIRE(argc>=10&&provider&&provider->context&&provider->forward&&provider->error&&provider->profile&&provider->schema);const char*output=argv[5];int max_ticks=atoi(argv[7]),scenario=atoi(argv[8]);REQUIRE(max_ticks>0&&max_ticks<=720&&scenario>=0&&scenario<=4&&noise_bytes==5280*4);float noise[5280];memcpy(noise,noise_raw,sizeof(noise));char error[2048]={0};
  const char *bindings[24];char names[24][128];for(int i=0;i<24;i++){coax_string s=pe_binding(i);REQUIRE(s.count>0&&s.count<128);memcpy(names[i],s.data,s.count);names[i][s.count]=0;bindings[i]=names[i];}
  for(int c=9;c<argc;c++){
    char filename[4096],xml[4096],dir[4096];const char *base=strrchr(argv[c],'/');base=base?base+1:argv[c];char label[256];REQUIRE(snprintf(label,sizeof(label),"%s-fault%d",base,scenario)<(int)sizeof(label));path(dir,output,label);REQUIRE(!mkdir(dir,0700));
    path(xml,argv[c],"scene.xml");path(filename,argv[c],"init.f64");size_t init_bytes;unsigned char *init_raw=read_file(filename,&init_bytes);REQUIRE(init_bytes==45*8);double initial[45];uint64_t initial_words[45];memcpy(initial,init_raw,sizeof(initial));memcpy(initial_words,init_raw,sizeof(initial_words));REQUIRE(ps_initial((coax_v_u64){initial_words,45}));
    PandaEnv *env=panda_env_open(xml,bindings,error,sizeof(error));if(!env)fprintf(stderr,"%s\n",error);REQUIRE(env);PandaDimensions dims;REQUIRE(panda_env_dimensions(env,&dims)&&pe_profile(dims.nq,dims.nv,dims.nu,dims.na,dims.state_count,dims.physics_dt));
    REQUIRE(panda_env_reset(env,initial,45));PandaTelemetry t;REQUIRE(panda_env_sample(env,&t));REQUIRE(pe_ranges((coax_v_f64){t.torque_min,7},(coax_v_f64){t.torque_max,7},(coax_v_f64){t.finger_lower,2},(coax_v_f64){t.finger_upper,2}));
    coax_t_PcState controller={0};coax_a7_f64 home=pc_default_home();REQUIRE(!pc_reset((coax_v_t_PcState){&controller,1},sample(&t),(coax_v_f64){home.e,7}));
    unsigned char identities[128],seed[32],digest[32];size_t xml_bytes;unsigned char *xml_raw=read_file(xml,&xml_bytes);hash(xml_raw,xml_bytes,identities);hash(init_raw,init_bytes,identities+32);hash(noise_raw,noise_bytes,identities+64);coax_string identity={provider->profile,(int64_t)strlen(provider->profile)};hash(identity.data,identity.count,identities+96);hash(identities,sizeof(identities),seed);memcpy(digest,seed,32);free(xml_raw);free(init_raw);
    char trace[4096];path(trace,dir,"physics.ptr");FILE *log=fopen(trace,"wb");REQUIRE(log&&fwrite(seed,1,32,log)==32);
    coax_t_PlState h=pl_reset_done(pl_new(),0).state;coax_t_PlAction actions[8]={0},work[8]={0};coax_t_PlTag pending_tag={0};uint32_t pending_words[112]={0};int pending=0,steps=0,calls=0,clipped=0;double gpu=0,total_infer=0,max_infer=0;double start=now();
    while(h.tick<max_ticks){
      coax_t_PsEffect effect=ps_effect(h,scenario);if(effect==COAX_PsEffect_stop)break;
      if(effect==COAX_PsEffect_infer){
        coax_t_PlResult request=pl_request(h);REQUIRE(!request.status);h=request.state;REQUIRE(panda_env_sample(env,&t));float physical[8];coax_t_PeRead obs=pe_observation((coax_v_f64){t.site_pos,3},(coax_v_f64){t.body_quat_wxyz,4},(coax_v_f64){t.finger_q,2},(coax_v_f32){physical,8},h.tick!=0,0);REQUIRE(!obs.status);
        unsigned char *cameras=malloc(393216);REQUIRE(cameras&&panda_env_render(env,0,cameras,196608)&&panda_env_render(env,1,cameras+196608,196608));
        float decoded[112];PandaPolicyTiming timing;int status=provider->forward(provider->context,cameras,cameras+196608,physical,noise,decoded,&timing);free(cameras);if(!status)fprintf(stderr,"%s\n",provider->error(provider->context));REQUIRE(status&&ps_tokens(timing.tokens));calls++;gpu+=timing.vision_gpu_seconds+timing.text_gpu_seconds+timing.action_gpu_seconds;total_infer+=timing.total_seconds;if(timing.total_seconds>max_infer)max_infer=timing.total_seconds;
        uint32_t words[112];memcpy(words,decoded,sizeof(words));for(int i=0;i<112;i++)words[i]=ps_word(scenario,h.tick,i,words[i]);
        if(calls==1){path(filename,dir,"first-decoded.f32");dump(filename,decoded,sizeof(decoded));}
        if(ps_delayed(scenario,h.tick)){pending_tag=request.tag;memcpy(pending_words,words,sizeof(words));pending=1;}
        else {coax_t_PlResult admitted=pl_accept(h,request.tag,h.tick,(coax_v_u32){words,112},(coax_v_t_PlAction){actions,8},(coax_v_t_PlAction){work,8},0);if(!admitted.status){h=admitted.state;continue;}REQUIRE(admitted.status==COAX_PlStatus_limit);coax_t_PlResult refused=pl_refuse(h);REQUIRE(!refused.status);h=refused.state;}
        effect=COAX_PsEffect_hold;
      }
      coax_t_PlResult selected=effect==COAX_PsEffect_hold?pl_hold(h,h.tick):pl_choose(h,h.tick);REQUIRE(!selected.status);h=selected.state;coax_t_PlAction chosen={0};if(!selected.fallback){REQUIRE(selected.slot>=0&&selected.slot<8);chosen=actions[selected.slot];}coax_a7_f64 action=ps_action(chosen);int actual_steps=0,io_status=0;
      for(int substep=0;substep<ps_substeps();substep++){
        if(!panda_env_forward(env)||!panda_env_sample(env,&t)){io_status=1;break;}
        coax_t_PcSample measured=ps_sample(scenario,h.tick,substep,sample(&t));coax_t_PcOutput out=ps_control((coax_v_t_PcState){&controller,1},measured,(coax_v_f64){action.e,7},substep);if(out.status){fprintf(stderr,"Controller refused %u at tick %lld substep %d\n",out.status,(long long)h.tick,substep);io_status=1;break;}
        double ctrl[9],state[45];memcpy(ctrl,out.torque.e,7*8);memcpy(ctrl+7,out.gripper.e,2*8);if(!panda_env_step(env,ctrl,9)||!panda_env_state(env,state,45)){io_status=1;break;}actual_steps++;uint64_t clock_word;memcpy(&clock_word,state,8);if(!ps_time(initial_words[0],clock_word,h.tick,substep+1)){io_status=1;break;}steps++;clipped+=out.torque_clipped;
        coax_t_PrSample record={.epoch=h.epoch,.serial=h.serial,.tick=h.tick,.substep=substep+1,.flags=ps_flags(selected.fallback,out.torque_clipped,0)};memcpy(record.action.e,chosen.v.e,7*4);memcpy(record.control.e,ctrl,sizeof(ctrl));memcpy(record.state.e,state,sizeof(state));unsigned char encoded[512];REQUIRE(!pr_encode(record,(coax_v_u8){encoded,512},0)&&fwrite(encoded,1,512,log)==512);chain(digest,encoded);
      }
      coax_t_PlResult committed=pl_commit(h,h.tick,actual_steps,io_status);REQUIRE(!committed.status);h=committed.state;
      if(pending){coax_t_PlResult late=pl_accept(h,pending_tag,h.tick,(coax_v_u32){pending_words,112},(coax_v_t_PlAction){actions,8},(coax_v_t_PlAction){work,8},0);REQUIRE(late.status==COAX_PlStatus_stale);late=pl_refuse(h);REQUIRE(!late.status);h=late.state;pending=0;}
      REQUIRE(panda_env_sample(env,&t));coax_t_PeSuccess outcome=success(&t);REQUIRE(!outcome.status);if(outcome.success){coax_t_PlResult finished=pl_finish(h,1);REQUIRE(!finished.status);h=finished.state;}
    }
    double wall=now()-start;REQUIRE(fwrite(digest,1,32,log)==32&&!fclose(log));int exact=replay(env,initial,trace,steps,seed,digest);REQUIRE(exact);
    path(filename,dir,"record.json");FILE *r=fopen(filename,"w");REQUIRE(r);coax_string profile={provider->profile,(int64_t)strlen(provider->profile)};char gpu_text[64];if(provider->gpu_timing_measured)snprintf(gpu_text,sizeof(gpu_text),"%.9g",gpu);else strcpy(gpu_text,"null");fprintf(r,"{\n  \"schema\": \"%s\",\n  \"profile\": \"%.*s\",\n  \"case\": \"%s\",\n  \"scenario\": %d,\n  \"model_calls\": %d,\n  \"ticks\": %lld,\n  \"physics_steps\": %d,\n  \"success\": %s,\n  \"fallbacks\": %lld,\n  \"refusals\": %lld,\n  \"torques_clipped\": %d,\n  \"physics_replay_exact\": %s,\n  \"wall_seconds\": %.9g,\n  \"model_context_open_seconds\": %.9g,\n  \"inference_seconds\": %.9g,\n  \"inference_max_seconds\": %.9g,\n  \"gpu_seconds\": %s,\n  \"trace_records\": %d,\n  \"trace_seed_sha256\": \"",provider->schema,(int)profile.count,profile.data,base,scenario,calls,(long long)h.tick,steps,h.success?"true":"false",(long long)h.fallbacks,(long long)h.refusals,clipped,exact?"true":"false",wall,open_seconds,total_infer,max_infer,gpu_text,steps);hex(r,seed,32);fprintf(r,"\",\n  \"trace_final_sha256\": \"");hex(r,digest,32);fprintf(r,"\"\n}\n");REQUIRE(!fclose(r));printf("%s fault%d: ticks=%lld calls=%d success=%d holds=%lld refusals=%lld clip=%d exact-replay=%d wall=%.3fs\n",base,scenario,(long long)h.tick,calls,h.success,(long long)h.fallbacks,(long long)h.refusals,clipped,exact,wall);fflush(stdout);panda_env_close(env);
  }
  return 0;
}
#endif
