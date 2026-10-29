/* Chapter 169: the harness's interpreter against the independent oracle, and every planted fault caught natively. */
#include "compiler-harness.h"
#include "gate-oracle.h"
#include "harness-oracle.h"
#include <assert.h>
#include <stdio.h>

static coax_v_s64 sv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}

int main(void){
    size_t checked=0;static double weights[1<<16];
    for(size_t c=0;c<sizeof(graph_cases)/sizeof(*graph_cases);c++){
        const graph_case *t=&graph_cases[c];if(t->ok!=1)continue;
        coax_t_GgNode nodes[24];int64_t outs[3],in[9],vals[24],read[3];
        for(int i=0;i<t->n;i++)nodes[i]=(coax_t_GgNode){(coax_t_GgOp)t->op[i],t->a[i],t->b[i],t->c[i],t->k[i]};
        for(int i=0;i<t->outputs;i++)outs[i]=t->out[i];
        for(int i=0;i<t->inputs;i++)in[i]=t->in[i];
        coax_v_t_GgNode g={nodes,t->n};
        coax_t_GgPlan plan=gg_compile(g,sv(outs,t->outputs),(coax_v_f64){weights,1<<16});
        if(plan.status==COAX_GgStatus_terms)continue; /* a gadget's own form overflowed: a declared refusal */
        assert(plan.status==COAX_GgStatus_ok);
        assert(ch_interpret(plan,g,sv(in,t->inputs),sv(vals,24),sv(read,3)).status==COAX_GgStatus_ok);
        for(int i=0;i<t->outputs;i++)assert(vals[t->out[i]]==t->want[i] && read[i]==t->want[i]);
        checked++;
    }
    /* Every case the generator makes matches the oracle's copy of its description. */
    for(size_t c=0;c<sizeof(harness_cases)/sizeof(*harness_cases);c++){
        const harness_case *h=&harness_cases[c];coax_t_GgNode nodes[16];
        coax_t_ChCase k=ch_case(h->seed,h->graph,(coax_v_t_GgNode){nodes,16});
        assert(k.count==h->count && k.inputs==h->inputs && k.outputs.e[0]==h->outputs[0] && k.outputs.e[1]==h->outputs[1]);
        for(int i=0;i<h->inputs;i++)assert(k.values.e[i]==h->values[i]);
        for(int i=0;i<h->count;i++)assert((int)nodes[i].op==h->op[i] && nodes[i].a==h->a[i] && nodes[i].b==h->b[i] && nodes[i].c==h->c[i] && nodes[i].k==h->k[i]);
    }
    size_t honest=0,skipped=0;
    for(int seed=1;seed<=20;seed++){
        coax_t_ChCatch h=ch_differential(seed,1000,COAX_GgFault_none);
        assert(h.status==COAX_GgStatus_ok && !h.caught && h.skipped==harness_skipped[seed] && h.graphs+h.skipped==1000);
        honest+=(size_t)h.graphs;skipped+=(size_t)h.skipped;
        for(int f=1;f<=4;f++){
            coax_t_ChCatch r=ch_differential(seed,1000,(coax_t_GgFault)f);
            assert(r.caught && r.graph<20);
            assert(r.where==(f==4?COAX_ChWhere_emission:COAX_ChWhere_schedule));
        }
    }
    printf("compiler harness: interpreter matches the oracle on %zu graphs; %zu generated cases match its generator; %zu honest graphs agree and %zu are skipped as predicted; every planted fault caught in 20 seeds\n",
        checked,sizeof(harness_cases)/sizeof(*harness_cases),honest,skipped);
    return 0;
}
