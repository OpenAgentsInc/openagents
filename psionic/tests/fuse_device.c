/* Chapter 43 on a CUDA device: the step of psionic/lib/device/fuse.coax,
   planned in Coax, run four ways (unfused launches, fused launches, and both
   recorded once as a device graph and replayed), each compared bit for bit
   with the unfused step run on the CPU from the same Coax compiled to C.
   The plan, the fusion decisions and the replay-or-record decision are all
   Coax; this file only allocates, launches, records and times. Usage:
   fuse-device GENERATED.cu. Prints SKIP without a CUDA device or NVRTC. */
#define _POSIX_C_SOURCE 199309L
#include "fuse.h"
#include "cuda-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
#define COLS 2880
#define MAX_ROWS 512
#define EPS 0.00001f

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
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec * 1e-9; }

static const char *kernel_names[] = {"fk_rms", "fk_scale", "fk_bias", "fk_relu", "fk_add", "fk_norm_scale", "fk_bias_relu_add"};
static PcKernel kernels[7];
static PcBuffer dev[4];
static float *host[4];
static int64_t sizes[4];

static PcArg view(coax_t_FuSpan s) {
    if (s.count == 0) return (PcArg){PC_ARG_VIEW, 0, 0, 0, 0};
    return (PcArg){PC_ARG_VIEW, 0, 0, dev[s.buffer] + (PcBuffer)s.offset * 4, s.count};
}
static void args_of(const coax_t_FuOp *op, PcArg *a) {
    a[0] = (PcArg){PC_ARG_INT, COLS, 0, 0, 0};
    a[1] = (PcArg){PC_ARG_F32, 0, EPS, 0, 0};
    a[2] = view(op->a); a[3] = view(op->b); a[4] = view(op->c); a[5] = view(op->out);
}
static coax_v_f32 cpu_view(float **buffers, coax_t_FuSpan s) { return (coax_v_f32){s.count ? buffers[s.buffer] + s.offset : NULL, s.count}; }
/* The CPU oracle: the plan's launches as loops over the C build. */
static void run_cpu(const coax_t_FuOp *ops, int64_t count, int64_t rows, float **b) {
    typedef void (*K)(int64_t, int64_t, float, coax_v_f32, coax_v_f32, coax_v_f32, coax_v_f32);
    K fns[] = {fk_rms, fk_scale, fk_bias, fk_relu, fk_add, fk_norm_scale, fk_bias_relu_add};
    for (int64_t i = 0; i < count; i++) {
        const coax_t_FuOp *op = &ops[i];
        for (int64_t item = 0; item < fu_items(op->kind, rows, COLS); item++)
            fns[op->kind](item, COLS, EPS, cpu_view(b, op->a), cpu_view(b, op->b), cpu_view(b, op->c), cpu_view(b, op->out));
    }
}
static void enqueue(const coax_t_FuOp *ops, int64_t count, int64_t rows) {
    for (int64_t i = 0; i < count; i++) {
        PcArg a[6]; args_of(&ops[i], a);
        TRY(pc_launch(kernels[ops[i].kind], fu_threads(ops[i].kind, rows, COLS), (int)fu_block(ops[i].kind), a, 6, NULL));
    }
}
static PcProgram record(const coax_t_FuOp *ops, int64_t count, int64_t rows) {
    PcProgram p; TRY(pc_program_create((size_t)count, &p));
    for (int64_t i = 0; i < count; i++) {
        PcArg a[6]; args_of(&ops[i], a);
        TRY(pc_program_add(p, kernels[ops[i].kind], fu_threads(ops[i].kind, rows, COLS), (int)fu_block(ops[i].kind), a, 6));
    }
    TRY(pc_program_ready(p));
    return p;
}
/* New arguments for every node of a recorded step: the item counts and the
   span offsets all move with the row count. */
static void patch(PcProgram p, const coax_t_FuOp *ops, int64_t count, int64_t rows) {
    for (int64_t i = 0; i < count; i++) {
        PcArg a[6]; args_of(&ops[i], a);
        TRY(pc_program_set(p, (size_t)i, kernels[ops[i].kind], fu_threads(ops[i].kind, rows, COLS), (int)fu_block(ops[i].kind), a, 6));
    }
}
/* Fill the output buffer with a NaN pattern, so stale values cannot pass. */
static void clear(void) { TRY(pc_fill(dev[3], 0x7fc00001u, (size_t)sizes[3], NULL)); TRY(pc_fill(dev[2], 0x7fc00001u, (size_t)sizes[2], NULL)); }
/* Elements of the output (and of y) that differ from the CPU, bit for bit. */
static int64_t compare(int64_t rows, float **ref) {
    int64_t n = rows * COLS, d = 0;
    TRY(pc_sync());
    TRY(pc_download(host[3], dev[3], (size_t)(4 * n) * 4, NULL));
    for (int64_t i = 0; i < n; i++) {
        if (memcmp(&host[3][3 * n + i], &ref[3][3 * n + i], 4)) d++;
        if (memcmp(&host[3][i], &ref[3][i], 4)) d++;
    }
    return d;
}

int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: fuse-device GENERATED.cu\n"); return 2; }
    PcDeviceInfo info;
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    printf("device: %s (sm_%d%d)\n", info.name, info.major, info.minor);
    size_t n; char *src = read_all(argv[1], &n); double seconds = 0;
    PcModule module; TRY(pc_compile(src, n, "fuse.cu", &module, &seconds));
    for (int k = 0; k < 7; k++) TRY(pc_kernel(module, kernel_names[k], &kernels[k]));

    int64_t max[4]; fu_sizes(MAX_ROWS, COLS, (coax_v_s64){max, 4});
    float *ref[4];
    for (int b = 0; b < 4; b++) {
        sizes[b] = max[b];
        TRY(pc_alloc((size_t)max[b] * 4, &dev[b]));
        host[b] = calloc((size_t)max[b], 4); ref[b] = calloc((size_t)max[b], 4);
    }
    int failures = 0;
    int64_t rows_list[] = {1, 7, 128, 512, 1};
    coax_t_FuOp ops[5], fused[5], recorded[5]; coax_t_FuDecision dec[2];
    int64_t recorded_count = 0; PcProgram fused_graph = NULL, unfused_graph = NULL;
    for (size_t r = 0; r < sizeof rows_list / sizeof *rows_list; r++) {
        int64_t rows = rows_list[r], nn = rows * COLS, buffers[4];
        fu_step(rows, COLS, (coax_v_t_FuOp){ops, 5});
        fu_sizes(rows, COLS, (coax_v_s64){buffers, 4});
        coax_t_FuPlan plan = fu_plan((coax_v_t_FuOp){ops, 5}, rows, COLS, (coax_v_s64){buffers, 4}, (coax_v_t_FuOp){fused, 5}, (coax_v_t_FuDecision){dec, 2});
        if (plan.status != COAX_FuStatus_ok || plan.launches != 2) { printf("FAIL: plan status %d, %lld launches\n", plan.status, (long long)plan.launches); return 1; }
        /* Inputs: x and the residual in buffer 0, gain and bias in buffer 1. */
        for (int64_t i = 0; i < 2 * nn; i++) host[0][i] = ref[0][i] = uniform();
        for (int64_t i = 0; i < 2 * COLS; i++) host[1][i] = ref[1][i] = uniform();
        memset(ref[3], 0, (size_t)(4 * nn) * 4);
        TRY(pc_upload(dev[0], host[0], (size_t)(2 * nn) * 4, NULL));
        TRY(pc_upload(dev[1], host[1], (size_t)(2 * COLS) * 4, NULL));
        run_cpu(ops, 5, rows, ref);

        clear(); enqueue(ops, 5, rows); int64_t d_unfused = compare(rows, ref);
        clear(); enqueue(fused, 2, rows); int64_t d_fused = compare(rows, ref);
        /* Record once, at the first row count; afterwards Coax decides whether
           the recorded kernels still match, and every node is patched. */
        const char *how;
        if (fused_graph && fu_same_kernels((coax_v_t_FuOp){recorded, recorded_count}, (coax_v_t_FuOp){fused, plan.launches})) {
            patch(fused_graph, fused, 2, rows); patch(unfused_graph, ops, 5, rows); how = "patched";
        } else {
            if (fused_graph) { pc_program_free(fused_graph); pc_program_free(unfused_graph); }
            fused_graph = record(fused, 2, rows); unfused_graph = record(ops, 5, rows);
            memcpy(recorded, fused, sizeof fused); recorded_count = plan.launches; how = "recorded";
        }
        clear(); TRY(pc_program_replay(fused_graph, NULL)); int64_t d_replay = compare(rows, ref);
        clear(); TRY(pc_program_replay(unfused_graph, NULL)); int64_t d_replay_unfused = compare(rows, ref);
        printf("%4lld rows x %d: differ from the CPU: unfused 5 launches %lld, fused 2 launches %lld, "
               "graph %s: fused %lld, unfused %lld\n",
               (long long)rows, COLS, (long long)d_unfused, (long long)d_fused, how, (long long)d_replay, (long long)d_replay_unfused);
        failures += d_unfused != 0 || d_fused != 0 || d_replay != 0 || d_replay_unfused != 0;
    }

    /* A layout Coax keeps apart: the output one element along from y. The
       unfused fallback must still match the CPU. */
    {
        int64_t rows = 7, nn = rows * COLS, buffers[4];
        fu_step(rows, COLS, (coax_v_t_FuOp){ops, 5});
        fu_sizes(rows, COLS, (coax_v_s64){buffers, 4});
        ops[4].out = (coax_t_FuSpan){3, 1, nn};
        coax_t_FuPlan plan = fu_plan((coax_v_t_FuOp){ops, 5}, rows, COLS, (coax_v_s64){buffers, 4}, (coax_v_t_FuOp){fused, 5}, (coax_v_t_FuDecision){dec, 2});
        memset(ref[3], 0, (size_t)(4 * nn) * 4);
        run_cpu(ops, 5, rows, ref);
        clear(); enqueue(fused, plan.launches, rows);
        TRY(pc_sync()); TRY(pc_download(host[3], dev[3], (size_t)(nn + 1) * 4, NULL));
        int64_t d = 0;
        for (int64_t i = 1; i < nn + 1; i++) if (memcmp(&host[3][i], &ref[3][i], 4)) d++;
        printf("output over y: %lld launches (decision %d on ops %lld-%lld), %lld differ from the CPU\n",
               (long long)plan.launches, dec[1].why, (long long)dec[1].first, (long long)(dec[1].first + dec[1].ops - 1), (long long)d);
        failures += plan.status != COAX_FuStatus_ok || plan.launches != 4 || dec[1].why != COAX_FuWhy_overlap || d != 0;
    }

    /* Two timings, each over 2000 steps after 50 warm-up steps, for a decode
       step (1 row) and a prefill chunk (128 rows). Host enqueue: the device
       is idle before each step (the wait is not timed), so this is the host
       cost of submitting one step. Device: steps back to back, between two
       events, so this is the time per step including gaps between kernels. */
    int64_t time_rows[] = {1, 128};
    for (size_t t = 0; t < 2; t++) {
        int64_t rows = time_rows[t], buffers[4];
        fu_step(rows, COLS, (coax_v_t_FuOp){ops, 5});
        fu_sizes(rows, COLS, (coax_v_s64){buffers, 4});
        fu_plan((coax_v_t_FuOp){ops, 5}, rows, COLS, (coax_v_s64){buffers, 4}, (coax_v_t_FuOp){fused, 5}, (coax_v_t_FuDecision){dec, 2});
        patch(fused_graph, fused, 2, rows); patch(unfused_graph, ops, 5, rows);
        const char *names[] = {"unfused, 5 launches", "fused, 2 launches", "unfused graph replay", "fused graph replay"};
        for (int way = 0; way < 4; way++) {
            int steps = 2000;
#define STEP() do { if (way == 0) enqueue(ops, 5, rows); else if (way == 1) enqueue(fused, 2, rows); \
                     else TRY(pc_program_replay(way == 2 ? unfused_graph : fused_graph, NULL)); } while (0)
            for (int w = 0; w < 50; w++) STEP();
            double host_s = 0;
            for (int s = 0; s < steps; s++) {
                TRY(pc_sync());
                double t0 = now(); STEP(); host_s += now() - t0;
            }
            TRY(pc_sync());
            PcEvent a, b; TRY(pc_event(&a)); TRY(pc_event(&b));
            TRY(pc_record(a, NULL));
            for (int s = 0; s < steps; s++) STEP();
            TRY(pc_record(b, NULL)); TRY(pc_sync());
            double dev_s = 0; TRY(pc_event_seconds(a, b, &dev_s));
            pc_destroy_event(a); pc_destroy_event(b);
#undef STEP
            printf("time %4lld rows %-21s host enqueue %5.2f us/step, device %6.2f us/step\n",
                   (long long)rows, names[way], host_s / steps * 1e6, dev_s / steps * 1e6);
        }
    }
    pc_program_free(fused_graph); pc_program_free(unfused_graph);
    for (int b = 0; b < 4; b++) { pc_free(dev[b]); free(host[b]); free(ref[b]); }
    if (failures) { printf("FAIL: %d checks\n", failures); return 1; }
    printf("chapter 43 on CUDA: fused, unfused and replayed steps are bit-identical to the CPU\n");
    return 0;
}
