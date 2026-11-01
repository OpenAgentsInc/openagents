/* Chapter 130: profile chapter 129's training step on the CPU and on a CUDA
   device. This file only allocates, copies, launches and reads the clock;
   the accounting, statistics, roofline placement and bottleneck are Coax
   (tp_report in psionic/lib/transformer/train-profile.coax, compiled to C
   with chapter 129's plan and decisions by train-profile-device.coax).

   For each target it measures the target's binary64 roofline with tp_triad
   and tp_madd, runs WARM unprofiled steps, then RUNS steps with every phase
   timed on the monotonic clock (each kernel phase ends with a device sync),
   and after each step one tp_forward launch on the same rows. The run's
   parameters must match chapter 118's CPU run within chapter 129's frozen
   tolerance: profiling must not change the numbers.

   Prints a summary and writes RECEIPT (JSON). Times are a receipt, never a
   transcript. Usage: train-profile-device KERNELS.cu RECEIPT [RUNS [ROWS]].
   Without a CUDA device the device half prints SKIP. */
#define _POSIX_C_SOURCE 200112L
#include "train-profile.h"
#include "cuda-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
#define WARM 3
#define PHASES 10

enum { B_P, B_M, B_V, B_STAGE, B_TOTAL, B_GRADS, B_WORK, B_DWORK, B_ROWS, B_ROPE, B_LOSS, B_SCALARS, B_SHAPE, NB };
enum { K_ROWS, K_MEAN, K_SQUARES, K_CLIP, K_ADAMW, K_ADMIT, K_COMMIT, K_FORWARD, K_TRIAD, K_MADD, NK };
static const char *kernel_names[NK] = {"gk_rows", "gk_mean", "gk_squares", "gk_clip", "gk_adamw", "gk_admit", "gk_commit", "tp_forward", "tp_triad", "tp_madd"};
static const char *phase_names[PHASES] = {"upload", "rows", "reduce", "losses", "decide", "optimizer", "flags", "admit", "commit", "forward"};
static PcKernel kernels[NK];
static PcBuffer buf[NB];
static int64_t elem[NB] = {8, 8, 8, 8, 8, 8, 8, 8, 1, 8, 8, 8, 8};

static int64_t now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (int64_t)t.tv_sec * 1000000000 + t.tv_nsec; }
static char *read_all(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *t = malloc((size_t)size + 1);
    if (!t || fread(t, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); t[size] = 0; *n = (size_t)size; return t;
}
static coax_v_f64 hv(double *d, int64_t n) { return (coax_v_f64){d, n}; }
static PcArg view(int b, int64_t first, int64_t count) { return (PcArg){PC_ARG_VIEW, 0, 0, buf[b] + (PcBuffer)(first * elem[b]), count}; }
static PcArg integer(int64_t i) { return (PcArg){PC_ARG_INT, i, 0, 0, 0}; }
static PcArg real(double f) { return (PcArg){PC_ARG_F64, 0, f, 0, 0}; }
static void launch(int k, int64_t items, PcArg *a, int count) { TRY(pc_launch(kernels[k], items, items < 128 ? 32 : 128, a, count, NULL)); }

/* The run, and host copies of every buffer for the CPU target. */
typedef struct {
    coax_t_TdShape s; coax_t_TrConfig c; coax_t_GtPlan plan; coax_t_TtFix *fix; int64_t n, nw, micro;
    int64_t sh[8]; double *rope;
    /* CPU target */
    double *p, *m, *v, *stage, *total, *grads, *work, *dwork, losses[64], scalars[3]; uint8_t rows[4096];
} Run;
static coax_v_t_PkDoc docs(Run *r) { return (coax_v_t_PkDoc){r->fix->docs.e, 47}; }
static coax_v_u8 tokens(Run *r) { return (coax_v_u8){r->fix->tokens.e, r->fix->n}; }
static int64_t no_order[1];
static coax_v_s64 order(void) { return (coax_v_s64){no_order, 0}; }
static PcArg whole(Run *r, int b) { return view(b, 0, r->plan.sizes.e[b] / elem[b]); }

/* One profiled step. t[k] gets phase k's nanoseconds; returns the step's
   wall time, or -1 when the trainer refused it. gpu selects the target. */
static int64_t step(Run *r, int gpu, coax_t_TrRun *run, const uint8_t *named, coax_t_TrBatch b, int64_t *t, int *clipped) {
    int64_t n = r->n, M = b.micro, a0, s0 = now(); double losses[64], scalars[3]; PcArg a[14];
    coax_v_s64 sh = {r->sh, 8};
    a0 = now();
    if (gpu) TRY(pc_upload(buf[B_ROWS], named, (size_t)(M * r->c.length), NULL)); else memcpy(r->rows, named, (size_t)(M * r->c.length));
    t[0] = now() - a0; a0 = now();
    if (gpu) {
        a[0] = view(B_SHAPE, 0, 8); a[1] = whole(r, B_P); a[2] = whole(r, B_ROWS); a[3] = whole(r, B_ROPE);
        a[4] = whole(r, B_WORK); a[5] = whole(r, B_DWORK); a[6] = whole(r, B_GRADS); a[7] = whole(r, B_LOSS);
        launch(K_ROWS, M, a, 8); TRY(pc_sync());
    } else for (int64_t i = 0; i < M; i++)
        gk_rows(i, sh, hv(r->p, n), (coax_v_u8){r->rows, 4096}, hv(r->rope, r->s.context * r->s.head_width), hv(r->work, M * r->nw), hv(r->dwork, M * r->nw), hv(r->grads, M * n), hv(r->losses, M));
    t[1] = now() - a0; a0 = now();
    if (gpu) {
        a[0] = integer(M); a[1] = integer(n); a[2] = whole(r, B_GRADS); a[3] = whole(r, B_TOTAL); launch(K_MEAN, n, a, 4);
        a[0] = integer(n); a[1] = whole(r, B_TOTAL); a[2] = view(B_SCALARS, 0, 1); launch(K_SQUARES, 1, a, 3); TRY(pc_sync());
    } else {
        for (int64_t i = 0; i < n; i++) gk_mean(i, M, n, hv(r->grads, M * n), hv(r->total, n));
        gk_squares(0, n, hv(r->total, n), hv(r->scalars, 3));
    }
    t[2] = now() - a0; a0 = now();
    if (gpu) { TRY(pc_download(losses, buf[B_LOSS], (size_t)M * 8, NULL)); TRY(pc_download(scalars, buf[B_SCALARS], 8, NULL)); }
    else { memcpy(losses, r->losses, (size_t)M * 8); scalars[0] = r->scalars[0]; }
    t[3] = now() - a0; a0 = now();
    coax_t_GtDecide d = gt_decide(r->c, *run, b, hv(losses, M), scalars[0]);
    t[4] = now() - a0;
    if (d.status != COAX_TrStatus_ok) return -1;
    a0 = now();
    *clipped = d.clip;
    if (gpu) {
        if (d.clip) { a[0] = real(r->c.clip); a[1] = real(d.norm); a[2] = whole(r, B_TOTAL); launch(K_CLIP, n, a, 3); }
        a[0] = integer(n); a[1] = real(d.rate); a[2] = real(r->c.beta1); a[3] = real(r->c.beta2); a[4] = real(r->c.eps); a[5] = real(0);
        a[6] = real(d.c1); a[7] = real(d.c2); a[8] = whole(r, B_P); a[9] = whole(r, B_TOTAL); a[10] = whole(r, B_M); a[11] = whole(r, B_V); a[12] = whole(r, B_STAGE);
        launch(K_ADAMW, n, a, 13);
        a[0] = integer(n); a[1] = real(r->c.bound); a[2] = whole(r, B_STAGE); a[3] = view(B_SCALARS, 1, 2); launch(K_ADMIT, 1, a, 4); TRY(pc_sync());
    } else {
        if (d.clip) for (int64_t i = 0; i < n; i++) gk_clip(i, r->c.clip, d.norm, hv(r->total, n));
        for (int64_t i = 0; i < n; i++) gk_adamw(i, n, d.rate, r->c.beta1, r->c.beta2, r->c.eps, 0, d.c1, d.c2, hv(r->p, n), hv(r->total, n), hv(r->m, n), hv(r->v, n), hv(r->stage, 3 * n));
        gk_admit(0, n, r->c.bound, hv(r->stage, 3 * n), (coax_v_f64){r->scalars + 1, 2});
    }
    t[5] = now() - a0; a0 = now();
    if (gpu) TRY(pc_download(scalars, buf[B_SCALARS], 24, NULL)); else memcpy(scalars, r->scalars, 24);
    t[6] = now() - a0; a0 = now();
    coax_t_TrStep st = gt_commit(r->c, *run, b, d, hv(scalars + 1, 2));
    t[7] = now() - a0;
    if (st.status != COAX_TrStatus_ok) return -1;
    a0 = now();
    if (gpu) { a[0] = integer(n); a[1] = whole(r, B_STAGE); a[2] = whole(r, B_P); a[3] = whole(r, B_M); a[4] = whole(r, B_V); launch(K_COMMIT, n, a, 5); TRY(pc_sync()); }
    else for (int64_t i = 0; i < n; i++) gk_commit(i, n, hv(r->stage, 3 * n), hv(r->p, n), hv(r->m, n), hv(r->v, n));
    t[8] = now() - a0;
    int64_t wall = now() - s0;
    *run = st.run;
    /* Apart from the step: the forward half on the same rows. */
    a0 = now();
    if (gpu) {
        a[0] = view(B_SHAPE, 0, 8); a[1] = whole(r, B_P); a[2] = whole(r, B_ROWS); a[3] = whole(r, B_ROPE); a[4] = whole(r, B_WORK);
        launch(K_FORWARD, M, a, 5); TRY(pc_sync());
    } else for (int64_t i = 0; i < M; i++) tp_forward(i, sh, hv(r->p, n), (coax_v_u8){r->rows, 4096}, hv(r->rope, r->s.context * r->s.head_width), hv(r->work, M * r->nw));
    t[9] = now() - a0;
    return wall;
}

/* The target's roofline: best of 7 runs of each kernel. */
static coax_t_RfHost roof(int gpu, int64_t *triad_ns, int64_t *madd_ns) {
    int64_t items = gpu ? (1 << 24) : (1 << 22), madd_items = gpu ? (1 << 20) : 4096, rounds = 2000;
    int64_t best_t = -1, best_m = -1;
    double *a = NULL, *b = NULL, *c = NULL; PcBuffer da = 0, db = 0, dc = 0;
    if (gpu) {
        TRY(pc_alloc((size_t)items * 8, &da)); TRY(pc_alloc((size_t)items * 8, &db)); TRY(pc_alloc((size_t)items * 8, &dc));
        TRY(pc_fill(db, 0x3ff00000u, (size_t)items * 2, NULL)); TRY(pc_fill(dc, 0x3ff00000u, (size_t)items * 2, NULL)); TRY(pc_fill(da, 0, (size_t)items * 2, NULL));
    } else {
        a = calloc((size_t)items, 8); b = calloc((size_t)items, 8); c = calloc((size_t)items, 8);
        for (int64_t i = 0; i < items; i++) { b[i] = 1; c[i] = 2; }
    }
    for (int k = 0; k < 8; k++) {
        int64_t t0 = now();
        if (gpu) {
            PcArg x[4] = {real(0.5), {PC_ARG_VIEW, 0, 0, da, items}, {PC_ARG_VIEW, 0, 0, db, items}, {PC_ARG_VIEW, 0, 0, dc, items}};
            TRY(pc_launch(kernels[K_TRIAD], items, 256, x, 4, NULL)); TRY(pc_sync());
        } else for (int64_t i = 0; i < items; i++) tp_triad(i, 0.5, hv(a, items), hv(b, items), hv(c, items));
        int64_t dt = now() - t0;
        if (k > 0 && (best_t < 0 || dt < best_t)) best_t = dt;  /* run 0 warms up */
    }
    for (int k = 0; k < 8; k++) {
        int64_t t0 = now();
        if (gpu) {
            PcArg x[4] = {integer(rounds), real(0.999999), real(0.000001), {PC_ARG_VIEW, 0, 0, da, madd_items}};
            TRY(pc_launch(kernels[K_MADD], madd_items, 256, x, 4, NULL)); TRY(pc_sync());
        } else for (int64_t i = 0; i < madd_items; i++) tp_madd(i, rounds, 0.999999, 0.000001, hv(a, madd_items));
        int64_t dt = now() - t0;
        if (k > 0 && (best_m < 0 || dt < best_m)) best_m = dt;
    }
    if (gpu) { pc_free(da); pc_free(db); pc_free(dc); } else { free(a); free(b); free(c); }
    *triad_ns = best_t; *madd_ns = best_m;
    return (coax_t_RfHost){.mflops = rf_rate(madd_items * (16 * rounds + 16), best_m), .mbytes = rf_rate(items * 24, best_t)};
}

static void alloc_plan(Run *r) {
    for (int k = 0; k < NB; k++) TRY(pc_alloc((size_t)r->plan.sizes.e[k], &buf[k]));
    TRY(pc_upload(buf[B_SHAPE], r->sh, 64, NULL));
    TRY(pc_upload(buf[B_ROPE], r->rope, (size_t)(r->s.context * r->s.head_width) * 8, NULL));
}

typedef struct { coax_t_TpReport rep; coax_t_RfHost host; int64_t triad_ns, madd_ns, runs, clips; int64_t ns[PHASES * 1024], wall[1024]; double worst_gap; int64_t far; } Profile;

/* WARM + runs steps on one target from a fresh start; the profile, and the
   parameters against chapter 118's CPU run of as many steps. */
static int profile(Run *r, int gpu, int64_t runs, Profile *out, double *host_p, double *ref_p, coax_t_TrRun begun) {
    out->host = roof(gpu, &out->triad_ns, &out->madd_ns);
    gt_init(r->s, hv(r->p, r->n)); memset(r->m, 0, (size_t)r->n * 8); memset(r->v, 0, (size_t)r->n * 8);
    if (gpu) {
        TRY(pc_upload(buf[B_P], r->p, (size_t)r->n * 8, NULL));
        TRY(pc_fill(buf[B_M], 0, (size_t)r->n * 2, NULL)); TRY(pc_fill(buf[B_V], 0, (size_t)r->n * 2, NULL));
    }
    coax_t_TrRun run = begun; uint8_t named[4096]; int64_t t[PHASES]; int clipped = 0;
    out->runs = runs; out->clips = 0;
    for (int64_t k = 0; k < WARM + runs; k++) {
        coax_t_TrBatch b = gt_next(r->c, run, docs(r), tokens(r), (coax_v_u8){named, r->c.accumulate * r->c.length});
        if (b.status != COAX_TrStatus_ok) { printf("FAIL: the trainer refused batch %lld\n", (long long)k); return 1; }
        int64_t wall = step(r, gpu, &run, named, b, t, &clipped);
        if (wall < 0) { printf("FAIL: the trainer refused step %lld\n", (long long)k); return 1; }
        if (k < WARM) continue;
        int64_t j = k - WARM;
        for (int p = 0; p < PHASES; p++) out->ns[p * runs + j] = t[p];
        out->wall[j] = wall; out->clips += clipped;
    }
    if (gpu) TRY(pc_download(host_p, buf[B_P], (size_t)r->n * 8, NULL)); else memcpy(host_p, r->p, (size_t)r->n * 8);
    coax_t_GtGap g = gt_gap(hv(host_p, r->n), hv(ref_p, r->n), gt_tolerance());
    out->worst_gap = g.worst; out->far = g.far;
    out->rep = tp_report((coax_v_s64){r->sh, 8}, r->micro, out->clips, out->host, (coax_v_s64){out->ns, PHASES * runs}, (coax_v_s64){out->wall, runs});
    return g.far != 0 || out->rep.status != COAX_TpStatus_ok;
}

static void summary(const char *target, Profile *f) {
    coax_t_TpReport *r = &f->rep;
    printf("%s: roofline %lld MFLOP/s, %lld MB/s (binary64); %lld steps, mean %.3f ms (sd %.3f); status %d\n", target,
           (long long)f->host.mflops, (long long)f->host.mbytes, (long long)f->runs, r->step.mean / 1e6, r->step.sd / 1e6, (int)r->status);
    for (int p = 0; p < PHASES; p++) {
        coax_t_TpLine *l = &r->lines.e[p];
        printf("  %-9s %12.0f ns (sd %9.0f) %6.2f%%  %12.0f FLOPs %10.0f bytes", phase_names[p], l->stats.mean, l->stats.sd, 100 * l->share, l->flops, l->bytes);
        if (l->least > 0) printf("  least %10.1f ns, %s-bound, %.0fx", l->least, l->bound == COAX_RfBound_compute ? "compute" : "memory", l->gap);
        printf("\n");
    }
    printf("  bottleneck: %s, %.2f%% of the step, %.0fx its least time; unaccounted %.0f ns (worst step %lld ns); kernels idle %.0f ns; %.2f launches a step; backward %.0f ns\n",
           phase_names[r->bottleneck], 100 * r->share, r->gap, r->unaccounted, (long long)r->slack, r->idle, r->launches, r->backward);
    printf("  parameters against chapter 118's CPU run: worst relative gap %.3g, %lld past the tolerance\n", f->worst_gap, (long long)f->far);
}
static void json(FILE *o, const char *target, Profile *f) {
    coax_t_TpReport *r = &f->rep;
    fprintf(o, "  \"%s\": {\"status\": %d, \"runs\": %lld, \"clips\": %lld, \"roof\": {\"mflops\": %lld, \"mbytes\": %lld, \"triad_best_ns\": %lld, \"madd_best_ns\": %lld},\n",
            target, (int)r->status, (long long)f->runs, (long long)f->clips, (long long)f->host.mflops, (long long)f->host.mbytes, (long long)f->triad_ns, (long long)f->madd_ns);
    fprintf(o, "    \"step_ns\": {\"mean\": %.1f, \"sd\": %.1f}, \"unaccounted_ns\": %.1f, \"worst_slack_ns\": %lld, \"kernel_idle_ns\": %.1f, \"launches\": %.2f,\n",
            r->step.mean, r->step.sd, r->unaccounted, (long long)r->slack, r->idle, r->launches);
    fprintf(o, "    \"bottleneck\": \"%s\", \"share\": %.4f, \"gap\": %.1f, \"backward_ns\": %.1f, \"worst_relative_gap\": %.3g, \"past_tolerance\": %lld,\n",
            phase_names[r->bottleneck], r->share, r->gap, r->backward, f->worst_gap, (long long)f->far);
    fprintf(o, "    \"phases\": [\n");
    for (int p = 0; p < PHASES; p++) {
        coax_t_TpLine *l = &r->lines.e[p];
        fprintf(o, "      {\"phase\": \"%s\", \"mean_ns\": %.1f, \"sd_ns\": %.1f, \"share\": %.5f, \"flops\": %.1f, \"bytes\": %.1f, \"least_ns\": %.1f, \"bound\": \"%s\", \"gap\": %.1f}%s\n",
                phase_names[p], l->stats.mean, l->stats.sd, l->share, l->flops, l->bytes, l->least,
                l->bound == COAX_RfBound_compute ? "compute" : l->bound == COAX_RfBound_memory ? "memory" : "none", l->gap, p + 1 < PHASES ? "," : "");
    }
    fprintf(o, "    ]}");
}

int main(int argc, char **argv) {
    if (argc < 3) { fprintf(stderr, "usage: train-profile-device KERNELS.cu RECEIPT [RUNS [ROWS]]\n"); return 2; }
    int64_t runs = argc > 3 ? atoll(argv[3]) : 20, rows = argc > 4 ? atoll(argv[4]) : 4;
    if (runs < 5 || runs > 1024 || (rows != 4 && rows != 64)) { fprintf(stderr, "RUNS must be 5 to 1024, ROWS 4 or 64\n"); return 2; }
    static coax_t_TtFix fix; fix = gt_fix();
    static Run r; r.s = gt_tiny(); r.fix = &fix; r.c = gt_config(r.s, rows, WARM + runs); r.micro = rows;
    coax_t_TrState begun = gt_begin(r.c, 3152, docs(&r), tokens(&r), order());
    if (begun.status != COAX_TrStatus_ok) { printf("FAIL: the trainer refused the run\n"); return 1; }
    r.n = 3152;
    r.plan = gt_plan(r.s, r.c, docs(&r), tokens(&r), (int64_t)1 << 40);
    if (r.plan.status != COAX_GtStatus_ok) { printf("FAIL: plan refused\n"); return 1; }
    r.nw = r.plan.work; gt_shape(r.s, (coax_v_s64){r.sh, 8});
    r.rope = calloc(4096, 8); gt_rope(r.s, hv(r.rope, r.s.context * r.s.head_width));
    int64_t n = r.n;
    r.p = calloc((size_t)n, 8); r.m = calloc((size_t)n, 8); r.v = calloc((size_t)n, 8); r.stage = calloc((size_t)(3 * n), 8); r.total = calloc((size_t)n, 8);
    r.grads = calloc((size_t)(rows * n), 8); r.work = calloc((size_t)(rows * r.nw), 8); r.dwork = calloc((size_t)(rows * r.nw), 8);
    double *host_p = calloc((size_t)n, 8), *ref_p = calloc((size_t)n, 8), *cm = calloc((size_t)n, 8), *cv = calloc((size_t)n, 8), *cl = calloc(2048, 8);
    static uint8_t ref_rows[8192]; static double ref_w[16384 * 64], ref_dw[16384 * 64], ref_g[4096];
    gt_init(r.s, hv(ref_p, n));
    coax_t_TrStep ref = gt_reference(r.s, r.c, docs(&r), tokens(&r), order(), begun.run, WARM + runs, (coax_v_u8){ref_rows, 8192}, hv(ref_w, r.nw), hv(ref_dw, r.nw), hv(ref_g, n),
                                     hv(ref_p, n), hv(cm, n), hv(cv, n), hv(r.total, n), hv(r.stage, 3 * n), hv(cl, WARM + runs));
    if (ref.status != COAX_TrStatus_ok) { printf("FAIL: chapter 118's run refused\n"); return 1; }

    static Profile cpu, dev; int failures = 0;
    FILE *o = fopen(argv[2], "w"); if (!o) { perror(argv[2]); return 2; }
    char load[128] = "unknown"; FILE *la = fopen("/proc/loadavg", "r"); if (la) { if (!fgets(load, sizeof load, la)) strcpy(load, "unknown"); fclose(la); load[strcspn(load, "\n")] = 0; }
    char host[64] = "unknown", date[32] = "unknown"; time_t clock_now = time(NULL); gethostname(host, sizeof host); host[63] = 0;
    strftime(date, sizeof date, "%Y-%m-%dT%H:%M:%SZ", gmtime(&clock_now));
    const char *coax_build = getenv("COAX_BUILD");
    fprintf(o, "{\n  \"schema\": \"psionic.train-profile.v1\",\n  \"profile\": \"tp-step-v1\",\n  \"host\": \"%s\", \"date\": \"%s\",\n  \"cc\": \"gcc %s -O2 -ffp-contract=off -fno-fast-math\", \"coax\": \"%s\", \"cuda\": \"NVRTC --fmad=false\",\n",
            host, date, __VERSION__, coax_build ? coax_build : "unknown");
    fprintf(o, "  \"rows\": %lld, \"warm\": %d, \"parameters\": %lld, \"plan_bytes\": %lld,\n  \"loadavg_before\": \"%s\",\n",
            (long long)rows, WARM, (long long)n, (long long)r.plan.bytes, load);

    failures += profile(&r, 0, runs, &cpu, host_p, ref_p, begun.run);
    summary("CPU, one thread", &cpu); json(o, "cpu", &cpu); fprintf(o, ",\n");

    PcDeviceInfo info; PcStatus st = pc_open(&info);
    if (st == PC_ABSENT || st == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); fprintf(o, "  \"device\": null\n}\n"); fclose(o); return failures != 0; }
    TRY(st);
    size_t len; char *src = read_all(argv[1], &len); double seconds = 0;
    PcModule module; TRY(pc_compile(src, len, "profile-kernels.cu", &module, &seconds));
    for (int k = 0; k < NK; k++) TRY(pc_kernel(module, kernel_names[k], &kernels[k]));
    printf("device: %s (sm_%d%d, %d SMs); NVRTC compile %.3f s, kept apart from the step\n", info.name, info.major, info.minor, info.sms, seconds);
    alloc_plan(&r);
    failures += profile(&r, 1, runs, &dev, host_p, ref_p, begun.run);
    summary("GPU", &dev);
    fprintf(o, "  \"device\": {\"name\": \"%s\", \"sm\": \"%d%d\", \"sms\": %d, \"driver\": %d, \"compile_s\": %.3f},\n", info.name, info.major, info.minor, info.sms, info.driver_version, seconds);
    json(o, "gpu", &dev);
    la = fopen("/proc/loadavg", "r"); if (la) { if (!fgets(load, sizeof load, la)) strcpy(load, "unknown"); fclose(la); load[strcspn(load, "\n")] = 0; }
    fprintf(o, ",\n  \"loadavg_after\": \"%s\"\n}\n", load); fclose(o);
    for (int k = 0; k < NB; k++) pc_free(buf[k]);
    if (failures) { printf("FAIL: %d targets\n", failures); return 1; }
    printf("chapter 130: both profiles account for their steps within 5%%, and the profiled runs match chapter 118's within the frozen tolerance\n");
    return 0;
}
