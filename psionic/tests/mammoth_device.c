/* Chapter 46: the authenticated original Mammoth decision on a CUDA device.
   The CPU session authenticates and binds both original shards (chapter 15);
   the Coax device plan (psionic/lib/device/plan.coax, kind 1) decides
   residency, streaming and every launch; psionic/platform/elephant-gpu.c
   performs the effects. This harness times a cold open, warm decisions, a
   forked decision and a Lab-style request of five questions about one
   message: whole, forked, and batched in one suffix pass (chapter 36).
   Usage: mammoth-device ORIGINAL_DIRECTORY TOKENIZER_PST PROJECT.cu LAYER.cu WARP.cu [REPEAT]
   Prints SKIP and exits 0 without a device or NVRTC. */
#define _GNU_SOURCE
#include "elephant-gpu.h"
#include "mammoth-inventory.h"
#include "mammoth-request.h"
#include "lab_request.h"
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

coax_t_PsSession psionic_session_open(uint8_t, int64_t, coax_v_u8, coax_v_u8, coax_v_u8, coax_v_u8, coax_v_t_PaFile, coax_v_t_P5Spec, coax_v_u8, coax_t_PsWorkspace);
coax_t_PrResult psionic_prepare(coax_v_u8, coax_t_PtEntry, coax_t_PtQuestion, coax_v_t_PtOption, coax_v_s64, coax_t_PrWorkspace);

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }
static long peak_kib(void) { struct rusage u; getrusage(RUSAGE_SELF, &u); return u.ru_maxrss; }
static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static void *zeroed(int64_t count, size_t width) { void *p = calloc((size_t)count, width); if (!p) { fprintf(stderr, "out of memory\n"); exit(2); } return p; }
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

static coax_v_u8 table;
static coax_t_PrWorkspace prep;
static coax_t_PtOption opts[2];
/* One typed choice question about `message`, prepared into tokens. */
static coax_t_PrResult prepare(const char *message, const char *question, int64_t *tokens) {
    coax_t_PtEntry entry = {COAX_PtEntryKind_text, str(message)};
    coax_t_PtQuestion q = {COAX_PtQuestionKind_choice, {COAX_PtEntryKind_text, str(question)}};
    return psionic_prepare(table, entry, q, (coax_v_t_PtOption){opts, 2}, (coax_v_s64){tokens, 16384}, prep);
}
static coax_t_QrResult read_answer(EgEngine *e, double *probabilities) {
    double work[2];
    int64_t slots[2] = {qr_pinned_slot(0), qr_pinned_slot(1)};
    return qr_read(COAX_QrKind_choice, 1, (coax_v_f32){e->logits, EG_VOCAB}, (coax_v_s64){slots, 2}, (coax_v_f64){probabilities, 2}, (coax_v_f64){work, 2}, 0);
}

static int lab_prepare(const char *question, int64_t *tokens, int64_t *written, int64_t *prefix) {
    coax_t_PrResult r = prepare("Water is wet.", question, tokens);
    *written = r.written;
    *prefix = r.prefix;
    return r.status != 0;
}
static int lab_read(const float *logits, double *p_yes, int *best) {
    double probabilities[2], work[2];
    int64_t slots[2] = {qr_pinned_slot(0), qr_pinned_slot(1)};
    coax_t_QrResult a = qr_read(COAX_QrKind_choice, 1, (coax_v_f32){(float *)logits, EG_VOCAB}, (coax_v_s64){slots, 2}, (coax_v_f64){probabilities, 2}, (coax_v_f64){work, 2}, 0);
    *p_yes = probabilities[0];
    *best = (int)a.best;
    return a.status != 0;
}

/* Open the device and compile the kernels: 0, 1 on SKIP, 2 on failure. */
static int compile(EgEngine *e, const EgSources *src) {
    int er = eg_compile(e, src);
    if (er == EG_SKIP) { printf("SKIP: %s\n", pc_log()); return 1; }
    if (er) { fprintf(stderr, "compile: %s\n", pc_log()); return 2; }
    PcDeviceInfo info = e->device;
    printf("device: %s, compute %d.%d, %.2f of %.2f GiB free\n", info.name, info.major, info.minor, info.free_bytes / 1073741824.0, info.total_bytes / 1073741824.0);
    printf("compiled %d generated kernels with NVRTC in %.3f s\n", EG_KERNELS, e->compile_seconds);
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 6) { fprintf(stderr, "usage: mammoth-device ORIGINAL_DIRECTORY TOKENIZER_PST PROJECT.cu LAYER.cu WARP.cu [REPEAT]\n"); return 2; }
    int repeat = argc > 6 ? atoi(argv[6]) : 3;
    double t0 = now();
    EgSources src;
    for (int i = 0; i < 3; i++) {
        size_t n;
        char *text = read_text(argv[3 + i], &n);
        if (!text) { fprintf(stderr, "cannot read %s\n", argv[3 + i]); return 2; }
        src.text[i] = text;
        src.length[i] = n;
    }
    EgEngine *e = zeroed(1, sizeof *e);
    /* PSIONIC_DEVICE_GATE=PATH: authenticate first, then wait for PATH to
       exist before opening the device, so the card is needed only from
       there on. Without it the device opens first, so a missing driver or
       NVRTC skips before the long authentication. */
    const char *gate = getenv("PSIONIC_DEVICE_GATE");
    int gated = gate && *gate;
    int er = gated ? 0 : compile(e, &src);
    if (er) return er == 1 ? 0 : 1;

    table = map_file(argv[2]);
    unsigned char *prompt = zeroed(65536, 1), *render_work = zeroed(65536, 1), *normal = zeroed(262144, 1);
    int64_t *candidate = zeroed(16384, 8), *full = zeroed(16384, 8), *token_stage = zeroed(16384, 8), *scalars = zeroed(262144, 8), *symbols = zeroed(1024, 8), *tokens = zeroed(16384, 8);
    prep = (coax_t_PrWorkspace){{prompt, 65536}, {render_work, 65536}, {candidate, 16384}, {full, 16384}, {token_stage, 16384}, {normal, 262144}, {scalars, 262144}, {symbols, 1024}};
    opts[0] = (coax_t_PtOption){str("yes"), {COAX_PtEntryKind_none, str("")}};
    opts[1] = (coax_t_PtOption){str("no"), {COAX_PtEntryKind_none, str("")}};
    coax_t_PrResult p = prepare("Water is wet.", "Is the water wet?", tokens);
    if (p.status || p.prefix != mammoth_request_prefix || p.written != (int64_t)(sizeof(mammoth_request_tokens) / sizeof(*mammoth_request_tokens)) ||
        memcmp(tokens, mammoth_request_tokens, sizeof(mammoth_request_tokens))) { fprintf(stderr, "prepared token stream differs from reference\n"); return 1; }
    int64_t T = p.written, P = p.prefix;

    /* The CPU session authenticates both shards and binds the 64 layers (chapter 15),
       sized by ps_plan for the largest decision this harness runs. */
    int64_t capacity = 128;
    coax_t_PsPlan plan = ps_plan(1, capacity, T, P);
    if (plan.status) { fprintf(stderr, "plan status=%u\n", plan.status); return 1; }
    coax_v_u8 config = named(argv[1], "config.json"), tokenizer = named(argv[1], "tokenizer.json"), readout = named(argv[1], "readout.json"), index = named(argv[1], "model.safetensors.index.json");
    coax_t_PaFile files[2] = {{.data=named(argv[1], "model-00001-of-00002.safetensors")}, {.data=named(argv[1], "model-00002-of-00002.safetensors")}};
    size_t count = sizeof(mammoth_specs) / sizeof(*mammoth_specs);
    if ((int64_t)count != plan.tensors) return 1;
    coax_t_P5Spec *specs = zeroed(plan.tensors, sizeof(coax_t_P5Spec));
    coax_t_P5Tensor *inventory = zeroed(plan.tensors, sizeof(coax_t_P5Tensor)), *draft = zeroed(plan.tensors, sizeof(coax_t_P5Tensor));
    coax_t_QeLayer *layers = zeroed(plan.layers, sizeof(coax_t_QeLayer));
    float *params = zeroed(plan.params, 4), *stage = zeroed(plan.params, 4), *work = zeroed(plan.layer_work, 4), *norm = zeroed(plan.norm, 4), *norm_work = zeroed(plan.norm, 4);
    for (size_t i = 0; i < count; i++) {
        specs[i].name = str(mammoth_specs[i].name);
        specs[i].shape.rank = mammoth_specs[i].rank;
        for (int j = 0; j < 5; j++) specs[i].shape.dims.e[j] = mammoth_specs[i].dims[j];
        specs[i].dtype = COAX_PwDtype_bf16;
        specs[i].shard = mammoth_specs[i].shard;
    }
    coax_t_PsWorkspace memory = {{inventory, plan.tensors}, {draft, plan.tensors}, {layers, plan.layers}, {params, plan.params}, {stage, plan.params}, {work, plan.layer_work}, {norm, plan.norm}, {norm_work, plan.norm}};
    puts("authenticating original Mammoth");
    fflush(stdout);
    double ta = now();
    coax_t_PsSession session = psionic_session_open(1, capacity, config, tokenizer, readout, index, (coax_v_t_PaFile){files, 2}, (coax_v_t_P5Spec){specs, (int64_t)count}, table, memory);
    if (session.status) { fprintf(stderr, "session status=%u cause=%lld\n", session.status, (long long)session.cause); return 1; }
    double authenticated = now() - ta;

    /* PSIONIC_DEVICE_BUDGET_MIB lowers the budget to stream more. */
    const char *budget_env = getenv("PSIONIC_DEVICE_BUDGET_MIB");
    int64_t budget = budget_env && atoll(budget_env) > 0 ? atoll(budget_env) * 1048576LL : 0;
    double waited = 0;
    if (gated) {
        printf("authenticated; waiting for %s before placing\n", gate);
        fflush(stdout);
        double tw = now();
        while (access(gate, F_OK)) usleep(100000);
        waited = now() - tw;
        if ((er = compile(e, &src))) return er == 1 ? 0 : 1;
    }
    double tp = now();
    er = eg_place_kind(e, 1, session.model, 2 * capacity, budget); /* room for a batch of five suffixes */
    if (er) { fprintf(stderr, "placement status %d (budget %lld): %s\n", er, (long long)e->budget, eg_log()); return 1; }
    double placed = now() - tp;
    coax_t_DpPlan pl = e->placed;
    printf("plan: workspace %.1f MiB, resident %.2f GiB, streamed %.2f GiB in %lld copies through 2 slots of %.1f MiB (budget %.2f GiB)\n",
           pl.workspace / 1048576.0, pl.resident / 1073741824.0, pl.streamed / 1073741824.0, (long long)pl.streamed_tensors, pl.slot / 1048576.0, e->budget / 1073741824.0);
    printf("session: authenticated in %.1f s; placed in %.1f s: uploaded %.2f GiB resident in %.2f s, pinned %.2f GiB for streaming; peak rss %.1f GiB\n",
           authenticated, placed, e->resident_bytes / 1073741824.0, e->upload_seconds, e->pinned_bytes / 1073741824.0, peak_kib() / 1048576.0);
    printf("cold open: %.1f s from process start to a placed engine (not counting %.1f s waiting for the card)\n", now() - t0 - waited, waited);
    fflush(stdout);

    double probabilities[2] = {0};
    coax_t_QrResult answer = {0};
    for (int run = 0; run < repeat; run++) {
        double tr = now();
        if (eg_decide(e, tokens, T, P, NULL, NULL)) { fprintf(stderr, "decision failed: %s\n", eg_log()); return 1; }
        answer = read_answer(e, probabilities);
        if (answer.status) { fprintf(stderr, "readout status %u\n", answer.status); return 1; }
        printf("run %d: time to response %.1f ms (%lld operations), p_yes %.9g\n", run + 1, (now() - tr) * 1e3, (long long)e->plan.ops, probabilities[0]);
        fflush(stdout);
    }
    static float whole[EG_VOCAB];
    memcpy(whole, e->logits, sizeof whole);

    /* The same decision through the fork: prefix pass and suffix, then the suffix alone. */
    for (int pass = 0; pass < 2; pass++) {
        double tf = now();
        if (eg_decide_fork(e, tokens, T, P, NULL, NULL) || !e->forked) { fprintf(stderr, "forked decision failed: %s\n", eg_log()); return 1; }
        double ms = (now() - tf) * 1e3;
        int64_t differ = 0;
        for (int64_t v = 0; v < EG_VOCAB; v++) if (memcmp(&whole[v], &e->logits[v], 4)) differ++;
        double fp[2];
        coax_t_QrResult forked = read_answer(e, fp);
        printf("forked %s: %.1f ms; %lld of %d logits differ from the whole decision; p_yes %.9g\n", pass ? "suffix alone" : "prefix pass and suffix", ms, (long long)differ, EG_VOCAB, fp[0]);
        if (forked.status || forked.best != answer.best || fabs(fp[0] - probabilities[0]) > 0.0005) { fprintf(stderr, "forked answer differs\n"); return 1; }
    }

    /* A Lab-style request (chapter 36): whole, forked, batched. */
    if (lab_request(e, lab_prepare, lab_read, 2)) return 1;

    size_t free_after, total_bytes;
    pc_memory(&free_after, &total_bytes);
    printf("device memory in use by this process: about %.2f GiB; %.2f GiB left free\n", ((double)e->free_before - (double)free_after) / 1073741824.0, free_after / 1073741824.0);
    printf("choice=%s p_yes=%.9g p_no=%.9g slot_mass=%.9g confidence=%.9g\n", answer.best == 0 ? "yes" : "no", probabilities[0], probabilities[1], answer.mass, answer.confidence);
    return 0;
}
