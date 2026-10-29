/* Shared native address observations. No tensor/training admission policy. */
#ifndef PSIONIC_SPANS_H
#define PSIONIC_SPANS_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

static inline bool psionic_spans_overlap(const void *a_data, int64_t a_count, size_t a_width,
        const void *b_data, int64_t b_count, size_t b_width) {
    /* Invalid/unrepresentable span metadata cannot establish disjoint storage.
       Coax checks shape/capacity metadata before using the observation.
       As with other C view APIs, valid pointers and extents are caller duties. */
    if (a_count < 0 || b_count < 0) return true;
    if (a_count == 0 || b_count == 0) return false;
    if (a_data == NULL || b_data == NULL || a_width == 0 || b_width == 0) return true;
    if ((uint64_t)a_count > UINTPTR_MAX / a_width ||
        (uint64_t)b_count > UINTPTR_MAX / b_width) return true;
    uintptr_t ap = (uintptr_t)a_data, bp = (uintptr_t)b_data;
    uintptr_t an = (uintptr_t)a_count * a_width;
    uintptr_t bn = (uintptr_t)b_count * b_width;
    if (ap > UINTPTR_MAX - an || bp > UINTPTR_MAX - bn) return true;
    return ap < bp + bn && bp < ap + an;
}

static inline bool psionic_f32_spans_overlap(const float *a_data, int64_t a_count,
        const float *b_data, int64_t b_count) {
    return psionic_spans_overlap(a_data, a_count, sizeof(float), b_data, b_count, sizeof(float));
}

#endif
