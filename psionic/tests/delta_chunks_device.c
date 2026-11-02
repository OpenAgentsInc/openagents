/* Chapter 42 on a CUDA device: dkd_chunk (psionic/lib/device/delta-chunks-
   kernels.coax) inside chapter 44's linear-layer core (dl_conv, dl_gates,
   dl_qk, then dkd_chunk in place of dl_scan, then dl_gate_norm) at
   Elephant's linear-layer shape, against chapter 44's dl_scan path, the C
   build of the same kernel, chapter 42's CPU chunked profile and chapter
   11's recurrence, with chapter 34's warm-run statistics for the timings;
   with the original weights, linear layer 0 on a recorded prompt too.
   Firm reason: 5 (thin harness: the CUDA driver, file reads and byte moves
   are host effects; admission, launch checks, the tolerance verdict, the
   statistics, the projections and every kernel are Coax).
   Usage: delta-chunks-device KERNELS.cu BUILD [MODEL_SAFETENSORS]
   Prints SKIP without a CUDA device or NVRTC. */
#define _POSIX_C_SOURCE 200809L
#include "delta-chunks-device-runtime.h"
#include "comparator.h"
#include "cuda-host.h"
#include "elephant-request.h"
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
#define RUNS 10
enum { KH = 16, H = 32, C = 8192, V = 4096, STATE = H * 16384, D = 4096 };
static const int64_t CHUNKS[] = {2, 4, 8, 16};
enum { NCHUNKS = 4 };

static char *read_all(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *t = malloc((size_t)size + 1);
    if (!t || fread(t, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); t[size] = 0; *n = (size_t)size; return t;
}
static uint64_t seed = 0x9e3779b97f4a7c15ULL;
static uint64_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return seed; }
static float uniform(void) { return (float)((next() >> 11) % 2000001) / 1000000.0f - 1.0f; }
static uint64_t fnv(uint64_t h, const void *p, size_t n) { const unsigned char *b = p; for (size_t i = 0; i < n; i++) { h ^= b[i]; h *= 0x100000001b3ULL; } return h; }
static uint64_t fnv_str(const char *s) { return fnv(0xcbf29ce484222325ULL, s, strlen(s)); }
static coax_v_f32 fv(float *p, int64_t n) { return (coax_v_f32){p, n}; }

static PcArg view(PcBuffer b, int64_t count) { return (PcArg){PC_ARG_VIEW, 0, 0, b, count}; }
static PcArg integer(int64_t v) { return (PcArg){PC_ARG_INT, v, 0, 0, 0}; }
static PcArg real(float v) { PcArg a = {PC_ARG_F32, 0, 0, 0, 0}; a.f = v; return a; }
static PcBuffer dev(size_t bytes) { PcBuffer b; TRY(pc_alloc(bytes ? bytes : 4, &b)); return b; }
static double since(PcEvent a, PcEvent b) { double t = 0; TRY(pc_event_seconds(a, b, &t)); return t; }

typedef struct { PcKernel conv, gates, qk, scan, chunk, norm; PcEvent a, b; } K;
typedef struct {
    int64_t T;
    PcBuffer mixed, weight, a, b, alog, dt, z, gamma, conv, q, k, beta, decay, draft, out, state;
} Run;

static void setup(Run *r, int64_t T) {
    r->T = T;
    r->mixed = dev((size_t)T * C * 4); r->weight = dev(4 * C * 4); r->a = dev((size_t)T * H * 4); r->b = dev((size_t)T * H * 4);
    r->alog = dev(H * 4); r->dt = dev(H * 4); r->z = dev((size_t)T * V * 4); r->gamma = dev(128 * 4);
    r->conv = dev((size_t)T * C * 4); r->q = dev((size_t)T * KH * 128 * 4); r->k = dev((size_t)T * KH * 128 * 4);
    r->beta = dev((size_t)T * H * 4); r->decay = dev((size_t)T * H * 4); r->draft = dev((size_t)T * V * 4);
    r->out = dev((size_t)T * V * 4); r->state = dev((size_t)STATE * 4);
}
static void teardown(Run *r) {
    PcBuffer b[] = {r->mixed, r->weight, r->a, r->b, r->alog, r->dt, r->z, r->gamma, r->conv, r->q, r->k, r->beta, r->decay, r->draft, r->out, r->state};
    for (size_t i = 0; i < sizeof b / sizeof *b; i++) pc_free(b[i]);
}
/* Upload chapter 11's packed rows, unpacked into the device layouts. */
static void upload(Run *r, float *rows) {
    int64_t T = r->T;
    float *mixed = malloc((size_t)T * C * 4), *weight = malloc(4 * C * 4), *a = malloc((size_t)T * H * 4), *b = malloc((size_t)T * H * 4);
    float alog[H], dt[H], gamma[128], *z = malloc((size_t)T * V * 4);
    dkr_unpack(KH, H, T, fv(rows, T * dkr_input_count(KH, H)), fv(mixed, T * C), fv(weight, 4 * C), fv(a, T * H), fv(b, T * H),
               fv(alog, H), fv(dt, H), fv(z, T * V), fv(gamma, 128));
    TRY(pc_upload(r->mixed, mixed, (size_t)T * C * 4, NULL)); TRY(pc_upload(r->weight, weight, 4 * C * 4, NULL));
    TRY(pc_upload(r->a, a, (size_t)T * H * 4, NULL)); TRY(pc_upload(r->b, b, (size_t)T * H * 4, NULL));
    TRY(pc_upload(r->alog, alog, sizeof alog, NULL)); TRY(pc_upload(r->dt, dt, sizeof dt, NULL));
    TRY(pc_upload(r->z, z, (size_t)T * V * 4, NULL)); TRY(pc_upload(r->gamma, gamma, sizeof gamma, NULL));
    free(mixed); free(weight); free(a); free(b); free(z);
}
/* Chapter 44's convolution, gates and normalized queries and keys. */
static void front(K *k, Run *r) {
    int64_t T = r->T;
    PcArg c[] = {integer(C), integer(T), view(r->mixed, T * C), view(r->weight, 4 * C), view(r->conv, T * C)};
    PcArg g[] = {integer(H), view(r->a, T * H), view(r->b, T * H), view(r->alog, H), view(r->dt, H), view(r->beta, T * H), view(r->decay, T * H)};
    PcArg q[] = {integer(KH), integer(128), integer(C), view(r->conv, T * C), view(r->q, T * KH * 128), view(r->k, T * KH * 128)};
    TRY(pc_launch(k->conv, C, 128, c, 5, NULL));
    TRY(pc_launch(k->gates, T * H, 128, g, 7, NULL));
    TRY(pc_launch(k->qk, T * KH, 128, q, 6, NULL));
    TRY(pc_sync());
}
/* The scan alone, timed: dl_scan (chunk 0) or dkd_chunk from the zero state. */
static double scan(K *k, Run *r, int64_t chunk) {
    int64_t T = r->T;
    TRY(pc_fill(r->state, 0, STATE, NULL));
    TRY(pc_record(k->a, NULL));
    if (chunk == 0) {
        PcArg s[] = {integer(T), integer(H), integer(KH), integer(C), view(r->conv, T * C), view(r->q, T * KH * 128), view(r->k, T * KH * 128),
                     view(r->beta, T * H), view(r->decay, T * H), view(r->draft, T * V)};
        TRY(pc_launch(k->scan, H * 128, 128, s, 10, NULL));
    } else {
        coax_t_DkStatus st = dkp_check(T, chunk, H, KH, T * C, T * KH * 128, T * KH * 128, T * H, T * H, T * V, STATE);
        if (st != COAX_DkStatus_ok) { printf("FAIL: dkp_check refused an Elephant launch (%d)\n", (int)st); exit(1); }
        PcArg s[] = {integer(T), integer(chunk), integer(H), integer(KH), integer(C), view(r->conv, T * C), view(r->q, T * KH * 128),
                     view(r->k, T * KH * 128), view(r->beta, T * H), view(r->decay, T * H), view(r->draft, T * V), view(r->state, STATE)};
        TRY(pc_launch_warps(k->chunk, dkp_groups(H) * 4, 128, s, 12, NULL));
    }
    TRY(pc_record(k->b, NULL)); TRY(pc_sync());
    return since(k->a, k->b);
}
static void gate_norm(K *k, Run *r, float *host) {
    int64_t T = r->T;
    PcArg n[] = {real(0.000001f), view(r->draft, T * V), view(r->gamma, 128), view(r->z, T * V), view(r->out, T * V)};
    TRY(pc_launch(k->norm, T * H, 128, n, 5, NULL)); TRY(pc_sync());
    TRY(pc_download(host, r->out, (size_t)T * V * 4, NULL));
}

static int failures = 0;
static void verdict(const char *what, const float *a, const float *b, int64_t n) {
    int64_t at = dkp_worst(fv((float *)a, n), fv((float *)b, n));
    double worst = fabs((double)a[at] - b[at]);
    int64_t same = 0; for (int64_t i = 0; i < n; i++) same += memcmp(&a[i], &b[i], 4) == 0;
    int ok = dkp_within(worst, fv((float *)b, n));
    printf("  %s: largest difference %.3g, %lld of %lld bit-identical, %s\n", what, worst, (long long)same, (long long)n, ok ? "within tolerance" : "OUTSIDE TOLERANCE");
    failures += !ok;
}
static void stats(const char *name, int64_t *ns, coax_t_CmStats *st) {
    *st = cm_two_pass((coax_v_s64){ns, RUNS});
    int64_t lo = ns[0]; for (int i = 1; i < RUNS; i++) if (ns[i] < lo) lo = ns[i];
    printf("  %s: mean %.3f ms, sd %.3f ms, min %.3f ms (%d warm runs)\n", name, st->mean / 1e6, st->sd / 1e6, lo / 1e6, RUNS);
}

/* One prompt: every chunk size against dl_scan, the C build, the CPU
   chunked profile and chapter 11's recurrence; timings when asked. Returns
   the mean time of each chunk size (index 0: dl_scan). */
static void compare(K *k, Run *r, float *rows, uint64_t machine, uint64_t build, int timed, int cpu_refs, double *means) {
    int64_t T = r->T;
    float *old = malloc((size_t)T * V * 4), *mine = malloc((size_t)T * V * 4), *state = malloc((size_t)STATE * 4);
    front(k, r);
    scan(k, r, 0); gate_norm(k, r, old);
    int64_t ns[NCHUNKS + 1][RUNS];
    coax_t_CmStats st[NCHUNKS + 1];
    if (timed) {
        for (int i = 0; i < RUNS; i++) for (int c = 0; c <= NCHUNKS; c++) ns[c][i] = llround(scan(k, r, c ? CHUNKS[c - 1] : 0) * 1e9);
        stats("dl_scan (chapter 44's recurrence, one item per value column)", ns[0], &st[0]);
        means[0] = st[0].mean;
    }
    /* Chapter 11's recurrence and chapter 42's CPU chunked profile. */
    float *rec = NULL, *rec_state = NULL, *cpu = NULL;
    if (cpu_refs) {
        rec = malloc((size_t)T * V * 4); rec_state = malloc((size_t)STATE * 4); cpu = malloc((size_t)T * V * 4);
        float *history = malloc(3 * C * 4), *work = malloc((size_t)dkr_step_work(KH, H) * 4), *cpu_state = malloc((size_t)STATE * 4);
        double *cw = malloc((size_t)dkr_work_count(KH, H, 64) * 8);
        int64_t rs = dkr_recurrence(KH, H, T, fv(rows, T * dkr_input_count(KH, H)), fv(rec, T * V), fv(history, 3 * C), fv(rec_state, STATE), fv(work, dkr_step_work(KH, H)));
        int64_t cs = dkr_chunked(KH, H, T, 64, fv(rows, T * dkr_input_count(KH, H)), fv(cpu, T * V), fv(history, 3 * C), fv(cpu_state, STATE),
                                 (coax_v_f64){cw, dkr_work_count(KH, H, 64)});
        if (rs || cs) { printf("FAIL: CPU references refused (%lld, %lld)\n", (long long)rs, (long long)cs); exit(1); }
        free(history); free(work); free(cpu_state); free(cw);
        verdict("dl_scan path against chapter 11's recurrence", old, rec, T * V);
    }
    for (int c = 0; c < NCHUNKS; c++) {
        int64_t chunk = CHUNKS[c];
        scan(k, r, chunk); gate_norm(k, r, mine);
        TRY(pc_download(state, r->state, (size_t)STATE * 4, NULL));
        printf(" chunk %lld:\n", (long long)chunk);
        verdict("against the dl_scan path", mine, old, T * V);
        if (cpu_refs) {
            verdict("against chapter 42's CPU chunked profile (chunk 64)", mine, cpu, T * V);
            verdict("against chapter 11's recurrence", mine, rec, T * V);
            verdict("carried state against chapter 11's", state, rec_state, STATE);
        }
        if (timed) { stats("dkd_chunk", ns[c + 1], &st[c + 1]); means[c + 1] = st[c + 1].mean; }
    }
    /* The C build of dkd_chunk on value heads 0 and 31, chunk 16: the
       device's draft against it. */
    {
        float *conv = malloc((size_t)T * C * 4), *q = malloc((size_t)T * KH * 512), *kk = malloc((size_t)T * KH * 512);
        float *beta = malloc((size_t)T * H * 4), *decay = malloc((size_t)T * H * 4), *draft = malloc((size_t)T * V * 4), *cdraft = calloc((size_t)T * V, 4);
        float *cstate = calloc(STATE, 4), *ga = malloc((size_t)T * 256 * 4), *gb = malloc((size_t)T * 256 * 4);
        scan(k, r, 16);
        TRY(pc_download(conv, r->conv, (size_t)T * C * 4, NULL)); TRY(pc_download(q, r->q, (size_t)T * KH * 512, NULL));
        TRY(pc_download(kk, r->k, (size_t)T * KH * 512, NULL)); TRY(pc_download(beta, r->beta, (size_t)T * H * 4, NULL));
        TRY(pc_download(decay, r->decay, (size_t)T * H * 4, NULL)); TRY(pc_download(draft, r->draft, (size_t)T * V * 4, NULL));
        int64_t heads[2] = {0, H - 1};
        for (int i = 0; i < 2; i++) for (int g = 0; g < 8; g++)
            dkd_chunk(heads[i] * 8 + g, T, 16, H, KH, C, fv(conv, T * C), fv(q, T * KH * 128), fv(kk, T * KH * 128), fv(beta, T * H), fv(decay, T * H),
                      fv(cdraft, T * V), fv(cstate, STATE));
        for (int64_t t = 0; t < T; t++) for (int i = 0; i < 2; i++) {
            memcpy(ga + (t * 2 + i) * 128, draft + (t * H + heads[i]) * 128, 512); memcpy(gb + (t * 2 + i) * 128, cdraft + (t * H + heads[i]) * 128, 512);
        }
        verdict("chunk 16, heads 0 and 31: device draft against the C build", ga, gb, T * 256);
        free(conv); free(q); free(kk); free(beta); free(decay); free(draft); free(cdraft); free(cstate); free(ga); free(gb);
    }
    if (timed) {
        int best = 1; for (int c = 2; c <= NCHUNKS; c++) if (st[c].mean < st[best].mean) best = c;
        coax_t_CmWork work_id = {fnv(fnv_str("delta-chunks-prefill"), &r->T, 8), COAX_CmFormat_f32, COAX_CmFormat_f32, COAX_CmFormat_f32, V, 128, T};
        coax_t_CmSide a = {work_id, fnv_str("psionic-dkd_chunk"), build, machine, 1}, b = {work_id, fnv_str("psionic-dl_scan"), build, machine, 1};
        coax_t_CmCall call = cm_compare(a, st[best], b, st[0]);
        static const char *verdicts[] = {"refused", "faster", "slower", "same", "noisy"};
        printf("  best chunk %lld; chapter 34 verdict: dkd_chunk is %s than dl_scan (time ratio dl_scan/dkd_chunk %.2f, standard error %.3f ms)\n",
               (long long)CHUNKS[best - 1], verdicts[call.verdict], call.ratio, call.se / 1e6);
    }
    free(old); free(mine); free(state); free(rec); free(rec_state); free(cpu);
}

int main(int argc, char **argv) {
    if (argc != 3 && argc != 4) { fprintf(stderr, "usage: delta-chunks-device KERNELS.cu BUILD [MODEL_SAFETENSORS]\n"); return 2; }
    PcDeviceInfo info;
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    coax_t_DaStatus adm = dkp_admit(info.major, info.minor, info.max_threads, info.max_shared);
    printf("device: %s (sm_%d%d); dkd_chunk (128 threads, 26816 shared bytes): %s\n", info.name, info.major, info.minor, adm == COAX_DaStatus_ok ? "admitted" : "refused");
    if (adm != COAX_DaStatus_ok) return 1;
    char host[256] = "unknown"; gethostname(host, sizeof host - 1);
    uint64_t machine = fnv(fnv_str(host), info.name, strlen(info.name)), build = fnv_str(argv[2]);
    K k; size_t n; double secs; PcModule m;
    char *src = read_all(argv[1], &n); TRY(pc_compile(src, n, "delta-chunks.cu", &m, &secs)); free(src);
    TRY(pc_kernel(m, "dl_conv", &k.conv)); TRY(pc_kernel(m, "dl_gates", &k.gates)); TRY(pc_kernel(m, "dl_qk", &k.qk));
    TRY(pc_kernel(m, "dl_scan", &k.scan)); TRY(pc_kernel(m, "dkd_chunk", &k.chunk)); TRY(pc_kernel(m, "dl_gate_norm", &k.norm));
    TRY(pc_event(&k.a)); TRY(pc_event(&k.b));
    int64_t M = dkr_input_count(KH, H);

    printf("synthetic packed inputs at Elephant's linear-layer shape: 16 key heads, 32 value heads, width 128, log A in [-4, 0]\n");
    int64_t lengths[] = {64, 256, 1024, 2048};
    double table[4][NCHUNKS + 1];
    for (int li = 0; li < 4; li++) {
        int64_t T = lengths[li];
        float *rows = malloc((size_t)T * M * 4);
        for (int64_t t = 0; t < T; t++) {
            float *row = rows + t * M;
            for (int64_t i = 0; i < C; i++) row[i] = 2.0f * uniform();
            for (int64_t i = 0; i < 4 * C; i++) row[C + i] = t ? rows[C + i] : 0.5f * uniform();
            for (int64_t i = 5 * C; i < M; i++) row[i] = uniform();
        }
        /* A_log and dt_bias are per layer: token 0's; gamma near 1. */
        for (int64_t t = 0; t < T; t++) for (int i = 0; i < H; i++) {
            rows[t * M + 5 * C + 2 * H + i] = -2.0f + 2.0f * (float)((i * 7) % 11) / 10.0f * (i % 2 ? 1 : -1) / 1.0f;
            rows[t * M + 5 * C + 3 * H + i] = rows[5 * C + 3 * H + i];
        }
        for (int i = 0; i < 128; i++) rows[5 * C + 4 * H + V + i] = 1.0f + 0.25f * rows[5 * C + 4 * H + V + i];
        for (int64_t t = 1; t < T; t++) for (int i = 0; i < 128; i++) rows[t * M + 5 * C + 4 * H + V + i] = rows[5 * C + 4 * H + V + i];
        Run r; setup(&r, T); upload(&r, rows);
        printf("%lld tokens:\n", (long long)T);
        compare(&k, &r, rows, machine, build, 1, 1, table[li]);
        teardown(&r); free(rows);
    }
    printf("mean scan times (ms): tokens, dl_scan");
    for (int c = 0; c < NCHUNKS; c++) printf(", chunk %lld", (long long)CHUNKS[c]);
    printf("\n");
    for (int li = 0; li < 4; li++) {
        printf("  %lld", (long long)lengths[li]);
        for (int c = 0; c <= NCHUNKS; c++) printf(", %.3f", table[li][c] / 1e6);
        printf("\n");
    }

    if (argc == 4) {
        /* Linear layer 0 of the original Elephant over the recorded decision
           prompt: embedding, input norm and projections on the CPU in Coax,
           then the layer core on the device. */
        int fd = open(argv[3], O_RDONLY); if (fd < 0) { perror("open"); return 2; }
        struct stat sb; if (fstat(fd, &sb) || sb.st_size != 18819722392LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
        void *map = mmap(NULL, (size_t)sb.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
        if (map == MAP_FAILED) { perror("mmap"); return 2; }
        coax_t_P5Tensor *inv = calloc(4096, sizeof *inv), *draft = calloc(4096, sizeof *draft);
        coax_v_u8 bytes = {map, sb.st_size};
        coax_t_PwResult scanned = p5_inspect_checked(bytes, (coax_v_t_P5Tensor){inv, 4096}, (coax_v_t_P5Tensor){draft, 4096}, 0);
        if (scanned.status || scanned.written != 760) { fprintf(stderr, "inventory %u\n", scanned.status); return 1; }
        coax_t_PaFile file = {.data = bytes}; coax_v_t_PaFile files = {&file, 1}; coax_v_t_P5Tensor inventory = {inv, scanned.written};
        coax_t_PdShape delta = {KH, H, 128, 128, 0.000001f}; coax_t_QfShape attention = {16, 4, 256, 64, 0.000001f};
        coax_t_QeShape shape = {.linear = {D, 12288, delta, COAX_QmDtype_bf16}, .full = {D, 12288, 1, attention, COAX_QmDtype_bf16},
                                .state = {24, 8, 1, delta, attention}};
        coax_t_P5Shape matrix = {2, {{248320, D, 0, 0, 0}}};
        coax_t_PaView embedding = pa_view((coax_string){"model.language_model.embed_tokens.weight", 40}, matrix, COAX_PwDtype_bf16, inventory, files);
        if (embedding.status) { fprintf(stderr, "embedding mapping failed\n"); return 1; }
        float *params = calloc(50000, 4), *temporary = calloc(50000, 4);
        coax_t_PmResult bound = pm_layer(shape, 0, inventory, files, fv(params, 50000), fv(temporary, 50000), 0);
        if (bound.status || bound.layer.kind != 0) { fprintf(stderr, "layer 0 mapping %u\n", bound.status); return 1; }
        int64_t T = (int64_t)(sizeof elephant_request_tokens / sizeof *elephant_request_tokens);
        float *x = malloc((size_t)T * D * 4), *rows = malloc((size_t)T * M * 4), *work = malloc((size_t)dkr_pack_work(shape.linear) * 4);
        coax_t_EsResult e = es_embed(embedding.data, D, (coax_v_s64){(int64_t *)elephant_request_tokens, T}, fv(x, T * D), fv(temporary, D));
        if (e.status) { fprintf(stderr, "embed status %u\n", e.status); return 1; }
        int64_t p = dkr_pack(shape.linear, bound.layer.linear, fv(x, T * D), T, fv(rows, T * M), fv(work, dkr_pack_work(shape.linear)));
        if (p) { printf("FAIL: projections refused (%lld)\n", (long long)p); return 1; }
        printf("original Elephant linear layer 0 over the %lld-token recorded decision prompt:\n", (long long)T);
        double means[NCHUNKS + 1];
        Run r; setup(&r, T); upload(&r, rows);
        compare(&k, &r, rows, machine, build, 1, 1, means);
        teardown(&r); free(x); free(rows); free(work); free(params); free(temporary); free(inv); free(draft);
        munmap(map, (size_t)sb.st_size);
    }
    if (failures) { printf("FAIL: %d comparisons outside the frozen tolerance\n", failures); return 1; }
    printf("chapter 42 on CUDA: every comparison within the frozen tolerance\n");
    return 0;
}
