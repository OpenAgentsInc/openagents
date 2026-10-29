/* Chapter 71 against two independent references: 300 random programs whose
   values and derivatives tape_oracle.py computed in 60-digit decimal
   arithmetic by central differences, and chapter 70's oracle, which trained
   the same models with hand-written gradients in exactly rounded sums.
   Tolerances were frozen before comparison: results 1e-11 relative, program
   gradients 1e-9 of max(1, |g|), trained parameters 1e-9 relative. */
#include "tape-spans.h"
#include "tape-oracle.h"
#include "tabular-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>

#define N TB_ORACLE_ROWS
static coax_v_f64 fv(double *p,int64_t n){return (coax_v_f64){p,n};}
static coax_v_s64 sv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}
static coax_v_t_TpNode nv(coax_t_TpNode *p,int64_t n){return (coax_v_t_TpNode){p,n};}
static double worst_value,worst_grad,worst_param;
static double rel(double got,double want,double floor){return fabs(got-want)/fmax(fabs(want),floor);}

static double x[4*N],y[N],lat[N],xs[4*N],mean[4],sd[4];static int64_t g[N],tr[N];
static coax_t_TpNode nodes[4096];static double values[4096];static int64_t order[N];

static void trained(coax_t_TbModel m,double rate,const double *target,const double *want,int count){
    coax_t_TbConfig c={40,16,rate,0,1,2};double p[161];
    coax_t_TbTrain t=tp_train(m,c,fv(xs,4*N),fv((double*)target,N),sv(tr,1440),fv(p,161),sv(order,N),nv(nodes,4096),fv(values,4096),false);
    assert(t.status==COAX_TbStatus_ok&&t.steps==40*90);
    for(int s=0;s<count;s++){double e=rel(p[s],want[s],1e-300);assert(e<=1e-9);if(e>worst_param)worst_param=e;}
}

int main(void){
    int cases=(int)(sizeof(tape_cases)/sizeof(*tape_cases)),params=0;
    for(int k=0;k<cases;k++){
        const tape_case *t=&tape_cases[k];coax_t_TpNode prog[32];double p[4],v[32],gr[4]={0,0,0,0};
        for(int i=0;i<t->n;i++)prog[i]=(coax_t_TpNode){(coax_t_TpOp)t->op[i],t->a[i],t->b[i],t->slot[i],t->value[i]};
        for(int s=0;s<t->params;s++)p[s]=t->p[s];
        assert(psionic_tp_forward(nv(prog,t->n),fv(p,t->params),fv(v,32))==COAX_TpStatus_ok);
        double e=rel(v[t->n-1],t->result,1);assert(e<=1e-11);if(e>worst_value)worst_value=e;
        assert(psionic_tp_backward(nv(prog,t->n),fv(v,t->n),fv(gr,t->params))==COAX_TpStatus_ok);
        for(int s=0;s<t->params;s++){e=rel(gr[s],t->grad[s],1);assert(e<=1e-9);if(e>worst_grad)worst_grad=e;params++;}
    }
    /* Chapter 70's seed-70 experiment, trained through the tape. */
    assert(tb_simulate(70,300,fv(x,4*N),fv(y,N),fv(lat,N),sv(g,N),false).status==COAX_TbStatus_ok);
    assert(tb_rows(sv(g,N),0,sv(tr,N),false)==1440);
    assert(tb_fit_scale(fv(x,4*N),4,sv(tr,1440),fv(mean,4),fv(sd,4),false)==COAX_TbStatus_ok);
    assert(tb_apply_scale(fv(x,4*N),4,fv(mean,4),fv(sd,4),fv(xs,4*N),false)==COAX_TbStatus_ok);
    trained((coax_t_TbModel){4,0,true},0.5,y,tb_oracle_logistic_params,5);
    trained((coax_t_TbModel){4,8,true},0.5,y,tb_oracle_mlp_params,49);
    trained((coax_t_TbModel){4,0,false},0.05,lat,tb_oracle_linear_params,5);
    /* Native refusals: an operation outside the enumeration, NaN and infinity, overlap. */
    coax_t_TpNode bad[2]={{COAX_TpOp_param,0,0,0,0},{(coax_t_TpOp)13,0,0,0,0}};double p[1]={NAN},v[2]={-7,-7},gr[1]={-7};
    assert(psionic_tp_forward(nv(bad,2),fv(p,1),fv(v,2))==COAX_TpStatus_shape);
    bad[1].op=COAX_TpOp_exp;assert(psionic_tp_forward(nv(bad,2),fv(p,1),fv(v,2))==COAX_TpStatus_number);
    p[0]=INFINITY;assert(psionic_tp_forward(nv(bad,2),fv(p,1),fv(v,2))==COAX_TpStatus_number&&v[0]==-7);
    v[0]=NAN;v[1]=1;assert(psionic_tp_backward(nv(bad,2),fv(v,2),fv(gr,1))==COAX_TpStatus_number&&gr[0]==-7);
    double pv[2]={1,0};assert(psionic_tp_forward(nv(bad,2),fv(pv,1),fv(pv,2))==COAX_TpStatus_alias);
    assert(psionic_tp_backward(nv(bad,2),fv(v,2),fv(v+1,1))==COAX_TpStatus_alias);
    coax_t_TbModel m={1,0,true};double tx[2]={1,2},ty[2]={1,0},tp[2]={0,0},tg[2];int64_t rows[2]={0,1};
    assert(psionic_tp_tabular(m,fv(tx,2),fv(ty,2),sv(rows,2),nv((coax_t_TpNode*)tx,1)).status==COAX_TpStatus_alias);
    assert(psionic_tp_gradient(m,fv(tp,2),fv(tx,2),fv(ty,2),sv(rows,2),fv(tg,2),nv(nodes,64),fv(tp,2))==COAX_TpStatus_alias);
    assert(psionic_tp_gradient(m,fv(tp,2),fv(tx,2),fv(ty,2),sv(rows,2),fv(tg,2),nv(nodes,64),fv(values,64))==COAX_TpStatus_ok);
    printf("tape: %d programs and %d gradients agree with 60-digit central differences (worst %.1e and %.1e); "
        "logistic, hidden-layer and line training reproduce chapter 70's oracle (worst %.1e); bad operations, NaN, infinity and overlap refused\n",
        cases,params,worst_value,worst_grad,worst_param);
    return 0;
}
