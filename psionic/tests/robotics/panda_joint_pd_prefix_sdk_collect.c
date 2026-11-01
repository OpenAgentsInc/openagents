/* First SDK plant prefix; Coax owns the controller, SDK owns this reference
   plant only. The old authored support/metadata entry is never called. */
#define main panda_actuation_prefix_entry_not_called
#include "panda_actuation_sdk_collect.c"
#undef main
#include "panda_joint_pd_prefix_format.h"
static void ppr_model(mjModel *m,int joint[9],int act[9]) {
 support(m,joint,act);NEED(m->opt.disableflags==0&&m->opt.enableflags==0&&m->opt.integrator==mjINT_EULER&&m->opt.timestep==0.002);
 NEED(m->opt.density==1.2&&m->opt.viscosity==0.00002&&m->opt.wind[0]==0&&m->opt.wind[1]==0&&m->opt.wind[2]==0);
 NEED(m->opt.gravity[0]==0&&m->opt.gravity[1]==0&&m->opt.gravity[2]==-9.81);m->opt.disableflags=mjDSBL_CONSTRAINT|mjDSBL_CONTACT;NEED(m->opt.disableflags==17);
}
static void ppr_warnings(const mjData *d) {for(int i=0;i<mjNWARNING;i++)NEED(d->warning[i].number==0);}
int main(int argc,char **argv) {
 NEED(argc==4||argc==7);int capture=!strcmp(argv[1],"capture");NEED((capture&&argc==7)||(!strcmp(argv[1],"metadata")&&argc==4));
 char error[2048]={0};mjModel *m=mj_loadXML(argv[2],NULL,error,sizeof error);if(!m)fprintf(stderr,"%s\n",error);NEED(m);int joint[9],act[9];ppr_model(m,joint,act);metadata(capture?argv[5]:argv[3],m,joint,act);
 if(!capture){mj_deleteModel(m);puts("{\"prepared\":true,\"steps\":0,\"explicit_forwards\":0}");return 0;}
 PprInput input=ppr_input(argv[3]);mjData *d=mj_makeData(m);NEED(d);mj_resetData(m,d);
 for(int i=0;i<9;i++){d->qpos[m->jnt_qposadr[joint[i]]]=input.q[i];d->qvel[m->jnt_dofadr[joint[i]]]=input.v[i];}
 for(int i=0;i<m->nv;i++)NEED(d->qfrc_applied[i]==0);for(int i=0;i<6*m->nbody;i++)NEED(d->xfrc_applied[i]==0);
 FILE *progress=fopen(argv[6],"wbx");NEED(progress);NEED(fprintf(progress,"{\"event\":\"before_initial_forward\"}\n")>0&&!fflush(progress));mj_forward(m,d);NEED(fprintf(progress,"{\"event\":\"after_initial_forward\"}\n")>0&&!fflush(progress));ppr_warnings(d);NEED(d->time==0);
 mjtNum mass[21*21];mj_fullM(m,mass,d->qM);for(int i=0;i<9;i++)for(int j=9;j<21;j++)NEED(mass[21*i+j]==0&&mass[21*j+i]==0);
 FILE *output=fopen(argv[4],"wbx");NEED(output);uint32_t header[2]={UINT32_C(0x31525050),PPR_STEPS};NEED(fwrite(header,1,8,output)==8);double time=0;
 for(int step=0;step<PPR_STEPS;step++) {
  double q[9],v[9],controls[9];for(int i=0;i<9;i++){q[i]=d->qpos[m->jnt_qposadr[joint[i]]];v[i]=d->qvel[m->jnt_dofadr[joint[i]]];}ppr_state(q,v);coax_t_PjpOutput pd=ppr_control(&input,q,v);ppr_controls(&input,pd,controls);for(int i=0;i<9;i++)d->ctrl[act[i]]=controls[i];
  NEED(fprintf(progress,"{\"step\":%d,\"event\":\"before_step\"}\n",step)>0&&!fflush(progress));mj_step(m,d);NEED(fprintf(progress,"{\"step\":%d,\"event\":\"after_step\"}\n",step)>0&&!fflush(progress));ppr_warnings(d);time+=0.002;NEED(d->time==time);
  for(int i=0;i<m->nv;i++)NEED(d->qfrc_constraint[i]==0&&d->qfrc_applied[i]==0);
  for(int i=0;i<9;i++){q[i]=d->qpos[m->jnt_qposadr[joint[i]]];v[i]=d->qvel[m->jnt_dofadr[joint[i]]];NEED(d->ctrl[act[i]]==controls[i]);}ppr_state(q,v);PprRecord record;ppr_pack(&record,q,v,pd,d->time);NEED(fwrite(&record,1,sizeof record,output)==sizeof record&&!fflush(output));
 }
 NEED(!fclose(output)&&!fclose(progress));mj_deleteData(d);mj_deleteModel(m);printf("{\"passed\":true,\"steps\":250,\"explicit_forwards\":1,\"post_step_forwards\":0,\"NN_calls\":0,\"render_calls\":0,\"record_bytes\":440,\"assertions\":%zu}\n",checks+ppr_checks);return 0;
}
