/* The Metal host's own contract (platform/metal-host.h) on a real device,
   with the minimum compute consumer (fixtures/compute/consumer.coax): its
   result against the same Coax compiled to C, every argument and launch
   refusal, a voided launch, an empty launch, and transfer measured apart
   from compute. Usage: metal-host CONSUMER.metal. SKIP without a GPU. */
#include "metal-host.h"
#include "consumer.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#define TRY(x) do { PmStatus s_ = (x); if (s_ != PM_OK) { fprintf(stderr, "%s: status %d: %s\n", #x, s_, pm_log()); exit(1); } } while (0)
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }
static PmArg I(int64_t v) { return (PmArg){PM_ARG_INT, v, 0, NULL, 0, 0}; }
static PmArg V(PmBuffer b, int64_t offset, int64_t count) { return (PmArg){PM_ARG_VIEW, 0, 0, b, offset, count}; }
static PmBuffer alloc(size_t bytes) { PmBuffer b; TRY(pm_alloc(bytes, &b)); return b; }
static uint64_t seed = 0x2545f4914f6cdd1dULL;
static uint32_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return (uint32_t)(seed >> 16); }
/* A launch the host must refuse before anything is encoded. */
static void refuse(PmKernel k, int64_t items, int lanes, const PmArg *args, int count, const char *what) {
    PmBatch b; TRY(pm_begin(&b));
    PmStatus s = pm_launch(b, k, items, lanes, 0, args, count);
    if (s != PM_ARGUMENT) { fprintf(stderr, "%s: status %d, want PM_ARGUMENT\n", what, s); exit(1); }
    TRY(pm_wait(b, NULL)); /* nothing was encoded: the batch completes clean */
}
int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: metal-host CONSUMER.metal\n"); return 2; }
    PmDeviceInfo info;
    if (pm_open(&info) == PM_ABSENT) { puts("SKIP: no Metal device"); return 0; }
    FILE *f = fopen(argv[1], "rb"); if (!f) return 2;
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *text = malloc((size_t)size); if (!text || fread(text, 1, (size_t)size, f) != (size_t)size) return 2; fclose(f);
    PmModule m; PmKernel matvec, reduce; double compile_s;
    TRY(pm_compile(text, (size_t)size, "consumer", &m, &compile_s)); free(text);
    TRY(pm_kernel(m, "cc_matvec", &matvec)); TRY(pm_kernel(m, "cc_reduce", &reduce));
    PmKernel none; assert(pm_kernel(m, "cc_absent", &none) == PM_ARGUMENT);
    PmModule bad; assert(pm_compile("kernel void k(", 14, "broken", &bad, NULL) == PM_COMPILE);
    printf("%s, unified memory %d: compiled the consumer in %.3f s; a missing kernel and broken source refuse\n", info.name, info.unified_memory, compile_s);

    /* The result: rows x cols BF16 weights times f32 x, against the C build. */
    enum { ROWS = 256, COLS = 4096, SLICES = 8 };
    PmBuffer w = alloc((size_t)ROWS * COLS / 4 * 8), x = alloc(COLS * 4), part = alloc(ROWS * SLICES * 4), out = alloc(ROWS * 4);
    uint64_t *wv = pm_contents(w); float *xv = pm_contents(x);
    for (int64_t i = 0; i < (int64_t)ROWS * COLS / 4; i++) {
        uint64_t bits = 0;
        for (int j = 0; j < 4; j++) { uint32_t r = next(); bits |= (uint64_t)((((r >> 10) & 1) << 15) | ((119 + r % 8) << 7) | ((r >> 3) & 0x7f)) << (16 * j); }
        wv[i] = bits;
    }
    for (int i = 0; i < COLS; i++) xv[i] = ((float)(next() % 2001) - 1000.0f) / 1000.0f;
    PmArg mv[] = {I(ROWS), I(COLS), I(SLICES), V(w, 0, (int64_t)ROWS * COLS / 4), V(x, 0, COLS), V(part, 0, ROWS * SLICES)};
    PmArg rd[] = {I(ROWS), I(SLICES), V(part, 0, ROWS * SLICES), V(out, 0, ROWS)};
    PmBatch b; double gpu;
    TRY(pm_begin(&b)); TRY(pm_launch(b, matvec, ROWS * SLICES, 1, 0, mv, 6)); TRY(pm_launch(b, reduce, ROWS, 1, 0, rd, 4)); TRY(pm_wait(b, &gpu));
    float *cpu_part = malloc(ROWS * SLICES * 4), *cpu = malloc(ROWS * 4);
    coax_v_u64 cw = {wv, (int64_t)ROWS * COLS / 4}; coax_v_f32 cx = {xv, COLS}, cp = {cpu_part, ROWS * SLICES}, co = {cpu, ROWS};
    for (int64_t i = 0; i < ROWS * SLICES; i++) cc_matvec(i, ROWS, COLS, SLICES, cw, cx, cp);
    for (int64_t i = 0; i < ROWS; i++) cc_reduce(i, ROWS, SLICES, cp, co);
    int differ = 0; float *gv = pm_contents(out);
    for (int i = 0; i < ROWS; i++) differ += memcmp(&gv[i], &cpu[i], 4) != 0;
    printf("consumer %dx%d, %d slices: %d of %d outputs differ from the C build; GPU %.3f ms\n", ROWS, COLS, SLICES, differ, ROWS, 1e3 * gpu);
    if (differ) return 1;

    /* Binding and launch refusals: nothing is encoded, the batch stays usable. */
    PmArg a[6]; memcpy(a, mv, sizeof a);
    a[3].buffer = NULL; refuse(matvec, 8, 1, a, 6, "a view without a buffer"); memcpy(a, mv, sizeof a);
    a[4].offset = -4; refuse(matvec, 8, 1, a, 6, "a negative offset"); memcpy(a, mv, sizeof a);
    a[4].offset = COLS * 4 + 1; refuse(matvec, 8, 1, a, 6, "an offset past the end"); memcpy(a, mv, sizeof a);
    a[4].count = -1; refuse(matvec, 8, 1, a, 6, "a negative count"); memcpy(a, mv, sizeof a);
    a[4].count = COLS * 4 + 1; refuse(matvec, 8, 1, a, 6, "a view longer than its buffer"); memcpy(a, mv, sizeof a);
    a[4].offset = COLS * 4; a[4].count = 1; refuse(matvec, 8, 1, a, 6, "a view starting at the end"); memcpy(a, mv, sizeof a);
    a[0].kind = (PmArgKind)9; refuse(matvec, 8, 1, a, 6, "an unknown argument kind"); memcpy(a, mv, sizeof a);
    refuse(matvec, -1, 1, a, 6, "negative items"); refuse(matvec, 8, 33, a, 6, "33 lanes"); refuse(matvec, 8, 2048, a, 6, "2048 lanes");
    PmArg many[17]; for (int i = 0; i < 17; i++) many[i] = I(0); refuse(matvec, 8, 1, many, 17, "17 arguments");
    puts("binding: null, negative, past-the-end and over-long views, unknown kinds and malformed launches refuse before encoding");

    /* An empty launch does nothing; a launch whose Coax checks fail is void. */
    float before[ROWS]; memcpy(before, gv, sizeof before);
    TRY(pm_begin(&b)); TRY(pm_launch(b, reduce, 0, 1, 0, rd, 4)); TRY(pm_wait(b, NULL)); assert(!memcmp(before, gv, sizeof before));
    PmArg short_part[] = {I(ROWS), I(SLICES + 1), V(part, 0, ROWS * SLICES), V(out, 0, ROWS)};
    TRY(pm_begin(&b)); TRY(pm_launch(b, reduce, ROWS, 1, 0, short_part, 4));
    PmStatus voided = pm_wait(b, NULL);
    if (voided != PM_CHECK) { fprintf(stderr, "an out-of-bounds read: status %d, want PM_CHECK\n", voided); return 1; }
    printf("an empty launch writes nothing; an out-of-bounds read voids its batch (%s)\n", pm_log());

    /* Transfer apart from compute. Shared storage: the CPU writes the GPU's
       own bytes, so a transfer is a CPU copy into the buffer, timed here. */
    size_t big = (size_t)64 << 20; char *host = malloc(big); memset(host, 7, big);
    PmBuffer stage = alloc(big); double t0 = now(); memcpy(pm_contents(stage), host, big); double up = now() - t0;
    memset(host, 0, big); t0 = now(); memcpy(host, pm_contents(stage), big); double down = now() - t0;
    uint64_t sum = 0; for (size_t i = 0; i < big; i += 4096) sum += (unsigned char)host[i]; if (sum != 7 * (big / 4096)) return 1;
    TRY(pm_begin(&b)); TRY(pm_launch(b, matvec, ROWS * SLICES, 1, 0, mv, 6)); TRY(pm_launch(b, reduce, ROWS, 1, 0, rd, 4));
    double t1 = now(); TRY(pm_wait(b, &gpu)); double wall = now() - t1;
    printf("transfer: 64 MiB into shared storage %.2f ms (%.1f GB/s), out %.2f ms; compute: GPU %.3f ms, commit to completion %.3f ms\n",
           1e3 * up, (double)big / up / 1e9, 1e3 * down, 1e3 * gpu, 1e3 * wall);
    pm_free(stage); free(host); pm_free(w); pm_free(x); pm_free(part); pm_free(out);
    pm_kernel_free(matvec); pm_kernel_free(reduce); pm_unload(m); free(cpu_part); free(cpu);
    puts("Metal host contract holds");
    return 0;
}
