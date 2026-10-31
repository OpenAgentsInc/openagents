/* Firm reason: 2 (independent oracle). Closed-form affine imitation target,
   independently calculated SI clipping/integration, and held-out rollouts.
   No robotics runtime, simulator package, model download or actuator access. */
#include "robotics.h"
#include <assert.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static unsigned cases;
static double clip(double x,double limit){return x < -limit ? -limit : x > limit ? limit : x;}
static bool near(double a,double b,double e){return fabs(a-b)<=e;}
static coax_t_RkState state(double x,double y,double gx,double gy){return (coax_t_RkState){{x,y},{gx,gy}};}
static bool same_state(coax_t_RkState a,coax_t_RkState b){return !memcmp(&a,&b,sizeof a);}
static coax_t_RkVec expert(coax_t_RkState s){return (coax_t_RkVec){clip(s.goal.x-s.position.x,.5),clip(s.goal.y-s.position.y,.5)};}
static coax_t_RkVec action(coax_t_RkPolicy p,coax_t_RkState s){return (coax_t_RkVec){.5*clip(p.x_gain*((s.goal.x-s.position.x)/.25)+p.x_bias,1),.5*clip(p.y_gain*((s.goal.y-s.position.y)/.25)+p.y_bias,1)};}
static coax_t_RkState integrate(coax_t_RkState s,coax_t_RkVec v){s.position.x+=.1*v.x;s.position.y+=.1*v.y;return s;}
static void state_field(coax_t_RkState *s,unsigned i,double v){if(i==0)s->position.x=v;else if(i==1)s->position.y=v;else if(i==2)s->goal.x=v;else s->goal.y=v;}
static void policy_field(coax_t_RkPolicy *p,unsigned i,double v){if(i==0)p->x_gain=v;else if(i==1)p->x_bias=v;else if(i==2)p->y_gain=v;else p->y_bias=v;}
static const double eval[6][4]={
 {-0.9,0.9,0.7,-0.7},{0.8,-0.8,-0.6,0.6},{-0.55,-0.75,0.35,0.85},
 {0.65,0.45,-0.85,-0.25},{0,0,0.45,-0.9},{-1,1,1,-1}};
static const double train[4][4]={
 {-0.5,-0.25,-0.25,0},{0.5,0.25,0.25,0},{0.25,-0.5,0.375,-0.625},{-0.25,0.5,-0.375,0.625}};

static void boundary_checks(void){
 const double grid[]={-1,-.75,-.5,0,.5,.75,1};
 coax_t_RkPolicy ideal={.5,0,.5,0};
 for(unsigned x=0;x<7;x++)for(unsigned y=0;y<7;y++)for(unsigned gx=0;gx<7;gx++)for(unsigned gy=0;gy<7;gy++){
  coax_t_RkState s=state(grid[x],grid[y],grid[gx],grid[gy]);
  coax_t_RkAction a=rk_expert(s),b=rk_action(ideal,s);coax_t_RkVec want=expert(s);
  assert(a.status==COAX_RkStatus_ok&&b.status==COAX_RkStatus_ok);
  assert(a.velocity.x==want.x&&a.velocity.y==want.y&&b.velocity.x==want.x&&b.velocity.y==want.y);
  assert(a.clipped==(fabs(s.goal.x-s.position.x)>.5||fabs(s.goal.y-s.position.y)>.5));
  assert(b.clipped==(fabs(2*(s.goal.x-s.position.x))>1||fabs(2*(s.goal.y-s.position.y))>1));
  coax_t_RkStep t=rk_step(s,want,.1);coax_t_RkState next=integrate(s,want);
  assert(t.status==COAX_RkStatus_ok&&same_state(t.state,next));cases++;
 }
 const double gains[]={-8,-2,0,2,8},biases[]={-2,0,2};
 for(unsigned i=0;i<5;i++)for(unsigned j=0;j<5;j++)for(unsigned k=0;k<3;k++)for(unsigned l=0;l<3;l++){
  coax_t_RkPolicy p={gains[i],biases[k],gains[j],biases[l]};
  for(unsigned h=0;h<6;h++){
   coax_t_RkState s=state(eval[h][0],eval[h][1],eval[h][2],eval[h][3]);
   coax_t_RkAction got=rk_action(p,s);coax_t_RkVec want=action(p,s);
   double ux=p.x_gain*((s.goal.x-s.position.x)/.25)+p.x_bias,uy=p.y_gain*((s.goal.y-s.position.y)/.25)+p.y_bias;
   assert(got.status==COAX_RkStatus_ok&&got.velocity.x==want.x&&got.velocity.y==want.y);
   assert(got.clipped==(fabs(ux)>1||fabs(uy)>1));cases++;
  }
 }
 const double bad_state[]={nextafter(-1,-INFINITY),nextafter(1,INFINITY),NAN,INFINITY,-INFINITY};
 for(unsigned k=0;k<5;k++)for(unsigned field=0;field<4;field++){
  coax_t_RkState s=state(.25,-.25,.5,-.5);state_field(&s,field,bad_state[k]);
  assert(rk_expert(s).status==COAX_RkStatus_state&&rk_action(ideal,s).status==COAX_RkStatus_state);
  coax_t_RkStep t=rk_step(s,(coax_t_RkVec){0,0},.1);assert(t.status==COAX_RkStatus_state&&same_state(t.state,s));cases++;
 }
 const double bad_gain[]={nextafter(-8,-INFINITY),nextafter(8,INFINITY),NAN,INFINITY,-INFINITY};
 const double bad_bias[]={nextafter(-2,-INFINITY),nextafter(2,INFINITY),NAN,INFINITY,-INFINITY};
 for(unsigned k=0;k<5;k++)for(unsigned field=0;field<4;field++){
  coax_t_RkPolicy p=ideal;policy_field(&p,field,(field%2)?bad_bias[k]:bad_gain[k]);
  assert(rk_action(p,state(0,0,.25,-.25)).status==COAX_RkStatus_policy);
  coax_t_RkEval refused=rk_evaluate(p);assert(refused.status==COAX_RkStatus_policy&&refused.episodes==0&&refused.steps==0);cases++;
 }
 const double bad_action[]={nextafter(-.5,-INFINITY),nextafter(.5,INFINITY),NAN,INFINITY,-INFINITY};
 for(unsigned k=0;k<5;k++)for(unsigned field=0;field<2;field++){
  coax_t_RkState s=state(0,0,.25,-.25);coax_t_RkVec v={0,0};if(field==0)v.x=bad_action[k];else v.y=bad_action[k];
  coax_t_RkStep t=rk_step(s,v,.1);assert(t.status==COAX_RkStatus_action&&same_state(t.state,s));cases++;
 }
 const double bad_time[]={0,-.1,.05,.2,nextafter(.1,0),nextafter(.1,INFINITY),NAN,INFINITY,-INFINITY};
 for(unsigned k=0;k<9;k++){
  coax_t_RkState s=state(.5,-.5,0,0);coax_t_RkStep t=rk_step(s,(coax_t_RkVec){.25,-.25},bad_time[k]);
  assert(t.status==COAX_RkStatus_time&&same_state(t.state,s));cases++;
 }
 for(unsigned field=0;field<2;field++)for(int sign=-1;sign<=1;sign+=2){
  coax_t_RkState s=state(0,0,0,0);coax_t_RkVec v={0,0};
  if(field==0){s.position.x=sign;v.x=sign*.5;}else{s.position.y=sign;v.y=sign*.5;}
  coax_t_RkStep t=rk_step(s,v,.1);assert(t.status==COAX_RkStatus_workspace&&same_state(t.state,s));cases++;
 }
}

static void episode_checks(void){
 for(int i=0;i<4;i++){
  coax_t_RkEpisode e=rk_episode(i);assert(e.status==COAX_RkStatus_ok&&e.split==0);
  assert(same_state(e.state,state(train[i][0],train[i][1],train[i][2],train[i][3])));cases++;
 }
 for(int i=0;i<6;i++){
  coax_t_RkEpisode e=rk_episode(100+i);assert(e.status==COAX_RkStatus_ok&&e.split==1);
  assert(same_state(e.state,state(eval[i][0],eval[i][1],eval[i][2],eval[i][3])));cases++;
 }
 const int64_t bad[]={-1,4,99,106,INT64_MIN,INT64_MAX};
 for(unsigned i=0;i<6;i++){assert(rk_episode(bad[i]).status==COAX_RkStatus_state);cases++;}
}

static void demonstration_checks(void){
 coax_t_TbConfig config=rk_training_config();
 assert(config.epochs==128&&config.batch==32&&config.rate==1&&config.decay==0&&config.init_seed==0&&config.shuffle_seed==112);cases++;
 coax_t_RkDemonstrations d=rk_demonstrations();assert(d.count==32&&d.episodes==4);
 for(unsigned episode=0;episode<4;episode++){
  coax_t_RkState s=state(train[episode][0],train[episode][1],train[episode][2],train[episode][3]);
  for(unsigned tick=0;tick<8;tick++){
   unsigned row=episode*8+tick;coax_t_RkVec v=expert(s);
   assert(d.rows.e[row]==row&&d.x.e[row]==(s.goal.x-s.position.x)/.25&&d.y.e[row]==v.x/.5);
   assert(d.other_x.e[row]==(s.goal.y-s.position.y)/.25&&d.other_y.e[row]==v.y/.5);
   s=integrate(s,v);cases++;
  }
 }
}

static void evaluation_check(coax_t_RkPolicy p,bool use_expert,const char *name){
 double sq=0,max_error=0;int successes=0,worst_episode=-1,worst_axis=0;
 for(unsigned episode=0;episode<6;episode++){
  coax_t_RkState teacher=state(eval[episode][0],eval[episode][1],eval[episode][2],eval[episode][3]),learner=teacher;
  for(unsigned tick=0;tick<96;tick++){
   coax_t_RkVec reference=expert(teacher),predicted=use_expert?reference:action(p,teacher);
   double dx=predicted.x-reference.x,dy=predicted.y-reference.y;sq+=dx*dx+dy*dy;
   teacher=integrate(teacher,reference);learner=integrate(learner,use_expert?expert(learner):action(p,learner));
  }
  double ex=fabs(learner.position.x-learner.goal.x),ey=fabs(learner.position.y-learner.goal.y),e=fmax(ex,ey);
  if(e<=.01)successes++;
  if(e>max_error){max_error=e;worst_episode=100+(int)episode;worst_axis=ex>=ey?0:1;}
 }
 coax_t_RkEval actual=use_expert?rk_evaluate_expert():rk_evaluate(p);
 assert(actual.status==COAX_RkStatus_ok&&actual.episodes==6&&actual.steps==576&&actual.successes==successes);
 assert(actual.worst_episode==worst_episode&&actual.worst_axis==worst_axis);
 assert(near(actual.mse,sq/1152,1e-14)&&near(actual.final_error_max,max_error,1e-13));cases++;
 printf("robotics oracle %s: %d/6 successes, mse=%.17g, max-final-error=%.17g m\n",name,successes,actual.mse,actual.final_error_max);
}

int main(void){
 boundary_checks();episode_checks();demonstration_checks();
 const double errors[]={-.01,-0x1p-1074,-0.0,0.0,0x1p-1074,nextafter(.01,0),.01,nextafter(.01,INFINITY),.5,NAN,INFINITY,-INFINITY};
 for(unsigned i=0;i<sizeof errors/sizeof errors[0];i++){assert(rk_success(errors[i])==(errors[i]>=0&&errors[i]<=.01));cases++;}
 coax_t_RkFit learned=rk_train();
 assert(learned.status==COAX_RkStatus_ok&&learned.steps==256);
 assert(near(learned.policy.x_gain,.5,1e-8)&&near(learned.policy.y_gain,.5,1e-8));
 assert(fabs(learned.policy.x_bias)<1e-8&&fabs(learned.policy.y_bias)<1e-8&&learned.loss_x<1e-14&&learned.loss_y<1e-14);cases++;
 evaluation_check((coax_t_RkPolicy){0},false,"zero");evaluation_check(learned.policy,false,"learned");evaluation_check((coax_t_RkPolicy){0},true,"expert");
 evaluation_check((coax_t_RkPolicy){.25,0,.5,0},false,"half-x-gain");evaluation_check((coax_t_RkPolicy){.5,0,.25,0},false,"half-y-gain");
 coax_t_RkEval refused=rk_evaluate((coax_t_RkPolicy){0,-1,0,1});
 assert(refused.status==COAX_RkStatus_workspace&&refused.episodes==0&&refused.steps==0&&refused.successes==0);cases++;
 printf("robotics native: %u independent SI/clip/boundary/episode/learning/rollout cases passed\n",cases);
 return 0;
}
