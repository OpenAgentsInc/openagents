/* Native address observations only. No training, capacity or numeric policy. */
#include <stddef.h>
#include <stdint.h>
#include "memory.h"

static bool overlaps(coax_v_f32 a, coax_v_f32 b) {
    /* Invalid/unrepresentable span metadata cannot establish disjoint storage.
       Coax checks sample/step/capacity metadata before using the observation.
       As with other C view APIs, valid pointers and extents are caller duties. */
    if (a.count < 0 || b.count < 0) return true;
    if (a.count == 0 || b.count == 0) return false;
    if (a.data == NULL || b.data == NULL) return true;
    if ((uint64_t)a.count > UINTPTR_MAX / sizeof(float) ||
        (uint64_t)b.count > UINTPTR_MAX / sizeof(float)) return true;
    uintptr_t ap = (uintptr_t)a.data, bp = (uintptr_t)b.data;
    uintptr_t an = (uintptr_t)a.count * sizeof(float);
    uintptr_t bn = (uintptr_t)b.count * sizeof(float);
    if (ap > UINTPTR_MAX - an || bp > UINTPTR_MAX - bn) return true;
    return ap < bp + bn && bp < ap + an;
}

coax_t_PsionicTraining psionic_train(coax_v_f32 xs, coax_v_f32 ys,
        float initial_weight, int64_t steps, coax_v_f32 trace, coax_v_f32 scratch) {
    bool overlap = overlaps(trace, xs) || overlaps(trace, ys) ||
        overlaps(scratch, xs) || overlaps(scratch, ys) || overlaps(trace, scratch);
    return psionic_train_checked(xs, ys, initial_weight, steps, trace, scratch, overlap);
}
