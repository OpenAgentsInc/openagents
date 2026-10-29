#ifndef PSIONIC_TRACE_MEMORY_H
#define PSIONIC_TRACE_MEMORY_H
#include "exact-memory.h"
coax_t_TmResult psionic_tm_run(coax_v_t_TmOp program, int64_t budget,
    coax_v_s64 trace, coax_v_s64 outputs);
coax_t_TmResult psionic_em_run(coax_v_t_TmOp program, int64_t budget,
    coax_v_s64 trace, coax_v_s64 outputs, coax_v_s64 keys, coax_v_s64 values);
#endif
