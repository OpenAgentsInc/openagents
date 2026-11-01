/* NEW three-case one-step evaluator; no owned numerical implementation.
   Reuse the frozen authored actuator support/metadata effects, not its entry. */
#define main panda_actuation_entry_not_called
#include "panda_actuation_sdk_collect.c"
#undef main

static void euler_support(mjModel *model,int joint[9],int act[9]) {
 support(model,joint,act);
 NEED(model->opt.disableflags==0&&model->opt.enableflags==0);
 NEED(model->opt.integrator==mjINT_EULER&&model->opt.timestep==0.002);
 NEED(model->opt.density==1.2&&model->opt.viscosity==0.00002);
 NEED(model->opt.wind[0]==0&&model->opt.wind[1]==0&&model->opt.wind[2]==0);
 NEED(model->opt.gravity[0]==0&&model->opt.gravity[1]==0&&model->opt.gravity[2]==-9.81);
 model->opt.disableflags=mjDSBL_CONSTRAINT|mjDSBL_CONTACT;
 NEED(model->opt.disableflags==17&&!(model->opt.disableflags&mjDSBL_EULERDAMP));
}
static void progress_event(FILE *file,int row,const char *kind) {
 NEED(fprintf(file,"{\"case\":%d,\"event\":\"%s\"}\n",row,kind)>0&&!fflush(file));
}
static void no_warnings(const mjData *data) {for(int i=0;i<mjNWARNING;i++)NEED(data->warning[i].number==0);}
int main(int argc,char **argv) {
 NEED(argc==4||argc==7);int capture=!strcmp(argv[1],"capture");
 NEED((capture&&argc==7)||(!strcmp(argv[1],"metadata")&&argc==4));
 uint16_t endian=1;NEED(*(uint8_t*)&endian==1);
 char error[2048]={0};mjModel *model=mj_loadXML(argv[2],NULL,error,sizeof error);
 if(!model)fprintf(stderr,"mj_loadXML: %s\n",error);NEED(model);
 int joint[9],act[9];euler_support(model,joint,act);
 metadata(capture?argv[5]:argv[3],model,joint,act);
 if(!capture){mj_deleteModel(model);puts("{\"metadata_admitted\":true,\"explicit_forwards\":0,\"steps\":0,\"disableflags\":17}");return 0;}
 FILE *input=fopen(argv[3],"rb");NEED(input);uint32_t header[2];
 NEED(fread(header,1,8,input)==8&&header[0]==UINT32_C(0x31554550)&&header[1]==3);
 double cases[3][36];NEED(fread(cases,1,sizeof cases,input)==sizeof cases&&fgetc(input)==EOF&&!fclose(input));
 for(int row=0;row<3;row++)for(int i=0;i<36;i++) {
  double cap=i<9?(i<7?100:1):i<18?(i<16?20:2):i<27?(i<25?10000:1):10000;
  NEED(isfinite(cases[row][i])&&fabs(cases[row][i])<=cap);
 }
 FILE *output=fopen(argv[4],"wbx"),*progress=fopen(argv[6],"wbx");NEED(output&&progress);
 mjData *data=mj_makeData(model);NEED(data);double result[3][154];
 for(int row=0;row<3;row++) {
  mj_resetData(model,data);NEED(data->time==0);
  for(int i=0;i<9;i++) {int q=model->jnt_qposadr[joint[i]],v=model->jnt_dofadr[joint[i]];data->qpos[q]=cases[row][i];data->qvel[v]=cases[row][9+i];data->ctrl[act[i]]=cases[row][18+i];data->qfrc_applied[v]=cases[row][27+i];}
  for(int i=9;i<model->nv;i++)NEED(data->qvel[i]==0&&data->qfrc_applied[i]==0);
  for(int i=0;i<6*model->nbody;i++)NEED(data->xfrc_applied[i]==0);
  progress_event(progress,row,"before_forward");mj_forward(model,data);progress_event(progress,row,"after_forward");NEED(data->time==0);no_warnings(data);
  mjtNum mass[21*21];mj_fullM(model,mass,data->qM);
  for(int i=0;i<9;i++)for(int j=9;j<21;j++)NEED(mass[21*i+j]==0&&mass[21*j+i]==0);
  for(int i=0;i<9;i++) {
   int dof=model->jnt_dofadr[joint[i]],a=act[i],at=data->moment_rowadr[a];
   NEED(data->moment_rownnz[a]==1&&at>=0&&at<model->nJmom&&data->moment_colind[at]==dof&&data->actuator_moment[at]==1);
   NEED(!memcmp(data->qpos+i,cases[row]+i,8)&&!memcmp(data->qvel+dof,cases[row]+9+i,8)&&!memcmp(data->ctrl+a,cases[row]+18+i,8)&&!memcmp(data->qfrc_applied+dof,cases[row]+27+i,8));
   for(int j=0;j<9;j++)result[row][9*i+j]=mass[21*dof+model->jnt_dofadr[joint[j]]];
   result[row][81+i]=data->qfrc_bias[dof];result[row][90+i]=data->qfrc_passive[dof];result[row][99+i]=data->qfrc_actuator[dof];result[row][108+i]=data->qfrc_smooth[dof];result[row][117+i]=data->qacc_smooth[dof];
  }
  for(int i=0;i<model->nv;i++)NEED(data->qfrc_constraint[i]==0);
  progress_event(progress,row,"before_step");mj_step(model,data);progress_event(progress,row,"after_step");
  NEED(data->time==model->opt.timestep);no_warnings(data);
  for(int i=0;i<9;i++) {int dof=model->jnt_dofadr[joint[i]];result[row][126+i]=data->qpos[model->jnt_qposadr[joint[i]]];result[row][135+i]=data->qvel[dof];result[row][144+i]=(data->qvel[dof]-cases[row][9+i])/model->opt.timestep;}
  result[row][153]=data->time;for(int i=0;i<model->nv;i++)NEED(data->qfrc_constraint[i]==0);
  for(int i=0;i<154;i++)NEED(isfinite(result[row][i]));
  /* Deliberately no post-step forward: state is current, geometry remains old. */
 }
 NEED(fwrite(result,1,sizeof result,output)==sizeof result&&!fclose(output)&&!fclose(progress));
 mj_deleteData(data);mj_deleteModel(model);
 printf("{\"passed\":true,\"explicit_forwards\":3,\"steps\":3,\"post_step_forwards\":0,\"NN_calls\":0,\"render_calls\":0,\"rows\":3,\"words_per_row\":154,\"assertions\":%zu}\n",checks);return 0;
}
