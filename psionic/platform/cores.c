// Chapter 35: run Coax kernels on 1, 2, 4 and 8 host threads. The host owns
// the effects only: the threads, a barrier that starts and ends each run, the
// buffers and the monotonic clock. The partition (pc_plan, pc_first), each
// worker's chunks (pc_sum_worker, pc_matvec_worker, pc_heads_worker), the
// publish check (pc_publish) and the tree merge (pc_sum_merge) are Coax, from
// psionic/lib/cores.coax. The harness checks that every worker count gives
// the one-worker bytes, that a cancelled worker publishes nothing, and prints
// one JSON receipt on stdout.
// Build and run with psionic/tests/cores.sh, which records the receipt.
#define _POSIX_C_SOURCE 200809L
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <sys/utsname.h>
#include "cores.h"

enum { REPS = 7, KINDS = 3, COUNTS = 4 };
enum { SUM, MATVEC, HEADS };
static const int counts[COUNTS] = {1, 2, 4, 8};
static const char *kinds[KINDS] = {"sum", "matvec", "heads"};

// One run's inputs. The plan differs per worker count; the chunks do not.
typedef struct {
    int kind;
    coax_t_PcPlan plan;
    coax_v_f32 x, part, m, y, q, k, v, out;
    coax_v_u8 done;
    int64_t tokens;
    int cancel;   // a worker index that returns without running, or -1
} Job;

typedef struct {
    pthread_t thread;
    pthread_barrier_t *start, *end;
    const Job **job;
    int index, quit;
    coax_t_PcStatus status;
} Worker;

static void *worker_main(void *arg) {
    Worker *w = arg;
    for (;;) {
        pthread_barrier_wait(w->start);
        if (w->quit) return NULL;
        const Job *j = *w->job;
        w->status = COAX_PcStatus_ok;
        if (j->cancel != w->index) {
            if (j->kind == SUM) w->status = pc_sum_worker(j->plan, w->index, j->x, j->part, j->done);
            else if (j->kind == MATVEC) w->status = pc_matvec_worker(j->plan, w->index, j->m, j->x, j->y, j->done);
            else w->status = pc_heads_worker(j->plan, w->index, j->q, j->k, j->v, j->out, j->done, j->tokens);
        }
        pthread_barrier_wait(w->end);
    }
}

typedef struct {
    int n;
    pthread_barrier_t start, end;
    const Job *job;
    Worker w[8];
} Pool;

static int pool_open(Pool *p, int n) {
    p->n = n;
    if (pthread_barrier_init(&p->start, NULL, (unsigned)n + 1) || pthread_barrier_init(&p->end, NULL, (unsigned)n + 1)) return 1;
    for (int i = 0; i < n; i++) {
        p->w[i] = (Worker){.start = &p->start, .end = &p->end, .job = &p->job, .index = i};
        if (pthread_create(&p->w[i].thread, NULL, worker_main, &p->w[i])) return 1;
    }
    return 0;
}
// Run one job on every worker; returns the first worker's refusal, if any.
static coax_t_PcStatus pool_run(Pool *p, const Job *j) {
    p->job = j;
    pthread_barrier_wait(&p->start);
    pthread_barrier_wait(&p->end);
    for (int i = 0; i < p->n; i++) if (p->w[i].status != COAX_PcStatus_ok) return p->w[i].status;
    return COAX_PcStatus_ok;
}
static void pool_close(Pool *p) {
    for (int i = 0; i < p->n; i++) p->w[i].quit = 1;
    pthread_barrier_wait(&p->start);
    for (int i = 0; i < p->n; i++) pthread_join(p->w[i].thread, NULL);
    pthread_barrier_destroy(&p->start);
    pthread_barrier_destroy(&p->end);
}

static int64_t now_ns(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (int64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}
static int cmp(const void *a, const void *b) {
    int64_t x = *(const int64_t *)a, y = *(const int64_t *)b;
    return (x > y) - (x < y);
}
static uint64_t fnv(const void *data, size_t n) {
    const unsigned char *b = data;
    uint64_t h = 14695981039346656037ull;
    for (size_t i = 0; i < n; i++) { h ^= b[i]; h *= 1099511628211ull; }
    return h;
}
// Deterministic inputs in [-1, 1).
static void fill(float *a, int64_t n, uint64_t seed) {
    uint64_t s = seed;
    for (int64_t i = 0; i < n; i++) {
        s = s * 6364136223846793005ull + 1442695040888963407ull;
        a[i] = (float)((int64_t)(s >> 40) - (1 << 23)) / (float)(1 << 23);
    }
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

typedef struct { int64_t items, chunk; const char *shape; } Shape;

int main(int argc, char **argv) {
    const char *compiler = argc > 1 ? argv[1] : "unknown";
    const char *flags = argc > 2 ? argv[2] : "unknown";
    int small = argc > 3 && strcmp(argv[3], "small") == 0;
    int reps = small ? 2 : REPS;
    int64_t sum_n = small ? 1 << 14 : 1 << 24, rows = small ? 64 : 4096, cols = rows;
    int64_t heads = 32, dim = small ? 16 : 128, tokens = small ? 64 : 2048;
    Shape shapes[KINDS] = {
        {sum_n, sum_n / 256, small ? "16384 values" : "16777216 values"},
        {rows, rows / 64, small ? "64 x 64" : "4096 x 4096"},
        {heads, 1, small ? "32 heads x 16 wide x 64 tokens" : "32 heads x 128 wide x 2048 tokens"},
    };
    int64_t chunks_max = 256;
    float *x = malloc(sizeof(float) * (size_t)sum_n), *part = malloc(sizeof(float) * (size_t)chunks_max),
          *m = malloc(sizeof(float) * (size_t)(rows * cols)), *y = malloc(sizeof(float) * (size_t)rows),
          *q = malloc(sizeof(float) * (size_t)(heads * dim)), *k = malloc(sizeof(float) * (size_t)(tokens * heads * dim)),
          *v = malloc(sizeof(float) * (size_t)(tokens * heads * dim)), *out = malloc(sizeof(float) * (size_t)(heads * dim));
    float *ref[KINDS] = {malloc(sizeof(float)), malloc(sizeof(float) * (size_t)rows), malloc(sizeof(float) * (size_t)(heads * dim))};
    uint8_t *done = malloc((size_t)chunks_max);
    if (!x || !part || !m || !y || !q || !k || !v || !out || !ref[0] || !ref[1] || !ref[2] || !done) {
        fprintf(stderr, "cores: allocation failed\n");
        return 1;
    }
    fill(x, sum_n, 1); fill(m, rows * cols, 2); fill(q, heads * dim, 3); fill(k, tokens * heads * dim, 4); fill(v, tokens * heads * dim, 5);
    for (int64_t i = 0; i < heads * dim; i++) q[i] *= 0.125f;
    // The sum's values carry a large offset on every other element, so the
    // grouping changes the rounding.
    for (int64_t i = 0; i < sum_n; i += 2) x[i] += 1000.0f;
    // The matrix-vector input is the first cols values of the sum's input.
    coax_v_f32 xs = {x, sum_n}, xm = {x, cols};

    int64_t times[KINDS][COUNTS][REPS];
    uint64_t digest[KINDS][COUNTS];
    for (int c = 0; c < COUNTS; c++) {
        int n = counts[c];
        Pool pool;
        if (pool_open(&pool, n)) { fprintf(stderr, "cores: thread start failed\n"); return 1; }
        for (int kind = 0; kind < KINDS; kind++) {
            coax_t_PcPlan plan = pc_plan(shapes[kind].items, shapes[kind].chunk, n);
            if (plan.status != COAX_PcStatus_ok) { fprintf(stderr, "cores: plan refused\n"); return 1; }
            Job j = {.kind = kind, .plan = plan, .x = kind == SUM ? xs : xm, .part = {part, plan.chunks},
                     .m = {m, rows * cols}, .y = {y, rows}, .q = {q, heads * dim}, .k = {k, tokens * heads * dim},
                     .v = {v, tokens * heads * dim}, .out = {out, heads * dim}, .done = {done, plan.chunks},
                     .tokens = tokens, .cancel = -1};
            for (int r = -1; r < reps; r++) {   // run -1 warms up
                memset(done, 0, (size_t)plan.chunks);
                int64_t t = now_ns();
                coax_t_PcStatus s = pool_run(&pool, &j);
                float total = 0;
                if (s == COAX_PcStatus_ok && kind == SUM) {
                    coax_t_PcValue value = pc_sum_merge(plan, j.part, j.done);
                    s = value.status;
                    total = value.value;
                } else if (s == COAX_PcStatus_ok) {
                    s = pc_publish(plan, j.done);
                }
                if (r >= 0) times[kind][c][r] = now_ns() - t;
                if (s != COAX_PcStatus_ok) { fprintf(stderr, "cores: %s at %d workers refused (%d)\n", kinds[kind], n, s); return 1; }
                const float *got = kind == SUM ? &total : kind == MATVEC ? y : out;
                size_t bytes = sizeof(float) * (size_t)(kind == SUM ? 1 : kind == MATVEC ? rows : heads * dim);
                if (c == 0 && r == -1) memcpy(ref[kind], got, bytes);
                if (memcmp(ref[kind], got, bytes) != 0) {
                    fprintf(stderr, "cores: %s at %d workers differs from 1 worker\n", kinds[kind], n);
                    return 1;
                }
                digest[kind][c] = fnv(got, bytes);
            }
            qsort(times[kind][c], (size_t)reps, sizeof(int64_t), cmp);
        }
        pool_close(&pool);
    }

    // A cancelled worker: worker 2 of 4 returns without running its chunks.
    Pool pool;
    if (pool_open(&pool, 4)) return 1;
    coax_t_PcPlan plan = pc_plan(shapes[SUM].items, shapes[SUM].chunk, 4);
    Job j = {.kind = SUM, .plan = plan, .x = xs, .part = {part, plan.chunks}, .done = {done, plan.chunks}, .cancel = 2};
    memset(done, 0, (size_t)plan.chunks);
    coax_t_PcStatus ran = pool_run(&pool, &j);
    coax_t_PcValue cancelled = pc_sum_merge(plan, j.part, j.done);
    pool_close(&pool);
    if (ran != COAX_PcStatus_ok || cancelled.status != COAX_PcStatus_incomplete) {
        fprintf(stderr, "cores: a cancelled worker was not refused\n");
        return 1;
    }

    char host[256] = "unknown", cpu[256], date[32], load[64] = "unknown";
    gethostname(host, sizeof host);
    cpu_name(cpu, sizeof cpu);
    struct utsname u;
    uname(&u);
    time_t now = time(NULL);
    strftime(date, sizeof date, "%Y-%m-%d", gmtime(&now));
    FILE *lf = fopen("/proc/loadavg", "r");
    if (lf) { if (fgets(load, sizeof load, lf)) load[strcspn(load, "\n")] = 0; fclose(lf); }
    printf("{\n  \"schema\": \"psionic.cores.v1\",\n  \"profile\": \"pc-cores-v1\",\n");
    printf("  \"host\": \"%s\",\n  \"cpu\": \"%s\",\n  \"hardware_threads\": %ld,\n  \"kernel\": \"%s %s %s\",\n  \"date\": \"%s\",\n",
           host, cpu, sysconf(_SC_NPROCESSORS_ONLN), u.sysname, u.release, u.machine, date);
    printf("  \"loadavg\": \"%s\",\n  \"compiler\": \"%s\",\n  \"flags\": \"%s\",\n  \"runs\": %d,\n", load, compiler, flags, reps);
    printf("  \"split_sum_bits\": [");
    for (int c = 0; c < COUNTS; c++) {
        float s = pc_sum_split(xs, counts[c]);
        uint32_t b;
        memcpy(&b, &s, sizeof b);
        printf("\"0x%08x\"%s", b, c + 1 < COUNTS ? ", " : "");
    }
    uint32_t tree;
    memcpy(&tree, ref[SUM], sizeof tree);
    printf("],\n  \"tree_sum_bits\": \"0x%08x\",\n  \"cancelled_worker\": \"%s\",\n  \"kernels\": [\n", tree,
           cancelled.status == COAX_PcStatus_incomplete ? "incomplete" : "published");
    for (int kind = 0; kind < KINDS; kind++) {
        printf("    {\"kernel\": \"%s\", \"shape\": \"%s\", \"chunks\": %lld, \"workers\": [\n", kinds[kind], shapes[kind].shape,
               (long long)((shapes[kind].items + shapes[kind].chunk - 1) / shapes[kind].chunk));
        for (int c = 0; c < COUNTS; c++) {
            int64_t *t = times[kind][c];
            printf("      {\"workers\": %d, \"digest\": \"%016llx\", \"best_ns\": %lld, \"median_ns\": %lld, \"worst_ns\": %lld, "
                   "\"speedup_milli\": %lld}%s\n",
                   counts[c], (unsigned long long)digest[kind][c], (long long)t[0], (long long)t[reps / 2], (long long)t[reps - 1],
                   (long long)pc_speedup(times[kind][0][0], t[0]), c + 1 < COUNTS ? "," : "");
        }
        printf("    ]}%s\n", kind + 1 < KINDS ? "," : "");
    }
    printf("  ]\n}\n");
    free(x); free(part); free(m); free(y); free(q); free(k); free(v); free(out); free(done);
    for (int kind = 0; kind < KINDS; kind++) free(ref[kind]);
    return 0;
}
