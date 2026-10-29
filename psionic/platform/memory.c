/* Native address observations only. No training, capacity or numeric policy. */
#include "memory.h"
#include "spans.h"

coax_t_PsionicTraining psionic_train(coax_v_f32 xs, coax_v_f32 ys,
        float initial_weight, int64_t steps, coax_v_f32 trace, coax_v_f32 scratch) {
    bool overlap = psionic_f32_spans_overlap(trace.data, trace.count, xs.data, xs.count) ||
        psionic_f32_spans_overlap(trace.data, trace.count, ys.data, ys.count) ||
        psionic_f32_spans_overlap(scratch.data, scratch.count, xs.data, xs.count) ||
        psionic_f32_spans_overlap(scratch.data, scratch.count, ys.data, ys.count) ||
        psionic_f32_spans_overlap(trace.data, trace.count, scratch.data, scratch.count);
    return psionic_train_checked(xs, ys, initial_weight, steps, trace, scratch, overlap);
}
