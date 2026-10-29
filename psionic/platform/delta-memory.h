#ifndef PSIONIC_DELTA_MEMORY_H
#define PSIONIC_DELTA_MEMORY_H
#include "delta.h"
/* C observes declared address ranges only. All extents, lifetime, alignment
   and exclusive access are caller obligations. */
coax_t_PdResult psionic_delta_step(coax_t_PdShape, coax_v_f32, coax_v_f32,
    coax_v_f32, coax_v_f32, coax_v_f32);
#endif
