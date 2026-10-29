/* Chapter 157 against threshold_oracle.py, which derived the records by a
   different search (over the bit patterns of doubles) and ran the experiment
   in exactly rounded sums. Tolerances were frozen before comparison: rho,
   band counts, edges, actions, decisions and every tally exactly; means,
   intervals and seconds within 1e-12 relative. */
#include "threshold-spans.h"
#include "threshold-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>

static coax_v_f64 fv(double *p,int64_t n){return (coax_v_f64){p,n};}
static double worst;
static void near(double got,double want){double e=fabs(got-want)/fmax(fabs(want),1);assert(e<=1e-12);if(e>worst)worst=e;}

static const coax_t_ThCosts voice={1,3,0.8,2,0.0005,0.15,0.01,1.2,1};
static const coax_t_ThCosts background={5,1,0.2,0.001,0.0005,0.15,0.01,1.2,300};
static double e[2000],m[2000],y[2000],ce[4000],cm[4000],cy[4000],cost[4][4000],scratch[4000];

static void consumer(coax_t_ThCosts c,coax_t_ThRecord r,int64_t bands,const double *edges,const int64_t *actions,
        const double (*means)[3],const int64_t (*tallies)[3],int64_t best,const double *paired){
    assert(r.status==COAX_ThStatus_ok&&r.version==1&&r.bands==bands);
    for(int b=0;b<bands;b++)assert(r.edge.e[b]==edges[b]&&(int64_t)r.action.e[b]==actions[b]);
    for(int p=0;p<4;p++){
        coax_t_ThReport x=psionic_th_run(c,(coax_t_ThPolicy)p,r,fv(ce,4000),fv(cm,4000),fv(cy,4000),fv(cost[p],4000));
        assert(x.status==COAX_ThStatus_ok);
        near(x.mean,means[p][0]);near(x.half,means[p][1]);near(x.seconds,means[p][2]);
        assert(x.errors==tallies[p][0]&&x.asked==tallies[p][1]&&x.escalated==tallies[p][2]);
    }
    coax_t_ThValue d=psionic_th_paired(fv(cost[3],4000),fv(cost[best],4000),fv(scratch,4000));
    assert(d.status==COAX_ThStatus_ok);near(d.value,paired[0]);near(d.half,paired[1]);
}

int main(void){
    assert(psionic_th_simulate(157,0.9,fv(e,2000),fv(m,2000),fv(y,2000))==COAX_ThStatus_ok);
    coax_t_ThValue rho=th_rho(fv(e,2000),fv(m,2000),fv(y,2000));
    assert(rho.status==COAX_ThStatus_ok&&rho.value==th_oracle_rho);
    assert(psionic_th_simulate(158,0.9,fv(ce,4000),fv(cm,4000),fv(cy,4000))==COAX_ThStatus_ok);
    coax_t_ThRecord rv=th_record(voice,rho.value),rb=th_record(background,rho.value);
    int64_t differ=0,agree=0;
    for(int i=0;i<4000;i++){
        coax_t_ThAction a=th_decide(rv,ce[i]),b=th_decide(rb,ce[i]);differ+=a!=b;
        agree+=a==th_best(voice,ce[i],rho.value).action;agree+=b==th_best(background,ce[i],rho.value).action;
    }
    assert(differ==th_oracle_counts[0]&&agree==th_oracle_counts[1]);
    consumer(voice,rv,th_oracle_bands_0,th_oracle_edges_0,th_oracle_actions_0,th_oracle_means_0,th_oracle_tallies_0,th_oracle_best_0,th_oracle_paired_0);
    consumer(background,rb,th_oracle_bands_1,th_oracle_edges_1,th_oracle_actions_1,th_oracle_means_1,th_oracle_tallies_1,th_oracle_best_1,th_oracle_paired_1);

    /* Native refusals: NaN and infinity, and overlapping storage. */
    double p2[2]={NAN,0.5},q2[2]={0.5,0.5},l2[2]={0,1},c2[2];
    assert(psionic_th_run(voice,COAX_ThPolicy_elephant,rv,fv(p2,2),fv(q2,2),fv(l2,2),fv(c2,2)).status==COAX_ThStatus_number);
    p2[0]=INFINITY;assert(psionic_th_run(voice,COAX_ThPolicy_elephant,rv,fv(p2,2),fv(q2,2),fv(l2,2),fv(c2,2)).status==COAX_ThStatus_number);
    coax_t_ThCosts nan=voice;nan.ask=NAN;assert(th_record(nan,0.9).status==COAX_ThStatus_shape);
    assert(th_record(voice,NAN).status==COAX_ThStatus_shape);
    l2[1]=NAN;assert(th_rho(fv(q2,2),fv(q2,2),fv(l2,2)).status==COAX_ThStatus_number);
    p2[0]=0.5;l2[1]=1;
    assert(psionic_th_run(voice,COAX_ThPolicy_elephant,rv,fv(p2,2),fv(q2,2),fv(l2,2),fv(q2,2)).status==COAX_ThStatus_alias);
    assert(psionic_th_simulate(1,0.9,fv(e,2),fv(e+1,2),fv(y,2))==COAX_ThStatus_alias);
    assert(psionic_th_paired(fv(p2,2),fv(q2,2),fv(p2+1,2)).status==COAX_ThStatus_alias);
    c2[0]=INFINITY;c2[1]=0;assert(psionic_th_paired(fv(c2,2),fv(q2,2),fv(scratch,2)).status==COAX_ThStatus_number);
    printf("threshold: rho, both records' edges and actions, every decision and tally equal to the oracle; "
        "means and intervals within %.1e; NaN, infinity and overlap refused\n",worst);
    return 0;
}
