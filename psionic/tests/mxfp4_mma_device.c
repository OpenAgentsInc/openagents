/* Chapter 69 on a CUDA device: the MXFP4-BF16-MMA-v1 expert products of
   psionic/lib/device/mxfp4-mma.coax against the binary64 reference and
   chapter 66's gpt-oss-cuda-f32-v1 kernels, at gpt-oss-20b expert shapes.
   The plan, device admission, the reference, the frozen bound and chapter
   66's rule are Coax (mxfp4-mma-plan.coax); this file reports the device,
   reads bytes, moves them, launches and times. Prints SKIP without a CUDA
   device or NVRTC.
   Usage: mxfp4-mma-device GENERATED.cu [MODEL.safetensors MLP1_BLOCKS
          MLP1_SCALES MLP1_BIAS MLP2_BLOCKS MLP2_SCALES MLP2_BIAS]
   where the six numbers are absolute byte offsets of layer 0's tensors. */
#define _POSIX_C_SOURCE 200809L
#include "mxfp4-mma-plan.h"
#include "cuda-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
enum { EXPERTS = 32, COLS = 2880, SLOTS = 4, REPEAT = 20 };
static const int64_t IDS[SLOTS] = {3, 11, 19, 30};

static uint64_t seed = 0x9e3779b97f4a7c15ULL;
static uint64_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return seed; }
static float uniform(void) { return (float)((next() >> 11) % 2000001) / 1000000.0f - 1.0f; }

typedef struct { const char *name; int64_t rows; uint8_t *packed, *scales; uint16_t *bias; PcBuffer dp, ds, db; } Weights;
static PcDeviceInfo info;
static PcKernel k_split, k_part, k_finish, k_gpart, k_gfinish, k_gbatch, k_gprefix;
static int failures;

static PcBuffer upload(const void *host, size_t bytes) { PcBuffer b; TRY(pc_alloc(bytes, &b)); TRY(pc_upload(b, host, bytes, NULL)); TRY(pc_sync()); return b; }
static PcBuffer zeros(size_t bytes) { PcBuffer b; TRY(pc_alloc(bytes, &b)); TRY(pc_fill(b, 0, bytes / 4, NULL)); return b; }
static PcArg V(PcBuffer b, int64_t n) { return (PcArg){PC_ARG_VIEW, 0, 0, b, n}; }
static PcArg I(int64_t v) { return (PcArg){PC_ARG_INT, v, 0, 0, 0}; }

typedef struct { PcKernel k; int64_t items; int block; int warp; PcArg a[16]; int n; } Launch;
static void go(const Launch *l) {
    if (l->warp) TRY(pc_launch_warps(l->k, l->items, l->block, l->a, l->n, NULL));
    else TRY(pc_launch(l->k, l->items, l->block, l->a, l->n, NULL));
}
/* Mean milliseconds of REPEAT runs of the launches, after one warm-up. */
static double timed(const Launch *ls, int count) {
    for (int i = 0; i < count; i++) go(&ls[i]);
    PcEvent a, b; TRY(pc_event(&a)); TRY(pc_event(&b)); TRY(pc_record(a, NULL));
    for (int r = 0; r < REPEAT; r++) for (int i = 0; i < count; i++) go(&ls[i]);
    TRY(pc_record(b, NULL)); TRY(pc_sync());
    double t = 0; TRY(pc_event_seconds(a, b, &t)); pc_destroy_event(a); pc_destroy_event(b);
    return t * 1e3 / REPEAT;
}

static void load(Weights *w) {
    int64_t blocks = EXPERTS * w->rows * (COLS / 32);
    w->dp = upload(w->packed, (size_t)blocks * 16); w->ds = upload(w->scales, (size_t)blocks);
    w->db = upload(w->bias, (size_t)EXPERTS * w->rows * 2);
}
static void unload(Weights *w) { pc_free(w->dp); pc_free(w->ds); pc_free(w->db); }

/* One product: SLOTS selected experts, `tokens` tokens per slot. shared:
   every slot reads one input (the first projection); otherwise each slot
   has its own (the second). check_every: compare every n-th row. */
static void product(Weights *w, int tokens, int shared, const int *slice_list, int slice_count, int check_every) {
    int64_t rows = w->rows, stride = shared ? 0 : (int64_t)tokens * COLS, xn = (int64_t)SLOTS * tokens * COLS;
    float *x = malloc((size_t)xn * 4);
    for (int64_t i = 0; i < xn; i++) x[i] = uniform();
    PcBuffer dx = upload(x, (size_t)xn * 4), dids = upload(IDS, sizeof IDS);
    int64_t outs = (int64_t)SLOTS * tokens * rows;
    float *got = malloc((size_t)outs * 4); uint8_t *flags = malloc((size_t)outs);
    coax_v_u8 vp = {w->packed, EXPERTS * rows * (COLS / 32) * 16}, vs = {w->scales, EXPERTS * rows * (COLS / 32)};
    coax_v_u16 vb = {w->bias, EXPERTS * rows}; coax_v_f32 vx = {x, xn};
    printf("%s %lldx%d, %d experts of %d, %d token%s per slot, %s input\n", w->name, (long long)rows, COLS, SLOTS, EXPERTS,
           tokens, tokens == 1 ? "" : "s", shared ? "shared" : "per-slot");
    double best_total = 0, best_part = 0; int best_slices = 0;
    for (int si = 0; si < slice_count; si++) {
        int slices = slice_list[si]; int64_t plan[4];
        int64_t status = mm_plan_c(rows, COLS, tokens, SLOTS, slices, info.major, info.minor, info.max_threads, info.max_shared, (coax_v_s64){plan, 4});
        if (status != 0) { printf("  plan refused: status %lld\n", (long long)status); failures++; continue; }
        int64_t padded = plan[0] / (SLOTS * COLS / 2), partials = (int64_t)slices * outs;
        PcBuffer hi = zeros((size_t)plan[0] * 4), lo = zeros((size_t)plan[0] * 4), part = zeros((size_t)partials * 4),
                 marks = zeros((size_t)(partials + 3) / 4 * 4), out = zeros((size_t)outs * 4), fl = zeros((size_t)(outs + 3) / 4 * 4);
        Launch ls[3] = {
            {k_split, plan[0], 256, 0, {I(COLS), I(tokens), I(padded), I(stride), V(dx, xn), V(hi, plan[0]), V(lo, plan[0])}, 7},
            {k_part, plan[1], 128, 1, {I(rows), I(COLS), I(tokens), I(SLOTS), I(slices), V(w->dp, vp.count), V(w->ds, vs.count), V(dids, SLOTS),
                                       V(hi, plan[0]), V(lo, plan[0]), V(part, partials), V(marks, partials)}, 12},
            {k_finish, plan[2], 256, 0, {I(rows), I(tokens), I(SLOTS), I(slices), V(dids, SLOTS), V(part, partials), V(marks, partials),
                                         V(w->db, vb.count), V(out, outs), V(fl, outs)}, 10}};
        double total = timed(ls, 3), only = timed(&ls[1], 1);
        TRY(pc_download(got, out, (size_t)outs * 4, NULL)); TRY(pc_download(flags, fl, (size_t)outs, NULL)); TRY(pc_sync());
        double worst = 0, bound = 0, e[2]; int64_t over = 0, flagged = 0, checked = 0;
        for (int64_t i = 0; i < outs; i++) {
            int64_t row = i % rows, token = i / rows % tokens, slot = i / rows / tokens;
            flagged += flags[i] != 0;
            if (row % check_every) continue;
            e[0] = e[1] = 0; checked++;
            over += !mm_check_c(rows, COLS, stride, IDS[slot], row, slot, token, plan[3], got[i], vp, vs, vb, vx, (coax_v_f64){e, 2});
            if (e[0] > worst) worst = e[0];
            bound = e[1];
        }
        printf("  MXFP4-BF16-MMA-v1, %2d slice%s: %lld outputs checked, %lld over the bound, %lld flagged; worst %.2e of sum |w x| (bound %.2e); split+part+finish %.3f ms, part %.3f ms\n",
               slices, slices == 1 ? " " : "s", (long long)checked, (long long)over, (long long)flagged, worst, bound, total, only);
        failures += over != 0 || flagged != 0;
        if (best_slices == 0 || total < best_total) { best_total = total; best_part = only; best_slices = slices; }
        pc_free(hi); pc_free(lo); pc_free(part); pc_free(marks); pc_free(out); pc_free(fl);
    }
    /* Chapter 66's kernels on the same inputs. */
    const int64_t lanes = 16;
    if (tokens == 1) {
        int64_t partials = SLOTS * lanes * rows;
        PcBuffer part = zeros((size_t)partials * 4), out = zeros((size_t)outs * 4), fl = zeros((size_t)(outs + 3) / 4 * 4);
        Launch ls[2] = {
            {k_gpart, SLOTS * rows * lanes, 128, 0, {I(rows), I(COLS), I(lanes), I(stride), V(w->dp, vp.count / 8), V(w->ds, vs.count), V(dids, SLOTS), V(dx, xn), V(part, partials)}, 9},
            {k_gfinish, SLOTS * rows, 256, 0, {I(rows), I(lanes), V(dids, SLOTS), V(part, partials), V(w->db, vb.count), V(out, outs), V(fl, outs)}, 7}};
        double total = timed(ls, 2), only = timed(&ls[0], 1);
        float *ref = malloc((size_t)outs * 4);
        TRY(pc_download(ref, out, (size_t)outs * 4, NULL)); TRY(pc_sync());
        /* Rerun the best MMA plan's outputs into got for the rule. */
        double worst = 0, e[2]; int64_t fails = 0;
        for (int64_t i = 0; i < outs; i++) {
            int64_t row = i % rows, slot = i / rows;
            fails += !mm_rule_c(got[i], ref[i]);
            if (row % check_every) continue;
            mm_check_c(rows, COLS, stride, IDS[slot], row, slot, 0, 2, ref[i], vp, vs, vb, vx, (coax_v_f64){e, 2});
            if (e[0] > worst) worst = e[0];
        }
        printf("  gpt-oss-cuda-f32-v1: worst %.2e of sum |w x|; part+finish %.3f ms, part %.3f ms; chapter 66's rule against the last MMA run: %lld of %lld fail\n",
               worst, total, only, (long long)fails, (long long)outs);
        printf("  receipt: decode %s, f32 %.3f ms, MMA (%d slices) %.3f ms, speedup %.2fx\n", w->name, total, best_slices, best_total, total / best_total);
        failures += fails != 0;
        free(ref); pc_free(part); pc_free(out); pc_free(fl);
    } else {
        /* Chapter 67's batched product (timing only): each selected expert
           takes its slot's tokens, as gk_group would map them. */
        int64_t n = tokens, z = SLOTS, *map = calloc((size_t)(EXPERTS * n), 8), counts[2 * EXPERTS + 1] = {0};
        for (int s = 0; s < SLOTS; s++) { counts[IDS[s]] = n; for (int64_t j = 0; j < n; j++) map[IDS[s] * n + j] = j * z + s; }
        float *xb = malloc((size_t)(n * z * COLS) * 4);
        for (int64_t j = 0; j < n; j++) for (int s = 0; s < SLOTS; s++) memcpy(xb + (j * z + s) * COLS, x + (int64_t)s * stride + j * COLS, COLS * 4);
        PcBuffer dmap = upload(map, (size_t)(EXPERTS * n) * 8), dcounts = upload(counts, sizeof counts), dxb = upload(xb, (size_t)(n * z * COLS) * 4);
        int64_t partials = n * z * lanes * rows; PcBuffer part = zeros((size_t)partials * 4);
        PcArg pa[] = {I(EXPERTS), V(dcounts, 2 * EXPERTS + 1)}; TRY(pc_launch(k_gprefix, 1, 256, pa, 2, NULL));
        Launch l = {k_gbatch, ((n * z + 7) / 8 + EXPERTS) * (rows / 4) * lanes, 128, 0,
                    {I(rows), I(COLS), I(n), I(EXPERTS), I(z), I(lanes), I(1), V(w->dp, vp.count / 8), V(w->ds, vs.count), V(dmap, EXPERTS * n),
                     V(dcounts, 2 * EXPERTS + 1), V(dxb, n * z * COLS), V(part, partials)}, 13};
        double only = timed(&l, 1);
        printf("  gpt-oss-cuda-f32-v1 batched (gk_batch_mx_part, lanes not yet summed): part %.3f ms\n", only);
        printf("  receipt: batched %s, f32 part %.3f ms, MMA (%d slices) part %.3f ms, speedup %.2fx; MMA split+part+finish %.3f ms\n",
               w->name, only, best_slices, best_part, only / best_part, best_total);
        free(map); free(xb); pc_free(dmap); pc_free(dcounts); pc_free(dxb); pc_free(part);
    }
    free(x); free(got); free(flags); pc_free(dx); pc_free(dids);
}

static void read_at(FILE *f, long long offset, void *dst, size_t bytes) {
    if (fseeko(f, (off_t)offset, SEEK_SET) || fread(dst, 1, bytes, f) != bytes) { fprintf(stderr, "short read at %lld\n", offset); exit(2); }
}
static Weights alloc_weights(const char *name, int64_t rows) {
    Weights w = {name, rows, malloc((size_t)(EXPERTS * rows * COLS / 2)), malloc((size_t)(EXPERTS * rows * COLS / 32)), malloc((size_t)(EXPERTS * rows) * 2), 0, 0, 0};
    return w;
}
static void free_weights(Weights *w) { free(w->packed); free(w->scales); free(w->bias); }

int main(int argc, char **argv) {
    if (argc != 2 && argc != 9) { fprintf(stderr, "usage: mxfp4-mma-device GENERATED.cu [MODEL OFFSETS...]\n"); return 2; }
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    int64_t plan[4];
    int64_t here = mm_plan_c(5760, COLS, 1, SLOTS, 4, info.major, info.minor, info.max_threads, info.max_shared, (coax_v_s64){plan, 4});
    int64_t old = mm_plan_c(5760, COLS, 1, SLOTS, 4, 7, 5, 1024, 49152, (coax_v_s64){plan, 4});
    printf("device: %s (sm_%d%d): MXFP4-BF16-MMA-v1 %s\n", info.name, info.major, info.minor, here == 0 ? "admitted" : "refused");
    printf("a reported sm_75: status %lld (4 + 16*5: needs compute capability 8.0)\n", (long long)old);
    failures += old != 4 + 16 * 5;
    if (here != 0) { printf("refused on this device: status %lld\n", (long long)here); return 1; }

    FILE *f = fopen(argv[1], "rb"); if (!f) { perror(argv[1]); return 2; }
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *src = malloc((size_t)size + 1); if (fread(src, 1, (size_t)size, f) != (size_t)size) return 2; fclose(f); src[size] = 0;
    PcModule module; double seconds; TRY(pc_compile(src, (size_t)size, "mxfp4-mma.cu", &module, &seconds)); free(src);
    TRY(pc_kernel(module, "mm_split", &k_split)); TRY(pc_kernel(module, "mm_part", &k_part)); TRY(pc_kernel(module, "mm_finish", &k_finish));
    TRY(pc_kernel(module, "gk_mx_part", &k_gpart)); TRY(pc_kernel(module, "gk_mx_finish", &k_gfinish));
    TRY(pc_kernel(module, "gk_batch_mx_part", &k_gbatch)); TRY(pc_kernel(module, "gk_group_prefix", &k_gprefix));

    const int decode_slices[] = {1, 2, 4, 6, 12}, batch_slices[] = {1, 2, 4};
    /* Synthetic weights at the first projection's shape: random codes, scales
       118 to 125, biases within +-1/2. */
    Weights s = alloc_weights("synthetic mlp1", 2 * COLS);
    for (int64_t i = 0; i < EXPERTS * s.rows * COLS / 2; i++) s.packed[i] = (uint8_t)next();
    for (int64_t i = 0; i < EXPERTS * s.rows * COLS / 32; i++) s.scales[i] = (uint8_t)(118 + next() % 8);
    for (int64_t i = 0; i < EXPERTS * s.rows; i++) { float b = uniform() * 0.5f; uint32_t u; memcpy(&u, &b, 4); s.bias[i] = (uint16_t)(u >> 16); }
    load(&s);
    product(&s, 1, 1, decode_slices, 5, 1);
    product(&s, 32, 0, batch_slices, 3, 7);
    unload(&s); free_weights(&s);

    if (argc == 9) {
        FILE *m = fopen(argv[2], "rb"); if (!m) { perror(argv[2]); return 2; }
        const char *names[2] = {"original layer 0 mlp1", "original layer 0 mlp2"};
        for (int p = 0; p < 2; p++) {
            Weights w = alloc_weights(names[p], p == 0 ? 2 * COLS : COLS);
            read_at(m, atoll(argv[3 + 3 * p]), w.packed, (size_t)(EXPERTS * w.rows * COLS / 2));
            read_at(m, atoll(argv[4 + 3 * p]), w.scales, (size_t)(EXPERTS * w.rows * COLS / 32));
            read_at(m, atoll(argv[5 + 3 * p]), w.bias, (size_t)(EXPERTS * w.rows) * 2);
            load(&w);
            product(&w, 1, p == 0, decode_slices, 5, 1);
            if (p == 0) product(&w, 32, 0, batch_slices, 3, 7);
            unload(&w); free_weights(&w);
        }
        fclose(m);
    }
    pc_unload(module);
    if (failures) { printf("FAIL: %d checks\n", failures); return 1; }
    printf("chapter 69 on CUDA: admitted, every checked output within the frozen bound, chapter 66's rule held\n");
    return 0;
}
