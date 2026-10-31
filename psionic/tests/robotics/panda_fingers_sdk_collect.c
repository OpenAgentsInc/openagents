/* Opt-in independent SDK reference effects only: one mj_forward per NEW arm+finger case.
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
static void metadata(FILE*f,const mjModel*m,const int body[16],const int joint[9],int site){
 assert(fprintf(f,"{\n\"schema\":\"psionic.panda-fingers.sdk-model.v1\",\"sdk_version_number\":%d,\"model\":{\"nq\":%d,\"nv\":%d,\"nbody\":%d,\"njnt\":%d,\"nsite\":%d,\"nplugin\":%d,\"timestep\":%.17g,\"integrator\":%d},\n\"qpos0\":",mj_version(),(int)m->nq,(int)m->nv,(int)m->nbody,(int)m->njnt,(int)m->nsite,(int)m->nplugin,m->opt.timestep,(int)m->opt.integrator)>0);numbers(f,m->qpos0,(int)m->nq);assert(fprintf(f,",\"gravity\":")>0);numbers(f,m->opt.gravity,3);
 assert(fprintf(f,",\n\"bodies\":[")>0);
 for(int i=0;i<16;i++){int id=body[i],parent=m->body_parentid[id];if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"{\"index\":%d,\"id\":%d,\"name\":",i,id)>0);string(f,body_names[i]);assert(fprintf(f,",\"parent_id\":%d,\"parent_name\":",parent)>0);body_name(f,m,parent);assert(fprintf(f,",\"position\":")>0);numbers(f,m->body_pos+3*id,3);assert(fprintf(f,",\"quaternion_wxyz\":")>0);numbers(f,m->body_quat+4*id,4);assert(fprintf(f,",\"mass\":%.17g,\"inertial_position\":",m->body_mass[id])>0);numbers(f,m->body_ipos+3*id,3);assert(fprintf(f,",\"inertial_quaternion_wxyz\":")>0);numbers(f,m->body_iquat+4*id,4);assert(fprintf(f,",\"principal_inertia\":")>0);numbers(f,m->body_inertia+3*id,3);assert(fprintf(f,",\"joint_count\":%d,\"joint_adr\":%d,\"mocap_id\":%d}",(int)m->body_jntnum[id],(int)m->body_jntadr[id],(int)m->body_mocapid[id])>0);}
 assert(fprintf(f,"],\n\"robot_joints\":[")>0);
 for(int i=0;i<9;i++){int id=joint[i],qa=m->jnt_qposadr[id],da=m->jnt_dofadr[id],b=m->jnt_bodyid[id];if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"{\"index\":%d,\"id\":%d,\"name\":",i,id)>0);string(f,joint_names[i]);assert(fprintf(f,",\"type\":%d,\"type_name\":\"%s\",\"body_id\":%d,\"body_name\":",(int)m->jnt_type[id],i<7?"hinge":"slide",b)>0);body_name(f,m,b);assert(fprintf(f,",\"axis\":")>0);numbers(f,m->jnt_axis+3*id,3);assert(fprintf(f,",\"position\":")>0);numbers(f,m->jnt_pos+3*id,3);assert(fprintf(f,",\"qpos_adr\":%d,\"dof_adr\":%d,\"qpos0_reference\":%.17g,\"qpos_spring\":%.17g,\"limited\":%d,\"range\":",qa,da,m->qpos0[qa],m->qpos_spring[qa],(int)m->jnt_limited[id])>0);numbers(f,m->jnt_range+2*id,2);assert(fprintf(f,",\"armature\":%.17g,\"damping\":%.17g,\"frictionloss\":%.17g",m->dof_armature[da],m->dof_damping[da],m->dof_frictionloss[da])>0);assert(fputc('}',f)!=EOF);}
 assert(fprintf(f,"],\n\"site\":{\"id\":%d,\"name\":\"gripper0_grip_site\",\"body_id\":%d,\"body_name\":",site,(int)m->site_bodyid[site])>0);body_name(f,m,m->site_bodyid[site]);assert(fprintf(f,",\"position\":")>0);numbers(f,m->site_pos+3*site,3);assert(fprintf(f,",\"quaternion_wxyz\":")>0);numbers(f,m->site_quat+4*site,4);assert(fprintf(f,"}\n}\n")>0);
}
int main(int argc,char**argv){
 assert(argc==6&&mj_version()==331);uint16_t endian=1;assert(*(uint8_t*)&endian==1);
 char error[2048]={0};mjModel*m=mj_loadXML(argv[1],NULL,error,sizeof error);if(!m){fprintf(stderr,"mj_loadXML: %s\n",error);return 2;}assert(m->nq>0&&m->nq<1000&&m->nv>0&&m->nv<1000&&m->nplugin==0);
 mjData*d=mj_makeData(m);assert(d);mjtNum*dense=calloc((size_t)m->nv*(size_t)m->nv,sizeof(mjtNum));assert(dense);int body[16],joint[9];
 for(int i=0;i<16;i++){body[i]=mj_name2id(m,mjOBJ_BODY,body_names[i]);assert(body[i]>=0&&m->body_mocapid[body[i]]==-1);}
 for(int i=0;i<9;i++){joint[i]=mj_name2id(m,mjOBJ_JOINT,joint_names[i]);assert(joint[i]>=0&&m->jnt_type[joint[i]]==(i<7?mjJNT_HINGE:mjJNT_SLIDE));assert(m->jnt_qposadr[joint[i]]>=0&&m->jnt_qposadr[joint[i]]<m->nq);assert(m->jnt_dofadr[joint[i]]>=0&&m->jnt_dofadr[joint[i]]<m->nv);}
 int site=mj_name2id(m,mjOBJ_SITE,"gripper0_grip_site");assert(site>=0);
 FILE*input=fopen(argv[2],"rb"),*poses=fopen(argv[3],"wbx"),*model=fopen(argv[4],"wbx"),*dynamic_file=fopen(argv[5],"wbx");assert(input&&poses&&model&&dynamic_file);metadata(model,m,body,joint,site);assert(!fclose(model));
 for(int case_id=0;case_id<5;case_id++){
  double q[9],out[268],dynamics[90];assert(fread(q,sizeof(double),9,input)==9);mj_resetData(m,d);assert(d->time==0);
  for(int i=0;i<9;i++){assert(isfinite(q[i])&&fabs(q[i])<=4);d->qpos[m->jnt_qposadr[joint[i]]]=q[i];}
  mj_forward(m,d);assert(d->time==0);
  for(int i=0;i<9;i++)assert(memcmp(d->qpos+m->jnt_qposadr[joint[i]],q+i,sizeof(double))==0);
  for(int i=0;i<m->nv;i++)assert(d->qvel[i]==0);
  for(int i=0;i<16;i++){int id=body[i];memcpy(out+16*i,d->xpos+3*id,3*sizeof(double));memcpy(out+16*i+3,d->xquat+4*id,4*sizeof(double));memcpy(out+16*i+7,d->xmat+9*id,9*sizeof(double));}
  memcpy(out+256,d->site_xpos+3*site,3*sizeof(double));memcpy(out+259,d->site_xmat+9*site,9*sizeof(double));
  for(int i=0;i<268;i++)assert(isfinite(out[i]));assert(fwrite(out,sizeof(double),268,poses)==268);
  mj_fullM(m,dense,d->qM);for(int r=0;r<9;r++){int dr=m->jnt_dofadr[joint[r]];for(int c=0;c<9;c++){int dc=m->jnt_dofadr[joint[c]];dynamics[r*9+c]=dense[dr*m->nv+dc];}dynamics[81+r]=d->qfrc_bias[dr];}for(int i=0;i<90;i++)assert(isfinite(dynamics[i]));assert(fwrite(dynamics,sizeof(double),90,dynamic_file)==90);
 }
 assert(fgetc(input)==EOF&&!fclose(input)&&!fclose(poses)&&!fclose(dynamic_file));free(dense);mj_deleteData(d);mj_deleteModel(m);puts("panda fingers SDK reference: 5 NEW forwards, 16 body poses + grip site, 1340 pose words +450 future-dynamics words, zero steps/render/NN");return 0;
}
