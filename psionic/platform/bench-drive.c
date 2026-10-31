/* Chapter 170's load generator: the host effects only. Coax
   (psionic/lib/bench-wire.coax and bench.coax) makes the schedule, writes each
   request body, says which received lines are tokens, reads the server's own
   time, and computes every percentile. This file connects to 127.0.0.1, sends
   at the scheduled times, reads to the end of each answer, and keeps a
   monotonic clock. Answers are measured and discarded: nothing a server sends
   is printed or stored, so a replay is a shadow.

   bench-drive responses|systemone PORT MODEL SEED REQUESTS GAP_MS JITTER_PCT
       [--label NAME] [--out FILE] [--record FILE] [--replay FILE [--scale PCT]]
   bench-drive compare BASE REPEAT CANDIDATE [MARGIN_PCT]
   --label names what the client cannot see (the server's build, backend and
   options); with the model, target, seed, request count, gap, jitter and
   replay scale it is the run's identity. --out keeps the run's identity and
   percentiles for compare, which asks bench.coax for each verdict.
   The bearer key, if any, is PSIONIC_API_KEY; it is never printed. */
#define _POSIX_C_SOURCE 200809L
#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>
#include "bench.h"

enum { MAX_REQ = 4096, MAX_TOK = 4096, BODY = 65536, RX = 1 << 20 };

typedef struct {
    coax_t_BnRequest r;
    int64_t index, code, sent, done, server, count, times[MAX_TOK];
    char body[BODY]; int64_t body_len;
} Job;

static struct timespec t0;
static int port;
static coax_t_BwTarget target;
static const char *key;

static int64_t now_us(void) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
    return (int64_t)(t.tv_sec - t0.tv_sec) * 1000000 + (t.tv_nsec - t0.tv_nsec) / 1000;
}
static void sleep_until(int64_t us) {
    for (;;) {
        int64_t left = us - now_us();
        if (left <= 0) return;
        struct timespec d = {left / 1000000, (left % 1000000) * 1000};
        nanosleep(&d, NULL);
    }
}
static coax_string str(const char *s, int64_t n) { return (coax_string){s, n}; }

static void *run(void *arg) {
    Job *j = arg;
    char *rx = malloc(RX);
    if (!rx) { j->code = 0; j->sent = j->done = now_us(); j->server = -1; j->count = 0; return NULL; }
    char head[1024];
    sleep_until(j->r.at_us);
    j->code = 0; j->server = -1; j->count = 0;
    j->sent = now_us();
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons((uint16_t)port)};
    a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    struct timeval tv = {180, 0}; int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof tv);
    setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
    if (fd < 0 || connect(fd, (struct sockaddr *)&a, sizeof a) != 0) { if (fd >= 0) close(fd); j->done = now_us(); free(rx); return NULL; }
    const char *path = target == 0 ? "/v1/responses" : "/v1/systemone";
    int hn = snprintf(head, sizeof head, "POST %s HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n%s%s%sContent-Length: %lld\r\nConnection: close\r\n\r\n",
        path, key ? "Authorization: Bearer " : "", key ? key : "", key ? "\r\n" : "", (long long)j->body_len);
    if (send(fd, head, (size_t)hn, MSG_NOSIGNAL) != hn || send(fd, j->body, (size_t)j->body_len, MSG_NOSIGNAL) != j->body_len) {
        close(fd); j->done = now_us(); free(rx); return NULL;
    }
    /* Read to the end; each complete line is time-stamped when its last byte arrives. */
    int64_t have = 0, line = 0, body = -1;
    for (;;) {
        ssize_t got = recv(fd, rx + have, (size_t)(RX - have), 0);
        if (got <= 0) break;
        int64_t t = now_us(), end = have + got;
        for (int64_t i = have; i < end; i++) {
            if (rx[i] != '\n') continue;
            int64_t n = i - line; if (n > 0 && rx[i - 1] == '\r') n--;
            if (line == 0 && n > 12) j->code = atoi(rx + 9);
            if (body < 0 && n == 0) body = i + 1;
            if (body >= 0 && target == 0 && bw_event(str(rx + line, n)) == 1 && j->count < MAX_TOK) j->times[j->count++] = t;
            line = i + 1;
        }
        have = end;
        if (have == RX) break;
    }
    j->done = now_us();
    close(fd);
    if (target == 1 && body >= 0) j->server = bw_latency_us(str(rx + body, have - body));
    if (j->server < -1 || j->server > j->done - j->sent) j->server = -1;
    free(rx); return NULL;
}

static int by_sent(const void *x, const void *y) {
    const Job *a = *(Job *const *)x, *b = *(Job *const *)y;
    return a->sent != b->sent ? (a->sent < b->sent ? -1 : 1) : (a->index < b->index ? -1 : 1);
}
static const char *NAMES[6] = {"time to first token", "inter-token latency", "end to end",
    "inference (server's latency_ms)", "end to end less inference", "per-user tokens/s"};
static uint64_t fnv(const char *s) {
    uint64_t h = 1469598103934665603ull;
    for (; *s; s++) h = (h ^ (uint8_t)*s) * 1099511628211ull;
    return h;
}
static int load(const char *path, coax_t_BnRun *run, coax_t_BnStats st[6]) {
    FILE *f = fopen(path, "r");
    if (!f) { perror(path); return 0; }
    int ok = 1;
    for (int i = 0; i < 8; i++) { unsigned long long v; ok &= fscanf(f, "%llu", &v) == 1; run->v.e[i] = v; }
    for (int k = 0; k < 6; k++) {
        long long v[5]; ok &= fscanf(f, "%lld %lld %lld %lld %lld", &v[0], &v[1], &v[2], &v[3], &v[4]) == 5;
        st[k].n = v[0]; for (int q = 0; q < 4; q++) st[k].p.e[q] = v[q + 1];
    }
    fclose(f);
    if (!ok) fprintf(stderr, "bench-drive: %s is not a run file\n", path);
    return ok;
}
static int compare(int argc, char **argv) {
    coax_t_BnRun run[3]; coax_t_BnStats st[3][6];
    for (int i = 0; i < 3; i++) if (!load(argv[2 + i], &run[i], st[i])) return 1;
    int64_t margin = argc > 5 ? atoll(argv[5]) : 2;
    const char *verdicts[7] = {"none", "refused", "repeat", "better", "worse", "within", "unmeasured"};
    const char *pct[4] = {"P50", "P90", "P95", "P99"};
    for (int k = 0; k < 6; k++) {
        printf("  %-34s", NAMES[k]);
        for (int q = 0; q < 4; q++) {
            coax_t_BnCall c = bn_compare(run[0], run[1], run[2], st[0][k], st[1][k], st[2][k], q, k != 5, margin);
            printf(" %s %-10s", pct[q], verdicts[c.verdict]);
            if (k == 0 && q == 0) fprintf(stderr, "bench-drive: %lld of 8 run variables changed\n", (long long)c.changed);
        }
        printf("\n");
    }
    return 0;
}
static void line(const char *name, coax_t_BnStats s, double scale, const char *unit) {
    if (s.n == 0) { printf("  %-34s n=0\n", name); return; }
    printf("  %-34s n=%-4lld P50 %9.1f  P90 %9.1f  P95 %9.1f  P99 %9.1f %s\n", name, (long long)s.n,
        s.p.e[0] * scale, s.p.e[1] * scale, s.p.e[2] * scale, s.p.e[3] * scale, unit);
}

int main(int argc, char **argv) {
    if (argc >= 5 && !strcmp(argv[1], "compare")) return compare(argc, argv);
    if (argc < 8) {
        fprintf(stderr, "usage: bench-drive responses|systemone PORT MODEL SEED REQUESTS GAP_MS JITTER_PCT [--record FILE] [--replay FILE [--scale PCT]]\n");
        return 2;
    }
    target = strcmp(argv[1], "systemone") == 0 ? 1 : 0;
    port = atoi(argv[2]);
    const char *model = argv[3];
    uint64_t seed = strtoull(argv[4], NULL, 10);
    int64_t requests = atoll(argv[5]), gap_ms = atoll(argv[6]), jitter = atoll(argv[7]), scale = 100;
    const char *record = NULL, *replay = NULL, *label = "", *out = NULL;
    for (int i = 8; i + 1 < argc; i += 2) {
        if (!strcmp(argv[i], "--record")) record = argv[i + 1];
        else if (!strcmp(argv[i], "--replay")) replay = argv[i + 1];
        else if (!strcmp(argv[i], "--scale")) scale = atoll(argv[i + 1]);
        else if (!strcmp(argv[i], "--label")) label = argv[i + 1];
        else if (!strcmp(argv[i], "--out")) out = argv[i + 1];
        else { fprintf(stderr, "bench-drive: unknown option %s\n", argv[i]); return 2; }
    }
    key = getenv("PSIONIC_API_KEY");
    if (key && !*key) key = NULL;
    static coax_t_BnRequest plan[MAX_REQ], rec[MAX_REQ];
    coax_t_BnStatus st;
    if (replay) {
        FILE *f = fopen(replay, "r"); int64_t n = 0;
        if (!f) { perror(replay); return 1; }
        long long v[5];
        while (n < MAX_REQ && fscanf(f, "%lld %lld %lld %lld %lld", &v[0], &v[1], &v[2], &v[3], &v[4]) == 5)
            rec[n++] = (coax_t_BnRequest){v[0], v[1], v[2], v[3], v[4]};
        fclose(f);
        st = bn_replay((coax_v_t_BnRequest){rec, n}, scale, (coax_v_t_BnRequest){plan, MAX_REQ});
        requests = n;
    } else {
        coax_t_BnWorkload w = bw_workload(target, seed, requests, gap_ms * 1000, jitter);
        st = bn_schedule(w, (coax_v_t_BnRequest){plan, MAX_REQ});
    }
    if (st != 0) { fprintf(stderr, "bench-drive: the schedule was refused (status %d)\n", st); return 1; }
    if (record) {
        FILE *f = fopen(record, "w");
        if (!f) { perror(record); return 1; }
        for (int64_t i = 0; i < requests; i++)
            fprintf(f, "%lld %lld %lld %lld %lld\n", (long long)plan[i].at_us, (long long)plan[i].input, (long long)plan[i].output, (long long)plan[i].content, (long long)plan[i].sampling);
        fclose(f);
    }
    Job *jobs = calloc((size_t)requests, sizeof(Job));
    pthread_t *th = calloc((size_t)requests, sizeof(pthread_t));
    if (!jobs || !th) return 1;
    for (int64_t i = 0; i < requests; i++) {
        jobs[i].r = plan[i]; jobs[i].index = i;
        jobs[i].body_len = bw_body((coax_v_u8){(uint8_t *)jobs[i].body, BODY}, target, str(model, (int64_t)strlen(model)), plan[i], i, seed);
        if (jobs[i].body_len < 0) { fprintf(stderr, "bench-drive: request %lld has no body\n", (long long)i); return 1; }
    }
    clock_gettime(CLOCK_MONOTONIC, &t0);
    pthread_attr_t attr; pthread_attr_init(&attr);
    for (int64_t i = 0; i < requests; i++) pthread_create(&th[i], &attr, run, &jobs[i]);
    for (int64_t i = 0; i < requests; i++) pthread_join(th[i], NULL);
    /* Coax wants the answers in send order and their token times end to end. */
    Job **order = calloc((size_t)requests, sizeof(Job *));
    coax_t_BnObs *obs = calloc((size_t)requests, sizeof(coax_t_BnObs));
    int64_t total = 0, end = 0;
    for (int64_t i = 0; i < requests; i++) { order[i] = &jobs[i]; total += jobs[i].count; if (jobs[i].done > end) end = jobs[i].done; }
    qsort(order, (size_t)requests, sizeof(Job *), by_sent);
    int64_t *times = calloc((size_t)total + 1, sizeof(int64_t)), cap = total > requests ? total : requests;
    int64_t *values = calloc((size_t)cap + 1, sizeof(int64_t)), at = 0;
    for (int64_t i = 0; i < requests; i++) {
        Job *j = order[i];
        obs[i] = (coax_t_BnObs){j->code, j->sent, j->done, j->server, at, j->count};
        memcpy(times + at, j->times, (size_t)j->count * sizeof(int64_t)); at += j->count;
    }
    coax_t_BnSummary m = bn_summarize((coax_v_t_BnObs){obs, requests}, (coax_v_s64){times, total}, end, (coax_v_s64){values, cap});
    if (m.status != 0) { fprintf(stderr, "bench-drive: the timings were refused (status %d)\n", m.status); return 1; }
    printf("%s, %lld requests%s: %lld answered, %lld refused busy, %lld failed; %lld token events in %.3f s, %.2f tokens/s in total\n",
        argv[1], (long long)requests, replay ? " (shadow replay)" : "", (long long)m.ok, (long long)m.refused, (long long)m.failed,
        (long long)m.tokens, m.span_us / 1e6, m.total_milli / 1000.0);
    line(NAMES[0], m.ttft, 1e-3, "ms");
    line(NAMES[1], m.itl, 1e-3, "ms");
    line(NAMES[2], m.e2e, 1e-3, "ms");
    line(NAMES[3], m.infer, 1e-3, "ms");
    line(NAMES[4], m.overhead, 1e-3, "ms");
    line(NAMES[5], m.user_milli, 1e-3, "tok/s");
    if (out) {
        FILE *f = fopen(out, "w");
        if (!f) { perror(out); return 1; }
        uint64_t id[8] = {fnv(label), fnv(model), (uint64_t)target, replay ? fnv(replay) : seed, (uint64_t)requests, (uint64_t)gap_ms, (uint64_t)jitter, (uint64_t)scale};
        for (int i = 0; i < 8; i++) fprintf(f, "%llu%s", (unsigned long long)id[i], i < 7 ? " " : "\n");
        coax_t_BnStats all[6] = {m.ttft, m.itl, m.e2e, m.infer, m.overhead, m.user_milli};
        for (int k = 0; k < 6; k++) fprintf(f, "%lld %lld %lld %lld %lld\n", (long long)all[k].n,
            (long long)all[k].p.e[0], (long long)all[k].p.e[1], (long long)all[k].p.e[2], (long long)all[k].p.e[3]);
        fclose(f);
    }
    free(values); free(times); free(obs); free(order); free(th); free(jobs);
    return 0;
}
