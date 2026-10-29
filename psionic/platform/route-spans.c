/* Observe native span overlap; routing and estimation stay in Coax. */
#include "route-spans.h"
#include "spans.h"
#define OV(a,b) psionic_spans_overlap((a).data,(a).count,sizeof(*(a).data), \
    (b).data,(b).count,sizeof(*(b).data))

coax_t_RtStatus psionic_rt_price(coax_v_f64 x, coax_v_f64 out) {
    return rt_price(x,out,OV(out,x));
}

coax_t_RtStatus psionic_rt_simulate(uint64_t seed, coax_v_f64 truth, coax_v_f64 x, coax_v_f64 success, coax_v_f64 prob) {
    bool o = OV(x,success)||OV(x,prob)||OV(success,prob);
    return rt_simulate(seed,truth,x,success,prob,o||OV(x,truth)||OV(success,truth)||OV(prob,truth));
}

coax_t_RtStatus psionic_rt_scores(coax_v_f64 params, coax_v_f64 x, coax_v_f64 out) {
    return rt_scores(params,x,out,OV(out,params)||OV(out,x));
}

coax_t_RtStatus psionic_rt_choose(coax_v_f64 prob, coax_v_f64 price, coax_v_s64 out) {
    return rt_choose(prob,price,out,OV(out,prob)||OV(out,price));
}

coax_t_RtStatus psionic_rt_old(coax_v_f64 x, double explore, bool large, coax_v_f64 out) {
    return rt_old(x,explore,large,out,OV(out,x));
}

coax_t_RtStatus psionic_rt_log(uint64_t seed, coax_v_f64 props, coax_v_f64 success, coax_v_f64 price,
        coax_v_s64 actions, coax_v_f64 outcome, coax_v_f64 reward) {
    bool o = OV(actions,outcome)||OV(actions,reward)||OV(outcome,reward);
    o = o||OV(actions,props)||OV(actions,success)||OV(actions,price);
    o = o||OV(outcome,props)||OV(outcome,success)||OV(outcome,price);
    o = o||OV(reward,props)||OV(reward,success)||OV(reward,price);
    return rt_log(seed,props,success,price,actions,outcome,reward,o);
}

int64_t psionic_rt_rows(coax_v_s64 actions, int64_t k, coax_v_s64 out) {
    return rt_rows(actions,k,out,OV(out,actions));
}

coax_t_RtStatus psionic_rt_average(coax_v_s64 actions, coax_v_f64 outcome, coax_v_f64 out) {
    return rt_average(actions,outcome,out,OV(out,actions)||OV(out,outcome));
}
