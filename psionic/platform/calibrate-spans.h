#ifndef PSIONIC_CALIBRATE_SPANS_H
#define PSIONIC_CALIBRATE_SPANS_H
#include "calibrate.h"
#include <stdbool.h>
coax_t_CfStatus psionic_cf_simulate(uint64_t seed, coax_v_f64 model, coax_v_s64 q, coax_v_f64 p, coax_v_f64 y);
coax_t_CfStatus psionic_cf_apply(coax_v_t_CfMap maps, uint64_t model, uint64_t questions, coax_v_s64 q, coax_v_f64 p,
    coax_v_f64 out);
coax_t_CfStatus psionic_cf_reliability(coax_v_f64 p, coax_v_f64 y, coax_v_s64 rows, coax_v_s64 count, coax_v_f64 mean,
    coax_v_f64 rate);
#endif
