/* Observe native span overlap; costs and decisions stay in Coax. */
#include "threshold-spans.h"
#include "spans.h"
#define OV(a,b) psionic_spans_overlap((a).data,(a).count,sizeof(*(a).data), \
    (b).data,(b).count,sizeof(*(b).data))

coax_t_ThStatus psionic_th_simulate(uint64_t seed, double rho, coax_v_f64 elephant, coax_v_f64 mammoth, coax_v_f64 label) {
    return th_simulate(seed,rho,elephant,mammoth,label,OV(elephant,mammoth)||OV(elephant,label)||OV(mammoth,label));
}

coax_t_ThReport psionic_th_run(coax_t_ThCosts c, coax_t_ThPolicy policy, coax_t_ThRecord r, coax_v_f64 elephant,
        coax_v_f64 mammoth, coax_v_f64 label, coax_v_f64 cost) {
    return th_run(c,policy,r,elephant,mammoth,label,cost,OV(cost,elephant)||OV(cost,mammoth)||OV(cost,label));
}

coax_t_ThValue psionic_th_paired(coax_v_f64 a, coax_v_f64 b, coax_v_f64 scratch) {
    return th_paired(a,b,scratch,OV(scratch,a)||OV(scratch,b));
}
