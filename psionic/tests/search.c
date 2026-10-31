/* Chapter 242 against search_oracle.py, which wrote the manifests, the
   operators, the world, the tiers, successive halving and the lone-best rule
   again, and took its floor and gate from chapters 238 and 239's independent
   oracles. Tolerances were frozen before comparison: policies, digests,
   tasks, rounds, spends, winners, counts and verdicts exactly; means, the
   floor and the gain within 1e-12 relative. */
#include "search-spans.h"
#include "search-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

static double worst;
static void near(double got,double want){double e=fabs(got-want)/fmax(fabs(want),1);assert(e<=1e-12);if(e>worst)worst=e;}
static double sum(const double *v,int n){double t=0;for(int i=0;i<n;i++)t+=v[i];return t;}

enum {POLICIES=32,TASKS=60,TRIALS=4};
static coax_t_SrEntry entries[POLICIES];
static coax_t_SrTask dev[TASKS],held[TASKS];
static coax_t_SrCell full[POLICIES][TASKS],cells[TASKS];
static double base[TASKS],runs[16*TASKS];
static int64_t groups[TASKS],zero[TASKS];

int main(void){
    coax_t_SrExpand e=sr_expand(psionic_sr_entries(entries,POLICIES));
    assert(e.status==COAX_SrStatus_ok&&e.count==POLICIES);
    for(int i=0;i<POLICIES;i++){
        const sr_oracle_entry *o=&sr_oracle_entries[i];coax_t_SrEntry x=entries[i];
        assert(x.policy.model==o->model&&x.policy.retries==o->retries&&x.policy.context==o->context&&x.policy.verify==(o->verify!=0));
        assert(x.parent==o->parent&&(int64_t)x.op==o->op&&memcmp(x.digest.e,o->digest,32)==0);
    }
    assert(sr_tasks(183,psionic_sr_tasks(dev,TASKS))==COAX_SrStatus_ok);
    for(int i=0;i<TASKS;i++){
        assert(dev[i].difficulty==sr_oracle_dev[i][0]&&dev[i].needs_context==(sr_oracle_dev[i][1]!=0)&&dev[i].checkable==(sr_oracle_dev[i][2]!=0));
    }
    coax_t_SrTier tiers[3]={{12,1,301},{30,2,302},{60,4,303}};
    int64_t keeps[2]={16,8};coax_t_SrRound rounds[3];
    coax_t_SrSearch s=sr_search(psionic_sr_entries(entries,POLICIES),psionic_sr_tasks(dev,TASKS),psionic_sr_tiers(tiers,3),
        (coax_v_s64){keeps,2},psionic_sr_rounds(rounds,3));
    assert(s.status==COAX_SrStatus_ok&&s.rounds==3&&s.winner==sr_oracle_winner&&s.spend==sr_oracle_spend);
    for(int k=0;k<3;k++){
        const sr_oracle_round *o=&sr_oracle_rounds[k];
        assert(rounds[k].measured==o->measured&&rounds[k].best==o->best&&rounds[k].kept==o->kept&&rounds[k].frontier==o->frontier);
        assert(rounds[k].spend==o->spend);
    }

    /* For comparison only: every policy measured in full. */
    double spend=0,total[POLICIES];int best=0;
    for(int i=0;i<POLICIES;i++){
        coax_t_SrMeasure m=sr_measure(303,entries[i].policy,psionic_sr_tasks(dev,TASKS),TRIALS,psionic_sr_cells(full[i],TASKS));
        assert(m.status==COAX_SrStatus_ok);spend+=m.spend;total[i]=m.score;
        if(total[i]>total[best])best=i;
    }
    assert(spend==sr_oracle_full_spend&&best==sr_oracle_full_best);
    int off=0;double top=0;
    for(int i=0;i<TASKS;i++){
        double b=full[0][i].score;
        for(int p=1;p<POLICIES;p++)if(full[p][i].score>b)b=full[p][i].score;
        off+=b>full[s.winner][i].score;top+=b;
    }
    assert(off==sr_oracle_off);near(top/TASKS,sr_oracle_oracle);near(total[s.winner]/TASKS,sr_oracle_winner_total);

    /* The winner is written down; now the held-out tasks are read. */
    assert(sr_tasks(9183,psionic_sr_tasks(held,TASKS))==COAX_SrStatus_ok);
    const coax_t_SrPolicy simplest={0};
    assert(sr_measure(501,simplest,psionic_sr_tasks(held,TASKS),TRIALS,psionic_sr_cells(cells,TASKS)).status==COAX_SrStatus_ok);
    for(int i=0;i<TASKS;i++)base[i]=cells[i].score;
    for(int r=0;r<16;r++){
        assert(sr_measure(501+r,entries[s.winner].policy,psionic_sr_tasks(held,TASKS),TRIALS,psionic_sr_cells(cells,TASKS)).status==COAX_SrStatus_ok);
        for(int i=0;i<TASKS;i++)runs[r*TASKS+i]=cells[i].score;
    }
    near(sum(base,TASKS)/TASKS,sr_oracle_base);near(sum(runs,TASKS)/TASKS,sr_oracle_held_winner);
    for(int i=0;i<TASKS;i++)groups[i]=i;
    coax_t_NsFloor f=ns_floor((coax_v_f64){runs,16*TASKS},(coax_v_s64){zero,TASKS},0,16,1);
    assert(f.status==COAX_NsStatus_ok);near(f.floor,sr_oracle_floor);
    coax_t_VdDecision d=vd_decision((coax_v_f64){base,TASKS},(coax_v_f64){runs,TASKS},(coax_v_s64){groups,TASKS},
        (coax_v_s64){zero,TASKS},0,f,1,7,1000);
    assert(d.status==COAX_VdStatus_ok&&(int64_t)d.result.verdict==sr_oracle_gate[0]&&(int64_t)d.result.reason==sr_oracle_gate[1]);
    assert(d.gain.below==sr_oracle_gate[2]&&d.gain.tasks==sr_oracle_gate[3]);near(d.gain.gain,sr_oracle_gate_gain);

    /* Native refusals: NaN and infinity in a task's difficulty. */
    coax_t_SrTask bad[TASKS];memcpy(bad,dev,sizeof bad);
    bad[11].difficulty=NAN;
    assert(sr_measure(303,simplest,psionic_sr_tasks(bad,TASKS),1,psionic_sr_cells(cells,TASKS)).status==COAX_SrStatus_number);
    assert(sr_search(psionic_sr_entries(entries,POLICIES),psionic_sr_tasks(bad,TASKS),psionic_sr_tiers(tiers,3),
        (coax_v_s64){keeps,2},psionic_sr_rounds(rounds,3)).status==COAX_SrStatus_number);
    bad[11].difficulty=INFINITY;
    assert(sr_measure(303,simplest,psionic_sr_tasks(bad,TASKS),1,psionic_sr_cells(cells,TASKS)).status==COAX_SrStatus_number);
    bad[11].difficulty=-INFINITY;
    assert(sr_measure(303,simplest,psionic_sr_tasks(bad,TASKS),1,psionic_sr_cells(cells,TASKS)).status==COAX_SrStatus_number);
    printf("search: every policy, digest, task, round, spend and verdict equal to the oracle "
        "(largest mean, floor or gain difference %.3g); NaN and infinity refused\n",worst);
    return 0;
}
