/* Native addresses are observations; all admission and copying is Coax. */
#include "tensor-memory.h"
#include "spans.h"

coax_t_PsionicCopy psionic_tensor_copy(coax_t_PsionicShape shape,
        coax_v_f32 source, coax_v_f32 destination, coax_v_f32 scratch) {
    bool overlap = psionic_f32_spans_overlap(destination.data, destination.count, source.data, source.count) ||
        psionic_f32_spans_overlap(scratch.data, scratch.count, source.data, source.count) ||
        psionic_f32_spans_overlap(destination.data, destination.count, scratch.data, scratch.count);
    return psionic_tensor_copy_checked(shape, source, destination, scratch, overlap);
}
