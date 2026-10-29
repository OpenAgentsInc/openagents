/* Observe native span overlap; fitting, mapping and measuring stay in Coax. */
#include "calibrate-spans.h"
#include "spans.h"
#define OV(a,b) psionic_spans_overlap((a).data,(a).count,sizeof(*(a).data), \
    (b).data,(b).count,sizeof(*(b).data))

coax_t_CfStatus psionic_cf_simulate(uint64_t seed, coax_v_f64 model, coax_v_s64 q, coax_v_f64 p, coax_v_f64 y) {
    return cf_simulate(seed,model,q,p,y,OV(q,p)||OV(q,y)||OV(p,y)||OV(model,q)||OV(model,p)||OV(model,y));
}

coax_t_CfStatus psionic_cf_apply(coax_v_t_CfMap maps, uint64_t model, uint64_t questions, coax_v_s64 q, coax_v_f64 p,
        coax_v_f64 out) {
    return cf_apply(maps,model,questions,q,p,out,OV(out,maps)||OV(out,q)||OV(out,p));
}

coax_t_CfStatus psionic_cf_reliability(coax_v_f64 p, coax_v_f64 y, coax_v_s64 rows, coax_v_s64 count, coax_v_f64 mean,
        coax_v_f64 rate) {
    return cf_reliability(p,y,rows,count,mean,rate,
        OV(count,p)||OV(count,y)||OV(count,rows)||OV(mean,p)||OV(mean,y)||OV(mean,rows)||OV(rate,p)||OV(rate,y)
        ||OV(rate,rows)||OV(count,mean)||OV(count,rate)||OV(mean,rate));
}
