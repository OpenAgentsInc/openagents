#ifndef PSIONIC_MEMORY_H
#define PSIONIC_MEMORY_H
#include <stdbool.h>
#include "learn.h"

/* Spans are caller-owned float storage, alive throughout this synchronous call.
   Inputs may share storage. Writable spans must be disjoint from every other
   span. The Coax core owns admission and all numerical work. */
coax_t_PsionicTraining psionic_train(coax_v_f32 xs, coax_v_f32 ys,
    float initial_weight, int64_t steps, coax_v_f32 trace, coax_v_f32 scratch);
#endif
