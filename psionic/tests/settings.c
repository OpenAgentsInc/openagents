/* Chapter 241 against settings_oracle.py, which wrote the simulator, the rule,
   the candidates and the search again, and took its floors and gate from
   chapters 238 and 239's independent oracles. Tolerances were frozen before
   comparison: rows, settings, counts, costs, outcomes, verdicts and
   replicate counts exactly; floors and gains within 1e-12 relative. */
#include "settings-spans.h"
#include "settings-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

static double worst;
static void near(double got,double want){double e=fabs(got-want)/fmax(fabs(want),1);assert(e<=1e-12);if(e>worst)worst=e;}
static void score_is(coax_t_StScore s,const st_oracle_score *o){
    assert(s.rows==o->rows&&s.stalls==o->stalls&&s.flagged==o->flagged&&s.caught==o->caught);
    assert(s.missed==o->missed&&s.alarms==o->alarms&&s.cost==o->cost);
}
static void settings_is(coax_t_StSettings s,const double o[3]){assert(s.weight==o[0]&&s.threshold==o[1]&&s.quiet==o[2]);}

enum {FIT=400,HELD=800,STEPS=24};
static coax_t_StRow rows[FIT],other[FIT],held[HELD];
static coax_t_StStep trail[STEPS];
static coax_t_StSettings fits[2][16];
static double runs[2][16*HELD],base[HELD],means[2][16];
static int64_t groups[HELD],zero[HELD],sets[16],none[16];

static void values(coax_t_StSettings s,double *out){
    coax_t_StScore one;
    for(int i=0;i<HELD;i++){
        one=st_score(psionic_st_rows(&held[i],1),s,(coax_t_StCosts){4,1});
        out[i]=-one.cost;
    }
}

int main(void){
    const coax_t_StSettings start={0,0.5,700};const coax_t_StCosts costs={4,1};
    assert(st_simulate(182,40,psionic_st_rows(rows,FIT))==COAX_StStatus_ok);
    for(int i=0;i<FIT;i++){
        const st_oracle_row *o=&st_oracle_rows[i];
        assert(rows[i].group==o->group&&rows[i].p==o->p&&rows[i].repeats==o->repeats&&rows[i].quiet==o->quiet&&rows[i].stalled==(o->stalled!=0));
    }
    score_is(st_score(psionic_st_rows(rows,FIT),start,costs),&st_oracle_start);

    const st_oracle_step *logs[2]={st_oracle_cv_log,st_oracle_fit_only_log};
    const int64_t lens[2]={sizeof st_oracle_cv_log/sizeof st_oracle_cv_log[0],sizeof st_oracle_fit_only_log/sizeof st_oracle_fit_only_log[0]};
    const double (*all[2])[3]={st_oracle_cv_settings,st_oracle_fit_only_settings};
    const st_oracle_score *scores[2]={&st_oracle_cv_score,&st_oracle_fit_only_score};
    for(int k=0;k<2;k++){
        for(int s=0;s<16;s++){
            coax_t_StRow *set=rows;
            if(s>0){assert(st_simulate(1000+s,40,psionic_st_rows(other,FIT))==COAX_StStatus_ok);set=other;}
            coax_t_StFit f=st_fit(psionic_st_rows(set,FIT),start,costs,5,k==0,8,psionic_st_trail(trail,STEPS));
            assert(f.status==COAX_StStatus_ok);settings_is(f.settings,all[k][s]);fits[k][s]=f.settings;
            if(s>0)continue;
            assert(f.steps==lens[k]);
            for(int i=0;i<f.steps;i++){
                const st_oracle_step *o=&logs[k][i];coax_t_StStep t=trail[i];
                assert(t.pass==o->pass&&t.which==o->which&&(int64_t)t.outcome==o->outcome);
                assert(t.from==o->from&&t.to==o->to&&t.before==o->before&&t.after==o->after&&t.held==o->held);
            }
            score_is(st_score(psionic_st_rows(rows,FIT),f.settings,costs),scores[k]);
        }
    }

    /* Every setting is written down; now the held-out runs are read. */
    assert(st_simulate(9182,80,psionic_st_rows(held,HELD))==COAX_StStatus_ok);
    score_is(st_score(psionic_st_rows(held,HELD),start,costs),&st_oracle_held_start);
    score_is(st_score(psionic_st_rows(held,HELD),fits[0][0],costs),&st_oracle_cv_held);
    score_is(st_score(psionic_st_rows(held,HELD),fits[1][0],costs),&st_oracle_fit_only_held);
    const double *held_costs[2]={st_oracle_cv_held_costs,st_oracle_fit_only_held_costs};
    const double floor_want[2]={st_oracle_cv_floor,st_oracle_fit_only_floor};
    coax_t_NsFloor floors[2];
    for(int i=0;i<HELD;i++)groups[i]=held[i].group;
    for(int k=0;k<2;k++){
        for(int s=0;s<16;s++){
            coax_t_StScore c=st_score(psionic_st_rows(held,HELD),fits[k][s],costs);
            assert(c.cost==held_costs[k][s]);means[k][s]=-c.cost/HELD;
            values(fits[k][s],&runs[k][s*HELD]);
        }
        floors[k]=ns_floor((coax_v_f64){runs[k],16*HELD},(coax_v_s64){zero,HELD},0,16,1);
        assert(floors[k].status==COAX_NsStatus_ok);near(floors[k].floor,floor_want[k]);
    }
    values(start,base);
    coax_t_VdDecision d=vd_decision((coax_v_f64){base,HELD},(coax_v_f64){runs[0],HELD},(coax_v_s64){groups,HELD},
        (coax_v_s64){zero,HELD},0,floors[0],0,7,1000);
    assert(d.status==COAX_VdStatus_ok&&(int64_t)d.result.verdict==st_oracle_gate_fit.verdict&&(int64_t)d.result.reason==st_oracle_gate_fit.reason);
    assert(d.gain.below==st_oracle_gate_fit.below&&d.gain.tasks==st_oracle_gate_fit.tasks);near(d.gain.gain,st_oracle_gate_fit.gain);
    for(int s=0;s<16;s++)sets[s]=s;
    coax_t_NsBoot b=ns_boot((coax_v_f64){means[0],16},(coax_v_f64){means[1],16},(coax_v_s64){sets,16},(coax_v_s64){none,16},0,0,7,1000);
    assert(b.status==COAX_NsStatus_ok&&b.below==st_oracle_boot_below);near(b.gain,st_oracle_boot_gain);

    /* Native refusals: NaN and infinity in recorded rows, settings and costs. */
    coax_t_StRow bad[FIT];memcpy(bad,rows,sizeof bad);bad[7].p=NAN;
    assert(st_fit(psionic_st_rows(bad,FIT),start,costs,5,true,8,psionic_st_trail(trail,STEPS)).status==COAX_StStatus_number);
    bad[7]=rows[7];bad[9].quiet=INFINITY;
    assert(st_fit(psionic_st_rows(bad,FIT),start,costs,5,true,8,psionic_st_trail(trail,STEPS)).status==COAX_StStatus_number);
    coax_t_StSettings s=start;s.threshold=NAN;
    assert(st_fit(psionic_st_rows(rows,FIT),s,costs,5,true,8,psionic_st_trail(trail,STEPS)).status==COAX_StStatus_number);
    s=start;s.weight=INFINITY;
    assert(st_fit(psionic_st_rows(rows,FIT),s,costs,5,true,8,psionic_st_trail(trail,STEPS)).status==COAX_StStatus_number);
    assert(st_fit(psionic_st_rows(rows,FIT),start,(coax_t_StCosts){INFINITY,1},5,true,8,psionic_st_trail(trail,STEPS)).status==COAX_StStatus_number);
    assert(st_fit(psionic_st_rows(rows,FIT),start,(coax_t_StCosts){4,NAN},5,true,8,psionic_st_trail(trail,STEPS)).status==COAX_StStatus_number);
    printf("settings: every recorded row, fit, step, score, floor and verdict equal to the oracle "
        "(largest floor or gain difference %.3g); NaN and infinity refused\n",worst);
    return 0;
}
