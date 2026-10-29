/* Chapter 31: the chapter 26 and 30 kernels on an Apple GPU through coax
   metal-compute, against the same Coax kernels compiled to C, the CPU
   reference profile, and (for the float-float f64 profile) binary64.
   Usage: metal-kernels PROJECT.metal WARP.metal FF.metal [quick]
   Prints SKIP and exits 0 without a Metal device. */
#include "metal-host.h"
#include "project.h"
#include "warp.h"
#include "metal-ff.h"
#include "qwen-matvec.h"
#include <dispatch/dispatch.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static uint64_t seed = 0x9e3779b97f4a7c15ULL;
static uint32_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return (uint32_t)(seed >> 16); }
static uint16_t random_bf16(void) {
    uint32_t r = next();
    return (uint16_t)((((r >> 10) & 1) << 15) | ((119 + r % 8) << 7) | ((r >> 3) & 0x7f));
}
static uint32_t wide_bf16(void) { uint32_t r = next(); return ((r >> 10) & 1) << 15 | (110 + r % 24) << 7 | ((r >> 3) & 0x7f); }
static float random_x(void) { return ((float)(next() % 2000001) - 1000000.0f) / 1000000.0f; }
static float bf16(uint32_t h) { uint32_t b = h << 16; float f; memcpy(&f, &b, 4); return f; }
static float half_of(uint32_t word, int high) { uint32_t b = high ? word & 0xffff0000u : word << 16; float f; memcpy(&f, &b, 4); return f; }
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }
#define TRY(x) do { PmStatus s_ = (x); if (s_ != PM_OK) { fprintf(stderr, "%s: status %d: %s\n", #x, s_, pm_log()); exit(1); } } while (0)

static char *read_file(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *text = malloc((size_t)size + 1);
    if (!text || fread(text, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); text[size] = 0; *n = (size_t)size;
    return text;
}
static PmModule compile(const char *path, const char *name) {
    size_t n; char *text = read_file(path, &n);
    if (!text) { fprintf(stderr, "cannot read %s\n", path); exit(2); }
    PmModule m; double seconds;
    TRY(pm_compile(text, n, name, &m, &seconds));
    printf("compiled %s in %.3f s\n", name, seconds);
    free(text);
    return m;
}
static PmArg I(int64_t v) { return (PmArg){PM_ARG_INT, v, 0, NULL, 0, 0}; }
static PmArg V(PmBuffer b, int64_t count) { return (PmArg){PM_ARG_VIEW, 0, 0, b, 0, count}; }
static PmBuffer alloc(size_t bytes) { PmBuffer b; TRY(pm_alloc(bytes, &b)); return b; }

/* ---- Chapter 26's work-item projections ---- */
static PmKernel k_project, k_matvec, k_reduce, k_split, k_mma_project, k_tile, k_math, k_arith;
/* Frozen before measuring, as in chapter 26: the same kernels compiled to C
   agree bit for bit (or the launch is reported), and each output is within
   2^-15 sum |w x| of the CPU reference profile. */
static const double PROJECT_BOUND = 1.0 / 32768.0;
typedef struct { int64_t rows, cols, tokens, slices; const char *what; } Shape;
static void run_project(PmBatch b, Shape s, PmBuffer w, PmBuffer x, PmBuffer out, PmBuffer part, int64_t parts) {
    int64_t groups = s.cols / 4, on = s.tokens * s.rows;
    if (s.slices == 0) {
        PmArg a[] = {I(s.rows), I(s.cols), V(w, s.rows * groups), V(x, s.tokens * s.cols), V(part, parts)};
        TRY(pm_launch(b, k_matvec, s.rows * 16, 1, 256, a, 5));
        PmArg r[] = {I(on), I(16), V(part, parts), V(out, on)};
        TRY(pm_launch(b, k_reduce, on, 1, 256, r, 4));
        return;
    }
    int64_t items = s.rows / 4 * ((s.tokens + 7) / 8) * s.slices;
    PmArg a[] = {I(s.rows), I(s.cols), I(s.tokens), I(s.slices), V(w, s.rows * groups), V(x, s.tokens * s.cols), s.slices == 1 ? V(out, on) : V(part, parts)};
    TRY(pm_launch(b, k_project, items, 1, 128, a, 7));
    if (s.slices > 1) {
        PmArg r[] = {I(on), I(s.slices), V(part, parts), V(out, on)};
        TRY(pm_launch(b, k_reduce, on, 1, 256, r, 4));
    }
}
static int project_shape(Shape s, int timing) {
    int64_t groups = s.cols / 4, wn = s.rows * s.cols, xn = s.tokens * s.cols, on = s.tokens * s.rows;
    int64_t parts = on * (s.slices == 0 ? 16 : s.slices);
    PmBuffer dw = alloc((size_t)wn * 2), dx = alloc((size_t)xn * 4), dout = alloc((size_t)on * 4), dpart = alloc((size_t)parts * 4);
    uint16_t *w = pm_contents(dw);
    float *x = pm_contents(dx), *gpu = pm_contents(dout), *cpu = malloc((size_t)on * 4), *part = malloc((size_t)parts * 4);
    for (int64_t i = 0; i < wn; i++) w[i] = random_bf16();
    for (int64_t i = 0; i < xn; i++) x[i] = random_x();
    memset(gpu, 0xff, (size_t)on * 4);
    PmBatch b;
    TRY(pm_begin(&b)); run_project(b, s, dw, dx, dout, dpart, parts); TRY(pm_wait(b, NULL));
    coax_v_u64 vw = {(uint64_t *)w, s.rows * groups};
    coax_v_f32 vx = {x, xn}, vout = {cpu, on}, vpart = {part, parts};
    if (s.slices == 0) {
        for (int64_t i = 0; i < s.rows * 16; i++) dv_matvec(i, s.rows, s.cols, vw, vx, vpart);
        for (int64_t i = 0; i < on; i++) dv_reduce(i, on, 16, vpart, vout);
    } else {
        int64_t items = s.rows / 4 * ((s.tokens + 7) / 8) * s.slices;
        for (int64_t i = 0; i < items; i++) dv_project(i, s.rows, s.cols, s.tokens, s.slices, vw, vx, s.slices == 1 ? vout : vpart);
        if (s.slices > 1) for (int64_t i = 0; i < on; i++) dv_reduce(i, on, s.slices, vpart, vout);
    }
    int64_t differ = 0;
    for (int64_t i = 0; i < on; i++) if (memcmp(&gpu[i], &cpu[i], 4)) differ++;
    double worst = 0;
    float *ref = malloc((size_t)s.rows * 4), *work = malloc((size_t)s.rows * 4);
    for (int64_t t = 0; t < s.tokens; t += (s.tokens > 4 ? s.tokens / 4 : 1)) {
        for (int64_t r0 = 0; r0 < s.rows; r0 += 32768) {
            int64_t n = s.rows - r0 < 32768 ? s.rows - r0 : 32768;
            coax_t_QmResult r = qm_project(COAX_QmDtype_bf16, n, s.cols, (coax_v_u8){(uint8_t *)(w + r0 * s.cols), n * s.cols * 2},
                                           (coax_v_f32){x + t * s.cols, s.cols}, (coax_v_f32){ref + r0, n}, (coax_v_f32){work + r0, n}, 0);
            if (r.status) { fprintf(stderr, "reference status %d\n", r.status); exit(1); }
        }
        for (int64_t row = 0; row < s.rows; row++) {
            double scale = 0;
            for (int64_t c = 0; c < s.cols; c++) scale += fabs((double)bf16(w[row * s.cols + c]) * x[t * s.cols + c]);
            double error = fabs((double)gpu[t * s.rows + row] - (double)ref[row]) / (scale > 0 ? scale : 1);
            if (error > worst) worst = error;
        }
    }
    int ok = worst <= PROJECT_BOUND;
    printf("%-30s %6lldx%-5lld x %3lld tokens, %s %2lld: differ from the C build in %lld of %lld; reference error %.2g of sum|wx| (bound %.2g) %s\n",
           s.what, (long long)s.rows, (long long)s.cols, (long long)s.tokens, s.slices ? "slices" : "lanes ", (long long)(s.slices ? s.slices : 16),
           (long long)differ, (long long)on, worst, PROJECT_BOUND, ok ? "ok" : "FAIL");
    if (timing) {
        int repeat = 20;
        double gpu_seconds = 0, total = 0;
        TRY(pm_begin(&b)); run_project(b, s, dw, dx, dout, dpart, parts); TRY(pm_wait(b, NULL));
        TRY(pm_begin(&b));
        for (int i = 0; i < repeat; i++) run_project(b, s, dw, dx, dout, dpart, parts);
        TRY(pm_wait(b, &gpu_seconds));
        total = gpu_seconds / repeat;
        double bytes = (double)wn * 2 + (double)xn * 4 + (double)on * 4, flops = 2.0 * (double)wn * (double)s.tokens;
        printf("%-30s %8.3f ms  %6.1f GB/s  %5.2f TFLOP/s\n", "", total * 1e3, bytes / total / 1e9, flops / total / 1e12);
    }
    pm_free(dw); pm_free(dx); pm_free(dout); pm_free(dpart);
    free(cpu); free(part); free(ref); free(work);
    return ok;
}

/* ---- Chapter 26's warp kernels on simdgroups ---- */
/* Frozen as in chapter 26: |device - Coax meaning| <= 2^-20 (|C| + sum |A B|)
   for the primitive, and the projection within 2^-14 sum |w x| of the
   reference profile. */
static const double MMA_BOUND = 1.0 / 1048576.0, MMA_PROJECT_BOUND = 1.0 / 16384.0;
static int mma_tiles(void) {
    enum { ITEMS = 4096 };
    PmBuffer da = alloc(ITEMS * 128 * 4), db = alloc(ITEMS * 64 * 4), dc = alloc(ITEMS * 128 * 4), dd = alloc(ITEMS * 128 * 4);
    uint32_t *a = pm_contents(da), *b = pm_contents(db);
    float *c = pm_contents(dc), *gpu = pm_contents(dd), *cpu = malloc(ITEMS * 128 * 4);
    for (int i = 0; i < ITEMS * 128; i++) { a[i] = wide_bf16() | wide_bf16() << 16; c[i] = ((float)(next() % 2001) - 1000.0f) / 1000.0f; }
    for (int i = 0; i < ITEMS * 64; i++) b[i] = wide_bf16() | wide_bf16() << 16;
    PmArg args[] = {V(da, ITEMS * 128), V(db, ITEMS * 64), V(dc, ITEMS * 128), V(dd, ITEMS * 128)};
    PmBatch batch;
    TRY(pm_begin(&batch)); TRY(pm_launch(batch, k_tile, ITEMS, 32, 128, args, 4)); TRY(pm_wait(batch, NULL));
    for (int i = 0; i < ITEMS; i++)
        dw_mma_tile(i, (coax_v_u32){a, ITEMS * 128}, (coax_v_u32){b, ITEMS * 64}, (coax_v_f32){c, ITEMS * 128}, (coax_v_f32){cpu, ITEMS * 128});
    double worst = 0;
    int64_t exact = 0;
    for (int item = 0; item < ITEMS; item++)
        for (int m = 0; m < 16; m++)
            for (int n = 0; n < 8; n++) {
                int at = item * 128 + 4 * ((m % 8) * 4 + n / 2) + 2 * (m / 8) + n % 2;
                double scale = fabs(c[at]);
                for (int k = 0; k < 16; k++) {
                    int la = (m % 8) * 4 + (k % 8) / 2, ra = m / 8 + 2 * (k / 8), lb = n * 4 + (k % 8) / 2;
                    scale += fabs((double)half_of(a[item * 128 + 4 * la + ra], k % 2) * half_of(b[item * 64 + 2 * lb + k / 8], k % 2));
                }
                double e = fabs((double)gpu[at] - cpu[at]) / (scale > 0 ? scale : 1);
                if (e > worst) worst = e;
                if (!memcmp(&gpu[at], &cpu[at], 4)) exact++;
            }
    int ok = worst <= MMA_BOUND;
    printf("%d simdgroups of the m16n8k16 BF16 primitive: %lld of %d outputs bit-identical to the Coax meaning; largest difference %.3g of |C|+sum|AB| (bound %.3g) %s\n",
           ITEMS, (long long)exact, ITEMS * 128, worst, MMA_BOUND, ok ? "ok" : "FAIL");
    pm_free(da); pm_free(db); pm_free(dc); pm_free(dd); free(cpu);
    return ok;
}
static int mma_projections(int quick) {
    struct { int rows, cols, tokens, slices; const char *what; } shapes[] = {
        {8192, 4096, 86, 8, "qkv, q"}, {12288, 4096, 86, 8, "gate, up"}, {4096, 12288, 86, 16, "down"},
        {4096, 4096, 86, 16, "z, out"}, {1024, 4096, 86, 16, "k, v"}, {32, 4096, 86, 16, "a, b"}};
    int ok = 1, count = quick ? 1 : (int)(sizeof shapes / sizeof *shapes);
    for (int i = 0; i < count; i++) {
        int rows = shapes[i].rows, cols = shapes[i].cols, T = shapes[i].tokens, S = shapes[i].slices;
        int groups = (T + 87) / 88, padded = groups * 88, half = cols / 2;
        size_t wn = (size_t)rows * half, xn = (size_t)T * cols, hn = (size_t)padded * half, pn = (size_t)S * T * rows, on = (size_t)T * rows;
        PmBuffer dw = alloc(wn * 4), dx = alloc(xn * 4), dh = alloc(hn * 4), dl = alloc(hn * 4), dp = alloc(pn * 4), dout = alloc(on * 4);
        uint32_t *w = pm_contents(dw);
        float *x = pm_contents(dx), *out = pm_contents(dout), *ref = malloc((size_t)rows * 4), *work = malloc((size_t)rows * 4);
        for (size_t k = 0; k < wn; k++) w[k] = wide_bf16() | wide_bf16() << 16;
        for (size_t k = 0; k < xn; k++) x[k] = random_x();
        PmArg sa[] = {I(cols), I(T), V(dx, (int64_t)xn), V(dh, (int64_t)hn), V(dl, (int64_t)hn)};
        PmArg pa[] = {I(rows), I(cols), I(T), I(S), V(dw, (int64_t)wn), V(dh, (int64_t)hn), V(dl, (int64_t)hn), V(dp, (int64_t)pn)};
        PmArg ra[] = {I((int64_t)on), I(S), V(dp, (int64_t)pn), V(dout, (int64_t)on)};
        int64_t warps = (int64_t)rows / 16 * groups * S;
        double t_split = 0, t_project = 0, t_reduce = 0;
        for (int pass = 0; pass < 2; pass++) {
            int repeat = pass ? 10 : 1;
            PmBatch b; double t;
            TRY(pm_begin(&b)); for (int r = 0; r < repeat; r++) TRY(pm_launch(b, k_split, (int64_t)hn, 1, 256, sa, 5)); TRY(pm_wait(b, &t)); t_split = t / repeat;
            TRY(pm_begin(&b)); for (int r = 0; r < repeat; r++) TRY(pm_launch(b, k_mma_project, warps, 32, 128, pa, 8)); TRY(pm_wait(b, &t)); t_project = t / repeat;
            TRY(pm_begin(&b)); for (int r = 0; r < repeat; r++) TRY(pm_launch(b, k_reduce, (int64_t)on, 1, 256, ra, 4)); TRY(pm_wait(b, &t)); t_reduce = t / repeat;
        }
        double worst = 0;
        for (int t = 0; t < T; t += T / 4) {
            for (int r0 = 0; r0 < rows; r0 += 32768) {
                int nr = rows - r0 < 32768 ? rows - r0 : 32768;
                coax_t_QmResult qr = qm_project(COAX_QmDtype_bf16, nr, cols, (coax_v_u8){(uint8_t *)(w + (size_t)r0 * half), (int64_t)nr * cols * 2},
                                                (coax_v_f32){x + (size_t)t * cols, cols}, (coax_v_f32){ref + r0, nr}, (coax_v_f32){work + r0, nr}, 0);
                if (qr.status) { fprintf(stderr, "reference status %u\n", qr.status); exit(1); }
            }
            for (int r = 0; r < rows; r++) {
                double scale = 0;
                for (int c = 0; c < cols; c++) {
                    uint32_t word = w[(size_t)r * half + c / 2];
                    scale += fabs((double)bf16(c % 2 ? word >> 16 : word & 0xffff) * x[(size_t)t * cols + c]);
                }
                double e = fabs((double)out[(size_t)t * rows + r] - ref[r]) / (scale > 0 ? scale : 1);
                if (e > worst) worst = e;
            }
        }
        double total = t_split + t_project + t_reduce, flops = 2.0 * rows * cols * T;
        int good = worst <= MMA_PROJECT_BOUND;
        ok &= good;
        printf("%-9s %5d x %5d x %d, %2d slices: split %.3f + project %.3f + reduce %.3f = %.3f ms, %5.2f TFLOP/s; reference error %.2g of sum|wx| (bound %.2g) %s\n",
               shapes[i].what, rows, cols, T, S, t_split * 1e3, t_project * 1e3, t_reduce * 1e3, total * 1e3, flops / total / 1e12, worst, MMA_PROJECT_BOUND, good ? "ok" : "FAIL");
        pm_free(dw); pm_free(dx); pm_free(dh); pm_free(dl); pm_free(dp); pm_free(dout);
        free(ref); free(work);
    }
    return ok;
}

/* ---- APPLE-FF-v1 against binary64 ---- */
static double pair(const float *p) { return (double)p[0] + (double)p[1]; }
static double rel(double got, double want) {
    if (got == want) return 0;
    if (want == 0) return fabs(got);
    return fabs(got - want) / fabs(want);
}
/* Frozen before measuring: sums, products and quotients of widened f32
   values within 2^-44 of binary64, relative to the magnitudes combined; exp, log and sqrt within 2^-40
   wherever the result is a normal f32 (log, sqrt) or at least 2^-86 (exp). The
   exp floor was 2^-102 when frozen; the first exhaustive run showed why that
   was wrong: a low half below 2^-126 is flushed, which costs up to
   2^-126 / |r| of the result, so 40 bits hold only from 2^-86. The bound did
   not move; results between 2^-126 and 2^-86 are reported, not judged. */
static const double ARITH_BOUND = 5.684341886080802e-14, MATH_BOUND = 9.094947017729282e-13;
static int ff_arith_check(void) {
    enum { N = 1 << 22 };
    PmBuffer da = alloc(N * 4), db = alloc(N * 4), dc = alloc(N * 4), dout = alloc(N * 3 * 8);
    float *a = pm_contents(da), *b = pm_contents(db), *c = pm_contents(dc), *out = pm_contents(dout);
    for (int i = 0; i < N; i++) {
        uint32_t r = next();
        a[i] = ldexpf(random_x(), (int)(r % 40) - 20); b[i] = ldexpf(random_x(), (int)((r >> 8) % 40) - 20); c[i] = ldexpf(random_x(), (int)((r >> 16) % 40) - 20);
        if (b[i] == 0) b[i] = 1;
    }
    PmArg args[] = {V(da, N), V(db, N), V(dc, N), V(dout, N * 3)};
    PmBatch batch;
    TRY(pm_begin(&batch)); TRY(pm_launch(batch, k_arith, N, 1, 256, args, 4)); TRY(pm_wait(batch, NULL));
    double *cpu = malloc((size_t)N * 3 * 8), worst[3] = {0, 0, 0};
    for (int i = 0; i < N; i++) ff_arith(i, (coax_v_f32){a, N}, (coax_v_f32){b, N}, (coax_v_f32){c, N}, (coax_v_f64){cpu, N * 3});
    /* Relative to the operands' magnitudes: a compensated sum is not correctly
       rounded, so cancellation is measured against what was added. */
    for (int i = 0; i < N * 3; i++) {
        int j = i / 3, k = i % 3;
        double x = a[j], y = b[j], z = c[j];
        double scale = k == 0 ? fabs(x * y) + fabs(z) : k == 1 ? fabs(x) + fabs(y) + fabs(z) : fabs(x / y);
        double e = fabs(pair(out + 2 * i) - cpu[i]) / (scale > 0 ? scale : 1);
        if (e > worst[k]) worst[k] = e;
    }
    int ok = worst[0] <= ARITH_BOUND && worst[1] <= ARITH_BOUND && worst[2] <= ARITH_BOUND;
    printf("float-float on %d random triples against binary64: a*b+c %.3g of |ab|+|c|, (a+b)-c %.3g of |a|+|b|+|c|, a/b %.3g of |a/b| (bound %.3g) %s\n", N, worst[0], worst[1], worst[2], ARITH_BOUND, ok ? "ok" : "FAIL");
    pm_free(da); pm_free(db); pm_free(dc); pm_free(dout); free(cpu);
    return ok;
}
/* Every f32 bit pattern (or every 4096th with quick): exp of x, log and sqrt
   of |x|, compared where binary64 says the result is a normal f32. */
static int ff_math_check(int quick) {
    const int64_t chunk = (int64_t)1 << 24;
    int64_t items = quick ? 4096 : chunk; /* quick: the first 4096 patterns of each block of 2^24 */
    PmBuffer dex = alloc((size_t)chunk * 8), dlg = alloc((size_t)chunk * 8), dsq = alloc((size_t)chunk * 8);
    float *ex = pm_contents(dex), *lg = pm_contents(dlg), *sq = pm_contents(dsq);
    double worst_exp = 0, worst_log = 0, worst_sqrt = 0, worst_exp_small = 0;
    uint32_t at_exp = 0, at_log = 0, at_sqrt = 0;
    int64_t checked = 0;
    double t0 = now();
    for (uint64_t first = 0; first < ((uint64_t)1 << 32); first += (uint64_t)chunk) {
        PmBatch b;
        TRY(pm_begin(&b));
        PmArg args[] = {{PM_ARG_U32, (int64_t)first, 0, NULL, 0, 0}, V(dex, chunk), V(dlg, chunk), V(dsq, chunk)};
        TRY(pm_launch(b, k_math, items, 1, 256, args, 4));
        TRY(pm_wait(b, NULL));
        enum { PARTS = 64 };
        typedef struct { double exp, log, sqrt, small; uint32_t at[3]; int64_t n; } Part;
        Part *parts = calloc(PARTS, sizeof *parts);
        dispatch_apply(PARTS, dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^(size_t p) {
            double we = 0, wl = 0, ws = 0, wsm = 0; uint32_t ae = 0, al = 0, as = 0; int64_t n = 0;
            for (int64_t i = (int64_t)p * items / PARTS; i < ((int64_t)p + 1) * items / PARTS; i++) {
                uint32_t bits = (uint32_t)(first + (uint64_t)i);
                float xf; memcpy(&xf, &bits, 4);
                if (!isfinite(xf)) continue;
                double x = xf, ax = fabs(x);
                double e = exp(x);
                if (e >= 0x1p-126 && e <= 3.4028234663852886e38) {
                    double r = rel(pair(ex + 2 * i), e);
                    if (e >= 0x1p-86) { if (r > we) { we = r; ae = bits; } } else if (r > wsm) wsm = r;
                    n++;
                }
                if (!(bits & 0x80000000u) && ax >= 0x1p-126) {
                    double l = log(ax), s = sqrt(ax);
                    if (fabs(l) >= 0x1p-102) { double r = rel(pair(lg + 2 * i), l); if (r > wl) { wl = r; al = bits; } }
                    double r = rel(pair(sq + 2 * i), s); if (r > ws) { ws = r; as = bits; }
                    n += 2;
                }
            }
            parts[p] = (Part){we, wl, ws, wsm, {ae, al, as}, n};
        });
        for (int p = 0; p < PARTS; p++) {
            if (parts[p].exp > worst_exp) { worst_exp = parts[p].exp; at_exp = parts[p].at[0]; }
            if (parts[p].log > worst_log) { worst_log = parts[p].log; at_log = parts[p].at[1]; }
            if (parts[p].sqrt > worst_sqrt) { worst_sqrt = parts[p].sqrt; at_sqrt = parts[p].at[2]; }
            if (parts[p].small > worst_exp_small) worst_exp_small = parts[p].small;
            checked += parts[p].n;
        }
        free(parts);
    }
    int ok = worst_exp <= MATH_BOUND && worst_log <= MATH_BOUND && worst_sqrt <= MATH_BOUND;
    printf("float-float exp, log, sqrt over %s f32 inputs (%lld results, %.0f s): exp %.3g (at 0x%08x), log %.3g (at 0x%08x), sqrt %.3g (at 0x%08x) relative (bound %.3g); "
           "exp results below 2^-86, where flushing the low half below 2^-126 leaves fewer than 40 bits: %.3g %s\n",
           quick ? "the first 4096 of each block of 2^24" : "all 2^32", (long long)checked, now() - t0, worst_exp, at_exp, worst_log, at_log, worst_sqrt, at_sqrt,
           MATH_BOUND, worst_exp_small, ok ? "ok" : "FAIL");
    pm_free(dex); pm_free(dlg); pm_free(dsq);
    return ok;
}

int main(int argc, char **argv) {
    if (argc < 4) { fprintf(stderr, "usage: metal-kernels PROJECT.metal WARP.metal FF.metal [quick]\n"); return 2; }
    int quick = argc > 4 && !strcmp(argv[4], "quick"), ok = 1;
    PmDeviceInfo info;
    PmStatus s = pm_open(&info);
    if (s == PM_ABSENT) { printf("SKIP: no Metal device\n"); return 0; }
    if (s) { fprintf(stderr, "pm_open: %s\n", pm_log()); return 1; }
    printf("device: %s, Apple GPU family %d, unified memory %s, working set %.1f GiB\n", info.name, info.apple_family, info.unified_memory ? "yes" : "no",
           info.working_set_bytes / 1073741824.0);
    PmModule project = compile(argv[1], "project.metal"), warp = compile(argv[2], "warp.metal"), ff = compile(argv[3], "metal-ff.metal");
    TRY(pm_kernel(project, "dv_project", &k_project)); TRY(pm_kernel(project, "dv_matvec", &k_matvec)); TRY(pm_kernel(project, "dv_reduce", &k_reduce));
    TRY(pm_kernel(warp, "dw_split", &k_split)); TRY(pm_kernel(warp, "dw_project", &k_mma_project)); TRY(pm_kernel(warp, "dw_mma_tile", &k_tile));
    TRY(pm_kernel(ff, "ff_math", &k_math)); TRY(pm_kernel(ff, "ff_arith", &k_arith));
    Shape small[] = {{4, 4, 1, 1, "smallest"}, {16, 260, 9, 1, "ragged tokens"}, {64, 1024, 17, 2, "two slices"},
                     {32, 4096, 1, 0, "one token, lanes"}, {64, 12288, 3, 3, "three slices"}};
    for (size_t i = 0; i < sizeof small / sizeof *small; i++) ok &= project_shape(small[i], 0);
    if (!quick) {
        Shape elephant[] = {{4096, 4096, 1, 0, "z / out projection, one token"}, {12288, 4096, 1, 0, "gate / up, one token"},
                            {248320, 4096, 1, 0, "vocabulary readout, one token"}, {8192, 4096, 86, 8, "qkv, 86 tokens"},
                            {12288, 4096, 86, 8, "gate / up, 86 tokens"}, {4096, 12288, 86, 16, "down, 86 tokens"}};
        for (size_t i = 0; i < sizeof elephant / sizeof *elephant; i++) ok &= project_shape(elephant[i], 1);
    }
    ok &= mma_tiles();
    ok &= mma_projections(quick);
    ok &= ff_arith_check();
    ok &= ff_math_check(quick);
    /* A bounds check that fails inside a kernel voids the launch: slice 1 of 2
       of dv_reduce reads past its view, and the host reports it. */
    PmBuffer tiny = alloc(64 * 4);
    PmArg bad[] = {I(64), I(2), V(tiny, 64), V(tiny, 64)};
    PmBatch b;
    TRY(pm_begin(&b));
    TRY(pm_launch(b, k_reduce, 64, 1, 64, bad, 4));
    PmStatus finished = pm_wait(b, NULL);
    if (finished == PM_CHECK) printf("an out-of-bounds read voided the launch: %s\n", pm_log());
    else { printf("an out-of-bounds read was not reported (status %d)\n", finished); ok = 0; }
    puts(ok ? "Metal kernels qualify" : "Metal kernels FAILED");
    return ok ? 0 : 1;
}
