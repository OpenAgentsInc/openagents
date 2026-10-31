/* Group kernels (chapter 48) on a GPU against the same Coax compiled to C:
   the tiled product through threadgroup memory and barriers, and a group
   total read by shared code. Each output adds its terms in the same order
   on every target, so results must be bit-identical. Build with
   -DGROUP_METAL (metal-host.m) or without it (cuda-host.c).
   Usage: group-kernels GENERATED_SOURCE. Prints SKIP without a device. */
#include "group-matmul.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef GROUP_METAL
#include "metal-host.h"
#define BACKEND "Metal"
#else
#include "cuda-host.h"
#define BACKEND "CUDA"
#endif

static char *read_all(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *t = malloc((size_t)size + 1);
    if (!t || fread(t, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); t[size] = 0; *n = (size_t)size; return t;
}
static uint64_t seed = 0x2545f4914f6cdd1dULL;
static float uniform(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return (float)((seed >> 11) % 2000001) / 1000000.0f - 1.0f; }

/* Device buffers and launches behind one small interface. */
typedef struct { void *host; size_t bytes;
#ifdef GROUP_METAL
    PmBuffer b;
#else
    PcBuffer b;
#endif
} Buf;
#ifdef GROUP_METAL
#define TRY(x) do { PmStatus s_ = (x); if (s_ != PM_OK) { fprintf(stderr, "%s: %s\n", #x, pm_log()); exit(1); } } while (0)
static PmModule module;
static Buf buf(size_t bytes) { Buf r = {0}; TRY(pm_alloc(bytes, &r.b)); r.host = pm_contents(r.b); r.bytes = bytes; return r; }
static void put(Buf *b) { (void)b; }
static void get(Buf *b) { (void)b; }
typedef PmKernel Kernel;
static Kernel kernel(const char *name) { PmKernel k; TRY(pm_kernel(module, name, &k)); return k; }
static PmArg view(Buf *b, int64_t count) { return (PmArg){PM_ARG_VIEW, 0, 0, b->b, 0, count}; }
static PmArg integer(int64_t v) { return (PmArg){PM_ARG_INT, v, 0, NULL, 0, 0}; }
static int launch(Kernel k, int64_t items, int threads, PmArg *args, int count, double *seconds) {
    PmBatch b; TRY(pm_begin(&b)); TRY(pm_launch(b, k, items, threads, threads, args, count));
    return (int)pm_wait(b, seconds);
}
#else
#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
static PcModule module;
static Buf buf(size_t bytes) { Buf r = {0}; TRY(pc_alloc(bytes, &r.b)); r.host = calloc(1, bytes); r.bytes = bytes; TRY(pc_upload(r.b, r.host, bytes, NULL)); return r; }
static void put(Buf *b) { TRY(pc_upload(b->b, b->host, b->bytes, NULL)); }
static void get(Buf *b) { TRY(pc_download(b->host, b->b, b->bytes, NULL)); }
typedef PcKernel Kernel;
static Kernel kernel(const char *name) { PcKernel k; TRY(pc_kernel(module, name, &k)); return k; }
static PcArg view(Buf *b, int64_t count) { return (PcArg){PC_ARG_VIEW, 0, 0, b->b, count}; }
static PcArg integer(int64_t v) { return (PcArg){PC_ARG_INT, v, 0, 0, 0}; }
static int launch(Kernel k, int64_t items, int threads, PcArg *args, int count, double *seconds) {
    PcEvent a, b; TRY(pc_event(&a)); TRY(pc_event(&b));
    TRY(pc_record(a, NULL));
    TRY(pc_launch_warps(k, items * threads / 32, threads, args, count, NULL));
    TRY(pc_record(b, NULL)); TRY(pc_sync());
    double t = 0; TRY(pc_event_seconds(a, b, &t)); if (seconds) *seconds = t;
    pc_destroy_event(a); pc_destroy_event(b);
    return 0;
}
#endif

int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: group-kernels GENERATED_SOURCE\n"); return 2; }
#ifdef GROUP_METAL
    PmDeviceInfo info;
    if (pm_open(&info) == PM_ABSENT) { printf("SKIP: no Metal device\n"); return 0; }
    printf("device: %s (Apple GPU family %d)\n", info.name, info.apple_family);
#else
    PcDeviceInfo info;
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    printf("device: %s (sm_%d%d)\n", info.name, info.major, info.minor);
#endif
    size_t n; char *src = read_all(argv[1], &n); double seconds = 0;
#ifdef GROUP_METAL
    TRY(pm_compile(src, n, "group-matmul.metal", &module, &seconds));
#else
    TRY(pc_compile(src, n, "group-matmul.cu", &module, &seconds));
#endif
    Kernel k_matmul = kernel("gm_matmul"), k_total = kernel("gm_total");
    int failures = 0;
    int shapes[][3] = {{32, 32, 32}, {37, 45, 70}, {128, 96, 256}, {1, 1, 1}, {200, 33, 129}, {512, 512, 1024}};
    for (size_t s = 0; s < sizeof shapes / sizeof *shapes; s++) {
        int m = shapes[s][0], nn = shapes[s][1], k = shapes[s][2];
        Buf a = buf((size_t)m * k * 4), b = buf((size_t)k * nn * 4), c = buf((size_t)m * nn * 4);
        float *ah = a.host, *bh = b.host, *ch = c.host, *ref = calloc((size_t)m * nn, 4);
        for (int i = 0; i < m * k; i++) ah[i] = uniform();
        for (int i = 0; i < k * nn; i++) bh[i] = uniform();
        for (int i = 0; i < m * nn; i++) ch[i] = NAN;
        put(&a); put(&b); put(&c);
        int64_t items = (int64_t)((m + 31) / 32) * ((nn + 31) / 32);
        for (int64_t item = 0; item < items; item++)
            gm_matmul(item, m, nn, k, (coax_v_f32){ah, (int64_t)m * k}, (coax_v_f32){bh, (int64_t)k * nn}, (coax_v_f32){ref, (int64_t)m * nn});
#ifdef GROUP_METAL
        PmArg args[] = {integer(m), integer(nn), integer(k), view(&a, (int64_t)m * k), view(&b, (int64_t)k * nn), view(&c, (int64_t)m * nn)};
#else
        PcArg args[] = {integer(m), integer(nn), integer(k), view(&a, (int64_t)m * k), view(&b, (int64_t)k * nn), view(&c, (int64_t)m * nn)};
#endif
        int st = launch(k_matmul, items, 128, args, 6, &seconds);
        get(&c);
        int64_t differ = 0;
        for (int i = 0; i < m * nn; i++) if (memcmp(&ch[i], &ref[i], 4)) differ++;
        printf("gm_matmul %4dx%-4dx%-5d %6lld items: %lld of %d outputs differ from the CPU (status %d, %.3f ms)\n",
               m, nn, k, (long long)items, (long long)differ, m * nn, st, seconds * 1e3);
        failures += differ != 0 || st != 0;
        free(ref);
    }
    {
        int count = 1000;
        Buf x = buf((size_t)count * 4), out = buf((size_t)count * 4);
        float *xh = x.host, *oh = out.host, *ref = calloc((size_t)count, 4);
        for (int i = 0; i < count; i++) { xh[i] = uniform() + 2.0f; oh[i] = NAN; }
        put(&x); put(&out);
        int64_t items = (count + 255) / 256;
        for (int64_t item = 0; item < items; item++) gm_total(item, count, (coax_v_f32){xh, count}, (coax_v_f32){ref, count});
#ifdef GROUP_METAL
        PmArg args[] = {integer(count), view(&x, count), view(&out, count)};
#else
        PcArg args[] = {integer(count), view(&x, count), view(&out, count)};
#endif
        int st = launch(k_total, items, 128, args, 3, NULL);
        get(&out);
        int64_t differ = 0;
        for (int i = 0; i < count; i++) if (memcmp(&oh[i], &ref[i], 4)) differ++;
        printf("gm_total %d values: %lld differ from the CPU (status %d)\n", count, (long long)differ, st);
        failures += differ != 0 || st != 0;
#ifdef GROUP_METAL
        /* A group kernel launched with the wrong group size voids the launch. */
        PmBatch b; TRY(pm_begin(&b)); TRY(pm_launch(b, k_total, items * 2, 32, 64, args, 3));
        PmStatus w = pm_wait(b, NULL);
        printf("gm_total launched with 64-thread groups: %s\n", w == PM_CHECK ? "refused (PM_CHECK)" : "NOT refused");
        failures += w != PM_CHECK;
#endif
        free(ref);
    }
    if (failures) { printf("FAIL: %d group-kernel checks\n", failures); return 1; }
    printf("group kernels on %s: bit-identical to the CPU\n", BACKEND);
    return 0;
}
