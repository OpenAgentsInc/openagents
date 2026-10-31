/* NEW recorded-action controlled prefix. No copied old actuator controls,
   neural provider, renderer or task-outcome evaluation. */
#include "panda-query-session.h"
#include "panda-env.h"
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
_Static_assert(sizeof(PandaTelemetry)==173*8&&offsetof(PandaTelemetry,site_pos)==106*8&&offsetof(PandaTelemetry,body_quat_wxyz)==124*8&&offsetof(PandaTelemetry,finger_q)==128*8,"published telemetry ABI unchanged");
_Static_assert(sizeof(PandaRobotStateV1)==18*8&&offsetof(PandaRobotStateV1,qd)==9*8,"new named9q/9v effect ABI");
static size_t checks;static int failed;
#define NEED(x) do{checks++;if(!(x)){fprintf(stderr,"owned query prefix line%d: %s\n",__LINE__,#x);exit(1);}}while(0)
static void adopt(coax_t_PkState*s,coax_t_PkResult r){*s=r.state;NEED(r.status==COAX_PkStatus_ok);}
static uint64_t bits(double x){uint64_t w;memcpy(&w,&x,8);return w;}
static unsigned char*read_all(const char*p,size_t*n){FILE*f=fopen(p,"rb");NEED(f);NEED(!fseek(f,0,SEEK_END));long z=ftell(f);NEED(z>=0);rewind(f);unsigned char*b=malloc((size_t)z+1);NEED(b&&fread(b,1,(size_t)z,f)==(size_t)z&&!fclose(f));*n=(size_t)z;return b;}
static FILE*open_suffix(const char*p,const char*s){char out[4096];NEED(snprintf(out,sizeof out,"%s%s",p,s)<(int)sizeof out);FILE*f=fopen(out,"wbx");NEED(f);return f;}
static coax_t_PrSample frame(const unsigned char*raw,int i){coax_t_PrRead r=pr_decode((coax_v_u8){(unsigned char*)raw+32+512*i,512});NEED(!r.status);return r.sample;}
static void commands(const unsigned char*raw,int count,int row,uint32_t w[112]){memset(w,0,448);for(int slot=0;slot<8;slot++){int i=(row+slot)*25;if(i<count){coax_t_PrSample r=frame(raw,i);memcpy(w+7*slot,r.action.e,28);}}}
static void snapshot(FILE*f,FILE*obs,int kind,coax_t_PkState s,const PandaTelemetry*t){
 int64_t tag[4]={kind,s.clock.now.step,s.last_forward.step,s.clock.now.step};double v[10];v[0]=t->time;memcpy(v+1,t->site_pos,24);memcpy(v+4,t->body_quat_wxyz,32);memcpy(v+8,t->finger_q,16);for(int i=0;i<10;i++)NEED(isfinite(v[i]));NEED(fwrite(tag,8,4,f)==4&&fwrite(v,8,10,f)==10);
 if(kind==0){float x[8];coax_t_PeRead r=pe_observation((coax_v_f64){(double*)t->site_pos,3},(coax_v_f64){(double*)t->body_quat_wxyz,4},(coax_v_f64){(double*)t->finger_q,2},(coax_v_f32){x,8},s.supervisor.tick!=0,0);NEED(!r.status&&fwrite(x,4,8,obs)==8);}
}
static int query(PandaEnv*env,int kind,const PandaTelemetry*t,int sampled,coax_t_PcSample*out,coax_t_PcSample*shadow){
 PandaRobotStateV1 robot={0};double before[45],after[45];int state_ok=panda_env_state(env,before,45);int read=panda_env_robot_state(env,&robot);int after_ok=panda_env_state(env,after,45);NEED(state_ok&&after_ok&&!memcmp(before,after,sizeof before));
 /* This assertion qualifies the pinned compiled model's first-nine qpos/DOF addresses; the effect itself resolves named joints. */
 if(read){NEED(!memcmp(robot.q,before+1,9*8)&&!memcmp(robot.qd,before+24,9*8));}
 uint64_t telemetry[173],q[9],v[9];memcpy(telemetry,t,sizeof telemetry);memcpy(q,robot.q,sizeof q);memcpy(v,robot.qd,sizeof v);
 coax_t_PqsResult a=pqs_select(kind,(coax_v_u64){telemetry,173},(coax_v_u64){q,9},(coax_v_u64){v,9},sampled?0:1,read?0:1,(coax_v_t_PcSample){out,1});
 coax_t_PqsResult b=pqs_select(1-kind,(coax_v_u64){telemetry,173},(coax_v_u64){q,9},(coax_v_u64){v,9},sampled?0:1,read?0:1,(coax_v_t_PcSample){shadow,1});
 if(a.code||b.code){fprintf(stderr,"source refused mode%d primary%u/%u shadow%u/%u\n",kind,a.code,a.detail,b.code,b.detail);failed=1;return 0;}return 1;
}
static void controller_record(FILE*f,int index,const coax_t_PcOutput*a,const coax_t_PcOutput*b,const coax_t_PcState*x,const coax_t_PcState*y){
 int64_t meta[9]={index,a->status,b->status,a->torque_clipped,b->torque_clipped,a->action_clipped,b->action_clipped,x->ready,y->ready};double values[60];memcpy(values,a->torque.e,56);memcpy(values+7,a->gripper.e,16);memcpy(values+9,x->goal_position.e,24);memcpy(values+12,x->goal_rotation.e,72);memcpy(values+21,x->home.e,56);memcpy(values+28,x->grip.e,16);memcpy(values+30,b->torque.e,56);memcpy(values+37,b->gripper.e,16);memcpy(values+39,y->goal_position.e,24);memcpy(values+42,y->goal_rotation.e,72);memcpy(values+51,y->home.e,56);memcpy(values+58,y->grip.e,16);for(int i=0;i<60;i++)NEED(isfinite(values[i]));NEED(fwrite(meta,8,9,f)==9&&fwrite(values,8,60,f)==60);
}
int main(int argc,char**argv){
 NEED(argc==6);int mode=atoi(argv[4]);NEED(mode==0||mode==1);size_t iz,tz;unsigned char*ib=read_all(argv[2],&iz),*tb=read_all(argv[3],&tz);NEED(iz==360&&tz>=64+250*512&&(tz-64)%512==0);int count=(int)((tz-64)/512);double initial[45];memcpy(initial,ib,360);
 char names[24][128];const char*bindings[24];for(int i=0;i<24;i++){coax_string x=pe_binding(i);NEED(x.count>0&&x.count<128);memcpy(names[i],x.data,x.count);names[i][x.count]=0;bindings[i]=names[i];}
 char error[2048];PandaEnv*env=panda_env_open(argv[1],bindings,error,sizeof error);if(!env)fprintf(stderr,"%s\n",error);NEED(env);PandaDimensions d;NEED(panda_env_dimensions(env,&d)&&pe_profile(d.nq,d.nv,d.nu,d.na,d.state_count,d.physics_dt));int reset=panda_env_reset(env,initial,45);coax_t_PkResult begin=pk_begin(initial[0],reset?0:1);coax_t_PkState s=begin.state;NEED(!begin.status);
 PandaRobotStateV1 poison,prior;memset(&poison,0xa5,sizeof poison);memcpy(&prior,&poison,sizeof prior);NEED(!panda_env_robot_state(NULL,&poison)&&!memcmp(&poison,&prior,sizeof prior)&&!panda_env_robot_state(env,NULL));
 PandaTelemetry t={0};int sampled=panda_env_sample(env,&t);coax_t_PcSample current,other;NEED(query(env,mode,&t,sampled,&current,&other));coax_t_PcState controller={0},shadow={0};coax_a7_f64 home=pc_default_home();NEED(!pc_reset((coax_v_t_PcState){&controller,1},current,(coax_v_f64){home.e,7})&&!pc_reset((coax_v_t_PcState){&shadow,1},other,(coax_v_f64){home.e,7}));
 FILE*states=open_suffix(argv[5],".states.f64"),*records=open_suffix(argv[5],".records.ptr1"),*snaps=open_suffix(argv[5],".snapshots.bin"),*obs=open_suffix(argv[5],".observations.f32"),*controls=open_suffix(argv[5],".controls.f64"),*paired=open_suffix(argv[5],".controllers.bin"),*j=open_suffix(argv[5],".json");NEED(fwrite(initial,8,45,states)==45);
 coax_t_PlAction out[8]={0},work[8]={0};coax_t_PlTag pending={0};uint32_t pending_words[112]={0};int origins[3]={0},requests=0,holds=0,refused=0,captures=0,fresh=0;
 for(int row=0;row<10;row++){
  if(s.supervisor.phase==COAX_PlPhase_empty){t=(PandaTelemetry){0};int captured=panda_env_sample(env,&t);if(captured)snapshot(snaps,obs,0,s,&t);coax_t_PkResult request=pk_request(s,captured?0:1);adopt(&s,request);NEED(requests<3);origins[requests++]=(int)s.clock.now.step;captures++;uint32_t words[112];commands(tb,count,row,words);if(row==0){pending=request.tag;memcpy(pending_words,words,448);}else adopt(&s,pk_accept(s,request.tag,row,(coax_v_u32){words,112},(coax_v_t_PlAction){out,8},(coax_v_t_PlAction){work,8},0));}
  coax_t_PkResult selected=pk_select(s,row==0);adopt(&s,selected);coax_t_PlAction chosen={0};if(!selected.fallback){NEED(selected.slot>=0&&selected.slot<8);chosen=out[selected.slot];}else holds++;coax_a7_f64 action=ps_action(chosen);
  for(int sub=0;sub<25;sub++){
   int index=row*25+sub;coax_t_PrSample original=frame(tb,index);for(int k=0;k<7;k++){uint32_t word;memcpy(&word,chosen.v.e+k,4);NEED(word==original.action.e[k]);}
   adopt(&s,pk_forward(s,panda_env_forward(env)?0:1));t=(PandaTelemetry){0};sampled=panda_env_sample(env,&t);int queried=query(env,mode,&t,sampled,&current,&other);adopt(&s,pk_sample(s,queried?0:1));if(sub==0){snapshot(snaps,obs,1,s,&t);fresh++;}adopt(&s,pk_control(s,sub));
   coax_t_PcOutput a=ps_control((coax_v_t_PcState){&controller,1},current,(coax_v_f64){action.e,7},sub),b=ps_control((coax_v_t_PcState){&shadow,1},other,(coax_v_f64){action.e,7},sub);controller_record(paired,index,&a,&b,&controller,&shadow);if(a.status||b.status){s=pk_abort(s).state;failed=1;fprintf(stderr,"controller refused %u/%u at%d\n",a.status,b.status,index);NEED(!failed);}
   double control[9],actual[45];memcpy(control,a.torque.e,56);memcpy(control+7,a.gripper.e,16);NEED(fwrite(control,8,9,controls)==9);int integrated=panda_env_step(env,control,9)&&panda_env_state(env,actual,45);adopt(&s,pk_receipt(s,row,sub+1,integrated?bits(actual[0]):0,integrated?0:1));NEED(s.clock.now.step==index+1&&s.supervisor.tick==row);for(int i=0;i<45;i++)NEED(isfinite(actual[i]));NEED(fwrite(actual,8,45,states)==45);
   coax_t_PrSample record={.epoch=s.supervisor.epoch,.serial=s.supervisor.serial,.tick=s.supervisor.tick,.substep=sub+1,.flags=ps_flags(selected.fallback,a.torque_clipped,0)};memcpy(record.action.e,chosen.v.e,28);memcpy(record.control.e,control,72);memcpy(record.state.e,actual,360);unsigned char encoded[512];NEED(!pr_encode(record,(coax_v_u8){encoded,512},0)&&fwrite(encoded,1,512,records)==512);
  }
  adopt(&s,pk_commit(s,25,0));if(row==0){coax_t_PkResult late=pk_accept(s,pending,s.supervisor.tick,(coax_v_u32){pending_words,112},(coax_v_t_PlAction){out,8},(coax_v_t_PlAction){work,8},0);s=late.state;NEED(late.status==COAX_PkStatus_stale);adopt(&s,pk_refuse(s));refused++;}
 }
 NEED(!failed&&requests==3&&origins[0]==0&&origins[1]==25&&origins[2]==225&&holds==1&&refused==1&&s.clock.now.step==250);fprintf(j,"{\"schema\":\"psionic-panda-query-source-prefix-v1\",\"mode\":%d,\"steps\":250,\"requests\":3,\"origins\":[%d,%d,%d],\"holds\":1,\"stale_refusals\":1,\"cached_observations\":%d,\"fresh_observations\":%d,\"controller_recomputed\":true,\"SDK_geometry_retained\":true,\"NN\":false,\"render\":false,\"task_outcome_evaluated\":false,\"checks\":%zu}\n",mode,origins[0],origins[1],origins[2],captures,fresh,checks);
 NEED(!fclose(states)&&!fclose(records)&&!fclose(snaps)&&!fclose(obs)&&!fclose(controls)&&!fclose(paired)&&!fclose(j));panda_env_close(env);free(ib);free(tb);puts("NEW recomputed query source prefix:250steps,one source mode,zeroNN/render");return 0;
}
