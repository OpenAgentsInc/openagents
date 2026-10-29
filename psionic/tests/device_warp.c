/* The warp-kernel MMA primitive on a CUDA device against its Coax meaning
   compiled to C. Usage: device-warp WARP.cu. Prints SKIP without a device. */
#include "cuda-host.h"
#include "warp.h"
#include "qwen-matvec.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t seed = 0x853c49e6748fea9bULL;
static uint32_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return (uint32_t)(seed >> 16); }
static uint32_t bf16(void) { uint32_t r = next(); return ((r >> 10) & 1) << 15 | (110 + r % 24) << 7 | ((r >> 3) & 0x7f); }
static float half(uint32_t word, int high) { uint32_t b = high ? word & 0xffff0000u : word << 16; float f; memcpy(&f, &b, 4); return f; }
#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)

/* Frozen before measuring: |device - CPU| <= 2^-20 * (|C| + sum |A B|). */
static const double TOLERANCE = 1.0 / 1048576.0;

#ifndef DWT
#define DWT 88 /* tokens per warp of dw_project (DW_TOKENS) */
#endif
static PcModule warp_module;
static char *read_all(const char *path, size_t *n) {
    *n = 0;
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *t = malloc((size_t)size);
    if (!t || fread(t, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); *n = (size_t)size; return t;
}
static float from_bf16(uint32_t h) { uint32_t b = h << 16; float f; memcpy(&f, &b, 4); return f; }
/* Frozen before measuring: the tensor-core projection is within 2^-14 of
   sum |w x| of the CPU reference profile (halves carry x to 2^-17; f32
   fragments accumulate at most 768 steps). */
static const double PROJECT_TOLERANCE = 1.0 / 16384.0;
static int projections(const char *project_path) {
    size_t n = 0;
    char *src = read_all(project_path, &n);
    PcModule pm; double seconds; PcKernel split, project, reduce;
    TRY(pc_compile(src, n, "project.cu", &pm, &seconds));
    TRY(pc_kernel(warp_module, "dw_split", &split));
    TRY(pc_kernel(warp_module, "dw_project", &project));
    TRY(pc_kernel(pm, "dv_reduce", &reduce));
    struct { int rows, cols, tokens, slices; const char *what; } shapes[] = {
        {8192, 4096, 86, 8, "qkv, q"}, {12288, 4096, 86, 8, "gate, up"}, {4096, 12288, 86, 16, "down"},
        {4096, 4096, 86, 16, "z, out"}, {1024, 4096, 86, 16, "k, v"}, {32, 4096, 86, 16, "a, b"}};
    int ok = 1;
    for (size_t i = 0; i < sizeof shapes / sizeof *shapes; i++) {
        int rows = shapes[i].rows, cols = shapes[i].cols, T = shapes[i].tokens, S = shapes[i].slices;
        int groups = (T + DWT - 1) / DWT, padded = groups * DWT, half = cols / 2;
        size_t wn = (size_t)rows * half, xn = (size_t)T * cols, hn = (size_t)padded * half, pn = (size_t)S * T * rows, on = (size_t)T * rows;
        uint32_t *w = malloc(wn * 4);
        float *x = malloc(xn * 4), *out = malloc(on * 4), *ref = malloc((size_t)rows * 4), *work = malloc((size_t)rows * 4);
        for (size_t k = 0; k < wn; k++) w[k] = bf16() | bf16() << 16;
        for (size_t k = 0; k < xn; k++) x[k] = ((float)(next() % 2000001) - 1000000.0f) / 1000000.0f;
        PcBuffer dw, dx, dh, dl, dp, dout;
        TRY(pc_alloc(wn * 4, &dw)); TRY(pc_alloc(xn * 4, &dx)); TRY(pc_alloc(hn * 4, &dh)); TRY(pc_alloc(hn * 4, &dl));
        TRY(pc_alloc(pn * 4, &dp)); TRY(pc_alloc(on * 4, &dout));
        TRY(pc_upload(dw, w, wn * 4, NULL)); TRY(pc_upload(dx, x, xn * 4, NULL));
        PcArg sa[] = {{PC_ARG_INT, cols}, {PC_ARG_INT, T}, {PC_ARG_VIEW, .data = dx, .count = (int64_t)xn},
                      {PC_ARG_VIEW, .data = dh, .count = (int64_t)hn}, {PC_ARG_VIEW, .data = dl, .count = (int64_t)hn}};
        PcArg pa[] = {{PC_ARG_INT, rows}, {PC_ARG_INT, cols}, {PC_ARG_INT, T}, {PC_ARG_INT, S}, {PC_ARG_VIEW, .data = dw, .count = (int64_t)wn},
                      {PC_ARG_VIEW, .data = dh, .count = (int64_t)hn}, {PC_ARG_VIEW, .data = dl, .count = (int64_t)hn}, {PC_ARG_VIEW, .data = dp, .count = (int64_t)pn}};
        PcArg ra[] = {{PC_ARG_INT, (int64_t)on}, {PC_ARG_INT, S}, {PC_ARG_VIEW, .data = dp, .count = (int64_t)pn}, {PC_ARG_VIEW, .data = dout, .count = (int64_t)on}};
        int64_t warps = (int64_t)rows / 16 * groups * S;
        PcEvent a, b;
        TRY(pc_event(&a)); TRY(pc_event(&b));
        double t_split = 0, t_project = 0, t_reduce = 0;
        for (int pass = 0; pass < 2; pass++) {
            int repeat = pass ? 20 : 1;
            double t;
            TRY(pc_record(a, NULL));
            for (int r = 0; r < repeat; r++) TRY(pc_launch(split, (int64_t)hn, 256, sa, 5, NULL));
            TRY(pc_record(b, NULL)); TRY(pc_sync()); TRY(pc_event_seconds(a, b, &t)); t_split = t / repeat;
            TRY(pc_record(a, NULL));
            for (int r = 0; r < repeat; r++) TRY(pc_launch_warps(project, warps, 128, pa, 8, NULL));
            TRY(pc_record(b, NULL)); TRY(pc_sync()); TRY(pc_event_seconds(a, b, &t)); t_project = t / repeat;
            TRY(pc_record(a, NULL));
            for (int r = 0; r < repeat; r++) TRY(pc_launch(reduce, (int64_t)on, 256, ra, 4, NULL));
            TRY(pc_record(b, NULL)); TRY(pc_sync()); TRY(pc_event_seconds(a, b, &t)); t_reduce = t / repeat;
        }
        TRY(pc_download(out, dout, on * 4, NULL)); TRY(pc_sync());
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
                    scale += fabs((double)from_bf16(c % 2 ? word >> 16 : word & 0xffff) * x[(size_t)t * cols + c]);
                }
                double e = fabs((double)out[(size_t)t * rows + r] - ref[r]) / (scale > 0 ? scale : 1);
                if (e > worst) worst = e;
            }
        }
        double total = t_split + t_project + t_reduce, flops = 2.0 * rows * cols * T;
        int good = worst <= PROJECT_TOLERANCE;
        ok &= good;
        printf("%-9s %5d x %5d x %d, %2d slices: split %.3f + project %.3f + reduce %.3f = %.3f ms, %5.2f TFLOP/s; reference error %.2g of sum|wx| (bound %.2g) %s\n",
               shapes[i].what, rows, cols, T, S, t_split * 1e3, t_project * 1e3, t_reduce * 1e3, total * 1e3, flops / total / 1e12, worst, PROJECT_TOLERANCE, good ? "ok" : "FAIL");
        pc_free(dw); pc_free(dx); pc_free(dh); pc_free(dl); pc_free(dp); pc_free(dout);
        free(w); free(x); free(out); free(ref); free(work);
    }
    return ok;
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: device-warp WARP.cu [PROJECT.cu]\n"); return 2; }
    PcDeviceInfo info;
    PcStatus s = pc_open(&info);
    if (s == PC_ABSENT) { printf("SKIP: no CUDA device (%s)\n", pc_log()); return 0; }
    if (s) { fprintf(stderr, "%s\n", pc_log()); return 1; }
    FILE *f = fopen(argv[1], "rb");
    if (!f) return 2;
    fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET);
    char *src = malloc((size_t)n);
    if (!src || fread(src, 1, (size_t)n, f) != (size_t)n) return 2;
    fclose(f);
    PcModule module; double seconds; PcKernel tile;
    s = pc_compile(src, (size_t)n, "warp.cu", &module, &seconds);
    warp_module = module;
    if (s == PC_NO_NVRTC) { printf("SKIP: %s\n", pc_log()); return 0; }
    if (s) { fprintf(stderr, "%s\n", pc_log()); return 1; }
    TRY(pc_kernel(module, "dw_mma_tile", &tile));
    enum { ITEMS = 4096 };
    uint32_t *a = malloc(ITEMS * 128 * 4), *b = malloc(ITEMS * 64 * 4);
    float *c = malloc(ITEMS * 128 * 4), *gpu = malloc(ITEMS * 128 * 4), *cpu = malloc(ITEMS * 128 * 4);
    if (!a || !b || !c || !gpu || !cpu) return 2;
    for (int i = 0; i < ITEMS * 128; i++) { a[i] = bf16() | bf16() << 16; c[i] = ((float)(next() % 2001) - 1000.0f) / 1000.0f; }
    for (int i = 0; i < ITEMS * 64; i++) b[i] = bf16() | bf16() << 16;
    PcBuffer da, db, dc, dd;
    TRY(pc_alloc(ITEMS * 128 * 4, &da)); TRY(pc_alloc(ITEMS * 64 * 4, &db)); TRY(pc_alloc(ITEMS * 128 * 4, &dc)); TRY(pc_alloc(ITEMS * 128 * 4, &dd));
    TRY(pc_upload(da, a, ITEMS * 128 * 4, NULL)); TRY(pc_upload(db, b, ITEMS * 64 * 4, NULL)); TRY(pc_upload(dc, c, ITEMS * 128 * 4, NULL));
    PcArg args[] = {{PC_ARG_VIEW, .data = da, .count = ITEMS * 128}, {PC_ARG_VIEW, .data = db, .count = ITEMS * 64},
                    {PC_ARG_VIEW, .data = dc, .count = ITEMS * 128}, {PC_ARG_VIEW, .data = dd, .count = ITEMS * 128}};
    TRY(pc_launch_warps(tile, ITEMS, 128, args, 4, NULL));
    TRY(pc_sync());
    TRY(pc_download(gpu, dd, ITEMS * 128 * 4, NULL));
    TRY(pc_sync());
    for (int i = 0; i < ITEMS; i++)
        dw_mma_tile(i, (coax_v_u32){a, ITEMS * 128}, (coax_v_u32){b, ITEMS * 64}, (coax_v_f32){c, ITEMS * 128}, (coax_v_f32){cpu, ITEMS * 128});
    double worst = 0;
    int64_t exact = 0;
    for (int item = 0; item < ITEMS; item++)
        for (int m = 0; m < 16; m++)
            for (int nn = 0; nn < 8; nn++) {
                int at = item * 128 + 4 * ((m % 8) * 4 + nn / 2) + 2 * (m / 8) + nn % 2;
                double scale = fabs(c[at]);
                for (int k = 0; k < 16; k++) {
                    int la = (m % 8) * 4 + (k % 8) / 2, ra = m / 8 + 2 * (k / 8), lb = nn * 4 + (k % 8) / 2;
                    scale += fabs((double)half(a[item * 128 + 4 * la + ra], k % 2) * half(b[item * 64 + 2 * lb + k / 8], k % 2));
                }
                double e = fabs((double)gpu[at] - cpu[at]) / (scale > 0 ? scale : 1);
                if (e > worst) worst = e;
                if (!memcmp(&gpu[at], &cpu[at], 4)) exact++;
            }
    int ok = worst <= TOLERANCE;
    printf("device: %s; %d warps of m16n8k16 BF16 MMA: %lld of %d outputs bit-identical to the Coax meaning; largest difference %.3g of |C|+sum|AB| (bound %.3g) %s\n",
           info.name, ITEMS, (long long)exact, ITEMS * 128, worst, TOLERANCE, ok ? "ok" : "FAIL");
    if (argc > 2) ok &= projections(argv[2]);
    return ok ? 0 : 1;
}
