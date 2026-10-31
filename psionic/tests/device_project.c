/* Chapter 36: the Coax projection kernel on a CUDA device, against the same
   Coax kernel compiled to C and against the CPU reference profile.
   Usage: device-project KERNEL.cu [quick]
   Exits 0 with a SKIP line when no device or NVRTC is present. */
#include "cuda-host.h"
#include "project.h"
#include "qwen-matvec.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t seed = 0x9e3779b97f4a7c15ULL;
static uint32_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return (uint32_t)(seed >> 16); }
/* A finite BF16 value of magnitude about 2^-8 .. 2^0, either sign. */
static uint16_t random_bf16(void) {
    uint32_t r = next();
    uint32_t exponent = 119 + (r % 8), mantissa = (r >> 3) & 0x7f, sign = (r >> 10) & 1;
    return (uint16_t)((sign << 15) | (exponent << 7) | mantissa);
}
static float random_x(void) { return ((float)(next() % 2000001) - 1000000.0f) / 1000000.0f; }
static float bf16(uint16_t h) { uint32_t b = (uint32_t)h << 16; float f; memcpy(&f, &b, 4); return f; }

static char *read_file(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *text = malloc((size_t)size + 1);
    if (!text || fread(text, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(text); return NULL; }
    fclose(f);
    text[size] = 0;
    *n = (size_t)size;
    return text;
}
#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: status %d: %s\n", #x, s_, pc_log()); exit(1); } } while (0)

typedef struct { int64_t rows, cols, tokens, slices; const char *what; } Shape;

/* Frozen before measuring: |device - reference| <= 2^-15 * sum |w x| for the
   CPU-f32-wide-v1 reference (the kernel's own worst case is under that bound);
   the same kernels compiled to C must agree bit for bit. */
static const double TOLERANCE = 1.0 / 32768.0;
static PcKernel k_project, k_matvec, k_reduce;

/* One projection on the device: dv_project (and dv_reduce when sliced), or
   dv_matvec and dv_reduce for one token. Slices 0 means the matvec path. */
static void device_run(Shape s, PcBuffer dw, PcBuffer dx, PcBuffer dout, PcBuffer dpart, int64_t parts) {
    int64_t groups = s.cols / 4, on = s.tokens * s.rows;
    PcArg view_w = {PC_ARG_VIEW, .data = dw, .count = s.rows * groups}, view_x = {PC_ARG_VIEW, .data = dx, .count = s.tokens * s.cols};
    PcArg view_out = {PC_ARG_VIEW, .data = dout, .count = on}, view_part = {PC_ARG_VIEW, .data = dpart, .count = parts};
    if (s.slices == 0) {
        PcArg a[] = {{PC_ARG_INT, s.rows}, {PC_ARG_INT, s.cols}, view_w, view_x, view_part};
        TRY(pc_launch(k_matvec, s.rows * 16, 256, a, 5, NULL));
        PcArg b[] = {{PC_ARG_INT, on}, {PC_ARG_INT, 16}, view_part, view_out};
        TRY(pc_launch(k_reduce, on, 256, b, 4, NULL));
        return;
    }
    int64_t items = s.rows / 4 * ((s.tokens + 7) / 8) * s.slices; /* dv_project_items */
    PcArg a[] = {{PC_ARG_INT, s.rows}, {PC_ARG_INT, s.cols}, {PC_ARG_INT, s.tokens}, {PC_ARG_INT, s.slices}, view_w, view_x,
                 s.slices == 1 ? view_out : view_part};
    TRY(pc_launch(k_project, items, 128, a, 7, NULL));
    if (s.slices > 1) {
        PcArg b[] = {{PC_ARG_INT, on}, {PC_ARG_INT, s.slices}, view_part, view_out};
        TRY(pc_launch(k_reduce, on, 256, b, 4, NULL));
    }
}

static int qualify(Shape s, int timing) {
    int64_t groups = s.cols / 4, wn = s.rows * s.cols, xn = s.tokens * s.cols, on = s.tokens * s.rows;
    int64_t parts = on * (s.slices == 0 ? 16 : s.slices);
    uint16_t *w = malloc((size_t)wn * 2);
    float *x = malloc((size_t)xn * 4), *gpu = malloc((size_t)on * 4), *cpu = malloc((size_t)on * 4), *part = malloc((size_t)parts * 4);
    if (!w || !x || !gpu || !cpu || !part) { fprintf(stderr, "out of host memory\n"); exit(2); }
    for (int64_t i = 0; i < wn; i++) w[i] = random_bf16();
    for (int64_t i = 0; i < xn; i++) x[i] = random_x();
    PcBuffer dw, dx, dout, dpart;
    TRY(pc_alloc((size_t)wn * 2, &dw));
    TRY(pc_alloc((size_t)xn * 4, &dx));
    TRY(pc_alloc((size_t)on * 4, &dout));
    TRY(pc_alloc((size_t)parts * 4, &dpart));
    TRY(pc_upload(dw, w, (size_t)wn * 2, NULL));
    TRY(pc_upload(dx, x, (size_t)xn * 4, NULL));
    TRY(pc_fill(dout, 0xffffffffu, (size_t)on, NULL));
    device_run(s, dw, dx, dout, dpart, parts);
    TRY(pc_sync());
    TRY(pc_download(gpu, dout, (size_t)on * 4, NULL));
    TRY(pc_sync());
    /* The same Coax kernels, compiled to C, over every work item. */
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
    /* The CPU reference profile on a sample of tokens (it is slow). */
    double worst = 0;
    int64_t sampled = 0;
    float *ref = malloc((size_t)s.rows * 4), *work = malloc((size_t)s.rows * 4);
    for (int64_t t = 0; t < s.tokens; t += (s.tokens > 4 ? s.tokens / 4 : 1)) {
        /* The reference admits at most 32768 rows at a time, as qx_block's readout does. */
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
        sampled++;
    }
    int ok = differ == 0 && worst <= TOLERANCE;
    printf("%-30s %6lldx%-5lld x %3lld tokens, %s %2lld: same kernels on the CPU differ in %lld of %lld; reference error %.2g of sum|wx| (bound %.2g) %s\n",
           s.what, (long long)s.rows, (long long)s.cols, (long long)s.tokens, s.slices ? "slices" : "lanes ",
           (long long)(s.slices ? s.slices : 16), (long long)differ, (long long)on, worst, TOLERANCE, ok ? "ok" : "FAIL");
    if (timing) {
        PcEvent a, b;
        TRY(pc_event(&a));
        TRY(pc_event(&b));
        int repeat = 20;
        device_run(s, dw, dx, dout, dpart, parts);
        TRY(pc_record(a, NULL));
        for (int i = 0; i < repeat; i++) device_run(s, dw, dx, dout, dpart, parts);
        TRY(pc_record(b, NULL));
        TRY(pc_sync());
        double seconds;
        TRY(pc_event_seconds(a, b, &seconds));
        seconds /= repeat;
        double bytes = (double)wn * 2 + (double)xn * 4 + (double)on * 4, flops = 2.0 * (double)wn * (double)s.tokens;
        printf("%-30s %8.3f ms  %6.1f GB/s of weights and activations  %5.2f TFLOP/s\n", "", seconds * 1e3, bytes / seconds / 1e9, flops / seconds / 1e12);
    }
    pc_free(dw); pc_free(dx); pc_free(dout); pc_free(dpart);
    free(w); free(x); free(gpu); free(cpu); free(part); free(ref); free(work);
    return ok;
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: device-project KERNEL.cu [quick]\n"); return 2; }
    PcDeviceInfo info;
    PcStatus s = pc_open(&info);
    if (s == PC_ABSENT) { printf("SKIP: no CUDA device (%s)\n", pc_log()); return 0; }
    if (s) { fprintf(stderr, "pc_open: %s\n", pc_log()); return 1; }
    printf("device: %s, compute %d.%d, %d SMs, driver %d, %.2f of %.2f GiB free\n", info.name, info.major, info.minor, info.sms,
           info.driver_version, info.free_bytes / 1073741824.0, info.total_bytes / 1073741824.0);
    size_t n;
    char *source = read_file(argv[1], &n);
    if (!source) { fprintf(stderr, "cannot read %s\n", argv[1]); return 2; }
    PcModule module;
    double seconds;
    s = pc_compile(source, n, "project.cu", &module, &seconds);
    if (s == PC_NO_NVRTC) { printf("SKIP: %s\n", pc_log()); return 0; }
    if (s) { fprintf(stderr, "compile: %s\n", pc_log()); return 1; }
    printf("NVRTC compiled and loaded the generated kernel in %.3f s\n", seconds);
    TRY(pc_kernel(module, "dv_project", &k_project));
    TRY(pc_kernel(module, "dv_matvec", &k_matvec));
    TRY(pc_kernel(module, "dv_reduce", &k_reduce));
    int quick = argc > 2 && !strcmp(argv[2], "quick"), ok = 1;
    Shape small[] = {{4, 4, 1, 1, "smallest"}, {16, 260, 9, 1, "ragged tokens"}, {64, 1024, 17, 2, "two slices"},
                     {32, 4096, 1, 0, "one token, lanes"}, {64, 12288, 3, 3, "three slices"}};
    for (size_t i = 0; i < sizeof small / sizeof *small; i++) ok &= qualify(small[i], 0);
    if (!quick) {
        Shape elephant[] = {{4096, 4096, 1, 0, "z / out projection, one token"}, {12288, 4096, 1, 0, "gate / up, one token"},
                            {4096, 12288, 1, 0, "down, one token"}, {248320, 4096, 1, 0, "vocabulary readout, one token"},
                            {8192, 4096, 86, 1, "qkv, 86 tokens"}, {8192, 4096, 86, 8, "qkv, 86 tokens"},
                            {12288, 4096, 86, 8, "gate / up, 86 tokens"}, {4096, 12288, 86, 1, "down, 86 tokens"},
                            {4096, 12288, 86, 16, "down, 86 tokens"}, {4096, 4096, 86, 16, "z / out, 86 tokens"},
                            {1024, 4096, 86, 16, "k / v, 86 tokens"}, {32, 4096, 86, 16, "a / b, 86 tokens"}};
        for (size_t i = 0; i < sizeof elephant / sizeof *elephant; i++) ok &= qualify(elephant[i], 1);
    }
    TRY(pc_unload(module));
    /* Completion and lifetime: the launches above completed before their
       buffers were freed (each qualify synchronizes first). A bounds check
       that fails on the device traps the launch, and the host sees it at the
       next completion point. The context is unusable afterwards, so this is
       the last step. */
    if (ok) {
        TRY(pc_compile(source, n, "project.cu", &module, &seconds));
        TRY(pc_kernel(module, "dv_reduce", &k_reduce));
        PcBuffer small;
        TRY(pc_alloc(64 * 4, &small));
        PcArg bad[] = {{PC_ARG_INT, 64}, {PC_ARG_INT, 2}, {PC_ARG_VIEW, .data = small, .count = 64}, {PC_ARG_VIEW, .data = small, .count = 64}};
        PcStatus launched = pc_launch(k_reduce, 64, 64, bad, 4, NULL); /* slice 1 of 2 reads past its view */
        PcStatus finished = launched == PC_OK ? pc_sync() : launched;
        if (finished == PC_DRIVER) printf("an out-of-bounds read trapped the launch: %s\n", pc_log());
        else { printf("an out-of-bounds read was not reported (status %d)\n", finished); ok = 0; }
    }
    puts(ok ? "device projection qualifies" : "device projection FAILED");
    return ok ? 0 : 1;
}
