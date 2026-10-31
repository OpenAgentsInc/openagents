/* Chapters 226 and 227 against the independent oracle in gate_oracle.py. */
#include "gate-spans.h"
#include "gate-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

static coax_v_f64 fv(double *p,int64_t n){return (coax_v_f64){p,n};}
static coax_v_s64 sv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}

int main(void){
    for(size_t c=0;c<sizeof(gadget_cases)/sizeof(*gadget_cases);c++){
        const gadget_case *g=&gadget_cases[c];
        double got=ra_gadget((coax_t_RaGadget)g->gadget,g->a,g->b,g->c,g->single);
        assert(got==g->want);
    }
    /* NaN and infinities are refused; output untouched. */
    double w[12]={0},x[2]={1,5},out[2]={-9,-9};
    x[1]=NAN;assert(psionic_ra_forward(2,2,fv(w,12),fv(x,2),fv(out,2),false)==COAX_RaStatus_number);
    x[1]=INFINITY;assert(psionic_ra_forward(2,2,fv(w,12),fv(x,2),fv(out,2),false)==COAX_RaStatus_number);
    x[1]=5;w[4]=-INFINITY;assert(psionic_ra_forward(2,2,fv(w,12),fv(x,2),fv(out,2),false)==COAX_RaStatus_number);
    assert(out[0]==-9 && out[1]==-9);w[4]=0;
    assert(psionic_ra_forward(2,2,fv(w,12),fv(x,2),fv(w,2),false)==COAX_RaStatus_alias);
    size_t compiled=0,refused=0,long_forms=0;static double weights[1<<16];
    for(size_t c=0;c<sizeof(graph_cases)/sizeof(*graph_cases);c++){
        const graph_case *t=&graph_cases[c];coax_t_GgNode nodes[24];int64_t outs[3],in[9],vals[3];double got[3];
        for(int i=0;i<t->n;i++)nodes[i]=(coax_t_GgNode){(coax_t_GgOp)t->op[i],t->a[i],t->b[i],t->c[i],t->k[i]};
        for(int i=0;i<t->outputs;i++)outs[i]=t->out[i];
        for(int i=0;i<t->inputs;i++)in[i]=t->in[i];
        coax_v_t_GgNode g={nodes,t->n};
        coax_t_GgResult r=gg_eval(g,sv(in,t->inputs),sv(outs,t->outputs),sv(vals,3));
        if(t->ok==0){assert(r.status==COAX_GgStatus_range && r.node==t->want[0]);refused++;continue;}
        assert(r.status==COAX_GgStatus_ok);
        coax_t_GgPlan plan=gg_compile(g,sv(outs,t->outputs),fv(weights,1<<16));
        if(t->ok==2){assert(plan.status==COAX_GgStatus_terms && plan.node==t->want[0]);long_forms++;continue;}
        for(int i=0;i<t->outputs;i++)assert(vals[i]==t->want[i]);
        assert(plan.status==COAX_GgStatus_ok);
        assert(psionic_gg_run(plan,fv(weights,1<<16),sv(in,t->inputs),fv(got,3),false)==COAX_GgStatus_ok);
        for(int i=0;i<t->outputs;i++)assert(got[i]==(double)t->want[i]);
        compiled++;
    }
    /* An operation code outside the enumeration is refused at admission. */
    coax_t_GgNode bad[1]={{(coax_t_GgOp)10,0,0,0,0}};int64_t o[1]={0};
    assert(gg_admit((coax_v_t_GgNode){bad,1},sv(o,1)).status==COAX_GgStatus_operand);
    printf("gates: %zu gadget results agree; %zu graphs compiled and reproduced exactly; %zu out of range and %zu over the term limit, each refused at the oracle's node\n",
        sizeof(gadget_cases)/sizeof(*gadget_cases),compiled,refused,long_forms);
    return 0;
}
