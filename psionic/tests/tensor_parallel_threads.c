/* Chapter 183 on host threads: a decoder of chapter 113's family (vocabulary
   256, width 128, 8 heads over 4 key-value heads, 256 FFN columns, 4
   layers, context 64; 525,440 parameters) split over D logical devices,
   each device one POSIX thread. The plan, the slices, the halves of every
   block, the all-reduce and its order are Coax
   (psionic/lib/tensor-parallel-run.coax compiled to C); this file only
   starts threads, puts a barrier between the steps tl_run takes one after
   another, and times them. It checks:
     1. at 1, 2, 4 and 8 devices the threads give tl_run's logits bit for bit;
     2. at 1 device that is chapter 113's td_forward bit for bit, and at
        every count within TL_TOLERANCE (1e-12) of it, with the same greedy
        token at every position;
     3. decoding one position at a time gives the prefill's logits bit for bit.
   It then prints a speed receipt (not a transcript: times vary).
   Usage: tensor-parallel-threads [REPEATS] [MAX_DEVICES] */
#define _POSIX_C_SOURCE 200809L
#include "tensor-parallel.h"
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define CTX 64
#define MAXD 8

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec * 1e-9; }
static coax_v_f64 hv(double *d, int64_t n) { return (coax_v_f64){d, n}; }

static const coax_t_TdShape shape = {256, 128, 8, 4, 16, 256, 4, CTX};
static int64_t tokens[CTX];

/* One split: every device's shard and work, the shared addends and sums. */
typedef struct {
    coax_t_TlPlan plan;
    double *shards, *works, *partials, *reduced, *logits;
    int64_t ns, nw, np, nr;
    pthread_barrier_t bar;
    int64_t first, n, repeats;
} Split;
typedef struct { Split *s; int64_t d; } Job;

static void *device(void *arg) {
    Job *j = arg; Split *s = j->s; coax_t_TlPlan p = s->plan; int64_t d = j->d;
    coax_v_f64 sh = hv(s->shards, s->ns), wk = hv(s->works, s->nw), pa = hv(s->partials, s->np), re = hv(s->reduced, s->nr);
    for (int64_t r = 0; r < s->repeats; r++) {
        /* repeats > 1 is the prefill; repeats == CTX with n == 1 decodes one position a step. */
        int64_t first = s->n == 1 ? r : s->first, n = s->n;
        tl_embed(p, d, sh, wk, (coax_v_s64){tokens + first, n}, first);
        for (int64_t l = 0; l < p.shape.layers; l++) {
            tl_attend(p, d, l, sh, wk, first, n, pa); pthread_barrier_wait(&s->bar);
            tl_reduce(p, d, wk, first, n, pa, re); pthread_barrier_wait(&s->bar);
            tl_gather(p, d, wk, first, n, re);
            tl_ffn(p, d, l, sh, wk, first, n, pa); pthread_barrier_wait(&s->bar);
            tl_reduce(p, d, wk, first, n, pa, re); pthread_barrier_wait(&s->bar);
            tl_gather(p, d, wk, first, n, re);
        }
        if (d == 0) tl_head(p, sh, wk, first, n, hv(s->logits + (s->n == 1 ? first * shape.vocab : 0), n * shape.vocab));
        pthread_barrier_wait(&s->bar);
    }
    return NULL;
}
static double run_threads(Split *s, int64_t first, int64_t n, int64_t repeats) {
    pthread_t t[MAXD]; Job jobs[MAXD]; int64_t D = s->plan.devices;
    s->first = first; s->n = n; s->repeats = repeats;
    pthread_barrier_init(&s->bar, NULL, (unsigned)D);
    double t0 = now();
    for (int64_t d = 0; d < D; d++) {
        jobs[d] = (Job){s, d};
        if (pthread_create(&t[d], NULL, device, &jobs[d]) != 0) { perror("pthread_create"); exit(2); }
    }
    for (int64_t d = 0; d < D; d++) pthread_join(t[d], NULL);
    double sec = now() - t0;
    pthread_barrier_destroy(&s->bar);
    return sec;
}
static int same(const double *a, const double *b, int64_t n) {
    for (int64_t i = 0; i < n; i++) { uint64_t x, y; memcpy(&x, a + i, 8); memcpy(&y, b + i, 8); if (x != y) return 0; }
    return 1;
}
static double gap(const double *a, const double *b, int64_t n) {
    double g = 0; for (int64_t i = 0; i < n; i++) { double x = a[i] - b[i]; if (x < 0) x = -x; if (x > g) g = x; }
    return g;
}
static int64_t argmax(const double *x, int64_t n) { int64_t b = 0; for (int64_t i = 1; i < n; i++) if (x[i] > x[b]) b = i; return b; }

int main(int argc, char **argv) {
    int64_t repeats = argc > 1 ? atoll(argv[1]) : 10, most = argc > 2 ? atoll(argv[2]) : MAXD;
    if (repeats < 1 || repeats > 1000 || most < 1 || most > MAXD) { fprintf(stderr, "usage: tensor-parallel-threads [REPEATS 1-1000] [MAX_DEVICES 1-8]\n"); return 2; }
    int64_t V = shape.vocab, NP = 2 * V * 128 + 128 + 4 * (2 * 128 + 128 * 128 + 2 * 64 * 128 + 128 * 128 + 2 * 256 * 128), NW = 1 << 20;
    double *p = malloc(NP * sizeof(double)), *w = malloc(NW * sizeof(double)), *ref = malloc(CTX * V * sizeof(double));
    td_init(shape, (coax_t_CrSeed){183, 0}, hv(p, NP));
    for (int64_t t = 0; t < CTX; t++) tokens[t] = (t * 37 + 11) % V;
    double t0 = now();
    coax_t_TdResult r = td_forward(shape, hv(p, NP), (coax_v_s64){tokens, CTX}, hv(w, NW), hv(ref, CTX * V), 0);
    double ref_s = now() - t0;
    if (r.status != COAX_TdStatus_ok) { printf("FAIL: chapter 113's forward refused\n"); return 1; }
    printf("a %lld-parameter decoder, %d positions, split over host threads\n", (long long)NP, CTX);

    int failures = 0, counts[4] = {1, 2, 4, 8};
    double prefill[4] = {0}, decode[4] = {0}, seq[4] = {0};
    for (int i = 0; i < 4 && counts[i] <= most; i++) {
        Split s = {0};
        s.plan = tl_plan(shape, counts[i], (int64_t)1 << 40);
        if (s.plan.status != COAX_TlStatus_ok) { printf("FAIL: plan refused at %d devices\n", counts[i]); return 1; }
        s.ns = s.plan.devices * s.plan.shard; s.nw = s.plan.devices * (s.plan.cache + s.plan.work); s.np = s.plan.devices * CTX * shape.width; s.nr = CTX * shape.width;
        s.shards = malloc(s.ns * sizeof(double)); s.works = calloc(s.nw, sizeof(double)); s.partials = calloc(s.np, sizeof(double)); s.reduced = calloc(s.nr, sizeof(double));
        s.logits = malloc(CTX * V * sizeof(double));
        double *one = malloc(CTX * V * sizeof(double)), *steps = malloc(CTX * V * sizeof(double));
        if (tl_split(s.plan, hv(p, NP), hv(s.shards, s.ns)) != COAX_TlStatus_ok) { printf("FAIL: split refused\n"); return 1; }
        /* tl_run: the devices one after another. */
        t0 = now();
        coax_t_TlResult k = tl_run(s.plan, hv(s.shards, s.ns), hv(s.works, s.nw), hv(s.partials, s.np), hv(s.reduced, s.nr), (coax_v_s64){tokens, CTX}, 0, hv(one, CTX * V));
        seq[i] = now() - t0;
        if (k.status != COAX_TlStatus_ok) { printf("FAIL: tl_run refused\n"); return 1; }
        /* The threads: the prefill, repeated for the timing, then one position a step. */
        prefill[i] = run_threads(&s, 0, CTX, repeats) / repeats;
        int threads = same(s.logits, one, CTX * V);
        memcpy(steps, s.logits, CTX * V * sizeof(double));
        decode[i] = run_threads(&s, 0, 1, CTX) / CTX;
        int stepped = same(s.logits, steps, CTX * V);
        int exact = same(one, ref, CTX * V), greedy = 1;
        for (int64_t t = 0; t < CTX; t++) greedy &= argmax(one + t * V, V) == argmax(ref + t * V, V);
        double g = gap(one, ref, CTX * V);
        printf("  %d devices: %lld bytes a device, %lld all-reduce bytes a token; threads equal tl_run bit for bit: %s; decode equals prefill: %s; "
               "equal to td_forward bit for bit: %s; largest gap %.3g; greedy tokens agree: %s\n",
               counts[i], (long long)s.plan.bytes, (long long)s.plan.token, threads ? "yes" : "no", stepped ? "yes" : "no", exact ? "yes" : "no", g, greedy ? "yes" : "no");
        failures += !threads || !stepped || !greedy || !(g < 1e-12) || (counts[i] == 1 && !exact);
        free(s.shards); free(s.works); free(s.partials); free(s.reduced); free(s.logits); free(one); free(steps);
    }
    /* Speed receipt: times vary from run to run; recorded in the chapter by hand. */
    printf("receipt: td_forward, one thread: %.2f ms for %d positions\n", ref_s * 1e3, CTX);
    for (int i = 0; i < 4 && counts[i] <= most; i++)
        printf("receipt: %d devices: tl_run in turn %.2f ms; threads: prefill %.2f ms (%.2fx one device), decode %.1f us a position (%.2fx)\n", counts[i], seq[i] * 1e3,
               prefill[i] * 1e3, prefill[0] / prefill[i], decode[i] * 1e6, decode[0] / decode[i]);
    free(p); free(w); free(ref);
    if (failures) { printf("FAIL: %d checks\n", failures); return 1; }
    printf("chapter 183 on host threads: every split equals its own sequential run bit for bit and chapter 113's forward within 1e-12\n");
    return 0;
}
