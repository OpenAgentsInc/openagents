/* Observe native span overlap; admission, execution and digests stay in Coax. */
#include "trace-memory.h"
#include "spans.h"
#define OV(a,b) psionic_spans_overlap((a).data,(a).count,sizeof(*(a).data), \
    (b).data,(b).count,sizeof(*(b).data))

coax_t_TmResult psionic_tm_run(coax_v_t_TmOp program, int64_t budget,
        coax_v_s64 trace, coax_v_s64 outputs) {
    bool alias=OV(trace,outputs)||OV(trace,program)||OV(outputs,program);
    return tm_run(program,budget,trace,outputs,alias);
}

coax_t_TmResult psionic_em_run(coax_v_t_TmOp program, int64_t budget,
        coax_v_s64 trace, coax_v_s64 outputs, coax_v_s64 keys, coax_v_s64 values) {
    coax_v_s64 w[4]={trace,outputs,keys,values};bool alias=false;
    for(int i=0;i<4;i++){
        if(OV(w[i],program))alias=true;
        for(int j=0;j<i;j++)if(OV(w[i],w[j]))alias=true;
    }
    return em_run(program,budget,trace,outputs,keys,values,alias);
}
