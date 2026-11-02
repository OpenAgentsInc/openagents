/* Firm reason: 5 (thin wrapper with no logic: fills buffers, moves bytes, launches and times; plans, admission, the oracle and the statistics are Coax) */
/* Chapter 48 on a CUDA device: the int8-block-i32-v1 kernels of
   psionic/lib/device/int8-kernels.coax against chapter 48's CPU quantizer
   and kernels (int8_quantize, int8_row, int8_tiled), bit for bit, then timed
   against chapter 37's f32 matrix-vector and projection kernels and its BF16
   tensor-core projection. The plan, device and input admission, the naive
   oracle and chapter 34's statistics are Coax (int8-device.coax); this file
   reports the device, fills buffers, moves bytes, launches and times.
   Prints SKIP without a CUDA device or NVRTC.
   Usage: int8-device INT8.cu PROJECT.cu WARP.cu */
#define _POSIX_C_SOURCE 200809L
#include "int8-device.h"
#include "cuda-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s: %s\n", #x, pc_log()); exit(1); } } while (0)
enum { RUNS = 20, BLOCK = 32, DIM = 8192, DV_LANES = 16, DW_TOKENS = 88 };

static uint64_t seed = 0x9e3779b97f4a7c15ULL;
static uint64_t next(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return seed; }
static float uniform(void) { return (float)((next() >> 11) % 2000001) / 1000000.0f - 1.0f; }
static int8_t code(void) { return (int8_t)((int)(next() % 255) - 127); }

static PcDeviceInfo info;
static PcKernel k_quant, k_dots, k_finish, k_mma, k_matvec, k_reduce, k_project, k_split, k_wproject;
static int failures;

static PcBuffer upload(const void *host, size_t bytes) { PcBuffer b; TRY(pc_alloc(bytes, &b)); TRY(pc_upload(b, host, bytes, NULL)); TRY(pc_sync()); return b; }
static PcBuffer zeros(size_t bytes) { PcBuffer b; TRY(pc_alloc(bytes, &b)); TRY(pc_fill(b, 0, (bytes + 3) / 4, NULL)); return b; }
static void down(void *dst, PcBuffer b, size_t bytes) { TRY(pc_download(dst, b, bytes, NULL)); TRY(pc_sync()); }
static PcArg V(PcBuffer b, int64_t n) { return (PcArg){PC_ARG_VIEW, 0, 0, b, n}; }
static PcArg I(int64_t v) { return (PcArg){PC_ARG_INT, v, 0, 0, 0}; }

typedef struct { PcKernel k; int64_t items; int block; int warp; PcArg a[16]; int n; } Launch;
static void go(const Launch *l) {
    if (l->items == 0) return;
    if (l->warp) TRY(pc_launch_warps(l->k, l->items, l->block, l->a, l->n, NULL));
    else TRY(pc_launch(l->k, l->items, l->block, l->a, l->n, NULL));
}
/* One warm-up, then RUNS runs each timed alone, in nanoseconds. */
static void timed(const Launch *ls, int count, int64_t *ns) {
    for (int i = 0; i < count; i++) go(&ls[i]);
    TRY(pc_sync());
    PcEvent a, b; TRY(pc_event(&a)); TRY(pc_event(&b));
    for (int r = 0; r < RUNS; r++) {
        TRY(pc_record(a, NULL));
        for (int i = 0; i < count; i++) go(&ls[i]);
        TRY(pc_record(b, NULL)); TRY(pc_sync());
        double t = 0; TRY(pc_event_seconds(a, b, &t)); ns[r] = (int64_t)(t * 1e9 + 0.5);
    }
    pc_destroy_event(a); pc_destroy_event(b);
}
static int64_t plan(int kernel, int64_t rows, int64_t width, int64_t tokens, int64_t block, int64_t *out) {
    return i8_plan_c(kernel, rows, width, tokens, block, info.major, info.minor, info.max_threads, info.max_shared, (coax_v_s64){out, 6});
}

/* Codes, scales and offsets of a random weight matrix, and random activations. */
typedef struct { int64_t rows, width, tokens, block, nb; int8_t *wc; float *ws, *wo, *x; } Case;
static Case make(int64_t rows, int64_t width, int64_t tokens, int64_t block) {
    Case c = {rows, width, tokens, block, width / block, malloc((size_t)(rows * width)), malloc((size_t)(rows * width / block) * 4),
              malloc((size_t)(rows * width / block) * 4), malloc((size_t)(tokens * width) * 4)};
    for (int64_t i = 0; i < rows * width; i++) c.wc[i] = code();
    for (int64_t i = 0; i < rows * c.nb; i++) { c.ws[i] = (float)(1 + next() % 1000) / 65536.0f; c.wo[i] = uniform() / 64.0f; }
    /* Blocks of different sizes, so the scales differ. */
    for (int64_t i = 0; i < tokens * width; i++) c.x[i] = uniform() * (float)(1 + (i / block) % 7);
    return c;
}
static void unmake(Case *c) { free(c->wc); free(c->ws); free(c->wo); free(c->x); }

/* The device's quantize, row kernels and tile against the CPU, bit for bit. */
static void exact(int64_t rows, int64_t width, int64_t tokens, int64_t block) {
    Case c = make(rows, width, tokens, block);
    int64_t nb = c.nb, nd = rows * tokens * nb, ny = rows * tokens, pr[6], pm[6];
    int8_t *qc = malloc((size_t)(tokens * width)), *scratch = malloc((size_t)(rows * width));
    float *qs = malloc((size_t)(tokens * nb) * 4); int32_t *qm = malloc((size_t)(tokens * nb) * 4);
    int32_t *d1 = malloc((size_t)nd * 4), *d2 = malloc((size_t)nd * 4), *dd = malloc((size_t)nd * 4);
    double *y1 = malloc((size_t)ny * 8), *y2 = malloc((size_t)ny * 8), *yd = malloc((size_t)ny * 8);
    int64_t st = i8_cpu_quantize_c(rows, width, tokens, block, (coax_v_f32){c.x, tokens * width}, (coax_v_s8){qc, tokens * width},
                                   (coax_v_f32){qs, tokens * nb}, (coax_v_s32){qm, tokens * nb});
    st |= i8_cpu_c(0, rows, width, tokens, block, (coax_v_s8){c.wc, rows * width}, (coax_v_f32){c.ws, rows * nb}, (coax_v_f32){c.wo, rows * nb},
                   (coax_v_s8){qc, tokens * width}, (coax_v_f32){qs, tokens * nb}, (coax_v_s32){qm, tokens * nb}, (coax_v_s8){scratch, rows * width},
                   (coax_v_s32){d1, nd}, (coax_v_f64){y1, ny});
    int tiled = rows % 4 == 0 && tokens % 4 == 0, same = 1;
    if (tiled) {
        st |= i8_cpu_c(1, rows, width, tokens, block, (coax_v_s8){c.wc, rows * width}, (coax_v_f32){c.ws, rows * nb}, (coax_v_f32){c.wo, rows * nb},
                       (coax_v_s8){qc, tokens * width}, (coax_v_f32){qs, tokens * nb}, (coax_v_s32){qm, tokens * nb}, (coax_v_s8){scratch, rows * width},
                       (coax_v_s32){d2, nd}, (coax_v_f64){y2, ny});
        same = !memcmp(d1, d2, (size_t)nd * 4) && !memcmp(y1, y2, (size_t)ny * 8);
    }
    if (st != 0) { printf("  CPU refused %lldx%lld, %lld tokens: status %lld\n", (long long)rows, (long long)width, (long long)tokens, (long long)st); failures++; return; }
    int64_t sr = plan(0, rows, width, tokens, block, pr), sm = plan(1, rows, width, tokens, block, pm);
    int64_t xw = sm == 0 ? pm[5] : pr[5];
    PcBuffer dw = upload(c.wc, (size_t)(rows * width)), dws = upload(c.ws, (size_t)(rows * nb) * 4), dwo = upload(c.wo, (size_t)(rows * nb) * 4);
    PcBuffer dx = upload(c.x, (size_t)(tokens * width) * 4), dq = zeros((size_t)xw * 4), dqs = zeros((size_t)(tokens * nb) * 4), dqm = zeros((size_t)(tokens * nb) * 4);
    PcBuffer ddots = zeros((size_t)nd * 4), dy = zeros((size_t)ny * 8);
    Launch q = {k_quant, tokens * nb, 256, 0, {I(block), V(dx, tokens * width), V(dq, xw), V(dqs, tokens * nb), V(dqm, tokens * nb)}, 5};
    Launch f = {k_finish, ny, 256, 0, {I(rows), I(nb), V(dws, rows * nb), V(dwo, rows * nb), V(dqs, tokens * nb), V(dqm, tokens * nb), V(ddots, nd), V(dy, ny)}, 8};
    go(&q);
    int8_t *gq = malloc((size_t)xw * 4); float *gs = malloc((size_t)(tokens * nb) * 4); int32_t *gm = malloc((size_t)(tokens * nb) * 4);
    down(gq, dq, (size_t)xw * 4); down(gs, dqs, (size_t)(tokens * nb) * 4); down(gm, dqm, (size_t)(tokens * nb) * 4);
    int quant = !memcmp(gq, qc, (size_t)(tokens * width)) && !memcmp(gs, qs, (size_t)(tokens * nb) * 4) && !memcmp(gm, qm, (size_t)(tokens * nb) * 4);
    int row = 0, mma = -1;
    if (sr == 0) {
        Launch d = {k_dots, nd, 256, 0, {I(rows), I(width), I(block), V(dw, rows * width / 4), V(dq, xw), V(ddots, nd)}, 6};
        go(&d); go(&f); down(dd, ddots, (size_t)nd * 4); down(yd, dy, (size_t)ny * 8);
        row = !memcmp(dd, d1, (size_t)nd * 4) && !memcmp(yd, y1, (size_t)ny * 8);
    }
    if (sm == 0) {
        TRY(pc_fill(ddots, 0, (size_t)nd, NULL)); TRY(pc_fill(dy, 0, (size_t)ny * 2, NULL));
        Launch m = {k_mma, pm[2], 128, 1, {I(rows), I(width), I(tokens), I(block), V(dw, rows * width / 4), V(dq, xw), V(dws, rows * nb), V(dwo, rows * nb),
                                           V(dqs, tokens * nb), V(dqm, tokens * nb), I(1), V(ddots, nd), V(dy, ny)}, 13};
        go(&m); down(dd, ddots, (size_t)nd * 4); down(yd, dy, (size_t)ny * 8);
        mma = !memcmp(dd, d1, (size_t)nd * 4) && !memcmp(yd, y1, (size_t)ny * 8);
    }
    printf("  %4lld x %4lld times %3lld tokens, block %4lld: quantize %s; int8_tiled %s; i8_dots+i8_finish %s; i8_mma %s\n",
           (long long)rows, (long long)width, (long long)tokens, (long long)block, quant ? "equal" : "DIFFERENT",
           tiled ? (same ? "equals int8_row" : "DIFFERS") : "not applicable", row ? "equal" : "DIFFERENT",
           mma < 0 ? "not admitted" : mma ? "equal" : "DIFFERENT");
    failures += !quant || !same || !row || mma == 0;
    pc_free(dw); pc_free(dws); pc_free(dwo); pc_free(dx); pc_free(dq); pc_free(dqs); pc_free(dqm); pc_free(ddots); pc_free(dy);
    free(qc); free(scratch); free(qs); free(qm); free(d1); free(d2); free(dd); free(y1); free(y2); free(yd); free(gq); free(gs); free(gm);
    unmake(&c);
}

typedef struct { const char *name; int64_t ns[RUNS]; double mean, sd; } Side;
static void stats(Side *s) {
    double o[4];
    int64_t st = i8_stats_c((coax_v_s64){s->ns, RUNS}, (coax_v_f64){o, 4});
    if (st != 0) { printf("  %s: statistics refused (%lld)\n", s->name, (long long)st); failures++; }
    s->mean = o[0]; s->sd = o[1];
}
static const char *VERDICT[] = {"refused", "faster", "slower", "the same within two standard errors", "too noisy to rank"};
static void rank(const Side *a, const Side *b) {
    double o[2];
    int64_t v = i8_rank_c((coax_v_s64){(int64_t *)a->ns, RUNS}, (coax_v_s64){(int64_t *)b->ns, RUNS}, (coax_v_f64){o, 2});
    printf("    %s against %s: %.2fx, %s\n", a->name, b->name, o[0], VERDICT[v]);
}
static void line(const Side *s, double bytes, double macs) {
    printf("    %-34s %9.1f us  sd %6.1f  (n=%d)  %6.0f GB/s of weights  %7.0f G multiply-adds/s\n", s->name, s->mean / 1e3, s->sd / 1e3, RUNS,
           bytes / s->mean, macs / s->mean);
}

/* Throughput at DIM x DIM weights, `tokens` tokens. */
static void measure(int64_t tokens) {
    int64_t rows = DIM, width = DIM, nb = width / BLOCK, nd = rows * tokens * nb, ny = rows * tokens, pr[6], pm[6];
    Case c = make(rows, width, tokens, BLOCK);
    if (plan(0, rows, width, tokens, BLOCK, pr) != 0 || plan(1, rows, width, tokens, BLOCK, pm) != 0) { printf("  plan refused\n"); failures++; return; }
    int64_t xw = pm[5];
    PcBuffer dw = upload(c.wc, (size_t)(rows * width)), dws = upload(c.ws, (size_t)(rows * nb) * 4), dwo = upload(c.wo, (size_t)(rows * nb) * 4);
    PcBuffer dx = upload(c.x, (size_t)(tokens * width) * 4), dq = zeros((size_t)xw * 4), dqs = zeros((size_t)(tokens * nb) * 4), dqm = zeros((size_t)(tokens * nb) * 4);
    PcBuffer ddots = zeros((size_t)nd * 4), dy = zeros((size_t)ny * 8), ddots2 = zeros((size_t)nd * 4), dy2 = zeros((size_t)ny * 8);
    Launch q = {k_quant, pr[0], 256, 0, {I(BLOCK), V(dx, tokens * width), V(dq, xw), V(dqs, tokens * nb), V(dqm, tokens * nb)}, 5};
    Launch rowl[3] = {q, {k_dots, pr[1], 256, 0, {I(rows), I(width), I(BLOCK), V(dw, rows * width / 4), V(dq, xw), V(ddots, nd)}, 6},
                      {k_finish, ny, 256, 0, {I(rows), I(nb), V(dws, rows * nb), V(dwo, rows * nb), V(dqs, tokens * nb), V(dqm, tokens * nb), V(ddots, nd), V(dy, ny)}, 8}};
    /* Timed without recording the block sums; then once more recording them. */
    Launch mmal[2] = {q, {k_mma, pm[2], 128, 1, {I(rows), I(width), I(tokens), I(BLOCK), V(dw, rows * width / 4), V(dq, xw), V(dws, rows * nb), V(dwo, rows * nb),
                                                 V(dqs, tokens * nb), V(dqm, tokens * nb), I(0), V(ddots2, 1), V(dy2, ny)}, 13}};
    Side srow = {"int8 i8_dots+i8_finish (dp4a-shaped)"}, smma = {"int8 i8_mma (BF16 tensor cores)"}, sf32 = {""}, sbf = {"BF16-weight MMA dw_project (ch. 37)"};
    timed(rowl, 3, srow.ns); timed(mmal, 2, smma.ns);
    Launch rec = mmal[1]; rec.a[10] = I(1); rec.a[11] = V(ddots2, nd); go(&rec);
    /* The int8 outputs: the two kernels agree bit for bit, the activations
       pass input admission, and sampled block sums equal the naive oracle. */
    int32_t *a = malloc((size_t)nd * 4), *b = malloc((size_t)nd * 4); double *ya = malloc((size_t)ny * 8), *yb = malloc((size_t)ny * 8);
    int8_t *qc = malloc((size_t)xw * 4); float *qs = malloc((size_t)(tokens * nb) * 4); int32_t *qm = malloc((size_t)(tokens * nb) * 4);
    down(a, ddots, (size_t)nd * 4); down(b, ddots2, (size_t)nd * 4); down(ya, dy, (size_t)ny * 8); down(yb, dy2, (size_t)ny * 8);
    down(qc, dq, (size_t)xw * 4); down(qs, dqs, (size_t)(tokens * nb) * 4); down(qm, dqm, (size_t)(tokens * nb) * 4);
    int agree = !memcmp(a, b, (size_t)nd * 4) && !memcmp(ya, yb, (size_t)ny * 8);
    int64_t inputs = i8_inputs_c(rows, width, tokens, BLOCK, (coax_v_s8){c.wc, rows * width}, (coax_v_f32){c.ws, rows * nb}, (coax_v_f32){c.wo, rows * nb},
                                 (coax_v_s8){qc, tokens * width}, (coax_v_f32){qs, tokens * nb}, (coax_v_s32){qm, tokens * nb});
    int64_t sampled = 0, wrong = 0;
    for (int64_t i = 0; i < nd; i += 997) {
        int64_t blk = i % nb, row = i / nb % rows, t = i / nb / rows; sampled++;
        wrong += a[i] != i8_naive_c((coax_v_s8){c.wc, rows * width}, (coax_v_s8){qc, tokens * width}, width, BLOCK, row, t, blk);
    }
    failures += !agree || inputs != 0 || wrong != 0;
    /* Chapter 37's kernels on BF16 weights of the same shape: one buffer of
       BF16 bits, read as u64 groups of four by dv_* and as u32 pairs by dw_*. */
    uint16_t *bf = malloc((size_t)(rows * width) * 2);
    for (int64_t i = 0; i < rows * width; i++) { float v = uniform() / 32.0f; uint32_t u; memcpy(&u, &v, 4); bf[i] = (uint16_t)(u >> 16); }
    PcBuffer dbf = upload(bf, (size_t)(rows * width) * 2), dout = zeros((size_t)ny * 4), dpart = zeros((size_t)(ny * DV_LANES) * 4);
    if (tokens == 1) {
        sf32.name = "f32 dv_matvec+dv_reduce (ch. 37/44)";
        Launch fl[2] = {{k_matvec, rows * DV_LANES, 256, 0, {I(rows), I(width), V(dbf, rows * width / 4), V(dx, width), V(dpart, rows * DV_LANES)}, 5},
                        {k_reduce, rows, 256, 0, {I(rows), I(DV_LANES), V(dpart, rows * DV_LANES), V(dout, rows)}, 4}};
        timed(fl, 2, sf32.ns);
    } else {
        sf32.name = "f32 dv_project (ch. 37/44)";
        Launch fl = {k_project, rows / 4 * ((tokens + 7) / 8), 128, 0, {I(rows), I(width), I(tokens), I(1), V(dbf, rows * width / 4), V(dx, tokens * width), V(dout, ny)}, 7};
        timed(&fl, 1, sf32.ns);
    }
    int64_t padded = (tokens + DW_TOKENS - 1) / DW_TOKENS * DW_TOKENS, half = padded * width / 2;
    PcBuffer hi = zeros((size_t)half * 4), lo = zeros((size_t)half * 4);
    Launch wl[2] = {{k_split, half, 256, 0, {I(width), I(tokens), V(dx, tokens * width), V(hi, half), V(lo, half)}, 5},
                    {k_wproject, padded / DW_TOKENS * (rows / 16), 128, 1, {I(rows), I(width), I(tokens), I(1), V(dbf, rows * width / 2), V(hi, half), V(lo, half), V(dout, ny)}, 8}};
    timed(wl, 2, sbf.ns);
    stats(&srow); stats(&smma); stats(&sf32); stats(&sbf);
    double macs = (double)rows * width * tokens;
    printf("  %s: %lld x %lld weights, %lld token%s, block %d\n", tokens == 1 ? "decode" : "prefill", (long long)rows, (long long)width,
           (long long)tokens, tokens == 1 ? "" : "s", BLOCK);
    printf("    int8 outputs: i8_mma equals i8_dots+i8_finish bit for bit: %s; activations pass input admission: %s; %lld sampled block sums equal the naive oracle: %s\n",
           agree ? "yes" : "NO", inputs == 0 ? "yes" : "NO", (long long)sampled, wrong == 0 ? "yes" : "NO");
    line(&srow, (double)rows * width, macs); line(&smma, (double)rows * width, macs);
    line(&sf32, (double)rows * width * 2, macs); line(&sbf, (double)rows * width * 2, macs);
    const Side *best = smma.mean < srow.mean ? &smma : &srow;
    rank(best, &sf32); rank(best, &sbf); rank(&smma, &srow);
    pc_free(dw); pc_free(dws); pc_free(dwo); pc_free(dx); pc_free(dq); pc_free(dqs); pc_free(dqm); pc_free(ddots); pc_free(dy); pc_free(ddots2); pc_free(dy2);
    pc_free(dbf); pc_free(dout); pc_free(dpart); pc_free(hi); pc_free(lo);
    free(a); free(b); free(ya); free(yb); free(qc); free(qs); free(qm); free(bf); unmake(&c);
}

static PcModule compile(const char *path) {
    FILE *f = fopen(path, "rb"); if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *src = malloc((size_t)size + 1); if (fread(src, 1, (size_t)size, f) != (size_t)size) exit(2); fclose(f); src[size] = 0;
    PcModule m; double seconds; TRY(pc_compile(src, (size_t)size, path, &m, &seconds)); free(src);
    return m;
}

int main(int argc, char **argv) {
    if (argc != 4) { fprintf(stderr, "usage: int8-device INT8.cu PROJECT.cu WARP.cu\n"); return 2; }
    PcStatus o = pc_open(&info);
    if (o == PC_ABSENT || o == PC_NO_NVRTC) { printf("SKIP: no CUDA device or NVRTC\n"); return 0; }
    TRY(o);
    int64_t p[6];
    int64_t row = plan(0, DIM, DIM, 128, BLOCK, p), mma = plan(1, DIM, DIM, 128, BLOCK, p);
    int64_t old_mma = i8_plan_c(1, DIM, DIM, 128, BLOCK, 7, 5, 1024, 49152, (coax_v_s64){p, 6});
    int64_t old_row = i8_plan_c(0, DIM, DIM, 128, BLOCK, 7, 5, 1024, 49152, (coax_v_s64){p, 6});
    printf("device: %s (sm_%d%d): row kernels %s, tensor-core tile %s\n", info.name, info.major, info.minor, row == 0 ? "admitted" : "refused", mma == 0 ? "admitted" : "refused");
    printf("a reported sm_75: tile status %lld (5 + 16*5: needs compute capability 8.0), row kernels status %lld\n", (long long)old_mma, (long long)old_row);
    failures += old_mma != 5 + 16 * 5 || old_row != 0;
    if (row != 0 || mma != 0) { printf("refused on this device\n"); return 1; }
    PcModule m8 = compile(argv[1]), mp = compile(argv[2]), mw = compile(argv[3]);
    TRY(pc_kernel(m8, "i8_quantize", &k_quant)); TRY(pc_kernel(m8, "i8_dots", &k_dots)); TRY(pc_kernel(m8, "i8_finish", &k_finish)); TRY(pc_kernel(m8, "i8_mma", &k_mma));
    TRY(pc_kernel(mp, "dv_matvec", &k_matvec)); TRY(pc_kernel(mp, "dv_reduce", &k_reduce)); TRY(pc_kernel(mp, "dv_project", &k_project));
    TRY(pc_kernel(mw, "dw_split", &k_split)); TRY(pc_kernel(mw, "dw_project", &k_wproject));

    printf("against chapter 48's CPU kernels (int8_quantize, int8_row, int8_tiled), bit for bit:\n");
    exact(256, 4096, 1, 32);
    exact(1024, 1024, 1, 32);
    exact(64, 512, 32, 32);
    exact(128, 1024, 8, 32);
    exact(32, 512, 37, 64);
    exact(16, 1040, 12, 1040);
    exact(12, 960, 5, 48);
    printf("throughput on the device (chapter 34 statistics: one warm-up, %d timed runs per side):\n", RUNS);
    measure(1);
    measure(32);
    measure(128);
    pc_unload(m8); pc_unload(mp); pc_unload(mw);
    if (failures) { printf("FAIL: %d checks\n", failures); return 1; }
    printf("chapter 48 on CUDA: every device block sum and output equals the CPU kernels bit for bit\n");
    return 0;
}
