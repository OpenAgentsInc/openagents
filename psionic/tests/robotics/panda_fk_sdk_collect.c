/* Opt-in independent SDK reference effects only: one mj_forward per new q case.
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
static const char*const body_names[12]={"robot0_base","robot0_link0","robot0_link1","robot0_link2","robot0_link3","robot0_link4","robot0_link5","robot0_link6","robot0_link7","robot0_right_hand","gripper0_right_gripper","gripper0_eef"};
static const char*const joint_names[7]={"robot0_joint1","robot0_joint2","robot0_joint3","robot0_joint4","robot0_joint5","robot0_joint6","robot0_joint7"};
static void string(FILE*f,const char*s){assert(fputc('"',f)!=EOF);for(;s&&*s;s++){unsigned char c=(unsigned char)*s;if(c=='"'||c=='\\'){assert(fputc('\\',f)!=EOF);assert(fputc(c,f)!=EOF);}else if(c<32){assert(fprintf(f,"\\u%04x",c)>0);}else assert(fputc(c,f)!=EOF);}assert(fputc('"',f)!=EOF);}
static void numbers(FILE*f,const mjtNum*x,int n){assert(fputc('[',f)!=EOF);for(int i=0;i<n;i++){assert(isfinite(x[i]));if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"%.17g",x[i])>0);}assert(fputc(']',f)!=EOF);}
static void body_name(FILE*f,const mjModel*m,int id){const char*s=mj_id2name(m,mjOBJ_BODY,id);string(f,s?s:"");}
static void metadata(FILE*f,const mjModel*m,const int body[12],const int joint[7],int site){
 assert(fprintf(f,"{\n\"schema\":\"psionic.panda-fk.sdk-model.v1\",\"sdk_version_number\":%d,\"model\":{\"nq\":%d,\"nv\":%d,\"nbody\":%d,\"njnt\":%d,\"nsite\":%d,\"nplugin\":%d,\"timestep\":%.17g,\"integrator\":%d},\n\"qpos0\":",mj_version(),(int)m->nq,(int)m->nv,(int)m->nbody,(int)m->njnt,(int)m->nsite,(int)m->nplugin,m->opt.timestep,(int)m->opt.integrator)>0);numbers(f,m->qpos0,(int)m->nq);
 assert(fprintf(f,",\n\"bodies\":[")>0);
 for(int i=0;i<12;i++){int id=body[i],parent=m->body_parentid[id];if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"{\"index\":%d,\"id\":%d,\"name\":",i,id)>0);string(f,body_names[i]);assert(fprintf(f,",\"parent_id\":%d,\"parent_name\":",parent)>0);body_name(f,m,parent);assert(fprintf(f,",\"position\":")>0);numbers(f,m->body_pos+3*id,3);assert(fprintf(f,",\"quaternion_wxyz\":")>0);numbers(f,m->body_quat+4*id,4);assert(fprintf(f,",\"joint_count\":%d,\"joint_adr\":%d,\"mocap_id\":%d}",(int)m->body_jntnum[id],(int)m->body_jntadr[id],(int)m->body_mocapid[id])>0);}
 assert(fprintf(f,"],\n\"arm_joints\":[")>0);
 for(int i=0;i<7;i++){int id=joint[i],qa=m->jnt_qposadr[id],da=m->jnt_dofadr[id],b=m->jnt_bodyid[id];if(i)assert(fputc(',',f)!=EOF);assert(fprintf(f,"{\"index\":%d,\"id\":%d,\"name\":",i,id)>0);string(f,joint_names[i]);assert(fprintf(f,",\"type\":%d,\"type_name\":\"hinge\",\"body_id\":%d,\"body_name\":",(int)m->jnt_type[id],b)>0);body_name(f,m,b);assert(fprintf(f,",\"axis\":")>0);numbers(f,m->jnt_axis+3*id,3);assert(fprintf(f,",\"position\":")>0);numbers(f,m->jnt_pos+3*id,3);assert(fprintf(f,",\"qpos_adr\":%d,\"dof_adr\":%d,\"qpos0_reference\":%.17g,\"qpos_spring\":%.17g,\"limited\":%d,\"range\":",qa,da,m->qpos0[qa],m->qpos_spring[qa],(int)m->jnt_limited[id])>0);numbers(f,m->jnt_range+2*id,2);assert(fputc('}',f)!=EOF);}
 assert(fprintf(f,"],\n\"site\":{\"id\":%d,\"name\":\"gripper0_grip_site\",\"body_id\":%d,\"body_name\":",site,(int)m->site_bodyid[site])>0);body_name(f,m,m->site_bodyid[site]);assert(fprintf(f,",\"position\":")>0);numbers(f,m->site_pos+3*site,3);assert(fprintf(f,",\"quaternion_wxyz\":")>0);numbers(f,m->site_quat+4*site,4);assert(fprintf(f,"}\n}\n")>0);
}
int main(int argc,char**argv){
 assert(argc==5&&mj_version()==331);uint16_t endian=1;assert(*(uint8_t*)&endian==1);
 char error[2048]={0};mjModel*m=mj_loadXML(argv[1],NULL,error,sizeof error);if(!m){fprintf(stderr,"mj_loadXML: %s\n",error);return 2;}assert(m->nq>0&&m->nq<1000&&m->nv>0&&m->nv<1000&&m->nplugin==0);
 mjData*d=mj_makeData(m);assert(d);int body[12],joint[7];
 for(int i=0;i<12;i++){body[i]=mj_name2id(m,mjOBJ_BODY,body_names[i]);assert(body[i]>=0&&m->body_mocapid[body[i]]==-1);}
 for(int i=0;i<7;i++){joint[i]=mj_name2id(m,mjOBJ_JOINT,joint_names[i]);assert(joint[i]>=0&&m->jnt_type[joint[i]]==mjJNT_HINGE);assert(m->jnt_qposadr[joint[i]]>=0&&m->jnt_qposadr[joint[i]]<m->nq);assert(m->jnt_dofadr[joint[i]]>=0&&m->jnt_dofadr[joint[i]]<m->nv);}
 int site=mj_name2id(m,mjOBJ_SITE,"gripper0_grip_site");assert(site>=0);
 FILE*input=fopen(argv[2],"rb"),*poses=fopen(argv[3],"wbx"),*model=fopen(argv[4],"wbx");assert(input&&poses&&model);metadata(model,m,body,joint,site);assert(!fclose(model));
 for(int case_id=0;case_id<7;case_id++){
  double q[7],out[204];assert(fread(q,sizeof(double),7,input)==7);mj_resetData(m,d);assert(d->time==0);
  for(int i=0;i<7;i++){assert(isfinite(q[i])&&fabs(q[i])<=4);d->qpos[m->jnt_qposadr[joint[i]]]=q[i];}
  mj_forward(m,d);assert(d->time==0);
  for(int i=0;i<7;i++)assert(memcmp(d->qpos+m->jnt_qposadr[joint[i]],q+i,sizeof(double))==0);
  for(int i=0;i<m->nv;i++)assert(d->qvel[i]==0);
  for(int i=0;i<12;i++){int id=body[i];memcpy(out+16*i,d->xpos+3*id,3*sizeof(double));memcpy(out+16*i+3,d->xquat+4*id,4*sizeof(double));memcpy(out+16*i+7,d->xmat+9*id,9*sizeof(double));}
  memcpy(out+192,d->site_xpos+3*site,3*sizeof(double));memcpy(out+195,d->site_xmat+9*site,9*sizeof(double));
  for(int i=0;i<204;i++)assert(isfinite(out[i]));assert(fwrite(out,sizeof(double),204,poses)==204);
 }
 assert(fgetc(input)==EOF&&!fclose(input)&&!fclose(poses));mj_deleteData(d);mj_deleteModel(m);puts("panda FK SDK reference: 7 fresh forwards, 12 body poses + grip site, 1428 F64 words, zero steps/render/NN");return 0;
}
