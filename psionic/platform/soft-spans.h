#ifndef PSIONIC_SOFT_SPANS_H
#define PSIONIC_SOFT_SPANS_H
#include "soft-memory.h"
coax_t_SmLimit psionic_sm_limit(int64_t k, int64_t v, double beta, coax_t_SmFormat format,
    coax_v_s64 keys, coax_v_s64 values);
#endif
