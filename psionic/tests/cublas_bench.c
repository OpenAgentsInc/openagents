/* Opt-in comparator (chapter 34): Psionic's Coax projection kernels against
   cuBLAS on the same shapes. cuBLAS is test-only: this harness opens it with
   dlopen from PSIONIC_CUBLAS (default ~/cublas-13.2/lib/libcublas.so.13) and
   skips when it is absent. No library or server build links or loads it.
   Usage: cublas-bench PROJECT.cu */
#define _GNU_SOURCE
#include "cuda-host.h"
#include <dlfcn.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef int (*Create)(void **);
typedef int (*SetStream)(void *, PcStream);
typedef int (*Sgemm)(void *, int, int, int, int, int, const float *, const float *, int, const float *, int, const float *, float *, int);
typedef int (*GemmEx)(void *, int, int, int, int, int, const void *, const void *, int, int, const void *, int, int, const void *, void *, int, int, int, int);
enum { OP_N = 0, OP_T = 1, R_32F = 0, R_16BF = 14, COMPUTE_32F = 68, ALGO_DEFAULT = -1 };

static uint64_t seed = 0x2545f4914f6cdd1dULL;
static uint32_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return (uint32_t)(seed >> 16); }
static uint16_t random_bf16(void) { uint32_t r = next(); return (uint16_t)((((r >> 10) & 1) << 15) | ((119 + r % 8) << 7) | ((r >> 3) & 0x7f)); }
static float bf16(uint16_t h) { uint32_t b = (uint32_t)h << 16; float f; memcpy(&f, &b, 4); return f; }
static uint16_t to_bf16(float f) { uint32_t b; memcpy(&b, &f, 4); b += 0x7fff + ((b >> 16) & 1); return (uint16_t)(b >> 16); }
#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
#define CB(x) do { int r_ = (x); if (r_) { fprintf(stderr, "%s: cuBLAS status %d\n", #x, r_); exit(1); } } while (0)

static double seconds_of(PcEvent a, PcEvent b) { double s; TRY(pc_event_seconds(a, b, &s)); return s; }

int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: cublas-bench PROJECT.cu\n"); return 2; }
    char path[4096];
    const char *env = getenv("PSIONIC_CUBLAS"), *home = getenv("HOME");
    if (env) snprintf(path, sizeof path, "%s", env);
    else snprintf(path, sizeof path, "%s/cublas-13.2/lib/libcublas.so.13", home ? home : "");
    PcDeviceInfo info;
    if (pc_open(&info) != PC_OK) { printf("SKIP: no CUDA device (%s)\n", pc_log()); return 0; }
    void *lib = dlopen(path, RTLD_NOW | RTLD_LOCAL);
    if (!lib) { printf("SKIP: no cuBLAS at %s\n", path); return 0; }
    Create create = (Create)dlsym(lib, "cublasCreate_v2");
    SetStream set_stream = (SetStream)dlsym(lib, "cublasSetStream_v2");
    Sgemm sgemm = (Sgemm)dlsym(lib, "cublasSgemm_v2");
    GemmEx gemm_ex = (GemmEx)dlsym(lib, "cublasGemmEx");
    if (!create || !set_stream || !sgemm || !gemm_ex) { printf("SKIP: cuBLAS symbols missing\n"); return 0; }
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
    PcKernel project, matvec, reduce;
    TRY(pc_kernel(module, "dv_project", &project));
    TRY(pc_kernel(module, "dv_matvec", &matvec));
    TRY(pc_kernel(module, "dv_reduce", &reduce));
    PcEvent a, b;
    TRY(pc_event(&a)); TRY(pc_event(&b));
    printf("device: %s; cuBLAS from %s\n", info.name, path);
    printf("%-20s %8s %10s %10s %10s %8s %8s %10s %10s\n", "shape rows x cols x T", "Psionic", "SGEMM f32", "GEMM bf16", "TFLOP/s", "vs f32", "vs bf16", "f32 diff", "bf16 diff");
    struct { int rows, cols, tokens, slices; } shapes[] = {
        {248320, 4096, 1, 0}, {12288, 4096, 1, 0}, {4096, 12288, 1, 0}, {4096, 4096, 1, 0},
        {8192, 4096, 86, 8}, {12288, 4096, 86, 8}, {4096, 12288, 86, 16}, {4096, 4096, 86, 16}, {1024, 4096, 86, 16}};
    for (size_t s = 0; s < sizeof shapes / sizeof *shapes; s++) {
        int rows = shapes[s].rows, cols = shapes[s].cols, T = shapes[s].tokens, S = shapes[s].slices;
        size_t wn = (size_t)rows * cols, xn = (size_t)T * cols, on = (size_t)T * rows;
        uint16_t *w = malloc(wn * 2), *xb = malloc(xn * 2);
        float *wf = malloc(wn * 4), *x = malloc(xn * 4), *ours = malloc(on * 4), *ref = malloc(on * 4), *refb = malloc(on * 4);
        if (!w || !xb || !wf || !x || !ours || !ref || !refb) return 2;
        for (size_t i = 0; i < wn; i++) { w[i] = random_bf16(); wf[i] = bf16(w[i]); }
        for (size_t i = 0; i < xn; i++) { x[i] = ((float)(next() % 2000001) - 1000000.0f) / 1000000.0f; xb[i] = to_bf16(x[i]); }
        PcBuffer dw, dwf, dx, dxb, dout, dref, drefb, dpart;
        size_t parts = on * (S ? (size_t)S : 16);
        TRY(pc_alloc(wn * 2, &dw)); TRY(pc_alloc(wn * 4, &dwf)); TRY(pc_alloc(xn * 4, &dx)); TRY(pc_alloc(xn * 2, &dxb));
        TRY(pc_alloc(on * 4, &dout)); TRY(pc_alloc(on * 4, &dref)); TRY(pc_alloc(on * 4, &drefb)); TRY(pc_alloc(parts * 4, &dpart));
        TRY(pc_upload(dw, w, wn * 2, NULL)); TRY(pc_upload(dwf, wf, wn * 4, NULL)); TRY(pc_upload(dx, x, xn * 4, NULL)); TRY(pc_upload(dxb, xb, xn * 2, NULL));
        PcArg vw = {PC_ARG_VIEW, .data = dw, .count = (int64_t)wn / 4}, vx = {PC_ARG_VIEW, .data = dx, .count = (int64_t)xn};
        PcArg vo = {PC_ARG_VIEW, .data = dout, .count = (int64_t)on}, vp = {PC_ARG_VIEW, .data = dpart, .count = (int64_t)parts};
        double t_ours = 0, t_f32 = 0, t_bf = 0;
        const float one = 1, zero = 0;
        for (int pass = 0; pass < 2; pass++) {
            int repeat = pass ? 20 : 1;
            TRY(pc_record(a, NULL));
            for (int r = 0; r < repeat; r++) {
                if (S == 0) {
                    PcArg m[] = {{PC_ARG_INT, rows}, {PC_ARG_INT, cols}, vw, vx, vp};
                    TRY(pc_launch(matvec, (int64_t)rows * 16, 256, m, 5, NULL));
                    PcArg d[] = {{PC_ARG_INT, (int64_t)on}, {PC_ARG_INT, 16}, vp, vo};
                    TRY(pc_launch(reduce, (int64_t)on, 256, d, 4, NULL));
                } else {
                    PcArg m[] = {{PC_ARG_INT, rows}, {PC_ARG_INT, cols}, {PC_ARG_INT, T}, {PC_ARG_INT, S}, vw, vx, vp};
                    TRY(pc_launch(project, (int64_t)rows / 4 * ((T + 7) / 8) * S, 128, m, 7, NULL));
                    PcArg d[] = {{PC_ARG_INT, (int64_t)on}, {PC_ARG_INT, S}, vp, vo};
                    TRY(pc_launch(reduce, (int64_t)on, 256, d, 4, NULL));
                }
            }
            TRY(pc_record(b, NULL)); TRY(pc_sync()); t_ours = seconds_of(a, b) / repeat;
            TRY(pc_record(a, NULL));
            for (int r = 0; r < repeat; r++)
                CB(sgemm(handle, OP_T, OP_N, rows, T, cols, &one, (const float *)dwf, cols, (const float *)dx, cols, &zero, (float *)dref, rows));
            TRY(pc_record(b, NULL)); TRY(pc_sync()); t_f32 = seconds_of(a, b) / repeat;
            TRY(pc_record(a, NULL));
            for (int r = 0; r < repeat; r++)
                CB(gemm_ex(handle, OP_T, OP_N, rows, T, cols, &one, (const void *)dw, R_16BF, cols, (const void *)dxb, R_16BF, cols, &zero, (void *)drefb, R_32F, rows, COMPUTE_32F, ALGO_DEFAULT));
            TRY(pc_record(b, NULL)); TRY(pc_sync()); t_bf = seconds_of(a, b) / repeat;
        }
        TRY(pc_download(ours, dout, on * 4, NULL)); TRY(pc_download(ref, dref, on * 4, NULL)); TRY(pc_download(refb, drefb, on * 4, NULL)); TRY(pc_sync());
        /* Differences relative to sum |w x| on sampled outputs. */
        double worst = 0, worst_b = 0;
        for (size_t i = 0; i < on; i += on / 997 + 1) {
            size_t t = i / rows, r = i % rows;
            double scale = 0;
            for (int c = 0; c < cols; c++) scale += fabs((double)wf[r * cols + c] * x[t * cols + c]);
            double e = fabs((double)ours[i] - ref[i]) / scale, eb = fabs((double)ours[i] - refb[i]) / scale;
            if (e > worst) worst = e;
            if (eb > worst_b) worst_b = eb;
        }
        double flops = 2.0 * rows * cols * T;
        char name[64];
        snprintf(name, sizeof name, "%d x %d x %d", rows, cols, T);
        printf("%-20s %7.3fms %8.3fms %8.3fms %10.2f %7.0f%% %7.0f%% %10.2g %10.2g\n", name, t_ours * 1e3, t_f32 * 1e3, t_bf * 1e3, flops / t_ours / 1e12,
               100 * t_f32 / t_ours, 100 * t_bf / t_ours, worst, worst_b);
        pc_free(dw); pc_free(dwf); pc_free(dx); pc_free(dxb); pc_free(dout); pc_free(dref); pc_free(drefb); pc_free(dpart);
        free(w); free(xb); free(wf); free(x); free(ours); free(ref); free(refb);
    }
    puts("vs f32 and vs bf16: Psionic's throughput as a fraction of cuBLAS SGEMM (f32 weights and activations)\n"
         "and of cuBLAS GemmEx (BF16 weights and activations, f32 accumulation); diffs are |Psionic - cuBLAS| / sum|w x|.");
    return 0;
}
