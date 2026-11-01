/* Chapter 129 on a CUDA device: chapter 115's tiny decoder trained by
   chapter 118's trainer with every step's arithmetic in the Coax-generated
   kernels of psionic/lib/transformer/train-kernels.coax. The plan, the batch
   order, the step decisions, checkpoints and comparisons are Coax
   (gpu-train.coax compiled to C); this file only allocates, copies,
   launches and times. It checks:
     1. the plan's bytes against the hand count, the buffers actually
        allocated against the plan, and a budget one byte short refused;
     2. the step pieces with a fixed summation order (mean, squared norm,
        AdamW, admission) bit for bit against the C build on equal inputs;
     3. a run of STEPS steps against chapter 118's CPU run within
        GT_TOLERANCE, frozen in gpu-train.coax before the first device run;
     4. a device run saved after half its steps and resumed into fresh
        buffers, bit for bit against the uninterrupted device run.
   It then prints a speed receipt (not a transcript: times vary).
   Usage: gpu-train-device KERNELS.cu [STEPS]. Prints SKIP without a CUDA
   device or NVRTC. */
#define _POSIX_C_SOURCE 199309L
#include "gpu-train.h"
#include "cuda-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
#define HAND_BYTES 1174648 /* GT_HAND_BYTES in gpu-train.coax */

enum { B_P, B_M, B_V, B_STAGE, B_TOTAL, B_GRADS, B_WORK, B_DWORK, B_ROWS, B_ROPE, B_LOSS, B_SCALARS, B_SHAPE, NB };
enum { K_ROWS, K_MEAN, K_SQUARES, K_CLIP, K_ADAMW, K_ADMIT, K_COMMIT, NK };
static const char *kernel_names[NK] = {"gk_rows", "gk_mean", "gk_squares", "gk_clip", "gk_adamw", "gk_admit", "gk_commit"};
static PcKernel kernels[NK];
static PcBuffer buf[NB];
static int64_t elem[NB] = {8, 8, 8, 8, 8, 8, 8, 8, 1, 8, 8, 8, 8};

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec * 1e-9; }
static char *read_all(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *t = malloc((size_t)size + 1);
    if (!t || fread(t, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); t[size] = 0; *n = (size_t)size; return t;
}
static PcArg view(int b, int64_t first, int64_t count) { return (PcArg){PC_ARG_VIEW, 0, 0, buf[b] + (PcBuffer)(first * elem[b]), count}; }
static PcArg whole(int b, const coax_t_GtPlan *plan) { return view(b, 0, plan->sizes.e[b] / elem[b]); }
static PcArg integer(int64_t i) { return (PcArg){PC_ARG_INT, i, 0, 0, 0}; }
static PcArg real(double f) { return (PcArg){PC_ARG_F64, 0, f, 0, 0}; }
static void launch(int k, int64_t items, PcArg *a, int count) { TRY(pc_launch(kernels[k], items, items < 128 ? 32 : 128, a, count, NULL)); }
static coax_v_f64 hv(double *d, int64_t n) { return (coax_v_f64){d, n}; }

typedef struct {
    coax_t_TdShape s; coax_t_TrConfig c; coax_t_GtPlan plan; coax_t_TtFix *fix; int64_t n;
} Run;
static coax_v_t_PkDoc docs(Run *r) { return (coax_v_t_PkDoc){r->fix->docs.e, 47}; }
static coax_v_u8 tokens(Run *r) { return (coax_v_u8){r->fix->tokens.e, r->fix->n}; }
static int64_t no_order[1];
static coax_v_s64 order(void) { return (coax_v_s64){no_order, 0}; }
/* Host scratch for chapter 118's CPU run: a step's rows, one row's work and gradient. */
static uint8_t ref_rows[4096]; static double ref_w[16384], ref_dw[16384], ref_g[4096];
#define REF(r) (coax_v_u8){ref_rows, 4096}, hv(ref_w, (r)->plan.work), hv(ref_dw, (r)->plan.work), hv(ref_g, (r)->n)
#define TOL gt_tolerance()

/* One step: the launches of gt_step_cpu, in its order, on the device. */
static coax_t_TrStep device_step(Run *r, coax_t_TrRun run, coax_t_TrBatch b, uint8_t *rows) {
    const coax_t_GtPlan *pl = &r->plan; int64_t n = r->n, micro = b.micro; double losses[64], scalars[3];
    TRY(pc_upload(buf[B_ROWS], rows, (size_t)(micro * r->c.length), NULL));
    PcArg a[14];
    a[0] = view(B_SHAPE, 0, 8); a[1] = whole(B_P, pl); a[2] = whole(B_ROWS, pl); a[3] = whole(B_ROPE, pl);
    a[4] = whole(B_WORK, pl); a[5] = whole(B_DWORK, pl); a[6] = whole(B_GRADS, pl); a[7] = whole(B_LOSS, pl);
    launch(K_ROWS, micro, a, 8);
    a[0] = integer(micro); a[1] = integer(n); a[2] = whole(B_GRADS, pl); a[3] = whole(B_TOTAL, pl);
    launch(K_MEAN, n, a, 4);
    a[0] = integer(n); a[1] = whole(B_TOTAL, pl); a[2] = view(B_SCALARS, 0, 1);
    launch(K_SQUARES, 1, a, 3);
    TRY(pc_download(losses, buf[B_LOSS], (size_t)micro * 8, NULL));
    TRY(pc_download(scalars, buf[B_SCALARS], 8, NULL));
    coax_t_GtDecide d = gt_decide(r->c, run, b, hv(losses, micro), scalars[0]);
    if (d.status != COAX_TrStatus_ok) return (coax_t_TrStep){.status = d.status, .run = run};
    if (d.clip) { a[0] = real(r->c.clip); a[1] = real(d.norm); a[2] = whole(B_TOTAL, pl); launch(K_CLIP, n, a, 3); }
    a[0] = integer(n); a[1] = real(d.rate); a[2] = real(r->c.beta1); a[3] = real(r->c.beta2); a[4] = real(r->c.eps); a[5] = real(0);
    a[6] = real(d.c1); a[7] = real(d.c2); a[8] = whole(B_P, pl); a[9] = whole(B_TOTAL, pl); a[10] = whole(B_M, pl); a[11] = whole(B_V, pl); a[12] = whole(B_STAGE, pl);
    launch(K_ADAMW, n, a, 13);
    a[0] = integer(n); a[1] = real(r->c.bound); a[2] = whole(B_STAGE, pl); a[3] = view(B_SCALARS, 1, 2);
    launch(K_ADMIT, 1, a, 4);
    TRY(pc_download(scalars, buf[B_SCALARS], 24, NULL));
    coax_t_TrStep st = gt_commit(r->c, run, b, d, hv(scalars + 1, 2));
    if (st.status == COAX_TrStatus_ok) {
        a[0] = integer(n); a[1] = whole(B_STAGE, pl); a[2] = whole(B_P, pl); a[3] = whole(B_M, pl); a[4] = whole(B_V, pl);
        launch(K_COMMIT, n, a, 5);
    }
    return st;
}
/* count steps from run; each step's loss into losses. */
static coax_t_TrStep device_steps(Run *r, coax_t_TrRun run, int64_t count, double *losses) {
    uint8_t rows[4096];
    for (int64_t k = 0; k < count; k++) {
        coax_t_TrBatch b = gt_next(r->c, run, docs(r), tokens(r), (coax_v_u8){rows, r->c.accumulate * r->c.length});
        if (b.status != COAX_TrStatus_ok) return (coax_t_TrStep){.status = b.status, .run = run};
        coax_t_TrStep st = device_step(r, run, b, rows);
        if (st.status != COAX_TrStatus_ok) return st;
        run = st.run; losses[k] = st.loss;
    }
    TRY(pc_sync());
    return (coax_t_TrStep){.run = run};
}
/* Fresh device state: chapter 113's initialization, zero moments. */
static void device_start(Run *r, double *host_p) {
    gt_init(r->s, hv(host_p, r->n));
    TRY(pc_upload(buf[B_P], host_p, (size_t)r->n * 8, NULL));
    TRY(pc_fill(buf[B_M], 0, (size_t)r->n * 2, NULL)); TRY(pc_fill(buf[B_V], 0, (size_t)r->n * 2, NULL));
}
static void download3(Run *r, double *p, double *m, double *v) {
    TRY(pc_download(p, buf[B_P], (size_t)r->n * 8, NULL)); TRY(pc_download(m, buf[B_M], (size_t)r->n * 8, NULL)); TRY(pc_download(v, buf[B_V], (size_t)r->n * 8, NULL));
}
static void alloc_plan(Run *r, double *rope) {
    for (int k = 0; k < NB; k++) TRY(pc_alloc((size_t)r->plan.sizes.e[k], &buf[k]));
    int64_t sh[8]; gt_shape(r->s, (coax_v_s64){sh, 8});
    TRY(pc_upload(buf[B_SHAPE], sh, 64, NULL));
    gt_rope(r->s, hv(rope, r->s.context * r->s.head_width));
    TRY(pc_upload(buf[B_ROPE], rope, (size_t)(r->s.context * r->s.head_width) * 8, NULL));
}
static void free_plan(void) { for (int k = 0; k < NB; k++) pc_free(buf[k]); }
static int report(const char *what, coax_t_GtGap g) {
    printf("  %-22s %6lld values, %6lld differ in bits, %lld past the tolerance, worst relative gap %.3g\n",
           what, (long long)g.count, (long long)g.differ, (long long)g.far, g.worst);
    return g.far != 0;
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: gpu-train-device KERNELS.cu [STEPS]\n"); return 2; }
    int64_t steps = argc > 2 ? atoll(argv[2]) : 20;
    if (steps < 2 || steps > 400) { fprintf(stderr, "STEPS must be 2 to 400\n"); return 2; }
    PcDeviceInfo info;
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    printf("device: %s (sm_%d%d), %zu MiB free of %zu\n", info.name, info.major, info.minor, info.free_bytes >> 20, info.total_bytes >> 20);
    size_t len; char *src = read_all(argv[1], &len); double seconds = 0;
    PcModule module; TRY(pc_compile(src, len, "train-kernels.cu", &module, &seconds));
    for (int k = 0; k < NK; k++) TRY(pc_kernel(module, kernel_names[k], &kernels[k]));
    int failures = 0;

    static coax_t_TtFix fix; fix = gt_fix();
    Run r = {.s = gt_tiny(), .fix = &fix};
    r.c = gt_config(r.s, 4, steps);
    coax_t_TrState begun = gt_begin(r.c, 3152, docs(&r), tokens(&r), order());
    if (begun.status != COAX_TrStatus_ok) { printf("FAIL: the trainer refused the run\n"); return 1; }
    r.n = 3152;

    /* 1. Memory. */
    size_t free0, total0, free1, total1;
    TRY(pc_memory(&free0, &total0));
    r.plan = gt_plan(r.s, r.c, docs(&r), tokens(&r), (int64_t)free0);
    coax_t_GtPlan tight = gt_plan(r.s, r.c, docs(&r), tokens(&r), r.plan.bytes - 1);
    printf("plan: %lld bytes in %d buffers (hand count %d): %s; with a budget of %lld bytes: %s\n",
           (long long)r.plan.bytes, NB, HAND_BYTES, r.plan.status == COAX_GtStatus_ok ? "admitted" : "refused",
           (long long)tight.bytes - 1, tight.status == COAX_GtStatus_budget ? "refused, the buffers do not fit" : "ADMITTED");
    failures += r.plan.status != COAX_GtStatus_ok || r.plan.bytes != HAND_BYTES || tight.status != COAX_GtStatus_budget;
    if (r.plan.status != COAX_GtStatus_ok) { printf("FAIL: the plan does not fit this device\n"); return 1; }
    double *rope = calloc(4096, 8);
    alloc_plan(&r, rope);
    TRY(pc_memory(&free1, &total1));
    int64_t asked = 0; for (int k = 0; k < NB; k++) asked += r.plan.sizes.e[k];
    printf("allocated: %lld bytes asked in %d allocations; the driver's free memory fell by %zu bytes\n", (long long)asked, NB, free0 - free1);
    failures += asked != r.plan.bytes || free0 - free1 < (size_t)asked;

    int64_t n = r.n, nw = r.plan.work;
    double *p = calloc((size_t)n, 8), *m = calloc((size_t)n, 8), *v = calloc((size_t)n, 8);
    double *cp = calloc((size_t)n, 8), *cm = calloc((size_t)n, 8), *cv = calloc((size_t)n, 8);
    double *total = calloc((size_t)n, 8), *stage = calloc((size_t)(3 * n), 8);
    double *grads = calloc((size_t)(64 * n), 8), *work = calloc((size_t)(64 * nw), 8), *dwork = calloc((size_t)(64 * nw), 8);
    double *dl = calloc(400, 8), *cl = calloc(400, 8);

    /* 2. Fixed-order pieces on equal inputs. The first batch's row
       gradients from the C build go to the device; mean, squared norm,
       AdamW and admission run on both and must agree bit for bit. The
       device's own row kernel is reported against the C build. */
    {
        device_start(&r, p);
        uint8_t rows[128]; coax_t_TrBatch b = gt_next(r.c, begun.run, docs(&r), tokens(&r), (coax_v_u8){rows, 128});
        int64_t sh[8]; gt_shape(r.s, (coax_v_s64){sh, 8}); double losses[4], dlosses[4], sq[3], dsq[3];
        for (int i = 0; i < 4; i++)
            gk_rows(i, (coax_v_s64){sh, 8}, hv(p, n), (coax_v_u8){rows, 128}, hv(rope, 256), hv(work, 4 * nw), hv(dwork, 4 * nw), hv(grads, 4 * n), hv(losses, 4));
        TRY(pc_upload(buf[B_ROWS], rows, 128, NULL));
        PcArg a[14];
        a[0] = view(B_SHAPE, 0, 8); a[1] = whole(B_P, &r.plan); a[2] = whole(B_ROWS, &r.plan); a[3] = whole(B_ROPE, &r.plan);
        a[4] = whole(B_WORK, &r.plan); a[5] = whole(B_DWORK, &r.plan); a[6] = whole(B_GRADS, &r.plan); a[7] = whole(B_LOSS, &r.plan);
        launch(K_ROWS, 4, a, 8);
        double *dg = calloc((size_t)(4 * n), 8);
        TRY(pc_sync()); TRY(pc_download(dg, buf[B_GRADS], (size_t)(4 * n) * 8, NULL)); TRY(pc_download(dlosses, buf[B_LOSS], 32, NULL));
        printf("one step's rows, device against the C build of the same kernel:\n");
        failures += report("row gradients", gt_gap(hv(dg, 4 * n), hv(grads, 4 * n), TOL));
        failures += report("row losses", gt_gap(hv(dlosses, 4), hv(losses, 4), TOL));
        TRY(pc_upload(buf[B_GRADS], grads, (size_t)(4 * n) * 8, NULL));
        for (int64_t i = 0; i < n; i++) gk_mean(i, 4, n, hv(grads, 4 * n), hv(total, n));
        gk_squares(0, n, hv(total, n), hv(sq, 1));
        coax_t_GtDecide d = gt_decide(r.c, begun.run, b, hv(losses, 4), sq[0]);
        for (int64_t i = 0; i < n; i++) gk_adamw(i, n, d.rate, r.c.beta1, r.c.beta2, r.c.eps, 0, d.c1, d.c2, hv(p, n), hv(total, n), hv(m, n), hv(v, n), hv(stage, 3 * n));
        gk_admit(0, n, r.c.bound, hv(stage, 3 * n), hv(sq + 1, 2));
        a[0] = integer(4); a[1] = integer(n); a[2] = whole(B_GRADS, &r.plan); a[3] = whole(B_TOTAL, &r.plan); launch(K_MEAN, n, a, 4);
        a[0] = integer(n); a[1] = whole(B_TOTAL, &r.plan); a[2] = view(B_SCALARS, 0, 1); launch(K_SQUARES, 1, a, 3);
        a[0] = integer(n); a[1] = real(d.rate); a[2] = real(r.c.beta1); a[3] = real(r.c.beta2); a[4] = real(r.c.eps); a[5] = real(0);
        a[6] = real(d.c1); a[7] = real(d.c2); a[8] = whole(B_P, &r.plan); a[9] = whole(B_TOTAL, &r.plan); a[10] = whole(B_M, &r.plan); a[11] = whole(B_V, &r.plan); a[12] = whole(B_STAGE, &r.plan);
        launch(K_ADAMW, n, a, 13);
        a[0] = integer(n); a[1] = real(r.c.bound); a[2] = whole(B_STAGE, &r.plan); a[3] = view(B_SCALARS, 1, 2); launch(K_ADMIT, 1, a, 4);
        double *dt = calloc((size_t)n, 8), *ds = calloc((size_t)(3 * n), 8);
        TRY(pc_sync()); TRY(pc_download(dt, buf[B_TOTAL], (size_t)n * 8, NULL)); TRY(pc_download(ds, buf[B_STAGE], (size_t)(3 * n) * 8, NULL)); TRY(pc_download(dsq, buf[B_SCALARS], 24, NULL));
        printf("the fixed-order pieces on the same row gradients:\n");
        coax_t_GtGap g1 = gt_gap(hv(dt, n), hv(total, n), TOL), g2 = gt_gap(hv(dsq, 3), hv(sq, 3), TOL), g3 = gt_gap(hv(ds, 3 * n), hv(stage, 3 * n), TOL);
        report("mean gradient", g1); report("norm and flags", g2); report("AdamW stage", g3);
        failures += g1.differ != 0 || g2.differ != 0 || g3.differ != 0;
        free(dg); free(dt); free(ds);
    }

    /* 3. The run against chapter 118's CPU run. */
    device_start(&r, p);
    double t0 = now();
    coax_t_TrStep dev = device_steps(&r, begun.run, steps, dl);
    double device_s = now() - t0;
    download3(&r, p, m, v);
    gt_init(r.s, hv(cp, n)); memset(cm, 0, (size_t)n * 8); memset(cv, 0, (size_t)n * 8);
    t0 = now();
    coax_t_TrStep cpu = gt_reference(r.s, r.c, docs(&r), tokens(&r), order(), begun.run, steps, REF(&r), hv(cp, n), hv(cm, n), hv(cv, n), hv(total, n), hv(stage, 3 * n), hv(cl, steps));
    double cpu_s = now() - t0;
    printf("%lld steps of 4 rows, device against chapter 118's CPU run (tolerance %g, frozen before the first run):\n", (long long)steps, TOL);
    if (dev.status != COAX_TrStatus_ok || cpu.status != COAX_TrStatus_ok) { printf("FAIL: device status %d, CPU status %d\n", dev.status, cpu.status); return 1; }
    failures += report("losses", gt_gap(hv(dl, steps), hv(cl, steps), TOL));
    failures += report("parameters", gt_gap(hv(p, n), hv(cp, n), TOL));
    failures += report("first moments", gt_gap(hv(m, n), hv(cm, n), TOL));
    failures += report("second moments", gt_gap(hv(v, n), hv(cv, n), TOL));
    printf("  loss: step 1 %.6f, step %lld %.6f\n", dl[0], (long long)steps, dl[steps - 1]);
    failures += dev.run.revision != steps || dl[steps - 1] >= dl[0];

    /* 4. Save after half the steps, resume into fresh buffers. */
    {
        int64_t half = steps / 2;
        coax_t_CkIds ids = gt_ids(r.s, r.c, docs(&r), tokens(&r));
        int64_t size = gt_size(n); uint8_t *bytes = malloc((size_t)size), pointer[80];
        double *rl = calloc(400, 8);
        device_start(&r, cp);
        coax_t_TrStep a = device_steps(&r, begun.run, half, rl);
        download3(&r, cp, cm, cv);
        coax_t_CkStatus saved = gt_save(ids, a.run, hv(cp, n), hv(cm, n), hv(cv, n), (coax_v_u8){bytes, size}, (coax_v_u8){pointer, 80});
        for (int k = 0; k < NB; k++) if (k != B_SHAPE && k != B_ROPE) TRY(pc_fill(buf[k], 0x7ff80001u, (size_t)r.plan.sizes.e[k] / 4, NULL));
        memset(cp, 0, (size_t)n * 8); memset(cm, 0, (size_t)n * 8); memset(cv, 0, (size_t)n * 8);
        coax_t_TrResume back = gt_resume(r.c, ids, (coax_v_u8){pointer, 80}, (coax_v_u8){bytes, size}, docs(&r), tokens(&r), hv(cp, n), hv(cm, n), hv(cv, n), order());
        TRY(pc_upload(buf[B_P], cp, (size_t)n * 8, NULL)); TRY(pc_upload(buf[B_M], cm, (size_t)n * 8, NULL)); TRY(pc_upload(buf[B_V], cv, (size_t)n * 8, NULL));
        coax_t_TrStep b = device_steps(&r, back.run, steps - half, rl + half);
        download3(&r, cp, cm, cv);
        coax_t_GtGap g0 = gt_gap(hv(rl, steps), hv(dl, steps), TOL), g1 = gt_gap(hv(cp, n), hv(p, n), TOL), g2 = gt_gap(hv(cm, n), hv(m, n), TOL), g3 = gt_gap(hv(cv, n), hv(v, n), TOL);
        printf("saved after %lld steps (%lld bytes), resumed into buffers filled with NaN: %s; losses, parameters and moments differ in %lld bits from the uninterrupted run\n",
               (long long)half, (long long)size, saved == COAX_CkStatus_ok && back.status == COAX_CkStatus_ok && b.status == COAX_TrStatus_ok ? "ok" : "FAILED",
               (long long)(g0.differ + g1.differ + g2.differ + g3.differ));
        failures += saved != COAX_CkStatus_ok || back.status != COAX_CkStatus_ok || b.status != COAX_TrStatus_ok || g0.differ + g1.differ + g2.differ + g3.differ != 0 || b.run.revision != steps;
        free(bytes); free(rl);
    }

    /* Speed receipt: times vary from run to run, so they are printed here and
       recorded in the chapter by hand, never compared with a transcript. */
    double tokens4 = (double)(steps * 4 * 32);
    printf("receipt, 4 rows a step: device %.2f ms/step (%.0f tokens/s), C build of chapter 118's CPU run %.2f ms/step (%.0f tokens/s)\n",
           device_s / steps * 1e3, tokens4 / device_s, cpu_s / steps * 1e3, tokens4 / cpu_s);
    free_plan();
    {
        Run w = r; w.c = gt_config(w.s, 64, steps); w.plan = gt_plan(w.s, w.c, docs(&w), tokens(&w), (int64_t)free0);
        if (w.plan.status == COAX_GtStatus_ok) {
            alloc_plan(&w, rope);
            coax_t_TrRun run = gt_begin(w.c, n, docs(&w), tokens(&w), order()).run;
            device_start(&w, p);
            t0 = now(); coax_t_TrStep a = device_steps(&w, run, steps, dl); device_s = now() - t0;
            gt_init(w.s, hv(cp, n)); memset(cm, 0, (size_t)n * 8); memset(cv, 0, (size_t)n * 8);
            t0 = now(); coax_t_TrStep b = gt_reference(w.s, w.c, docs(&w), tokens(&w), order(), run, steps, REF(&w), hv(cp, n), hv(cm, n), hv(cv, n), hv(total, n), hv(stage, 3 * n), hv(cl, steps)); cpu_s = now() - t0;
            download3(&w, p, m, v);
            coax_t_GtGap g = gt_gap(hv(p, n), hv(cp, n), TOL);
            double tokens64 = (double)(steps * 64 * 32);
            printf("receipt, 64 rows a step (%lld bytes on the device): device %.2f ms/step (%.0f tokens/s), CPU %.2f ms/step (%.0f tokens/s); parameters %lld past the tolerance\n",
                   (long long)w.plan.bytes, device_s / steps * 1e3, tokens64 / device_s, cpu_s / steps * 1e3, tokens64 / cpu_s, (long long)g.far);
            failures += a.status != COAX_TrStatus_ok || b.status != COAX_TrStatus_ok || g.far != 0;
            free_plan();
        }
    }
    if (failures) { printf("FAIL: %d checks\n", failures); return 1; }
    printf("chapter 129 on CUDA: the decoder trains on the device within the frozen tolerance of chapter 118's CPU run, and resumes bit for bit\n");
    return 0;
}
