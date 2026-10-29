#ifndef PSIONIC_GATE_SPANS_H
#define PSIONIC_GATE_SPANS_H
#include "gate-graph.h"
#include <stdbool.h>
coax_t_RaStatus psionic_ra_forward(int64_t width, int64_t units, coax_v_f64 w, coax_v_f64 x,
    coax_v_f64 out, bool single);
coax_t_GgStatus psionic_gg_run(coax_t_GgPlan plan, coax_v_f64 weights, coax_v_s64 inputs,
    coax_v_f64 outputs, bool single);
#endif
