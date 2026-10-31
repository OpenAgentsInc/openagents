/* Chapters 224 and 225 against the independent oracle in trace_oracle.py. */
#include "trace-memory.h"
#include "trace-oracle.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static coax_v_s64 sv(int64_t *p,int64_t n){return (coax_v_s64){p,n};}

int main(void){
    size_t ok=0,refused=0;
    for(size_t c=0;c<sizeof(trace_cases)/sizeof(*trace_cases);c++){
        const trace_case *t=&trace_cases[c];coax_t_TmOp program[64];
        for(int i=0;i<t->n;i++)program[i]=(coax_t_TmOp){(coax_t_TmCode)t->code[i],t->arg[i]};
        static int64_t trace[4*4096],trace2[4*4096],keys[4096],values[4096];int64_t out[64],out2[64];
        for(size_t i=0;i<sizeof trace/sizeof *trace;i++)trace[i]=trace2[i]=-9;
        for(int i=0;i<64;i++)out[i]=out2[i]=-9;
        coax_v_t_TmOp p={program,t->n};
        coax_t_TmResult a=psionic_tm_run(p,t->budget,sv(trace,4*4096),sv(out,64));
        coax_t_TmResult b=psionic_em_run(p,t->budget,sv(trace2,4*4096),sv(out2,64),sv(keys,4096),sv(values,4096));
        assert((int)a.status==t->status && (int)b.status==t->status);
        if(t->status==COAX_TmStatus_ok){
            ok++;
            assert(a.steps==t->steps && b.steps==t->steps && a.outputs==t->outputs && b.outputs==t->outputs);
            assert(!memcmp(a.digest.e,t->digest,32) && !memcmp(b.digest.e,t->digest,32));
            assert(!memcmp(trace,trace2,sizeof(int64_t)*4*(size_t)t->steps));
            for(int i=0;i<t->outputs;i++)assert(out[i]==t->out[i] && out2[i]==t->out[i]);
            assert(trace[4*t->steps]==-9 && out[t->outputs]==-9);
        } else {
            refused++;
            assert(trace[0]==-9 && out[0]==-9 && trace2[0]==-9 && out2[0]==-9);
        }
    }
    size_t reads=0;
    for(size_t s=0;s<sizeof(memory_streams)/sizeof(*memory_streams);s++){
        const memory_stream *m=&memory_streams[s];int64_t k[48],v[48];
        memcpy(k,m->key,sizeof k);memcpy(v,m->value,sizeof v);
        for(int q=0;q<15;q++){
            coax_t_EmRead r=em_read(sv(k,48),sv(v,48),m->count,m->query[q]);reads++;
            if(m->found[q])assert(r.status==COAX_EmStatus_ok && r.value==m->expect[q] && k[r.position]==m->query[q]);
            else assert(r.status==COAX_EmStatus_missing);
        }
    }
    /* Overlapping trace and outputs are refused before anything is written. */
    coax_t_TmOp one[3]={{COAX_TmCode_push,1},{COAX_TmCode_out,0},{COAX_TmCode_ret,0}};int64_t buf[16];
    for(int i=0;i<16;i++)buf[i]=-9;
    assert(psionic_tm_run((coax_v_t_TmOp){one,3},8,sv(buf,16),sv(buf+4,1)).status==COAX_TmStatus_alias);
    assert(psionic_em_run((coax_v_t_TmOp){one,3},8,sv(buf,12),sv(buf+12,1),sv(buf+13,2),sv(buf+14,2)).status==COAX_TmStatus_alias);
    for(int i=0;i<16;i++)assert(buf[i]==-9);
    printf("trace machine: %zu programs agree with the oracle (%zu ran, %zu refused); %zu attention reads exact\n",ok+refused,ok,refused,reads);
    return 0;
}
