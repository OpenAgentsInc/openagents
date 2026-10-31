#ifndef PSIONIC_PANDA_ENV_H
#define PSIONIC_PANDA_ENV_H
#include <stddef.h>
#include <stdint.h>

/* Opt-in independent MuJoCo3.3.1 simulation tooling. No controller, success
   predicate, action clipping or policy scheduling is implemented here.
   Every int-returning function returns 1 on success and 0 on failure.
   open returns a non-NULL handle on success. These are effect booleans,
   unlike the Coax PeStatus/PcStatus enums whose success value is 0. */
typedef struct PandaEnv PandaEnv;
typedef struct {
  int64_t nq,nv,nu,na,state_count;
  double physics_dt;
} PandaDimensions;
typedef struct {
  double time;
  double q[7],qd[7],mass[49],jp[21],jr[21];
  double site_pos[3],site_mat[9],velocity[6],body_quat_wxyz[4],finger_q[2];
  double finger_lower[2],finger_upper[2];
  double bias[7],torque_min[7],torque_max[7];
  double target_pos[3],target_mat[9],target_size[3],book_pos[3];
} PandaTelemetry;

/* Caller supplies24 Coax-selected semantic names: arm joints7,arm actuators7,
   finger joints2,EEFsite,EEFbody,targetsite,bookbody,cameras2,finger actuators2. */
PandaEnv *panda_env_open(const char *xml_path,const char *const bindings[24],char *error,size_t error_size);
void panda_env_close(PandaEnv *env);
int panda_env_dimensions(PandaEnv *env,PandaDimensions *out);
/* Reset is an SDK effect. Controller/gripper reset state belongs to Coax. */
int panda_env_reset(PandaEnv *env,const double *state,size_t count);
/* Exact flattened SDK state copy; no forward or integration. */
int panda_env_state(PandaEnv *env,double *state,size_t count);
int panda_env_forward(PandaEnv *env);
/* Neither sample nor render implicitly calls mj_forward. */
int panda_env_sample(PandaEnv *env,PandaTelemetry *out);
/* Exactly one physics step; control is copied without policy arithmetic. */
int panda_env_step(PandaEnv *env,const double *control,size_t count);
/* camera0=agentview,1=robot0_eye_in_hand; raw OpenGL RGB256x256HWC u8. */
int panda_env_render(PandaEnv *env,int camera,unsigned char *rgb,size_t capacity);
#endif
