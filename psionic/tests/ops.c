/* Independent Decimal fixtures, analytic boundaries and actual native storage. */
#include <assert.h>
#include <fenv.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "ops-memory.h"

_Static_assert(FLT_RADIX == 2 && FLT_MANT_DIG == 24 && DBL_MANT_DIG == 53 && DBL_MAX_EXP == 1024,
    "CPU-f32-wide-v1 requires binary32/binary64");

enum { ADD, SUM, DOT, MATMUL, GATHER, SOFTMAX, RMSNORM, LAYERNORM };
enum { OK, OPERATION, SHAPE, LIMIT, PARAMETER, CAPACITY, ALIAS, NUMBER, INDEX, MASK, ARITHMETIC };
static coax_v_f32 fv(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static coax_v_s64 iv(int64_t *p, int64_t n) { return (coax_v_s64){p, n}; }
static coax_v_u8 mv(uint8_t *p, int64_t n) { return (coax_v_u8){p, n}; }
#define NF fv(NULL, 0)
#define NI iv(NULL, 0)
#define NM mv(NULL, 0)
static coax_t_PsionicRequest request(int op, int64_t rows, int64_t width, int64_t columns, float epsilon) {
    return (coax_t_PsionicRequest){(uint8_t)op, rows, width, columns, epsilon};
}
static void fill(float *p, size_t n, float v) { for (size_t i = 0; i < n; ++i) p[i] = v; }
static void same(const float *p, size_t n, float v) { for (size_t i = 0; i < n; ++i) assert(p[i] == v); }
static uint32_t bits(float v) { uint32_t b; memcpy(&b, &v, sizeof b); return b; }
static float from_bits(uint32_t b) { float v; memcpy(&v, &b, sizeof v); return v; }
static void failed(coax_t_PsionicResult r, int expected) { assert(r.status == expected && r.written == 0); }
static void near(float got, long double want, const char *name, int index) {
    /* Selected before execution: <=2^-23 absolute + 2^-21 relative.
       These Decimal fixtures have |input|<=4 except shifted softmax logits,
       widths<=7, and epsilon is the exact f32 encoding of 1e-5. */
    long double bound = 0x1p-23L + 0x1p-21L * fabsl(want);
    if (!isfinite(got) || fabsl((long double)got - want) > bound) {
        fprintf(stderr, "%s[%d]: %.9g != %.21Lg (bound %.9Lg)\n", name, index, got, want, bound);
        abort();
    }
}
struct Oracle {
    const char *name;
    int op, rows, width, columns;
    uint32_t epsilon;
    int na, nb, ni, nm, no;
    uint32_t a[64], b[64];
    int64_t indices[64];
    uint8_t mask[64];
    long double expected[64];
};
#include "ops-oracle.h"

static void decimal_oracles(void) {
    size_t n = sizeof oracle_cases / sizeof oracle_cases[0]; assert(n == 120);
    for (size_t c = 0; c < n; ++c) {
        const struct Oracle *o = oracle_cases + c;
        float a[64] = {0}, b[64] = {0}, old_a[64], old_b[64], out[68], scratch[68];
        int64_t indices[64]; uint8_t mask[64];
        for (int i = 0; i < o->na; ++i) a[i] = from_bits(o->a[i]);
        for (int i = 0; i < o->nb; ++i) b[i] = from_bits(o->b[i]);
        memcpy(old_a, a, sizeof a); memcpy(old_b, b, sizeof b);
        memcpy(indices, o->indices, sizeof indices); memcpy(mask, o->mask, sizeof mask);
        fill(out, 68, -7); fill(scratch, 68, -9);
        coax_t_PsionicResult r = psionic_ops_run(request(o->op, o->rows, o->width, o->columns, from_bits(o->epsilon)),
            fv(a, o->na), fv(b, o->nb), iv(indices, o->ni), mv(mask, o->nm), fv(out + 1, o->no + 2), fv(scratch + 1, o->no + 2));
        assert(r.status == OK && r.written == o->no);
        for (int i = 0; i < o->no; ++i) {
            near(out[i + 1], o->expected[i], o->name, i);
            assert(bits(out[i + 1]) == bits(scratch[i + 1]));
            /* Dyadic inputs and these small dimensions make the first four
               operations' reference results exactly representable in f32. */
            if (o->op <= MATMUL) assert(bits(out[i + 1]) == bits((float)o->expected[i]));
            if (o->op == SOFTMAX && !o->mask[i]) assert(bits(out[i + 1]) == 0);
            if (o->op == GATHER) assert(bits(out[i + 1]) == o->a[o->indices[i / o->width] * o->width + i % o->width]);
        }
        assert(out[0] == -7 && scratch[0] == -9);
        same(out + o->no + 1, (size_t)(67 - o->no), -7); same(scratch + o->no + 1, (size_t)(67 - o->no), -9);
        assert(!memcmp(a, old_a, sizeof a) && !memcmp(b, old_b, sizeof b));
        assert(!memcmp(indices, o->indices, sizeof indices) && !memcmp(mask, o->mask, sizeof mask));
    }
    puts("120 independent Decimal(120) cases cover all eight operations");
}
static void analytic_and_magnitudes(void) {
    float a[4] = {1e20f, 1, -1e20f, 0}, b[4] = {1, 1, 1, 0}, out[4], scratch[4];
    fill(out, 4, -7); fill(scratch, 4, -9);
    assert(psionic_ops_run(request(SUM, 1, 3, 0, 0), fv(a, 3), NF, NI, NM, fv(out, 4), fv(scratch, 4)).status == OK && out[0] == 1);
    assert(psionic_ops_run(request(DOT, 1, 3, 0, 0), fv(a, 3), fv(b, 3), NI, NM, fv(out, 4), fv(scratch, 4)).status == OK && out[0] == 1);
    a[0] = a[1] = from_bits(0x7f7fffff); b[0] = a[0]; b[1] = -a[0];
    assert(psionic_ops_run(request(DOT, 1, 2, 0, 0), fv(a, 2), fv(b, 2), NI, NM, fv(out, 4), fv(scratch, 4)).status == OK && out[0] == 0);
    a[0] = from_bits(1); b[0] = from_bits(1);
    assert(psionic_ops_run(request(DOT, 1, 1, 0, 0), fv(a, 1), fv(b, 1), NI, NM, fv(out, 4), fv(scratch, 4)).status == OK && out[0] == 0);
    b[0] = 1; a[0] = from_bits(0x7f7fffff);
    assert(psionic_ops_run(request(DOT, 1, 1, 0, 0), fv(a, 1), fv(b, 1), NI, NM, fv(out, 4), fv(scratch, 4)).status == OK && bits(out[0]) == 0x7f7fffff);
    a[0] = from_bits(0xff7fffff); a[1] = from_bits(0x7f7fffff);
    uint8_t mask[2] = {1, 1};
    assert(psionic_ops_run(request(SOFTMAX, 1, 2, 0, 0), fv(a, 2), NF, NI, mv(mask, 2), fv(out, 4), fv(scratch, 4)).status == OK);
    assert(out[0] == 0 && out[1] == 1);
    for (int op = RMSNORM; op <= LAYERNORM; ++op) {
        assert(psionic_ops_run(request(op, 1, 2, 0, from_bits(1)), fv(a, 2), NF, NI, NM, fv(out, 4), fv(scratch, 4)).status == OK);
        near(out[0], -1, "extreme norm", 0); near(out[1], 1, "extreme norm", 1);
    }
    a[0] = a[1];
    assert(psionic_ops_run(request(LAYERNORM, 1, 2, 0, from_bits(1)), fv(a, 2), NF, NI, NM, fv(out, 4), fv(scratch, 4)).status == OK);
    assert(out[0] == 0 && out[1] == 0);
    a[0] = from_bits(0x80000000); a[1] = from_bits(1); a[2] = from_bits(0x7f7fffff);
    int64_t ids[3] = {2, 0, 1};
    assert(psionic_ops_run(request(GATHER, 3, 1, 3, 0), fv(a, 3), NF, iv(ids, 3), NM, fv(out, 4), fv(scratch, 4)).status == OK);
    assert(bits(out[0]) == 0x7f7fffff && bits(out[1]) == 0x80000000 && bits(out[2]) == 1);
}
static void arithmetic_publication(void) {
    float a[2] = {1, from_bits(0x7f7fffff)}, b[2] = {2, from_bits(0x7f7fffff)}, out[3], scratch[3];
    fill(out, 3, -7); fill(scratch, 3, -9);
    failed(psionic_ops_run(request(ADD, 1, 2, 0, 0), fv(a, 2), fv(b, 2), NI, NM, fv(out, 3), fv(scratch, 3)), ARITHMETIC);
    same(out, 3, -7); assert(scratch[0] == 3 && scratch[1] == -9 && scratch[2] == -9);
    fill(scratch, 3, -9); a[0] = a[1];
    failed(psionic_ops_run(request(SUM, 1, 2, 0, 0), fv(a, 2), NF, NI, NM, fv(out, 3), fv(scratch, 3)), ARITHMETIC);
    same(out, 3, -7); same(scratch, 3, -9);
    b[0] = b[1] = -1;
    failed(psionic_ops_run(request(DOT, 1, 2, 0, 0), fv(a, 2), fv(b, 2), NI, NM, fv(out, 3), fv(scratch, 3)), ARITHMETIC);
    same(out, 3, -7); same(scratch, 3, -9);
    a[0] = 2; b[0] = 1; b[1] = from_bits(0x7f7fffff);
    failed(psionic_ops_run(request(MATMUL, 1, 1, 2, 0), fv(a, 1), fv(b, 2), NI, NM, fv(out, 3), fv(scratch, 3)), ARITHMETIC);
    same(out, 3, -7); assert(scratch[0] == 2 && scratch[1] == -9 && scratch[2] == -9);
}
static void refusal_and_precedence(void) {
    float a[6] = {1, 2, 3, 4, 5, 6}, b[6] = {6, 5, 4, 3, 2, 1}, out[8], scratch[8];
    fill(out, 8, -7); fill(scratch, 8, -9);
    coax_t_PsionicRequest req = request(ADD, 2, 3, 0, 0);
    failed(psionic_ops_run(request(255, 0, 0, 0, 0), NF, NF, NI, NM, NF, NF), OPERATION);
    failed(psionic_ops_run(request(8, 1, 1, 0, 0), NF, NF, NI, NM, NF, NF), OPERATION);
    failed(psionic_ops_run(request(ADD, INT64_MIN, 1, 0, 0), NF, NF, NI, NM, NF, NF), SHAPE);
    failed(psionic_ops_run(request(ADD, INT64_MAX, 2, 0, 0), NF, NF, NI, NM, NF, NF), LIMIT);
    failed(psionic_ops_run(request(MATMUL, 129, 64, 128, 0), NF, NF, NI, NM, NF, NF), LIMIT);
    failed(psionic_ops_run(request(ADD, 1, 1, 1, 0), NF, NF, NI, NM, NF, NF), PARAMETER);
    uint32_t bad_eps[] = {0, 0x80000000, 0xbf800000, 0x7f800000, 0x7fc00000, 0xff800001};
    for (size_t k = 0; k < sizeof bad_eps / sizeof bad_eps[0]; ++k)
        failed(psionic_ops_run(request(RMSNORM, 2, 3, 0, from_bits(bad_eps[k])), fv(a, 6), NF, NI, NM, fv(out, 8), fv(scratch, 8)), PARAMETER);
    for (int which = 0; which < 6; ++which) {
        int64_t lengths[6] = {6, 6, 0, 0, 8, 8}; lengths[which] = -1;
        failed(psionic_ops_run(req, fv(a, lengths[0]), fv(b, lengths[1]), iv(NULL, lengths[2]), mv(NULL, lengths[3]), fv(out, lengths[4]), fv(scratch, lengths[5])), CAPACITY);
    }
    failed(psionic_ops_run(req, fv(a, INT64_MAX), fv(b, 6), NI, NM, fv(out, 8), fv(scratch, 8)), CAPACITY);
    failed(psionic_ops_run(req, fv(a, 6), fv(b, 6), NI, NM, fv(out, INT64_MAX), fv(scratch, 8)), ALIAS);
    failed(psionic_ops_run(req, fv(NULL, 6), fv(b, 6), NI, NM, fv(out, 8), fv(scratch, 8)), ALIAS);
    float *short_buffer = malloc(5 * sizeof(float)); assert(short_buffer); fill(short_buffer, 5, -11);
    failed(psionic_ops_run(req, fv(short_buffer, 5), fv(b, 6), NI, NM, fv(out, 8), fv(scratch, 8)), CAPACITY);
    failed(psionic_ops_run(req, fv(a, 6), fv(short_buffer, 5), NI, NM, fv(out, 8), fv(scratch, 8)), CAPACITY);
    failed(psionic_ops_run(req, fv(a, 6), fv(b, 6), NI, NM, fv(short_buffer, 5), fv(scratch, 8)), CAPACITY);
    failed(psionic_ops_run(req, fv(a, 6), fv(b, 6), NI, NM, fv(out, 8), fv(short_buffer, 5)), CAPACITY);
    same(short_buffer, 5, -11); free(short_buffer);
    uint32_t bad[] = {0x7f800000, 0xff800000, 0x7fc00000, 0xffc00000, 0x7f800001, 0xff800001};
    for (int operand = 0; operand < 2; ++operand) for (int position = 0; position < 6; ++position) {
        float *p = operand ? b : a; float old = p[position];
        for (size_t k = 0; k < sizeof bad / sizeof bad[0]; ++k) {
            p[position] = from_bits(bad[k]);
            failed(psionic_ops_run(req, fv(a, 6), fv(b, 6), NI, NM, fv(out, 8), fv(scratch, 8)), NUMBER);
            failed(psionic_ops_run(req, fv(a, 6), fv(b, 6), NI, NM, fv(a, 6), fv(scratch, 8)), ALIAS);
            failed(psionic_ops_run(req, fv(a, 5), fv(b, 6), NI, NM, fv(out, 8), fv(scratch, 8)), CAPACITY);
            assert(bits(p[position]) == bad[k]); same(out, 8, -7); same(scratch, 8, -9);
        }
        p[position] = old;
    }
    int64_t indices[3] = {2, 0, 2}; uint8_t mask[6] = {1, 1, 1, 1, 1, 1};
    failed(psionic_ops_run(request(GATHER, 3, 2, 3, 0), fv(a, 6), NF, iv(NULL, 3), NM, fv(out, 8), fv(scratch, 8)), ALIAS);
    failed(psionic_ops_run(request(SOFTMAX, 2, 3, 0, 0), fv(a, 6), NF, NI, mv(NULL, 6), fv(out, 8), fv(scratch, 8)), ALIAS);
    int64_t *short_indices = malloc(2 * sizeof(int64_t)); uint8_t *short_mask = malloc(5);
    assert(short_indices && short_mask); short_indices[0] = 0; short_indices[1] = 1; memset(short_mask, 1, 5);
    failed(psionic_ops_run(request(GATHER, 3, 2, 3, 0), fv(a, 6), NF, iv(short_indices, 2), NM, fv(out, 8), fv(scratch, 8)), CAPACITY);
    failed(psionic_ops_run(request(SOFTMAX, 2, 3, 0, 0), fv(a, 6), NF, NI, mv(short_mask, 5), fv(out, 8), fv(scratch, 8)), CAPACITY);
    assert(short_indices[0] == 0 && short_indices[1] == 1); for (int i = 0; i < 5; ++i) assert(short_mask[i] == 1);
    free(short_indices); free(short_mask); same(out, 8, -7); same(scratch, 8, -9);
    for (int position = 0; position < 3; ++position) {
        int64_t invalid[] = {-1, 3, INT64_MIN, INT64_MAX}; int64_t old = indices[position];
        for (size_t k = 0; k < sizeof invalid / sizeof invalid[0]; ++k) {
            indices[position] = invalid[k];
            failed(psionic_ops_run(request(GATHER, 3, 2, 3, 0), fv(a, 6), NF, iv(indices, 3), NM, fv(out, 8), fv(scratch, 8)), INDEX);
            same(out, 8, -7); same(scratch, 8, -9);
        }
        indices[position] = old;
    }
    for (int position = 0; position < 6; ++position) {
        mask[position] = 2;
        failed(psionic_ops_run(request(SOFTMAX, 2, 3, 0, 0), fv(a, 6), NF, NI, mv(mask, 6), fv(out, 8), fv(scratch, 8)), MASK);
        mask[position] = 1; same(out, 8, -7); same(scratch, 8, -9);
    }
    for (int row = 0; row < 2; ++row) {
        memset(mask + row * 3, 0, 3);
        failed(psionic_ops_run(request(SOFTMAX, 2, 3, 0, 0), fv(a, 6), NF, NI, mv(mask, 6), fv(out, 8), fv(scratch, 8)), MASK);
        memset(mask + row * 3, 1, 3); same(out, 8, -7); same(scratch, 8, -9);
    }
    mask[0] = 0; a[0] = from_bits(0x7fc00000);
    failed(psionic_ops_run(request(SOFTMAX, 2, 3, 0, 0), fv(a, 6), NF, NI, mv(mask, 6), fv(out, 8), fv(scratch, 8)), NUMBER);
    same(out, 8, -7); same(scratch, 8, -9);
}
static void aliases(void) {
    float arena[128], before[128];
    for (int kind = 0; kind < 9; ++kind) {
        for (int i = 0; i < 128; ++i) arena[i] = (float)(i + 1);
        memcpy(before, arena, sizeof arena);
        coax_v_f32 a = fv(arena + 32, 2), b = fv(arena + 64, 2), out = fv(arena + 96, 8), work = fv(arena + 112, 8);
        if (kind == 0) out.data = a.data;
        if (kind == 1) out.data = a.data + 1;
        if (kind == 2) out.data = a.data - 1;
        if (kind == 3) out.data = b.data;
        if (kind == 4) work.data = a.data;
        if (kind == 5) work.data = b.data;
        if (kind == 6) work.data = out.data;
        if (kind == 7) work.data = out.data + 7;
        if (kind == 8) out.data = a.data - 7;
        failed(psionic_ops_run(request(ADD, 1, 2, 0, 0), a, b, NI, NM, out, work), ALIAS);
        assert(!memcmp(arena, before, sizeof arena));
    }
    float a[3] = {1, 2, 3}, out[8], scratch[8]; fill(out, 8, -7); fill(scratch, 8, -9);
    for (int which = 0; which < 2; ++which) {
        float *target = which ? scratch : out;
        failed(psionic_ops_run(request(GATHER, 3, 1, 2, 0), fv(a, 3), NF, iv((int64_t *)(void *)(target + 2), 2), NM, fv(out, 8), fv(scratch, 8)), ALIAS);
        failed(psionic_ops_run(request(SOFTMAX, 1, 3, 0, 0), fv(a, 3), NF, NI, mv((uint8_t *)(void *)(target + 4), 3), fv(out, 8), fv(scratch, 8)), ALIAS);
        same(out, 8, -7); same(scratch, 8, -9);
    }
    assert(psionic_ops_run(request(ADD, 1, 2, 0, 0), fv(a, 2), fv(a, 2), NI, NM, fv(out, 8), fv(scratch, 8)).status == OK);
    assert(out[0] == 2 && out[1] == 4);
    assert(psionic_ops_run(request(ADD, 1, 2, 0, 0), fv(a, 2), fv(a + 1, 2), NI, NM, fv(out, 8), fv(scratch, 8)).status == OK);
    assert(out[0] == 3 && out[1] == 5);
    float adjacent[8] = {1, 2, 3, 4, -7, -7, -9, -9};
    assert(psionic_ops_run(request(ADD, 1, 2, 0, 0), fv(adjacent, 2), fv(adjacent + 2, 2), NI, NM, fv(adjacent + 4, 2), fv(adjacent + 6, 2)).status == OK);
    assert(adjacent[4] == 4 && adjacent[5] == 6);
}
static void maximum_work(void) {
    float *a = malloc(8192 * 4), *b = malloc(8192 * 4), *out = malloc(16385 * 4), *scratch = malloc(16385 * 4);
    assert(a && b && out && scratch); fill(a, 8192, 1); fill(b, 8192, 1); fill(out, 16385, -7); fill(scratch, 16385, -9);
    coax_t_PsionicResult r = psionic_ops_run(request(MATMUL, 128, 64, 128, 0), fv(a, 8192), fv(b, 8192), NI, NM, fv(out, 16385), fv(scratch, 16385));
    assert(r.status == OK && r.written == 16384); same(out, 16384, 64); same(scratch, 16384, 64);
    same(a, 8192, 1); same(b, 8192, 1); assert(out[16384] == -7 && scratch[16384] == -9);
    free(a); free(b); free(out); free(scratch);
    puts("1048576 matrix multiply-adds, magnitude, mask, alias and publication checks pass");
}
static void typed_range_boundaries(void) {
    float a[3] = {1, 2, 3}, scratch[8]; fill(scratch, 8, -9);
    unsigned char *arena = malloc(64), before[64]; assert(arena); memset(arena, 0, 64);
    int64_t *indices = (int64_t *)(void *)arena;
    indices[0] = 2; indices[1] = 0; indices[2] = 1;
    memcpy(before, arena, 64);
    /* Only the latter half of the full 24-byte index view overlaps output.
       Incorrectly measuring indices as 4-byte floats would miss this. */
    failed(psionic_ops_run(request(GATHER, 3, 1, 3, 0), fv(a, 3), NF, iv(indices, 3), NM,
        fv((float *)(void *)(arena + 16), 8), fv(scratch, 8)), ALIAS);
    assert(!memcmp(arena, before, 64)); same(scratch, 8, -9);
    /* A three-byte mask followed by aligned float storage is disjoint.
       Incorrectly measuring mask bytes as floats would reject it. */
    memset(arena, 0, 64); arena[0] = arena[1] = arena[2] = 1;
    float *out = (float *)(void *)(arena + 4); fill(out, 8, -7);
    assert(psionic_ops_run(request(SOFTMAX, 1, 3, 0, 0), fv(a, 3), NF, NI, mv(arena, 3), fv(out, 8), fv(scratch, 8)).status == OK);
    assert(arena[0] == 1 && arena[1] == 1 && arena[2] == 1 && arena[3] == 0);
    near(out[0] + out[1] + out[2], 1, "adjacent typed spans", 0);
    same(out + 3, 5, -7); same(scratch + 3, 5, -9); free(arena);
}
int main(void) {
    assert(fegetround() == FE_TONEAREST);
    decimal_oracles(); analytic_and_magnitudes(); arithmetic_publication(); refusal_and_precedence(); aliases(); typed_range_boundaries(); maximum_work();
    return 0;
}
