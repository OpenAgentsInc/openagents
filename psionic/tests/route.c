/* Chapter 164a against route_oracle.py, which simulated, trained, logged and
   estimated the same experiment from the specification in exactly rounded
   sums. Tolerances were frozen before comparison: every draw, logged action,
   choice, count and refusal exactly; values from true probabilities 1e-12;
   trained parameters 1e-9 relative; estimates and their errors 1e-9. */
#include "route-spans.h"
#include "route-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

static coax_v_f64 fv(double *p,int64_t n){return (coax_v_f64){p,n};}
static coax_v_s64 sv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}
static double worst_value,worst_param,worst_estimate;
static void near(double got,double want,double tol,double *worst){
    double e=fabs(got-want)/fmax(fabs(want),1);assert(e<=tol);if(e>*worst)*worst=e;
}
static double world_a[15]={-1,-3,-6,0,4, -0.5,2,-3,0,2, -0.5,0.5,-2.5,0,3.5};
static const coax_t_TbConfig config={40,16,0.5,0,1,2};

/* Full information (actions NULL): every row with its model's outcome. Logs:
   each model's logged rows. False when a model was never logged. */
static bool fit(double *x,const double *success,const int64_t *actions,int64_t n,double *params){
    static double y[1500];static int64_t rows[1500],order[1500];
    for(int k=0;k<3;k++){
        int64_t m=n;
        if(!actions){for(int64_t i=0;i<n;i++){y[i]=success[3*i+k];rows[i]=i;}}
        else{for(int64_t i=0;i<n;i++)y[i]=success[i];m=psionic_rt_rows(sv((int64_t*)actions,n),k,sv(rows,n));}
        if(m==0)return false;
        coax_t_TbTrain t=tb_train((coax_t_TbModel){4,0,true},config,fv(x,4*n),fv(y,n),sv(rows,m),fv(params+5*k,5),sv(order,n),false);
        assert(t.status==COAX_TbStatus_ok);
    }
    return true;
}

typedef struct {double truth,earned;coax_t_RtEstimate fitted,average;coax_t_RtStatus refused;int64_t actions[1000],target[1000];double outcome[1000],reward[1000];} judged;
static void judge(double *router,uint64_t tasks,uint64_t logs,double *truth,bool large,judged *j){
    static double x[4000],s[3000],p[3000],q[3000],score[3000],pr[3000];double model[15];
    memset(j,0,sizeof *j);
    assert(psionic_rt_simulate(tasks,fv(truth,15),fv(x,4000),fv(s,3000),fv(p,3000))==COAX_RtStatus_ok);
    assert(psionic_rt_price(fv(x,4000),fv(q,3000))==COAX_RtStatus_ok);
    assert(psionic_rt_scores(fv(router,15),fv(x,4000),fv(score,3000))==COAX_RtStatus_ok);
    assert(psionic_rt_choose(fv(score,3000),fv(q,3000),sv(j->target,1000))==COAX_RtStatus_ok);
    assert(psionic_rt_old(fv(x,4000),0.2,large,fv(pr,3000))==COAX_RtStatus_ok);
    assert(psionic_rt_log(logs,fv(pr,3000),fv(s,3000),fv(q,3000),sv(j->actions,1000),fv(j->outcome,1000),fv(j->reward,1000))==COAX_RtStatus_ok);
    j->truth=rt_value(sv(j->target,1000),fv(p,3000),fv(q,3000)).value;
    double sum=0;for(int i=0;i<1000;i++)sum+=j->reward[i];j->earned=sum/1000;
    if(!fit(x,j->outcome,j->actions,1000,model)){
        j->refused=rt_estimate(sv(j->target,1000),sv(j->actions,1000),fv(j->reward,1000),fv(pr,3000),fv(score,3000),fv(q,3000)).status;
        return;
    }
    assert(psionic_rt_scores(fv(model,15),fv(x,4000),fv(score,3000))==COAX_RtStatus_ok);
    j->fitted=rt_estimate(sv(j->target,1000),sv(j->actions,1000),fv(j->reward,1000),fv(pr,3000),fv(score,3000),fv(q,3000));
    assert(psionic_rt_average(sv(j->actions,1000),fv(j->outcome,1000),fv(score,3000))==COAX_RtStatus_ok);
    j->average=rt_estimate(sv(j->target,1000),sv(j->actions,1000),fv(j->reward,1000),fv(pr,3000),fv(score,3000),fv(q,3000));
}
static void estimates(coax_t_RtEstimate e,const double *want){
    assert(e.status==COAX_RtStatus_ok);
    near(e.naive,want[0],1e-9,&worst_estimate);near(e.direct,want[1],1e-9,&worst_estimate);
    near(e.ips,want[2],1e-9,&worst_estimate);near(e.dr,want[3],1e-9,&worst_estimate);
}

int main(void){
    static double x[6000],s[4500],p[4500],tx[4000],tp[3000],q[3000],score[3000];static int64_t pick[1000],choice[1000];
    double router[15];
    assert(psionic_rt_simulate(164,fv(world_a,15),fv(x,6000),fv(s,4500),fv(p,4500))==COAX_RtStatus_ok);
    assert(fit(x,s,NULL,1500,router));
    for(int k=0;k<15;k++)near(router[k],rt_oracle_router[k],1e-9,&worst_param);
    assert(psionic_rt_simulate(165,fv(world_a,15),fv(tx,4000),fv(s,3000),fv(tp,3000))==COAX_RtStatus_ok);
    assert(psionic_rt_price(fv(tx,4000),fv(q,3000))==COAX_RtStatus_ok);
    assert(psionic_rt_scores(fv(router,15),fv(tx,4000),fv(score,3000))==COAX_RtStatus_ok);
    assert(psionic_rt_choose(fv(score,3000),fv(q,3000),sv(pick,1000))==COAX_RtStatus_ok);
    for(int k=0;k<5;k++){
        for(int i=0;i<1000;i++)choice[i]=k<3?k:(int64_t)tx[4*i+1];
        if(k==4)assert(psionic_rt_choose(fv(tp,3000),fv(q,3000),sv(choice,1000))==COAX_RtStatus_ok);
        near(rt_value(sv(choice,1000),fv(tp,3000),fv(q,3000)).value,rt_oracle_values[k<4?k:5],1e-12,&worst_value);
    }
    near(rt_value(sv(pick,1000),fv(tp,3000),fv(q,3000)).value,rt_oracle_values[4],1e-12,&worst_value);
    int64_t counts[3]={0,0,0},agree=0;
    for(int i=0;i<1000;i++){counts[pick[i]]++;agree+=pick[i]==choice[i];}
    for(int k=0;k<3;k++)assert(counts[k]==rt_oracle_counts[k]);
    assert(agree==rt_oracle_counts[3]);

    static judged j,b;
    judge(router,166,167,world_a,true,&j);
    for(int i=0;i<1000;i++)assert(j.actions[i]==rt_oracle_log_actions[i]&&j.target[i]==rt_oracle_log_target[i]);
    near(j.truth,rt_oracle_log_values[0],1e-12,&worst_value);near(j.earned,rt_oracle_log_values[1],1e-12,&worst_value);
    assert(j.fitted.matched==rt_oracle_log_matched[0]&&j.average.matched==rt_oracle_log_matched[1]);
    estimates(j.fitted,rt_oracle_log_fitted);estimates(j.average,rt_oracle_log_average);
    double e[6]={0},e2[6]={0};
    for(int seed=0;seed<20;seed++){
        judge(router,1000+seed,2000+seed,world_a,true,&j);
        double d[6]={j.fitted.naive,j.fitted.direct,j.fitted.ips,j.fitted.dr,j.average.direct,j.average.dr};
        for(int m=0;m<6;m++){double err=d[m]-j.truth;e[m]+=err;e2[m]+=err*err;}
    }
    for(int m=0;m<6;m++){
        near(e[m]/20,rt_oracle_errors[2*m],1e-9,&worst_estimate);near(sqrt(e2[m]/20),rt_oracle_errors[2*m+1],1e-9,&worst_estimate);
    }
    /* Logs without the large model refuse, and look the same in a world where it is worse. */
    double world_b[15];memcpy(world_b,world_a,sizeof world_b);world_b[14]=0.5;
    judge(router,166,167,world_a,false,&j);judge(router,166,167,world_b,false,&b);
    assert(rt_oracle_narrow[0]==1&&j.refused==COAX_RtStatus_support&&b.refused==COAX_RtStatus_support);
    assert(rt_oracle_narrow[1]==1&&memcmp(j.actions,b.actions,sizeof j.actions)==0&&memcmp(j.outcome,b.outcome,sizeof j.outcome)==0);
    assert(memcmp(j.reward,b.reward,sizeof j.reward)==0);
    near(b.truth,rt_oracle_world_b[0],1e-12,&worst_value);
    judge(router,166,167,world_b,true,&b);near(b.fitted.ips,rt_oracle_world_b[1],1e-9,&worst_estimate);

    /* Native refusals: NaN and infinity, and overlapping storage. */
    double nx[4]={NAN,0,0,0},nq[3],np[3]={0.5,NAN,0.5},nc[3]={0,0,0};int64_t na[1];
    assert(psionic_rt_price(fv(nx,4),fv(nq,3))==COAX_RtStatus_number);
    nx[0]=INFINITY;assert(psionic_rt_price(fv(nx,4),fv(nq,3))==COAX_RtStatus_number);
    assert(psionic_rt_choose(fv(np,3),fv(nc,3),sv(na,1))==COAX_RtStatus_number);
    nx[0]=0;assert(psionic_rt_price(fv(nx,4),fv(nx+1,3))==COAX_RtStatus_alias);
    assert(psionic_rt_scores(fv(router,15),fv(tx,4000),fv(tx+1000,3000))==COAX_RtStatus_alias);
    assert(psionic_rt_simulate(1,fv(world_a,15),fv(x,4),fv(x+4,3),fv(x+6,3))==COAX_RtStatus_alias);
    assert(psionic_rt_log(1,fv(tp,3),fv(s,3),fv(q,3),sv(na,1),fv(tp,1),fv(nq,1))==COAX_RtStatus_alias);
    assert(psionic_rt_average(sv(pick,2),fv(score,2),fv(score+1,6))==COAX_RtStatus_alias);
    assert(psionic_rt_rows(sv(pick,2),0,sv(pick+1,2))==-1);
    printf("route: router parameters within %.1e of the oracle; every draw, logged action, choice and count equal; "
        "values within %.1e and estimates within %.1e; unseen-action refusal and identical logs reproduced; NaN, infinity and overlap refused\n",
        worst_param,worst_value,worst_estimate);
    return 0;
}
