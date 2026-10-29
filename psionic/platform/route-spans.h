#ifndef PSIONIC_ROUTE_SPANS_H
#define PSIONIC_ROUTE_SPANS_H
#include "route.h"
#include <stdbool.h>
coax_t_RtStatus psionic_rt_price(coax_v_f64 x, coax_v_f64 out);
coax_t_RtStatus psionic_rt_simulate(uint64_t seed, coax_v_f64 truth, coax_v_f64 x, coax_v_f64 success, coax_v_f64 prob);
coax_t_RtStatus psionic_rt_scores(coax_v_f64 params, coax_v_f64 x, coax_v_f64 out);
coax_t_RtStatus psionic_rt_choose(coax_v_f64 prob, coax_v_f64 price, coax_v_s64 out);
coax_t_RtStatus psionic_rt_old(coax_v_f64 x, double explore, bool large, coax_v_f64 out);
coax_t_RtStatus psionic_rt_log(uint64_t seed, coax_v_f64 props, coax_v_f64 success, coax_v_f64 price,
    coax_v_s64 actions, coax_v_f64 outcome, coax_v_f64 reward);
int64_t psionic_rt_rows(coax_v_s64 actions, int64_t k, coax_v_s64 out);
coax_t_RtStatus psionic_rt_average(coax_v_s64 actions, coax_v_f64 outcome, coax_v_f64 out);
#endif
