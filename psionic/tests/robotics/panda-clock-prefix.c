/* Recorded-command/control SDK fixture. Never initializes a neural provider. */
#include "panda-session.h"
#include "panda-env.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
static unsigned checks;
#define NEED(x) do{checks++;if(!(x)){fprintf(stderr,"clock prefix line%d: %s\n",__LINE__,#x);exit(1);}}while(0)
static coax_t_PkState adopt(coax_t_PkResult r){NEED(!r.status);return r.state;}
static uint64_t bits(double x){uint64_t w;memcpy(&w,&x,8);return w;}
static unsigned char *read_all(const char*p,size_t*n){FILE*f=fopen(p,"rb");NEED(f);NEED(!fseek(f,0,SEEK_END));long z=ftell(f);NEED(z>=0);rewind(f);unsigned char*b=malloc((size_t)z+1);NEED(b);NEED(fread(b,1,(size_t)z,f)==(size_t)z&&!fclose(f));*n=(size_t)z;return b;}
static FILE *open_suffix(const char*p,const char*s){char out[4096];NEED(snprintf(out,sizeof out,"%s%s",p,s)<(int)sizeof out);FILE*f=fopen(out,"wb");NEED(f);return f;}
static coax_t_PrSample frame(const unsigned char*trace,int i){coax_t_PrRead r=pr_decode((coax_v_u8){(unsigned char*)trace+32+512*i,512});NEED(!r.status);return r.sample;}
static void snapshot(FILE*f,FILE*obs,int kind,coax_t_PkState s,const PandaTelemetry*t){
 int64_t tag[4]={kind,s.clock.now.step,s.last_forward.step,s.clock.now.step};double v[10];v[0]=t->time;memcpy(v+1,t->site_pos,24);memcpy(v+4,t->body_quat_wxyz,32);memcpy(v+8,t->finger_q,16);NEED(fwrite(tag,8,4,f)==4&&fwrite(v,8,10,f)==10);
 if(kind==0){float x[8];coax_t_PeRead r=pe_observation((coax_v_f64){(double*)t->site_pos,3},(coax_v_f64){(double*)t->body_quat_wxyz,4},(coax_v_f64){(double*)t->finger_q,2},(coax_v_f32){x,8},s.supervisor.tick!=0,0);NEED(!r.status&&fwrite(x,4,8,obs)==8);}
}
static void commands(const unsigned char*trace,int frames,int row,uint32_t words[112]){
 memset(words,0,448);for(int slot=0;slot<8;slot++){int i=(row+slot)*25;if(i<frames){coax_t_PrSample r=frame(trace,i);memcpy(words+7*slot,r.action.e,28);}}
 /* All eight unconsumed rows, and any unavailable later executed lane, are
    declared test zeroes. This is not an authentic later NN reply. */
}
int main(int argc,char**argv){
 NEED(argc==6);int scenario=atoi(argv[4]);NEED(scenario==0||scenario==2);size_t iz,tz;unsigned char*ib=read_all(argv[2],&iz),*tb=read_all(argv[3],&tz);NEED(iz==360&&tz>=64+250*512&&(tz-64)%512==0);int count=(int)((tz-64)/512);double initial[45];memcpy(initial,ib,360);
 char names[24][128];const char*bindings[24];for(int i=0;i<24;i++){coax_string x=pe_binding(i);NEED(x.count>0&&x.count<128);memcpy(names[i],x.data,x.count);names[i][x.count]=0;bindings[i]=names[i];}
 char error[2048];PandaEnv*env=panda_env_open(argv[1],bindings,error,sizeof error);if(!env)fprintf(stderr,"%s\n",error);NEED(env);PandaDimensions d;NEED(panda_env_dimensions(env,&d)&&pe_profile(d.nq,d.nv,d.nu,d.na,d.state_count,d.physics_dt));int reset=panda_env_reset(env,initial,45);coax_t_PkState s=adopt(pk_begin(initial[0],reset?0:1));
 FILE*states=open_suffix(argv[5],".states.f64"),*records=open_suffix(argv[5],".records.ptr1"),*snaps=open_suffix(argv[5],".snapshots.bin"),*obs=open_suffix(argv[5],".observations.f32"),*fixture=open_suffix(argv[5],".recorded-commands.u32"),*j=open_suffix(argv[5],".json");NEED(fwrite(initial,8,45,states)==45);
 coax_t_PlAction out[8]={0},work[8]={0};coax_t_PlTag pending={0};uint32_t pending_words[112]={0};int origins[3]={0},requests=0,holds=0,refused=0,captures=0,fresh=0;
 for(int row=0;row<10;row++){
  if(s.supervisor.phase==COAX_PlPhase_empty){PandaTelemetry t;int sampled=panda_env_sample(env,&t);if(sampled)snapshot(snaps,obs,0,s,&t);coax_t_PkResult q=pk_request(s,sampled?0:1);s=adopt(q);NEED(requests<3);origins[requests++]=(int)s.clock.now.step;captures++;uint32_t w[112];commands(tb,count,row,w);NEED(fwrite(w,4,112,fixture)==112);
   if(scenario==2&&row==0){pending=q.tag;memcpy(pending_words,w,448);}else s=adopt(pk_accept(s,q.tag,row,(coax_v_u32){w,112},(coax_v_t_PlAction){out,8},(coax_v_t_PlAction){work,8},0));
  }
  int hold=scenario==2&&row==0;coax_t_PkResult selected=pk_select(s,hold);s=adopt(selected);coax_t_PlAction chosen={0};if(!selected.fallback){NEED(selected.slot>=0&&selected.slot<8);chosen=out[selected.slot];}else holds++;
  for(int sub=0;sub<25;sub++){
   int index=row*25+sub;coax_t_PrSample original=frame(tb,index);NEED(original.tick==row&&original.substep==sub+1&&original.epoch==s.supervisor.epoch&&original.serial==s.supervisor.serial);for(int k=0;k<7;k++){uint32_t w;memcpy(&w,chosen.v.e+k,4);NEED(w==original.action.e[k]);}
   s=adopt(pk_forward(s,panda_env_forward(env)?0:1));PandaTelemetry t;int sampled=panda_env_sample(env,&t);s=adopt(pk_sample(s,sampled?0:1));if(sub==0){snapshot(snaps,obs,1,s,&t);fresh++;}s=adopt(pk_control(s,sub));
   double control[9],actual[45];memcpy(control,original.control.e,72);int integrated=panda_env_step(env,control,9)&&panda_env_state(env,actual,45);s=adopt(pk_receipt(s,row,sub+1,integrated?bits(actual[0]):0,integrated?0:1));NEED(s.supervisor.tick==row&&s.clock.now.step==index+1);NEED(!memcmp(actual,original.state.e,360));NEED(fwrite(actual,8,45,states)==45);
   coax_t_PrSample actual_record=original;actual_record.epoch=s.supervisor.epoch;actual_record.serial=s.supervisor.serial;actual_record.tick=s.supervisor.tick;actual_record.substep=sub+1;actual_record.flags=(original.flags&~1)|(selected.fallback?1:0);for(int k=0;k<7;k++)memcpy(actual_record.action.e+k,chosen.v.e+k,4);memcpy(actual_record.state.e,actual,360);unsigned char encoded[512];NEED(!pr_encode(actual_record,(coax_v_u8){encoded,512},0)&&!memcmp(encoded,tb+32+512*index,512));NEED(fwrite(encoded,1,512,records)==512);
  }
  s=adopt(pk_commit(s,25,0));NEED(s.supervisor.tick==row+1&&pk_valid(s));
  if(scenario==2&&row==0){coax_t_PkResult late=pk_accept(s,pending,s.supervisor.tick,(coax_v_u32){pending_words,112},(coax_v_t_PlAction){out,8},(coax_v_t_PlAction){work,8},0);NEED(late.status==COAX_PkStatus_stale);s=adopt(pk_refuse(late.state));refused++;}
 }
 NEED(requests==(scenario?3:2)&&holds==(scenario?1:0)&&refused==(scenario?1:0)&&s.clock.now.step==250);
 fprintf(j,"{\"schema\":\"psionic-panda-clock-prefix-v1\",\"scenario\":%d,\"steps\":250,\"rows\":10,\"requests\":%d,\"holds\":%d,\"refusals\":%d,\"cached_observations\":%d,\"fresh_observations\":%d,\"origins\":[",scenario,requests,holds,refused,captures,fresh);for(int i=0;i<requests;i++)fprintf(j,"%s%d",i?",":"",origins[i]);fprintf(j,"],\"neural_model_initialized\":false,\"controller_arithmetic_recomputed\":false,\"task_outcome_evaluated\":false,\"checks_before_close\":%u}\n",checks);
 NEED(!fclose(states)&&!fclose(records)&&!fclose(snaps)&&!fclose(obs)&&!fclose(fixture)&&!fclose(j));panda_env_close(env);free(ib);free(tb);printf("Panda clock SDK prefix: scenario%d,250 exact45F64states/512Brecords;%uchecks; no NN/provider\n",scenario,checks);return 0;
}
