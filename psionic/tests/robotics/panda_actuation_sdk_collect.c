/* Independent installed-SDK effect collector. No owned actuation formulas.
   Metadata/support admission precedes every one of the eighteen forwards. */
#include <mujoco/mujoco.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
_Static_assert(sizeof(mjtNum)==8&&sizeof(double)==8&&DBL_MANT_DIG==53&&FLT_RADIX==2,"F64 SDK profile");
static size_t checks;
#define NEED(x) do{checks++;if(!(x)){fprintf(stderr,"actuation SDK collector line%d: %s\n",__LINE__,#x);exit(2);}}while(0)
static const char*const joint_names[9]={"robot0_joint1","robot0_joint2","robot0_joint3","robot0_joint4","robot0_joint5","robot0_joint6","robot0_joint7","gripper0_finger_joint1","gripper0_finger_joint2"};
static const char*const actuator_names[9]={"robot0_torq_j1","robot0_torq_j2","robot0_torq_j3","robot0_torq_j4","robot0_torq_j5","robot0_torq_j6","robot0_torq_j7","gripper0_gripper_finger_joint1","gripper0_gripper_finger_joint2"};
static void text(FILE*f,const char*s){NEED(fputc('"',f)!=EOF);for(;s&&*s;s++){unsigned char c=(unsigned char)*s;if(c=='"'||c=='\\'){NEED(fputc('\\',f)!=EOF);NEED(fputc(c,f)!=EOF);}else if(c<32){NEED(fprintf(f,"\\u%04x",c)>0);}else NEED(fputc(c,f)!=EOF);}NEED(fputc('"',f)!=EOF);}
static void nums(FILE*f,const mjtNum*v,int n){NEED(fputc('[',f)!=EOF);for(int i=0;i<n;i++){NEED(isfinite(v[i]));if(i)NEED(fputc(',',f)!=EOF);NEED(fprintf(f,"%.17g",v[i])>0);}NEED(fputc(']',f)!=EOF);}
static void vec(FILE*f,const char*key,const mjtNum*v,int n){NEED(fprintf(f,",\"%s\":",key)>0);nums(f,v,n);}
static void name(FILE*f,const mjModel*m,int kind,int id){text(f,mj_id2name(m,kind,id));}
static int callbacks_null(void){return !mjcb_passive&&!mjcb_control&&!mjcb_contactfilter&&!mjcb_sensor&&!mjcb_time&&!mjcb_act_dyn&&!mjcb_act_gain&&!mjcb_act_bias;}
static void support(const mjModel*m,int joint[9],int act[9]){
 NEED(mj_version()==331&&m->nq==23&&m->nv==21&&m->nu==9&&m->na==0&&m->njnt==11&&m->nbody==26&&m->nJmom>=9);
 NEED(m->nplugin==0&&m->ntendon==0&&m->nflex==0&&callbacks_null());NEED(m->opt.disableactuator==0&&!(m->opt.disableflags&(mjDSBL_PASSIVE|mjDSBL_ACTUATION|mjDSBL_CLAMPCTRL)));
 for(int b=0;b<m->nbody;b++)NEED(m->body_gravcomp[b]==0);
 for(int j=0;j<m->njnt;j++)NEED(m->jnt_stiffness[j]==0&&!m->jnt_actfrclimited[j]&&!m->jnt_actgravcomp[j]);
 const double damping[9]={.1,.1,.1,.1,.1,.01,.01,100,100};
 for(int i=0;i<9;i++){
  int j=mj_name2id(m,mjOBJ_JOINT,joint_names[i]),a=mj_name2id(m,mjOBJ_ACTUATOR,actuator_names[i]);NEED(j==i&&a==i);joint[i]=j;act[i]=a;
  NEED(m->jnt_type[j]==(i<7?mjJNT_HINGE:mjJNT_SLIDE)&&m->jnt_qposadr[j]==i&&m->jnt_dofadr[j]==i&&m->dof_damping[i]==damping[i]);
  NEED(m->actuator_trntype[a]==mjTRN_JOINT&&m->actuator_trnid[2*a]==j);for(int k=0;k<6;k++)NEED(m->actuator_gear[6*a+k]==(k==0?1:0));
  NEED(m->actuator_dyntype[a]==mjDYN_NONE&&m->actuator_actadr[a]==-1&&m->actuator_actnum[a]==0&&!m->actuator_actearly[a]&&!m->actuator_actlimited[a]&&m->actuator_plugin[a]==-1);
  NEED(m->actuator_gaintype[a]==mjGAIN_FIXED&&m->actuator_gainprm[mjNGAIN*a]==(i<7?1:1000)&&m->actuator_biastype[a]==(i<7?mjBIAS_NONE:mjBIAS_AFFINE));
  if(i>=7){NEED(m->actuator_biasprm[mjNBIAS*a]==0&&m->actuator_biasprm[mjNBIAS*a+1]==-1000&&m->actuator_biasprm[mjNBIAS*a+2]==0);}
  double lo=i<7?-(i<5?80:12):(i==7?0:-.04),hi=i<7?(i<5?80:12):(i==7?.04:0);
  NEED(m->actuator_ctrllimited[a]&&m->actuator_ctrlrange[2*a]==lo&&m->actuator_ctrlrange[2*a+1]==hi);
  NEED(m->actuator_forcelimited[a]==(i>=7));if(i>=7)NEED(m->actuator_forcerange[2*a]==-20&&m->actuator_forcerange[2*a+1]==20);
  NEED(m->actuator_group[a]>=0&&m->actuator_group[a]<31);
  for(int k=0;k<mjNDYN;k++)NEED(isfinite(m->actuator_dynprm[mjNDYN*a+k]));for(int k=0;k<mjNGAIN;k++)NEED(isfinite(m->actuator_gainprm[mjNGAIN*a+k]));for(int k=0;k<mjNBIAS;k++)NEED(isfinite(m->actuator_biasprm[mjNBIAS*a+k]));
 }
}
static void joint_metadata(FILE*f,const mjModel*m,int j,int index){
 int qa=m->jnt_qposadr[j],da=m->jnt_dofadr[j];NEED(fprintf(f,"{\"index\":%d,\"id\":%d,\"name\":",index,j)>0);name(f,m,mjOBJ_JOINT,j);
 NEED(fprintf(f,",\"type\":%d,\"body_id\":%d,\"qpos_adr\":%d,\"dof_adr\":%d,\"stiffness\":%.17g,\"damping\":%.17g,\"frictionloss\":%.17g,\"armature\":%.17g,\"qpos0_reference\":%.17g,\"qpos_spring_reference\":%.17g,\"actuator_force_limited\":%d,\"actuator_gravcomp\":%d",(int)m->jnt_type[j],m->jnt_bodyid[j],qa,da,m->jnt_stiffness[j],m->dof_damping[da],m->dof_frictionloss[da],m->dof_armature[da],m->qpos0[qa],m->qpos_spring[qa],(int)m->jnt_actfrclimited[j],(int)m->jnt_actgravcomp[j])>0);
 vec(f,"axis",m->jnt_axis+3*j,3);vec(f,"actuator_force_range",m->jnt_actfrcrange+2*j,2);NEED(fputc('}',f)!=EOF);
}
static void metadata(const char*path,const mjModel*m,const int joint[9],const int act[9]){
 FILE*f=fopen(path,"wbx");NEED(f);
 NEED(fprintf(f,"{\"schema\":\"psionic.panda-actuation.sdk-model.v1\",\"sdk_version_number\":%d,\"model\":{\"nq\":%d,\"nv\":%d,\"nu\":%d,\"na\":%d,\"njnt\":%d,\"nbody\":%d,\"ngeom\":%d,\"nJmom\":%d,\"nplugin\":%d,\"ntendon\":%d,\"nflex\":%d},\"options\":{\"disableflags\":%d,\"enableflags\":%d,\"disableactuator\":%d,\"density\":%.17g,\"viscosity\":%.17g,\"timestep\":%.17g,\"integrator\":%d",mj_version(),m->nq,m->nv,m->nu,m->na,m->njnt,m->nbody,m->ngeom,m->nJmom,m->nplugin,m->ntendon,m->nflex,m->opt.disableflags,m->opt.enableflags,m->opt.disableactuator,m->opt.density,m->opt.viscosity,m->opt.timestep,m->opt.integrator)>0);
 vec(f,"wind",m->opt.wind,3);vec(f,"gravity",m->opt.gravity,3);NEED(fprintf(f,"},\"callbacks\":{\"control\":%s,\"passive\":%s,\"contactfilter\":%s,\"sensor\":%s,\"time\":%s,\"act_dyn\":%s,\"act_gain\":%s,\"act_bias\":%s}",!mjcb_control?"true":"false",!mjcb_passive?"true":"false",!mjcb_contactfilter?"true":"false",!mjcb_sensor?"true":"false",!mjcb_time?"true":"false",!mjcb_act_dyn?"true":"false",!mjcb_act_gain?"true":"false",!mjcb_act_bias?"true":"false")>0);vec(f,"qpos0",m->qpos0,m->nq);vec(f,"qpos_spring",m->qpos_spring,m->nq);
 NEED(fprintf(f,",\"joints\":[")>0);for(int i=0;i<9;i++){if(i)NEED(fputc(',',f)!=EOF);joint_metadata(f,m,joint[i],i);}NEED(fprintf(f,"],\"all_joint_facts\":[")>0);for(int i=0;i<m->njnt;i++){if(i)NEED(fputc(',',f)!=EOF);joint_metadata(f,m,i,i);}NEED(fprintf(f,"],\"actuators\":[")>0);
 for(int i=0;i<9;i++){
  int a=act[i];if(i)NEED(fputc(',',f)!=EOF);NEED(fprintf(f,"{\"index\":%d,\"id\":%d,\"name\":",i,a)>0);name(f,m,mjOBJ_ACTUATOR,a);
  NEED(fprintf(f,",\"transmission_type\":%d,\"transmission_ids\":[%d,%d],\"dynamic_type\":%d,\"gain_type\":%d,\"bias_type\":%d,\"activation_adr\":%d,\"activation_count\":%d,\"activation_early\":%d,\"activation_limited\":%d,\"control_limited\":%d,\"force_limited\":%d,\"group\":%d,\"plugin\":%d",m->actuator_trntype[a],m->actuator_trnid[2*a],m->actuator_trnid[2*a+1],m->actuator_dyntype[a],m->actuator_gaintype[a],m->actuator_biastype[a],m->actuator_actadr[a],m->actuator_actnum[a],m->actuator_actearly[a],m->actuator_actlimited[a],m->actuator_ctrllimited[a],m->actuator_forcelimited[a],m->actuator_group[a],m->actuator_plugin[a])>0);
  vec(f,"gear",m->actuator_gear+6*a,6);vec(f,"dynamic_params",m->actuator_dynprm+mjNDYN*a,mjNDYN);vec(f,"gain_params",m->actuator_gainprm+mjNGAIN*a,mjNGAIN);vec(f,"bias_params",m->actuator_biasprm+mjNBIAS*a,mjNBIAS);vec(f,"control_range",m->actuator_ctrlrange+2*a,2);vec(f,"force_range",m->actuator_forcerange+2*a,2);vec(f,"activation_range",m->actuator_actrange+2*a,2);NEED(fputc('}',f)!=EOF);
 }
 NEED(fprintf(f,"],\"bodies\":[")>0);
 for(int b=0;b<m->nbody;b++){
  if(b)NEED(fputc(',',f)!=EOF);NEED(fprintf(f,"{\"id\":%d,\"name\":",b)>0);name(f,m,mjOBJ_BODY,b);
  NEED(fprintf(f,",\"parent_id\":%d,\"root_id\":%d,\"geom_adr\":%d,\"geom_count\":%d,\"dof_adr\":%d,\"dof_count\":%d,\"mass\":%.17g,\"gravcomp\":%.17g",m->body_parentid[b],m->body_rootid[b],m->body_geomadr[b],m->body_geomnum[b],m->body_dofadr[b],m->body_dofnum[b],m->body_mass[b],m->body_gravcomp[b])>0);vec(f,"inertia",m->body_inertia+3*b,3);vec(f,"inertial_position",m->body_ipos+3*b,3);vec(f,"inertial_quaternion_wxyz",m->body_iquat+4*b,4);NEED(fputc('}',f)!=EOF);
 }
 NEED(fprintf(f,"],\"geoms\":[")>0);
 for(int g=0;g<m->ngeom;g++){
  if(g)NEED(fputc(',',f)!=EOF);NEED(fprintf(f,"{\"id\":%d,\"name\":",g)>0);name(f,m,mjOBJ_GEOM,g);NEED(fprintf(f,",\"body_id\":%d,\"type\":%d",m->geom_bodyid[g],m->geom_type[g])>0);vec(f,"size",m->geom_size+3*g,3);vec(f,"position",m->geom_pos+3*g,3);vec(f,"quaternion_wxyz",m->geom_quat+4*g,4);vec(f,"fluid12",m->geom_fluid+mjNFLUID*g,mjNFLUID);NEED(fputc('}',f)!=EOF);
 }
 NEED(fprintf(f,"]}\n")>0);NEED(!fclose(f));
}
int main(int argc,char**argv){
 NEED(argc==4||argc==7);int capture=!strcmp(argv[1],"capture");NEED((capture&&argc==7)||(!strcmp(argv[1],"metadata")&&argc==4));uint16_t endian=1;NEED(*(uint8_t*)&endian==1);
 char error[2048]={0};mjModel*m=mj_loadXML(argv[2],NULL,error,sizeof error);if(!m)fprintf(stderr,"mj_loadXML: %s\n",error);NEED(m);int joint[9],act[9];support(m,joint,act);metadata(capture?argv[5]:argv[3],m,joint,act);
 if(!capture){mj_deleteModel(m);puts("{\"metadata_admitted\":true,\"forward_calls\":0,\"integration_steps\":0}");return 0;}
 FILE*input=fopen(argv[3],"rb");NEED(input);uint32_t header[2];NEED(fread(header,1,8,input)==8&&header[0]==UINT32_C(0x31464150)&&header[1]==18);double cases[18][27];NEED(fread(cases,1,sizeof cases,input)==sizeof cases&&fgetc(input)==EOF&&!fclose(input));
 for(int row=0;row<18;row++)for(int i=0;i<27;i++){double limit=i<9?(i<7?100:1):i<18?(i<16?20:2):(i<25?10000:1);NEED(isfinite(cases[row][i])&&fabs(cases[row][i])<=limit);}
 FILE*output=fopen(argv[4],"wbx"),*progress=fopen(argv[6],"wbx");NEED(output&&progress);mjData*d=mj_makeData(m);NEED(d);double values[18][162];size_t sparse_rows=0;
 for(int row=0;row<18;row++){
  mj_resetData(m,d);NEED(d->time==0);for(int i=0;i<9;i++){d->qpos[m->jnt_qposadr[joint[i]]]=cases[row][i];d->qvel[m->jnt_dofadr[joint[i]]]=cases[row][9+i];d->ctrl[act[i]]=cases[row][18+i];}
  NEED(fprintf(progress,"{\"case\":%d,\"event\":\"before_forward\"}\n",row)>0&&!fflush(progress));mj_forward(m,d);NEED(fprintf(progress,"{\"case\":%d,\"event\":\"after_forward\"}\n",row)>0&&!fflush(progress));NEED(d->time==0);
  for(int i=0;i<9;i++){int a=act[i],da=m->jnt_dofadr[joint[i]],at=d->moment_rowadr[a];NEED(d->moment_rownnz[a]==1&&at>=0&&at<m->nJmom&&d->moment_colind[at]==da&&d->actuator_moment[at]==1);sparse_rows++;NEED(!memcmp(d->qpos+i,cases[row]+i,8)&&!memcmp(d->qvel+i,cases[row]+9+i,8)&&!memcmp(d->ctrl+a,cases[row]+18+i,8));
   values[row][i]=d->actuator_force[a];values[row][9+i]=d->qfrc_actuator[da];values[row][18+i]=d->qfrc_spring[da];values[row][27+i]=d->qfrc_damper[da];values[row][36+i]=d->qfrc_gravcomp[da];values[row][45+i]=d->qfrc_fluid[da];values[row][54+i]=d->qfrc_passive[da];values[row][63+i]=d->actuator_length[a];values[row][72+i]=d->actuator_velocity[a];values[row][81+i]=d->actuator_moment[at];values[row][90+i]=d->ctrl[a];
  }
  for(int i=9;i<m->nv;i++)NEED(d->qvel[i]==0);memcpy(values[row]+99,d->qacc_smooth,21*8);memcpy(values[row]+120,d->qacc,21*8);memcpy(values[row]+141,d->qfrc_constraint,21*8);for(int i=0;i<162;i++)NEED(isfinite(values[row][i]));
 }
 NEED(sparse_rows==162&&fwrite(values,1,sizeof values,output)==sizeof values&&!fclose(output)&&!fclose(progress));mj_deleteData(d);mj_deleteModel(m);printf("{\"forward_calls\":18,\"integration_steps\":0,\"render_calls\":0,\"NN_calls\":0,\"rows\":18,\"words_per_row\":162,\"sparse_rows_verified\":%zu,\"assertions\":%zu}\n",sparse_rows,checks);return 0;
}
