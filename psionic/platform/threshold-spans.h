#ifndef PSIONIC_THRESHOLD_SPANS_H
#define PSIONIC_THRESHOLD_SPANS_H
#include "threshold.h"
#include <stdbool.h>
coax_t_ThStatus psionic_th_simulate(uint64_t seed, double rho, coax_v_f64 elephant, coax_v_f64 mammoth, coax_v_f64 label);
coax_t_ThReport psionic_th_run(coax_t_ThCosts c, coax_t_ThPolicy policy, coax_t_ThRecord r, coax_v_f64 elephant,
    coax_v_f64 mammoth, coax_v_f64 label, coax_v_f64 cost);
coax_t_ThValue psionic_th_paired(coax_v_f64 a, coax_v_f64 b, coax_v_f64 scratch);
#endif
