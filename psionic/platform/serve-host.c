/* psionic-serve: the chapter-16 host for POST /v1/systemone.
 *
 * Effects only. This file opens a loopback socket, reads and writes bounded
 * bytes, reads a monotonic clock, maps model files and runs each decision in
 * one child process so that it can be stopped. What the bytes mean, every
 * refusal, the deadlines, busy refusals and the answer body are decided by
 * Coax (psionic/lib/serve-request.coax, serve-engine.coax, serve-run.coax,
 * and serve-proto.coax for the protobuf wire of chapter 16b).
 * Nothing from a request or an answer is logged: a log line has a status, a
 * code name and a duration.
 *
 *   psionic-serve --toy [--port N] [--read-ms N] [--deadline-ms N] [--capacity N]
 *   psionic-serve --elephant DIR --table FILE [the same options]
 *   psionic-serve --elephant-gpu DIR --table FILE [the same options]
 *   psionic-serve --elephant-metal DIR --table FILE [the same options]
 *
 * --fixture-delay-ms N (toy only) holds each decision for N ms before it
 * starts, so tests can observe busy refusals, deadlines and cancellation.
 * The server binds 127.0.0.1 only.
 *
 * --elephant-gpu answers on a CUDA device (psionic/platform/elephant-gpu.c).
 * A CUDA context does not survive fork, so the decision runs in this process,
 * one at a time; between layers it asks whether the deadline has passed or
 * the client has gone, and a stopped decision publishes nothing. The kernels
 * are the generated project.cu, layer.cu and warp.cu next to this binary.
 *
 * --elephant-metal answers on an Apple GPU (psionic/platform/elephant-metal.c,
 * built with PSIONIC_SERVE_METAL) the same way, with project.metal,
 * layer.metal, warp.metal and tile.metal next to this binary. */
#define _GNU_SOURCE
#include "serve.h"
#include "spans.h"
#include "verified-file.h"
#include "elephant-inventory.h"
#if defined(PSIONIC_SERVE_METAL)
#include "elephant-metal.h"
#include <mach-o/dyld.h>
#define PSIONIC_SERVE_GPU 1 /* the same in-process device path */
#define DEVICE_FLAG "--elephant-metal"
#define DEVICE_KIND "Metal"
#define KERNEL_FILES 4
static const char *kernel_names[KERNEL_FILES] = {"project.metal", "layer.metal", "warp.metal", "tile.metal"};
#elif defined(PSIONIC_SERVE_GPU)
#include "elephant-gpu.h"
#define DEVICE_FLAG "--elephant-gpu"
#define DEVICE_KIND "CUDA"
#define KERNEL_FILES 3
static const char *kernel_names[KERNEL_FILES] = {"project.cu", "layer.cu", "warp.cu"};
#endif
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

coax_t_PsSession psionic_session_open(uint8_t,int64_t,coax_v_u8,coax_v_u8,coax_v_u8,coax_v_u8,coax_v_t_PaFile,coax_v_t_P5Spec,coax_v_u8,coax_t_PsWorkspace);

#define PROTO_SCRATCH (262144 + 65536 + 64) /* SV_BODY_MAX + 64 KiB + 64: a transcoded protobuf request */
#define PROTO_ARENA (2 << 20)                /* sp_answer's intermediate messages */
#define LINGER_MS 200 /* drain a closing client's unread bytes so the reply is not reset */

static volatile sig_atomic_t stopping;
static void on_signal(int sig) { (void)sig; stopping = 1; }

static int64_t now_ms(void) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
    return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static void *zalloc(size_t n, size_t width) {
    void *p = calloc(n ? n : 1, width);
    if (!p) { fprintf(stderr, "psionic-serve: out of memory\n"); exit(2); }
    return p;
}
static coax_v_u8 map_file(const char *path) {
    int fd = open(path, O_RDONLY); struct stat st;
    if (fd < 0 || fstat(fd, &st) || st.st_size <= 0) { fprintf(stderr, "psionic-serve: cannot read %s\n", path); exit(2); }
    void *p = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (p == MAP_FAILED) { fprintf(stderr, "psionic-serve: cannot map %s\n", path); exit(2); }
    return (coax_v_u8){p, st.st_size};
}
static coax_v_u8 map_named(const char *dir, const char *name) {
    char path[4096]; int n = snprintf(path, sizeof path, "%s/%s", dir, name);
    if (n < 0 || (size_t)n >= sizeof path) exit(2);
    return map_file(path);
}
static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }

/* Write all bytes or give up at the deadline. */
static void write_all(int fd, const uint8_t *data, int64_t n, int64_t deadline) {
    while (n > 0) {
        ssize_t w = send(fd, data, (size_t)n, MSG_NOSIGNAL);
        if (w > 0) { data += w; n -= w; continue; }
        if (w < 0 && errno == EINTR) continue;
        if (w < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            int64_t left = deadline - now_ms(); if (left <= 0) return;
            struct pollfd p = {fd, POLLOUT, 0}; poll(&p, 1, (int)left); continue;
        }
        return;
    }
}
static void finish(int fd, const uint8_t *data, int64_t n, int64_t write_ms) {
    if (n > 0) write_all(fd, data, n, now_ms() + write_ms);
    shutdown(fd, SHUT_WR);
    int64_t until = now_ms() + LINGER_MS; uint8_t sink[4096];
    for (;;) {
        int64_t left = until - now_ms(); if (left <= 0) break;
        struct pollfd p = {fd, POLLIN, 0};
        if (poll(&p, 1, (int)left) <= 0) break;
        ssize_t r = read(fd, sink, sizeof sink); if (r <= 0) break;
    }
    close(fd);
}

typedef struct {
    coax_t_SvEngine engine; coax_t_SvPools pools; coax_t_SvWork work; coax_v_f64 answers;
    coax_v_u8 rx, out, frame; coax_t_SvLimits limits; int64_t delay_ms;
    coax_v_u8 scratch, proto, arena; bool proto_out; /* chapter 16b */
#ifdef PSIONIC_SERVE_GPU
    EgEngine *gpu; int stop_fd; int64_t stop_at;
#endif
} Server;

static bool overlaps(coax_v_u8 *spans, size_t n, const Server *s) {
    /* Every writable buffer must be disjoint from every other and from the
       session's borrowed storage. Coax will not answer until this is sealed. */
    typedef struct { const void *p; int64_t n; size_t w; } V;
    V w[] = {
        {s->rx.data, s->rx.count, 1}, {s->out.data, s->out.count, 1}, {s->frame.data, s->frame.count, 1},
        {s->scratch.data, s->scratch.count, 1}, {s->proto.data, s->proto.count, 1}, {s->arena.data, s->arena.count, 1},
        {s->pools.questions.data, s->pools.questions.count, sizeof *s->pools.questions.data},
        {s->pools.options.data, s->pools.options.count, sizeof *s->pools.options.data},
        {s->pools.raw.data, s->pools.raw.count, sizeof *s->pools.raw.data},
        {s->pools.arena.data, s->pools.arena.count, 1},
        {s->work.prep.prompt.data, s->work.prep.prompt.count, 1}, {s->work.prep.render_work.data, s->work.prep.render_work.count, 1},
        {s->work.prep.candidate.data, s->work.prep.candidate.count, 8}, {s->work.prep.full.data, s->work.prep.full.count, 8},
        {s->work.prep.token_stage.data, s->work.prep.token_stage.count, 8}, {s->work.prep.normal.data, s->work.prep.normal.count, 1},
        {s->work.prep.scalars.data, s->work.prep.scalars.count, 8}, {s->work.prep.symbols.data, s->work.prep.symbols.count, 8},
        {s->work.tokens.data, s->work.tokens.count, 8}, {s->work.state.data, s->work.state.count, 4},
        {s->work.work.data, s->work.work.count, 4}, {s->work.prob.data, s->work.prob.count, 8},
        {s->answers.data, s->answers.count, 8}};
    size_t nw = sizeof w / sizeof *w;
    for (size_t i = 0; i < nw; i++) {
        for (size_t j = i + 1; j < nw; j++) if (psionic_spans_overlap(w[i].p, w[i].n, w[i].w, w[j].p, w[j].n, w[j].w)) return true;
        for (size_t j = 0; j < n; j++) if (psionic_spans_overlap(w[i].p, w[i].n, w[i].w, spans[j].data, spans[j].count, 1)) return true;
    }
    return false;
}

static void allocate(Server *s) {
    coax_t_SvPlan plan = sv_plan(s->engine); coax_t_SvSizes z = sv_sizes();
    if (!plan.ok) { fprintf(stderr, "psionic-serve: no plan for this engine\n"); exit(1); }
    s->rx = (coax_v_u8){zalloc((size_t)z.request, 1), z.request};
    s->out = (coax_v_u8){zalloc((size_t)(z.head_room + z.response), 1), z.head_room + z.response};
    s->frame = (coax_v_u8){zalloc((size_t)(z.frame + z.response), 1), z.frame + z.response};
    s->scratch = (coax_v_u8){zalloc(PROTO_SCRATCH, 1), PROTO_SCRATCH};
    s->proto = (coax_v_u8){zalloc((size_t)(z.head_room + z.response), 1), z.head_room + z.response};
    s->arena = (coax_v_u8){zalloc(PROTO_ARENA, 1), PROTO_ARENA};
    s->pools.questions = (coax_v_t_SvQuestion){zalloc((size_t)z.questions, sizeof(coax_t_SvQuestion)), z.questions};
    s->pools.options = (coax_v_t_PtOption){zalloc((size_t)z.options, sizeof(coax_t_PtOption)), z.options};
    s->pools.raw = (coax_v_string){zalloc((size_t)z.options, sizeof(coax_string)), z.options};
    s->pools.arena = (coax_v_u8){zalloc((size_t)z.arena, 1), z.arena};
    s->work.prep.prompt = (coax_v_u8){zalloc((size_t)plan.prompt, 1), plan.prompt};
    s->work.prep.render_work = (coax_v_u8){zalloc((size_t)plan.render, 1), plan.render};
    s->work.prep.candidate = (coax_v_s64){zalloc((size_t)plan.candidate, 8), plan.candidate};
    s->work.prep.full = (coax_v_s64){zalloc((size_t)plan.full, 8), plan.full};
    s->work.prep.token_stage = (coax_v_s64){zalloc((size_t)plan.stage, 8), plan.stage};
    s->work.prep.normal = (coax_v_u8){zalloc((size_t)plan.normal, 1), plan.normal};
    s->work.prep.scalars = (coax_v_s64){zalloc((size_t)plan.scalars, 8), plan.scalars};
    s->work.prep.symbols = (coax_v_s64){zalloc((size_t)plan.symbols, 8), plan.symbols};
    s->work.tokens = (coax_v_s64){zalloc((size_t)plan.tokens, 8), plan.tokens};
    s->work.state = (coax_v_f32){zalloc((size_t)plan.state, 4), plan.state};
    s->work.work = (coax_v_f32){zalloc((size_t)plan.work, 4), plan.work};
    s->work.prob = (coax_v_f64){zalloc((size_t)plan.prob, 8), plan.prob};
    s->answers = (coax_v_f64){zalloc((size_t)plan.answers, 8), plan.answers};
}

static void open_toy(Server *s, int64_t capacity) {
    coax_t_SvToySizes z = sv_toy_sizes();
    coax_t_SvToyFiles f = {{zalloc((size_t)z.model, 1), z.model}, {zalloc((size_t)z.config, 1), z.config},
        {zalloc((size_t)z.tokenizer, 1), z.tokenizer}, {zalloc((size_t)z.table, 1), z.table},
        {zalloc((size_t)z.tensors, sizeof(coax_t_P5Spec)), z.tensors}};
    if (!sv_toy_copy(f)) { fprintf(stderr, "psionic-serve: toy files do not fit\n"); exit(1); }
    coax_t_PsWorkspace m = {{zalloc((size_t)z.tensors, sizeof(coax_t_P5Tensor)), z.tensors}, {zalloc((size_t)z.tensors, sizeof(coax_t_P5Tensor)), z.tensors},
        {zalloc((size_t)z.layers, sizeof(coax_t_QeLayer)), z.layers}, {zalloc((size_t)z.params, 4), z.params}, {zalloc((size_t)z.params, 4), z.params},
        {zalloc((size_t)z.layer_work, 4), z.layer_work}, {zalloc((size_t)z.norm, 4), z.norm}, {zalloc((size_t)z.norm, 4), z.norm}};
    s->engine = sv_toy_open(f, m, capacity);
    if (!s->engine.ok) { fprintf(stderr, "psionic-serve: toy session refused: status %u cause %lld\n", s->engine.session.status, (long long)s->engine.session.cause); exit(1); }
    allocate(s);
    coax_v_u8 read_only[] = {f.model, f.config, f.tokenizer, f.table};
    s->engine = sv_seal(s->engine, overlaps(read_only, 4, s));
}

static void open_elephant(Server *s, const char *dir, const char *table_path, int64_t capacity, bool gpu) {
    coax_v_u8 config = map_named(dir, "config.json"), tokenizer = map_named(dir, "tokenizer.json"), readout = map_named(dir, "readout.json");
    coax_v_u8 index = {NULL, 0}, table = map_file(table_path);
    coax_t_PaFile *file = zalloc(1, sizeof *file); file->data = map_named(dir, "model.safetensors");
    char weights_path[4096]; snprintf(weights_path, sizeof weights_path, "%s/model.safetensors", dir);
    VfEntry *verified = zalloc(1, sizeof *verified); vf_attach(verified, weights_path, file);
    size_t count = sizeof elephant_specs / sizeof *elephant_specs;
    coax_t_P5Spec *specs = zalloc(count, sizeof *specs);
    for (size_t i = 0; i < count; i++) {
        specs[i].name = str(elephant_specs[i].name); specs[i].shape.rank = elephant_specs[i].rank;
        for (int j = 0; j < 5; j++) specs[i].shape.dims.e[j] = elephant_specs[i].dims[j];
        specs[i].dtype = COAX_PwDtype_bf16; specs[i].shard = 0;
    }
    coax_t_PsWorkspace m = {{zalloc(count, sizeof(coax_t_P5Tensor)), (int64_t)count}, {zalloc(count, sizeof(coax_t_P5Tensor)), (int64_t)count},
        {zalloc(32, sizeof(coax_t_QeLayer)), 32}, {zalloc(1057280, 4), 1057280}, {zalloc(1057280, 4), 1057280},
        {zalloc(50000, 4), 50000}, {zalloc(4096, 4), 4096}, {zalloc(4096, 4), 4096}};
    if (file->record.count) printf("psionic-serve: authenticating elephant-1 (a verified-file record names this checkpoint; its hash is skipped)\n");
    else printf("psionic-serve: authenticating elephant-1 (hashes the 18.8 GB checkpoint once; later starts skip it)\n");
    fflush(stdout);
    int64_t t0 = now_ms();
    coax_t_PsSession session = psionic_session_open(0, capacity, config, tokenizer, readout, index, (coax_v_t_PaFile){file, 1}, (coax_v_t_P5Spec){specs, (int64_t)count}, table, m);
    if (session.status) { fprintf(stderr, "psionic-serve: elephant session refused: status %u cause %lld\n", session.status, (long long)session.cause); exit(1); }
    printf("psionic-serve: authenticated in %lld ms\n", (long long)(now_ms() - t0));
    int kept = vf_keep(verified, 0, capacity, 0);
    if (kept == 1) printf("psionic-serve: kept a verified-file record at %s\n", verified->path);
    if (kept < 0) printf("psionic-serve: no verified-file record kept; the next start hashes the checkpoint again\n");
    fflush(stdout);
    s->engine = sv_elephant(session, capacity);
    if (!s->engine.ok) { fprintf(stderr, "psionic-serve: elephant engine refused\n"); exit(1); }
#ifdef PSIONIC_SERVE_GPU
    if (gpu) {
        t0 = now_ms();
        int r = eg_place(s->gpu, session.model, capacity, 0);
        if (r) { fprintf(stderr, "psionic-serve: device placement refused: %d %s\n", r, eg_log()); exit(1); }
        printf("psionic-serve: placed %.2f GiB resident and %.2f GiB pinned for streaming in %lld ms (plan budget %.2f GiB)\n",
            s->gpu->resident_bytes / 1073741824.0, s->gpu->pinned_bytes / 1073741824.0, (long long)(now_ms() - t0), s->gpu->budget / 1073741824.0);
        fflush(stdout);
    }
#else
    (void)gpu;
#endif
    allocate(s);
    coax_v_u8 read_only[] = {config, tokenizer, readout, table, file->data};
    s->engine = sv_seal(s->engine, overlaps(read_only, 5, s));
}

/* The response for an answered or refused request: a refusal is JSON; an
   answer is the rendered JSON, or its protobuf when the request asked for
   one (sp_answer). Sets *data to the buffer that holds the span. */
static coax_t_SvSpan respond(Server *s, coax_t_SvRequest request, coax_t_SvCode *code, int64_t started, const uint8_t **data) {
    coax_t_SvSpan span = {0, 0};
    *data = s->out.data;
    if (*code == COAX_SvCode_none) {
        span = sv_render(s->engine, request, s->pools, s->answers, now_ms() - started, s->out);
        if (span.to == 0) *code = COAX_SvCode_internal;
    }
    if (*code == COAX_SvCode_none && s->proto_out) {
        const uint8_t *p = s->out.data + span.from, *end = s->out.data + span.to, *body = NULL;
        for (; p + 4 <= end; p++) if (!memcmp(p, "\r\n\r\n", 4)) { body = p + 4; break; }
        coax_t_SpBody b = {COAX_SvCode_internal, 0, 0};
        if (body) b = sp_answer((coax_string){(const char *)body, end - body}, s->proto, s->arena);
        if (b.code == COAX_SvCode_none && b.to > b.from) { *data = s->proto.data; return (coax_t_SvSpan){b.from, b.to}; }
        *code = COAX_SvCode_internal;
    }
    if (*code != COAX_SvCode_none) span = (coax_t_SvSpan){0, sv_refusal(s->out, *code)};
    return span;
}

/* The child: run the admitted request, frame the response, exit. */
static void decide(Server *s, coax_t_SvRequest request, int64_t started, int fd) {
    if (s->delay_ms > 0) { struct timespec t = {s->delay_ms / 1000, (s->delay_ms % 1000) * 1000000}; while (nanosleep(&t, &t) && errno == EINTR) {} }
    coax_t_SvCode code = sv_answers(s->engine, request, s->pools, s->work, s->answers);
    const uint8_t *data;
    coax_t_SvSpan span = respond(s, request, &code, started, &data);
    uint8_t head[8];
    if (span.to <= span.from || sv_frame_put((coax_v_u8){head, sizeof head}, code, span.to - span.from) < 0) _exit(1);
    coax_t_SvSizes z = sv_sizes();
    const uint8_t *p = head; int64_t n = z.frame;
    for (int part = 0; part < 2; part++) {
        while (n > 0) { ssize_t w = write(fd, p, (size_t)n); if (w < 0 && errno == EINTR) continue; if (w <= 0) _exit(1); p += w; n -= w; }
        p = data + span.from; n = span.to - span.from;
    }
    _exit(0);
}

#ifdef PSIONIC_SERVE_GPU
static void open_gpu(Server *s) {
    char exe[4096];
#if defined(PSIONIC_SERVE_METAL)
    uint32_t size = sizeof exe;
    ssize_t n = _NSGetExecutablePath(exe, &size) == 0 ? (ssize_t)strlen(exe) : -1;
#else
    ssize_t n = readlink("/proc/self/exe", exe, sizeof exe - 1);
#endif
    if (n <= 0) { fprintf(stderr, "psionic-serve: cannot find the kernels\n"); exit(2); }
    exe[n] = 0; char *slash = strrchr(exe, '/'); if (slash) *slash = 0;
    EgSources src;
    for (int i = 0; i < KERNEL_FILES; i++) { coax_v_u8 text = map_named(exe, kernel_names[i]); src.text[i] = (const char *)text.data; src.length[i] = (size_t)text.count; }
    s->gpu = zalloc(1, sizeof *s->gpu);
    int64_t t0 = now_ms();
    int r = eg_compile(s->gpu, &src);
    if (r) { fprintf(stderr, "psionic-serve: no " DEVICE_KIND " device or compiler: %s\n", eg_log()); exit(1); }
    printf("psionic-serve: %s; compiled %d generated kernels in %lld ms\n", s->gpu->device.name, EG_KERNELS, (long long)(now_ms() - t0)); fflush(stdout);
}
/* Asked between layers: stop when the deadline has passed or the client has closed. */
static int gpu_stop(void *context) {
    Server *s = context;
    if (now_ms() >= s->stop_at) return 1;
    uint8_t b; ssize_t r = recv(s->stop_fd, &b, 1, MSG_PEEK | MSG_DONTWAIT);
    return r == 0 || (r < 0 && errno != EAGAIN && errno != EWOULDBLOCK && errno != EINTR);
}
/* Decide in this process and frame the response into s->frame as the child
   would have written it. Returns the frame length, or -1 when stopped. */
static int64_t decide_gpu(Server *s, coax_t_SvRequest request, int64_t started) {
    coax_t_SvCode code = sv_admit(s->engine, request, s->pools);
    for (int64_t i = 0; code == COAX_SvCode_none && i < request.questions; i++) {
        coax_t_SvStaged staged = sv_stage(s->engine, request, s->pools, s->work, i);
        if ((code = staged.code) != COAX_SvCode_none) break;
        int r = eg_decide(s->gpu, s->work.tokens.data, staged.prepared.written, staged.prepared.prefix, gpu_stop, s);
        if (r == EG_STOPPED) return -1;
        if (r) { code = COAX_SvCode_internal; break; }
        code = sv_read(s->engine, request, s->pools, i, staged.prepared, (coax_v_f32){s->gpu->logits, EG_VOCAB}, s->answers, s->work.prob);
    }
    const uint8_t *data;
    coax_t_SvSpan span = respond(s, request, &code, started, &data);
    coax_t_SvSizes z = sv_sizes();
    int64_t body = span.to - span.from;
    if (body <= 0 || z.frame + body > s->frame.count || sv_frame_put((coax_v_u8){s->frame.data, z.frame}, code, body) < 0) return 0;
    memcpy(s->frame.data + z.frame, data + span.from, (size_t)body);
    return z.frame + body;
}
#endif

static void log_line(int status, coax_t_SvCode code, int64_t ms) {
    coax_string name = sv_code_name(code);
    if (name.count) fprintf(stderr, "psionic-serve: %d %.*s %lldms\n", status, (int)name.count, name.data, (long long)ms);
    else fprintf(stderr, "psionic-serve: %d ok %lldms\n", status, (long long)ms);
}

static int listen_loopback(int port) {
    int fd = socket(AF_INET, SOCK_STREAM, 0), one = 1;
    if (fd < 0) { perror("socket"); exit(2); }
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    struct sockaddr_in a = {0}; a.sin_family = AF_INET; a.sin_port = htons((uint16_t)port); a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (bind(fd, (struct sockaddr *)&a, sizeof a) || listen(fd, 16)) { perror("psionic-serve: bind 127.0.0.1"); exit(2); }
    fcntl(fd, F_SETFL, O_NONBLOCK);
    return fd;
}

static long option(const char *v, long lo, long hi, const char *name) {
    char *end; long x = strtol(v, &end, 10);
    if (*v == 0 || *end || x < lo || x > hi) { fprintf(stderr, "psionic-serve: bad %s\n", name); exit(2); }
    return x;
}

int main(int argc, char **argv) {
    const char *elephant = NULL, *table = NULL, *device_flag = NULL; bool toy = false, gpu = false;
    long port = 8790, read_ms = 10000, deadline_ms = 30000, capacity = 0, delay = 0;
    for (int i = 1; i < argc; i++) {
        const char *a = argv[i], *v = i + 1 < argc ? argv[i + 1] : NULL;
        if (!strcmp(a, "--toy")) { toy = true; continue; }
        if (!v) { fprintf(stderr, "psionic-serve: %s needs a value\n", a); return 2; }
        if (!strcmp(a, "--elephant")) elephant = v;
        else if (!strcmp(a, "--elephant-gpu")) { elephant = v; gpu = true; device_flag = a; }
        else if (!strcmp(a, "--elephant-metal")) { elephant = v; gpu = true; device_flag = a; }
        else if (!strcmp(a, "--table")) table = v;
        else if (!strcmp(a, "--port")) port = option(v, 0, 65535, a);
        else if (!strcmp(a, "--read-ms")) read_ms = option(v, 1, 600000, a);
        else if (!strcmp(a, "--deadline-ms")) deadline_ms = option(v, 1, 86400000, a);
        else if (!strcmp(a, "--capacity")) capacity = option(v, 1, 16384, a);
        else if (!strcmp(a, "--fixture-delay-ms")) delay = option(v, 0, 600000, a);
        else { fprintf(stderr, "usage: psionic-serve (--toy | --elephant DIR | --elephant-gpu DIR | --elephant-metal DIR) --table FILE [--port N] [--read-ms N] [--deadline-ms N] [--capacity N]\n"); return 2; }
        i++;
    }
#ifndef PSIONIC_SERVE_GPU
    (void)device_flag;
    if (gpu) { fprintf(stderr, "psionic-serve: built without a device host; build with serve-build.sh OUT release plain gpu|metal\n"); return 2; }
#else
    if (gpu && strcmp(device_flag, DEVICE_FLAG)) { fprintf(stderr, "psionic-serve: built for " DEVICE_FLAG ", not %s\n", device_flag); return 2; }
#endif
    if (gpu && capacity > 512) { fprintf(stderr, "psionic-serve: the device plan takes at most 512 tokens\n"); return 2; }
    if (toy == (elephant != NULL) || (elephant && !table) || (delay && !toy)) {
        fprintf(stderr, "usage: psionic-serve (--toy | --elephant DIR | --elephant-gpu DIR | --elephant-metal DIR) --table FILE [--port N] [--read-ms N] [--deadline-ms N] [--capacity N]\n"); return 2;
    }
    signal(SIGPIPE, SIG_IGN); signal(SIGTERM, on_signal); signal(SIGINT, on_signal);
    Server s = {0}; s.limits = (coax_t_SvLimits){read_ms, deadline_ms}; s.delay_ms = delay;
    if (!sv_limits_valid(s.limits)) { fprintf(stderr, "psionic-serve: limits refused\n"); return 2; }
#ifdef PSIONIC_SERVE_GPU
    if (gpu) open_gpu(&s);
#endif
    if (toy) open_toy(&s, capacity ? capacity : 2048); else open_elephant(&s, elephant, table, capacity ? capacity : gpu ? 128 : 512, gpu);
    if (!s.engine.sealed) { fprintf(stderr, "psionic-serve: buffers overlap; refusing to serve\n"); return 1; }
    int lfd = listen_loopback((int)port);
    struct sockaddr_in bound; socklen_t len = sizeof bound; getsockname(lfd, (struct sockaddr *)&bound, &len);
    printf("psionic-serve: %.*s@sha256:%.*s on http://127.0.0.1:%d/v1/systemone\n", (int)s.engine.name.count, s.engine.name.data,
        (int)s.engine.digest.count, s.engine.digest.data, ntohs(bound.sin_port));
    fflush(stdout);

    coax_t_SvSlot slot = {0}; int cfd = -1, child_fd = -1; pid_t child = -1; uint32_t epoch = 0;
    int64_t rx = 0, frame = 0, accepted = 0, started = 0; bool overflow = false, ready = false; uint8_t sink[65536];
    while (!stopping) {
        struct pollfd p[3]; int n = 0, at_listen = -1, at_client = -1, at_child = -1;
        p[n] = (struct pollfd){lfd, POLLIN, 0}; at_listen = n++;
        if (cfd >= 0) { p[n] = (struct pollfd){cfd, POLLIN, 0}; at_client = n++; }
        if (child_fd >= 0) { p[n] = (struct pollfd){child_fd, POLLIN, 0}; at_child = n++; }
        int wait = 1000;
        if (slot.phase != COAX_SvPhase_idle) { int64_t left = slot.deadline - now_ms(); wait = left < 0 ? 0 : left > 1000 ? 1000 : (int)left; }
        if (ready) wait = 0; /* an in-process answer is waiting to be published */
        if (poll(p, (nfds_t)n, wait) < 0 && errno != EINTR) { perror("poll"); break; }
        coax_t_SvStep step = sv_tick(slot, now_ms());
        if (step.action == COAX_SvAction_none && ready) {
            ready = false;
            step = sv_done(slot, epoch, frame > 0, (coax_string){(const char *)s.frame.data, frame}, now_ms());
        } else if (step.action != COAX_SvAction_none) ready = false;
        if (step.action == COAX_SvAction_none && at_child >= 0 && (p[at_child].revents & (POLLIN | POLLHUP))) {
            ssize_t r = read(child_fd, frame < s.frame.count ? s.frame.data + frame : sink, frame < s.frame.count ? (size_t)(s.frame.count - frame) : sizeof sink);
            if (r > 0) { if (frame < s.frame.count) frame += r; else overflow = true; }
            else if (r == 0 || (errno != EINTR && errno != EAGAIN)) {
                int status = 0; waitpid(child, &status, 0); close(child_fd); child_fd = -1; child = -1;
                bool exited = WIFEXITED(status) && WEXITSTATUS(status) == 0 && !overflow;
                step = sv_done(slot, epoch, exited, (coax_string){(const char *)s.frame.data, frame}, now_ms());
            }
        }
        if (step.action == COAX_SvAction_none && at_client >= 0 && (p[at_client].revents & (POLLIN | POLLHUP | POLLERR))) {
            bool reading = slot.phase == COAX_SvPhase_reading;
            uint8_t *dst = reading ? s.rx.data + rx : sink; size_t room = reading ? (size_t)(s.rx.count - rx) : sizeof sink;
            ssize_t r = room ? read(cfd, dst, room) : 0;
            if (r == 0 || (r < 0 && errno != EINTR && errno != EAGAIN)) step = sv_closed(slot);
            else if (r > 0 && reading) {
                rx += r;
                coax_t_SpStep read = sp_bytes(slot, s.limits, s.engine, s.pools, (coax_string){(const char *)s.rx.data, rx}, (coax_v_u8){s.rx.data, rx}, s.scratch, now_ms());
                step = read.step;
                if (step.action == COAX_SvAction_decide) s.proto_out = read.proto_out; /* kept for this slot's answer */
            }
        }
        if (step.action == COAX_SvAction_none && (p[at_listen].revents & POLLIN)) {
            int fd = accept(lfd, NULL, NULL);
            if (fd >= 0) {
                fcntl(fd, F_SETFL, O_NONBLOCK);
                coax_t_SvStep a = sv_accept(slot, s.limits, now_ms());
                if (a.action == COAX_SvAction_read) { slot = a.slot; cfd = fd; rx = 0; accepted = now_ms(); }
                else { uint8_t busy[512]; int64_t m = sv_refusal((coax_v_u8){busy, sizeof busy}, a.code); finish(fd, busy, m, s.limits.read_ms); log_line((int)sv_status(a.code), a.code, 0); }
            }
            continue;
        }
        slot = step.slot;
        switch (step.action) {
        case COAX_SvAction_send_continue: { uint8_t c[64]; int64_t m = sv_continue((coax_v_u8){c, sizeof c}); write_all(cfd, c, m, now_ms() + s.limits.read_ms); break; }
        case COAX_SvAction_decide: {
#ifdef PSIONIC_SERVE_GPU
            if (s.gpu) {
                started = now_ms(); epoch = slot.epoch; s.stop_fd = cfd; s.stop_at = slot.deadline;
                frame = decide_gpu(&s, step.request, started);
                ready = frame >= 0; /* stopped: the next tick or read cancels it */
                break;
            }
#endif
            int pipefd[2]; if (pipe(pipefd)) { perror("pipe"); exit(2); }
            fflush(NULL); started = now_ms(); pid_t pid = fork();
            if (pid < 0) { perror("fork"); exit(2); }
            if (pid == 0) { close(pipefd[0]); close(lfd); close(cfd); decide(&s, step.request, started, pipefd[1]); }
            close(pipefd[1]); child = pid; child_fd = pipefd[0]; epoch = slot.epoch; frame = 0; overflow = false;
            break;
        }
        case COAX_SvAction_cancel_close: case COAX_SvAction_cancel_refuse: case COAX_SvAction_refuse:
            if (step.action != COAX_SvAction_refuse && child > 0) { kill(child, SIGKILL); waitpid(child, NULL, 0); close(child_fd); child = -1; child_fd = -1; }
            if (step.action == COAX_SvAction_cancel_close) { close(cfd); cfd = -1; fprintf(stderr, "psionic-serve: cancelled, client closed %lldms\n", (long long)(now_ms() - accepted)); break; }
            { uint8_t r[512]; int64_t m = sv_refusal((coax_v_u8){r, sizeof r}, step.code);
              finish(cfd, r, m, s.limits.read_ms); cfd = -1; log_line((int)sv_status(step.code), step.code, now_ms() - accepted); }
            break;
        case COAX_SvAction_answer:
            finish(cfd, s.frame.data + step.from, step.to - step.from, s.limits.read_ms); cfd = -1;
            log_line((int)sv_status(step.code), step.code, now_ms() - accepted); break;
        case COAX_SvAction_close: close(cfd); cfd = -1; fprintf(stderr, "psionic-serve: client closed before a request %lldms\n", (long long)(now_ms() - accepted)); break;
        default: break;
        }
    }
    if (child > 0) { kill(child, SIGKILL); waitpid(child, NULL, 0); }
    if (cfd >= 0) close(cfd);
    close(lfd);
    fprintf(stderr, "psionic-serve: stopped\n");
    return 0;
}
