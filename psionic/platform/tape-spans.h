#ifndef PSIONIC_TAPE_SPANS_H
#define PSIONIC_TAPE_SPANS_H
#include "tape.h"
#include <stdbool.h>
coax_t_TpStatus psionic_tp_forward(coax_v_t_TpNode nodes, coax_v_f64 params, coax_v_f64 values);
coax_t_TpStatus psionic_tp_backward(coax_v_t_TpNode nodes, coax_v_f64 values, coax_v_f64 grads);
coax_t_TpBuild psionic_tp_tabular(coax_t_TbModel m, coax_v_f64 x, coax_v_f64 y, coax_v_s64 rows,
    coax_v_t_TpNode nodes);
coax_t_TpStatus psionic_tp_gradient(coax_t_TbModel m, coax_v_f64 p, coax_v_f64 x, coax_v_f64 y,
    coax_v_s64 rows, coax_v_f64 g, coax_v_t_TpNode nodes, coax_v_f64 values);
#endif
