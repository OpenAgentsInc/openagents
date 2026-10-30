/* Chapter 31: the authenticated original Elephant decision on an Apple GPU.
   The CPU session authenticates and binds the original files; the Coax device
   plan (psionic/lib/device/plan.coax) decides every launch;
   psionic/platform/elephant-metal.c performs the effects through
   psionic/platform/metal-host.m, and this harness times them, profiles them
   and qualifies them against the CPU.
   Usage: elephant-metal ORIGINAL_DIRECTORY TOKENIZER_PST PROJECT.metal LAYER.metal WARP.metal [REPEAT]
   Prints SKIP and exits 0 without a Metal device. */
#include "elephant-metal.h"
#include "verified-file.h"
#include "elephant-inventory.h"
#include "elephant-request.h"
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

/* The plan's constants (psionic/lib/device/plan.coax). */
#define DP_KERNELS 23
#define DP_WEIGHTS 514
#define DP_WIDTH 4096
#define DP_VOCAB 248320

coax_t_PsSession psionic_session_open(uint8_t, int64_t, coax_v_u8, coax_v_u8, coax_v_u8, coax_v_u8, coax_v_t_PaFile, coax_v_t_P5Spec, coax_v_u8, coax_t_PsWorkspace);
coax_t_PrResult psionic_prepare(coax_v_u8, coax_t_PtEntry, coax_t_PtQuestion, coax_v_t_PtOption, coax_v_s64, coax_t_PrWorkspace);

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }
static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_u8 map_file(const char *path) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) { perror(path); exit(2); }
    struct stat st;
    if (fstat(fd, &st) || st.st_size <= 0) exit(2);
    void *p = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0);
    close(fd);
    if (p == MAP_FAILED) exit(2);
    return (coax_v_u8){p, st.st_size};
}
static coax_v_u8 named(const char *dir, const char *name) {
    char path[4096];
    int n = snprintf(path, sizeof path, "%s/%s", dir, name);
    if (n < 0 || (size_t)n >= sizeof path) exit(2);
    return map_file(path);
}
static char *read_text(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *text = malloc((size_t)size + 1);
    if (!text || fread(text, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f);
    text[size] = 0;
    *n = (size_t)size;
    return text;
}
#define TRY(x) do { PmStatus s_ = (x); if (s_ != PM_OK) { fprintf(stderr, "%s: status %d: %s\n", #x, s_, pm_log()); exit(1); } } while (0)

/* The same Coax kernel, compiled to C, over every work item of one launch.
   base(k) is the host address of view argument k. */
typedef struct { coax_t_DpOp *op; char *(*base)(void *, int64_t); void *ctx; } CpuLaunch;
static char *view(CpuLaunch *c, int k) { return c->base(c->ctx, k); }
#define I(k) (c->op->args.e[k].value)
#define F(k) ((float)c->op->args.e[k].real)
#define N(k) (c->op->args.e[k].count)
#define VF(k) ((coax_v_f32){(float *)view(c, k), N(k)})
#define VD(k) ((coax_v_f64){(double *)view(c, k), N(k)})
#define VU(k) ((coax_v_u64){(uint64_t *)view(c, k), N(k)})
#define VW(k) ((coax_v_u32){(uint32_t *)view(c, k), N(k)})
static void cpu_launch(CpuLaunch *c) {
    int64_t n = c->op->items;
    for (int64_t i = 0; i < n; i++) switch (c->op->kernel) {
        case 0: dv_project(i, I(0), I(1), I(2), I(3), VU(4), VF(5), VF(6)); break;
        case 1: dv_matvec(i, I(0), I(1), VU(2), VF(3), VF(4)); break;
        case 2: dv_reduce(i, I(0), I(1), VF(2), VF(3)); break;
        case 3: dl_squares(i, I(0), VF(1), VD(2)); break;
        case 4: dl_add_squares(i, I(0), VF(1), VF(2), VF(3), VD(4)); break;
        case 5: dl_scale(i, I(0), F(1), VF(2), VF(3), VD(4), VF(5)); break;
        case 6: dl_add(i, VF(0), VF(1), VF(2)); break;
        case 7: dl_swiglu(i, VF(0), VF(1), VF(2)); break;
        case 8: dl_conv(i, I(0), I(1), VF(2), VF(3), VF(4)); break;
        case 9: dl_gates(i, I(0), VF(1), VF(2), VF(3), VF(4), VF(5), VF(6)); break;
        case 10: dl_qk(i, I(0), I(1), I(2), VF(3), VF(4), VF(5)); break;
        case 11: dl_scan(i, I(0), I(1), I(2), I(3), VF(4), VF(5), VF(6), VF(7), VF(8), VF(9)); break;
        case 12: dl_gate_norm(i, F(0), VF(1), VF(2), VF(3), VF(4)); break;
        case 13: dl_prepare(i, I(0), I(1), VF(2), VF(3), VF(4), VF(5), VF(6), VF(7), VF(8), F(9)); break;
        case 14: dl_scores(i, I(0), I(1), I(2), I(3), VF(4), VF(5), VF(6)); break;
        case 15: dl_softmax(i, I(0), I(1), I(2), VF(3), VD(4), VD(5)); break;
        case 16: dl_attend(i, I(0), I(1), I(2), VD(3), VF(4), VF(5), VF(6)); break;
        case 17: dw_split(i, I(0), I(1), VF(2), VW(3), VW(4)); break;
        case 18: dw_project(i, I(0), I(1), I(2), I(3), VW(4), VW(5), VW(6), VF(7)); break;
        case 19: dl_conv_state(i, I(0), I(1), VF(2), VF(3), VF(4), VF(5), I(6)); break;
        case 20: dl_scan_state(i, I(0), I(1), I(2), I(3), VF(4), VF(5), VF(6), VF(7), VF(8), VF(9), VF(10), I(11)); break;
        case 21: dt_project_16x32(i, I(0), I(1), I(2), VW(3), VF(4), VF(5)); break;
        case 22: dt_project_16x64(i, I(0), I(1), I(2), VW(3), VF(4), VF(5)); break;
        default: fprintf(stderr, "unknown kernel %u\n", c->op->kernel); exit(1);
    }
}

/* Host addresses for the CPU repeat of one launch: the workspace copy, and
   the session's own weights. */
typedef struct { coax_t_DpOp *op; char *workspace; EgEngine *e; } HostBuffers;
static char *host_base(void *ctx, int64_t k) {
    HostBuffers *h = ctx;
    coax_t_DpArg *a = &h->op->args.e[k];
    return (a->buffer == 0 ? h->workspace : (char *)eg_weight(h->e, a->buffer - 3)) + a->offset;
}
/* f64 on the device is a float-float pair (APPLE-FF-v1) in the same 8 bytes. */
static double ff_value(const char *p) { float f[2]; memcpy(f, p, 8); return (double)f[0] + (double)f[1]; }
static int is_f64_view(coax_t_DpOp *op, int64_t k) {
    /* The f64 views of the plan's kernels: dl_squares 2, dl_add_squares 4,
       dl_scale 4, dl_softmax 4 and 5, dl_attend 3. */
    switch (op->kernel) {
        case 3: return k == 2;
        case 4: return k == 4;
        case 5: return k == 4;
        case 15: return k == 4 || k == 5;
        case 16: return k == 3;
        default: return 0;
    }
}

int main(int argc, char **argv) {
    if (argc < 7) { fprintf(stderr, "usage: elephant-metal ORIGINAL_DIRECTORY TOKENIZER_PST PROJECT.metal LAYER.metal WARP.metal TILE.metal [REPEAT]\n"); return 2; }
    int repeat = argc > 7 ? atoi(argv[7]) : 3;
    double t0 = now();
    EgSources src;
    for (int i = 0; i < 4; i++) {
        size_t n;
        char *text = read_text(argv[3 + i], &n);
        if (!text) { fprintf(stderr, "cannot read %s\n", argv[3 + i]); return 2; }
        src.text[i] = text;
        src.length[i] = n;
    }
    EgEngine *e = calloc(1, sizeof *e);
    if (!e) return 2;
    int er = eg_compile(e, &src);
    if (er == EG_SKIP) { printf("SKIP: %s\n", eg_log()); return 0; }
    if (er) { fprintf(stderr, "compile: %s\n", eg_log()); return 1; }
    PmDeviceInfo info = e->device;
    printf("device: %s, Apple GPU family %d, unified memory %s, recommended working set %.1f GiB\n", info.name, info.apple_family, info.unified_memory ? "yes" : "no",
           info.working_set_bytes / 1073741824.0);
    printf("compiled %d generated kernels with the Metal compiler in %.3f s\n", DP_KERNELS, e->compile_seconds);

    /* The CPU session authenticates and binds the original files (chapter 10). */
    coax_v_u8 config = named(argv[1], "config.json"), tokenizer = named(argv[1], "tokenizer.json"), readout = named(argv[1], "readout.json"), index = {NULL, 0}, table = map_file(argv[2]);
    coax_t_PaFile file = {0};
    char weights_path[4096]; snprintf(weights_path, sizeof weights_path, "%s/model.safetensors", argv[1]);
    static VfEntry verified; if (vf_map_file(&verified, weights_path, &file)) return 2;
    size_t count = sizeof(elephant_specs) / sizeof(*elephant_specs);
    coax_t_P5Spec *specs = calloc(count, sizeof(*specs));
    coax_t_P5Tensor *inventory = calloc(count, sizeof(*inventory)), *draft = calloc(count, sizeof(*draft));
    coax_t_QeLayer *layers = calloc(32, sizeof(*layers));
    float *params = calloc(1057280, sizeof(float)), *stage = calloc(1057280, sizeof(float)), *work = calloc(50000, sizeof(float)), *norm = calloc(4096, sizeof(float)), *norm_work = calloc(4096, sizeof(float));
    if (!specs || !inventory || !draft || !layers || !params || !stage || !work || !norm || !norm_work) return 2;
    for (size_t i = 0; i < count; i++) {
        specs[i].name = str(elephant_specs[i].name);
        specs[i].shape.rank = elephant_specs[i].rank;
        for (int j = 0; j < 5; j++) specs[i].shape.dims.e[j] = elephant_specs[i].dims[j];
        specs[i].dtype = COAX_PwDtype_bf16;
        specs[i].shard = 0;
    }
    coax_t_PsWorkspace memory = {{inventory, (int64_t)count}, {draft, (int64_t)count}, {layers, 32}, {params, 1057280}, {stage, 1057280}, {work, 50000}, {norm, 4096}, {norm_work, 4096}};
    puts("authenticating original Elephant");
    fflush(stdout);
    double ta = now();
    coax_t_PsSession session = psionic_session_open(0, 86, config, tokenizer, readout, index, (coax_v_t_PaFile){&file, 1}, (coax_v_t_P5Spec){specs, (int64_t)count}, table, memory);
    if (session.status) { fprintf(stderr, "session status=%u cause=%lld\n", session.status, (long long)session.cause); return 1; }
    double authenticated = now() - ta;
    if (vf_keep(&verified, 0, 86, 0) == 1) printf("kept a verified-file record at %s\n", verified.path);
    if (file.record.count) puts("a verified-file record named the checkpoint; its hash was skipped");
    unsigned char *prompt = calloc(65536, 1), *render_work = calloc(65536, 1), *normal = calloc(262144, 1);
    int64_t *candidate = calloc(16384, 8), *full = calloc(16384, 8), *token_stage = calloc(16384, 8), *scalars = calloc(262144, 8), *symbols = calloc(1024, 8), *tokens = calloc(16384, 8);
    coax_t_PrWorkspace prep = {{prompt, 65536}, {render_work, 65536}, {candidate, 16384}, {full, 16384}, {token_stage, 16384}, {normal, 262144}, {scalars, 262144}, {symbols, 1024}};
    coax_t_PtEntry entry = {COAX_PtEntryKind_text, str("Water is wet.")};
    coax_t_PtQuestion q = {COAX_PtQuestionKind_choice, {COAX_PtEntryKind_text, str("Is the water wet?")}};
    coax_t_PtOption opts[2] = {{str("yes"), {COAX_PtEntryKind_none, str("")}}, {str("no"), {COAX_PtEntryKind_none, str("")}}};
    coax_t_PrResult prepared = psionic_prepare(table, entry, q, (coax_v_t_PtOption){opts, 2}, (coax_v_s64){tokens, 16384}, prep);
    if (prepared.status || prepared.written != elephant_request_prefix + 37 || prepared.prefix != elephant_request_prefix ||
        memcmp(tokens, elephant_request_tokens, sizeof(elephant_request_tokens))) { fprintf(stderr, "prepared token stream differs from reference\n"); return 1; }
    int64_t T = prepared.written, P = prepared.prefix;

    er = eg_place(e, session.model, T, 0);
    if (er) { fprintf(stderr, "placement status %d (budget %lld): %s\n", er, (long long)e->budget, eg_log()); return 1; }
    if (eg_prepare(e, tokens, T, P)) { fprintf(stderr, "plan refused the decision\n"); return 1; }
    coax_t_DpPlan plan = e->plan;
    coax_t_DpOp *ops = e->ops;
    printf("plan: %lld operations; workspace %.1f MiB, resident %.2f GiB, nothing streamed (budget %.2f GiB)\n",
           (long long)plan.ops, plan.workspace / 1048576.0, plan.resident / 1073741824.0, e->budget / 1073741824.0);
    printf("session: authenticated in %.1f s; copied %.2f GiB of weights into shared memory in %.2f s\n", authenticated, e->resident_bytes / 1073741824.0, e->upload_seconds);
    double probabilities[2], prob_work[2];
    coax_t_QrResult answer = {0};
    double *times = calloc((size_t)repeat + 1, sizeof(double));
    for (int run = 0; run < repeat; run++) {
        double tr = now();
        er = eg_decide(e, tokens, T, P, NULL, NULL);
        if (er) { fprintf(stderr, "decision status %d: %s\n", er, eg_log()); return 1; }
        int64_t slot_ids[2] = {qr_pinned_slot(0), qr_pinned_slot(1)};
        answer = qr_read(COAX_QrKind_choice, 1, (coax_v_f32){e->logits, DP_VOCAB}, (coax_v_s64){slot_ids, 2}, (coax_v_f64){probabilities, 2}, (coax_v_f64){prob_work, 2}, 0);
        if (answer.status) { fprintf(stderr, "readout status %u\n", answer.status); return 1; }
        times[run] = now() - tr;
        printf("run %d: time to response %.1f ms (GPU %.1f ms in one command buffer)\n", run + 1, times[run] * 1e3, e->gpu_seconds * 1e3);
    }
    if (repeat > 1) { /* the median of the warm runs (every run after the first) */
        int n = repeat - 1;
        for (int i = 1; i < repeat; i++) for (int j = i + 1; j < repeat; j++) if (times[j] < times[i]) { double t = times[i]; times[i] = times[j]; times[j] = t; }
        double median = n % 2 ? times[1 + n / 2] : (times[n / 2] + times[1 + n / 2]) / 2;
        printf("warm median over %d runs: %.1f ms (min %.1f, max %.1f)\n", n, median * 1e3, times[1] * 1e3, times[repeat - 1] * 1e3);
    }
    /* A forked decision (chapter 8): the prefix pass and then the suffix from
       the kept state, then the suffix alone as a later question with the same
       prefix would run it. Both must give the whole decision's answer: the
       same logits bit for bit, or else within chapter 30's 0.0005. */
    {
        static float whole[DP_VOCAB];
        memcpy(whole, e->logits, sizeof whole);
        double fork_ms[2], worst = 0, fp[2], fw[2];
        int64_t differ = 0;
        for (int pass = 0; pass < 2; pass++) {
            double tf = now();
            er = eg_decide_fork(e, tokens, T, P, NULL, NULL);
            if (er || !e->forked) { fprintf(stderr, "forked decision status %d: %s\n", er, eg_log()); return 1; }
            fork_ms[pass] = (now() - tf) * 1e3;
            for (int64_t v = 0; v < DP_VOCAB; v++) {
                if (memcmp(&whole[v], &e->logits[v], 4)) differ++;
                double d = fabs((double)whole[v] - (double)e->logits[v]);
                if (d > worst) worst = d;
            }
        }
        int64_t slot_ids[2] = {qr_pinned_slot(0), qr_pinned_slot(1)};
        coax_t_QrResult forked = qr_read(COAX_QrKind_choice, 1, (coax_v_f32){e->logits, DP_VOCAB}, (coax_v_s64){slot_ids, 2}, (coax_v_f64){fp, 2}, (coax_v_f64){fw, 2}, 0);
        int same = !forked.status && fabs(fp[0] - probabilities[0]) <= 0.0005 && fabs(fp[1] - probabilities[1]) <= 0.0005 && forked.best == answer.best;
        printf("forked decision (prefix %lld, suffix %lld tokens): prefix pass and suffix %.1f ms, suffix alone %.1f ms (%lld prefix passes); "
               "%lld of %d logits differ from the whole decision (largest %.3g); p_yes %.9g %s\n",
               (long long)P, (long long)(T - P), fork_ms[0], fork_ms[1], (long long)e->prefix_passes, (long long)differ, 2 * DP_VOCAB, worst, fp[0], same ? "ok" : "FAIL");
        if (!same || e->prefix_passes != 1) return 1;
    }
    /* PSIONIC_DEVICE_PROFILE=1: one more run, one command buffer per launch,
       GPU time by kernel and by projection shape. */
    const char *profile_env = getenv("PSIONIC_DEVICE_PROFILE");
    if (profile_env && *profile_env == '1') {
        double per[DP_KERNELS] = {0}, sum = 0;
        int launches[DP_KERNELS] = {0};
        if (eg_prepare(e, tokens, T, P)) return 1;
        for (int64_t i = 0; i < plan.ops; i++) {
            PmBatch b;
            TRY(pm_begin(&b));
            if (eg_encode(e, b, i, i + 1)) { fprintf(stderr, "operation %lld: %s\n", (long long)i, eg_log()); return 1; }
            double sec = 0;
            TRY(pm_wait(b, &sec));
            sum += sec;
            per[ops[i].kernel] += sec;
            launches[ops[i].kernel]++;
        }
        printf("profiled run (one command buffer per launch): %.1f ms of GPU time\n", sum * 1e3);
        for (int k = 0; k < DP_KERNELS; k++) {
            if (!launches[k]) continue;
            coax_string name = dp_kernel_name(k);
            printf("  %-12.*s %4d launches %8.2f ms %5.1f%%\n", (int)name.count, name.data, launches[k], per[k] * 1e3, 100 * per[k] / sum);
        }
    }
    /* PSIONIC_DEVICE_QUALIFY=1: every launch of layers 0 and 3 (the first
       linear and full-attention layers) and of the readout is repeated on the
       CPU from the device's own inputs (float-float values widened to
       binary64); the whole workspace is compared. */
    const char *qualify_env = getenv("PSIONIC_DEVICE_QUALIFY");
    if (qualify_env && *qualify_env == '1') {
        size_t wsize = (size_t)plan.workspace;
        char *ws = pm_contents(e->workspace), *cpu = malloc(wsize);
        unsigned char *wide = calloc(wsize / 8 + 1, 1); /* 8-byte words that are f64 on the device */
        if (!cpu || !wide) return 2;
        HostBuffers hb = {NULL, cpu, e};
        int64_t layer = -1, checked = 0, exact = 0, worst_kernel = -1, worst_wide_kernel = -1;
        double worst = 0, worst_wide = 0;
        if (eg_prepare(e, tokens, T, P)) return 1;
        for (int64_t i = 0; i < plan.ops; i++) {
            coax_t_DpOp *op = &ops[i];
            if (op->kernel == 3 && op->args.e[1].count == T * DP_WIDTH) layer++; /* each layer starts with its input norm */
            int qualify = layer == 0 || layer == 3 || i >= plan.ops - 4;
            if (qualify) {
                memcpy(cpu, ws, wsize);
                /* The CPU kernels read and write binary64 where the device keeps pairs. */
                for (int64_t k = 0; k < op->count; k++) {
                    coax_t_DpArg *a = &op->args.e[k];
                    if (a->kind != 4 || a->buffer != 0 || !is_f64_view(op, k)) continue;
                    for (int64_t w = 0; w < a->count; w++) {
                        double v = ff_value(cpu + a->offset + 8 * w);
                        memcpy(cpu + a->offset + 8 * w, &v, 8);
                        wide[(a->offset + 8 * w) / 8] = 1;
                    }
                }
            }
            PmBatch b;
            TRY(pm_begin(&b));
            if (eg_encode(e, b, i, i + 1)) { fprintf(stderr, "operation %lld: %s\n", (long long)i, eg_log()); return 1; }
            TRY(pm_wait(b, NULL));
            if (!qualify) continue;
            hb.op = op;
            CpuLaunch c = {op, host_base, &hb};
            cpu_launch(&c);
            int64_t differ = 0;
            double max_rel = 0, max_wide = 0;
            for (size_t w = 0; w < wsize / 8; w++) {
                if (wide[w]) {
                    double want, got = ff_value(ws + 8 * w);
                    memcpy(&want, cpu + 8 * w, 8);
                    if (want == got) continue;
                    differ++;
                    double rel = fabs(want - got) / (fabs(want) > 1e-300 ? fabs(want) : 1e-300);
                    if (rel > max_wide) max_wide = rel;
                    continue;
                }
                for (int h = 0; h < 2; h++) {
                    float fu, fv;
                    memcpy(&fu, cpu + 8 * w + 4 * h, 4);
                    memcpy(&fv, ws + 8 * w + 4 * h, 4);
                    if (!memcmp(&fu, &fv, 4)) continue;
                    differ++;
                    double rel = fabs((double)fu - (double)fv) / (fabs((double)fu) > 1e-30 ? fabs((double)fu) : 1e-30);
                    if (rel > max_rel) max_rel = rel;
                }
            }
            for (int64_t k = 0; k < op->count; k++) { /* the next launch rereads these regions in the device format */
                coax_t_DpArg *a = &op->args.e[k];
                if (a->kind == 4 && a->buffer == 0 && is_f64_view(op, k)) memset(wide + a->offset / 8, 0, (size_t)a->count);
            }
            checked++;
            if (!differ) exact++;
            if (max_rel > worst) { worst = max_rel; worst_kernel = op->kernel; }
            if (max_wide > worst_wide) { worst_wide = max_wide; worst_wide_kernel = op->kernel; }
            coax_string name = dp_kernel_name(op->kernel);
            if (differ) printf("  qualify op %4lld %-14.*s layer %2lld: %lld values differ, largest relative difference %.3g (f32) %.3g (f64)\n", (long long)i,
                               (int)name.count, name.data, (long long)layer, (long long)differ, max_rel, max_wide);
        }
        coax_string wn = worst_kernel >= 0 ? dp_kernel_name(worst_kernel) : (coax_string){"none", 4};
        coax_string wf = worst_wide_kernel >= 0 ? dp_kernel_name(worst_wide_kernel) : (coax_string){"none", 4};
        printf("qualified %lld launches against the same kernels on the CPU: %lld identical; largest relative difference %.3g in f32 (%.*s), %.3g in f64 against float-float (%.*s)\n",
               (long long)checked, (long long)exact, worst, (int)wn.count, wn.data, worst_wide, (int)wf.count, wf.data);
        er = eg_decide(e, tokens, T, P, NULL, NULL); /* leave the reported answer from a whole decision */
        if (er) return 1;
        int64_t slot_ids[2] = {qr_pinned_slot(0), qr_pinned_slot(1)};
        answer = qr_read(COAX_QrKind_choice, 1, (coax_v_f32){e->logits, DP_VOCAB}, (coax_v_s64){slot_ids, 2}, (coax_v_f64){probabilities, 2}, (coax_v_f64){prob_work, 2}, 0);
    }
    PmDeviceInfo after;
    pm_memory(&after);
    printf("Metal memory allocated by this process: %.2f GiB\n", after.allocated_bytes / 1073741824.0);
    printf("choice=%s p_yes=%.9g p_no=%.9g slot_mass=%.9g confidence=%.9g\n", answer.best == 0 ? "yes" : "no", probabilities[0], probabilities[1], answer.mass, answer.confidence);
    printf("first decision after process start: %.1f s (compile, authenticate, copy, run)\n", now() - t0);
    return 0;
}
