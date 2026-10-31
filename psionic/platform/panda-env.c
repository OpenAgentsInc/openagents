/* MuJoCo/CGL platform effects for the opt-in original LIBERO/Panda evaluation.
   All new controller, observation conversion, scheduling and task decisions
   live in Coax. Original scenes/assets remain separately authenticated data. */
#include "panda-env.h"
#include <mujoco/mujoco.h>
#include <OpenGL/OpenGL.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct PandaEnv {
  mjModel *model;
  mjData *data;
  mjtNum *dense,*jacp,*jacr;
  int joints[7],qpos[7],qvel[7],actuators[7],fingers[2],finger_qvel[2],finger_actuators[2];
  int site,body,target,book,cameras[2];
  CGLContextObj gl;
  CGLPixelFormatObj pixel;
  mjrContext context;
  mjvScene scene;
  mjvOption option;
  int render_ready,render_attempted,locked;
};

static void panda_error(char *out,size_t size,const char *message) {
  if(out&&size)snprintf(out,size,"%s",message);
}
static int panda_name(PandaEnv *e,int kind,const char *name) {
  return mj_name2id(e->model,kind,name);
}
void panda_env_close(PandaEnv *e) {
  if(!e)return;
  if(e->gl)CGLSetCurrentContext(e->gl);
  if(e->render_ready){mjr_freeContext(&e->context);mjv_freeScene(&e->scene);}
  if(e->locked)CGLUnlockContext(e->gl);
  if(e->gl){CGLSetCurrentContext(NULL);CGLDestroyContext(e->gl);}
  if(e->pixel)CGLDestroyPixelFormat(e->pixel);
  if(e->data)mj_deleteData(e->data);
  if(e->model)mj_deleteModel(e->model);
  free(e->dense);free(e->jacp);free(e->jacr);free(e);
}
PandaEnv *panda_env_open(const char *path,const char *const bindings[24],char *error,size_t size) {
  if(!path||!bindings){panda_error(error,size,"missing XML path or bindings");return NULL;}
  for(int i=0;i<24;i++)if(!bindings[i]){panda_error(error,size,"missing binding");return NULL;}
  PandaEnv *e=calloc(1,sizeof(*e));
  if(!e){panda_error(error,size,"allocation failed");return NULL;}
  e->model=mj_loadXML(path,NULL,error,(int)(size>2147483647?2147483647:size));
  if(!e->model){panda_env_close(e);return NULL;}
  e->data=mj_makeData(e->model);
  size_t nv=(size_t)e->model->nv;
  if(!nv||nv>SIZE_MAX/nv/sizeof(mjtNum)){panda_error(error,size,"invalid mass extent");panda_env_close(e);return NULL;}
  e->dense=calloc(nv*nv,sizeof(mjtNum));e->jacp=calloc(3*nv,sizeof(mjtNum));e->jacr=calloc(3*nv,sizeof(mjtNum));
  if(!e->data||!e->dense||!e->jacp||!e->jacr){panda_error(error,size,"allocation failed");panda_env_close(e);return NULL;}
  for(int i=0;i<7;i++){
    e->joints[i]=panda_name(e,mjOBJ_JOINT,bindings[i]);e->actuators[i]=panda_name(e,mjOBJ_ACTUATOR,bindings[7+i]);
    if(e->joints[i]<0||e->actuators[i]<0){panda_error(error,size,"missing arm handle");panda_env_close(e);return NULL;}
    e->qpos[i]=e->model->jnt_qposadr[e->joints[i]];e->qvel[i]=e->model->jnt_dofadr[e->joints[i]];
  }
  for(int i=0;i<2;i++){
    int id=panda_name(e,mjOBJ_JOINT,bindings[14+i]);e->finger_actuators[i]=panda_name(e,mjOBJ_ACTUATOR,bindings[22+i]);
    if(id<0||e->finger_actuators[i]<0){panda_error(error,size,"missing finger handle");panda_env_close(e);return NULL;}
    e->fingers[i]=e->model->jnt_qposadr[id];e->finger_qvel[i]=e->model->jnt_dofadr[id];
  }
  e->site=panda_name(e,mjOBJ_SITE,bindings[16]);e->body=panda_name(e,mjOBJ_BODY,bindings[17]);
  e->target=panda_name(e,mjOBJ_SITE,bindings[18]);e->book=panda_name(e,mjOBJ_BODY,bindings[19]);
  e->cameras[0]=panda_name(e,mjOBJ_CAMERA,bindings[20]);e->cameras[1]=panda_name(e,mjOBJ_CAMERA,bindings[21]);
  if(e->site<0||e->body<0||e->target<0||e->book<0||e->cameras[0]<0||e->cameras[1]<0){panda_error(error,size,"missing source scene handle");panda_env_close(e);return NULL;}
  if(error&&size)error[0]=0;
  return e;
}
int panda_env_dimensions(PandaEnv *e,PandaDimensions *out) {
  if(!e||!out)return 0;const mjModel *m=e->model;
  *out=(PandaDimensions){m->nq,m->nv,m->nu,m->na,1+m->nq+m->nv+m->na,m->opt.timestep};return 1;
}
int panda_env_reset(PandaEnv *e,const double *state,size_t count) {
  if(!e||!state||count!=(size_t)(1+e->model->nq+e->model->nv+e->model->na))return 0;
  mj_resetData(e->model,e->data);e->data->time=state[0];
  memcpy(e->data->qpos,state+1,(size_t)e->model->nq*sizeof(double));
  memcpy(e->data->qvel,state+1+e->model->nq,(size_t)e->model->nv*sizeof(double));
  if(e->model->na)memcpy(e->data->act,state+1+e->model->nq+e->model->nv,(size_t)e->model->na*sizeof(double));
  mj_forward(e->model,e->data);return 1;
}
int panda_env_state(PandaEnv *e,double *state,size_t count) {
  if(!e||!state||count!=(size_t)(1+e->model->nq+e->model->nv+e->model->na))return 0;
  state[0]=e->data->time;memcpy(state+1,e->data->qpos,(size_t)e->model->nq*sizeof(double));
  memcpy(state+1+e->model->nq,e->data->qvel,(size_t)e->model->nv*sizeof(double));
  if(e->model->na)memcpy(state+1+e->model->nq+e->model->nv,e->data->act,(size_t)e->model->na*sizeof(double));return 1;
}
int panda_env_forward(PandaEnv *e){if(!e)return 0;mj_forward(e->model,e->data);return 1;}
int panda_env_robot_state(PandaEnv *e,PandaRobotStateV1 *out) {
  if(!e||!out)return 0;PandaRobotStateV1 state;
  for(int i=0;i<7;i++){state.q[i]=e->data->qpos[e->qpos[i]];state.qd[i]=e->data->qvel[e->qvel[i]];}
  for(int i=0;i<2;i++){state.q[7+i]=e->data->qpos[e->fingers[i]];state.qd[7+i]=e->data->qvel[e->finger_qvel[i]];}
  *out=state;return 1;
}
int panda_env_sample(PandaEnv *e,PandaTelemetry *out) {
  if(!e||!out)return 0;PandaTelemetry s={0};const mjModel *m=e->model;const mjData *d=e->data;
  mj_fullM(m,e->dense,d->qM);mj_jacSite(m,d,e->jacp,e->jacr,e->site);s.time=d->time;
  for(int i=0;i<7;i++){
    s.q[i]=d->qpos[e->qpos[i]];s.qd[i]=d->qvel[e->qvel[i]];s.bias[i]=d->qfrc_bias[e->qvel[i]];
    s.torque_min[i]=m->actuator_ctrlrange[2*e->actuators[i]];s.torque_max[i]=m->actuator_ctrlrange[2*e->actuators[i]+1];
    for(int j=0;j<7;j++)s.mass[i*7+j]=e->dense[e->qvel[i]*m->nv+e->qvel[j]];
    for(int row=0;row<3;row++){s.jp[row*7+i]=e->jacp[row*m->nv+e->qvel[i]];s.jr[row*7+i]=e->jacr[row*m->nv+e->qvel[i]];}
  }
  memcpy(s.site_pos,d->site_xpos+3*e->site,3*sizeof(double));memcpy(s.site_mat,d->site_xmat+9*e->site,9*sizeof(double));
  mjtNum velocity[6];mj_objectVelocity(m,d,mjOBJ_SITE,e->site,velocity,0);memcpy(s.velocity,velocity+3,3*sizeof(double));memcpy(s.velocity+3,velocity,3*sizeof(double));
  memcpy(s.body_quat_wxyz,d->xquat+4*e->body,4*sizeof(double));for(int i=0;i<2;i++){s.finger_q[i]=d->qpos[e->fingers[i]];s.finger_lower[i]=m->actuator_ctrlrange[2*e->finger_actuators[i]];s.finger_upper[i]=m->actuator_ctrlrange[2*e->finger_actuators[i]+1];}
  memcpy(s.target_pos,d->site_xpos+3*e->target,3*sizeof(double));memcpy(s.target_mat,d->site_xmat+9*e->target,9*sizeof(double));
  memcpy(s.target_size,m->site_size+3*e->target,3*sizeof(double));memcpy(s.book_pos,d->xpos+3*e->book,3*sizeof(double));*out=s;return 1;
}
int panda_env_step(PandaEnv *e,const double *control,size_t count) {
  if(!e||!control||count!=(size_t)e->model->nu)return 0;
  for(int i=0;i<7;i++)e->data->ctrl[e->actuators[i]]=control[i];for(int i=0;i<2;i++)e->data->ctrl[e->finger_actuators[i]]=control[7+i];mj_step(e->model,e->data);return 1;
}
static int panda_render_open(PandaEnv *e) {
  if(e->render_attempted)return 0;e->render_attempted=1;
  CGLPixelFormatAttribute attr[]={kCGLPFAOpenGLProfile,(CGLPixelFormatAttribute)kCGLOGLPVersion_Legacy,kCGLPFAColorSize,(CGLPixelFormatAttribute)24,kCGLPFAAlphaSize,(CGLPixelFormatAttribute)8,kCGLPFADepthSize,(CGLPixelFormatAttribute)24,kCGLPFAStencilSize,(CGLPixelFormatAttribute)8,kCGLPFAMultisample,kCGLPFASampleBuffers,(CGLPixelFormatAttribute)1,kCGLPFASamples,(CGLPixelFormatAttribute)4,kCGLPFAAccelerated,(CGLPixelFormatAttribute)0};
  GLint count=0;if(CGLChoosePixelFormat(attr,&e->pixel,&count)!=kCGLNoError||!e->pixel)return 0;
  if(CGLCreateContext(e->pixel,NULL,&e->gl)!=kCGLNoError||!e->gl)return 0;
  if(CGLSetCurrentContext(e->gl)!=kCGLNoError||CGLLockContext(e->gl)!=kCGLNoError)return 0;e->locked=1;
  mjv_defaultScene(&e->scene);mjv_makeScene(e->model,&e->scene,10000);mjv_defaultOption(&e->option);e->option.geomgroup[0]=0;e->option.geomgroup[1]=1;
  mjr_defaultContext(&e->context);mjr_makeContext(e->model,&e->context,mjFONTSCALE_150);mjr_setBuffer(mjFB_OFFSCREEN,&e->context);e->render_ready=1;return 1;
}
int panda_env_render(PandaEnv *e,int camera,unsigned char *rgb,size_t capacity) {
  if(!e||camera<0||camera>1||!rgb||capacity<196608)return 0;
  if(!e->render_ready&&!panda_render_open(e))return 0;
  CGLSetCurrentContext(e->gl);mjvCamera cam;mjv_defaultCamera(&cam);cam.type=mjCAMERA_FIXED;cam.fixedcamid=e->cameras[camera];
  mjv_updateScene(e->model,e->data,&e->option,NULL,&cam,mjCAT_ALL,&e->scene);mjrRect rect={0,0,256,256};mjr_render(rect,&e->scene,&e->context);mjr_readPixels(rgb,NULL,rect,&e->context);return 1;
}
