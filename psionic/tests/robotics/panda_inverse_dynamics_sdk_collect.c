/* Opt-in independent SDK reference effects only: one mj_forward per NEW nonzero-velocity case.
   No own FK formula, physics steps, renderer, controller or learned model. */
#include <mujoco/mujoco.h>
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
_Static_assert(sizeof(mjtNum)==8&&sizeof(double)==8&&FLT_RADIX==2&&DBL_MANT_DIG==53,"SDK double profile");
static const char*const body_names[16]={"robot0_base","robot0_link0","robot0_link1","robot0_link2","robot0_link3","robot0_link4","robot0_link5","robot0_link6","robot0_link7","robot0_right_hand","gripper0_right_gripper","gripper0_eef","gripper0_leftfinger","gripper0_finger_joint1_tip","gripper0_rightfinger","gripper0_finger_joint2_tip"};
static const char*const joint_names[9]={"robot0_joint1","robot0_joint2","robot0_joint3","robot0_joint4","robot0_joint5","robot0_joint6","robot0_joint7","gripper0_finger_joint1","gripper0_finger_joint2"};
static void string(FILE*f,const char*s){assert(fputc('"',f)!=EOF);for(;s&&*s;s++){unsigned char c=(unsigned char)*s;if(c=='"'||c=='\\'){assert(fputc('\\',f)!=EOF);assert(fputc(c,f)!=EOF);}else if(c<32){assert(fprintf(f,"\\u%04x",c)>0);}else assert(fputc(c,f)!=EOF);}assert(fputc('"',f)!=EOF);}
static void numbers(FILE*f,const mjtNum*x,int n){assert(fputc('[',f)!=EOF);for(int i=0;i<n;i++){assert(isfinite(x[i]));if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"%.17g",x[i])>0);}assert(fputc(']',f)!=EOF);}
static void body_name(FILE*f,const mjModel*m,int id){const char*s=mj_id2name(m,mjOBJ_BODY,id);string(f,s?s:"");}
static void metadata(FILE*f,const mjModel*m,const int body[16],const int joint[9],const int actuator[7],int site){
 assert(fprintf(f,"{\n\"schema\":\"psionic.panda-inverse-dynamics.sdk-model.v1\",\"sdk_version_number\":%d,\"model\":{\"nq\":%d,\"nv\":%d,\"nbody\":%d,\"njnt\":%d,\"nsite\":%d,\"nplugin\":%d,\"timestep\":%.17g,\"integrator\":%d},\n\"qpos0\":",mj_version(),(int)m->nq,(int)m->nv,(int)m->nbody,(int)m->njnt,(int)m->nsite,(int)m->nplugin,m->opt.timestep,(int)m->opt.integrator)>0);numbers(f,m->qpos0,(int)m->nq);assert(fprintf(f,",\"gravity\":")>0);numbers(f,m->opt.gravity,3);
 assert(fprintf(f,",\n\"bodies\":[")>0);
 for(int i=0;i<16;i++){int id=body[i],parent=m->body_parentid[id];if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"{\"index\":%d,\"id\":%d,\"name\":",i,id)>0);string(f,body_names[i]);assert(fprintf(f,",\"parent_id\":%d,\"parent_name\":",parent)>0);body_name(f,m,parent);assert(fprintf(f,",\"position\":")>0);numbers(f,m->body_pos+3*id,3);assert(fprintf(f,",\"quaternion_wxyz\":")>0);numbers(f,m->body_quat+4*id,4);assert(fprintf(f,",\"mass\":%.17g,\"inertial_position\":",m->body_mass[id])>0);numbers(f,m->body_ipos+3*id,3);assert(fprintf(f,",\"inertial_quaternion_wxyz\":")>0);numbers(f,m->body_iquat+4*id,4);assert(fprintf(f,",\"principal_inertia\":")>0);numbers(f,m->body_inertia+3*id,3);assert(fprintf(f,",\"joint_count\":%d,\"joint_adr\":%d,\"mocap_id\":%d}",(int)m->body_jntnum[id],(int)m->body_jntadr[id],(int)m->body_mocapid[id])>0);}
 assert(fprintf(f,"],\n\"robot_joints\":[")>0);
 for(int i=0;i<9;i++){int id=joint[i],qa=m->jnt_qposadr[id],da=m->jnt_dofadr[id],b=m->jnt_bodyid[id];if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"{\"index\":%d,\"id\":%d,\"name\":",i,id)>0);string(f,joint_names[i]);assert(fprintf(f,",\"type\":%d,\"type_name\":\"%s\",\"body_id\":%d,\"body_name\":",(int)m->jnt_type[id],i<7?"hinge":"slide",b)>0);body_name(f,m,b);assert(fprintf(f,",\"axis\":")>0);numbers(f,m->jnt_axis+3*id,3);assert(fprintf(f,",\"position\":")>0);numbers(f,m->jnt_pos+3*id,3);assert(fprintf(f,",\"qpos_adr\":%d,\"dof_adr\":%d,\"qpos0_reference\":%.17g,\"qpos_spring\":%.17g,\"limited\":%d,\"range\":",qa,da,m->qpos0[qa],m->qpos_spring[qa],(int)m->jnt_limited[id])>0);numbers(f,m->jnt_range+2*id,2);assert(fprintf(f,",\"armature\":%.17g,\"damping\":%.17g,\"frictionloss\":%.17g",m->dof_armature[da],m->dof_damping[da],m->dof_frictionloss[da])>0);assert(fputc('}',f)!=EOF);}
 assert(fprintf(f,"],\n\"site\":{\"id\":%d,\"name\":\"gripper0_grip_site\",\"body_id\":%d,\"body_name\":",site,(int)m->site_bodyid[site])>0);body_name(f,m,m->site_bodyid[site]);assert(fprintf(f,",\"position\":")>0);numbers(f,m->site_pos+3*site,3);assert(fprintf(f,",\"quaternion_wxyz\":")>0);numbers(f,m->site_quat+4*site,4);assert(fprintf(f,"},\n\"context\":{\"disableflags\":%d,\"enableflags\":%d,\"density\":%.17g,\"viscosity\":%.17g,\"ntendon\":%d,\"wind\":",m->opt.disableflags,m->opt.enableflags,m->opt.density,m->opt.viscosity,(int)m->ntendon)>0);numbers(f,m->opt.wind,3);assert(fprintf(f,",\"arm_actuators\":[")>0);for(int i=0;i<7;i++){int id=actuator[i];if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"{\"id\":%d,\"name\":",id)>0);string(f,mj_id2name(m,mjOBJ_ACTUATOR,id));assert(fprintf(f,",\"trntype\":%d,\"joint_id\":%d,\"ctrllimited\":%d,\"ctrlrange\":",(int)m->actuator_trntype[id],(int)m->actuator_trnid[2*id],(int)m->actuator_ctrllimited[id])>0);numbers(f,m->actuator_ctrlrange+2*id,2);assert(fputc('}',f)!=EOF);}assert(fprintf(f,"]}\n}\n")>0);
}
int main(int argc,char**argv){
 assert(argc==7&&mj_version()==331);uint16_t endian=1;assert(*(uint8_t*)&endian==1);
 char error[2048]={0};mjModel*m=mj_loadXML(argv[1],NULL,error,sizeof error);if(!m){fprintf(stderr,"mj_loadXML: %s\n",error);return 2;}
 assert(m->nq==23&&m->nv==21&&m->nplugin==0&&m->ntendon==0&&mjcb_control==NULL&&mjcb_passive==NULL);
 mjData*d=mj_makeData(m);assert(d);mjtNum*dense=calloc((size_t)m->nv*(size_t)m->nv,sizeof(mjtNum)),*rne=calloc((size_t)m->nv,sizeof(mjtNum));assert(dense&&rne);int body[16],joint[9],actuator[7];mjtNum*jp=calloc(3*(size_t)m->nv,sizeof(mjtNum)),*jr=calloc(3*(size_t)m->nv,sizeof(mjtNum));assert(jp&&jr);
 for(int i=0;i<16;i++){body[i]=mj_name2id(m,mjOBJ_BODY,body_names[i]);assert(body[i]>=0&&m->body_mocapid[body[i]]==-1);}
 for(int i=0;i<9;i++){joint[i]=mj_name2id(m,mjOBJ_JOINT,joint_names[i]);assert(joint[i]>=0&&m->jnt_type[joint[i]]==(i<7?mjJNT_HINGE:mjJNT_SLIDE));assert(m->jnt_qposadr[joint[i]]==i&&m->jnt_dofadr[joint[i]]==i);}
 for(int i=0;i<7;i++){char name[32];assert(snprintf(name,sizeof name,"robot0_torq_j%d",i+1)>0);actuator[i]=mj_name2id(m,mjOBJ_ACTUATOR,name);assert(actuator[i]>=0);}
 int site=mj_name2id(m,mjOBJ_SITE,"gripper0_grip_site");assert(site>=0);
 FILE*input=fopen(argv[2],"rb"),*mass_bias=fopen(argv[3],"wbx"),*raw_rne=fopen(argv[4],"wbx"),*model=fopen(argv[5],"wbx"),*samples=fopen(argv[6],"wbx");assert(input&&mass_bias&&raw_rne&&model&&samples);uint32_t header[2];assert(fread(header,sizeof header,1,input)==1&&header[0]==UINT32_C(0x31444950)&&header[1]==15);metadata(model,m,body,joint,actuator,site);assert(!fclose(model));
 for(uint32_t case_id=0;case_id<header[1];case_id++){
  double values[27],out[90],body_force[9];assert(fread(values,sizeof values,1,input)==1);double*q=values,*v=values+9,*a=values+18;
  for(int i=0;i<9;i++){assert(isfinite(q[i])&&fabs(q[i])<=(i<7?100:1));assert(isfinite(v[i])&&fabs(v[i])<=(i<7?20:2));assert(isfinite(a[i])&&fabs(a[i])<=(i<7?200:20));}
  mj_resetData(m,d);assert(d->time==0);for(int i=0;i<9;i++){d->qpos[m->jnt_qposadr[joint[i]]]=q[i];d->qvel[m->jnt_dofadr[joint[i]]]=v[i];}
  mj_forward(m,d);assert(d->time==0);for(int i=0;i<9;i++){assert(!memcmp(d->qpos+i,q+i,sizeof(double)));assert(!memcmp(d->qvel+i,v+i,sizeof(double)));}for(int i=9;i<m->nv;i++)assert(d->qvel[i]==0);
  mj_fullM(m,dense,d->qM);for(int r=0;r<9;r++){int dr=m->jnt_dofadr[joint[r]];for(int c=0;c<9;c++){int dc=m->jnt_dofadr[joint[c]];out[r*9+c]=dense[dr*m->nv+dc];}out[81+r]=d->qfrc_bias[dr];}
  for(int i=0;i<90;i++)assert(isfinite(out[i]));assert(fwrite(out,sizeof out,1,mass_bias)==1);
  /* Separately versioned NEW whole PcSample observations in this same fresh
     forward generation. Reuse the already expanded M, no extra forward. */
  double pc[144];mjtNum velocity[6];mj_jacSite(m,d,jp,jr,site);mj_objectVelocity(m,d,mjOBJ_SITE,site,velocity,0);
  memcpy(pc,d->site_xpos+3*site,3*sizeof(double));memcpy(pc+3,d->site_xmat+9*site,9*sizeof(double));
  for(int i=0;i<7;i++){pc[12+i]=d->qpos[m->jnt_qposadr[joint[i]]];pc[19+i]=d->qvel[m->jnt_dofadr[joint[i]]];pc[123+i]=out[81+i];pc[130+i]=m->actuator_ctrlrange[2*actuator[i]];pc[137+i]=m->actuator_ctrlrange[2*actuator[i]+1];for(int j=0;j<7;j++)pc[26+i*7+j]=out[i*9+j];for(int row=0;row<3;row++){pc[75+row*7+i]=jp[row*m->nv+m->jnt_dofadr[joint[i]]];pc[96+row*7+i]=jr[row*m->nv+m->jnt_dofadr[joint[i]]];}}
  memcpy(pc+117,velocity+3,3*sizeof(double));memcpy(pc+120,velocity,3*sizeof(double));for(int i=0;i<144;i++)assert(isfinite(pc[i]));assert(fwrite(pc,sizeof pc,1,samples)==1);
  /* Public body RNE omits joint armature*a. This separate raw observation
     never stands in for M*a+bias or qfrc_inverse. */
  for(int i=0;i<m->nv;i++)d->qacc[i]=0;for(int i=0;i<9;i++)d->qacc[m->jnt_dofadr[joint[i]]]=a[i];mj_rne(m,d,1,rne);
  for(int i=0;i<9;i++){body_force[i]=rne[m->jnt_dofadr[joint[i]]];assert(isfinite(body_force[i]));assert(!memcmp(d->qpos+i,q+i,sizeof(double)));assert(!memcmp(d->qvel+i,v+i,sizeof(double)));assert(!memcmp(d->qacc+i,a+i,sizeof(double)));}for(int i=9;i<m->nv;i++)assert(d->qvel[i]==0&&d->qacc[i]==0);assert(d->time==0);assert(fwrite(body_force,sizeof body_force,1,raw_rne)==1);
 }
 assert(fgetc(input)==EOF&&!fclose(input)&&!fclose(mass_bias)&&!fclose(raw_rne)&&!fclose(samples));free(dense);free(rne);free(jp);free(jr);mj_deleteData(d);mj_deleteModel(m);puts("Panda inverse reference:15NEW matched q/v/a contexts,15forwards/fullM/bodyRNE/jacSite/objectVelocity,zero steps/render/NN");return 0;
}
