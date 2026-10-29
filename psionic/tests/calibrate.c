/* Chapter 156 against calibrate_oracle.py, which found the temperature by a
   different search (over the bit patterns of doubles), pooled isotonic steps
   with exact fractions and measured in exactly rounded sums. Tolerances were
   frozen before comparison: chosen kinds, gate verdicts, knot counts, step
   values, confident errors and reliability counts exactly; slopes, errors,
   Brier, log loss and reliability means and rates within 1e-12 relative. */
#include "calibrate-spans.h"
#include "calibrate-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>

#define N 4000
static coax_v_f64 fv(double *p,int64_t n){return (coax_v_f64){p,n};}
static coax_v_s64 iv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}
static double worst;
static void near(double got,double want){double e=fabs(got-want)/fmax(fabs(want),1);assert(e<=1e-12);if(e>worst)worst=e;}
static void same(coax_t_CfReport r,const cf_oracle_report *o){
    assert(r.status==COAX_CfStatus_ok);near(r.ece,o->ece);near(r.brier,o->brier);near(r.log_loss,o->loss);
    assert(r.confident_errors==o->errors);
}

static int64_t q[3][N],rows[N],own[N],count[10];
static double p[3][N],y[3][N],out[N],mean[10],rate[10],both[15];
static coax_t_CfMap maps[2];

/* Rows of question k, in row order. */
static int64_t rows_of(const int64_t *qs,int64_t k){int64_t n=0;for(int64_t i=0;i<N;i++)if(qs[i]==k)own[n++]=i;return n;}
static coax_t_CfMap none(uint64_t model,int64_t k){coax_t_CfMap m={0};m.kind=COAX_CfKind_none;m.model=model;m.questions=1;m.question=k;return m;}

static void model(uint64_t id,uint64_t seed,double *params,const cf_oracle_question *oracle){
    for(int s=0;s<3;s++)assert(psionic_cf_simulate(seed+s,fv(params,4),iv(q[s],N),fv(p[s],N),fv(y[s],N))==COAX_CfStatus_ok);
    coax_t_CfMap chosen[2];
    for(int64_t k=0;k<2;k++){
        const cf_oracle_question *o=&oracle[k];
        int64_t n=rows_of(q[1],k);coax_t_CfReport raw=cf_measure(fv(p[1],N),fv(y[1],N),iv(own,n));same(raw,&o->raw_gate);
        chosen[k]=none(id,k);double best=raw.log_loss;
        for(int kind=1;kind<4;kind++){
            coax_t_CfMap m=cf_fit((coax_t_CfKind)kind,id,1,k,iv(q[0],N),fv(p[0],N),fv(y[0],N),iv(rows,N));
            const cf_oracle_map *om=&o->map[kind-1];
            assert(m.status==COAX_CfStatus_ok&&m.question==k&&m.knots==om->knots);near(m.slope,om->slope);
            if(m.knots>0)assert(m.value.e[0]==om->first&&m.value.e[m.knots-1]==om->last);
            /* The candidate in its own slot, the raw probabilities in the other. */
            maps[k]=m;maps[1-k]=none(id,1-k);
            assert(psionic_cf_apply((coax_v_t_CfMap){maps,2},id,1,iv(q[1],N),fv(p[1],N),fv(out,N))==COAX_CfStatus_ok);
            coax_t_CfReport g=cf_measure(fv(out,N),fv(y[1],N),iv(own,n));same(g,&o->gate[kind-1]);
            int pass=cf_gate(raw,g);assert(pass==o->passes[kind-1]);
            if(pass&&g.log_loss<best){chosen[k]=m;best=g.log_loss;}
        }
        assert((int64_t)chosen[k].kind==o->chosen);
    }
    assert(psionic_cf_apply((coax_v_t_CfMap){chosen,2},id,1,iv(q[2],N),fv(p[2],N),fv(out,N))==COAX_CfStatus_ok);
    for(int64_t k=0;k<2;k++){
        const cf_oracle_question *o=&oracle[k];int64_t n=rows_of(q[2],k);
        same(cf_measure(fv(p[2],N),fv(y[2],N),iv(own,n)),&o->raw);same(cf_measure(fv(out,N),fv(y[2],N),iv(own,n)),&o->mapped);
        for(int pass=0;pass<2;pass++){
            assert(psionic_cf_reliability(fv(pass?out:p[2],N),fv(y[2],N),iv(own,n),iv(count,10),fv(mean,10),fv(rate,10))==COAX_CfStatus_ok);
            for(int b=0;b<10;b++){assert(count[b]==o->count[pass][b]);near(mean[b],o->mean[pass][b]);near(rate[b],o->rate[pass][b]);}
        }
    }
    /* The other model's identity, or another question set, is refused. */
    assert(psionic_cf_apply((coax_v_t_CfMap){chosen,2},id+1,1,iv(q[2],N),fv(p[2],N),fv(out,N))==COAX_CfStatus_stale);
    assert(psionic_cf_apply((coax_v_t_CfMap){chosen,2},id,2,iv(q[2],N),fv(p[2],N),fv(out,N))==COAX_CfStatus_stale);
}

int main(void){
    double v1[4]={2,0,0.6,-0.6},v2[4]={1,0.9,1.5,0};
    model(1,156,v1,cf_oracle[0]);
    model(2,256,v2,cf_oracle[1]);

    /* Native refusals: a kind past isotonic, NaN and infinity, and overlapping storage. */
    assert(cf_fit((coax_t_CfKind)4,1,1,0,iv(q[0],N),fv(p[0],N),fv(y[0],N),iv(rows,N)).status==COAX_CfStatus_shape);
    double keep=p[0][7];p[0][7]=NAN;
    assert(cf_fit(COAX_CfKind_binned,1,1,0,iv(q[0],N),fv(p[0],N),fv(y[0],N),iv(rows,N)).status==COAX_CfStatus_number);
    p[0][7]=INFINITY;
    assert(cf_fit(COAX_CfKind_binned,1,1,0,iv(q[0],N),fv(p[0],N),fv(y[0],N),iv(rows,N)).status==COAX_CfStatus_number);
    maps[0]=none(1,0);maps[1]=none(1,1);
    assert(psionic_cf_apply((coax_v_t_CfMap){maps,2},1,1,iv(q[0],N),fv(p[0],N),fv(out,N))==COAX_CfStatus_number);
    p[0][7]=keep;y[0][7]=NAN;own[0]=7;
    assert(cf_measure(fv(p[0],N),fv(y[0],N),iv(own,1)).status==COAX_CfStatus_number);
    y[0][7]=1;
    double bad[4]={2,NAN,0.6,-0.6};
    assert(psionic_cf_simulate(1,fv(bad,4),iv(q[0],2),fv(p[0],2),fv(y[0],2))==COAX_CfStatus_number);
    assert(psionic_cf_apply((coax_v_t_CfMap){maps,2},1,1,iv(q[0],N-1),fv(p[0],N-1),fv(p[0]+1,N-1))==COAX_CfStatus_alias);
    assert(psionic_cf_simulate(1,fv(v1,4),iv(q[0],2),fv(p[0],2),fv(p[0]+1,2))==COAX_CfStatus_alias);
    assert(psionic_cf_reliability(fv(p[0],N),fv(y[0],N),iv(own,1),iv(count,10),fv(both,10),fv(both+5,10))==COAX_CfStatus_alias);
    printf("calibrate: both models' chosen kinds, gate verdicts, knots and tallies equal to the oracle; "
        "slopes and measures within %.1e; stale maps, NaN, infinity and overlap refused\n",worst);
    return 0;
}
