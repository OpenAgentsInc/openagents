/* Chapter 132 with the GPU as a member: a group of two devices training
   chapter 115's tiny decoder, 16 rows a step. Device 0 is a host thread
   running the C build of chapter 129's row kernel on rows 0 to 7; device 1
   is the CUDA device running the Coax-generated gk_rows on rows 8 to 15,
   at the same time. Its rows' gradients and losses are copied back, and
   the chain all-reduce, the step and its refusals run in Coax
   (device-group.coax compiled to C) on the host. It checks the run against
   chapter 118's CPU run within GT_TOLERANCE (the device's exp and log may
   round differently), that the device's rows differ from the host's in
   bits only where chapter 129 says they may, and that a step whose GPU
   report is late commits nothing. Prints SKIP without a CUDA device or NVRTC.
   Usage: device-group-gpu KERNELS.cu [STEPS] */
#define _POSIX_C_SOURCE 199309L
#include "device-group.h"
#include "cuda-host.h"
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define ROWS 16
#define NP 3152
#define NW 13984
#define CTX 32
#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec * 1e-9; }
static coax_v_f64 hv(double *d, int64_t n) { return (coax_v_f64){d, n}; }
static char *read_all(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *t = malloc((size_t)size + 1);
    if (!t || fread(t, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); t[size] = 0; *n = (size_t)size; return t;
}

static coax_t_TtFix fix;
static coax_t_TdShape shape;
static coax_t_TrConfig config;
static coax_v_t_PkDoc docs(void) { return (coax_v_t_PkDoc){fix.docs.e, 47}; }
static coax_v_u8 tokens(void) { return (coax_v_u8){fix.tokens.e, fix.n}; }
static int64_t no_order[1];
static coax_v_s64 order(void) { return (coax_v_s64){no_order, 0}; }

static double p[NP], m[NP], v[NP], stage[3 * NP], total[NP], scalars[3], losses[ROWS], slots[2 * NP], rope[CTX * 8];
static double grads[ROWS * NP], work[ROWS * NW], dwork[ROWS * NW];
static uint8_t rows[ROWS * CTX];
static coax_t_DgReport reports[2];
enum { G_P, G_ROWS, G_ROPE, G_WORK, G_DWORK, G_GRADS, G_LOSS, G_SHAPE, NG };
static PcBuffer gb[NG];
static PcKernel krows;
static coax_t_DgPlan plan;

typedef struct { coax_t_TrBatch b; double t0; } Job;
static void *host_device(void *arg) {
    Job *j = arg;
    coax_t_DgReport r = dg_rows(shape, plan, 0, j->b, (coax_v_u8){rows, ROWS * CTX}, hv(rope, CTX * 8), hv(p, NP),
                                hv(work, ROWS * NW), hv(dwork, ROWS * NW), hv(grads, ROWS * NP), hv(losses, ROWS));
    r.at = (int64_t)((now() - j->t0) * 1e6);
    reports[0] = r;
    return NULL;
}
static PcArg view(int b, int64_t first, int64_t count, int64_t elem) { return (PcArg){PC_ARG_VIEW, 0, 0, gb[b] + (PcBuffer)(first * elem), count}; }
/* Device 1: its rows on the GPU, item 0 being row lo. */
static coax_t_DgReport gpu_device(coax_t_TrBatch b, double t0) {
    int64_t lo = dg_first(plan, 1), hi = dg_first(plan, 2), k = hi - lo;
    coax_t_DgReport r = {.device = 1, .revision = b.revision, .first = lo, .count = k};
    if (b.status != COAX_TrStatus_ok || b.micro != plan.rows) { r.failed = 1; return r; }
    TRY(pc_upload(gb[G_P], p, NP * 8, NULL));
    TRY(pc_upload(gb[G_ROWS], rows + lo * CTX, (size_t)(k * CTX), NULL));
    PcArg a[8] = {view(G_SHAPE, 0, 8, 8), view(G_P, 0, NP, 8), view(G_ROWS, 0, k * CTX, 1), view(G_ROPE, 0, CTX * 8, 8),
                  view(G_WORK, 0, k * NW, 8), view(G_DWORK, 0, k * NW, 8), view(G_GRADS, 0, k * NP, 8), view(G_LOSS, 0, k, 8)};
    TRY(pc_launch(krows, k, 32, a, 8, NULL));
    TRY(pc_sync());
    TRY(pc_download(grads + lo * NP, gb[G_GRADS], (size_t)(k * NP) * 8, NULL));
    TRY(pc_download(losses + lo, gb[G_LOSS], (size_t)k * 8, NULL));
    r.at = (int64_t)((now() - t0) * 1e6);
    return r;
}
static coax_t_DgStep step(coax_t_TrRun run, coax_t_TrBatch b, int64_t deadline, int64_t gpu_delay) {
    pthread_t t; Job j = {b, now()};
    if (pthread_create(&t, NULL, host_device, &j) != 0) { perror("pthread_create"); exit(2); }
    reports[1] = gpu_device(b, j.t0); reports[1].at += gpu_delay;
    pthread_join(t, NULL);
    return dg_finish(config, plan, run, b, (coax_v_t_DgReport){reports, 2}, deadline, hv(p, NP), hv(m, NP), hv(v, NP),
                     hv(stage, 3 * NP), hv(total, NP), hv(grads, ROWS * NP), hv(losses, ROWS), hv(scalars, 3), hv(slots, 2 * NP));
}
static int report(const char *what, coax_t_GtGap g) {
    printf("  %-16s %5lld values, %5lld differ in bits, %lld past the tolerance, worst relative gap %.3g\n",
           what, (long long)g.count, (long long)g.differ, (long long)g.far, g.worst);
    return g.far != 0;
}
#define DEADLINE 60000000

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: device-group-gpu KERNELS.cu [STEPS]\n"); return 2; }
    int64_t steps = argc > 2 ? atoll(argv[2]) : 6;
    if (steps < 2 || steps > 100) { fprintf(stderr, "STEPS must be 2 to 100\n"); return 2; }
    PcDeviceInfo info;
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    printf("device 1: %s (sm_%d%d); device 0: a host thread\n", info.name, info.major, info.minor);
    size_t len; char *src = read_all(argv[1], &len); double seconds = 0;
    PcModule module; TRY(pc_compile(src, len, "train-kernels.cu", &module, &seconds));
    TRY(pc_kernel(module, "gk_rows", &krows));
    fix = gt_fix(); shape = gt_tiny(); config = gt_config(shape, ROWS, steps); plan = dg_plan(2, ROWS, COAX_DgCollective_chain);
    coax_t_TrState begun = gt_begin(config, NP, docs(), tokens(), order());
    if (begun.status != COAX_TrStatus_ok || plan.status != COAX_DgStatus_ok) { printf("FAIL: the run or the plan was refused\n"); return 1; }
    int64_t half = ROWS / 2, sizes[NG] = {NP * 8, half * CTX, CTX * 8 * 8, half * NW * 8, half * NW * 8, half * NP * 8, half * 8, 64};
    for (int k = 0; k < NG; k++) TRY(pc_alloc((size_t)sizes[k], &gb[k]));
    int64_t sh[8]; gt_shape(shape, (coax_v_s64){sh, 8}); TRY(pc_upload(gb[G_SHAPE], sh, 64, NULL));
    gt_rope(shape, hv(rope, CTX * 8)); TRY(pc_upload(gb[G_ROPE], rope, CTX * 8 * 8, NULL));
    int failures = 0;

    /* Chapter 118's CPU run, the reference. */
    static double rp[NP], rm[NP], rv[NP], rt[NP], rs[3 * NP], rl[100], rw[NW], rdw[NW], rg[NP], dl[100]; static uint8_t rrows[ROWS * CTX];
    gt_init(shape, hv(rp, NP));
    coax_t_TrStep ref = gt_reference(shape, config, docs(), tokens(), order(), begun.run, steps, (coax_v_u8){rrows, ROWS * CTX}, hv(rw, NW), hv(rdw, NW), hv(rg, NP),
                                     hv(rp, NP), hv(rm, NP), hv(rv, NP), hv(rt, NP), hv(rs, 3 * NP), hv(rl, steps));
    if (ref.status != COAX_TrStatus_ok) { printf("FAIL: chapter 118's run refused\n"); return 1; }

    /* A step whose GPU report is late: refused, nothing written; then the run. */
    gt_init(shape, hv(p, NP)); memset(m, 0, sizeof m); memset(v, 0, sizeof v);
    coax_t_TrRun run = begun.run;
    coax_t_TrBatch b = gt_next(config, run, docs(), tokens(), (coax_v_u8){rows, ROWS * CTX});
    coax_t_DgStep late = step(run, b, DEADLINE, 2 * DEADLINE);
    int kept = late.status == COAX_DgStatus_late && late.device == 1 && late.step.run.revision == 0 && m[0] == 0 && v[0] == 0;
    printf("a step whose GPU report comes after the deadline: %s, device %lld named, nothing written: %s\n",
           late.status == COAX_DgStatus_late ? "refused as late" : "NOT REFUSED", (long long)late.device, kept ? "yes" : "no");
    failures += !kept;
    /* The device's rows of the first step against the host's C build of the same kernel. */
    {
        static double hg[ROWS * NP], hl[ROWS];
        memcpy(hg, grads, sizeof grads); memcpy(hl, losses, sizeof losses);
        coax_t_DgPlan whole = dg_plan(1, ROWS, COAX_DgCollective_chain);
        dg_rows(shape, whole, 0, b, (coax_v_u8){rows, ROWS * CTX}, hv(rope, CTX * 8), hv(p, NP), hv(work, ROWS * NW), hv(dwork, ROWS * NW), hv(hg, ROWS * NP), hv(hl, ROWS));
        printf("rows 8 to 15 of the first step, GPU against the host's C build:\n");
        failures += report("row gradients", gt_gap(hv(grads + half * NP, half * NP), hv(hg + half * NP, half * NP), gt_tolerance()));
        failures += report("row losses", gt_gap(hv(losses + half, half), hv(hl + half, half), gt_tolerance()));
    }
    double t0 = now();
    for (int64_t k = 0; k < steps; k++) {
        if (k > 0) b = gt_next(config, run, docs(), tokens(), (coax_v_u8){rows, ROWS * CTX});
        coax_t_DgStep s = step(run, b, DEADLINE, 0);
        if (s.status != COAX_DgStatus_ok || s.step.status != COAX_TrStatus_ok) { printf("FAIL: step %lld: group %d, trainer %d\n", (long long)k, s.status, s.step.status); return 1; }
        run = s.step.run; dl[k] = s.step.loss;
    }
    double mixed_s = now() - t0;
    printf("%lld steps of %d rows, host thread and GPU, against chapter 118's CPU run (tolerance %g):\n", (long long)steps, ROWS, gt_tolerance());
    failures += report("losses", gt_gap(hv(dl, steps), hv(rl, steps), gt_tolerance()));
    failures += report("parameters", gt_gap(hv(p, NP), hv(rp, NP), gt_tolerance()));
    failures += report("first moments", gt_gap(hv(m, NP), hv(rm, NP), gt_tolerance()));
    failures += report("second moments", gt_gap(hv(v, NP), hv(rv, NP), gt_tolerance()));
    failures += run.revision != steps;
    printf("receipt: host thread and GPU, %.2f ms/step (%.0f tokens/s)\n", mixed_s / steps * 1e3, (double)(steps * ROWS * CTX) / mixed_s);
    for (int k = 0; k < NG; k++) pc_free(gb[k]);
    if (failures) { printf("FAIL: %d checks\n", failures); return 1; }
    printf("chapter 132 with the GPU as a member: the mixed group trains within the frozen tolerance of chapter 118's run, and a late GPU step commits nothing\n");
    return 0;
}
