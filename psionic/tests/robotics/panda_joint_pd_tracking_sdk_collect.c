/* NEW four-case SDK plant capture. Only C effects; Coax chooses references,
   feedback, saturation and external generalized-force schedules. */
#define main panda_tracking_old_actuation_entry_not_called
#include "panda_actuation_sdk_collect.c"
#undef main
#include "panda_joint_pd_tracking_format.h"
static void pnt_model(mjModel*m,int joint[9],int act[9]){support(m,joint,act);NEED(m->nJmom==9);NEED(m->opt.disableflags==0&&m->opt.enableflags==0&&m->opt.integrator==mjINT_EULER&&m->opt.timestep==0.002);NEED(m->opt.density==1.2&&m->opt.viscosity==0.00002&&m->opt.wind[0]==0&&m->opt.wind[1]==0&&m->opt.wind[2]==0);NEED(m->opt.gravity[0]==0&&m->opt.gravity[1]==0&&m->opt.gravity[2]==-9.81);m->opt.disableflags=mjDSBL_CONSTRAINT|mjDSBL_CONTACT;NEED(m->opt.disableflags==17);}
static void pnt_warnings(const mjData*d){for(int j=0;j<mjNWARNING;j++)NEED(d->warning[j].number==0);}
static void pnt_observe(const mjModel*m,const mjData*d,const int joint[9],double*q,double*v){for(int j=0;j<9;j++){q[j]=d->qpos[m->jnt_qposadr[joint[j]]];v[j]=d->qvel[m->jnt_dofadr[joint[j]]];}pnt_state(q,v);}
static void pnt_progress(FILE*f,int id,int tick,const char*event,coax_t_PntStatus status){NEED(fprintf(f,"{\"case\":%d,\"tick\":%d,\"event\":\"%s\",\"stage\":%lld,\"code\":%lld}\n",id,tick,event,(long long)status.stage,(long long)status.code)>0&&!fflush(f));}
int main(int argc,char**argv){
 NEED(argc==4||argc==7);uint16_t endian=1;NEED(*(uint8_t*)&endian==1);uint64_t fpcr=UINT64_MAX;
#if defined(__aarch64__)
 __asm__ volatile("mrs %0,fpcr":"=r"(fpcr));
#endif
 NEED(fpcr==0);int capture=!strcmp(argv[1],"capture");NEED((capture&&argc==7)||(!strcmp(argv[1],"metadata")&&argc==4));char error[2048]={0};mjModel*m=mj_loadXML(argv[2],NULL,error,sizeof error);if(!m)fprintf(stderr,"%s\n",error);NEED(m);int joint[9],act[9];pnt_model(m,joint,act);metadata(capture?argv[5]:argv[3],m,joint,act);
 if(!capture){mj_deleteModel(m);puts("{\"prepared\":true,\"steps\":0,\"explicit_forwards\":0}");return 0;}
 coax_t_PntInitial input[PNT_CASES];pnt_input(argv[3],input);mjData*d=mj_makeData(m);NEED(d);FILE*output=fopen(argv[4],"wbx"),*progress=fopen(argv[6],"wbx");NEED(output&&progress);pnt_header(output,UINT32_C(0x31525450),1040);size_t steps=0,forwards=0,sparse=0;coax_t_PntStatus ok={0,0};
 for(int id=0;id<PNT_CASES;id++){
  coax_t_PntInitial authored;pnt_initial_observed(id,&authored);NEED(!memcmp(&authored,&input[id],144));mj_resetData(m,d);for(int j=0;j<m->nq;j++)NEED(d->qpos[j]==m->qpos0[j]);for(int j=0;j<m->nv;j++){d->qvel[j]=0;d->qfrc_applied[j]=0;}for(int j=0;j<6*m->nbody;j++)d->xfrc_applied[j]=0;for(int j=0;j<m->nu;j++)d->ctrl[j]=0;
  for(int j=0;j<9;j++){d->qpos[m->jnt_qposadr[joint[j]]]=input[id].q.e[j];d->qvel[m->jnt_dofadr[joint[j]]]=input[id].v.e[j];}
  pnt_progress(progress,id,-1,"before_initial_forward",ok);mj_forward(m,d);pnt_progress(progress,id,-1,"after_initial_forward",ok);forwards++;pnt_warnings(d);NEED(d->time==0);mjtNum mass[21*21];mj_fullM(m,mass,d->qM);for(int i=0;i<9;i++)for(int j=9;j<21;j++)NEED(mass[21*i+j]==0&&mass[21*j+i]==0);
  for(int tick=0;tick<PNT_STEPS;tick++){
   double preq[9],prev[9],postq[9],postv[9],force[9],generalized[9];pnt_observe(m,d,joint,preq,prev);coax_t_PntCommand command;coax_t_PntStatus status=pnt_control_observed(id,tick,preq,prev,&command);if(status.stage||status.code){pnt_progress(progress,id,tick,"control_refused",status);NEED(status.stage==0&&status.code==0);}
   for(int j=0;j<m->nv;j++)d->qfrc_applied[j]=0;for(int j=0;j<6*m->nbody;j++)d->xfrc_applied[j]=0;for(int j=0;j<9;j++){d->ctrl[act[j]]=command.controls.e[j];d->qfrc_applied[m->jnt_dofadr[joint[j]]]=command.external.e[j];}
   pnt_progress(progress,id,tick,"before_step",status);mj_step(m,d);pnt_progress(progress,id,tick,"after_step",status);steps++;pnt_warnings(d);NEED(fabs(d->time-(double)(tick+1)/500.)<=0x1p-26);pnt_observe(m,d,joint,postq,postv);
   /* These SDK fields are the PREintegration actuator cache. One +1 sparse
      transmission column per actuator makes scalar force equal generalized. */
   for(int j=0;j<9;j++){int a=act[j],da=m->jnt_dofadr[joint[j]],at=d->moment_rowadr[a];NEED(d->moment_rownnz[a]==1&&at>=0&&at<m->nJmom&&d->moment_colind[at]==da&&d->actuator_moment[at]==1);sparse++;force[j]=d->actuator_force[a];generalized[j]=d->qfrc_actuator[da];NEED(isfinite(force[j])&&isfinite(generalized[j])&&generalized[j]==force[j]&&d->ctrl[a]==command.controls.e[j]&&d->qfrc_applied[da]==command.external.e[j]);}
   for(int j=0;j<m->nv;j++){NEED(d->qfrc_constraint[j]==0);if(j>=9)NEED(d->qfrc_actuator[j]==0&&d->qfrc_applied[j]==0);}for(int j=0;j<6*m->nbody;j++)NEED(d->xfrc_applied[j]==0);
   PntRecord record;pnt_pack(&record,id,tick,status,preq,prev,&command,force,postq,postv,d->time);NEED(fwrite(&record,1,sizeof record,output)==sizeof record&&!fflush(output));
  }
 }
 NEED(!fclose(output)&&!fclose(progress));mj_deleteData(d);mj_deleteModel(m);printf("{\"passed\":true,\"cases\":4,\"steps\":%zu,\"explicit_forwards\":%zu,\"post_step_forwards\":0,\"NN_calls\":0,\"render_calls\":0,\"record_bytes\":1040,\"sparse_moment_rows\":%zu,\"generalized_force_equalities\":%zu,\"assertions\":%zu}\n",steps,forwards,sparse,sparse,checks+pnt_checks);return 0;
}
