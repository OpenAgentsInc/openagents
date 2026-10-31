/* Chapter 103 against the independent oracle in tabular_oracle.py. Tolerances
   were frozen before comparison: draws, labels, splits, thresholds and counts
   exact; latencies 1e-12 relative; parameters, losses and errors 1e-9 relative. */
#include "tabular-spans.h"
#include "tabular-oracle.h"
#include <assert.h>
#include <stdbool.h>
#include <math.h>
#include <stdio.h>

#define N TB_ORACLE_ROWS
static coax_v_f64 fv(double *p,int64_t n){return (coax_v_f64){p,n};}
static coax_v_s64 sv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}
static double worst;
static void near(double got,double want,double rel){
    double e=fabs(got-want)/fmax(fabs(want),1e-300);
    assert(e<=rel);if(rel>1e-12&&e>worst)worst=e;
}
static double sigmoid(double z){return z>=0?1/(1+exp(-z)):exp(z)/(1+exp(z));}

typedef struct {
    double x[4*N],y[N],lat[N],xs[4*N],z[N],p[N],tmp[N];int64_t g[N],tr[N],va[N],te[N],order[N];
    int64_t ntr,nva,nte;double mean[4],sd[4];
} World;
static World w;

static void train(coax_t_TbModel m,coax_t_TbConfig c,const double *x,const double *y,const int64_t *rows,
        int64_t n,double *params){
    coax_t_TbTrain t=psionic_tb_train(m,c,fv((double*)x,4*N),fv((double*)y,N),sv((int64_t*)rows,n),fv(params,161),sv(w.order,N));
    assert(t.status==COAX_TbStatus_ok);
}
static void predict(coax_t_TbModel m,double *params,const double *x,double *out){
    assert(psionic_tb_predict(m,fv(params,161),fv((double*)x,4*N),fv(out,N))==COAX_TbStatus_ok);
}
static double logloss(const double *z,const int64_t *rows,int64_t n){
    coax_t_TbValue v=tb_log_loss(fv((double*)z,N),fv(w.y,N),sv((int64_t*)rows,n));
    assert(v.status==COAX_TbStatus_ok);return v.value;
}

/* The chapter's experiment for one seed: returns test log loss (constant,
   logistic, MLP) and small-data test log loss (logistic, MLP, MLP + decay). */
static void experiment(uint64_t seed,double ll[3],double small[3],int full){
    coax_t_TbSim s=psionic_tb_simulate(seed,300,fv(w.x,4*N),fv(w.y,N),fv(w.lat,N),sv(w.g,N));
    assert(s.status==COAX_TbStatus_ok&&s.rows==N);
    w.ntr=psionic_tb_rows(sv(w.g,N),0,sv(w.tr,N));w.nva=psionic_tb_rows(sv(w.g,N),1,sv(w.va,N));
    w.nte=psionic_tb_rows(sv(w.g,N),2,sv(w.te,N));
    assert(w.ntr==1440&&w.nva==480&&w.nte==480);
    assert(psionic_tb_fit_scale(fv(w.x,4*N),4,sv(w.tr,w.ntr),fv(w.mean,4),fv(w.sd,4))==COAX_TbStatus_ok);
    assert(psionic_tb_apply_scale(fv(w.x,4*N),4,fv(w.mean,4),fv(w.sd,4),fv(w.xs,4*N))==COAX_TbStatus_ok);
    coax_t_TbConfig c={40,16,0.5,0,1,2};
    coax_t_TbModel lg={4,0,true},ml={4,8,true},li={4,0,false};
    static double plg[161],pml[161],pli[161],pc[N],zg[N],zm[N];
    coax_t_TbValue b=tb_base_logit(fv(w.y,N),sv(w.tr,w.ntr));assert(b.status==COAX_TbStatus_ok);
    for(int i=0;i<N;i++)pc[i]=b.value;
    train(lg,c,w.xs,w.y,w.tr,w.ntr,plg);predict(lg,plg,w.xs,zg);
    train(ml,c,w.xs,w.y,w.tr,w.ntr,pml);predict(ml,pml,w.xs,zm);
    ll[0]=logloss(pc,w.te,w.nte);ll[1]=logloss(zg,w.te,w.nte);ll[2]=logloss(zm,w.te,w.nte);
    if(full){
        for(int i=0;i<4*N;i++)assert(w.x[i]==tb_oracle_x[i]);
        for(int i=0;i<N;i++){assert(w.y[i]==tb_oracle_y[i]&&w.g[i]==tb_oracle_groups[i]);near(w.lat[i],tb_oracle_latency[i],1e-12);}
        for(int i=0;i<5;i++)near(plg[i],tb_oracle_logistic_params[i],1e-9);
        for(int i=0;i<49;i++)near(pml[i],tb_oracle_mlp_params[i],1e-9);
        near(logloss(zg,w.va,w.nva),tb_oracle_scaled_val,1e-9);
        /* Unscaled features at three learning rates. */
        double rates[3]={0.5,1e-3,1e-6};
        for(int r=0;r<3;r++){
            coax_t_TbConfig cr=c;cr.rate=rates[r];static double pr[161];
            train(lg,cr,w.x,w.y,w.tr,w.ntr,pr);predict(lg,pr,w.x,w.z);
            near(logloss(w.z,w.va,w.nva),tb_oracle_raw[r],1e-9);
        }
        /* The line predicts latency; its decision is "predicted seconds within the deadline". */
        coax_t_TbConfig cl=c;cl.rate=0.05;
        train(li,cl,w.xs,w.lat,w.tr,w.ntr,pli);predict(li,pli,w.xs,w.z);
        for(int i=0;i<5;i++)near(pli[i],tb_oracle_linear_params[i],1e-9);
        double mean=0;{double a=0;for(int i=0;i<w.ntr;i++)a+=w.lat[w.tr[i]];mean=a/w.ntr;}
        for(int i=0;i<N;i++)w.tmp[i]=mean;
        near(tb_mse(fv(w.tmp,N),fv(w.lat,N),sv(w.te,w.nte)).value,tb_oracle_mse[0],1e-9);
        near(tb_mse(fv(w.z,N),fv(w.lat,N),sv(w.te,w.nte)).value,tb_oracle_mse[1],1e-9);
        static double on[5][N];
        for(int i=0;i<N;i++){on[0][i]=sigmoid(pc[i]);on[2][i]=w.z[i]<=9.0;on[3][i]=sigmoid(zg[i]);on[4][i]=sigmoid(zm[i]);}
        assert(psionic_tb_rule(fv(w.x,4*N),fv(on[1],N))==COAX_TbStatus_ok);
        for(int k=0;k<5;k++){
            coax_t_TbClass q=tb_classify(fv(on[k],N),fv(w.y,N),sv(w.te,w.nte),0.5,5,1);
            assert(q.status==COAX_TbStatus_ok&&q.late==(int64_t)tb_oracle_classify[k][2]&&q.caught==(int64_t)tb_oracle_classify[k][3]);
            assert(q.accuracy==tb_oracle_classify[k][0]&&q.cost==tb_oracle_classify[k][1]);
        }
        for(int k=0;k<2;k++){
            double *pp=on[3+k];
            coax_t_TbChoice t=tb_choose(fv(pp,N),fv(w.y,N),sv(w.va,w.nva),5,1);
            assert(t.status==COAX_TbStatus_ok&&t.threshold==tb_oracle_choose[k][0]&&t.cost==tb_oracle_choose[k][1]);
            coax_t_TbClass q=tb_classify(fv(pp,N),fv(w.y,N),sv(w.te,w.nte),t.threshold,5,1);
            assert(q.accuracy==tb_oracle_chosen[k][0]&&q.cost==tb_oracle_chosen[k][1]&&q.caught==(int64_t)tb_oracle_chosen[k][3]);
        }
    }
    /* Small data: training sessions below 40 only, scaled on those rows. */
    static int64_t few[N];int64_t nf=0;
    for(int i=0;i<w.ntr;i++)if(w.g[w.tr[i]]<40)few[nf++]=w.tr[i];
    if(full)assert(nf==tb_oracle_small_rows);
    assert(psionic_tb_fit_scale(fv(w.x,4*N),4,sv(few,nf),fv(w.mean,4),fv(w.sd,4))==COAX_TbStatus_ok);
    assert(psionic_tb_apply_scale(fv(w.x,4*N),4,fv(w.mean,4),fv(w.sd,4),fv(w.xs,4*N))==COAX_TbStatus_ok);
    coax_t_TbConfig cs={200,16,0.5,0,1,2},cd=cs;cd.decay=0.01;
    static double pa[161];
    train(lg,cs,w.xs,w.y,few,nf,pa);predict(lg,pa,w.xs,w.z);small[0]=logloss(w.z,w.te,w.nte);
    train(ml,cs,w.xs,w.y,few,nf,pa);predict(ml,pa,w.xs,w.z);small[1]=logloss(w.z,w.te,w.nte);
    train(ml,cd,w.xs,w.y,few,nf,pa);predict(ml,pa,w.xs,w.z);small[2]=logloss(w.z,w.te,w.nte);
}

int main(void){
    double ll[3],small[3];int seeds=(int)(sizeof(tb_oracle_seeds)/sizeof(*tb_oracle_seeds));
    for(int k=0;k<seeds;k++){
        experiment(tb_oracle_seeds[k],ll,small,k==0);
        for(int j=0;j<3;j++){near(ll[j],tb_oracle_seed_log_loss[k][j],1e-9);near(small[j],tb_oracle_seed_small[k][j],1e-9);}
        if(k==0)for(int j=0;j<3;j++){near(ll[j],tb_oracle_log_loss[j],1e-9);near(small[j],tb_oracle_small[j],1e-9);}
    }
    /* Native refusals: overlap observed by the adapter, non-finite values by Coax. */
    static double a[64],b[16];static int64_t g[16];
    assert(psionic_tb_simulate(1,2,fv(a,64),fv(a+60,16),fv(b,16),sv(g,16)).status==COAX_TbStatus_alias);
    assert(psionic_tb_rows(sv(g,16),0,sv(g+4,8))==-1);
    double x[4]={1,2,3,NAN},mean[2]={0,0},sd[2]={1,1},out[4]={-7,-7,-7,-7};
    assert(psionic_tb_apply_scale(fv(x,4),2,fv(mean,2),fv(sd,2),fv(out,4))==COAX_TbStatus_number&&out[0]==-7);
    x[3]=INFINITY;assert(psionic_tb_apply_scale(fv(x,4),2,fv(mean,2),fv(sd,2),fv(out,4))==COAX_TbStatus_number);
    assert(psionic_tb_apply_scale(fv(x,4),2,fv(mean,2),fv(sd,2),fv(x,4))==COAX_TbStatus_alias);
    double y[2]={1,NAN},p[3]={-7,-7,-7};int64_t rows[2]={0,1},order[2];x[3]=4;
    coax_t_TbConfig c={1,2,0.5,0,0,0};coax_t_TbModel m={2,0,true};
    assert(psionic_tb_train(m,c,fv(x,4),fv(y,2),sv(rows,2),fv(p,3),sv(order,2)).status==COAX_TbStatus_number&&p[0]==-7);
    y[1]=0;assert(psionic_tb_train(m,c,fv(x,4),fv(y,2),sv(rows,2),fv(x,3),sv(order,2)).status==COAX_TbStatus_alias);
    assert(psionic_tb_train(m,c,fv(x,4),fv(y,2),sv(rows,2),fv(p,3),sv(rows,2)).status==COAX_TbStatus_alias);
    assert(psionic_tb_predict(m,fv(p,3),fv(x,4),fv(p,2))==COAX_TbStatus_alias);
    assert(psionic_tb_rule(fv(a,8),fv(a+1,2))==COAX_TbStatus_alias);
    printf("tabular: %d seeds of %d requests agree with the oracle (draws, labels, splits, thresholds and counts exact; worst relative difference %.1e); overlap and non-finite input refused\n",
        seeds,N,worst);
    return 0;
}
