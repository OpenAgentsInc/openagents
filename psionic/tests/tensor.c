/* Independent shape/offset/bit oracles and real storage; verification tooling. */
#include <assert.h>
#include <float.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "tensor-memory.h"

_Static_assert(sizeof(float) == 4 && FLT_RADIX == 2 && FLT_MANT_DIG == 24, "binary32 target");
enum { OK, RANK, DIMENSION, LIMIT, CAPACITY, ALIAS, NUMBER, INDEX };
static coax_t_PsionicShape shape(int64_t rank, int64_t a, int64_t b, int64_t c, int64_t d) {
    return (coax_t_PsionicShape){rank, {{a, b, c, d}}};
}
static coax_v_f32 view(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static void fill(float *p, size_t n, float v) { for (size_t i = 0; i < n; ++i) p[i] = v; }
static void same(const float *p, size_t n, float v) { for (size_t i = 0; i < n; ++i) assert(p[i] == v); }
static uint32_t bits(float v) { uint32_t b; memcpy(&b, &v, sizeof b); return b; }
static void set_bits(float *p, uint32_t b) { memcpy(p, &b, sizeof b); }
static void layout_failed(coax_t_PsionicShape s, int expected) {
    coax_t_PsionicLayout r = psionic_tensor_layout(s);
    assert(r.status == expected && r.elements == 0 && r.bytes == 0);
    for (int a = 0; a < 4; ++a) assert(r.strides.e[a] == 0);
    coax_t_PsionicIndex i = psionic_tensor_index(s, (coax_a4_s64){{0, 0, 0, 0}});
    assert(i.status == expected && i.offset == 0);
    coax_t_PsionicCopy c = psionic_tensor_copy(s, view(NULL, 0), view(NULL, 0), view(NULL, 0));
    assert(c.status == expected && c.written == 0);
}
static void failed(coax_t_PsionicCopy r, int expected) { assert(r.status == expected && r.written == 0); }

static void layouts_and_indices(void) {
    int layouts = 0, indices = 0;
    for (int rank = 0, combinations = 1; rank <= 4; ++rank, combinations *= 3) {
        for (int code = 0; code < combinations; ++code) {
            coax_t_PsionicShape s = {0}; s.rank = rank;
            int digits = code, count = 1;
            for (int a = 0; a < rank; ++a) { s.dims.e[a] = 1 + digits % 3; digits /= 3; count *= (int)s.dims.e[a]; }
            coax_t_PsionicLayout r = psionic_tensor_layout(s);
            assert(r.status == OK && r.elements == count && r.bytes == count * 4);
            int suffix[4] = {0};
            for (int a = 0; a < rank; ++a) {
                suffix[a] = 1;
                for (int b = a + 1; b < rank; ++b) suffix[a] *= (int)s.dims.e[b];
            }
            for (int a = 0; a < 4; ++a) assert(r.strides.e[a] == suffix[a]);
            for (int flat = 0; flat < count; ++flat) {
                coax_a4_s64 coords = {{0}};
                for (int a = 0; a < rank; ++a) coords.e[a] = flat / suffix[a] % s.dims.e[a];
                coax_t_PsionicIndex i = psionic_tensor_index(s, coords);
                assert(i.status == OK && i.offset == flat); ++indices;
            }
            for (int a = 0; a < 4; ++a) {
                int64_t bad[] = {-1, INT64_MIN, a < rank ? s.dims.e[a] : 1, INT64_MAX};
                for (size_t k = 0; k < sizeof bad / sizeof bad[0]; ++k) {
                    coax_a4_s64 coords = {{0}}; coords.e[a] = bad[k];
                    coax_t_PsionicIndex i = psionic_tensor_index(s, coords);
                    assert(i.status == INDEX && i.offset == 0);
                }
            }
            /* The copy oracle is the original bit sequence, not another copy kernel. */
            float source[84], original[84], destination[86], scratch[86];
            for (int a = 0; a < 84; ++a) source[a] = (float)(a * 2 - 63);
            memcpy(original, source, sizeof source);
            fill(destination, 86, -7); fill(scratch, 86, -9);
            coax_t_PsionicCopy c = psionic_tensor_copy(s, view(source, count), view(destination + 1, count + 3), view(scratch + 1, count + 3));
            assert(c.status == OK && c.written == count);
            assert(!memcmp(destination + 1, original, (size_t)count * 4));
            assert(!memcmp(scratch + 1, original, (size_t)count * 4));
            assert(!memcmp(source, original, sizeof source));
            assert(destination[0] == -7 && scratch[0] == -9);
            same(destination + count + 1, (size_t)(85 - count), -7);
            same(scratch + count + 1, (size_t)(85 - count), -9);
            ++layouts;
        }
    }
    assert(layouts == 121 && indices == 1555);
    puts("121 independent layouts/copies and 1555 row-major indices pass");
}
static void metadata_boundaries(void) {
    int64_t ranks[] = {INT64_MIN, -1, 5, INT64_MAX};
    for (size_t k = 0; k < sizeof ranks / sizeof ranks[0]; ++k) layout_failed(shape(ranks[k], 0, 0, 0, 0), RANK);
    for (int rank = 1; rank <= 4; ++rank) {
        for (int a = 0; a < rank; ++a) {
            int64_t bad[] = {0, -1, INT64_MIN};
            for (size_t k = 0; k < sizeof bad / sizeof bad[0]; ++k) {
                coax_t_PsionicShape s = {0}; s.rank = rank;
                for (int b = 0; b < rank; ++b) s.dims.e[b] = 1;
                s.dims.e[a] = bad[k]; layout_failed(s, DIMENSION);
            }
        }
    }
    for (int rank = 0; rank < 4; ++rank) {
        for (int a = rank; a < 4; ++a) {
            coax_t_PsionicShape s = {0}; s.rank = rank;
            for (int b = 0; b < rank; ++b) s.dims.e[b] = 1;
            s.dims.e[a] = 1; layout_failed(s, DIMENSION);
            s.dims.e[a] = -1; layout_failed(s, DIMENSION);
        }
    }
    for (int a = 0; a < 4; ++a) {
        coax_t_PsionicShape s = shape(4, 1, 1, 1, 1);
        s.dims.e[a] = 1048576;
        coax_t_PsionicLayout r = psionic_tensor_layout(s);
        assert(r.status == OK && r.elements == 1048576 && r.bytes == 4194304);
        for (int b = 0; b < 4; ++b) assert(r.strides.e[b] == (b < a ? 1048576 : 1));
        s.dims.e[a] = 1048577; layout_failed(s, LIMIT);
        s.dims.e[a] = INT64_MAX; layout_failed(s, LIMIT);
    }
    coax_t_PsionicLayout square = psionic_tensor_layout(shape(2, 1024, 1024, 0, 0));
    assert(square.status == OK && square.elements == 1048576 && square.bytes == 4194304);
    layout_failed(shape(2, 1024, 1025, 0, 0), LIMIT);
    layout_failed(shape(2, INT64_MAX, INT64_MAX, 0, 0), LIMIT);
    layout_failed(shape(4, 1, 1, 0, INT64_MAX), DIMENSION);
    layout_failed(shape(4, INT64_MAX, 1, 1, 0), DIMENSION);
    coax_t_PsionicIndex last = psionic_tensor_index(shape(2, 1024, 1024, 0, 0), (coax_a4_s64){{1023, 1023, 0, 0}});
    assert(last.status == OK && last.offset == 1048575);
}
static void capacities_and_representations(void) {
    coax_t_PsionicShape s = shape(2, 2, 3, 0, 0);
    float source[9] = {10, 20, 30, 40, 50, 60}, original[9], destination[9], scratch[9];
    memcpy(original, source, sizeof source);
    for (int in = 0; in <= 8; ++in) for (int out = 0; out <= 8; ++out) for (int work = 0; work <= 8; ++work) {
        fill(destination, 9, -7); fill(scratch, 9, -9);
        coax_t_PsionicCopy r = psionic_tensor_copy(s, view(source, in), view(destination, out), view(scratch, work));
        if (in == 6 && out >= 6 && work >= 6) {
            assert(r.status == OK && r.written == 6);
            assert(!memcmp(destination, source, 24) && !memcmp(scratch, source, 24));
            same(destination + 6, 3, -7); same(scratch + 6, 3, -9);
        } else { failed(r, CAPACITY); same(destination, 9, -7); same(scratch, 9, -9); }
        assert(!memcmp(source, original, sizeof source));
    }
    fill(destination, 9, -7); fill(scratch, 9, -9);
    int64_t bad_counts[] = {-1, INT64_MIN, INT64_MAX};
    for (size_t k = 0; k < sizeof bad_counts / sizeof bad_counts[0]; ++k) {
        int64_t n = bad_counts[k];
        failed(psionic_tensor_copy(s, view(source, n), view(destination, 9), view(scratch, 9)), CAPACITY);
        failed(psionic_tensor_copy(s, view(source, 6), view(destination, n), view(scratch, 9)), n < 0 ? CAPACITY : ALIAS);
        failed(psionic_tensor_copy(s, view(source, 6), view(destination, 9), view(scratch, n)), n < 0 ? CAPACITY : ALIAS);
    }
    failed(psionic_tensor_copy(s, view(NULL, 0), view(destination, 9), view(scratch, 9)), CAPACITY);
    failed(psionic_tensor_copy(s, view(NULL, 6), view(destination, 9), view(scratch, 9)), ALIAS);
    failed(psionic_tensor_copy(s, view(source, 6), view(NULL, 9), view(scratch, 9)), ALIAS);
    failed(psionic_tensor_copy(s, view(source, 6), view(destination, 9), view(NULL, 9)), ALIAS);
    /* These are genuinely short allocations, not only short logical views. */
    float *short_buffer = malloc(5 * sizeof(float));
    assert(short_buffer); fill(short_buffer, 5, -11);
    failed(psionic_tensor_copy(s, view(short_buffer, 5), view(destination, 9), view(scratch, 9)), CAPACITY);
    failed(psionic_tensor_copy(s, view(source, 6), view(short_buffer, 5), view(scratch, 9)), CAPACITY);
    failed(psionic_tensor_copy(s, view(source, 6), view(destination, 9), view(short_buffer, 5)), CAPACITY);
    same(short_buffer, 5, -11); free(short_buffer);
    failed(psionic_tensor_copy(s, view((float *)(uintptr_t)(UINTPTR_MAX - 3), 6), view(destination, 9), view(scratch, 9)), ALIAS);
    failed(psionic_tensor_copy(s, view(source, 6), view((float *)(uintptr_t)(UINTPTR_MAX - 3), 9), view(scratch, 9)), ALIAS);
    same(destination, 9, -7); same(scratch, 9, -9);
    uint32_t nonfinite[] = {0x7f800000, 0xff800000, 0x7fc00000, 0xffc00000, 0x7f800001, 0xff800001, 0x7fffffff, 0xffffffff};
    for (size_t k = 0; k < sizeof nonfinite / sizeof nonfinite[0]; ++k) {
        for (int a = 0; a < 6; ++a) {
            set_bits(source + a, nonfinite[k]);
            failed(psionic_tensor_copy(s, view(source, 6), view(destination, 9), view(scratch, 9)), NUMBER);
            assert(bits(source[a]) == nonfinite[k]);
            failed(psionic_tensor_copy(s, view(source, 6), view(destination, 5), view(scratch, 9)), CAPACITY);
            failed(psionic_tensor_copy(s, view(source, 6), view(source, 6), view(scratch, 9)), ALIAS);
            memcpy(source, original, sizeof source);
            same(destination, 9, -7); same(scratch, 9, -9);
        }
    }
    /* Non-finite data outside the logical source view is never read. */
    set_bits(source + 6, 0x7fc00000);
    assert(psionic_tensor_copy(s, view(source, 6), view(destination, 9), view(scratch, 9)).status == OK);
    assert(bits(source[6]) == 0x7fc00000); same(destination + 6, 3, -7); same(scratch + 6, 3, -9);
    uint32_t finite[] = {0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x41800001, 0xc1800001, 0x7f7fffff, 0xff7fffff};
    for (size_t k = 0; k < sizeof finite / sizeof finite[0]; ++k) {
        set_bits(source, finite[k]);
        coax_t_PsionicCopy r = psionic_tensor_copy(shape(0, 0, 0, 0, 0), view(source, 1), view(destination, 1), view(scratch, 1));
        assert(r.status == OK && r.written == 1 && bits(source[0]) == finite[k]);
        assert(bits(destination[0]) == finite[k] && bits(scratch[0]) == finite[k]);
    }
}
static void aliases(void) {
    float arena[40], before[40];
    for (int kind = 0; kind < 9; ++kind) {
        for (int a = 0; a < 40; ++a) arena[a] = (float)(a + 1);
        memcpy(before, arena, sizeof arena);
        coax_v_f32 in = view(arena + 8, 6), out = view(arena + 16, 8), scratch = view(arena + 28, 8);
        if (kind == 0) out = in;
        if (kind == 1) out.data = in.data + 1;
        if (kind == 2) out.data = in.data - 1;
        if (kind == 3) scratch.data = in.data;
        if (kind == 4) scratch.data = in.data + 5;
        if (kind == 5) scratch.data = out.data;
        if (kind == 6) scratch.data = out.data + 7;
        if (kind == 7) out = view(arena + 1, 8); // Only the declared output tail overlaps input.
        if (kind == 8) scratch = view(arena + 1, 8);
        failed(psionic_tensor_copy(shape(2, 2, 3, 0, 0), in, out, scratch), ALIAS);
        assert(!memcmp(arena, before, sizeof arena));
    }
    fill(arena, 40, -7);
    for (int a = 0; a < 6; ++a) arena[a] = (float)(a + 1);
    assert(psionic_tensor_copy(shape(2, 2, 3, 0, 0), view(arena, 6), view(arena + 6, 8), view(arena + 14, 8)).status == OK);
    for (int a = 0; a < 6; ++a) assert(arena[a] == a + 1 && arena[a + 6] == a + 1 && arena[a + 14] == a + 1);
    same(arena + 12, 2, -7); same(arena + 20, 20, -7);
}
static void maximum_copy(void) {
    const size_t n = 1048576;
    float *source = malloc(n * 4), *destination = malloc((n + 1) * 4), *scratch = malloc((n + 1) * 4);
    assert(source && destination && scratch);
    for (size_t a = 0; a < n; ++a) source[a] = (float)((int)(a % 127) - 63);
    fill(destination, n + 1, -7); fill(scratch, n + 1, -9);
    coax_t_PsionicCopy r = psionic_tensor_copy(shape(2, 1024, 1024, 0, 0), view(source, (int64_t)n), view(destination, (int64_t)n + 1), view(scratch, (int64_t)n + 1));
    assert(r.status == OK && r.written == (int64_t)n);
    assert(!memcmp(source, destination, n * 4) && !memcmp(source, scratch, n * 4));
    for (size_t a = 0; a < n; ++a) assert(source[a] == (float)((int)(a % 127) - 63));
    assert(destination[n] == -7 && scratch[n] == -9);
    free(source); free(destination); free(scratch);
    puts("1048576-element copy, representation, refusal, alias and publication checks pass");
}
int main(void) {
    layouts_and_indices(); metadata_boundaries(); capacities_and_representations(); aliases(); maximum_copy();
    return 0;
}
