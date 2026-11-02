/* Chapter 41 on a CUDA device: td_attend (psionic/lib/device/tiled-attention-
   kernels.coax) at gpt-oss-20b's attention shapes against chapter 66/67's
   prefill attention kernels (psionic/lib/gptoss/device-kernels.coax), the C
   build of the same kernel, chapter 41's CPU profile and a binary64 dense
   oracle, with chapter 34's warm-run statistics for the timings.
   Firm reason: 5 (thin harness: the CUDA driver, file reads and byte moves
   are host effects; admission, launch checks, the tolerance verdict, the
   statistics and every kernel are Coax).
   Usage: tiled-attention-device TD.cu GK.cu BUILD [MODEL OFFSETS... TOKENS]
   MODEL is gpt-oss-20b's original/model.safetensors and OFFSETS the absolute
   byte offsets of embedding.weight, block.0.attn.norm.scale,
   block.0.attn.qkv.weight, block.0.attn.qkv.bias and block.0.attn.sinks.
   Prints SKIP without a CUDA device or NVRTC. */
#define _POSIX_C_SOURCE 200809L
#include "tiled-attention-plan.h"
#include "comparator.h"
#include "cuda-host.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
#define RUNS 10
enum { HEADS = 64, KV = 8, QW = 5120, KVW = 1024, OUTW = 4096, CHUNK = 128 };

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

static PcArg view(PcBuffer b, size_t offset_bytes, int64_t count) { return (PcArg){PC_ARG_VIEW, 0, 0, b + offset_bytes, count}; }
static PcArg integer(int64_t v) { return (PcArg){PC_ARG_INT, v, 0, 0, 0}; }
static PcBuffer dev(size_t bytes) { PcBuffer b; TRY(pc_alloc(bytes ? bytes : 4, &b)); return b; }
static double since(PcEvent a, PcEvent b) { double t = 0; TRY(pc_event_seconds(a, b, &t)); return t; }

typedef struct {
    PcKernel td, sinks, scores, softmax, context, finish, embed, norm_part, norm, bf_part, rotate;
    PcEvent a, b;
} K;

/* One prefill's attention: P rows over P keys, q rows inside qkv (stride
   5120), keys then values in kv rows of 1024. */
typedef struct {
    int64_t P, kind, window;
    PcBuffer qkv, kv, sinks_bf, sinks_f, out_tiled, out_old, state, tiles, scores, partial, flags, ring;
    float *kv_host;
} Run;

/* td_attend over the whole prompt in one launch. Seconds. */
static double tiled(K *k, Run *r) {
    int64_t P = r->P, groups = (P + 63) / 64 * HEADS;
    coax_t_TaStatus st = tdp_check(HEADS, KV, 0, P, P, r->kind, r->window, 0, QW, KVW, 512,
                                   (P - 1) * QW + OUTW, (P - 1) * KVW + KVW, HEADS, P * OUTW, 2 * P * HEADS, groups);
    if (st != COAX_TaStatus_ok) { printf("FAIL: tdp_check refused a gpt-oss launch (%d)\n", (int)st); exit(1); }
    PcArg args[] = {integer(HEADS), integer(KV), integer(0), integer(P), integer(P), integer(r->kind), integer(r->window), integer(0),
                    integer(QW), integer(KVW), integer(512), view(r->qkv, 0, (P - 1) * QW + OUTW), view(r->kv, 0, (P - 1) * KVW + KVW),
                    view(r->sinks_f, 0, HEADS), view(r->out_tiled, 0, P * OUTW), view(r->state, 0, 2 * P * HEADS), view(r->tiles, 0, groups)};
    TRY(pc_record(k->a, NULL));
    TRY(pc_launch_warps(k->td, groups * 2, 64, args, 17, NULL));
    TRY(pc_record(k->b, NULL)); TRY(pc_sync());
    return since(k->a, k->b);
}
/* Chapter 67's prefill attention, as its program runs it: chunks of 128
   rows; scores, softmax, eight-lane context sums and their f64 finish. A
   windowed layer keeps a 128-slot ring cache; a full layer a cache of 2048
   (the specialized kernels) or 4096 positions (the generic ones). Seconds
   of kernel time, ring uploads excluded. */
static double existing(K *k, Run *r) {
    int64_t P = r->P, window = r->kind == 1 ? r->window : 0;
    int64_t cap = window ? 128 : (P <= 2048 ? 2048 : 4096);
    double total = 0;
    for (int64_t s = 0; s < P; s += CHUNK) {
        int64_t n = P - s < CHUNK ? P - s : CHUNK, stride = s + n;
        if (window && stride > window) stride = window;
        PcBuffer cache = r->kv; int64_t cache_n = P * KVW;
        if (window) {
            for (int64_t t = s - 128 < 0 ? 0 : s - 128; t < s; t++)
                TRY(pc_upload(r->ring + (size_t)(t % 128) * KVW * 4, r->kv_host + t * KVW, KVW * 4, NULL));
            cache = r->ring; cache_n = 128 * KVW;
        }
        PcArg qkv = view(r->qkv, (size_t)s * QW * 4, n * QW), pend = view(r->kv, (size_t)s * KVW * 4, n * KVW), c = view(cache, 0, cache_n);
        PcArg sc = view(r->scores, 0, n * HEADS * stride), part = view(r->partial, 0, 8 * n * OUTW), fl = view(r->flags, 0, n * OUTW);
        PcArg a1[] = {integer(HEADS), integer(KV), integer(s), integer(window), integer(cap), integer(stride), qkv, c, pend, sc};
        PcArg a2[] = {integer(HEADS), integer(s), integer(window), integer(stride), view(r->sinks_bf, 0, HEADS), sc, fl};
        PcArg a3[] = {integer(HEADS), integer(KV), integer(n), integer(s), integer(window), integer(cap), integer(stride), integer(8), sc, c, pend, part};
        PcArg a4[] = {integer(OUTW), integer(n), integer(8), part, view(r->sinks_bf, 0, 0), view(r->out_old, (size_t)s * OUTW * 4, n * OUTW), fl};
        TRY(pc_record(k->a, NULL));
        TRY(pc_launch(k->scores, n * HEADS * stride, 256, a1, 10, NULL));
        TRY(pc_launch(k->softmax, n * HEADS, 256, a2, 7, NULL));
        TRY(pc_launch(k->context, n * OUTW * 8, 256, a3, 12, NULL));
        TRY(pc_launch(k->finish, n * OUTW, 256, a4, 7, NULL));
        TRY(pc_record(k->b, NULL)); TRY(pc_sync());
        total += since(k->a, k->b);
    }
    return total;
}

static int failures = 0;
static void verdict(const char *what, const float *a, const float *b, int64_t n, const float *values, int64_t nv) {
    int64_t at = tdp_worst((coax_v_f32){(float *)a, n}, (coax_v_f32){(float *)b, n});
    double worst = fabs((double)a[at] - b[at]);
    int64_t same = 0; for (int64_t i = 0; i < n; i++) same += memcmp(&a[i], &b[i], 4) == 0;
    int ok = tdp_within(worst, (coax_v_f32){(float *)values, nv});
    printf("  %s: largest difference %.3g, %lld of %lld bit-identical, %s\n", what, worst, (long long)same, (long long)n, ok ? "within tolerance" : "OUTSIDE TOLERANCE");
    failures += !ok;
}
static void verdict64(const char *what, const float *a, const double *b, int64_t n, const float *values, int64_t nv) {
    int64_t at = tdp_worst64((coax_v_f32){(float *)a, n}, (coax_v_f64){(double *)b, n});
    double worst = fabs((double)a[at] - b[at]);
    int ok = tdp_within(worst, (coax_v_f32){(float *)values, nv});
    printf("  %s: largest difference %.3g, %s\n", what, worst, ok ? "within tolerance" : "OUTSIDE TOLERANCE");
    failures += !ok;
}
static void stats(const char *name, int64_t *ns, coax_t_CmStats *st) {
    *st = cm_two_pass((coax_v_s64){ns, RUNS});
    int64_t lo = ns[0], hi = ns[0];
    for (int i = 1; i < RUNS; i++) { if (ns[i] < lo) lo = ns[i]; if (ns[i] > hi) hi = ns[i]; }
    printf("  %s: mean %.3f ms, sd %.3f ms, min %.3f, max %.3f (%d warm runs)\n", name, st->mean / 1e6, st->sd / 1e6, lo / 1e6, hi / 1e6, RUNS);
}

/* Compare one prefill: device tiled against the existing kernels (every
   output), the C build of td_attend (two groups per key/value head), the
   CPU profile and the binary64 oracle (the last 64 rows). */
static void compare(K *k, Run *r, uint64_t machine, uint64_t build, int timed) {
    int64_t P = r->P, groups = (P + 63) / 64 * HEADS;
    float *qkv = malloc((size_t)P * QW * 4), *tiled_h = malloc((size_t)P * OUTW * 4), *old_h = malloc((size_t)P * OUTW * 4);
    float *values = malloc((size_t)P * 512 * 4), sinks[HEADS];
    TRY(pc_download(qkv, r->qkv, (size_t)P * QW * 4, NULL)); TRY(pc_download(sinks, r->sinks_f, sizeof sinks, NULL));
    for (int64_t t = 0; t < P; t++) memcpy(values + t * 512, r->kv_host + t * KVW + 512, 512 * 4);
    int64_t ns[2][RUNS];
    double t_tiled = tiled(k, r), t_old = existing(k, r);
    if (timed) for (int i = 0; i < RUNS; i++) { ns[0][i] = llround(tiled(k, r) * 1e9); ns[1][i] = llround(existing(k, r) * 1e9); }
    TRY(pc_download(tiled_h, r->out_tiled, (size_t)P * OUTW * 4, NULL)); TRY(pc_download(old_h, r->out_old, (size_t)P * OUTW * 4, NULL));
    int64_t *tiles = malloc((size_t)groups * 8); TRY(pc_download(tiles, r->tiles, (size_t)groups * 8, NULL));
    int64_t loads = 0; for (int64_t g = 0; g < groups; g++) loads += tiles[g];
    printf("%lld tokens, %s: %lld groups of 64 threads, %lld tile loads\n", (long long)P,
           r->kind == 1 ? "window 128" : "causal", (long long)groups, (long long)loads);
    verdict("tiled kernel against the chapter 66/67 kernels", tiled_h, old_h, P * OUTW, values, P * 512);
    /* The C build of the same source, on 16 groups: the first and last query
       blocks of one head per key/value head. */
    float *cpu = malloc((size_t)P * OUTW * 4), *state = malloc((size_t)P * HEADS * 8);
    float *ga = malloc(16 * 64 * 64 * 4), *gb = malloc(16 * 64 * 64 * 4);
    int64_t blocks = (P + 63) / 64, sampled = 0, m = 0;
    for (int64_t b = 0; b < blocks; b += blocks - 1) {
        for (int64_t h = 0; h < HEADS; h += HEADS / KV) {
            td_attend(b * HEADS + h, HEADS, KV, 0, P, P, r->kind, r->window, 0, QW, KVW, 512, (coax_v_f32){qkv, (P - 1) * QW + OUTW},
                      (coax_v_f32){r->kv_host, P * KVW}, (coax_v_f32){sinks, HEADS}, (coax_v_f32){cpu, P * OUTW},
                      (coax_v_f32){state, 2 * P * HEADS}, (coax_v_s64){tiles, groups});
            for (int64_t row = b * 64; row < b * 64 + 64 && row < P; row++) {
                memcpy(ga + m * 64, tiled_h + row * OUTW + h * 64, 256); memcpy(gb + m * 64, cpu + row * OUTW + h * 64, 256); m++;
            }
            sampled++;
        }
        if (blocks == 1) break;
    }
    char what[96]; snprintf(what, sizeof what, "tiled kernel against its C build (%lld groups)", (long long)sampled);
    verdict(what, ga, gb, m * 64, values, P * 512);
    free(ga); free(gb);
    /* The last 64 rows against the CPU profile and the oracle. */
    int64_t r0 = P - 64;
    float *packed = malloc((size_t)tdp_cpu_packed(HEADS, KV, P) * 4), *prof = malloc(64 * OUTW * 4);
    double *work = malloc((size_t)tdp_cpu_work(HEADS, KV, P) * 8), *dense = malloc(64 * OUTW * 8);
    coax_v_f32 qv = {qkv + r0 * QW, 63 * QW + OUTW}, kvv = {r->kv_host, P * KVW}, sv = {sinks, HEADS};
    coax_t_TaStatus st = tdp_cpu(HEADS, KV, r0, 64, P, r->kind, r->window, 0, QW, KVW, 512, qv, kvv, sv, (coax_v_f32){prof, 64 * OUTW},
                                 (coax_v_f32){packed, tdp_cpu_packed(HEADS, KV, P)}, (coax_v_f64){work, tdp_cpu_work(HEADS, KV, P)});
    if (st != COAX_TaStatus_ok) { printf("FAIL: the CPU profile refused (%d)\n", (int)st); exit(1); }
    tdp_dense(HEADS, KV, r0, 64, P, r->kind, r->window, 0, QW, KVW, 512, qv, kvv, sv, (coax_v_f64){dense, 64 * OUTW});
    verdict("last 64 rows against chapter 41's CPU profile", tiled_h + r0 * OUTW, prof, 64 * OUTW, values, P * 512);
    verdict64("last 64 rows against the binary64 oracle", tiled_h + r0 * OUTW, dense, 64 * OUTW, values, P * 512);
    verdict64("chapter 66/67 kernels' last 64 rows against the oracle", old_h + r0 * OUTW, dense, 64 * OUTW, values, P * 512);
    if (timed) {
        coax_t_CmStats st_t, st_o;
        stats("tiled kernel, one launch", ns[0], &st_t);
        stats(P <= 2048 || r->kind == 1 ? "chapter 66/67 kernels, chunks of 128" : "chapter 66/67 generic kernels (cache 4096), chunks of 128", ns[1], &st_o);
        coax_t_CmWork work_id = {fnv(fnv_str("tiled-attention-prefill"), &r->P, 8) ^ (uint64_t)r->kind, COAX_CmFormat_f32, COAX_CmFormat_f32, COAX_CmFormat_f32, OUTW, P, P};
        coax_t_CmSide a = {work_id, fnv_str("psionic-td_attend"), build, machine, 1}, b = {work_id, fnv_str("psionic-gk_batch-attention"), build, machine, 1};
        coax_t_CmCall c = cm_compare(a, st_t, b, st_o);
        static const char *verdicts[] = {"refused", "faster", "slower", "same", "noisy"};
        printf("  chapter 34 verdict: the tiled kernel is %s (time ratio old/tiled %.2f, standard error %.3f ms)\n", verdicts[c.verdict], c.ratio, c.se / 1e6);
    } else {
        printf("  single runs: tiled %.3f ms, chapter 66/67 kernels %.3f ms\n", t_tiled * 1e3, t_old * 1e3);
    }
    free(qkv); free(tiled_h); free(old_h); free(values); free(tiles); free(cpu); free(state); free(packed); free(prof); free(work); free(dense);
}

static void setup(Run *r, int64_t P) {
    r->P = P;
    r->qkv = dev((size_t)P * QW * 4); r->kv = dev((size_t)P * KVW * 4);
    r->sinks_bf = dev(HEADS * 2); r->sinks_f = dev(HEADS * 4);
    r->out_tiled = dev((size_t)P * OUTW * 4); r->out_old = dev((size_t)P * OUTW * 4);
    r->state = dev((size_t)P * HEADS * 8); r->tiles = dev((size_t)(P + 63) / 64 * HEADS * 8);
    r->scores = dev((size_t)CHUNK * HEADS * P * 4); r->partial = dev((size_t)8 * CHUNK * OUTW * 4);
    r->flags = dev((size_t)CHUNK * OUTW); r->ring = dev(128 * KVW * 4);
    r->kv_host = malloc((size_t)P * KVW * 4);
}
static void teardown(Run *r) {
    PcBuffer b[] = {r->qkv, r->kv, r->sinks_bf, r->sinks_f, r->out_tiled, r->out_old, r->state, r->tiles, r->scores, r->partial, r->flags, r->ring};
    for (size_t i = 0; i < sizeof b / sizeof *b; i++) pc_free(b[i]);
    free(r->kv_host);
}
static void sinks_to_f32(K *k, Run *r) {
    PcArg a[] = {view(r->sinks_bf, 0, HEADS), view(r->sinks_f, 0, HEADS)};
    TRY(pc_launch(k->sinks, HEADS, 64, a, 2, NULL)); TRY(pc_sync());
}

static void read_at(FILE *f, long long offset, void *dst, size_t bytes) {
    if (fseeko(f, offset, SEEK_SET) != 0 || fread(dst, 1, bytes, f) != bytes) { fprintf(stderr, "read failed at %lld\n", offset); exit(2); }
}

int main(int argc, char **argv) {
    if (argc != 4 && argc != 11) { fprintf(stderr, "usage: tiled-attention-device TD.cu GK.cu BUILD [MODEL EMB NORM QKVW QKVB SINKS TOKENS]\n"); return 2; }
    PcDeviceInfo info;
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    coax_t_DaStatus adm = tdp_admit(info.major, info.minor, info.max_threads, info.max_shared);
    printf("device: %s (sm_%d%d); td_attend (64 threads, 16384 shared bytes): %s\n", info.name, info.major, info.minor, adm == COAX_DaStatus_ok ? "admitted" : "refused");
    if (adm != COAX_DaStatus_ok) return 1;
    char host[256] = "unknown"; gethostname(host, sizeof host - 1);
    uint64_t machine = fnv(fnv_str(host), info.name, strlen(info.name)), build = fnv_str(argv[3]);
    K k; size_t n; double secs;
    PcModule mt, mg;
    char *src = read_all(argv[1], &n); TRY(pc_compile(src, n, "tiled-attention.cu", &mt, &secs)); free(src);
    src = read_all(argv[2], &n); TRY(pc_compile(src, n, "device-kernels.cu", &mg, &secs)); free(src);
    TRY(pc_kernel(mt, "td_attend", &k.td)); TRY(pc_kernel(mt, "td_sinks", &k.sinks));
    TRY(pc_kernel(mg, "gk_batch_scores", &k.scores)); TRY(pc_kernel(mg, "gk_batch_softmax", &k.softmax));
    TRY(pc_kernel(mg, "gk_batch_context_part", &k.context)); TRY(pc_kernel(mg, "gk_batch_finish", &k.finish));
    TRY(pc_kernel(mg, "gk_batch_embed", &k.embed)); TRY(pc_kernel(mg, "gk_batch_norm_part", &k.norm_part));
    TRY(pc_kernel(mg, "gk_batch_norm", &k.norm)); TRY(pc_kernel(mg, "gk_batch_bf_part", &k.bf_part));
    TRY(pc_kernel(mg, "gk_batch_rotate", &k.rotate));
    TRY(pc_event(&k.a)); TRY(pc_event(&k.b));

    printf("synthetic projected Q/K/V at gpt-oss-20b's attention shape: 64 heads over 8 key/value heads, width 64, BF16 sinks\n");
    int64_t lengths[] = {512, 1024, 2048, 4096};
    for (int li = 0; li < 4; li++) {
        Run r; setup(&r, lengths[li]); int64_t P = r.P;
        float *qkv = calloc((size_t)P * QW, 4); uint16_t sb[HEADS];
        for (int64_t t = 0; t < P; t++) for (int i = 0; i < OUTW; i++) qkv[t * QW + i] = 2.0f * uniform();
        for (int64_t i = 0; i < P * KVW; i++) r.kv_host[i] = 2.0f * uniform();
        for (int h = 0; h < HEADS; h++) { float v = 2.0f * uniform(); uint32_t b; memcpy(&b, &v, 4); sb[h] = (uint16_t)(b >> 16); }
        TRY(pc_upload(r.qkv, qkv, (size_t)P * QW * 4, NULL)); TRY(pc_upload(r.kv, r.kv_host, (size_t)P * KVW * 4, NULL));
        TRY(pc_upload(r.sinks_bf, sb, sizeof sb, NULL)); sinks_to_f32(&k, &r);
        r.kind = 0; r.window = 0; compare(&k, &r, machine, build, 1);
        r.kind = 1; r.window = 128; compare(&k, &r, machine, build, 1);
        free(qkv); teardown(&r);
    }

    if (argc == 11) {
        /* Layer 0 of the original gpt-oss-20b on a recorded prefill: the
           tokens of TOKENS repeated to 512 positions. Embedding rows,
           RMS norm, the BF16 QKV projection with its bias and the YaRN
           rotation run as chapter 66/67's kernels; then both attentions. */
        FILE *f = fopen(argv[4], "rb"); if (!f) { perror(argv[4]); return 2; }
        long long off_emb = atoll(argv[5]), off_norm = atoll(argv[6]), off_w = atoll(argv[7]), off_b = atoll(argv[8]), off_s = atoll(argv[9]);
        size_t tn; char *traw = read_all(argv[10], &tn); int64_t recorded = (int64_t)(tn / 8); int64_t *rec = (int64_t *)traw;
        enum { P = 512, D = 2880 };
        int64_t ids[P]; for (int64_t i = 0; i < P; i++) ids[i] = i;
        uint16_t *emb = malloc((size_t)P * D * 2), *norm = malloc(D * 2), *w = malloc((size_t)QW * D * 2), *bias = malloc(QW * 2), sb[HEADS];
        for (int64_t i = 0; i < P; i++) read_at(f, off_emb + (long long)rec[i % recorded] * D * 2, emb + i * D, D * 2);
        read_at(f, off_norm, norm, D * 2); read_at(f, off_w, w, (size_t)QW * D * 2); read_at(f, off_b, bias, QW * 2); read_at(f, off_s, sb, sizeof sb);
        fclose(f);
        float *rope = malloc((size_t)P * 64 * 4);
        if (!tdp_rope(P, (coax_v_f32){rope, P * 64})) { printf("FAIL: rotary table\n"); return 1; }
        Run r; setup(&r, P);
        PcBuffer dids = dev(P * 8), demb = dev((size_t)P * D * 2), dnorm = dev(D * 2), dw = dev((size_t)QW * D * 2), db = dev(QW * 2), drope = dev((size_t)P * 64 * 4);
        PcBuffer x = dev((size_t)P * D * 4), normal = dev((size_t)P * D * 4), npart = dev((size_t)P * 64 * 8), bpart = dev((size_t)16 * P * QW * 4), fl = dev((size_t)P * QW);
        TRY(pc_upload(dids, ids, sizeof ids, NULL)); TRY(pc_upload(demb, emb, (size_t)P * D * 2, NULL)); TRY(pc_upload(dnorm, norm, D * 2, NULL));
        TRY(pc_upload(dw, w, (size_t)QW * D * 2, NULL)); TRY(pc_upload(db, bias, QW * 2, NULL)); TRY(pc_upload(drope, rope, (size_t)P * 64 * 4, NULL));
        TRY(pc_upload(r.sinks_bf, sb, sizeof sb, NULL)); sinks_to_f32(&k, &r);
        PcArg e[] = {integer(D), integer(0), view(dids, 0, P), view(demb, 0, P * D), view(x, 0, P * D), view(fl, 0, P * QW)};
        PcArg np[] = {integer(D), integer(64), view(x, 0, P * D), view(npart, 0, P * 64)};
        PcArg nn[] = {integer(D), integer(64), view(npart, 0, P * 64), view(dnorm, 0, D), view(x, 0, P * D), view(normal, 0, P * D), view(fl, 0, P * QW)};
        PcArg bp[] = {integer(QW), integer(D), integer(P), integer(16), view(dw, 0, (int64_t)QW * D / 4), view(normal, 0, P * D), view(bpart, 0, 16 * P * QW)};
        PcArg bf[] = {integer(QW), integer(P), integer(16), view(bpart, 0, 16 * P * QW), view(db, 0, QW), view(r.qkv, 0, P * QW), view(fl, 0, P * QW)};
        PcArg ro[] = {integer(HEADS), integer(KV), integer(0), view(r.qkv, 0, P * QW), view(drope, 0, P * 64), view(r.kv, 0, P * KVW), view(fl, 0, P * QW)};
        TRY(pc_launch(k.embed, P * D, 256, e, 6, NULL));
        TRY(pc_launch(k.norm_part, P * 64, 256, np, 4, NULL));
        TRY(pc_launch(k.norm, P * D, 256, nn, 7, NULL));
        TRY(pc_launch(k.bf_part, (P + 7) / 8 * (QW / 4) * 16, 256, bp, 7, NULL));
        TRY(pc_launch(k.finish, P * QW, 256, bf, 7, NULL));
        TRY(pc_launch(k.rotate, P * (HEADS + KV) * 32, 256, ro, 7, NULL));
        TRY(pc_sync());
        uint8_t *flags = malloc((size_t)P * QW); TRY(pc_download(flags, fl, (size_t)P * QW, NULL));
        int64_t bad = 0; for (int64_t i = 0; i < P * QW; i++) bad += flags[i] != 0;
        TRY(pc_download(r.kv_host, r.kv, (size_t)P * KVW * 4, NULL));
        printf("original gpt-oss-20b layer 0 on %lld recorded tokens repeated to %d positions (non-finite projection flags: %lld)\n", (long long)recorded, P, (long long)bad);
        failures += bad != 0;
        r.kind = 1; r.window = 128; compare(&k, &r, machine, build, 0);
        printf("the same layer-0 Q/K/V under the full layers' causal mask:\n");
        r.kind = 0; r.window = 0; compare(&k, &r, machine, build, 0);
        PcBuffer bs[] = {dids, demb, dnorm, dw, db, drope, x, normal, npart, bpart, fl};
        for (size_t i = 0; i < sizeof bs / sizeof *bs; i++) pc_free(bs[i]);
        teardown(&r); free(emb); free(norm); free(w); free(bias); free(rope); free(flags); free(traw);
    }
    if (failures) { printf("FAIL: %d comparisons outside the frozen tolerance\n", failures); return 1; }
    printf("chapter 41 on CUDA: every comparison within the frozen tolerance\n");
    return 0;
}
