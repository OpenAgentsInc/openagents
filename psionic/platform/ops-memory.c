/* Typed address observations only; no tensor arithmetic or request policy. */
#include "ops-memory.h"
#include "spans.h"

coax_t_PsionicResult psionic_ops_run(coax_t_PsionicRequest request,
        coax_v_f32 a, coax_v_f32 b, coax_v_s64 indices, coax_v_u8 mask,
        coax_v_f32 destination, coax_v_f32 scratch) {
    bool overlap = psionic_f32_spans_overlap(destination.data, destination.count, scratch.data, scratch.count) ||
        psionic_f32_spans_overlap(destination.data, destination.count, a.data, a.count) ||
        psionic_f32_spans_overlap(destination.data, destination.count, b.data, b.count) ||
        psionic_f32_spans_overlap(scratch.data, scratch.count, a.data, a.count) ||
        psionic_f32_spans_overlap(scratch.data, scratch.count, b.data, b.count) ||
        psionic_spans_overlap(destination.data, destination.count, sizeof(float), indices.data, indices.count, sizeof(int64_t)) ||
        psionic_spans_overlap(scratch.data, scratch.count, sizeof(float), indices.data, indices.count, sizeof(int64_t)) ||
        psionic_spans_overlap(destination.data, destination.count, sizeof(float), mask.data, mask.count, sizeof(uint8_t)) ||
        psionic_spans_overlap(scratch.data, scratch.count, sizeof(float), mask.data, mask.count, sizeof(uint8_t));
    return psionic_ops_checked(request, a, b, indices, mask, destination, scratch, overlap);
}
