/* char-gpt-shakespeare-v1 (#12627) on the CPU or one CUDA device. Effects
   only: read the corpus and checkpoints, allocate the three arenas, copy
   bytes, replay the launches char-gpt.coax writes, time them and write
   records. The layout, launches, data order, initial values, every step
   decision, the transfer ledger and the checkpoint header are Coax
   (char-gpt.coax and char-gpt-policy.coax, compiled to C beside this file).
   There is no host arithmetic on parameters, activations or gradients: a
   kernel the plan has no device form for cannot be launched at all.

   Usage:
     char-gpt selfcheck
     char-gpt run cpu|gpu tiny|full CORPUS OUT [--kernels K.cu] [--steps N]
         [--budget SECONDS] [--dump K] [--eval-every E] [--memory BYTES]
   A run resumes from OUT/state.ckpt when it exists and saves there when it
   stops (after --steps, or when --budget seconds of steps have passed).
   --dump K writes the oracle comparison files (init.f32, grad1.f64,
   params.f64, oracle.json) after K steps of a fresh run and stops. */
#define _POSIX_C_SOURCE 200809L
#include "char-gpt.h"
#include "cuda-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <math.h>

#define CG_P_ID ((int)cg_buffer(0))
#define CG_G_ID ((int)cg_buffer(1))
#define CG_M_ID ((int)cg_buffer(2))
#define CG_V_ID ((int)cg_buffer(3))
#define CG_LOSS_ID ((int)cg_buffer(4))
#define CG_SCAL_ID ((int)cg_buffer(5))
#define CG_DATA_ID ((int)cg_buffer(6))
#define CG_OFFS_ID ((int)cg_buffer(7))

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
#define NEED(c, msg) do { if (!(c)) { fprintf(stderr, "%s\n", msg); exit(1); } } while (0)

static const char *kernel_names[16] = {"ck_mm_nn","ck_mm_nt","ck_mm_tn","ck_softmax","ck_softmax_back","ck_rms","ck_rms_back","ck_ids","ck_embed","ck_embed_back","ck_pos_back","ck_relu2","ck_relu2_back","ck_ce","ck_sum","ck_adamw"};
static int gpu;
static PcKernel kernels[16];
static PcBuffer dev[3];
static float *hf; static uint8_t *hu; static int64_t *hw; /* CPU arenas */
static coax_t_CgPlan plan;
static int64_t launches_done;

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec * 1e-9; }
static uint8_t *read_all(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb"); if (!f) return NULL;
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    uint8_t *t = malloc((size_t)size + 1);
    if (!t || fread(t, 1, (size_t)size, f) != (size_t)size) exit(2);
    fclose(f); *n = (size_t)size; return t;
}
static void write_all(const char *path, const void *a, size_t n, const void *b, size_t m) {
    char tmp[4200]; snprintf(tmp, sizeof tmp, "%s.tmp", path);
    FILE *f = fopen(tmp, "wb"); NEED(f, "cannot write output");
    NEED(fwrite(a, 1, n, f) == n && (!m || fwrite(b, 1, m, f) == m) && fclose(f) == 0, "write failed");
    NEED(rename(tmp, path) == 0, "rename failed");
}
static const int64_t elem[3] = {4, 1, 8};
static size_t arena_bytes(int a) { return (size_t)(a == 0 ? plan.floats : a == 1 ? plan.bytes : plan.offsets) * (size_t)elem[a]; }
static int buf_arena(int b) { return (int)plan.arena.e[b]; }
/* Copies between a host array and a buffer's elements [first, first+count). */
static void up(int b, int64_t first, const void *src, int64_t count) {
    int a = buf_arena(b); size_t off = (size_t)(plan.at.e[b] + first) * (size_t)elem[a], n = (size_t)count * (size_t)elem[a];
    if (gpu) TRY(pc_upload(dev[a] + off, src, n, NULL));
    else memcpy((a == 0 ? (uint8_t *)hf : a == 1 ? hu : (uint8_t *)hw) + off, src, n);
}
static void down(void *dst, int b, int64_t first, int64_t count) {
    int a = buf_arena(b); size_t off = (size_t)(plan.at.e[b] + first) * (size_t)elem[a], n = (size_t)count * (size_t)elem[a];
    if (gpu) TRY(pc_download(dst, dev[a] + off, n, NULL));
    else memcpy(dst, (a == 0 ? (uint8_t *)hf : a == 1 ? hu : (uint8_t *)hw) + off, n);
}
static void run(coax_t_CgLaunch *ls, int64_t count) {
    for (int64_t i = 0; i < count; i++) {
        coax_t_CgLaunch *l = &ls[i];
        if (!gpu) {
            cg_exec(*l, (coax_v_f32){hf, plan.floats}, (coax_v_u8){hu, plan.bytes}, (coax_v_s64){hw, plan.offsets});
        } else {
            PcArg a[26];
            for (int j = 0; j < l->argc; j++) {
                coax_t_CgArg *x = &l->args.e[j];
                if (x->kind == 0) a[j] = (PcArg){PC_ARG_INT, x->i, 0, 0, 0};
                else if (x->kind == 1) a[j] = (PcArg){PC_ARG_F32, 0, x->f, 0, 0};
                else { int ar = (int)x->kind - 2; a[j] = (PcArg){PC_ARG_VIEW, 0, 0, dev[ar] + (PcBuffer)(x->at * elem[ar]), x->count}; }
            }
            if (l->block) TRY(pc_launch_warps(kernels[l->kernel], l->items * l->block / 32, (int)l->block, a, (int)l->argc, NULL));
            else TRY(pc_launch(kernels[l->kernel], l->items, 256, a, (int)l->argc, NULL));
        }
        launches_done++;
    }
}

int main(int argc, char **argv) {
    if (argc >= 2 && !strcmp(argv[1], "selfcheck")) {
        int64_t g = cgn_gradients(), a = cgn_accumulate(), u = cgn_update();
        printf("{\"gradients_failures\":%lld,\"accumulate_failures\":%lld,\"update_failures\":%lld}\n", (long long)g, (long long)a, (long long)u);
        return g || a || u;
    }
    NEED(argc >= 7 && !strcmp(argv[1], "run"), "usage: char-gpt selfcheck | run cpu|gpu tiny|full CORPUS OUT [options]");
    gpu = !strcmp(argv[2], "gpu");
    coax_t_CgShape s = !strcmp(argv[3], "full") ? cg_full() : cg_tiny();
    const char *corpus = argv[4], *out = argv[5], *cu = NULL;
    int64_t target = cg_steps(), dump = 0, every = 250; double budget = 780; int64_t memory = (int64_t)8 << 30;
    for (int i = 6; i + 1 < argc; i += 2) {
        if (!strcmp(argv[i], "--kernels")) cu = argv[i + 1];
        else if (!strcmp(argv[i], "--steps")) target = atoll(argv[i + 1]);
        else if (!strcmp(argv[i], "--budget")) budget = atof(argv[i + 1]);
        else if (!strcmp(argv[i], "--dump")) dump = atoll(argv[i + 1]);
        else if (!strcmp(argv[i], "--eval-every")) every = atoll(argv[i + 1]);
        else if (!strcmp(argv[i], "--memory")) memory = atoll(argv[i + 1]);
        else NEED(0, "unknown option");
    }
    double t0 = now();
    /* The corpus: admitted by Coax, then tokenized by Coax. */
    size_t n = 0; uint8_t *text = read_all(corpus, &n); NEED(text, "cannot read the corpus");
    NEED(cgp_corpus((int64_t)n, cgp_fnv((coax_v_u8){text, (int64_t)n})) == COAX_CgStatus_ok, "corpus refused: not the frozen Tiny Shakespeare");
    int64_t table[256]; NEED(cg_vocabulary((coax_v_u8){text, (int64_t)n}, (coax_v_s64){table, 256}) == s.vocab, "vocabulary differs");
    uint8_t *ids = malloc(n); cg_tokenize((coax_v_u8){text, (int64_t)n}, (coax_v_s64){table, 256}, (coax_v_u8){ids, (int64_t)n});
    int64_t train = cgp_train_bytes((int64_t)n), heldout = (int64_t)n - train;
    plan = cg_plan(s, (int64_t)n, memory);
    NEED(plan.status == COAX_CgStatus_ok, "plan refused (shape or memory budget)");
    int64_t np = plan.n, rows = s.rows * s.accumulate, tokens = rows * s.context;
    double compile_s = 0; size_t free0 = 0, free1 = 0, total = 0;
    if (gpu) {
        PcDeviceInfo info; NEED(pc_open(&info) == PC_OK, pc_log());
        NEED(cu, "--kernels K.cu is required on the GPU");
        size_t cn = 0; char *src = (char *)read_all(cu, &cn); NEED(src, "cannot read kernels");
        PcModule mod; TRY(pc_compile(src, cn, "char-gpt-kernels.cu", &mod, &compile_s));
        for (int k = 0; k < 16; k++) TRY(pc_kernel(mod, kernel_names[k], &kernels[k]));
        TRY(pc_memory(&free0, &total));
        fprintf(stderr, "device: %s (sm_%d%d), %zu MiB free of %zu; plan %lld bytes\n", info.name, info.major, info.minor, free0 >> 20, total >> 20, (long long)plan.used);
        for (int a = 0; a < 3; a++) TRY(pc_alloc(arena_bytes(a), &dev[a]));
        TRY(pc_fill(dev[0], 0, (size_t)plan.floats, NULL));
        TRY(pc_memory(&free1, &total));
    } else {
        hf = calloc((size_t)plan.floats, 4); hu = calloc((size_t)plan.bytes, 1); hw = calloc((size_t)plan.offsets, 8);
        NEED(hf && hu && hw, "out of host memory");
    }
    up(CG_DATA_ID, 0, ids, (int64_t)n);
    /* Parameters: a checkpoint, or the specification's initial values. */
    float *p = malloc((size_t)np * 12); NEED(p, "out of memory");
    int64_t shape[8]; cg_shape_ints(s, (coax_v_s64){shape, 8});
    char path[4096]; snprintf(path, sizeof path, "%s/state.ckpt", out);
    int64_t step = 0, skipped = 0; size_t cn = 0; uint8_t *ck = read_all(path, &cn);
    if (ck) {
        NEED(cn == 128 + (size_t)np * 12, "checkpoint size differs");
        int64_t h[16]; memcpy(h, ck, 128); memcpy(p, ck + 128, (size_t)np * 12);
        uint64_t fnv = cg_fnv_f32(0xcbf29ce484222325ULL, (coax_v_f32){p, np * 3});
        NEED(cgp_header_ok((coax_v_s64){h, 16}, 0, np, cg_steps(), (coax_v_s64){shape, 8}, fnv) == COAX_CgStatus_ok, "checkpoint refused");
        step = h[4]; skipped = h[5]; free(ck);
        up(CG_P_ID, 0, p, np); up(CG_M_ID, 0, p + np, np); up(CG_V_ID, 0, p + 2 * np, np);
    } else {
        NEED(cg_init(s, (coax_v_f32){p, np}) == COAX_CgStatus_ok, "init refused");
        up(CG_P_ID, 0, p, np);
        if (dump) { snprintf(path, sizeof path, "%s/init.f32", out); write_all(path, p, (size_t)np * 4, NULL, 0); }
    }
    if (gpu) TRY(pc_sync());
    double startup_s = now() - t0;
    /* Programs: one step before its decision, the update, the eval pass. */
    int64_t planned = cg_step_launches(s, plan);
    coax_t_CgLaunch *prog = malloc(sizeof *prog * (size_t)planned), upd[1], *ev = malloc(sizeof *ev * (size_t)planned);
    NEED(cg_step(s, plan, (coax_v_t_CgLaunch){prog, planned}) == planned, "program size differs");
    snprintf(path, sizeof path, "%s/log.jsonl", out); FILE *log = fopen(path, "a"); NEED(log, "cannot open log");
    int64_t offs[4096]; float *losses = malloc(sizeof(float) * (size_t)(s.rows * s.context));
    int64_t windows = cg_windows(s, heldout);
    /* The held-out loss over every window, in rows-sized batches. */
    #define EVAL() do { double sum_ = 0; int64_t done_ = 0; double e0_ = now(); \
        for (int64_t w_ = 0; w_ < windows; w_ += s.rows) { int64_t r_ = windows - w_ < s.rows ? windows - w_ : s.rows; \
            cg_eval_offsets(s, train, w_, r_, (coax_v_s64){offs, r_}); up(CG_OFFS_ID, 0, offs, r_); \
            int64_t c_ = cg_eval(s, plan, (coax_v_t_CgLaunch){ev, planned}, r_); run(ev, c_); \
            down(losses, CG_LOSS_ID, 0, r_ * s.context); sum_ += cg_loss_sum((coax_v_f32){losses, r_ * s.context}); done_ += r_ * s.context; } \
        fprintf(log, "{\"step\":%lld,\"held_out\":%.17g,\"predictions\":%lld,\"seconds\":%.3f}\n", (long long)step, sum_ / (double)done_, (long long)done_, now() - e0_); fflush(log); \
        fprintf(stderr, "step %lld held-out %.6f\n", (long long)step, sum_ / (double)done_); } while (0)
    if (step == 0 && !dump) EVAL();
    double train_s = 0, begin = now(); int64_t steps_here = 0, clipped = 0;
    FILE *orc = NULL;
    if (dump) { snprintf(path, sizeof path, "%s/oracle.json", out); orc = fopen(path, "w"); fprintf(orc, "{\"steps\":["); }
    while (step < target && now() - begin < budget && (!dump || step < dump)) {
        double ts = now(); int64_t up_bytes = 0, down_bytes = 0, l0 = launches_done;
        cg_rows(s, step, train, (coax_v_s64){offs, rows}); up(CG_OFFS_ID, 0, offs, rows); up_bytes += rows * 8;
        run(prog, planned);
        uint32_t two[2]; down(two, CG_SCAL_ID, 0, 2); down_bytes += 8;
        NEED(cgp_ledger(rows, up_bytes, down_bytes, launches_done - l0, planned) == COAX_CgStatus_ok, "ledger refused: a step moved data or launched work outside its plan");
        coax_t_CgDecision d = cgp_decide(step, cg_steps(), cg_warmup(), tokens, two[0], two[1]);
        if (dump && step == 0) {
            double *g = malloc((size_t)np * 8); float *gf = malloc((size_t)np * 4); down(gf, CG_G_ID, 0, np);
            for (int64_t i = 0; i < np; i++) g[i] = gf[i];
            snprintf(path, sizeof path, "%s/grad1.f64", out); write_all(path, g, (size_t)np * 8, NULL, 0); free(g); free(gf);
        }
        if (d.apply) { NEED(cg_update(plan, d, (coax_v_t_CgLaunch){upd, 1}) == 1, "update program"); run(upd, 1); }
        else { skipped++; NEED(cgp_skips(skipped) == COAX_CgStatus_ok, "more than ten skipped steps"); }
        if (gpu) TRY(pc_sync());
        double ms = (now() - ts) * 1e3; train_s += ms / 1e3; steps_here++; clipped += d.clipped;
        step++;
        fprintf(log, "{\"step\":%lld,\"loss\":%.9g,\"norm\":%.9g,\"rate\":%.9g,\"clipped\":%d,\"applied\":%d,\"ms\":%.3f}\n", (long long)step, d.loss, (double)d.norm, d.rate, d.clipped, d.apply, ms);
        if (orc) fprintf(orc, "%s{\"loss\":%.17g,\"norm\":%.17g,\"applied\":%s,\"rate\":%.17g}", step > 1 ? "," : "", d.loss, (double)d.norm, d.apply ? "true" : "false", d.rate);
        if (step % every == 0 || step == cg_steps()) { fflush(log); if (!dump) EVAL(); }
    }
    down(p, CG_P_ID, 0, np); down(p + np, CG_M_ID, 0, np); down(p + 2 * np, CG_V_ID, 0, np);
    if (orc) {
        fprintf(orc, "]}\n"); fclose(orc);
        double *g = malloc((size_t)np * 8); for (int64_t i = 0; i < np; i++) g[i] = p[i];
        snprintf(path, sizeof path, "%s/params.f64", out); write_all(path, g, (size_t)np * 8, NULL, 0); free(g);
    } else {
        int64_t h[16]; uint64_t fnv = cg_fnv_f32(0xcbf29ce484222325ULL, (coax_v_f32){p, np * 3});
        NEED(cgp_header((coax_v_s64){h, 16}, 0, np, step, skipped, (coax_v_s64){shape, 8}, fnv) == COAX_CgStatus_ok, "header");
        snprintf(path, sizeof path, "%s/state.ckpt", out); write_all(path, h, 128, p, (size_t)np * 12);
    }
    fclose(log);
    printf("{\"device\":\"%s\",\"step\":%lld,\"steps_here\":%lld,\"skipped\":%lld,\"clipped_here\":%lld,\"train_seconds\":%.3f,\"tokens_per_second\":%.1f,"
           "\"launches_per_step\":%lld,\"compile_seconds\":%.3f,\"startup_seconds\":%.3f,\"plan_bytes\":%lld,\"driver_free_drop_bytes\":%lld,\"wall_seconds\":%.3f}\n",
           gpu ? "gpu" : "cpu", (long long)step, (long long)steps_here, (long long)skipped, (long long)clipped, train_s, steps_here ? (double)(steps_here * tokens) / train_s : 0.0,
           (long long)planned, compile_s, startup_s, (long long)plan.used, (long long)(free0 - free1), now() - t0);
    return 0;
}
