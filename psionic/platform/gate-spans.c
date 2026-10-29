/* Observe native span overlap; layer arithmetic and compilation stay in Coax. */
#include "gate-spans.h"
#include "spans.h"
#define OV(a,b) psionic_spans_overlap((a).data,(a).count,sizeof(*(a).data), \
    (b).data,(b).count,sizeof(*(b).data))

coax_t_RaStatus psionic_ra_forward(int64_t width, int64_t units, coax_v_f64 w, coax_v_f64 x,
        coax_v_f64 out, bool single) {
    return ra_forward(width,units,w,x,out,single,OV(out,w)||OV(out,x));
}

coax_t_GgStatus psionic_gg_run(coax_t_GgPlan plan, coax_v_f64 weights, coax_v_s64 inputs,
        coax_v_f64 outputs, bool single) {
    return gg_run(plan,weights,inputs,outputs,single,OV(outputs,weights)||OV(outputs,inputs));
}
