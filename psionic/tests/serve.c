/* Chapter 20 native checks: a real psionic-serve process over loopback.
 *
 *   serve-test PATH_TO_PSIONIC_SERVE LOG_DIRECTORY
 *
 * Starts three toy servers, sends well-formed, malformed, oversized, slow,
 * partial and abandoned requests over TCP, and checks each status, code and
 * body. The answer's probabilities are compared with direct calls to the
 * chapter-14 C adapters (psionic_prepare, psionic_answer) on a separately
 * opened toy session, with typed inputs built here rather than decoded. */
#define _GNU_SOURCE
#include "serve.h"
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

coax_t_PrResult psionic_prepare(coax_v_u8,coax_t_PtEntry,coax_t_PtQuestion,coax_v_t_PtOption,coax_v_s64,coax_t_PrWorkspace);
coax_t_PsAnswer psionic_answer(coax_t_PsSession,coax_t_PrResult,coax_v_s64,coax_v_t_PtOption,float,coax_v_f32,coax_v_f64,coax_v_f64,coax_v_f32);

static int failures;
#define CHECK(c, ...) do { if (!(c)) { failures++; fprintf(stderr, "FAIL %s:%d: ", __FILE__, __LINE__); fprintf(stderr, __VA_ARGS__); fputc('\n', stderr); } } while (0)

static long long now_ms(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (long long)t.tv_sec * 1000 + t.tv_nsec / 1000000; }
static void pause_ms(int ms) { struct timespec t = {ms / 1000, (ms % 1000) * 1000000L}; while (nanosleep(&t, &t) && errno == EINTR) {} }

typedef struct { pid_t pid; int port; char log[512]; } Server;

static Server start(const char *binary, const char *logdir, const char *name, const char *const *extra) {
    Server s = {0}; int out[2]; if (pipe(out)) exit(2);
    snprintf(s.log, sizeof s.log, "%s/%s.log", logdir, name);
    s.pid = fork();
    if (s.pid == 0) {
        const char *argv[16] = {binary, "--toy", "--port", "0"}; int n = 4;
        for (int i = 0; extra[i]; i++) argv[n++] = extra[i];
        argv[n] = NULL;
        int log = open(s.log, O_WRONLY | O_CREAT | O_TRUNC, 0600);
        dup2(out[1], 1); dup2(log, 2); close(out[0]);
        execv(binary, (char *const *)argv); _exit(127);
    }
    close(out[1]);
    char line[512] = {0}; size_t got = 0;
    while (got < sizeof line - 1) { ssize_t r = read(out[0], line + got, 1); if (r <= 0 || line[got] == '\n') break; got++; }
    close(out[0]);
    char *colon = strstr(line, "127.0.0.1:");
    if (!colon) { fprintf(stderr, "server %s did not start: %s\n", name, line); exit(1); }
    s.port = atoi(colon + 10);
    return s;
}
static int stop(Server s) { kill(s.pid, SIGTERM); int status = 0; waitpid(s.pid, &status, 0); return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status); }

static int dial(int port) {
    int fd = socket(AF_INET, SOCK_STREAM, 0); struct sockaddr_in a = {0};
    a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port); a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (connect(fd, (struct sockaddr *)&a, sizeof a)) { perror("connect"); exit(1); }
    return fd;
}
static void send_all(int fd, const char *p, size_t n) { while (n) { ssize_t w = send(fd, p, n, MSG_NOSIGNAL); if (w <= 0) return; p += w; n -= (size_t)w; } }

typedef struct { int status; char *data; size_t n; const char *body; long long ms; } Reply;
static Reply receive(int fd, int timeout_ms) {
    Reply r = {0}; size_t cap = 1 << 16; r.data = calloc(cap + 1, 1); long long t0 = now_ms();
    for (;;) {
        struct pollfd p = {fd, POLLIN, 0}; int left = timeout_ms - (int)(now_ms() - t0);
        if (left <= 0 || poll(&p, 1, left) <= 0) break;
        if (r.n == cap) { cap *= 2; r.data = realloc(r.data, cap + 1); }
        ssize_t k = read(fd, r.data + r.n, cap - r.n); if (k <= 0) break; r.n += (size_t)k;
    }
    r.data[r.n] = 0; r.ms = now_ms() - t0; close(fd);
    if (r.n > 12 && !strncmp(r.data, "HTTP/1.1 ", 9)) r.status = atoi(r.data + 9);
    char *b = strstr(r.data, "\r\n\r\n"); r.body = b ? b + 4 : "";
    return r;
}
static Reply ask(int port, const char *bytes, size_t n) { int fd = dial(port); send_all(fd, bytes, n); return receive(fd, 120000); }
static char *post(const char *body) {
    size_t cap=strlen(body)+128;char *s = malloc(cap); snprintf(s,cap, "POST /v1/systemone HTTP/1.1\r\nHost: localhost\r\nContent-Length: %zu\r\n\r\n%s", strlen(body), body); return s;
}
static Reply ask_body(int port, const char *body) { char *m = post(body); Reply r = ask(port, m, strlen(m)); free(m); return r; }
static bool has_code(Reply r, const char *code) { char want[96]; snprintf(want, sizeof want, "{\"error\":{\"code\":\"%s\"", code); return !strncmp(r.body, want, strlen(want)); }
static void expect_refusal(Reply r, int status, const char *code, const char *what) {
    CHECK(r.status == status && has_code(r, code), "%s: wanted %d %s, got %d %s", what, status, code, r.status, r.body);
    free(r.data);
}

#define STATE "MARKER-STATE-7f3a: the kettle is boiling."
#define BODY "{\"model\":\"psionic-toy\",\"state\":\"" STATE "\",\"questions\":{\"MARKER-Q-hot\":{\"type\":\"noul\",\"instructions\":\"Is the water hot?\"},\"MARKER-Q-what\":{\"type\":\"choice\",\"criteria\":{\"MARKER-OPT-tea\":null,\"MARKER-OPT-soup\":\"a meal\"}},\"MARKER-Q-level\":{\"type\":\"score\",\"criteria\":[\"cold\",\"warm\",\"hot\"]}}}"
#define IDENTITY "\"identity\":\"psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573\""

static bool starts(const char *s, const char *prefix) { return !strncmp(s, prefix, strlen(prefix)); }
static double number_after(const char *body, const char *key) {
    if (!body) return NAN;
    const char *at = strstr(body, key); if (!at) return NAN;
    return strtod(at + strlen(key), NULL);
}

/* The same three questions through the chapter-14 adapters, without HTTP or decoding. */
static void direct(double hot[2], double what[2], double level[3]) {
    coax_t_SvToySizes z = sv_toy_sizes();
    coax_t_SvToyFiles f = {{calloc(z.model, 1), z.model}, {calloc(z.config, 1), z.config}, {calloc(z.tokenizer, 1), z.tokenizer}, {calloc(z.table, 1), z.table}, {calloc(z.tensors, sizeof(coax_t_P5Spec)), z.tensors}};
    if (!sv_toy_copy(f)) exit(1);
    coax_t_PsWorkspace m = {{calloc(z.tensors, sizeof(coax_t_P5Tensor)), z.tensors}, {calloc(z.tensors, sizeof(coax_t_P5Tensor)), z.tensors}, {calloc(z.layers, sizeof(coax_t_QeLayer)), z.layers},
        {calloc(z.params, 4), z.params}, {calloc(z.params, 4), z.params}, {calloc(z.layer_work, 4), z.layer_work}, {calloc(z.norm, 4), z.norm}, {calloc(z.norm, 4), z.norm}};
    coax_t_SvEngine e = sv_toy_open(f, m, 2048); if (!e.ok) exit(1);
    coax_t_SvPlan plan = sv_plan(e);
    coax_t_PrWorkspace w = {{calloc(plan.prompt, 1), plan.prompt}, {calloc(plan.render, 1), plan.render}, {calloc(plan.candidate, 8), plan.candidate}, {calloc(plan.full, 8), plan.full},
        {calloc(plan.stage, 8), plan.stage}, {calloc(plan.normal, 1), plan.normal}, {calloc(plan.scalars, 8), plan.scalars}, {calloc(plan.symbols, 8), plan.symbols}};
    coax_v_s64 tokens = {calloc(plan.tokens, 8), plan.tokens}; coax_v_f32 state = {calloc(plan.state, 4), plan.state}, work = {calloc(plan.work, 4), plan.work};
    double prob[20];
    coax_t_PtEntry st = {COAX_PtEntryKind_text, {STATE, (int64_t)strlen(STATE)}}, none = {COAX_PtEntryKind_none, {"", 0}};
    coax_t_PtOption noul[2] = {{{"yes", 3}, none}, {{"no", 2}, none}};
    coax_t_PtOption choice[2] = {{{"MARKER-OPT-tea", 14}, none}, {{"MARKER-OPT-soup", 15}, {COAX_PtEntryKind_text, {"a meal", 6}}}};
    coax_t_PtOption score[3] = {{{"", 0}, {COAX_PtEntryKind_text, {"cold", 4}}}, {{"", 0}, {COAX_PtEntryKind_text, {"warm", 4}}}, {{"", 0}, {COAX_PtEntryKind_text, {"hot", 3}}}};
    coax_t_PtQuestion q[3] = {{COAX_PtQuestionKind_noul, {COAX_PtEntryKind_text, {"Is the water hot?", 17}}}, {COAX_PtQuestionKind_choice, none}, {COAX_PtQuestionKind_score, none}};
    coax_t_PtOption *opts[3] = {noul, choice, score}; int64_t counts[3] = {0, 2, 3}; double *outs[3] = {hot, what, level};
    for (int i = 0; i < 3; i++) {
        coax_t_PrResult p = psionic_prepare(e.session.table, st, q[i], (coax_v_t_PtOption){opts[i], counts[i]}, tokens, w);
        CHECK(p.status == COAX_PrStatus_ok, "direct prepare %d status %u", i, p.status);
        coax_t_PsAnswer a = psionic_answer(e.session, p, (coax_v_s64){tokens.data, p.written}, (coax_v_t_PtOption){opts[i], counts[i]}, 1, state, (coax_v_f64){outs[i], i == 2 ? 3 : 2}, (coax_v_f64){prob, 20}, work);
        CHECK(a.status == COAX_QxStatus_ok, "direct answer %d status %u", i, a.status);
    }
}

static char *repeat_questions(int n) {
    size_t cap=64*(size_t)n+128;char *b = malloc(cap); int at = snprintf(b,cap, "{\"model\":\"psionic-toy\",\"state\":\"s\",\"questions\":{");
    for (int i = 0; i < n; i++) at += snprintf(b + at,cap-(size_t)at, "%s\"q%d\":{\"type\":\"noul\"}", i ? "," : "", i);
    snprintf(b + at,cap-(size_t)at, "}}"); return b;
}
static char *many_options(int n) {
    size_t cap=32*(size_t)n+128;char *b = malloc(cap); int at = snprintf(b,cap, "{\"model\":\"psionic-toy\",\"state\":\"s\",\"questions\":{\"q\":{\"type\":\"choice\",\"criteria\":{");
    for (int i = 0; i < n; i++) at += snprintf(b + at,cap-(size_t)at, "%s\"o%d\":null", i ? "," : "", i);
    snprintf(b + at,cap-(size_t)at, "}}}}"); return b;
}

static void answers(Server a) {
    Reply r = ask_body(a.port, BODY);
    CHECK(r.status == 200, "answer status %d %s", r.status, r.data);
    CHECK(strstr(r.body, IDENTITY) && starts(r.body, "{\"model\":\"psionic-toy\",\"answers\":{\"MARKER-Q-hot\":{\"type\":\"noul\",\"noul\":"), "answer shape %s", r.body);
    char want[64]; snprintf(want, sizeof want, "Content-Length: %zu\r\n", strlen(r.body)); CHECK(strstr(r.data, want) != NULL, "content length");
    double hot[2], what[2], level[3]; direct(hot, what, level);
    double served[7] = {number_after(r.body, "\"noul\":"), number_after(r.body, "\"MARKER-OPT-tea\":"), number_after(r.body, "\"MARKER-OPT-soup\":"),
        number_after(r.body, "\"probabilities\":{\"0\":"), number_after(strstr(r.body, "\"probabilities\":{\"0\":"), "\"1\":"),
        number_after(strstr(r.body, "\"probabilities\":{\"0\":"), "\"2\":"), number_after(r.body, "\"score\":")};
    double expected[7] = {hot[0], what[0], what[1], level[0], level[1], level[2], level[1] + 2 * level[2]};
    for (int i = 0; i < 7; i++) CHECK(fabs(served[i] - expected[i]) < 0.000000001, "value %d served %.12f direct %.12f", i, served[i], expected[i]);
    const char *pick = what[1] > what[0] ? "\"choice\":\"MARKER-OPT-soup\"" : "\"choice\":\"MARKER-OPT-tea\"";
    CHECK(strstr(r.body, pick) != NULL, "choice is the most probable option");
    CHECK(fabs(served[1] + served[2] - 1) < 0.000001 && fabs(served[3] + served[4] + served[5] - 1) < 0.000001, "distributions sum to one");
    printf("answer: noul %.9f, choice %.9f/%.9f, score %.9f, matches direct chapter-14 calls\n", served[0], served[1], served[2], served[6]);
    free(r.data);
    /* Each question forks from the same prefix snapshot, so neither its
       siblings nor their order can change it: every question alone, and all
       three in reverse order, give bit-identical numbers. */
    #define Q_HOT "\"MARKER-Q-hot\":{\"type\":\"noul\",\"instructions\":\"Is the water hot?\"}"
    #define Q_WHAT "\"MARKER-Q-what\":{\"type\":\"choice\",\"criteria\":{\"MARKER-OPT-tea\":null,\"MARKER-OPT-soup\":\"a meal\"}}"
    #define Q_LEVEL "\"MARKER-Q-level\":{\"type\":\"score\",\"criteria\":[\"cold\",\"warm\",\"hot\"]}"
    #define ASK(qs) "{\"model\":\"psionic-toy\",\"state\":\"" STATE "\",\"questions\":{" qs "}}"
    const char *bodies[4] = {ASK(Q_LEVEL "," Q_WHAT "," Q_HOT), ASK(Q_HOT), ASK(Q_WHAT), ASK(Q_LEVEL)};
    int from[7] = {1, 2, 2, 3, 3, 3, 3};
    for (int k = 0; k < 4; k++) {
        Reply q = ask_body(a.port, bodies[k]); CHECK(q.status == 200, "permuted/alone %d status %d %s", k, q.status, q.body);
        const char *probs = strstr(q.body, "\"probabilities\":{\"0\":");
        double got[7] = {number_after(q.body, "\"noul\":"), number_after(q.body, "\"MARKER-OPT-tea\":"), number_after(q.body, "\"MARKER-OPT-soup\":"),
            number_after(q.body, "\"probabilities\":{\"0\":"), number_after(probs, "\"1\":"), number_after(probs, "\"2\":"), number_after(q.body, "\"score\":")};
        for (int i = 0; i < 7; i++) if (k == 0 || from[i] == k) CHECK(got[i] == served[i], "request %d value %d: %.17g, joint %.17g", k, i, got[i], served[i]);
        if (k == 0) CHECK(starts(q.body, "{\"model\":\"psionic-toy\",\"answers\":{\"MARKER-Q-level\""), "answers keep request order: %s", q.body);
        free(q.data);
    }
    printf("answers: each question alone and all three reversed are bit-identical to the joint request\n");
    char *m = post(BODY); size_t n = strlen(m); int fd = dial(a.port);
    for (size_t i = 0; i < n; i += 97) { send_all(fd, m + i, n - i < 97 ? n - i : 97); pause_ms(5); }
    r = receive(fd, 120000); CHECK(r.status == 200 && strstr(r.body, IDENTITY), "partial writes %d", r.status); free(r.data);
    char head[256]; snprintf(head, sizeof head, "POST /v1/systemone HTTP/1.1\r\nExpect: 100-continue\r\nContent-Length: %zu\r\n\r\n", strlen(BODY));
    fd = dial(a.port); send_all(fd, head, strlen(head)); char c[64] = {0}; struct pollfd p = {fd, POLLIN, 0};
    CHECK(poll(&p, 1, 2000) == 1 && read(fd, c, 25) == 25 && !strcmp(c, "HTTP/1.1 100 Continue\r\n\r\n"), "100 continue: %s", c);
    send_all(fd, BODY, strlen(BODY)); r = receive(fd, 120000); CHECK(r.status == 200 && strstr(r.body, IDENTITY), "after continue %d", r.status); free(r.data); free(m);
    printf("answers: complete, partial writes and Expect: 100-continue carry the served identity\n");
}

static void refusals(Server a) {
    expect_refusal(ask_body(a.port, "{\"model\":"), 400, "malformed", "truncated JSON");
    expect_refusal(ask_body(a.port, "{\"state\":\"s\",\"questions\":{\"q\":{\"type\":\"noul\"}},\"model\":\"psionic-toy\",\"x\":}"), 400, "malformed", "bad JSON");
    expect_refusal(ask_body(a.port, "{\"model\":\"psionic-toy\",\"questions\":{\"q\":{\"type\":\"noul\"}}}"), 400, "invalid_request", "no state");
    expect_refusal(ask_body(a.port, "{\"model\":\"psionic-toy\",\"state\":7,\"questions\":{\"q\":{\"type\":\"noul\"}}}"), 400, "invalid_request", "number state");
    char *b = repeat_questions(65); expect_refusal(ask_body(a.port, b), 400, "too_many_questions", "65 questions"); free(b);
    b = many_options(256); expect_refusal(ask_body(a.port, b), 400, "too_many_options", "256 options"); free(b);
    b = many_options(27); expect_refusal(ask_body(a.port, b), 422, "unsupported_input", "27 options on the toy"); free(b);
    expect_refusal(ask_body(a.port, "{\"model\":\"elephant-1\",\"state\":\"s\",\"questions\":{\"q\":{\"type\":\"noul\"}}}"), 403, "not_admitted", "another model");
    expect_refusal(ask_body(a.port, "{\"model\":\"psionic-toy\",\"state\":{\"a\":1.5},\"questions\":{\"q\":{\"type\":\"noul\"}}}"), 422, "unsupported_input", "fractional state number");
    const char *get = "GET /v1/systemone HTTP/1.1\r\n\r\n"; expect_refusal(ask(a.port, get, strlen(get)), 405, "method_not_allowed", "GET");
    const char *other = "POST /v1/other HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}"; expect_refusal(ask(a.port, other, strlen(other)), 404, "not_found", "other route");
    const char *chunked = "POST /v1/systemone HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"; expect_refusal(ask(a.port, chunked, strlen(chunked)), 400, "malformed", "chunked");
    const char *junk = "\x16\x03\x01 hello\r\n\r\n"; expect_refusal(ask(a.port, junk, strlen(junk)), 400, "malformed", "not HTTP");
    const char *huge = "POST /v1/systemone HTTP/1.1\r\nContent-Length: 262145\r\n\r\n"; long long t0 = now_ms();
    expect_refusal(ask(a.port, huge, strlen(huge)), 413, "limit_exceeded", "declared body over 256 KiB");
    CHECK(now_ms() - t0 < 1000, "an oversized declaration is refused before its body");
    char *longhead = malloc(9001); memset(longhead, 'a', 9000); memcpy(longhead, "POST /v1/systemone HTTP/1.1\r\nX: ", 33); longhead[9000] = 0;
    expect_refusal(ask(a.port, longhead, 9000), 413, "limit_exceeded", "head over 8 KiB"); free(longhead);
    char *big = malloc(262144 + 256); int at = snprintf(big,262144+256, "{\"model\":\"psionic-toy\",\"state\":\""); memset(big + at, 's', 262144 - at - 50); at = 262144 - 50;
    snprintf(big + at,262144+256-(size_t)at, "\",\"questions\":{\"q\":{\"type\":\"noul\"}}}"); expect_refusal(ask_body(a.port, big), 400, "invalid_request", "state over 128 KiB"); free(big);
    printf("refusals: malformed, invalid, too many questions/options, not admitted, unsupported, route, method, chunked and oversized\n");
}

static void slow_clients(Server a) {
    int fd = dial(a.port); char *m = post(BODY); send_all(fd, m, strlen(m) / 2); long long t0 = now_ms();
    Reply r = receive(fd, 5000); long long waited = now_ms() - t0;
    CHECK(r.status == 408 && has_code(r, "request_timeout") && waited >= 300 && waited < 2000, "half a request: %d after %lld ms", r.status, waited); free(r.data);
    fd = dial(a.port); r = receive(fd, 5000); CHECK(r.status == 408 && has_code(r, "request_timeout"), "silent client %d", r.status); free(r.data);
    fd = dial(a.port); send_all(fd, m, strlen(m) / 3); pause_ms(50); close(fd); pause_ms(50);
    r = ask_body(a.port, BODY); CHECK(r.status == 200, "after an abandoned request %d", r.status); free(r.data); free(m);
    printf("slow clients: a partial or silent request times out at the read deadline; an abandoned one frees the slot\n");
}

static void busy_and_cancel(Server b) {
    char *m = post(BODY); int first = dial(b.port); send_all(first, m, strlen(m)); pause_ms(150);
    long long t0 = now_ms(); Reply busy = ask(b.port, m, strlen(m));
    CHECK(busy.status == 503 && has_code(busy, "busy") && strstr(busy.data, "\r\nRetry-After: 1\r\n") && now_ms() - t0 < 600, "busy %d after %lld ms", busy.status, now_ms() - t0); free(busy.data);
    Reply done = receive(first, 120000); CHECK(done.status == 200 && strstr(done.body, IDENTITY), "first after busy %d", done.status);
    CHECK(number_after(done.body, "\"latency_ms\":") >= 700, "latency includes the held decision"); free(done.data);
    first = dial(b.port); send_all(first, m, strlen(m)); pause_ms(150); close(first); pause_ms(100);
    t0 = now_ms(); Reply after = ask(b.port, m, strlen(m));
    CHECK(after.status == 200 && after.ms >= 650, "after cancel %d in %lld ms", after.status, after.ms); free(after.data); free(m);
    printf("one at a time: a second client is refused busy at once; a disconnect cancels the running decision\n");
}

static void deadline(Server c) {
    long long t0 = now_ms(); Reply r = ask_body(c.port, BODY); long long waited = now_ms() - t0;
    CHECK(r.status == 524 && has_code(r, "timeout") && waited >= 250 && waited < 1500, "deadline %d after %lld ms", r.status, waited); free(r.data);
    r = ask_body(c.port, BODY); CHECK(r.status == 524 && has_code(r, "timeout"), "second deadline %d", r.status); free(r.data);
    printf("deadline: an unfinished decision is stopped and refused with 524 timeout\n");
}

static void logs_hold_no_content(Server s) {
    FILE *f = fopen(s.log, "r"); char line[1024]; int lines = 0;
    if (!f) { CHECK(0, "missing log %s", s.log); return; }
    while (fgets(line, sizeof line, f)) {
        lines++;
        CHECK(!strstr(line, "MARKER") && !strstr(line, "kettle") && !strstr(line, "hot") && !strstr(line, "{"), "log leaks content: %s", line);
        CHECK(!strncmp(line, "psionic-serve: ", 15), "unexpected log line: %s", line);
    }
    fclose(f); CHECK(lines > 0, "empty log");
}

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: serve-test PSIONIC_SERVE LOG_DIRECTORY\n"); return 2; }
    signal(SIGPIPE, SIG_IGN);
    const char *fast[] = {"--read-ms", "400", "--deadline-ms", "120000", NULL};
    const char *held[] = {"--read-ms", "2000", "--deadline-ms", "120000", "--fixture-delay-ms", "700", NULL};
    const char *late[] = {"--read-ms", "2000", "--deadline-ms", "300", "--fixture-delay-ms", "3000", NULL};
    Server a = start(argv[1], argv[2], "fast", fast), b = start(argv[1], argv[2], "held", held), c = start(argv[1], argv[2], "late", late);
    answers(a); refusals(a); slow_clients(a); busy_and_cancel(b); deadline(c);
    Server all[3] = {a, b, c};
    for (int i = 0; i < 3; i++) { CHECK(stop(all[i]) == 0, "server %d exit", i); logs_hold_no_content(all[i]); }
    if (failures) { fprintf(stderr, "%d serve check(s) failed\n", failures); return 1; }
    printf("serve: all native checks pass\n");
    return 0;
}
