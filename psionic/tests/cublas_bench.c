/* Opt-in comparator (chapter 34): Psionic's Coax projection kernels against
   cuBLAS on the same shapes. cuBLAS is test-only: this harness opens it with
   dlopen from PSIONIC_CUBLAS (default ~/cublas-13.2/lib/libcublas.so.13) and
   skips when it is absent. No library or server build links or loads it.

   Protocol cm-warm-v1 (psionic/lib/comparator.coax owns the statistics and
   the verdict): for each shape and engine, one untimed warm-up run, then RUNS
   timed warm runs; a run is REPEAT back-to-back products timed with CUDA
   events, recorded as nanoseconds per product. Both engines read the same
   BF16 weight buffer and the same activation values (BF16 values; Psionic
   reads them from f32 storage, cuBLAS from BF16 storage) and accumulate in
   f32. cuBLAS SGEMM on f32 weights runs too, and the verdict refuses it for
   its different number formats.
   Usage: cublas-bench PROJECT.cu BUILD  (prints the middle of a JSON
   receipt; psionic/tests/bench_cublas.sh writes the rest) */
#define _GNU_SOURCE
#include "cuda-host.h"
#include "comparator.h"
#include <dlfcn.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef int (*Create)(void **);
typedef int (*Sgemm)(void *, int, int, int, int, int, const float *, const float *, int, const float *, int, const float *, float *, int);
typedef int (*GemmEx)(void *, int, int, int, int, int, const void *, const void *, int, int, const void *, int, int, const void *, void *, int, int, int, int);
enum { OP_N = 0, OP_T = 1, R_32F = 0, R_16BF = 14, COMPUTE_32F = 68, ALGO_DEFAULT = -1 };
enum { RUNS = 7, REPEAT = 20, ENGINES = 3 };

static uint64_t seed = 0x2545f4914f6cdd1dULL;
static uint32_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return (uint32_t)(seed >> 16); }
static uint16_t random_bf16(void) { uint32_t r = next(); return (uint16_t)((((r >> 10) & 1) << 15) | ((119 + r % 8) << 7) | ((r >> 3) & 0x7f)); }
static float bf16(uint16_t h) { uint32_t b = (uint32_t)h << 16; float f; memcpy(&f, &b, 4); return f; }
static uint16_t to_bf16(float f) { uint32_t b; memcpy(&b, &f, 4); b += 0x7fff + ((b >> 16) & 1); return (uint16_t)(b >> 16); }
/* FNV-1a, 64 bits: identity words for engines, builds, machines and weights. */
static uint64_t fnv(uint64_t h, const void *p, size_t n) {
    const unsigned char *b = p;
    for (size_t i = 0; i < n; i++) { h ^= b[i]; h *= 0x100000001b3ULL; }
    return h;
}
static uint64_t fnv_str(const char *s) { return fnv(0xcbf29ce484222325ULL, s, strlen(s)); }
#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
#define CB(x) do { int r_ = (x); if (r_) { fprintf(stderr, "%s: cuBLAS status %d\n", #x, r_); exit(1); } } while (0)

static double seconds_of(PcEvent a, PcEvent b) { double s; TRY(pc_event_seconds(a, b, &s)); return s; }

typedef struct {
    int rows, cols, tokens, slices;
    PcKernel project, matvec, reduce;
    PcArg vw, vx, vo, vp;
    void *handle; Sgemm sgemm; GemmEx gemm_ex;
    PcBuffer dwf, dx, dxb, dref, drefb;
} Shape;

static void product(const Shape *s, int engine) {
    const float one = 1, zero = 0;
    int rows = s->rows, cols = s->cols, T = s->tokens, S = s->slices;
    int64_t on = (int64_t)T * rows;
    if (engine == 0 && S == 0) {
        PcArg m[] = {{PC_ARG_INT, rows}, {PC_ARG_INT, cols}, s->vw, s->vx, s->vp};
        TRY(pc_launch(s->matvec, (int64_t)rows * 16, 256, m, 5, NULL));
        PcArg d[] = {{PC_ARG_INT, on}, {PC_ARG_INT, 16}, s->vp, s->vo};
        TRY(pc_launch(s->reduce, on, 256, d, 4, NULL));
    } else if (engine == 0) {
        PcArg m[] = {{PC_ARG_INT, rows}, {PC_ARG_INT, cols}, {PC_ARG_INT, T}, {PC_ARG_INT, S}, s->vw, s->vx, s->vp};
        TRY(pc_launch(s->project, (int64_t)rows / 4 * ((T + 7) / 8) * S, 128, m, 7, NULL));
        PcArg d[] = {{PC_ARG_INT, on}, {PC_ARG_INT, S}, s->vp, s->vo};
        TRY(pc_launch(s->reduce, on, 256, d, 4, NULL));
    } else if (engine == 1) {
        CB(s->gemm_ex(s->handle, OP_T, OP_N, rows, T, cols, &one, (const void *)s->vw.data, R_16BF, cols, (const void *)s->dxb, R_16BF, cols,
                      &zero, (void *)s->drefb, R_32F, rows, COMPUTE_32F, ALGO_DEFAULT));
    } else {
        CB(s->sgemm(s->handle, OP_T, OP_N, rows, T, cols, &one, (const float *)s->dwf, cols, (const float *)s->dx, cols, &zero, (float *)s->dref, rows));
    }
}
/* One run: REPEAT products between two events, as nanoseconds per product. */
static int64_t run(const Shape *s, int engine, PcEvent a, PcEvent b) {
    TRY(pc_record(a, NULL));
    for (int r = 0; r < REPEAT; r++) product(s, engine);
    TRY(pc_record(b, NULL)); TRY(pc_sync());
    return (int64_t)llround(seconds_of(a, b) * 1e9 / REPEAT);
}
static void print_stats(const char *name, const int64_t *x, coax_t_CmStats st, coax_t_CmStats wf) {
    printf("        \"%s\": {\"runs_ns\": [", name);
    for (int i = 0; i < RUNS; i++) printf("%s%lld", i ? ", " : "", (long long)x[i]);
    printf("], \"mean_ns\": %.1f, \"sd_ns\": %.1f, \"welford_mean_ns\": %.1f, \"welford_sd_ns\": %.1f}", st.mean, st.sd, wf.mean, wf.sd);
}
static void print_call(const char *name, coax_t_CmCall c, int last) {
    static const char *verdicts[] = {"refused", "faster", "slower", "same", "noisy"};
    static const char *reasons[] = {"none", "unbound", "machine", "file", "format", "shape", "warmup", "stats"};
    printf("        \"%s\": {\"verdict\": \"%s\", \"reason\": \"%s\", \"speed_ratio\": %.3f, \"se_ns\": %.1f}%s\n", name, verdicts[c.verdict], reasons[c.reason],
           c.ratio, c.se, last ? "" : ",");
}

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: cublas-bench PROJECT.cu BUILD\n"); return 2; }
    char path[4096], real[4096], host[256] = "unknown";
    const char *env = getenv("PSIONIC_CUBLAS"), *home = getenv("HOME");
    if (env) snprintf(path, sizeof path, "%s", env);
    else snprintf(path, sizeof path, "%s/cublas-13.2/lib/libcublas.so.13", home ? home : "");
    PcDeviceInfo info;
    if (pc_open(&info) != PC_OK) { printf("SKIP: no CUDA device (%s)\n", pc_log()); return 0; }
    void *lib = dlopen(path, RTLD_NOW | RTLD_LOCAL);
    if (!lib) { printf("SKIP: no cuBLAS at %s\n", path); return 0; }
    if (!realpath(path, real)) snprintf(real, sizeof real, "%s", path);
    gethostname(host, sizeof host - 1);
    Create create = (Create)dlsym(lib, "cublasCreate_v2");
    Sgemm sgemm = (Sgemm)dlsym(lib, "cublasSgemm_v2");
    GemmEx gemm_ex = (GemmEx)dlsym(lib, "cublasGemmEx");
    if (!create || !sgemm || !gemm_ex) { printf("SKIP: cuBLAS symbols missing\n"); return 0; }
    void *handle;
    CB(create(&handle));
    FILE *f = fopen(argv[1], "rb");
    if (!f) return 2;
    fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET);
    char *src = malloc((size_t)n);
    if (!src || fread(src, 1, (size_t)n, f) != (size_t)n) return 2;
    fclose(f);
    PcModule module; double compile;
    TRY(pc_compile(src, (size_t)n, "project.cu", &module, &compile));
    Shape sh = {.handle = handle, .sgemm = sgemm, .gemm_ex = gemm_ex};
    TRY(pc_kernel(module, "dv_project", &sh.project));
    TRY(pc_kernel(module, "dv_matvec", &sh.matvec));
    TRY(pc_kernel(module, "dv_reduce", &sh.reduce));
    PcEvent a, b;
    TRY(pc_event(&a)); TRY(pc_event(&b));
    /* Identity words: the machine is the host and the device; Psionic's build
       is its source blob ids; cuBLAS's is the resolved library file. */
    uint64_t machine = fnv(fnv_str(host), info.name, strlen(info.name));
    uint64_t engine[ENGINES] = {fnv_str("psionic"), fnv_str("cublas-gemmex"), fnv_str("cublas-sgemm")};
    uint64_t build[ENGINES] = {fnv_str(argv[2]), fnv_str(real), fnv_str(real)};
    printf("  \"device\": \"%s\",\n  \"cublas\": \"%s\",\n  \"protocol\": \"cm-warm-v1\",\n  \"warmup_runs\": 1,\n  \"runs\": %d,\n  \"products_per_run\": %d,\n  \"shapes\": [\n",
           info.name, real, RUNS, REPEAT);
    struct { int rows, cols, tokens, slices; } shapes[] = {
        {248320, 4096, 1, 0}, {12288, 4096, 1, 0}, {4096, 12288, 1, 0}, {4096, 4096, 1, 0},
        {8192, 4096, 86, 8}, {12288, 4096, 86, 8}, {4096, 12288, 86, 16}, {4096, 4096, 86, 16}, {1024, 4096, 86, 16}};
    int count = (int)(sizeof shapes / sizeof *shapes);
    for (int s = 0; s < count; s++) {
        int rows = shapes[s].rows, cols = shapes[s].cols, T = shapes[s].tokens, S = shapes[s].slices;
        sh.rows = rows; sh.cols = cols; sh.tokens = T; sh.slices = S;
        size_t wn = (size_t)rows * cols, xn = (size_t)T * cols, on = (size_t)T * rows;
        uint16_t *w = malloc(wn * 2), *xb = malloc(xn * 2);
        float *wf = malloc(wn * 4), *x = malloc(xn * 4), *ours = malloc(on * 4), *refb = malloc(on * 4);
        if (!w || !xb || !wf || !x || !ours || !refb) return 2;
        for (size_t i = 0; i < wn; i++) { w[i] = random_bf16(); wf[i] = bf16(w[i]); }
        /* Activations are BF16 values on both sides. */
        for (size_t i = 0; i < xn; i++) { xb[i] = to_bf16(((float)(next() % 2000001) - 1000000.0f) / 1000000.0f); x[i] = bf16(xb[i]); }
        uint64_t file = fnv(0xcbf29ce484222325ULL, w, wn * 2);
        PcBuffer dw, dout, dpart;
        size_t parts = on * (S ? (size_t)S : 16);
        TRY(pc_alloc(wn * 2, &dw)); TRY(pc_alloc(wn * 4, &sh.dwf)); TRY(pc_alloc(xn * 4, &sh.dx)); TRY(pc_alloc(xn * 2, &sh.dxb));
        TRY(pc_alloc(on * 4, &dout)); TRY(pc_alloc(on * 4, &sh.dref)); TRY(pc_alloc(on * 4, &sh.drefb)); TRY(pc_alloc(parts * 4, &dpart));
        TRY(pc_upload(dw, w, wn * 2, NULL)); TRY(pc_upload(sh.dwf, wf, wn * 4, NULL)); TRY(pc_upload(sh.dx, x, xn * 4, NULL)); TRY(pc_upload(sh.dxb, xb, xn * 2, NULL));
        sh.vw = (PcArg){PC_ARG_VIEW, .data = dw, .count = (int64_t)wn / 4};
        sh.vx = (PcArg){PC_ARG_VIEW, .data = sh.dx, .count = (int64_t)xn};
        sh.vo = (PcArg){PC_ARG_VIEW, .data = dout, .count = (int64_t)on};
        sh.vp = (PcArg){PC_ARG_VIEW, .data = dpart, .count = (int64_t)parts};
        int64_t t[ENGINES][RUNS];
        coax_t_CmStats st[ENGINES], wf_st[ENGINES];
        coax_t_CmSide side[ENGINES];
        for (int e = 0; e < ENGINES; e++) {
            run(&sh, e, a, b);  /* warm-up, never recorded */
            for (int r = 0; r < RUNS; r++) t[e][r] = run(&sh, e, a, b);
            st[e] = cm_two_pass((coax_v_s64){t[e], RUNS});
            wf_st[e] = cm_welford((coax_v_s64){t[e], RUNS});
            coax_t_CmFormat wfmt = e == 2 ? COAX_CmFormat_f32 : COAX_CmFormat_bf16, xfmt = wfmt;
            side[e] = (coax_t_CmSide){{file, wfmt, xfmt, COAX_CmFormat_f32, rows, cols, T}, engine[e], build[e], machine, 1};
        }
        TRY(pc_download(ours, dout, on * 4, NULL)); TRY(pc_download(refb, sh.drefb, on * 4, NULL)); TRY(pc_sync());
        /* Difference relative to sum |w x| on sampled outputs. */
        double worst = 0;
        for (size_t i = 0; i < on; i += on / 997 + 1) {
            size_t tk = i / rows, r = i % rows;
            double scale = 0;
            for (int c = 0; c < cols; c++) scale += fabs((double)wf[r * cols + c] * x[tk * cols + c]);
            double e = fabs((double)ours[i] - refb[i]) / scale;
            if (e > worst) worst = e;
        }
        printf("    {\"rows\": %d, \"cols\": %d, \"tokens\": %d, \"weights_fnv\": \"%016llx\", \"max_rel_diff_vs_gemmex\": %.2g,\n", rows, cols, T,
               (unsigned long long)file, worst);
        print_stats("psionic", t[0], st[0], wf_st[0]); printf(",\n");
        print_stats("cublas_gemmex_bf16", t[1], st[1], wf_st[1]); printf(",\n");
        print_stats("cublas_sgemm_f32", t[2], st[2], wf_st[2]); printf(",\n");
        printf("      \"verdicts\": {\n");
        print_call("psionic_vs_gemmex_bf16", cm_compare(side[0], st[0], side[1], st[1]), 0);
        print_call("psionic_vs_sgemm_f32", cm_compare(side[0], st[0], side[2], st[2]), 1);
        printf("      }}%s\n", s + 1 < count ? "," : "");
        pc_free(dw); pc_free(sh.dwf); pc_free(sh.dx); pc_free(sh.dxb); pc_free(dout); pc_free(sh.dref); pc_free(sh.drefb); pc_free(dpart);
        free(w); free(xb); free(wf); free(x); free(ours); free(refb);
    }
    printf("  ],\n");
    return 0;
}
