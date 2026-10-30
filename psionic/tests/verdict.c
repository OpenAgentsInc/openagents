/* Chapter 180 against verdict_oracle.py, which wrote the gates again and took
   its runs, maps and thresholds from chapters 179, 156 and 157's independent
   oracles. Tolerances were frozen before comparison: verdicts, reasons, task
   counts, replicates at or below each line, confident errors and right
   answers exactly; gains, floors, log losses, calibration errors, costs and
   seconds within 1e-12 relative. */
#include "verdict-spans.h"
#include "verdict-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdbool.h>
#include <stdio.h>

static double worst;
static void near(double got,double want){double e=fabs(got-want)/fmax(fabs(want),1);assert(e<=1e-12);if(e>worst)worst=e;}
static coax_v_f64 fv(double *p,int64_t n){return psionic_vd_scores(p,n);}
static coax_v_s64 iv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}
static void result(coax_t_VdResult r,int64_t verdict,int64_t reason){assert((int64_t)r.verdict==verdict&&(int64_t)r.reason==reason);}
static void decision(coax_t_VdDecision d,const vd_oracle_decision *o){
    assert(d.status==COAX_VdStatus_ok&&d.gain.below==o->below&&d.gain.tasks==o->tasks);
    result(d.result,o->verdict,o->reason);near(d.gain.gain,o->gain);
}

enum {ITEMS=400,RUNS=64,N=4000};
static double scores[RUNS*ITEMS],c[4*ITEMS],a[ITEMS],b[ITEMS];
static int64_t task[ITEMS],part[ITEMS];
static int64_t q[3][N],groups[N],zero[N],rows[N];
static double p[3][N],y[3][N],mapped[N],tp[N],tm[N],ty[N],runs[16*N],cost[N],base[N];
static double ve[2000],vm[2000],vy[2000],ce[N],cm[N],cy[N];

static const int64_t gold[12]={1,0,2,2,1,1,0,1,0,0,1,2};
static const int start[13]={0,3,6,9,13,16,18,20,22,24,27,30,33};
static double elephant[33]={
    0.002928807,0.993571220,0.003499973, 0.999760347,0.000120185,0.000119469,
    0.004286453,0.007238023,0.988475524, 0.000022106,0.000147891,0.999662403,0.000167600,
    0.000654594,0.999191446,0.000153959, 0.000371329,0.999628671, 0.999669108,0.000330892,
    0.000142345,0.999857655, 0.988583238,0.011416762, 0.999912545,0.000062176,0.000025279,
    0.008296165,0.991150862,0.000552973, 0.000428897,0.000749283,0.998821820};

int main(void){
    /* Chapter 179's suite. */
    for(int i=0;i<ITEMS;i++){task[i]=i/10;part[i]=i>=320;}
    for(int r=0;r<RUNS;r++)assert(ns_run(179,r,0,fv(scores+r*ITEMS,ITEMS))==COAX_NsStatus_ok);
    coax_t_NsFloor floors[2];
    for(int w=0;w<2;w++)floors[w]=ns_floor(fv(scores,RUNS*ITEMS),iv(part,ITEMS),w,RUNS,4);
    assert(ns_means(fv(scores,RUNS*ITEMS),ITEMS,0,4,fv(a,ITEMS),false)==COAX_NsStatus_ok);
    const double skill[4]={-0.5,0.5,1.5,2.5};const int64_t first[4]={4000,2000,3000,5000};
    for(int k=0;k<4;k++){
        for(int r=0;r<4;r++)assert(ns_run(179,first[k]+r,skill[k],fv(c+r*ITEMS,ITEMS))==COAX_NsStatus_ok);
        assert(ns_means(fv(c,4*ITEMS),ITEMS,0,4,fv(b,ITEMS),false)==COAX_NsStatus_ok);
        for(int w=0;w<2;w++)
            decision(vd_decision(fv(a,ITEMS),fv(b,ITEMS),iv(task,ITEMS),iv(part,ITEMS),w,floors[w],1,7,1000),&vd_oracle_candidates[k][w]);
    }
    /* Chapter 12: no floor and no headroom. */
    double correct[12];int64_t each[12],none[12]={0};
    for(int i=0;i<12;i++){correct[i]=ns_correct(fv(elephant+start[i],start[i+1]-start[i]),gold[i]);each[i]=i;}
    coax_t_NsFloor unmeasured={0};
    decision(vd_decision(fv(correct,12),fv(correct,12),iv(each,12),iv(none,12),0,unmeasured,1,7,1000),&vd_oracle_elephant);

    /* Chapter 156: model 1's maps on its test split. */
    double v1[4]={2,0,0.6,-0.6};
    for(int k=0;k<3;k++)assert(cf_simulate(156+k,fv(v1,4),iv(q[k],N),fv(p[k],N),fv(y[k],N),false)==COAX_CfStatus_ok);
    coax_t_CfMap maps[2];const coax_t_CfKind kinds[2]={COAX_CfKind_temperature,COAX_CfKind_binned};
    for(int k=0;k<2;k++){
        maps[k]=cf_fit(kinds[k],1,1,k,iv(q[0],N),fv(p[0],N),fv(y[0],N),iv(rows,N));assert(maps[k].status==COAX_CfStatus_ok);
    }
    assert(cf_apply((coax_v_t_CfMap){maps,2},1,1,iv(q[2],N),fv(p[2],N),fv(mapped,N),false)==COAX_CfStatus_ok);
    for(int k=0;k<2;k++){
        int64_t n=0;
        for(int i=0;i<N;i++)if(q[2][i]==k){tp[n]=p[2][i];tm[n]=mapped[i];ty[n]=y[2][i];groups[n]=n/4;n++;}
        coax_t_VdProbability r=vd_probability(fv(tp,n),fv(tm,n),fv(ty,n),iv(groups,n),7,1000);
        const vd_oracle_probability *o=&vd_oracle_calibration[k];
        assert(r.status==COAX_VdStatus_ok);result(r.result,o->verdict,o->reason);
        assert(r.lower.below==o->below[0]&&r.higher.below==o->below[1]&&r.confident.below==o->below[2]);
        for(int j=0;j<2;j++){
            assert(r.errors.e[j]==o->errors[j]&&r.correct.e[j]==o->correct[j]);near(r.loss.e[j],o->loss[j]);near(r.ece.e[j],o->ece[j]);
        }
    }

    /* Chapter 157: the record against the best fixed policy, then time. */
    assert(th_simulate(158,0.9,fv(ce,N),fv(cm,N),fv(cy,N),false)==COAX_ThStatus_ok);
    for(int i=0;i<N;i++)groups[i]=i/4;
    const coax_t_ThCosts costs[2]={{1,3,0.8,2,0.0005,0.15,0.01,1.2,1},{5,1,0.2,0.001,0.0005,0.15,0.01,1.2,300}};
    const coax_t_ThPolicy baseline[2]={COAX_ThPolicy_elephant,COAX_ThPolicy_mammoth};const double limits[2]={0.5,30};
    for(int k=0;k<2;k++){
        const vd_oracle_threshold *o=&vd_oracle_thresholds[k];coax_t_ThReport report={0};
        for(int s=0;s<16;s++){
            assert(th_simulate(s==0?157:1000+s,0.9,fv(ve,2000),fv(vm,2000),fv(vy,2000),false)==COAX_ThStatus_ok);
            coax_t_ThRecord record=th_record(costs[k],th_rho(fv(ve,2000),fv(vm,2000),fv(vy,2000)).value);
            coax_t_ThReport x=th_run(costs[k],COAX_ThPolicy_record,record,fv(ce,N),fv(cm,N),fv(cy,N),fv(cost,N),false);
            assert(x.status==COAX_ThStatus_ok);if(s==0)report=x;
            for(int i=0;i<N;i++)runs[s*N+i]=-cost[i];
        }
        coax_t_NsFloor f=ns_floor(fv(runs,16*N),iv(zero,N),0,16,1);near(f.floor,o->floor);
        coax_t_ThReport x=th_run(costs[k],baseline[k],th_record(costs[k],0.9),fv(ce,N),fv(cm,N),fv(cy,N),fv(cost,N),false);
        near(x.mean,o->base);near(report.mean,o->mean);near(report.seconds,o->seconds);
        for(int i=0;i<N;i++)base[i]=-cost[i];
        coax_t_VdDecision d=vd_decision(fv(base,N),fv(runs,N),iv(groups,N),iv(zero,N),0,f,0,7,1000);
        decision(d,&o->decision);
        result(vd_evaluate(d.result,report.mean,report.seconds,1,limits[k]),o->verdict,o->reason);
    }

    /* Native refusals: NaN and infinity. */
    coax_t_VdResult pass={COAX_VdVerdict_passed,COAX_VdReason_beyond_floor};
    result(vd_evaluate(pass,NAN,0,1,2),COAX_VdVerdict_unverifiable,COAX_VdReason_unmeasured);
    result(vd_evaluate(pass,INFINITY,0,1,2),COAX_VdVerdict_unverifiable,COAX_VdReason_unmeasured);
    result(vd_evaluate(pass,0,0,1,INFINITY),COAX_VdVerdict_unverifiable,COAX_VdReason_unmeasured);
    tp[0]=NAN;assert(vd_probability(fv(tp,40),fv(tm,40),fv(ty,40),iv(groups,40),1,2).status==COAX_VdStatus_number);
    a[0]=INFINITY;assert(vd_decision(fv(a,ITEMS),fv(b,ITEMS),iv(task,ITEMS),iv(part,ITEMS),0,floors[0],1,7,2).status==COAX_VdStatus_number);
    printf("verdict: verdicts, reasons, task counts, replicates at or below each line, confident errors and right answers "
        "equal to the oracle; gains, floors, losses, calibration errors, costs and seconds within %.1e; NaN and infinity "
        "refused\n",worst);
    return 0;
}
