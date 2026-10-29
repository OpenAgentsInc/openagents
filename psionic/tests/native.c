/* Independent numerical oracles and real caller-storage tests; tooling only. */
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "memory.h"

#define VIEW(a) ((coax_v_f32){(a), (int64_t)(sizeof(a) / sizeof((a)[0]))})
enum { OK = 0, SHAPE = 1, STEPS = 2, CAPACITY = 3, ALIAS = 4, NUMBER = 5, UPDATE = 6 };
static void near(float got, long double want, long double atol, long double rtol) {
    assert(isfinite(got));
    assert(fabsl((long double)got - want) <= atol + rtol * fabsl(want));
}
static void fill(float *p, size_t n, float v) { for (size_t i = 0; i < n; ++i) p[i] = v; }
static void same(float *p, size_t n, float v) { for (size_t i = 0; i < n; ++i) assert(p[i] == v); }
static void failed(coax_t_PsionicTraining r, int status) {
    assert(r.status == status && r.weight == 0 && r.loss == 0 && r.written == 0);
}
static float from_bits(uint32_t bits) { float out; memcpy(&out, &bits, sizeof out); return out; }

static void exact_and_convergence(void) {
    float x[] = {1, 2}, y[] = {2, 4}, out[67], scratch[67];
    fill(out, 67, -7); fill(scratch, 67, -9);
    coax_t_PsionicTraining first = psionic_train(VIEW(x), VIEW(y), 0, 1, VIEW(out), VIEW(scratch));
    assert(first.status == OK && first.weight == 1.25f && first.loss == 1.40625f && first.written == 2);
    assert(out[0] == 10 && out[1] == 1.40625f);
    same(out + 2, 65, -7); same(scratch + 2, 65, -9);
    for (int steps = 1; steps <= 64; ++steps) {
        fill(out, 67, -7); fill(scratch, 67, -9);
        coax_t_PsionicTraining r = psionic_train(VIEW(x), VIEW(y), 0, steps, VIEW(out), VIEW(scratch));
        assert(r.status == OK && r.written == steps + 1);
        /* Closed form from the derivative, independent of the implementation's loop. */
        long double error = 2 * powl(3.0L / 8, steps);
        near(r.weight, 2 - error, 0.000001L, 0);
        near(r.loss, 2.5L * error * error, 0.000001L, 0.000001L);
        for (int i = 0; i <= steps; ++i) {
            near(out[i], 10 * powl(9.0L / 64, i), 0.000001L, 0.000001L);
            assert(out[i] == scratch[i]);
            if (i) assert(out[i] <= out[i - 1]);
        }
        same(out + steps + 1, (size_t)(66 - steps), -7);
        same(scratch + steps + 1, (size_t)(66 - steps), -9);
        if (steps == 16) {
            assert(r.loss <= 1e-11f);
            coax_t_PsionicPrediction p = psionic_predict(r.weight, 3);
            assert(p.status == OK); near(p.value, 6, 0.000003L, 0);
            printf("16-step weight %.9g; loss %.9g; held-out prediction %.9g\n", r.weight, r.loss, p.value);
        }
    }
}

static void general_numerics(void) {
    float x[16], y[16], out[65], scratch[65];
    /* Independently use the quadratic's sufficient statistics a,b,c. */
    for (int n = 1; n <= 16; ++n) {
        long double a = 0, b = 0, c = 0;
        for (int i = 0; i < n; ++i) {
            x[i] = (float)(i % 5 - 2); y[i] = (float)((3 * i + n) % 7 - 3);
            a += (long double)x[i] * x[i]; b += (long double)x[i] * y[i]; c += (long double)y[i] * y[i];
        }
        a /= n; b /= n; c /= n;
        for (int initial = -2; initial <= 2; ++initial) {
            for (int steps = 1; steps <= 16; ++steps) {
                long double w = initial;
                coax_t_PsionicTraining r = psionic_train((coax_v_f32){x, n}, (coax_v_f32){y, n},
                    (float)initial, steps, VIEW(out), VIEW(scratch));
                assert(r.status == OK && r.written == steps + 1);
                for (int i = 0; i <= steps; ++i) {
                    near(out[i], a*w*w - 2*b*w + c, 0.000002L, 0.000002L);
                    if (i < steps) w = (1 - a/4)*w + b/4;
                }
                near(r.weight, w, 0.000002L, 0.000002L);
            }
        }
    }
}

static void refusal_and_bits(void) {
    float x[] = {1, 2}, y[] = {2, 4}, out[17], scratch[17];
    fill(out, 17, -7); fill(scratch, 17, -9);
    failed(psionic_train((coax_v_f32){NULL, 0}, VIEW(y), 0, 1, VIEW(out), VIEW(scratch)), SHAPE);
    failed(psionic_train((coax_v_f32){x, -1}, VIEW(y), 0, 1, VIEW(out), VIEW(scratch)), SHAPE);
    failed(psionic_train((coax_v_f32){x, INT64_MAX}, VIEW(y), 0, 1, VIEW(out), VIEW(scratch)), SHAPE);
    failed(psionic_train(VIEW(x), (coax_v_f32){y, 1}, 0, 1, VIEW(out), VIEW(scratch)), SHAPE);
    failed(psionic_train(VIEW(x), VIEW(y), 0, -1, VIEW(out), VIEW(scratch)), STEPS);
    failed(psionic_train(VIEW(x), VIEW(y), 0, 0, VIEW(out), VIEW(scratch)), STEPS);
    failed(psionic_train(VIEW(x), VIEW(y), 0, 65, VIEW(out), VIEW(scratch)), STEPS);
    failed(psionic_train(VIEW(x), VIEW(y), 0, INT64_MAX, VIEW(out), VIEW(scratch)), STEPS);
    failed(psionic_train(VIEW(x), VIEW(y), 0, 16, (coax_v_f32){out, 16}, VIEW(scratch)), CAPACITY);
    failed(psionic_train(VIEW(x), VIEW(y), 0, 16, VIEW(out), (coax_v_f32){scratch, 16}), CAPACITY);
    failed(psionic_train(VIEW(x), VIEW(y), 0, 1, (coax_v_f32){out, -1}, VIEW(scratch)), CAPACITY);
    failed(psionic_train(VIEW(x), VIEW(y), 0, 1, VIEW(out), (coax_v_f32){scratch, -1}), CAPACITY);
    uint32_t bad[] = {0x7f800000, 0xff800000, 0x7fc00000, 0xffc00000,
        0x7f800001, 0xff800001, 0x7f7fffff, 0xff7fffff, 0x41800001, 0xc1800001};
    for (size_t k = 0; k < sizeof bad / sizeof bad[0]; ++k) {
        float value = from_bits(bad[k]);
        failed(psionic_train(VIEW(x), VIEW(y), value, 1, VIEW(out), VIEW(scratch)), NUMBER);
        for (int i = 0; i < 2; ++i) {
            float old = x[i]; x[i] = value;
            failed(psionic_train(VIEW(x), VIEW(y), 0, 1, VIEW(out), VIEW(scratch)), NUMBER); x[i] = old;
            old = y[i]; y[i] = value;
            failed(psionic_train(VIEW(x), VIEW(y), 0, 1, VIEW(out), VIEW(scratch)), NUMBER); y[i] = old;
        }
        coax_t_PsionicPrediction p = psionic_predict(value, 1), q = psionic_predict(1, value);
        assert(p.status == NUMBER && p.value == 0 && q.status == NUMBER && q.value == 0);
    }
    same(out, 17, -7); same(scratch, 17, -9);
    float unstable_x[] = {3}, unstable_y[] = {16};
    failed(psionic_train(VIEW(unstable_x), VIEW(unstable_y), 0, 5, VIEW(out), VIEW(scratch)), UPDATE);
    same(out, 17, -7); assert(scratch[4] == 1525.87890625f && scratch[5] == -9);
    coax_t_PsionicPrediction low = psionic_predict(-16, 16), high = psionic_predict(16, 16);
    assert(low.status == OK && low.value == -256 && high.status == OK && high.value == 256);
    coax_t_PsionicPrediction sub = psionic_predict(from_bits(1), 1), negzero = psionic_predict(from_bits(0x80000000), 1);
    assert(sub.status == OK && sub.value == from_bits(1) && negzero.status == OK && signbit(negzero.value));
}

static void aliases(void) {
    float x[80], y[80], out[80], scratch[80], before[4][80];
    for (int kind = 0; kind < 6; ++kind) {
        fill(x, 80, 1); fill(y, 80, 2); fill(out, 80, -7); fill(scratch, 80, -9);
        memcpy(before[0], x, sizeof x); memcpy(before[1], y, sizeof y);
        memcpy(before[2], out, sizeof out); memcpy(before[3], scratch, sizeof scratch);
        coax_v_f32 ov = {out, 17}, sv = {scratch, 17};
        if (kind == 0) ov.data = x;
        if (kind == 1) ov.data = y;
        if (kind == 2) sv.data = x;
        if (kind == 3) sv.data = y;
        if (kind == 4) sv.data = out;
        if (kind == 5) ov.data = x + 1;
        failed(psionic_train((coax_v_f32){x, 2}, (coax_v_f32){y, 2}, 0, 16, ov, sv), ALIAS);
        assert(!memcmp(before[0], x, sizeof x) && !memcmp(before[1], y, sizeof y));
        assert(!memcmp(before[2], out, sizeof out) && !memcmp(before[3], scratch, sizeof scratch));
    }
    /* Adjacent half-open spans are disjoint. Read-only inputs may alias. */
    float arena[38] = {1, 2, 2, 4};
    coax_v_f32 xv = {arena, 2}, yv = {arena + 2, 2}, ov = {arena + 4, 17}, sv = {arena + 21, 17};
    assert(psionic_train(xv, yv, 0, 16, ov, sv).status == OK);
    coax_t_PsionicTraining r = psionic_train(xv, xv, 0, 16, ov, sv);
    assert(r.status == OK); near(r.weight, 1, 0.000001L, 0);
    assert(arena[0] == 1 && arena[1] == 2 && arena[2] == 2 && arena[3] == 4);
}
int main(void) {
    exact_and_convergence(); general_numerics(); refusal_and_bits(); aliases();
    puts("native oracles, refusals, aliases and publication checks pass");
    return 0;
}
