/* Chapter 132 on host threads: chapter 115's tiny decoder (3152 parameters,
   rows of 32 tokens, 16 rows a step) trained by a group of D logical
   devices, each device one POSIX thread. The plan, the rows each device
   owns, the gather, the all-reduce, the step and its refusals are Coax
   (psionic/lib/transformer/device-group.coax compiled to C); this file only
   starts threads, stamps each report with the time its thread finished, and
   times steps. It checks:
     1. the chain all-reduce at 1, 2, 4, 8 and 16 devices gives chapter 118's
        CPU run (gt_reference) bit for bit: losses, parameters, moments;
     2. the tree all-reduce gives the same bits at every device count, within
        GT_TOLERANCE of chapter 118's run;
     3. a step whose reports all come after the deadline is refused and
        changes nothing, and the same batch retried gives the uninterrupted
        run's bits.
   It then prints a speed receipt (not a transcript: times vary).
   Usage: device-group-threads [STEPS] [MAX_DEVICES] */
#define _POSIX_C_SOURCE 199309L
#include "device-group.h"
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define ROWS 16
#define NP 3152
#define NW 13984
#define CTX 32

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec * 1e-9; }
static coax_v_f64 hv(double *d, int64_t n) { return (coax_v_f64){d, n}; }

static coax_t_TtFix fix;
static coax_t_TdShape shape;
static coax_t_TrConfig config;
static coax_v_t_PkDoc docs(void) { return (coax_v_t_PkDoc){fix.docs.e, 47}; }
static coax_v_u8 tokens(void) { return (coax_v_u8){fix.tokens.e, fix.n}; }
static int64_t no_order[1];
static coax_v_s64 order(void) { return (coax_v_s64){no_order, 0}; }

/* Every buffer of one group: replicated parameters and moments, each row's
   slices of work and gradients, and the all-reduce's slots. */
typedef struct {
    double p[NP], m[NP], v[NP], stage[3 * NP], total[NP], scalars[3], losses[ROWS], slots[ROWS * NP], rope[CTX * 8];
    double grads[ROWS * NP], work[ROWS * NW], dwork[ROWS * NW];
    uint8_t rows[ROWS * CTX];
    coax_t_DgReport reports[ROWS];
} Group;
typedef struct { Group *g; coax_t_DgPlan plan; coax_t_TrBatch b; int64_t d; double t0; } Job;

static void *device(void *arg) {
    Job *j = arg; Group *g = j->g;
    coax_t_DgReport r = dg_rows(shape, j->plan, j->d, j->b, (coax_v_u8){g->rows, ROWS * CTX}, hv(g->rope, CTX * 8), hv(g->p, NP),
                                hv(g->work, ROWS * NW), hv(g->dwork, ROWS * NW), hv(g->grads, ROWS * NP), hv(g->losses, ROWS));
    r.at = (int64_t)((now() - j->t0) * 1e6); /* microseconds since the step began */
    g->reports[j->d] = r;
    return NULL;
}
static void start(Group *g) {
    gt_init(shape, hv(g->p, NP)); memset(g->m, 0, sizeof g->m); memset(g->v, 0, sizeof g->v);
    gt_rope(shape, hv(g->rope, CTX * 8));
}
/* One step: every device on its own thread, then the gather and the rest. */
static coax_t_DgStep step(Group *g, coax_t_DgPlan plan, coax_t_TrRun run, coax_t_TrBatch b, int64_t deadline) {
    pthread_t t[ROWS]; Job jobs[ROWS]; double t0 = now();
    for (int64_t d = 0; d < plan.devices; d++) {
        jobs[d] = (Job){g, plan, b, d, t0};
        if (pthread_create(&t[d], NULL, device, &jobs[d]) != 0) { perror("pthread_create"); exit(2); }
    }
    for (int64_t d = 0; d < plan.devices; d++) pthread_join(t[d], NULL);
    return dg_finish(config, plan, run, b, (coax_v_t_DgReport){g->reports, plan.devices}, deadline, hv(g->p, NP), hv(g->m, NP), hv(g->v, NP),
                     hv(g->stage, 3 * NP), hv(g->total, NP), hv(g->grads, ROWS * NP), hv(g->losses, ROWS), hv(g->scalars, 3), hv(g->slots, ROWS * NP));
}
#define DEADLINE 60000000 /* one minute, in microseconds */
static int run_group(Group *g, coax_t_DgPlan plan, coax_t_TrRun run, int64_t steps, double *losses, double *seconds) {
    start(g); double t0 = now();
    for (int64_t k = 0; k < steps; k++) {
        coax_t_TrBatch b = gt_next(config, run, docs(), tokens(), (coax_v_u8){g->rows, ROWS * CTX});
        if (b.status != COAX_TrStatus_ok) return 1;
        coax_t_DgStep s = step(g, plan, run, b, DEADLINE);
        if (s.status != COAX_DgStatus_ok || s.step.status != COAX_TrStatus_ok) { printf("FAIL: step %lld: group status %d, trainer status %d\n", (long long)k, s.status, s.step.status); return 1; }
        run = s.step.run; losses[k] = s.step.loss;
    }
    *seconds = now() - t0;
    return run.revision != steps;
}
static int same(const double *a, const double *b, int64_t n) {
    for (int64_t i = 0; i < n; i++) { uint64_t x, y; memcpy(&x, a + i, 8); memcpy(&y, b + i, 8); if (x != y) return 0; }
    return 1;
}

int main(int argc, char **argv) {
    int64_t steps = argc > 1 ? atoll(argv[1]) : 6, most = argc > 2 ? atoll(argv[2]) : 16;
    if (steps < 2 || steps > 100 || most < 1 || most > ROWS) { fprintf(stderr, "usage: device-group-threads [STEPS 2-100] [MAX_DEVICES 1-16]\n"); return 2; }
    fix = gt_fix(); shape = gt_tiny(); config = gt_config(shape, ROWS, steps);
    coax_t_TrState begun = gt_begin(config, NP, docs(), tokens(), order());
    if (begun.status != COAX_TrStatus_ok) { printf("FAIL: the trainer refused the run\n"); return 1; }
    int failures = 0;

    /* Chapter 118's own CPU run of the same configuration. */
    static double rp[NP], rm[NP], rv[NP], rt[NP], rs[3 * NP], rl[100], rw[NW], rdw[NW], rg[NP]; static uint8_t rrows[ROWS * CTX];
    gt_init(shape, hv(rp, NP));
    double t0 = now();
    coax_t_TrStep ref = gt_reference(shape, config, docs(), tokens(), order(), begun.run, steps, (coax_v_u8){rrows, ROWS * CTX}, hv(rw, NW), hv(rdw, NW), hv(rg, NP),
                                     hv(rp, NP), hv(rm, NP), hv(rv, NP), hv(rt, NP), hv(rs, 3 * NP), hv(rl, steps));
    double ref_s = now() - t0;
    if (ref.status != COAX_TrStatus_ok) { printf("FAIL: chapter 118's run refused\n"); return 1; }
    printf("%lld steps of %d rows of chapter 115's decoder (%d parameters), each device a host thread\n", (long long)steps, ROWS, NP);

    Group *g = malloc(sizeof(Group)), *first = malloc(sizeof(Group));
    double losses[100], seconds[2][5]; int counts[5] = {1, 2, 4, 8, 16}, used = 0;
    for (int k = 0; k < 2; k++) {
        coax_t_DgCollective kind = k == 0 ? COAX_DgCollective_chain : COAX_DgCollective_tree;
        printf("%s all-reduce:\n", k == 0 ? "chain" : "tree");
        for (int i = 0; i < 5 && counts[i] <= most; i++) {
            coax_t_DgPlan plan = dg_plan(counts[i], ROWS, kind);
            if (plan.status != COAX_DgStatus_ok) { printf("FAIL: plan refused\n"); return 1; }
            failures += run_group(g, plan, begun.run, steps, losses, &seconds[k][i]);
            int bits = same(losses, rl, steps) && same(g->p, rp, NP) && same(g->m, rm, NP) && same(g->v, rv, NP);
            if (i == 0) memcpy(first, g, sizeof(Group));
            int stable = same(g->p, first->p, NP) && same(g->m, first->m, NP) && same(g->v, first->v, NP);
            coax_t_GtGap gap = gt_gap(hv(g->p, NP), hv(rp, NP), gt_tolerance());
            printf("  %2d devices: equal to chapter 118's run bit for bit: %s; equal to 1 device: %s; parameters %lld differ in bits, %lld past the tolerance\n",
                   counts[i], bits ? "yes" : "no", stable ? "yes" : "no", (long long)gap.differ, (long long)gap.far);
            failures += !stable || gap.far != 0 || (k == 0 && !bits);
            if (k == 0) used = i + 1;
        }
    }

    /* A step every device reports after the deadline: refused, nothing written. */
    {
        coax_t_DgPlan plan = dg_plan(used > 3 ? 8 : 1, ROWS, COAX_DgCollective_chain);
        start(g); coax_t_TrRun run = begun.run;
        coax_t_TrBatch b = gt_next(config, run, docs(), tokens(), (coax_v_u8){g->rows, ROWS * CTX});
        double before[NP]; memcpy(before, g->p, sizeof before);
        coax_t_DgStep late = step(g, plan, run, b, -1);
        int kept = same(before, g->p, NP) && g->m[0] == 0 && late.step.run.revision == 0;
        coax_t_DgStep again = step(g, plan, run, b, DEADLINE);
        run = again.step.run;
        for (int64_t k = 1; k < steps && again.status == COAX_DgStatus_ok && again.step.status == COAX_TrStatus_ok; k++) {
            b = gt_next(config, run, docs(), tokens(), (coax_v_u8){g->rows, ROWS * CTX});
            again = step(g, plan, run, b, DEADLINE); run = again.step.run;
        }
        int bits = again.status == COAX_DgStatus_ok && run.revision == steps && same(g->p, rp, NP) && same(g->v, rv, NP);
        printf("a step with every report past the deadline at %lld devices: %s, device %lld named, parameters and run unchanged: %s; retried, the run ends with chapter 118's bits: %s\n",
               (long long)plan.devices, late.status == COAX_DgStatus_late ? "refused as late" : "NOT REFUSED", (long long)late.device, kept ? "yes" : "no", bits ? "yes" : "no");
        failures += late.status != COAX_DgStatus_late || late.device != 0 || !kept || !bits;
    }

    /* Speed receipt: times vary from run to run; recorded in the chapter by hand. */
    double tok = (double)(steps * ROWS * CTX);
    printf("receipt: chapter 118's CPU run %.2f ms/step (%.0f tokens/s)\n", ref_s / steps * 1e3, tok / ref_s);
    for (int i = 0; i < used; i++)
        printf("receipt: %2d devices, chain %.2f ms/step (%.0f tokens/s, %.2fx one device), tree %.2f ms/step\n", counts[i], seconds[0][i] / steps * 1e3,
               tok / seconds[0][i], seconds[0][0] / seconds[0][i], seconds[1][i] / steps * 1e3);
    free(g); free(first);
    if (failures) { printf("FAIL: %d checks\n", failures); return 1; }
    printf("chapter 132 on host threads: the chain matches chapter 118 bit for bit at every device count, the tree matches itself, a late step commits nothing\n");
    return 0;
}
