/* Chapter 238 against noise_oracle.py, which drew the same runs and resampled
   the same tasks in exactly rounded sums, and read Elephant's probabilities
   from chapter 16's recorded run. Tolerances were frozen before comparison:
   run sums, task and item counts, replicates at or below a floor, open items
   and refusals exactly; block means, standard deviations, floors, gains,
   standard errors, rooms and Brier scores within 1e-12 relative. */
#include "noise-spans.h"
#include "noise-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>

static double worst;
static void near(double got,double want){double e=fabs(got-want)/fmax(fabs(want),1);assert(e<=1e-12);if(e>worst)worst=e;}
static coax_v_f64 fv(double *p,int64_t n){return (coax_v_f64){p,n};}
static coax_v_s64 iv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}
static void boot(coax_t_NsBoot b,const ns_oracle_boot *o){
    assert(b.status==COAX_NsStatus_ok&&b.tasks==o->tasks&&b.items==o->items&&b.below==o->below);
    near(b.gain,o->gain);near(b.se,o->se);
}
static void room(coax_t_NsRoom r,const ns_oracle_room *o){
    assert(r.status==COAX_NsStatus_ok&&r.items==o->items&&r.open==o->open&&(int64_t)r.refused==o->refused);near(r.room,o->room);
}

enum {ITEMS=400,RUNS=64,BLOCK=4};
static double scores[RUNS*ITEMS],pair[8*ITEMS],a[ITEMS],b[ITEMS],mean[ITEMS];
static int64_t task[ITEMS],part[ITEMS],each[ITEMS];

static const int64_t kind[12]={0,0,0,0,0,1,1,1,1,2,2,2},gold[12]={1,0,2,2,1,1,0,1,0,0,1,2};
static const int start[13]={0,3,6,9,13,16,18,20,22,24,27,30,33};
static double elephant[33]={
    0.002928807,0.993571220,0.003499973, 0.999760347,0.000120185,0.000119469,
    0.004286453,0.007238023,0.988475524, 0.000022106,0.000147891,0.999662403,0.000167600,
    0.000654594,0.999191446,0.000153959, 0.000371329,0.999628671, 0.999669108,0.000330892,
    0.000142345,0.999857655, 0.988583238,0.011416762, 0.999912545,0.000062176,0.000025279,
    0.008296165,0.991150862,0.000552973, 0.000428897,0.000749283,0.998821820};

int main(void){
    for(int i=0;i<ITEMS;i++){task[i]=i/10;part[i]=i>=320;each[i]=i;}
    for(int r=0;r<RUNS;r++){
        assert(ns_run(179,r,0,fv(scores+r*ITEMS,ITEMS))==COAX_NsStatus_ok);
        double s=0;for(int i=0;i<ITEMS;i++)s+=scores[r*ITEMS+i];
        assert(s==ns_oracle_run_sums[r]);
    }
    assert(psionic_ns_means(fv(scores,RUNS*ITEMS),ITEMS,0,RUNS,fv(mean,ITEMS))==COAX_NsStatus_ok);
    double floors[2];
    for(int w=0;w<2;w++){
        coax_t_NsFloor f=ns_floor(fv(scores,RUNS*ITEMS),iv(part,ITEMS),w,RUNS,BLOCK);
        assert(f.status==COAX_NsStatus_ok&&f.blocks==16);
        for(int k=0;k<16;k++)near(f.mean.e[k],ns_oracle_block_mean[w][k]);
        near(f.sd,ns_oracle_floor[w][0]);near(f.floor,ns_oracle_floor[w][1]);floors[w]=f.floor;
        room(ns_headroom(fv(mean,ITEMS),iv(part,ITEMS),w,1,f.floor),&ns_oracle_rooms[w]);
    }
    for(int j=0;j<20;j++){
        for(int q=0;q<8;q++)assert(ns_run(179,RUNS+8*j+q,0,fv(pair+q*ITEMS,ITEMS))==COAX_NsStatus_ok);
        assert(psionic_ns_means(fv(pair,8*ITEMS),ITEMS,0,4,fv(a,ITEMS))==COAX_NsStatus_ok);
        assert(psionic_ns_means(fv(pair,8*ITEMS),ITEMS,4,4,fv(b,ITEMS))==COAX_NsStatus_ok);
        boot(ns_boot(fv(a,ITEMS),fv(b,ITEMS),iv(each,ITEMS),iv(part,ITEMS),0,0,10+j,200),&ns_oracle_pairs[j][0]);
        boot(ns_boot(fv(a,ITEMS),fv(b,ITEMS),iv(task,ITEMS),iv(part,ITEMS),0,0,10+j,200),&ns_oracle_pairs[j][1]);
    }
    assert(psionic_ns_means(fv(scores,RUNS*ITEMS),ITEMS,0,BLOCK,fv(a,ITEMS))==COAX_NsStatus_ok);
    const double skill[2]={0.5,1.5};const int64_t first[2]={2000,3000};
    for(int k=0;k<2;k++){
        for(int q=0;q<4;q++)assert(ns_run(179,first[k]+q,skill[k],fv(pair+q*ITEMS,ITEMS))==COAX_NsStatus_ok);
        assert(psionic_ns_means(fv(pair,4*ITEMS),ITEMS,0,4,fv(b,ITEMS))==COAX_NsStatus_ok);
        int64_t *cluster[2]={each,task};
        for(int c=0;c<2;c++)for(int x=0;x<2;x++)
            boot(ns_boot(fv(a,ITEMS),fv(b,ITEMS),iv(cluster[c],ITEMS),iv(part,ITEMS),0,x?floors[0]:0,7,1000),&ns_oracle_candidates[k][2*c+x]);
    }
    double correct[12],brier[12];int64_t kinds[12],same[12]={0};
    for(int i=0;i<12;i++){
        kinds[i]=kind[i];coax_v_f64 p=fv(elephant+start[i],start[i+1]-start[i]);
        correct[i]=ns_correct(p,gold[i]);brier[i]=-ns_brier(p,gold[i]);
        assert(correct[i]==1);near(brier[i],ns_oracle_elephant_brier[i]);
    }
    for(int w=0;w<3;w++){
        room(ns_headroom(fv(correct,12),iv(kinds,12),w,1,0),&ns_oracle_elephant_rooms[0][w]);
        room(ns_headroom(fv(brier,12),iv(kinds,12),w,0,0),&ns_oracle_elephant_rooms[1][w]);
    }
    room(ns_headroom(fv(correct,12),iv(same,12),0,1,0),&ns_oracle_elephant_rooms[0][3]);
    room(ns_headroom(fv(brier,12),iv(same,12),0,0,0),&ns_oracle_elephant_rooms[1][3]);

    /* Native refusals: overlap leaves the output unchanged; NaN and infinity. */
    mean[1]=7;
    assert(psionic_ns_means(fv(mean,4),2,0,2,fv(mean+1,2))==COAX_NsStatus_alias&&mean[1]==7);
    assert(psionic_ns_means(fv(scores,8),2,0,4,fv(scores+8,2))==COAX_NsStatus_ok);
    a[0]=NAN;assert(psionic_ns_means(fv(a,ITEMS),ITEMS,0,1,fv(b,ITEMS))==COAX_NsStatus_number);
    a[0]=INFINITY;assert(ns_boot(fv(a,ITEMS),fv(b,ITEMS),iv(task,ITEMS),iv(part,ITEMS),0,0,1,2).status==COAX_NsStatus_number);
    a[0]=0;assert(ns_boot(fv(a,ITEMS),fv(b,ITEMS),iv(task,ITEMS),iv(part,ITEMS),0,NAN,1,2).status==COAX_NsStatus_number);
    assert(ns_headroom(fv(a,ITEMS),iv(part,ITEMS),0,INFINITY,0).status==COAX_NsStatus_number);
    assert(ns_run(1,0,NAN,fv(b,ITEMS))==COAX_NsStatus_number);
    elephant[0]=NAN;assert(ns_brier(fv(elephant,3),1)==-1&&ns_correct(fv(elephant,3),1)==-1);
    printf("noise: run sums, task and item counts, replicates at or below each floor, open items and refusals "
        "equal to the oracle; means, floors, gains, errors, rooms and Brier scores within %.1e; overlap, NaN and "
        "infinity refused\n",worst);
    return 0;
}
