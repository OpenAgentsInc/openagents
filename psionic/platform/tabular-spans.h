#ifndef PSIONIC_TABULAR_SPANS_H
#define PSIONIC_TABULAR_SPANS_H
#include "tabular.h"
#include <stdbool.h>
coax_t_TbSim psionic_tb_simulate(uint64_t seed, int64_t sessions, coax_v_f64 x, coax_v_f64 y,
    coax_v_f64 latency, coax_v_s64 groups);
int64_t psionic_tb_rows(coax_v_s64 groups, int64_t part, coax_v_s64 out);
coax_t_TbStatus psionic_tb_fit_scale(coax_v_f64 x, int64_t d, coax_v_s64 rows, coax_v_f64 mean,
    coax_v_f64 sd);
coax_t_TbStatus psionic_tb_apply_scale(coax_v_f64 x, int64_t d, coax_v_f64 mean, coax_v_f64 sd,
    coax_v_f64 out);
coax_t_TbTrain psionic_tb_train(coax_t_TbModel m, coax_t_TbConfig c, coax_v_f64 x, coax_v_f64 y,
    coax_v_s64 rows, coax_v_f64 params, coax_v_s64 order);
coax_t_TbStatus psionic_tb_predict(coax_t_TbModel m, coax_v_f64 params, coax_v_f64 x, coax_v_f64 out);
coax_t_TbStatus psionic_tb_rule(coax_v_f64 x, coax_v_f64 out);
#endif
