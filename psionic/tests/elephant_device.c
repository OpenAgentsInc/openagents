/* Chapter 30: the authenticated original Elephant decision on a CUDA device.
   The CPU session authenticates and binds the original files; the Coax device
   plan (psionic/lib/device/plan.coax) decides residency, streaming and every
   launch; psionic/platform/elephant-gpu.c performs the effects, and this
   harness times them, profiles them and qualifies them against the CPU.
   Usage: elephant-device ORIGINAL_DIRECTORY TOKENIZER_PST PROJECT.cu LAYER.cu WARP.cu [REPEAT]
   Prints SKIP and exits 0 without a device or NVRTC. */
#define _GNU_SOURCE
#include "elephant-gpu.h"
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
#define DP_KERNELS 21
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
#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: status %d: %s\n", #x, s_, pc_log()); exit(1); } } while (0)

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
        default: fprintf(stderr, "unknown kernel %u\n", c->op->kernel); exit(1);
    }
}
typedef struct { coax_t_DpOp *op; char *workspace; char *slot_host[2]; char **weights; } HostBuffers;
static char *host_base(void *ctx, int64_t k) {
    HostBuffers *h = ctx;
    coax_t_DpArg *a = &h->op->args.e[k];
    char *base = a->buffer == 0 ? h->workspace : a->buffer <= 2 ? h->slot_host[a->buffer - 1] : h->weights[a->buffer - 3];
    return base + a->offset;
}

int main(int argc, char **argv) {
    if (argc < 6) { fprintf(stderr, "usage: elephant-device ORIGINAL_DIRECTORY TOKENIZER_PST PROJECT.cu LAYER.cu WARP.cu [REPEAT]\n"); return 2; }
    int repeat = argc > 6 ? atoi(argv[6]) : 3;
    double t0 = now();
    /* Kernels first, so a missing driver or NVRTC skips before the long authentication. */
    EgSources src;
    for (int i = 0; i < 3; i++) {
        size_t n;
        char *text = read_text(argv[3 + i], &n);
        if (!text) { fprintf(stderr, "cannot read %s\n", argv[3 + i]); return 2; }
        src.text[i] = text;
        src.length[i] = n;
    }
    EgEngine *e = calloc(1, sizeof *e);
    if (!e) return 2;
    int er = eg_compile(e, &src);
    if (er == EG_SKIP) { printf("SKIP: %s\n", pc_log()); return 0; }
    if (er) { fprintf(stderr, "compile: %s\n", pc_log()); return 1; }
    PcDeviceInfo info = e->device;
    printf("device: %s, compute %d.%d, %.2f of %.2f GiB free\n", info.name, info.major, info.minor, info.free_bytes / 1073741824.0, info.total_bytes / 1073741824.0);
    printf("compiled %d generated kernels with NVRTC in %.3f s\n", DP_KERNELS, e->compile_seconds);

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

    /* The plan: what fits, what streams, and every operation.
       PSIONIC_DEVICE_BUDGET_MIB lowers the budget to measure smaller headroom. */
    const char *budget_env = getenv("PSIONIC_DEVICE_BUDGET_MIB");
    int64_t budget = budget_env && atoll(budget_env) > 0 ? atoll(budget_env) * 1048576LL : 0;
    er = eg_place(e, session.model, T, budget);
    if (er) { fprintf(stderr, "placement status %d (budget %lld): %s\n", er, (long long)e->budget, eg_log()); return 1; }
    if (eg_prepare(e, tokens, T, P)) { fprintf(stderr, "plan refused the decision\n"); return 1; }
    coax_t_DpPlan plan = e->plan;
    coax_t_DpOp *ops = e->ops;
    printf("plan: %lld operations; workspace %.1f MiB, resident %.2f GiB, streamed %.2f GiB in %lld copies through 2 slots of %.1f MiB (budget %.2f GiB)\n",
           (long long)plan.ops, plan.workspace / 1048576.0, plan.resident / 1073741824.0, plan.streamed / 1073741824.0, (long long)plan.streamed_tensors,
           plan.slot / 1048576.0, e->budget / 1073741824.0);
    printf("session: authenticated in %.1f s; uploaded %.2f GiB resident in %.2f s (%.1f GB/s); pinned %.2f GiB for streaming\n",
           authenticated, e->resident_bytes / 1073741824.0, e->upload_seconds, e->resident_bytes / e->upload_seconds / 1e9, e->pinned_bytes / 1073741824.0);
    PcBuffer workspace = e->workspace, *slots = e->slots, *address = e->address;
    PcKernel *kernels = e->kernels;
    void **pinned = e->pinned;
    int64_t *bytes = e->bytes;
    PcStream compute = e->compute, copy = e->copy;
    PcEvent *released = e->released, start, mid, end;
    TRY(pc_event(&start)); TRY(pc_event(&mid)); TRY(pc_event(&end));
    float *x = e->x, *rope = e->rope, *logits = e->logits;
    /* PSIONIC_DEVICE_REPLAY=1 records the decision's copies and launches once
       and replays the recording for every run. */
    const char *replay_env = getenv("PSIONIC_DEVICE_REPLAY");
    int replay = replay_env && *replay_env == '1';
    PcGraph graph = NULL;
    PcEvent join, rejoin;
    TRY(pc_event(&join)); TRY(pc_event(&rejoin));
    double probabilities[2], prob_work[2];
    coax_t_QrResult answer = {0};
    /* PSIONIC_DEVICE_PROFILE=1 times every launch of one extra run with events. */
    const char *profile_env = getenv("PSIONIC_DEVICE_PROFILE");
    int profile = profile_env && *profile_env == '1';
    PcEvent *marks = NULL;
    if (profile) {
        marks = calloc((size_t)plan.ops + 1, sizeof(PcEvent));
        for (int64_t i = 0; i <= plan.ops; i++) TRY(pc_event(&marks[i]));
    }
    for (int run = 0; run < repeat + profile; run++) {
        int timed = profile && run == repeat;
        double tr = now();
        /* Inputs: embedding rows and rotary coefficients, computed by Coax on the host. */
        if (eg_prepare(e, tokens, T, P)) { fprintf(stderr, "inputs refused\n"); return 1; }
        if (replay && graph) {
            TRY(pc_replay(graph, compute));
            goto issued;
        }
        if (replay) {
            TRY(pc_capture_begin(compute));
            TRY(pc_record(join, compute));
            TRY(pc_wait(copy, join));
            for (int k = 0; k < 2; k++) TRY(pc_record(released[k], compute));
        }
        TRY(pc_record(start, compute));
        TRY(pc_upload(workspace + (PcBuffer)dp_input_offset(T), x, (size_t)T * DP_WIDTH * 4, compute));
        TRY(pc_upload(workspace + (PcBuffer)dp_rope_offset(T), rope, (size_t)T * 64 * 4, compute));
        TRY(pc_record(mid, compute));
        for (int64_t i = 0; i < plan.ops; i++) {
            if (timed) TRY(pc_record(marks[i], compute));
            if (eg_issue(e, i, compute)) { fprintf(stderr, "operation %lld: %s\n", (long long)i, pc_log()); return 1; }
        }
        if (timed) TRY(pc_record(marks[plan.ops], compute));
        TRY(pc_download(logits, workspace + (PcBuffer)dp_logits_offset(T), DP_VOCAB * 4, compute));
        TRY(pc_record(end, compute));
        if (replay) {
            TRY(pc_record(rejoin, copy));
            TRY(pc_wait(compute, rejoin));
            TRY(pc_capture_end(compute, &graph));
            TRY(pc_replay(graph, compute));
        }
    issued:
        TRY(pc_sync_stream(compute));
        int64_t slot_ids[2] = {qr_pinned_slot(0), qr_pinned_slot(1)};
        answer = qr_read(COAX_QrKind_choice, 1, (coax_v_f32){logits, DP_VOCAB}, (coax_v_s64){slot_ids, 2}, (coax_v_f64){probabilities, 2}, (coax_v_f64){prob_work, 2}, 0);
        double total = now() - tr, device = 0, inputs = 0;
        if (answer.status) { fprintf(stderr, "readout status %u\n", answer.status); return 1; }
        if (replay) { printf("run %d: time to response %.1f ms (one recorded replay of %lld operations)\n", run + 1, total * 1e3, (long long)plan.ops); continue; }
        TRY(pc_event_seconds(mid, end, &device));
        TRY(pc_event_seconds(start, mid, &inputs));
        if (!timed) { printf("run %d: time to response %.1f ms (device %.1f ms, input upload %.2f ms)\n", run + 1, total * 1e3, device * 1e3, inputs * 1e3); continue; }
        /* Per kernel, and per projection shape. */
        double per[DP_KERNELS] = {0}, shape_ms[64] = {0}, sum = 0;
        int launches[DP_KERNELS] = {0}, shape_n[64] = {0}, shapes = 0;
        int64_t shape_rows[64], shape_cols[64];
        for (int64_t i = 0; i < plan.ops; i++) {
            double sec;
            TRY(pc_event_seconds(marks[i], marks[i + 1], &sec));
            sum += sec;
            if (ops[i].kind != COAX_DpOpKind_launch) continue;
            per[ops[i].kernel] += sec;
            launches[ops[i].kernel]++;
            if (ops[i].kernel == 0) {
                int64_t r = ops[i].args.e[0].value, c = ops[i].args.e[1].value;
                int k = 0;
                while (k < shapes && (shape_rows[k] != r || shape_cols[k] != c)) k++;
                if (k == shapes && shapes < 64) { shape_rows[k] = r; shape_cols[k] = c; shapes++; }
                shape_ms[k] += sec;
                shape_n[k]++;
            }
        }
        printf("profiled run (events between launches): %.1f ms\n", sum * 1e3);
        for (int k = 0; k < DP_KERNELS; k++) {
            coax_string name = dp_kernel_name(k);
            printf("  %-12.*s %4d launches %8.2f ms %5.1f%%\n", (int)name.count, name.data, launches[k], per[k] * 1e3, 100 * per[k] / sum);
        }
        for (int k = 0; k < shapes; k++)
            printf("  projection %6lld x %-5lld %3d launches %8.2f ms  %.2f TFLOP/s\n", (long long)shape_rows[k], (long long)shape_cols[k], shape_n[k],
                   shape_ms[k] * 1e3, 2.0 * shape_rows[k] * shape_cols[k] * T * shape_n[k] / shape_ms[k] / 1e12);
    }
    /* A forked decision (chapter 8): the prefix pass and then the suffix from
       the kept state, then the suffix alone as a later question with the same
       prefix would run it. Both must give the whole decision's answer within
       0.0005; a suffix under 16 tokens projects with the work-item kernels,
       so its logits need not match bit for bit. */
    if (!replay) {
        static float whole[DP_VOCAB];
        memcpy(whole, logits, sizeof whole);
        double fork_ms[2], worst = 0, fp[2], fw[2];
        int64_t differ = 0;
        for (int pass = 0; pass < 2; pass++) {
            double tf = now();
            int fr = eg_decide_fork(e, tokens, T, P, NULL, NULL);
            if (fr || !e->forked) { fprintf(stderr, "forked decision status %d: %s\n", fr, eg_log()); return 1; }
            fork_ms[pass] = (now() - tf) * 1e3;
            for (int64_t v = 0; v < DP_VOCAB; v++) {
                if (memcmp(&whole[v], &logits[v], 4)) differ++;
                double d = fabs((double)whole[v] - (double)logits[v]);
                if (d > worst) worst = d;
            }
        }
        int64_t slot_ids[2] = {qr_pinned_slot(0), qr_pinned_slot(1)};
        coax_t_QrResult forked = qr_read(COAX_QrKind_choice, 1, (coax_v_f32){logits, DP_VOCAB}, (coax_v_s64){slot_ids, 2}, (coax_v_f64){fp, 2}, (coax_v_f64){fw, 2}, 0);
        int same = !forked.status && fabs(fp[0] - probabilities[0]) <= 0.0005 && fabs(fp[1] - probabilities[1]) <= 0.0005 && forked.best == answer.best;
        printf("forked decision (prefix %lld, suffix %lld tokens): prefix pass and suffix %.1f ms, suffix alone %.1f ms (%lld prefix passes); "
               "%lld of %d logits differ from the whole decision (largest %.3g); p_yes %.9g %s\n",
               (long long)P, (long long)(T - P), fork_ms[0], fork_ms[1], (long long)e->prefix_passes, (long long)differ, 2 * DP_VOCAB, worst, fp[0], same ? "ok" : "FAIL");
        if (!same || e->prefix_passes != 1) return 1;
        if (eg_prepare(e, tokens, T, P)) return 1; /* the whole decision's plan again, for what follows */
        memcpy(logits, whole, sizeof whole);
    }
    /* PSIONIC_DEVICE_QUALIFY=1: every launch of layers 0 and 3 (the first
       linear and full-attention layers) and of the readout is repeated on the
       CPU from the device's own inputs; the whole workspace is compared. */
    const char *qualify_env = getenv("PSIONIC_DEVICE_QUALIFY");
    if (qualify_env && *qualify_env == '1') {
        size_t wsize = (size_t)plan.workspace;
        char *before = malloc(wsize), *gpu = malloc(wsize), **weights = calloc(DP_WEIGHTS, sizeof(char *));
        if (!before || !gpu || !weights) return 2;
        for (int64_t w = 0; w < DP_WEIGHTS; w++) {
            if (!bytes[w]) continue;
            weights[w] = pinned[w] ? pinned[w] : (char *)eg_weight(e, w);
        }
        HostBuffers hb = {0};
        hb.weights = weights;
        int64_t layer = -1, streamed_slot_tensor[2] = {-1, -1}, checked = 0, exact = 0, worst_kernel = -1;
        double worst = 0;
        TRY(pc_upload(workspace + (PcBuffer)dp_input_offset(T), x, (size_t)T * DP_WIDTH * 4, compute));
        TRY(pc_upload(workspace + (PcBuffer)dp_rope_offset(T), rope, (size_t)T * 64 * 4, compute));
        for (int64_t i = 0; i < plan.ops; i++) {
            coax_t_DpOp *op = &ops[i];
            if (op->kind == COAX_DpOpKind_copy) {
                TRY(pc_sync());
                TRY(pc_upload(slots[op->slot], pinned[op->tensor], (size_t)bytes[op->tensor], NULL));
                TRY(pc_sync());
                streamed_slot_tensor[op->slot] = op->tensor;
                continue;
            }
            if (op->kind != COAX_DpOpKind_launch) continue;
            if (op->kernel == 3 && op->args.e[1].count == T * DP_WIDTH) layer++; /* each layer starts with its input norm */
            int qualify = layer == 0 || layer == 3 || i >= plan.ops - 4;
            PcArg args[12];
            for (int64_t k = 0; k < op->count; k++) {
                coax_t_DpArg *a = &op->args.e[k];
                args[k] = (PcArg){(PcArgKind)a->kind, a->value, a->real, 0, a->count};
                if (a->kind == 4) args[k].data = (a->buffer == 0 ? workspace : a->buffer <= 2 ? slots[a->buffer - 1] : address[a->buffer - 3]) + (PcBuffer)a->offset;
            }
            if (qualify) { TRY(pc_sync()); TRY(pc_download(before, workspace, wsize, NULL)); TRY(pc_sync()); }
            if (op->lanes == 32) TRY(pc_launch_warps(kernels[op->kernel], op->items, (int)op->block, args, (int)op->count, NULL));
            else TRY(pc_launch(kernels[op->kernel], op->items, (int)op->block, args, (int)op->count, NULL));
            if (!qualify) continue;
            TRY(pc_sync());
            TRY(pc_download(gpu, workspace, wsize, NULL));
            TRY(pc_sync());
            hb.op = op;
            hb.workspace = before;
            for (int s2 = 0; s2 < 2; s2++) hb.slot_host[s2] = streamed_slot_tensor[s2] >= 0 ? pinned[streamed_slot_tensor[s2]] : NULL;
            CpuLaunch c = {op, host_base, &hb};
            cpu_launch(&c);
            int64_t differ = 0;
            double max_rel = 0;
            for (size_t b = 0; b < wsize / 4; b++) {
                uint32_t u, v2;
                memcpy(&u, before + 4 * b, 4);
                memcpy(&v2, gpu + 4 * b, 4);
                if (u == v2) continue;
                differ++;
                float fu, fv;
                memcpy(&fu, &u, 4);
                memcpy(&fv, &v2, 4);
                double rel = fabs((double)fu - (double)fv) / (fabs((double)fu) > 1e-30 ? fabs((double)fu) : 1e-30);
                if (rel > max_rel) max_rel = rel;
            }
            checked++;
            if (!differ) exact++;
            if (max_rel > worst) { worst = max_rel; worst_kernel = op->kernel; }
            coax_string name = dp_kernel_name(op->kernel);
            if (differ) printf("  qualify op %4lld %-14.*s layer %2lld: %lld words differ, largest relative difference %.3g\n", (long long)i, (int)name.count, name.data,
                               (long long)layer, (long long)differ, max_rel);
        }
        coax_string wn = worst_kernel >= 0 ? dp_kernel_name(worst_kernel) : (coax_string){"none", 4};
        printf("qualified %lld launches against the same kernels on the CPU: %lld bit-identical; largest relative difference %.3g (%.*s)\n",
               (long long)checked, (long long)exact, worst, (int)wn.count, wn.data);
    }
    size_t free_after, total_bytes;
    pc_memory(&free_after, &total_bytes);
    printf("device memory in use by this process: about %.2f GiB; %.2f GiB left free\n", ((double)e->free_before - (double)free_after) / 1073741824.0, free_after / 1073741824.0);
    printf("choice=%s p_yes=%.9g p_no=%.9g slot_mass=%.9g confidence=%.9g\n", answer.best == 0 ? "yes" : "no", probabilities[0], probabilities[1], answer.mass, answer.confidence);
    printf("first decision after process start: %.1f s (compile, authenticate, upload, run)\n", now() - t0);
    return 0;
}
