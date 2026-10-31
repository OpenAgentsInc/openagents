/* Independent SDK data capture: no downloaded Python modules, policy or
   controller. Binding strings below are authored test expectations. */
#include "panda-env.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *bindings[24]={
 "robot0_joint1","robot0_joint2","robot0_joint3","robot0_joint4","robot0_joint5","robot0_joint6","robot0_joint7",
 "robot0_torq_j1","robot0_torq_j2","robot0_torq_j3","robot0_torq_j4","robot0_torq_j5","robot0_torq_j6","robot0_torq_j7",
 "gripper0_finger_joint1","gripper0_finger_joint2","gripper0_grip_site","robot0_right_hand","desk_caddy_1_back_contain_region","black_book_1_main","agentview","robot0_eye_in_hand","gripper0_gripper_finger_joint1","gripper0_gripper_finger_joint2"};
typedef struct{double position[3],rotation[9],q[7],qd[7],mass[49],jp[21],jr[21],velocity[6],bias[7],lower[7],upper[7];} PcFixture;
static void save(const char *prefix,const char *suffix,const void *bytes,size_t n){
 char path[4096];if(snprintf(path,sizeof(path),"%s%s",prefix,suffix)>=(int)sizeof(path))exit(4);
 FILE *f=fopen(path,"wb");if(!f||fwrite(bytes,1,n,f)!=n||fclose(f))exit(4);
}
int main(int argc,char **argv){
 if(argc!=4&&argc!=5){fprintf(stderr,"usage: panda-env-capture XML STATE45F64 OUTPUT_PREFIX [--render]\n");return 2;}
 char error[4096];PandaEnv *env=panda_env_open(argv[1],bindings,error,sizeof(error));if(!env){fprintf(stderr,"SDK XML: %s\n",error);return 3;}
 double state[45];FILE *f=fopen(argv[2],"rb");if(!f||fread(state,sizeof(double),45,f)!=45||fgetc(f)!=EOF){fprintf(stderr,"state shape\n");return 3;}fclose(f);
 if(!panda_env_reset(env,state,45))return 3;PandaDimensions dim;PandaTelemetry s;
 if(!panda_env_dimensions(env,&dim)||!panda_env_sample(env,&s))return 3;
 save(argv[3],".telemetry.f64",&s,sizeof(s));PcFixture pc;
 memcpy(pc.position,s.site_pos,sizeof(pc.position));memcpy(pc.rotation,s.site_mat,sizeof(pc.rotation));
 memcpy(pc.q,s.q,sizeof(pc.q));memcpy(pc.qd,s.qd,sizeof(pc.qd));memcpy(pc.mass,s.mass,sizeof(pc.mass));
 memcpy(pc.jp,s.jp,sizeof(pc.jp));memcpy(pc.jr,s.jr,sizeof(pc.jr));memcpy(pc.velocity,s.velocity,sizeof(pc.velocity));
 memcpy(pc.bias,s.bias,sizeof(pc.bias));memcpy(pc.lower,s.torque_min,sizeof(pc.lower));memcpy(pc.upper,s.torque_max,sizeof(pc.upper));save(argv[3],".pc-sample.f64",&pc,sizeof(pc));
 if(argc==5){if(strcmp(argv[4],"--render"))return 2;unsigned char image[196608];
  if(!panda_env_render(env,0,image,sizeof(image)))return 5;save(argv[3],".agentview.rgb",image,sizeof(image));
  if(!panda_env_render(env,1,image,sizeof(image)))return 5;save(argv[3],".wrist.rgb",image,sizeof(image));}
 printf("{\"nq\":%lld,\"nv\":%lld,\"nu\":%lld,\"na\":%lld,\"state_count\":%lld,\"physics_dt\":%.17g,\"telemetry_f64_words\":%zu,\"pc_sample_f64_words\":%zu,\"rendered\":%s,\"physics_steps\":0}\n",(long long)dim.nq,(long long)dim.nv,(long long)dim.nu,(long long)dim.na,(long long)dim.state_count,dim.physics_dt,sizeof(s)/sizeof(double),sizeof(pc)/sizeof(double),argc==5?"true":"false");panda_env_close(env);return 0;
}
