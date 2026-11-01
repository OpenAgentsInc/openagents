// Chapter 33: measure this host's roofline. The host owns the effects:
// allocation, the monotonic clock and naming the machine. Coax owns the
// kernels it times (rf_triad, rf_madd), the rates (rf_rate), the phase
// accounting (rp_decode, rp_prefill) and the placement (rf_place), all from
// psionic/lib/roofline.coax. Prints one JSON receipt on stdout.
// Build and run with psionic/tests/roofline.sh, which records the receipt.
#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <sys/utsname.h>
#include "roofline.h"

enum { TRIAD = 1 << 24, MADD = 4096, ROUNDS = 100000, REPS = 7 };

static int64_t now_ns(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (int64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}
static int cmp(const void *a, const void *b) {
    int64_t x = *(const int64_t *)a, y = *(const int64_t *)b;
    return (x > y) - (x < y);
}
static void cpu_name(char *out, size_t n) {
    snprintf(out, n, "unknown");
    FILE *f = fopen("/proc/cpuinfo", "r");
    if (!f) return;
    char line[512];
    while (fgets(line, sizeof line, f)) {
        if (strncmp(line, "model name", 10) == 0) {
            char *v = strchr(line, ':');
            if (v) { v += 2; v[strcspn(v, "\n")] = 0; snprintf(out, n, "%s", v); }
            break;
        }
    }
    fclose(f);
}
static void place(const char *name, const char *phase, int64_t tokens, coax_t_RpWork w, coax_t_RfHost h, int last) {
    coax_t_RfPlace p = rf_place(h, w.milli, w.flops);
    const char *bound = p.bound == 1 ? "memory" : p.bound == 2 ? "compute" : "invalid";
    printf("    {\"model\": \"%s\", \"phase\": \"%s\", \"tokens\": %lld, \"flops\": %lld, \"bytes\": %lld, "
           "\"milli_flops_per_byte\": %lld, \"bound\": \"%s\", \"attainable_mflops\": %lld, \"least_ns\": %lld}%s\n",
           name, phase, (long long)tokens, (long long)w.flops, (long long)w.bytes, (long long)w.milli, bound,
           (long long)p.attainable, (long long)p.ns, last ? "" : ",");
}

int main(int argc, char **argv) {
    const char *compiler = argc > 1 ? argv[1] : "unknown";
    const char *flags = argc > 2 ? argv[2] : "unknown";
    float *a = aligned_alloc(64, sizeof(float) * TRIAD), *b = aligned_alloc(64, sizeof(float) * TRIAD),
          *c = aligned_alloc(64, sizeof(float) * TRIAD), *x = aligned_alloc(64, sizeof(float) * MADD);
    if (!a || !b || !c || !x) { fprintf(stderr, "roofline: allocation failed\n"); return 1; }
    for (int i = 0; i < TRIAD; i++) { a[i] = 0; b[i] = (float)(i & 1023); c[i] = 1; }
    for (int i = 0; i < MADD; i++) x[i] = (float)i;
    coax_v_f32 va = {a, TRIAD}, vb = {b, TRIAD}, vc = {c, TRIAD}, vx = {x, MADD};

    // One warm-up run each, then REPS timed runs; the best run is the roof.
    int64_t triad[REPS], madd[REPS];
    if (rf_triad(va, vb, vc, 3.0f) != TRIAD) return 1;
    for (int r = 0; r < REPS; r++) { int64_t t = now_ns(); rf_triad(va, vb, vc, 3.0f); triad[r] = now_ns() - t; }
    int64_t flops = rf_madd(vx, ROUNDS, 0.5f, 1.0f);
    for (int r = 0; r < REPS; r++) { int64_t t = now_ns(); rf_madd(vx, ROUNDS, 0.5f, 1.0f); madd[r] = now_ns() - t; }
    if (a[5] != 8.0f || x[0] != 2.0f) { fprintf(stderr, "roofline: kernels gave wrong values\n"); return 1; }
    qsort(triad, REPS, sizeof triad[0], cmp);
    qsort(madd, REPS, sizeof madd[0], cmp);
    int64_t bytes = rf_triad_bytes(TRIAD);
    coax_t_RfHost h = {rf_rate(flops, madd[0]), rf_rate(bytes, triad[0])};

    char host[256] = "unknown", cpu[256];
    gethostname(host, sizeof host);
    cpu_name(cpu, sizeof cpu);
    struct utsname u;
    uname(&u);
    time_t now = time(NULL);
    char date[32];
    strftime(date, sizeof date, "%Y-%m-%d", gmtime(&now));

    printf("{\n  \"schema\": \"psionic.roofline.v1\",\n  \"profile\": \"rf-host-v1\",\n");
    printf("  \"host\": \"%s\",\n  \"cpu\": \"%s\",\n  \"kernel\": \"%s %s %s\",\n  \"date\": \"%s\",\n",
           host, cpu, u.sysname, u.release, u.machine, date);
    char load[64] = "unknown";
    FILE *lf = fopen("/proc/loadavg", "r");
    if (lf) { if (fgets(load, sizeof load, lf)) load[strcspn(load, "\n")] = 0; fclose(lf); }
    printf("  \"loadavg\": \"%s\",\n", load);
    printf("  \"compiler\": \"%s\",\n  \"flags\": \"%s\",\n  \"threads\": 1,\n", compiler, flags);
    printf("  \"triad\": {\"elements\": %d, \"bytes_per_run\": %lld, \"runs\": %d, \"best_ns\": %lld, \"median_ns\": %lld, \"worst_ns\": %lld, "
           "\"best_mbytes_per_s\": %lld, \"median_mbytes_per_s\": %lld},\n",
           TRIAD, (long long)bytes, REPS, (long long)triad[0], (long long)triad[REPS / 2], (long long)triad[REPS - 1],
           (long long)h.mbytes, (long long)rf_rate(bytes, triad[REPS / 2]));
    printf("  \"madd\": {\"values\": %d, \"rounds\": %d, \"flops_per_run\": %lld, \"runs\": %d, \"best_ns\": %lld, \"median_ns\": %lld, \"worst_ns\": %lld, "
           "\"best_mflops\": %lld, \"median_mflops\": %lld},\n",
           MADD, ROUNDS, (long long)flops, REPS, (long long)madd[0], (long long)madd[REPS / 2], (long long)madd[REPS - 1],
           (long long)h.mflops, (long long)rf_rate(flops, madd[REPS / 2]));
    coax_t_RfPlace ridge = rf_place(h, 0, 0);
    printf("  \"ridge_milli_flops_per_byte\": %lld,\n  \"phases\": [\n", (long long)ridge.ridge);
    coax_t_RpModel e = rp_elephant(), m = rp_mammoth(), g = rp_gptoss_20b();
    place("elephant-1", "decode", 86, rp_decode(e, 86), h, 0);
    place("elephant-1", "prefill", 86, rp_prefill(e, 86), h, 0);
    place("elephant-1", "prefill", 2048, rp_prefill(e, 2048), h, 0);
    place("mammoth-1", "decode", 86, rp_decode(m, 86), h, 0);
    place("mammoth-1", "prefill", 86, rp_prefill(m, 86), h, 0);
    place("gpt-oss-20b", "decode", 86, rp_decode(g, 86), h, 0);
    place("gpt-oss-20b", "prefill", 86, rp_prefill(g, 86), h, 1);
    printf("  ]\n}\n");
    free(a); free(b); free(c); free(x);
    return 0;
}
