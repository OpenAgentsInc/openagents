// Chapter 121: feed a consumer from a threaded loader. The host owns the
// effects only: a loader thread, a pool of worker threads, a lock and two
// condition variables around the ring, the buffers and the monotonic clock.
// Every decision is Coax, from psionic/lib/data/feed.coax: the plan
// (fd_plan), the row cursors (fd_cursors), each worker's rows (fd_worker),
// the publish check (fd_publish), which slot to fill or read (fd_free,
// fd_ready), the commits (fd_produce, fd_consume) and the stall model
// (fd_overlap). The consumer stands in for an accelerator: it copies each
// batch out and then runs a fixed single-threaded matrix-vector product
// (pc_matvec_worker) as its compute. There is no GPU here.
// The harness exits 1 if any worker count or depth gives other bytes than
// the single-threaded pk_next stream, or if a resume from the consumer's
// cursor does. It prints one JSON receipt on stdout.
// Build and run with psionic/tests/feed.sh, which records the receipt.
#define _POSIX_C_SOURCE 200809L
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <sys/utsname.h>
#include "feed.h"

enum { MAX_STEPS = 64, REPS = 3, COUNTS = 4, DEPTHS = 3 };
static const int counts[COUNTS] = {1, 2, 4, 8};
static const int depths[DEPTHS] = {1, 2, 4};

static int64_t now_ns(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (int64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}

// The dataset and the shape every run shares.
typedef struct {
    coax_v_t_PkDoc docs;
    coax_v_u8 tokens;
    int64_t rows, length, steps, mat;   // mat: the consumer's square matrix size
    float *m, *x, *y;
} World;

// Worker threads wait on a barrier; the loader hands them one batch at a time.
typedef struct Pool Pool;
typedef struct {
    pthread_t thread;
    Pool *pool;
    int index;
    coax_t_FdStatus status;
} Worker;
struct Pool {
    int n, quit;
    pthread_barrier_t start, end;
    const World *world;
    coax_t_FdPlan plan;
    coax_v_t_PkCursor cursors;
    coax_v_u8 batch, done;
    Worker w[8];
};
static void *worker_main(void *arg) {
    Worker *w = arg;
    Pool *p = w->pool;
    for (;;) {
        pthread_barrier_wait(&p->start);
        if (p->quit) return NULL;
        w->status = fd_worker(p->world->docs, p->world->tokens, p->plan, w->index, p->cursors, p->batch, p->done);
        pthread_barrier_wait(&p->end);
    }
}
static int pool_open(Pool *p, int n) {
    p->n = n;
    p->quit = 0;
    if (pthread_barrier_init(&p->start, NULL, (unsigned)n + 1) || pthread_barrier_init(&p->end, NULL, (unsigned)n + 1)) return 1;
    for (int i = 0; i < n; i++) {
        p->w[i] = (Worker){.pool = p, .index = i};
        if (pthread_create(&p->w[i].thread, NULL, worker_main, &p->w[i])) return 1;
    }
    return 0;
}
static coax_t_FdStatus pool_run(Pool *p) {
    pthread_barrier_wait(&p->start);
    pthread_barrier_wait(&p->end);
    for (int i = 0; i < p->n; i++) if (p->w[i].status != COAX_FdStatus_ok) return p->w[i].status;
    return COAX_FdStatus_ok;
}
static void pool_close(Pool *p) {
    p->quit = 1;
    pthread_barrier_wait(&p->start);
    for (int i = 0; i < p->n; i++) pthread_join(p->w[i].thread, NULL);
    pthread_barrier_destroy(&p->start);
    pthread_barrier_destroy(&p->end);
}

// One pipeline: the ring, its slots and what each side measured.
typedef struct {
    const World *world;
    coax_t_FdPlan plan;
    pthread_mutex_t lock;
    pthread_cond_t filled, freed;
    coax_t_FdRing ring;
    coax_t_PkCursor next;   // the loader's cursor: where its next batch starts
    int stop, failed;
    uint8_t *slots;
    coax_t_PkCursor cursors[65];
    uint8_t done[64];
    Pool pool;
    int64_t load_ns[MAX_STEPS + 8], compute_ns[MAX_STEPS];
    int64_t loads;
} Pipe;

static void *loader_main(void *arg) {
    Pipe *q = arg;
    const World *w = q->world;
    int64_t size = q->plan.rows * q->plan.length;
    for (;;) {
        pthread_mutex_lock(&q->lock);
        while (!q->stop && fd_free(q->ring) < 0) pthread_cond_wait(&q->freed, &q->lock);
        int64_t slot = fd_free(q->ring);
        int stop = q->stop;
        pthread_mutex_unlock(&q->lock);
        if (stop) return NULL;
        int64_t t = now_ns();
        coax_t_FdNext n = fd_cursors(w->docs, w->tokens, q->plan, q->next, (coax_v_t_PkCursor){q->cursors, q->plan.rows + 1});
        coax_t_FdStatus s = n.status;
        if (s == COAX_FdStatus_ok) {
            memset(q->done, 0, (size_t)q->plan.rows);
            q->pool.batch = (coax_v_u8){q->slots + slot * size, size};
            s = pool_run(&q->pool);
            if (s == COAX_FdStatus_ok) s = fd_publish(q->plan, q->pool.done);
        }
        int64_t spent = now_ns() - t;
        pthread_mutex_lock(&q->lock);
        if (s != COAX_FdStatus_ok) {
            q->failed = 1 + (int)s;
            pthread_cond_broadcast(&q->filled);
            pthread_mutex_unlock(&q->lock);
            return NULL;
        }
        q->ring = fd_produce(q->ring, n.cursor).ring;
        q->next = n.cursor;
        if (q->loads < MAX_STEPS + 8) q->load_ns[q->loads] = spent;
        q->loads++;
        pthread_cond_signal(&q->filled);
        pthread_mutex_unlock(&q->lock);
    }
}

// The accelerator's stand-in: take the batch into its own buffer, then run
// a fixed amount of arithmetic on one thread.
static void consume(const World *w, const uint8_t *batch, uint8_t *out, int64_t size) {
    memcpy(out, batch, (size_t)size);
    coax_t_PcPlan one = pc_plan(w->mat, w->mat, 1);
    uint8_t done[1];
    pc_matvec_worker(one, 0, (coax_v_f32){w->m, w->mat * w->mat}, (coax_v_f32){w->x, w->mat}, (coax_v_f32){w->y, w->mat}, (coax_v_u8){done, 1});
}

typedef struct {
    int ok;
    int64_t total_ns, idle_ns, load_mean, compute_mean, predicted_ns;
    coax_t_PkCursor producer, consumer;
    int64_t produced, consumed;
} Run;

// Takes steps batches from start into out; out must hold steps batches.
static Run run_pipe(const World *w, int workers, int depth, coax_t_PkCursor start, int64_t steps, uint8_t *out) {
    Run r = {0};
    Pipe *q = calloc(1, sizeof *q);
    if (!q) return r;
    q->world = w;
    q->plan = fd_plan(w->rows, w->length, workers, depth, 131072);
    if (q->plan.status != COAX_FdStatus_ok) { free(q); return r; }
    q->ring = fd_ring(q->plan, start).ring;
    q->next = start;
    int64_t size = w->rows * w->length;
    q->slots = malloc((size_t)q->plan.bytes);
    pthread_mutex_init(&q->lock, NULL);
    pthread_cond_init(&q->filled, NULL);
    pthread_cond_init(&q->freed, NULL);
    q->pool.world = w;
    q->pool.plan = q->plan;
    q->pool.cursors = (coax_v_t_PkCursor){q->cursors, w->rows + 1};
    q->pool.done = (coax_v_u8){q->done, w->rows};
    if (!q->slots || pool_open(&q->pool, workers)) { free(q->slots); free(q); return r; }
    pthread_t loader;
    int64_t t0 = now_ns(), idle = 0;
    pthread_create(&loader, NULL, loader_main, q);
    int64_t s;
    for (s = 0; s < steps; s++) {
        int64_t wait = now_ns();
        pthread_mutex_lock(&q->lock);
        while (!q->failed && fd_ready(q->ring) < 0) pthread_cond_wait(&q->filled, &q->lock);
        int64_t slot = fd_ready(q->ring);
        int failed = q->failed;
        pthread_mutex_unlock(&q->lock);
        idle += now_ns() - wait;
        if (failed) break;
        int64_t c = now_ns();
        consume(w, q->slots + slot * size, out + s * size, size);
        q->compute_ns[s] = now_ns() - c;
        pthread_mutex_lock(&q->lock);
        q->ring = fd_consume(q->ring).ring;
        pthread_cond_signal(&q->freed);
        pthread_mutex_unlock(&q->lock);
    }
    r.total_ns = now_ns() - t0;
    pthread_mutex_lock(&q->lock);
    q->stop = 1;
    pthread_cond_broadcast(&q->freed);
    pthread_mutex_unlock(&q->lock);
    pthread_join(loader, NULL);
    pool_close(&q->pool);
    r.ok = s == steps && !q->failed;
    r.idle_ns = idle;
    r.producer = q->next;
    r.consumer = q->ring.resume;
    r.produced = q->ring.produced;
    r.consumed = q->ring.consumed;
    if (r.ok && steps > 0) {
        int64_t lsum = 0, csum = 0;
        for (int64_t i = 0; i < steps; i++) { lsum += q->load_ns[i]; csum += q->compute_ns[i]; }
        r.load_mean = lsum / steps;
        r.compute_mean = csum / steps;
        coax_t_FdTime model = fd_overlap((coax_v_s64){q->compute_ns, steps}, (coax_v_s64){q->load_ns, steps}, depth);
        r.predicted_ns = model.status == COAX_FdStatus_ok ? model.total : -1;
    }
    pthread_mutex_destroy(&q->lock);
    pthread_cond_destroy(&q->filled);
    pthread_cond_destroy(&q->freed);
    free(q->slots);
    free(q);
    return r;
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
static uint64_t fnv(const void *data, size_t n) {
    const unsigned char *b = data;
    uint64_t h = 14695981039346656037ull;
    for (size_t i = 0; i < n; i++) { h ^= b[i]; h *= 1099511628211ull; }
    return h;
}

int main(int argc, char **argv) {
    const char *compiler = argc > 1 ? argv[1] : "unknown";
    const char *flags = argc > 2 ? argv[2] : "unknown";
    int small = argc > 3 && strcmp(argv[3], "small") == 0;
    int reps = small ? 1 : REPS;
    static coax_t_PkDoc docs[64];
    static uint8_t tokens[4096];
    int64_t n = fd_synthetic((coax_v_t_PkDoc){docs, 64}, (coax_v_u8){tokens, 4096});
    World w = {.docs = {docs, 64}, .tokens = {tokens, n}, .rows = small ? 8 : 64, .length = small ? 32 : 256,
               .steps = small ? 12 : MAX_STEPS, .mat = small ? 32 : 768};
    w.m = malloc(sizeof(float) * (size_t)(w.mat * w.mat));
    w.x = malloc(sizeof(float) * (size_t)w.mat);
    w.y = malloc(sizeof(float) * (size_t)w.mat);
    int64_t size = w.rows * w.length, bytes = size * w.steps;
    uint8_t *ref = malloc((size_t)bytes), *out = malloc((size_t)bytes);
    if (!w.m || !w.x || !w.y || !ref || !out) { fprintf(stderr, "feed: allocation failed\n"); return 1; }
    for (int64_t i = 0; i < w.mat * w.mat; i++) w.m[i] = (float)((i * 7) % 13) / 13.0f;
    for (int64_t i = 0; i < w.mat; i++) w.x[i] = (float)(i % 5) / 5.0f;

    // The reference: every row straight from pk_next on this thread.
    coax_t_PkCursor start = {.seed = 121}, c = start;
    int64_t t = now_ns();
    for (int64_t r = 0; r < w.steps * w.rows; r++) {
        coax_t_PkNext next = pk_next(w.docs, w.tokens, c, (coax_v_u8){ref + r * w.length, w.length}, 0);
        if (next.status != COAX_PkStatus_ok) { fprintf(stderr, "feed: reference refused\n"); return 1; }
        c = next.cursor;
    }
    int64_t reference_ns = now_ns() - t;

    Run best[COUNTS][DEPTHS];
    for (int i = 0; i < COUNTS; i++) {
        for (int d = 0; d < DEPTHS; d++) {
            for (int rep = -1; rep < reps; rep++) {   // run -1 warms up
                memset(out, 0, (size_t)bytes);
                Run r = run_pipe(&w, counts[i], depths[d], start, w.steps, out);
                if (!r.ok) { fprintf(stderr, "feed: %d workers, depth %d refused\n", counts[i], depths[d]); return 1; }
                if (memcmp(out, ref, (size_t)bytes) != 0) {
                    fprintf(stderr, "feed: %d workers, depth %d differ from the reference\n", counts[i], depths[d]);
                    return 1;
                }
                if (rep == 0 || (rep > 0 && r.total_ns < best[i][d].total_ns)) best[i][d] = r;
            }
        }
    }

    // Stop after a third of the batches, keep the consumer's cursor only,
    // and run the rest in a fresh pipeline.
    int64_t k = w.steps / 3;
    memset(out, 0, (size_t)bytes);
    Run first = run_pipe(&w, 4, 4, start, k, out);
    Run rest = run_pipe(&w, 4, 4, first.consumer, w.steps - k, out + k * size);
    if (!first.ok || !rest.ok || memcmp(out, ref, (size_t)bytes) != 0) { fprintf(stderr, "feed: the resumed stream differs\n"); return 1; }
    // The same from the producer's cursor skips whatever it had prefetched.
    Run skip = run_pipe(&w, 4, 4, first.producer, w.steps - k, out + k * size);
    int64_t differ = -1;
    for (int64_t i = 0; skip.ok && i < bytes; i++) if (out[i] != ref[i]) { differ = i; break; }

    char host[256] = "unknown", cpu[256], date[32], load[64] = "unknown";
    gethostname(host, sizeof host);
    cpu_name(cpu, sizeof cpu);
    struct utsname u;
    uname(&u);
    time_t now = time(NULL);
    strftime(date, sizeof date, "%Y-%m-%d", gmtime(&now));
    FILE *lf = fopen("/proc/loadavg", "r");
    if (lf) { if (fgets(load, sizeof load, lf)) load[strcspn(load, "\n")] = 0; fclose(lf); }
    printf("{\n  \"schema\": \"psionic.feed.v1\",\n  \"profile\": \"psionic-feed-v1\",\n  \"device\": \"cpu stand-in: one thread, %lld x %lld f32 matrix-vector product per batch; no GPU\",\n",
           (long long)w.mat, (long long)w.mat);
    printf("  \"host\": \"%s\",\n  \"cpu\": \"%s\",\n  \"hardware_threads\": %ld,\n  \"kernel\": \"%s %s %s\",\n  \"date\": \"%s\",\n",
           host, cpu, sysconf(_SC_NPROCESSORS_ONLN), u.sysname, u.release, u.machine, date);
    printf("  \"loadavg\": \"%s\",\n  \"compiler\": \"%s\",\n  \"flags\": \"%s\",\n  \"runs\": %d,\n", load, compiler, flags, reps);
    printf("  \"dataset\": {\"documents\": 64, \"training\": 48, \"tokens\": %lld},\n", (long long)n);
    printf("  \"batch\": {\"rows\": %lld, \"length\": %lld, \"bytes\": %lld, \"steps\": %lld},\n", (long long)w.rows, (long long)w.length,
           (long long)size, (long long)w.steps);
    printf("  \"stream_digest\": \"%016llx\",\n  \"reference_ns\": %lld,\n", (unsigned long long)fnv(ref, (size_t)bytes), (long long)reference_ns);
    printf("  \"resume\": {\"stopped_after\": %lld, \"produced\": %lld, \"consumer_resume\": \"equal\", \"producer_resume_first_difference\": %lld},\n",
           (long long)k, (long long)first.produced, (long long)differ);
    printf("  \"runs_by_config\": [\n");
    for (int i = 0; i < COUNTS; i++) {
        for (int d = 0; d < DEPTHS; d++) {
            Run *r = &best[i][d];
            coax_t_FdTime measured = {.total = r->total_ns, .idle = r->idle_ns};
            printf("    {\"workers\": %d, \"depth\": %d, \"ring_bytes\": %lld, \"stream\": \"equal\", \"load_mean_ns\": %lld, \"compute_mean_ns\": %lld, "
                   "\"total_ns\": %lld, \"idle_ns\": %lld, \"idle_per_mille\": %lld, \"model_total_ns\": %lld, \"steady_step_ns\": %lld}%s\n",
                   counts[i], depths[d], (long long)(depths[d] * size), (long long)r->load_mean, (long long)r->compute_mean,
                   (long long)r->total_ns, (long long)r->idle_ns, (long long)fd_lost(measured), (long long)r->predicted_ns,
                   (long long)fd_steady(r->compute_mean, r->load_mean, depths[d]), i + 1 < COUNTS || d + 1 < DEPTHS ? "," : "");
        }
    }
    printf("  ]\n}\n");
    free(w.m); free(w.x); free(w.y); free(ref); free(out);
    return 0;
}
