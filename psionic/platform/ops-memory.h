#ifndef PSIONIC_OPS_MEMORY_H
#define PSIONIC_OPS_MEMORY_H
#include "ops.h"

/* Synchronous CPU-f32-wide-v1. Exact logical read-only views; destination and
   scratch have at least the plan's output count. Read-only inputs may alias.
   All writable declared spans are disjoint from every other span, including
   typed indices/mask and unused writable capacity. Callers own truthful valid
   allocations, alignment, lifetime and exclusive access. */
coax_t_PsionicResult psionic_ops_run(coax_t_PsionicRequest request,
    coax_v_f32 a, coax_v_f32 b, coax_v_s64 indices, coax_v_u8 mask,
    coax_v_f32 destination, coax_v_f32 scratch);
#endif
