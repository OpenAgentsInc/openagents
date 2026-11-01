/* Chapter 68 on a CUDA device: the group kernels of
   psionic/lib/device/cooperate.coax against the same Coax compiled to C, bit
   for bit, after psionic/lib/device/admit.coax admits each kernel for the
   device the host reports. The host only reports attributes and launches;
   the admission decision is Coax. Usage: cooperate-device GENERATED.cu.
   Prints SKIP without a CUDA device or NVRTC. */
#include "cooperate.h"
#include "admit.h"
#include "cuda-host.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)

static char *read_all(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *t = malloc((size_t)size + 1);
    if (!t || fread(t, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); t[size] = 0; *n = (size_t)size; return t;
}
static uint64_t seed = 0x9e3779b97f4a7c15ULL;
static uint64_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return seed; }
static float uniform(void) { return (float)((next() >> 11) % 2000001) / 1000000.0f - 1.0f; }
static uint32_t bf16_of(float v) { uint32_t b; memcpy(&b, &v, 4); return b >> 16; }
static float of_bf16(uint32_t h) { uint32_t b = h << 16; float v; memcpy(&v, &b, 4); return v; }

typedef struct { void *host; size_t bytes; PcBuffer b; } Buf;
static Buf buf(size_t bytes) { Buf r = {0}; TRY(pc_alloc(bytes, &r.b)); r.host = calloc(1, bytes); r.bytes = bytes; return r; }
static void put(Buf *b) { TRY(pc_upload(b->b, b->host, b->bytes, NULL)); }
static void get(Buf *b) { TRY(pc_download(b->host, b->b, b->bytes, NULL)); }
static void drop(Buf *b) { pc_free(b->b); free(b->host); }
static PcArg view(Buf *b, int64_t count) { return (PcArg){PC_ARG_VIEW, 0, 0, b->b, count}; }
static PcArg integer(int64_t v) { return (PcArg){PC_ARG_INT, v, 0, 0, 0}; }
static double launch(PcKernel k, int64_t groups, int threads, PcArg *args, int count) {
    PcEvent a, b; TRY(pc_event(&a)); TRY(pc_event(&b));
    TRY(pc_record(a, NULL));
    TRY(pc_launch_warps(k, groups * threads / 32, threads, args, count, NULL));
    TRY(pc_record(b, NULL)); TRY(pc_sync());
    double t = 0; TRY(pc_event_seconds(a, b, &t));
    pc_destroy_event(a); pc_destroy_event(b);
    return t;
}
static int64_t differ(const float *a, const float *b, int64_t n) {
    int64_t d = 0;
    for (int64_t i = 0; i < n; i++) if (memcmp(&a[i], &b[i], 4)) d++;
    return d;
}

static const char *names[] = {"dc_tile_sum", "dc_butterfly", "dc_bf16_rows", "dw_mma_tile"};

int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: cooperate-device GENERATED.cu\n"); return 2; }
    PcDeviceInfo info;
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    printf("device: %s (sm_%d%d, %d threads and %d shared bytes per block)\n",
           info.name, info.major, info.minor, info.max_threads, info.max_shared);
    int admitted[4], failures = 0;
    for (int k = 0; k < 4; k++) {
        admitted[k] = da_admit_kernel(k, info.major, info.minor, info.max_threads, info.max_shared) == 0;
        printf("admission %-13s %s\n", names[k], admitted[k] ? "admitted" : "refused");
    }
    /* The same decision for a reported compute capability 7.5 device. */
    int old = (int)da_admit_kernel(3, 7, 5, 1024, 49152);
    printf("admission dw_mma_tile on a reported sm_75: status %d (5: needs compute capability 8.0)\n", old);
    failures += old != 5;

    size_t n; char *src = read_all(argv[1], &n); double seconds = 0;
    PcModule module; TRY(pc_compile(src, n, "cooperate.cu", &module, &seconds));
    PcKernel k_sum, k_fly, k_rows;
    TRY(pc_kernel(module, "dc_tile_sum", &k_sum));
    TRY(pc_kernel(module, "dc_butterfly", &k_fly));
    TRY(pc_kernel(module, "dc_bf16_rows", &k_rows));

    if (admitted[0]) {
        int counts[] = {1, 255, 256, 257, 100000, 1 << 22};
        for (size_t c = 0; c < sizeof counts / sizeof *counts; c++) {
            int64_t count = counts[c], groups = (count + 255) / 256;
            Buf x = buf((size_t)count * 4), s = buf((size_t)groups * 4);
            float *xh = x.host, *sh = s.host, *ref = calloc((size_t)groups, 4);
            for (int64_t i = 0; i < count; i++) xh[i] = uniform();
            for (int64_t i = 0; i < groups; i++) sh[i] = NAN;
            put(&x); put(&s);
            for (int64_t g = 0; g < groups; g++) dc_tile_sum(g, count, (coax_v_f32){xh, count}, (coax_v_f32){ref, groups});
            PcArg args[] = {integer(count), view(&x, count), view(&s, groups)};
            double t = launch(k_sum, groups, 128, args, 3);
            get(&s);
            int64_t d = differ(sh, ref, groups);
            printf("dc_tile_sum  %8lld values %6lld groups: %lld differ from the CPU (%.3f ms)\n",
                   (long long)count, (long long)groups, (long long)d, t * 1e3);
            failures += d != 0;
            drop(&x); drop(&s); free(ref);
        }
    }
    if (admitted[1]) {
        int counts[] = {1, 31, 32, 33, 10000, 1 << 20};
        for (size_t c = 0; c < sizeof counts / sizeof *counts; c++) {
            int64_t count = counts[c], groups = (count + 31) / 32;
            Buf x = buf((size_t)count * 4), out = buf((size_t)count * 4);
            float *xh = x.host, *oh = out.host, *ref = calloc((size_t)count, 4);
            for (int64_t i = 0; i < count; i++) { xh[i] = uniform(); oh[i] = NAN; }
            put(&x); put(&out);
            for (int64_t g = 0; g < groups; g++) dc_butterfly(g, count, (coax_v_f32){xh, count}, (coax_v_f32){ref, count});
            PcArg args[] = {integer(count), view(&x, count), view(&out, count)};
            double t = launch(k_fly, groups, 32, args, 3);
            get(&out);
            int64_t d = differ(oh, ref, count), uneven = 0;
            for (int64_t i = 0; i < count; i++) if (memcmp(&oh[i], &oh[i / 32 * 32], 4)) uneven++;
            printf("dc_butterfly %8lld values %6lld groups: %lld differ from the CPU, %lld lanes differ from lane 0 (%.3f ms)\n",
                   (long long)count, (long long)groups, (long long)d, (long long)uneven, t * 1e3);
            failures += d != 0 || uneven != 0;
            drop(&x); drop(&out); free(ref);
        }
    }
    if (admitted[2]) {
        int shapes[][2] = {{1, 2}, {130, 262}, {1000, 1002}, {4096, 4096}, {2880, 2880}};
        for (size_t s = 0; s < sizeof shapes / sizeof *shapes; s++) {
            int64_t rows = shapes[s][0], cols = shapes[s][1], words = cols / 2, groups = (rows + 127) / 128;
            Buf w = buf((size_t)(rows * words) * 4), x = buf((size_t)cols * 4), out = buf((size_t)rows * 4);
            uint32_t *wh = w.host; float *xh = x.host, *oh = out.host, *ref = calloc((size_t)rows, 4);
            for (int64_t i = 0; i < rows * words; i++) wh[i] = bf16_of(uniform()) | (bf16_of(uniform()) << 16);
            for (int64_t i = 0; i < cols; i++) xh[i] = uniform();
            for (int64_t i = 0; i < rows; i++) oh[i] = NAN;
            put(&w); put(&x); put(&out);
            for (int64_t g = 0; g < groups; g++)
                dc_bf16_rows(g, rows, cols, (coax_v_u32){wh, rows * words}, (coax_v_f32){xh, cols}, (coax_v_f32){ref, rows});
            PcArg args[] = {integer(rows), integer(cols), view(&w, rows * words), view(&x, cols), view(&out, rows)};
            double t = launch(k_rows, groups, 128, args, 5);
            get(&out);
            int64_t d = differ(oh, ref, rows);
            /* Error against an f64 sum, as a fraction of sum |w x|; the frozen
               bound is cols * 2^-24. */
            double worst = 0;
            for (int64_t r = 0; r < rows; r++) {
                double exact = 0, mag = 0;
                for (int64_t p = 0; p < words; p++) {
                    double a = of_bf16(wh[r * words + p] & 0xffff), b = of_bf16(wh[r * words + p] >> 16);
                    exact += a * xh[2 * p] + b * xh[2 * p + 1];
                    mag += fabs(a * xh[2 * p]) + fabs(b * xh[2 * p + 1]);
                }
                double e = mag > 0 ? fabs(oh[r] - exact) / mag : 0;
                if (e > worst) worst = e;
            }
            int over = worst > (double)cols / 16777216.0;
            printf("dc_bf16_rows %5lldx%-5lld %4lld groups: %lld differ from the CPU, worst error %.2e of sum |w x| (bound %.2e) (%.3f ms)\n",
                   (long long)rows, (long long)cols, (long long)groups, (long long)d, worst, (double)cols / 16777216.0, t * 1e3);
            failures += d != 0 || over;
            drop(&w); drop(&x); drop(&out); free(ref);
        }
    }
    if (failures) { printf("FAIL: %d checks\n", failures); return 1; }
    printf("chapter 68 group kernels on CUDA: admitted, and bit-identical to the CPU\n");
    return 0;
}
