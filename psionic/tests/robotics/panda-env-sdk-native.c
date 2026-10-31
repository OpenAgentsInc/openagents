/* Firm reason2: SDK effects integration fixture. Coax owns every controller
   torque and gripper value. The fixed test loop is not the product scheduler. */
#include "panda-env.h"
#include "panda-env-generated.h"
#include "panda-control.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned checks;
static void need(int condition,const char *message){checks++;if(!condition){fprintf(stderr,"SDK fixture: %s\n",message);exit(1);}}
static coax_t_PcSample pc_sample(PandaTelemetry *t){
 coax_t_PcSample s={0};memcpy(s.position.e,t->site_pos,24);memcpy(s.rotation.e,t->site_mat,72);memcpy(s.q.e,t->q,56);memcpy(s.qd.e,t->qd,56);memcpy(s.mass.e,t->mass,392);memcpy(s.jp.e,t->jp,168);memcpy(s.jr.e,t->jr,168);memcpy(s.velocity.e,t->velocity,48);memcpy(s.bias.e,t->bias,56);memcpy(s.lower.e,t->torque_min,56);memcpy(s.upper.e,t->torque_max,56);return s;
}
static void save(const char *prefix,const char *suffix,const void *data,size_t count){char path[4096];need(snprintf(path,sizeof(path),"%s%s",prefix,suffix)<(int)sizeof(path),"path");FILE *f=fopen(path,"wb");need(f&&fwrite(data,1,count,f)==count&&!fclose(f),"save");}
int main(int argc,char **argv){
 if(argc!=4){fprintf(stderr,"usage: panda-env-sdk-native XML STATE45F64 OUTPUT_PREFIX\n");return 2;}
 char names[24][96];const char *bindings[24];for(int i=0;i<24;i++){coax_string s=pe_binding(i);need(s.count>0&&s.count<96,"binding extent");memcpy(names[i],s.data,(size_t)s.count);names[i][s.count]=0;bindings[i]=names[i];}
 char error[4096];need(!panda_env_open(NULL,bindings,error,sizeof(error)),"null XML");need(!panda_env_open(argv[1],NULL,error,sizeof(error)),"null bindings");
 PandaEnv *e=panda_env_open(argv[1],bindings,error,sizeof(error));if(!e){fprintf(stderr,"%s\n",error);return 1;}
 double init[45];FILE *file=fopen(argv[2],"rb");need(file&&fread(init,8,45,file)==45&&fgetc(file)==EOF&&!fclose(file),"state input");
 need(!panda_env_reset(e,init,44),"reset extent");need(panda_env_reset(e,init,45),"reset");PandaDimensions dim;need(panda_env_dimensions(e,&dim),"dimensions");need(pe_profile(dim.nq,dim.nv,dim.nu,dim.na,dim.state_count,dim.physics_dt),"Coax model admission");
 PandaTelemetry t;need(panda_env_sample(e,&t),"initial sample");need(pe_ranges((coax_v_f64){t.torque_min,7},(coax_v_f64){t.torque_max,7},(coax_v_f64){t.finger_lower,2},(coax_v_f64){t.finger_upper,2}),"Coax actuator admission");save(argv[3],".initial-telemetry.f64",&t,sizeof(t));
 double states[26][45],controls[25][9];need(!panda_env_state(e,states[0],44),"state extent");need(panda_env_state(e,states[0],45),"initial state");need(!memcmp(init,states[0],sizeof(init)),"initial state exact");
 coax_t_PcState controller={0};coax_a7_f64 home=pc_default_home();need(pc_reset((coax_v_t_PcState){&controller,1},pc_sample(&t),(coax_v_f64){home.e,7})==0,"controller reset");
 for(int i=0;i<25;i++){
  need(panda_env_forward(e)&&panda_env_sample(e,&t),"pre-torque forward/sample");double action[7]={0};
  coax_t_PcOutput out=pc_step((coax_v_t_PcState){&controller,1},pc_sample(&t),(coax_v_f64){action,7},i==0);need(out.status==0,"Coax zero-pose/grip-hold controller");memcpy(controls[i],out.torque.e,56);memcpy(controls[i]+7,out.gripper.e,16);
  need(!panda_env_step(e,controls[i],8),"control extent");need(panda_env_step(e,controls[i],9),"one physics effect");need(panda_env_state(e,states[i+1],45),"post-step state");
 }
 need(panda_env_sample(e,&t),"stale post-step sample");save(argv[3],".stale-telemetry.f64",&t,sizeof(t));
 float observation[8];coax_t_PeRead read=pe_observation((coax_v_f64){t.site_pos,3},(coax_v_f64){t.body_quat_wxyz,4},(coax_v_f64){t.finger_q,2},(coax_v_f32){observation,8},1,0);need(read.status==0&&read.derived_lag==.002,"Coax stale observation stamp");
 save(argv[3],".observation.f32",observation,sizeof(observation));save(argv[3],".states.f64",states,sizeof(states));save(argv[3],".controls.f64",controls,sizeof(controls));
 need(panda_env_forward(e)&&panda_env_sample(e,&t),"fresh effect");save(argv[3],".fresh-telemetry.f64",&t,sizeof(t));panda_env_close(e);panda_env_close(NULL);
 printf("panda-env SDK: %u assertions;25 Coax-controlled physics steps;26 exactstate snapshots; no renderer in physics trace\n",checks);return 0;
}
