/* Chapter 215 against drift_oracle.py, which fitted maps with chapter 213's
   oracle, computed the run in closed form instead of stepping time, and
   measured in exactly rounded sums. Tolerances were frozen before comparison:
   kinds, adoptions, versions, counts, actions, draws reaching each window and
   alerts exactly; calibration errors, log losses, sums of probabilities and
   costs, estimates and true costs within 1e-12 relative. */
#include "drift-spans.h"
#include "drift-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>

static double worst;
static void near(double got,double want){double e=fabs(got-want)/fmax(fabs(want),1);assert(e<=1e-12);if(e>worst)worst=e;}
static void gate(coax_t_DrGate g,const dr_oracle_gate *o){
    assert((int64_t)g.adopted==o->adopted&&(int64_t)g.kind==o->kind);
    for(int k=0;k<4;k++){near(g.ece.e[k],o->ece[k]);near(g.loss.e[k],o->loss[k]);}
}

static coax_t_DrRecord calibration[3000],entries[12000];
static const coax_t_DrPolicy policy={1,0.25,0.1,0.1,1,3};

int main(void){
    assert(dr_calibration(158,psionic_dr_log(calibration,3000))==COAX_DrStatus_ok);
    coax_t_DrRefit first=dr_first(psionic_dr_log(calibration,3000),1000,1,1);
    gate(first.gate,&dr_oracle_first);
    coax_t_DrRun run=dr_run(158,policy,first.map,1000,4000,1.5,psionic_dr_log(entries,12000));
    assert(run.status==COAX_DrStatus_ok&&run.windows==12&&run.refits==dr_oracle_refits);
    for(int k=0;k<run.refits;k++)gate(run.refit.e[k],&dr_oracle_refit[k]);
    for(int w=0;w<12;w++)assert(run.version.e[w]==dr_oracle_versions[w]);
    assert(run.lost==dr_oracle_counts[0]&&run.joined==dr_oracle_counts[1]&&run.explored==dr_oracle_counts[2]);
    int64_t actions=0,eligible=0,joined_at=0;double calibrated=0,propensity=0,cost=0;
    for(int i=0;i<12000;i++){
        actions+=entries[i].action;eligible+=entries[i].eligible;joined_at+=entries[i].joined_at;
        calibrated+=entries[i].calibrated;propensity+=entries[i].propensity;cost+=entries[i].cost;
    }
    assert(actions==dr_oracle_counts[3]&&eligible==dr_oracle_counts[4]&&joined_at==dr_oracle_counts[5]);
    /* Plain sums of 12,000 values: within 1e-12 of the exactly rounded ones. */
    near(calibrated,dr_oracle_sums[0]);near(propensity,dr_oracle_sums[1]);near(cost,dr_oracle_sums[2]);

    coax_t_DrWatch watch=dr_watch(psionic_dr_log(entries,12000),1000,160);
    assert(watch.status==COAX_DrStatus_ok&&watch.windows==12);
    for(int w=0;w<12;w++){
        assert(watch.joined.e[w]==dr_oracle_joined[w]&&watch.exceed.e[w]==dr_oracle_exceed[w]);
        assert(watch.alert.e[w]==(dr_oracle_exceed[w]<2));near(watch.ece.e[w],dr_oracle_ece[w]);
    }
    const double thresholds[3]={0.25,0.5,0.05};
    for(int k=0;k<3;k++){
        coax_t_DrValue v=dr_ips(psionic_dr_log(entries,12000),thresholds[k]);
        assert((int64_t)v.status==dr_oracle_value_status[k]&&v.used==dr_oracle_used[k]);
        if(v.status==COAX_DrStatus_ok){near(v.value,dr_oracle_value[k][0]);near(v.half,dr_oracle_value[k][1]);}
        near(dr_truth(158,4000,1.5,policy,psionic_dr_log(entries,12000),thresholds[k]),dr_oracle_value[k][2]);
    }

    /* Native refusals: NaN and infinity in labels, shifts, thresholds and policies. */
    coax_t_DrRecord small[2]={{0},{0}};
    assert(dr_join(psionic_dr_log(small,2),2,0,NAN,1,policy)==COAX_DrStatus_number);
    assert(dr_join(psionic_dr_log(small,2),2,0,INFINITY,1,policy)==COAX_DrStatus_number);
    assert(small[0].status==COAX_DrOutcome_pending);
    assert(dr_run(1,policy,first.map,1000,0,NAN,psionic_dr_log(entries,12000)).status==COAX_DrStatus_number);
    coax_t_DrPolicy bad=policy;bad.explore=NAN;
    assert(dr_run(1,bad,first.map,1000,0,0,psionic_dr_log(entries,12000)).status==COAX_DrStatus_number);
    bad=policy;bad.wrong_no=INFINITY;
    assert(dr_run(1,bad,first.map,1000,0,0,psionic_dr_log(entries,12000)).status==COAX_DrStatus_number);
    assert(dr_ips(psionic_dr_log(entries,12000),NAN).status==COAX_DrStatus_number);
    printf("drift: maps, refits, versions, every count, draw count and alert equal to the oracle; "
        "errors, losses and estimates within %.1e; NaN and infinity refused\n",worst);
    return 0;
}
