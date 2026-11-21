/* Thin platform host for exec-gpu-metal-v1 (-DEXEC_GPU_METAL) and
   exec-gpu-cuda-v1 (-DEXEC_GPU_CUDA) (#13916, W1.3).

   No arithmetic, scheduling, admission, or crypto decisions live here:
   - Coax (#authority psionic/lib/exec-vm-gpu.coax & exec-vm-sched.coax) decides
     device admission, exactness envelopes, memory sizing, optimal schedules,
     and SHA-256 trace digests.
   - Coax (psionic/lib/exec-vm-kernels.coax) emits all Metal (fast math OFF) and
     CUDA (--fmad=false) compute kernels.
   - This file performs platform effects only: open GPU, compile emitted source,
     allocate device buffers, upload/download bytes, launch kernels, time runs.

   NEVER falls back to CPU: without a Metal or CUDA GPU, reports PM_ABSENT /
   PC_ABSENT cleanly and stops.

   Usage: exec-vm-gpu-{metal|cuda} KERNELS.{metal|cu} */
#if !defined(_POSIX_C_SOURCE) && !defined(__APPLE__)
#define _POSIX_C_SOURCE 200809L
#endif
#include "evg-bridge.h"
#ifdef EXEC_GPU_METAL
#include "metal-host.h"
#else
#include "cuda-host.h"
#endif

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static double now_s(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec;
}

static char *read_text_file(const char *path, size_t *out_len) {
    FILE *f = fopen(path, "rb");
    if (!f) { fprintf(stderr, "cannot open %s\n", path); return NULL; }
    if (fseek(f, 0, SEEK_END) != 0) { fclose(f); return NULL; }
    long sz = ftell(f);
    if (sz <= 0 || fseek(f, 0, SEEK_SET) != 0) { fclose(f); return NULL; }
    char *buf = (char *)malloc((size_t)sz + 1);
    if (!buf) { fclose(f); return NULL; }
    if (fread(buf, 1, (size_t)sz, f) != (size_t)sz) { free(buf); fclose(f); return NULL; }
    fclose(f);
    buf[sz] = '\0';
    *out_len = (size_t)sz;
    return buf;
}

static void fmt_hex(const uint8_t d[32], char out[65]) {
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out[2 * i] = hex[d[i] >> 4];
        out[2 * i + 1] = hex[d[i] & 15];
    }
    out[64] = '\0';
}

/* ---- Unified thin GPU backend adapter (Metal vs CUDA) ---- */

#ifdef EXEC_GPU_METAL
typedef PmBuffer GpuBuf;
typedef PmModule GpuMod;
typedef PmKernel GpuKern;
typedef PmBatch  GpuBatch;
typedef PmArg    GpuArg;

#define GPU_TRY(x) do { PmStatus s_ = (x); if (s_ != PM_OK) { fprintf(stderr, "%s failed (%d): %s\n", #x, (int)s_, pm_log()); exit(1); } } while (0)

static GpuBuf gpu_alloc(size_t bytes) { GpuBuf b = NULL; GPU_TRY(pm_alloc(bytes, &b)); return b; }
static void gpu_free(GpuBuf b) { GPU_TRY(pm_free(b)); }
static void gpu_upload(GpuBuf dst, const void *src, size_t bytes) { memcpy(pm_contents(dst), src, bytes); }
static void gpu_download(void *dst, GpuBuf src, size_t bytes) { memcpy(dst, pm_contents(src), bytes); }
static GpuArg arg_i(int64_t v) { return (GpuArg){PM_ARG_INT, v, 0.0, NULL, 0, 0}; }
static GpuArg arg_f64(double v) { return (GpuArg){PM_ARG_F64, 0, v, NULL, 0, 0}; }
static GpuArg arg_v(GpuBuf b, int64_t count) { return (GpuArg){PM_ARG_VIEW, 0, 0.0, b, 0, count}; }
static GpuBatch gpu_begin(void) { GpuBatch b = NULL; GPU_TRY(pm_begin(&b)); return b; }
static void gpu_launch(GpuBatch b, GpuKern k, int64_t items, int block, const GpuArg *a, int n) {
    GPU_TRY(pm_launch(b, k, items, 1, block, a, n));
}
static double gpu_wait(GpuBatch b) { double s = 0.0; GPU_TRY(pm_wait(b, &s)); return s; }

/* On Metal (coax metal-compute), []f64 device buffers store float-float (hi, lo) pairs. */
static void f64_to_dev(void *dst, const double *src, size_t count) {
    float *p = (float *)dst;
    for (size_t i = 0; i < count; i++) {
        float hi = (float)src[i];
        float lo = (float)(src[i] - (double)hi);
        p[2 * i] = hi;
        p[2 * i + 1] = lo;
    }
}
static void dev_to_f64(double *dst, const void *src, size_t count) {
    const float *p = (const float *)src;
    for (size_t i = 0; i < count; i++) {
        dst[i] = (double)p[2 * i] + (double)p[2 * i + 1];
    }
}
#else
typedef PcBuffer GpuBuf;
typedef PcModule GpuMod;
typedef PcKernel GpuKern;
typedef PcStream GpuBatch;
typedef PcArg    GpuArg;

static PcStream g_stream = NULL;
static PcEvent  g_ev0 = NULL, g_ev1 = NULL;

#define GPU_TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { fprintf(stderr, "%s failed (%d): %s\n", #x, (int)s_, pc_log()); exit(1); } } while (0)

static GpuBuf gpu_alloc(size_t bytes) { GpuBuf b = 0; GPU_TRY(pc_alloc(bytes ? bytes : 16, &b)); return b; }
static void gpu_free(GpuBuf b) { GPU_TRY(pc_free(b)); }
static void gpu_upload(GpuBuf dst, const void *src, size_t bytes) {
    GPU_TRY(pc_upload(dst, src, bytes, g_stream));
    GPU_TRY(pc_sync_stream(g_stream));
}
static void gpu_download(void *dst, GpuBuf src, size_t bytes) {
    GPU_TRY(pc_download(dst, src, bytes, g_stream));
    GPU_TRY(pc_sync_stream(g_stream));
}
static GpuArg arg_i(int64_t v) { return (GpuArg){PC_ARG_INT, v, 0.0, 0, 0}; }
static GpuArg arg_f64(double v) { return (GpuArg){PC_ARG_F64, 0, v, 0, 0}; }
static GpuArg arg_v(GpuBuf b, int64_t count) { return (GpuArg){PC_ARG_VIEW, 0, 0.0, b, count}; }
static GpuBatch gpu_begin(void) {
    GPU_TRY(pc_record(g_ev0, g_stream));
    return g_stream;
}
static void gpu_launch(GpuBatch b, GpuKern k, int64_t items, int block, const GpuArg *a, int n) {
    if (items <= 0) return;
    GPU_TRY(pc_launch(k, items, block > 0 ? block : 128, a, n, b));
}
static double gpu_wait(GpuBatch b) {
    double s = 0.0;
    GPU_TRY(pc_record(g_ev1, b));
    GPU_TRY(pc_sync_stream(b));
    GPU_TRY(pc_event_seconds(g_ev0, g_ev1, &s));
    return s;
}
static void f64_to_dev(void *dst, const double *src, size_t count) { memcpy(dst, src, count * sizeof(double)); }
static void dev_to_f64(double *dst, const void *src, size_t count) { memcpy(dst, src, count * sizeof(double)); }
#endif

typedef struct {
    GpuMod mod;
    GpuKern init_i64, ffn_up_i64, ffn_down_i64, copy_i64, store_kv_i64;
    GpuKern attn_uni_i64, attn_para_i64, attn_wo_i64, readout_i64;
    GpuKern rollout_i64, rollout_f64, precision_probe;
    GpuKern soft_top, soft_z, soft_red, soft_y;
} Kernels;

static void load_kernels(GpuMod mod, Kernels *k) {
    k->mod = mod;
#ifdef EXEC_GPU_METAL
#define K(field, name) GPU_TRY(pm_kernel(mod, name, &k->field))
#else
#define K(field, name) GPU_TRY(pc_kernel(mod, name, &k->field))
#endif
    K(init_i64,        "evk_init_i64");
    K(ffn_up_i64,      "evk_ffn_up_gate_i64");
    K(ffn_down_i64,    "evk_ffn_down_i64");
    K(copy_i64,        "evk_copy_i64");
    K(store_kv_i64,    "evk_store_kv_i64");
    K(attn_uni_i64,    "evk_attn_uniform_head_i64");
    K(attn_para_i64,   "evk_attn_parabolic_head_i64");
    K(attn_wo_i64,     "evk_attn_apply_wo_i64");
    K(readout_i64,     "evk_readout_i64");
    K(rollout_i64,     "evk_ch226_rollout_i64");
    K(rollout_f64,     "evk_ch226_rollout_f64");
    K(precision_probe, "evk_precision_probe");
    K(soft_top,        "evk_soft_top_logit");
    K(soft_z,          "evk_soft_z_chunk");
    K(soft_red,        "evk_soft_reduce_f64");
    K(soft_y,          "evk_soft_y_chunk");
#undef K
}

/* ---- 1. GPU Precision-Envelope Control Probe ---- */
static void run_precision_probe(const Kernels *k) {
    enum { N = 6 };
    int64_t ha[N] = {4096, 4097, 16777216, 16777217, 67108863, 67108864};
    int64_t hb[N] = {4096, 4097, 16777216, 16777217, 67108863, 67108864};
    GpuBuf da = gpu_alloc(N * 8), db = gpu_alloc(N * 8);
    GpuBuf df32 = gpu_alloc(N * 4), df64 = gpu_alloc(N * 8), di64 = gpu_alloc(N * 8);
    gpu_upload(da, ha, N * 8);
    gpu_upload(db, hb, N * 8);

    GpuArg args[] = {arg_v(da, N), arg_v(db, N), arg_v(df32, N), arg_v(df64, N), arg_v(di64, N)};
    GpuBatch batch = gpu_begin();
    gpu_launch(batch, k->precision_probe, N, 32, args, 5);
    double gpu_s = gpu_wait(batch);

    float rf32[N];
    uint8_t raw_f64[N * 8];
    double rf64[N];
    int64_t ri64[N];
    gpu_download(rf32, df32, N * 4);
    gpu_download(raw_f64, df64, N * 8);
    dev_to_f64(rf64, raw_f64, N);
    gpu_download(ri64, di64, N * 8);

    printf("\n[1] GPU exact arithmetic envelope probe (real GPU %.3f ms):\n", gpu_s * 1e3);
    for (int i = 0; i < N; i++) {
        int64_t want = ha[i] * hb[i];
        int ok32 = ((double)rf32[i] == (double)want);
        int ok64 = (rf64[i] == (double)want);
        int oki64 = (ri64[i] == want);
        printf("  %lld * %lld = %lld | f32:%s  f64/ff:%s  i64:%s\n",
               (long long)ha[i], (long long)hb[i], (long long)want,
               ok32 ? "EXACT" : "FAIL ", ok64 ? "EXACT" : "FAIL ", oki64 ? "EXACT" : "FAIL ");
        if (!oki64) { fprintf(stderr, "i64 precision probe failed at %d\n", i); exit(1); }
    }
    /* Verify f32 breaks at 4097^2 while i64 holds through (2^26-1)^2 and 2^52! */
    if ((double)rf32[1] == (double)(4097LL * 4097LL)) { fprintf(stderr, "expected f32 failure at 4097^2\n"); exit(1); }
    if (rf64[2] != (double)(16777216LL * 16777216LL)) { fprintf(stderr, "expected f64/ff exact at 2^48\n"); exit(1); }
#ifdef EXEC_GPU_METAL
    if (rf64[3] == (double)(16777217LL * 16777217LL)) { fprintf(stderr, "expected Metal f32x2 cliff above 2^48\n"); exit(1); }
    if (rf64[4] == (double)(67108863LL * 67108863LL)) { fprintf(stderr, "expected Metal f32x2 cliff at (2^26-1)^2\n"); exit(1); }
#else
    if (rf64[4] != (double)(67108863LL * 67108863LL)) { fprintf(stderr, "expected CUDA native f64 exact at (2^26-1)^2\n"); exit(1); }
    if (rf64[5] != (double)(67108864LL * 67108864LL)) { fprintf(stderr, "expected CUDA native f64 exact at 2^52\n"); exit(1); }
#endif
    gpu_free(da); gpu_free(db); gpu_free(df32); gpu_free(df64); gpu_free(di64);
}

/* ---- 2. Per-Layer Dense-Matmul GPU Execution (B programs step-by-step) ---- */
static void run_per_layer_i64(
    const Kernels *k,
    const coax_t_EvModel *m,
    int batch_n,
    const coax_t_TmOp *progs,
    const int *prog_lens,
    int max_prog_len,
    int budget,
    const int *ref_steps,
    const uint8_t (*ref_digests)[32],
    double *out_wall_s,
    double *out_gpu_s,
    int *out_total_steps
) {
    int64_t D = m->width;
    int64_t kv_stride = max_prog_len + budget + 2;
    int64_t max_units = 1;
    for (int64_t li = 0; li < m->n_layers; li++) {
        if (m->layers.data[li].kind == COAX_EvLayerKind_ffn && m->layers.data[li].units > max_units)
            max_units = m->layers.data[li].units;
    }

    int64_t *w_i64 = (int64_t *)calloc((size_t)m->n_weights, sizeof(int64_t));
    for (int64_t i = 0; i < m->n_weights; i++) w_i64[i] = (int64_t)m->weights.data[i];

    int64_t *h_kv = (int64_t *)calloc((size_t)batch_n * (size_t)kv_stride * (size_t)D, sizeof(int64_t));
    int64_t *h_kv_lens = (int64_t *)calloc((size_t)batch_n, sizeof(int64_t));
    int64_t *h_active = (int64_t *)calloc((size_t)batch_n, sizeof(int64_t));
    int64_t *h_ro = (int64_t *)calloc((size_t)batch_n * 6, sizeof(int64_t));
    int64_t *h_err = (int64_t *)calloc((size_t)batch_n, sizeof(int64_t));
    int64_t *traces = (int64_t *)calloc((size_t)batch_n * (size_t)budget * 4, sizeof(int64_t));
    int *steps = (int *)calloc((size_t)batch_n, sizeof(int));

    for (int b = 0; b < batch_n; b++) {
        coax_v_t_TmOp pv = {(coax_t_TmOp *)(progs + b * max_prog_len), prog_lens[b]};
        coax_v_s64 kvv = {h_kv + (size_t)b * (size_t)kv_stride * (size_t)D, kv_stride * D};
        if (!evgb_encode_program_i64(pv, D, kvv)) { fprintf(stderr, "encode_program failed\n"); exit(1); }
        h_kv_lens[b] = prog_lens[b];
        h_active[b] = 1;
    }

    GpuBuf d_w = gpu_alloc((size_t)m->n_weights * 8);
    GpuBuf d_kv = gpu_alloc((size_t)batch_n * (size_t)kv_stride * (size_t)D * 8);
    GpuBuf d_kvl = gpu_alloc((size_t)batch_n * 8);
    GpuBuf d_act = gpu_alloc((size_t)batch_n * 8);
    GpuBuf d_in = gpu_alloc(16);
    GpuBuf d_x = gpu_alloc((size_t)batch_n * (size_t)D * 8);
    GpuBuf d_st = gpu_alloc((size_t)batch_n * (size_t)D * 8);
    GpuBuf d_h = gpu_alloc((size_t)batch_n * (size_t)max_units * 8);
    GpuBuf d_yv = gpu_alloc((size_t)batch_n * 4 * 8);
    GpuBuf d_ro = gpu_alloc((size_t)batch_n * 6 * 8);
    GpuBuf d_err = gpu_alloc((size_t)batch_n * 8);

    gpu_upload(d_w, w_i64, (size_t)m->n_weights * 8);
    gpu_upload(d_kv, h_kv, (size_t)batch_n * (size_t)kv_stride * (size_t)D * 8);
    gpu_upload(d_err, h_err, (size_t)batch_n * 8);

    int64_t kv_total = (int64_t)batch_n * kv_stride * D;
    int active_count = batch_n;
    double gpu_total = 0.0;
    double t0 = now_s();

    while (active_count > 0) {
        gpu_upload(d_kvl, h_kv_lens, (size_t)batch_n * 8);
        gpu_upload(d_act, h_active, (size_t)batch_n * 8);

        GpuBatch batch = gpu_begin();
        GpuArg init_a[] = {arg_i(batch_n), arg_i(D), arg_i(0), arg_v(d_in, 1), arg_v(d_x, (int64_t)batch_n * D)};
        gpu_launch(batch, k->init_i64, (int64_t)batch_n * D, 128, init_a, 5);

        for (int64_t li = 0; li < m->n_layers; li++) {
            coax_t_EvLayerDesc ly = m->layers.data[li];
            if (ly.kind == COAX_EvLayerKind_ffn) {
                int64_t U = ly.units;
                if (U > 0) {
                    GpuArg up_a[] = {
                        arg_i(batch_n), arg_i(D), arg_i(U), arg_i(ly.w_offset),
                        arg_v(d_w, m->n_weights), arg_v(d_x, (int64_t)batch_n * D),
                        arg_v(d_h, (int64_t)batch_n * max_units), arg_v(d_err, batch_n)
                    };
                    gpu_launch(batch, k->ffn_up_i64, (int64_t)batch_n * U, 128, up_a, 8);
                    GpuArg dn_a[] = {
                        arg_i(batch_n), arg_i(D), arg_i(U), arg_i(ly.w_offset),
                        arg_v(d_w, m->n_weights), arg_v(d_x, (int64_t)batch_n * D),
                        arg_v(d_h, (int64_t)batch_n * max_units), arg_v(d_st, (int64_t)batch_n * D),
                        arg_v(d_err, batch_n)
                    };
                    gpu_launch(batch, k->ffn_down_i64, (int64_t)batch_n * D, 128, dn_a, 9);
                    GpuArg cp_a[] = {arg_v(d_st, (int64_t)batch_n * D), arg_v(d_x, (int64_t)batch_n * D)};
                    gpu_launch(batch, k->copy_i64, (int64_t)batch_n * D, 128, cp_a, 2);
                }
            } else {
                GpuArg cp0[] = {arg_v(d_x, (int64_t)batch_n * D), arg_v(d_st, (int64_t)batch_n * D)};
                gpu_launch(batch, k->copy_i64, (int64_t)batch_n * D, 128, cp0, 2);
                for (int64_t hi = ly.head_first; hi < ly.head_first + ly.head_count; hi++) {
                    coax_t_EvHeadDesc hd = m->heads.data[hi];
                    int64_t kv_self = (hd.mask == COAX_EvAttnMask_causal_self) ? 1 : 0;
                    if (hd.kind == COAX_EvHeadKind_uniform_sum) {
                        GpuArg ua[] = {
                            arg_i(batch_n), arg_i(D), arg_i(hd.d_v), arg_i(kv_stride),
                            arg_v(d_kvl, batch_n), arg_i(kv_self), arg_i(hd.w_offset),
                            arg_v(d_w, m->n_weights), arg_v(d_kv, kv_total),
                            arg_v(d_x, (int64_t)batch_n * D), arg_v(d_yv, (int64_t)batch_n * 4),
                            arg_v(d_err, batch_n)
                        };
                        gpu_launch(batch, k->attn_uni_i64, (int64_t)batch_n * hd.d_v, 64, ua, 12);
                    } else {
                        int64_t oldest = (hd.kind == COAX_EvHeadKind_parabolic_oldest) ? 1 : 0;
                        GpuArg pa[] = {
                            arg_i(batch_n), arg_i(D), arg_i(hd.d_v), arg_i(oldest), arg_i(kv_stride),
                            arg_v(d_kvl, batch_n), arg_i(kv_self), arg_i(hd.w_offset),
                            arg_v(d_w, m->n_weights), arg_v(d_kv, kv_total),
                            arg_v(d_x, (int64_t)batch_n * D), arg_v(d_yv, (int64_t)batch_n * 4),
                            arg_v(d_err, batch_n)
                        };
                        gpu_launch(batch, k->attn_para_i64, batch_n, 64, pa, 13);
                    }
                    int64_t wo_base = hd.w_offset + (2 * hd.d_k + hd.d_v) * D;
                    GpuArg wo_a[] = {
                        arg_i(batch_n), arg_i(D), arg_i(hd.d_v), arg_i(wo_base),
                        arg_v(d_w, m->n_weights), arg_v(d_yv, (int64_t)batch_n * 4),
                        arg_v(d_st, (int64_t)batch_n * D), arg_v(d_err, batch_n)
                    };
                    gpu_launch(batch, k->attn_wo_i64, (int64_t)batch_n * D, 128, wo_a, 8);
                }
                GpuArg cp1[] = {arg_v(d_st, (int64_t)batch_n * D), arg_v(d_x, (int64_t)batch_n * D)};
                gpu_launch(batch, k->copy_i64, (int64_t)batch_n * D, 128, cp1, 2);
            }
        }

        GpuArg ro_a[] = {
            arg_i(batch_n), arg_i(D), arg_i(6), arg_i(m->w_out_offset),
            arg_v(d_w, m->n_weights), arg_v(d_x, (int64_t)batch_n * D),
            arg_v(d_ro, (int64_t)batch_n * 6), arg_v(d_err, batch_n)
        };
        gpu_launch(batch, k->readout_i64, (int64_t)batch_n * 6, 64, ro_a, 8);
        GpuArg st_a[] = {
            arg_i(batch_n), arg_i(D), arg_i(kv_stride), arg_v(d_kvl, batch_n),
            arg_v(d_act, batch_n), arg_v(d_x, (int64_t)batch_n * D), arg_v(d_kv, kv_total)
        };
        gpu_launch(batch, k->store_kv_i64, (int64_t)batch_n * D, 128, st_a, 7);
        gpu_total += gpu_wait(batch);

        gpu_download(h_ro, d_ro, (size_t)batch_n * 6 * 8);
        gpu_download(h_err, d_err, (size_t)batch_n * 8);

        active_count = 0;
        for (int b = 0; b < batch_n; b++) {
            if (!h_active[b]) continue;
            if (h_err[b] != 0) { fprintf(stderr, "GPU exact envelope error %lld on prog %d\n", (long long)h_err[b], b); exit(1); }
            int64_t *ro = h_ro + b * 6;
            if (ro[5] != 0) { fprintf(stderr, "unexpected VM fault %lld on prog %d\n", (long long)ro[5], b); exit(1); }
            int64_t *tr = traces + ((size_t)b * (size_t)budget + (size_t)steps[b]) * 4;
            tr[0] = ro[0]; tr[1] = ro[1]; tr[2] = ro[2]; tr[3] = ro[3];
            steps[b]++;
            h_kv_lens[b]++;
            if (ro[4] == 1 || steps[b] >= budget) {
                h_active[b] = 0;
            } else {
                active_count++;
            }
        }
    }
    double wall = now_s() - t0;

    int total_steps = 0;
    for (int b = 0; b < batch_n; b++) {
        uint8_t digest[32];
        coax_v_s64 tv = {traces + (size_t)b * (size_t)budget * 4, (int64_t)steps[b] * 4};
        coax_v_u8 dv = {digest, 32};
        if (!evg_trace_digest(tv, steps[b], dv)) { fprintf(stderr, "digest failed\n"); exit(1); }
        if (steps[b] != ref_steps[b] || memcmp(digest, ref_digests[b], 32) != 0) {
            fprintf(stderr, "PER-LAYER GPU MISMATCH on program %d (steps %d vs %d)\n", b, steps[b], ref_steps[b]);
            exit(1);
        }
        total_steps += steps[b];
    }
    *out_wall_s = wall;
    *out_gpu_s = gpu_total;
    *out_total_steps = total_steps;

    gpu_free(d_w); gpu_free(d_kv); gpu_free(d_kvl); gpu_free(d_act); gpu_free(d_in);
    gpu_free(d_x); gpu_free(d_st); gpu_free(d_h); gpu_free(d_yv); gpu_free(d_ro); gpu_free(d_err);
    free(w_i64); free(h_kv); free(h_kv_lens); free(h_active); free(h_ro); free(h_err); free(traces); free(steps);
}

/* ---- 3. Persistent Batched GPU Rollout (i64 AND f64/double-single) ---- */
static void run_rollout_gpu(
    const Kernels *k,
    const coax_t_EvModel *m,
    int use_f64,
    double exact_lim,
    int batch_n,
    const coax_t_TmOp *progs,
    const int *prog_lens,
    int max_prog_len,
    int budget,
    const int *ref_steps,
    const uint8_t (*ref_digests)[32],
    double *out_wall_s,
    double *out_gpu_s,
    int *out_total_steps
) {
    int64_t D = m->width;
    int64_t kv_stride = max_prog_len + budget + 2;

    int64_t *h_ld = (int64_t *)calloc((size_t)m->n_layers * 3, sizeof(int64_t));
    for (int64_t li = 0; li < m->n_layers; li++) {
        coax_t_EvLayerDesc ly = m->layers.data[li];
        h_ld[3 * li] = (ly.kind == COAX_EvLayerKind_attn) ? 1 : 0;
        h_ld[3 * li + 1] = (ly.kind == COAX_EvLayerKind_attn) ? ly.head_first : ly.units;
        h_ld[3 * li + 2] = (ly.kind == COAX_EvLayerKind_attn) ? ly.head_count : ly.w_offset;
    }
    int64_t *h_hd = (int64_t *)calloc((size_t)(m->n_heads > 0 ? m->n_heads : 1) * 3, sizeof(int64_t));
    for (int64_t hi = 0; hi < m->n_heads; hi++) {
        coax_t_EvHeadDesc hd = m->heads.data[hi];
        h_hd[3 * hi] = (int64_t)hd.kind;
        h_hd[3 * hi + 1] = hd.d_v;
        h_hd[3 * hi + 2] = hd.w_offset;
    }
    int64_t *h_cfg = (int64_t *)calloc((size_t)batch_n * 2, sizeof(int64_t));
    for (int b = 0; b < batch_n; b++) {
        h_cfg[2 * b] = prog_lens[b];
        h_cfg[2 * b + 1] = budget;
    }

    GpuBuf d_ld = gpu_alloc((size_t)m->n_layers * 3 * 8);
    GpuBuf d_hd = gpu_alloc((size_t)(m->n_heads > 0 ? m->n_heads : 1) * 3 * 8);
    GpuBuf d_cfg = gpu_alloc((size_t)batch_n * 2 * 8);
    GpuBuf d_w = gpu_alloc((size_t)m->n_weights * 8);
    GpuBuf d_kv = gpu_alloc((size_t)batch_n * (size_t)kv_stride * (size_t)D * 8);
    GpuBuf d_tr = gpu_alloc((size_t)batch_n * (size_t)budget * 4 * 8);
    GpuBuf d_meta = gpu_alloc((size_t)batch_n * 4 * 8);

    gpu_upload(d_ld, h_ld, (size_t)m->n_layers * 3 * 8);
    gpu_upload(d_hd, h_hd, (size_t)(m->n_heads > 0 ? m->n_heads : 1) * 3 * 8);
    gpu_upload(d_cfg, h_cfg, (size_t)batch_n * 2 * 8);

    size_t kv_elems = (size_t)batch_n * (size_t)kv_stride * (size_t)D;
    if (!use_f64) {
        int64_t *w_i = (int64_t *)calloc((size_t)m->n_weights, sizeof(int64_t));
        for (int64_t i = 0; i < m->n_weights; i++) w_i[i] = (int64_t)m->weights.data[i];
        int64_t *kv_i = (int64_t *)calloc(kv_elems, sizeof(int64_t));
        for (int b = 0; b < batch_n; b++) {
            coax_v_t_TmOp pv = {(coax_t_TmOp *)(progs + b * max_prog_len), prog_lens[b]};
            coax_v_s64 kvv = {kv_i + (size_t)b * (size_t)kv_stride * (size_t)D, kv_stride * D};
            if (!evgb_encode_program_i64(pv, D, kvv)) { fprintf(stderr, "encode i64 failed\n"); exit(1); }
        }
        gpu_upload(d_w, w_i, (size_t)m->n_weights * 8);
        gpu_upload(d_kv, kv_i, kv_elems * 8);
        free(w_i); free(kv_i);
    } else {
        uint8_t *w_dev = (uint8_t *)calloc((size_t)m->n_weights, 8);
        f64_to_dev(w_dev, m->weights.data, (size_t)m->n_weights);
        double *kv_f = (double *)calloc(kv_elems, sizeof(double));
        for (int b = 0; b < batch_n; b++) {
            coax_v_t_TmOp pv = {(coax_t_TmOp *)(progs + b * max_prog_len), prog_lens[b]};
            coax_v_f64 kvv = {kv_f + (size_t)b * (size_t)kv_stride * (size_t)D, kv_stride * D};
            if (!evgb_encode_program_f64(pv, D, kvv)) { fprintf(stderr, "encode f64 failed\n"); exit(1); }
        }
        uint8_t *kv_dev = (uint8_t *)calloc(kv_elems, 8);
        f64_to_dev(kv_dev, kv_f, kv_elems);
        gpu_upload(d_w, w_dev, (size_t)m->n_weights * 8);
        gpu_upload(d_kv, kv_dev, kv_elems * 8);
        free(w_dev); free(kv_f); free(kv_dev);
    }

    double t0 = now_s();
    GpuBatch batch = gpu_begin();
    if (!use_f64) {
        GpuArg a[] = {
            arg_i(D), arg_i(m->n_layers), arg_i(kv_stride), arg_i(budget), arg_i(m->w_out_offset),
            arg_v(d_ld, m->n_layers * 3), arg_v(d_hd, (m->n_heads > 0 ? m->n_heads : 1) * 3),
            arg_v(d_w, m->n_weights), arg_v(d_cfg, (int64_t)batch_n * 2),
            arg_v(d_kv, (int64_t)kv_elems), arg_v(d_tr, (int64_t)batch_n * budget * 4),
            arg_v(d_meta, (int64_t)batch_n * 4)
        };
        gpu_launch(batch, k->rollout_i64, batch_n, 32, a, 12);
    } else {
        GpuArg a[] = {
            arg_i(D), arg_i(m->n_layers), arg_i(kv_stride), arg_i(budget), arg_i(m->w_out_offset),
            arg_f64(exact_lim),
            arg_v(d_ld, m->n_layers * 3), arg_v(d_hd, (m->n_heads > 0 ? m->n_heads : 1) * 3),
            arg_v(d_w, m->n_weights), arg_v(d_cfg, (int64_t)batch_n * 2),
            arg_v(d_kv, (int64_t)kv_elems), arg_v(d_tr, (int64_t)batch_n * budget * 4),
            arg_v(d_meta, (int64_t)batch_n * 4)
        };
        gpu_launch(batch, k->rollout_f64, batch_n, 32, a, 13);
    }
    double gpu_s = gpu_wait(batch);
    double wall = now_s() - t0;

    int64_t *h_meta = (int64_t *)calloc((size_t)batch_n * 4, sizeof(int64_t));
    gpu_download(h_meta, d_meta, (size_t)batch_n * 4 * 8);

    int64_t *h_tr_i = (int64_t *)calloc((size_t)batch_n * (size_t)budget * 4, sizeof(int64_t));
    if (!use_f64) {
        gpu_download(h_tr_i, d_tr, (size_t)batch_n * (size_t)budget * 4 * 8);
    } else {
        size_t n_tr = (size_t)batch_n * (size_t)budget * 4;
        uint8_t *raw = (uint8_t *)malloc(n_tr * 8);
        double *tr_f = (double *)malloc(n_tr * sizeof(double));
        gpu_download(raw, d_tr, n_tr * 8);
        dev_to_f64(tr_f, raw, n_tr);
        for (size_t i = 0; i < n_tr; i++) h_tr_i[i] = (int64_t)llround(tr_f[i]);
        free(raw); free(tr_f);
    }

    int total_steps = 0;
    for (int b = 0; b < batch_n; b++) {
        int64_t st = h_meta[4 * b];
        int64_t steps = h_meta[4 * b + 1];
        int64_t err = h_meta[4 * b + 3];
        if (st != 0 || err != 0 || steps != ref_steps[b]) {
            fprintf(stderr, "ROLLOUT(%s) MISMATCH prog %d: status=%lld err=%lld steps=%lld want=%d\n",
                    use_f64 ? "f64" : "i64", b, (long long)st, (long long)err, (long long)steps, ref_steps[b]);
            exit(1);
        }
        uint8_t digest[32];
        coax_v_s64 tv = {h_tr_i + (size_t)b * (size_t)budget * 4, steps * 4};
        coax_v_u8 dv = {digest, 32};
        if (!evg_trace_digest(tv, steps, dv) || memcmp(digest, ref_digests[b], 32) != 0) {
            fprintf(stderr, "ROLLOUT(%s) DIGEST MISMATCH on program %d\n", use_f64 ? "f64" : "i64", b);
            exit(1);
        }
        total_steps += (int)steps;
    }

    *out_wall_s = wall;
    *out_gpu_s = gpu_s;
    *out_total_steps = total_steps;

    gpu_free(d_ld); gpu_free(d_hd); gpu_free(d_cfg); gpu_free(d_w); gpu_free(d_kv); gpu_free(d_tr); gpu_free(d_meta);
    free(h_ld); free(h_hd); free(h_cfg); free(h_meta); free(h_tr_i);
}

/* ---- 4. Device-Level Ch 233 Softmax Precision-Cliff Sweep ---- */
typedef struct { int until; int failure; double gpu_ms; } SoftSweep;

static SoftSweep gpu_ch233_scan(
    const Kernels *k, int near, int64_t key_k, int64_t val_v,
    double beta, int fmt, int max_n,
    GpuBuf d_keys, GpuBuf d_vals, GpuBuf d_top, GpuBuf d_pz, GpuBuf d_tz, GpuBuf d_py, GpuBuf d_my, GpuBuf d_un,
    int64_t *h_keys, int64_t *h_vals
) {
    SoftSweep res = {0, 0, 0.0};
    for (int n = 1; n <= max_n; n *= 2) {
        int64_t best_pos = 0;
        if (near) {
            h_keys[0] = key_k; h_vals[0] = val_v;
            for (int t = 1; t < n; t++) { h_keys[t] = key_k - 1; h_vals[t] = -val_v; }
            best_pos = 0;
        } else {
            for (int t = 0; t < n; t++) { h_keys[t] = key_k; h_vals[t] = -val_v; }
            h_vals[n - 1] = val_v;
            best_pos = n - 1;
        }
        int64_t zero = 0;
        gpu_upload(d_keys, h_keys, (size_t)n * 8);
        gpu_upload(d_vals, h_vals, (size_t)n * 8);
        gpu_upload(d_un, &zero, 8);

        int n_chunks = n < 256 ? n : 256;
        GpuBatch batch = gpu_begin();
        GpuArg top_a[] = {arg_i(n), arg_i(key_k), arg_f64(beta), arg_i(fmt), arg_i(best_pos), arg_v(d_keys, n), arg_v(d_top, 1)};
        gpu_launch(batch, k->soft_top, 1, 32, top_a, 7);
        GpuArg z_a[] = {arg_i(n_chunks), arg_i(n), arg_i(key_k), arg_f64(beta), arg_i(fmt), arg_v(d_keys, n), arg_v(d_top, 1), arg_v(d_pz, n_chunks), arg_v(d_un, 1)};
        gpu_launch(batch, k->soft_z, n_chunks, 64, z_a, 9);
        GpuArg rz_a[] = {arg_i(n_chunks), arg_v(d_pz, n_chunks), arg_v(d_tz, 1)};
        gpu_launch(batch, k->soft_red, 1, 32, rz_a, 3);
        GpuArg y_a[] = {arg_i(n_chunks), arg_i(n), arg_i(key_k), arg_f64(beta), arg_i(fmt), arg_v(d_keys, n), arg_v(d_vals, n), arg_v(d_top, 1), arg_v(d_tz, 1), arg_v(d_py, n_chunks)};
        gpu_launch(batch, k->soft_y, n_chunks, 64, y_a, 10);
        GpuArg ry_a[] = {arg_i(n_chunks), arg_v(d_py, n_chunks), arg_v(d_my, 1)};
        gpu_launch(batch, k->soft_red, 1, 32, ry_a, 3);
        res.gpu_ms += gpu_wait(batch) * 1e3;

        int64_t un = 0;
        uint8_t raw_my[8];
        double mixed = 0.0;
        gpu_download(&un, d_un, 8);
        gpu_download(raw_my, d_my, 8);
        dev_to_f64(&mixed, raw_my, 1);
        int64_t got = evgb_sm_nearest(mixed);
        if (un != 0 || got != val_v) {
            res.failure = n;
            return res;
        }
        res.until = n;
    }
    return res;
}

static void run_ch233_gpu_sweep(const Kernels *k) {
    enum { MAX_N = 65536 };
    int64_t *hk = (int64_t *)calloc(MAX_N, sizeof(int64_t));
    int64_t *hv = (int64_t *)calloc(MAX_N, sizeof(int64_t));
    int64_t *ck = (int64_t *)calloc(MAX_N, sizeof(int64_t));
    int64_t *cv = (int64_t *)calloc(MAX_N, sizeof(int64_t));
    GpuBuf dk = gpu_alloc(MAX_N * 8), dv = gpu_alloc(MAX_N * 8);
    GpuBuf dtop = gpu_alloc(8), dpz = gpu_alloc(256 * 8), dtz = gpu_alloc(8);
    GpuBuf dpy = gpu_alloc(256 * 8), dmy = gpu_alloc(8), dun = gpu_alloc(8);

    double b1000 = sm_beta_bound(1000);
    typedef struct { const char *label; int64_t key_k; int64_t val_v; double beta; int fmt; } Case;
    Case cases[] = {
        {"f64  k=16  V=1000 beta=ln(4V)+1   ",  16, 1000, b1000 + 1.0,  0},
        {"f32  k=16  V=1000 beta=ln(4V)+1   ",  16, 1000, b1000 + 1.0,  1},
        {"bf16 k=16  V=1000 beta=ln(4V)+1   ",  16, 1000, b1000 + 1.0,  2},
        {"f64  k=256 V=1000 beta=ln(4V)+1   ", 256, 1000, b1000 + 1.0,  0},
        {"f32  k=256 V=1000 beta=ln(4V)+1   ", 256, 1000, b1000 + 1.0,  1},
        {"bf16 k=256 V=1000 beta=ln(4V)+1   ", 256, 1000, b1000 + 1.0,  2},
        {"f64  k=16  V=1000 beta=ln(4V)+0.01",  16, 1000, b1000 + 0.01, 0},
        {"f64  k=16  V=1000 beta=ln(4V)-1   ",  16, 1000, b1000 - 1.0,  0},
        {"f64  k=16  V=1    beta=0.40       ",  16,    1, 0.4,          0},
    };
    int n_cases = (int)(sizeof(cases) / sizeof(cases[0]));
    printf("\n[4] Device-level Ch 233 softmax vs. hard-max sweep up to N=%d (analytic ln(4000)=%.6f):\n", MAX_N, b1000);
    printf("  %-34s | same(until/fail) | near(until/fail) | CPU oracle | GPU ms\n", "case");
    for (int i = 0; i < n_cases; i++) {
        Case c = cases[i];
        SoftSweep gs = gpu_ch233_scan(k, 0, c.key_k, c.val_v, c.beta, c.fmt, MAX_N, dk, dv, dtop, dpz, dtz, dpy, dmy, dun, hk, hv);
        SoftSweep gn = gpu_ch233_scan(k, 1, c.key_k, c.val_v, c.beta, c.fmt, MAX_N, dk, dv, dtop, dpz, dtz, dpy, dmy, dun, hk, hv);
        coax_t_SmLimit cpu = sm_limit(c.key_k, c.val_v, c.beta, (coax_t_SmFormat)c.fmt, (coax_v_s64){ck, MAX_N}, (coax_v_s64){cv, MAX_N}, 0);
        int match = (cpu.status == COAX_SmStatus_ok &&
                     gs.until == cpu.same.until && gs.failure == cpu.same.failure &&
                     gn.until == cpu.near.until && gn.failure == cpu.near.failure);
        printf("  %-34s | %6d / %-7d | %6d / %-7d | %-10s | %6.3f\n",
               c.label, gs.until, gs.failure, gn.until, gn.failure,
               match ? "EXACT" : "MISMATCH", gs.gpu_ms + gn.gpu_ms);
        if (!match) {
            fprintf(stderr, "Ch 233 GPU vs CPU mismatch on %s: cpu same=%lld/%lld near=%lld/%lld\n",
                    c.label, (long long)cpu.same.until, (long long)cpu.same.failure,
                    (long long)cpu.near.until, (long long)cpu.near.failure);
            exit(1);
        }
    }
    gpu_free(dk); gpu_free(dv); gpu_free(dtop); gpu_free(dpz); gpu_free(dtz); gpu_free(dpy); gpu_free(dmy); gpu_free(dun);
    free(hk); free(hv); free(ck); free(cv);
}

int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: %s KERNELS.{metal|cu}\n", argv[0]); return 2; }

    size_t mem_bytes = 0;
    coax_t_EvGpuProfile prof_i64, prof_f64;
#ifdef EXEC_GPU_METAL
    const char *backend_name = "Metal (exec-gpu-metal-v1)";
    PmDeviceInfo info;
    PmStatus os = pm_open(&info);
    if (os == PM_ABSENT) { puts("SKIP (PM_ABSENT): no Metal GPU device present"); return 0; }
    if (os != PM_OK) { fprintf(stderr, "pm_open failed: %s\n", pm_log()); return 1; }
    printf("Backend: %s\nDevice:  %s (Apple GPU family %d, unified=%d, working set %.1f GiB)\n",
           backend_name, info.name, info.apple_family, info.unified_memory, (double)info.working_set_bytes / 1073741824.0);
    prof_i64 = COAX_EvGpuProfile_metal_i64;
    prof_f64 = COAX_EvGpuProfile_metal_ff;
    if (evg_admit_device(prof_i64, 0, 0, 1024, info.unified_memory, info.apple_family) != COAX_EvGpuStatus_ok ||
        evg_admit_device(prof_f64, 0, 0, 1024, info.unified_memory, info.apple_family) != COAX_EvGpuStatus_ok) {
        fprintf(stderr, "Coax #authority device admission refused this Metal device\n");
        return 1;
    }
    mem_bytes = info.working_set_bytes;
#else
    const char *backend_name = "CUDA (exec-gpu-cuda-v1)";
    PcDeviceInfo info;
    PcStatus os = pc_open(&info);
    if (os == PC_ABSENT || os == PC_NO_NVRTC) {
        printf("SKIP (%s): no NVIDIA CUDA GPU/NVRTC present on this host (%s)\n",
               os == PC_ABSENT ? "PC_ABSENT" : "PC_NO_NVRTC", pc_log());
        puts("exec-gpu-cuda-v1 host compiled cleanly & Coax CUDA kernels emitted; run on coderos-4080 / NVIDIA host for live CUDA execution.");
        return 0;
    }
    if (os != PC_OK) { fprintf(stderr, "pc_open failed: %s\n", pc_log()); return 1; }
    printf("Backend: %s\nDevice:  %s (SM %d.%d, %d SMs, %.1f GiB free / %.1f GiB total)\n",
           backend_name, info.name, info.major, info.minor, info.sms,
           (double)info.free_bytes / 1073741824.0, (double)info.total_bytes / 1073741824.0);
    prof_i64 = COAX_EvGpuProfile_cuda_i64;
    prof_f64 = COAX_EvGpuProfile_cuda_f64;
    if (evg_admit_device(prof_f64, info.major, info.minor, info.max_threads, 0, 0) != COAX_EvGpuStatus_ok ||
        evg_admit_device(prof_i64, info.major, info.minor, info.max_threads, 0, 0) != COAX_EvGpuStatus_ok) {
        fprintf(stderr, "Coax #authority device admission refused this CUDA device\n");
        return 1;
    }
    mem_bytes = info.free_bytes;
    GPU_TRY(pc_stream(&g_stream));
    GPU_TRY(pc_event(&g_ev0));
    GPU_TRY(pc_event(&g_ev1));
#endif

    size_t src_len = 0;
    char *src = read_text_file(argv[1], &src_len);
    if (!src) return 2;
    GpuMod mod = NULL;
    double compile_s = 0.0;
#ifdef EXEC_GPU_METAL
    GPU_TRY(pm_compile(src, src_len, "exec-vm-kernels.metal", &mod, &compile_s));
#else
    GPU_TRY(pc_compile(src, src_len, "exec-vm-kernels.cu", &mod, &compile_s));
#endif
    free(src);
    Kernels kernels;
    load_kernels(mod, &kernels);
    printf("Compiled Coax GPU kernels (fast math OFF / --fmad=false) in %.3f s\n", compile_s);

    /* Compile both W1.1 greedy and W1.3 optimal Ch 226 VM models, serialize & reload with SHA-256. */
    coax_t_EvLayerDesc lg[24], lo[24], lo2[24];
    coax_t_EvHeadDesc  hg[16], ho[16], ho2[16];
    double *wg = (double *)calloc(131072, sizeof(double));
    double *wo = (double *)calloc(131072, sizeof(double));
    double *wo2 = (double *)calloc(131072, sizeof(double));
    coax_t_EvSchedMetrics met[1];

    coax_t_EvModel mg = ev_ch226_compile(64, (coax_v_t_EvLayerDesc){lg, 24}, (coax_v_t_EvHeadDesc){hg, 16}, (coax_v_f64){wg, 131072});
    coax_t_EvModel mo_raw = evs_compile_ch226_metrics(64, (coax_v_t_EvLayerDesc){lo, 24}, (coax_v_t_EvHeadDesc){ho, 16}, (coax_v_f64){wo, 131072}, (coax_v_t_EvSchedMetrics){met, 1});
    if (mg.status != COAX_EvStatus_ok || mo_raw.status != COAX_EvStatus_ok) { fprintf(stderr, "VM compile failed\n"); return 1; }

    uint8_t *sf = (uint8_t *)malloc(1048576);
    coax_t_EvSerialResult ser = ev_serialize(mo_raw, (coax_v_u8){sf, 1048576});
    if (ser.status != COAX_EvStatus_ok) { fprintf(stderr, "ev_serialize failed\n"); return 1; }
    coax_t_EvLoadResult ld = ev_deserialize((coax_v_u8){sf, ser.written}, ser.digest, 1,
                                            (coax_v_t_EvLayerDesc){lo2, 24}, (coax_v_t_EvHeadDesc){ho2, 16}, (coax_v_f64){wo2, 131072});
    if (ld.model.status != COAX_EvStatus_ok) { fprintf(stderr, "ev_deserialize failed\n"); return 1; }
    coax_t_EvModel mo = ld.model;
    char hex[65];
    fmt_hex(ld.digest.e, hex);
    printf("Ch 226 VM greedy  (W1.1): width=%lld layers=%lld heads=%lld weights=%lld\n",
           (long long)mg.width, (long long)mg.n_layers, (long long)mg.n_heads, (long long)mg.n_weights);
    printf("Ch 226 VM optimal (W1.3): width=%lld layers=%lld heads=%lld weights=%lld (safetensors %lld B, sha256 %.16s...)\n",
           (long long)mo.width, (long long)mo.n_layers, (long long)mo.n_heads, (long long)mo.n_weights, (long long)ser.written, hex);

    /* Coax #authority pre-launch admission & typed refusal checks. */
    coax_t_EvGpuAdmit adm_i = evg_admit_model(mo, prof_i64, (int64_t)mem_bytes, 512, 54);
    coax_t_EvGpuAdmit adm_f = evg_admit_model(mo, prof_f64, (int64_t)mem_bytes, 512, 54);
    if (adm_i.status != COAX_EvGpuStatus_ok || adm_f.status != COAX_EvGpuStatus_ok) {
        fprintf(stderr, "model admission unexpectedly refused\n"); return 1;
    }
    double saved_w = wo2[10];
    wo2[10] = 0.5;
    if (evg_admit_model(mo, prof_i64, (int64_t)mem_bytes, 512, 54).status != COAX_EvGpuStatus_non_integer) {
        fprintf(stderr, "expected pre-launch non_integer refusal\n"); return 1;
    }
    wo2[10] = saved_w;
    if (evg_admit_model(mo, prof_i64, adm_i.required_bytes - 1, 512, 54).status != COAX_EvGpuStatus_memory) {
        fprintf(stderr, "expected pre-launch memory refusal\n"); return 1;
    }
    coax_t_TmOp bad_prog[2] = {{COAX_TmCode_push, 16777217}, {COAX_TmCode_ret, 0}};
    if (evg_admit_program((coax_v_t_TmOp){bad_prog, 2}, COAX_EvGpuProfile_metal_ff) != COAX_EvGpuStatus_envelope) {
        fprintf(stderr, "expected pre-launch metal_ff envelope refusal\n"); return 1;
    }
    bad_prog[0].arg = 67108865;
    if (evg_admit_program((coax_v_t_TmOp){bad_prog, 2}, prof_i64) != COAX_EvGpuStatus_envelope) {
        fprintf(stderr, "expected pre-launch i64 envelope refusal\n"); return 1;
    }
    printf("Pre-launch admission: admitted (required %.2f MiB); non-integer, OOM & envelope refusals verified before launch\n",
           (double)adm_i.required_bytes / 1048576.0);

    /* 1. Precision envelope probe on live GPU. */
    run_precision_probe(&kernels);

    /* Load all 54 Ch 226 programs and compute CPU reference traces & digests. */
    enum { N_PROGS = 54, MAX_PROG = 48, BUDGET = 400 };
    coax_t_TmOp progs[N_PROGS * MAX_PROG];
    int prog_lens[N_PROGS];
    int ref_steps[N_PROGS];
    uint8_t ref_digests[N_PROGS][32];
    int64_t t_ref[2048], o_ref[64];
    int sum_ref_steps = 0;

    for (int i = 0; i < N_PROGS; i++) {
        prog_lens[i] = (int)evgb_program(i, (coax_v_t_TmOp){progs + i * MAX_PROG, MAX_PROG});
        if (prog_lens[i] <= 0) { fprintf(stderr, "evgb_program(%d) failed\n", i); return 1; }
        if (evg_admit_program((coax_v_t_TmOp){progs + i * MAX_PROG, prog_lens[i]}, prof_i64) != COAX_EvGpuStatus_ok ||
            evg_admit_program((coax_v_t_TmOp){progs + i * MAX_PROG, prog_lens[i]}, prof_f64) != COAX_EvGpuStatus_ok) {
            fprintf(stderr, "program %d refused\n", i); return 1;
        }
        coax_t_TmResult r = tm_run((coax_v_t_TmOp){progs + i * MAX_PROG, prog_lens[i]}, BUDGET,
                                   (coax_v_s64){t_ref, 2048}, (coax_v_s64){o_ref, 64}, 0);
        if (r.status != COAX_TmStatus_ok) { fprintf(stderr, "tm_run failed on %d\n", i); return 1; }
        ref_steps[i] = (int)r.steps;
        memcpy(ref_digests[i], r.digest.e, 32);
        sum_ref_steps += ref_steps[i];
    }

    /* 2. Full 54-program exactness verification on REAL GPU (3 paths!). */
    double pl_wall = 0, pl_gpu = 0, r_i64_wall = 0, r_i64_gpu = 0, r_f64_wall = 0, r_f64_gpu = 0;
    int pl_steps = 0, r_i64_steps = 0, r_f64_steps = 0;

    run_per_layer_i64(&kernels, &mo, N_PROGS, progs, prog_lens, MAX_PROG, BUDGET, ref_steps, ref_digests, &pl_wall, &pl_gpu, &pl_steps);
    run_rollout_gpu(&kernels, &mo, 0, (double)evg_exact_max(prof_i64), N_PROGS, progs, prog_lens, MAX_PROG, BUDGET, ref_steps, ref_digests, &r_i64_wall, &r_i64_gpu, &r_i64_steps);
    run_rollout_gpu(&kernels, &mo, 1, (double)evg_exact_max(prof_f64), N_PROGS, progs, prog_lens, MAX_PROG, BUDGET, ref_steps, ref_digests, &r_f64_wall, &r_f64_gpu, &r_f64_steps);

    printf("\n[2] 54-program Ch 226 exact-trace verification on REAL GPU (%d total autoregressive steps):\n", sum_ref_steps);
    printf("  per-layer dense i64 (B=54):         54/54 EXACT SHA-256 match (%d steps, wall %.2f ms, GPU %.2f ms)\n", pl_steps, pl_wall * 1e3, pl_gpu * 1e3);
    printf("  persistent dense rollout i64 (B=54): 54/54 EXACT SHA-256 match (%d steps, wall %.2f ms, GPU %.2f ms)\n", r_i64_steps, r_i64_wall * 1e3, r_i64_gpu * 1e3);
    printf("  persistent dense rollout f64 (B=54): 54/54 EXACT SHA-256 match (%d steps, wall %.2f ms, GPU %.2f ms)\n", r_f64_steps, r_f64_wall * 1e3, r_f64_gpu * 1e3);

    /* 3. Honest side-by-side throughput benchmark (4-program Ch 226 suite & batched 54/216). */
    double *kv_cpu = (double *)calloc(131072, sizeof(double));
    double ex_x[128], ex_st[128], ex_h[256];
    coax_t_EvExecArena exec = {{ex_x, 128}, {ex_st, 128}, {ex_h, 256}};

    double t0 = now_s();
    int nat_steps = 0;
    for (int rep = 0; rep < 400; rep++) {
        for (int i = 0; i < 4; i++) {
            coax_t_TmResult r = tm_run((coax_v_t_TmOp){progs + i * MAX_PROG, prog_lens[i]}, BUDGET, (coax_v_s64){t_ref, 2048}, (coax_v_s64){o_ref, 64}, 0);
            nat_steps += (int)r.steps;
        }
    }
    double nat_s = now_s() - t0;

    t0 = now_s();
    int cg_steps = 0;
    for (int rep = 0; rep < 10; rep++) {
        for (int i = 0; i < 4; i++) {
            coax_t_TmResult w = ev_ch226_run(mg, (coax_v_t_TmOp){progs + i * MAX_PROG, prog_lens[i]}, BUDGET, (coax_v_s64){t_ref, 2048}, (coax_v_s64){o_ref, 64}, (coax_v_f64){kv_cpu, 131072}, exec, 0);
            cg_steps += (int)w.steps;
        }
    }
    double cg_s = now_s() - t0;

    t0 = now_s();
    int co_steps = 0;
    for (int rep = 0; rep < 10; rep++) {
        for (int i = 0; i < 4; i++) {
            coax_t_TmResult w = ev_ch226_run(mo, (coax_v_t_TmOp){progs + i * MAX_PROG, prog_lens[i]}, BUDGET, (coax_v_s64){t_ref, 2048}, (coax_v_s64){o_ref, 64}, (coax_v_f64){kv_cpu, 131072}, exec, 0);
            co_steps += (int)w.steps;
        }
    }
    double co_s = now_s() - t0;

    /* Single-program (B=1) GPU per-layer latency across the 4 named programs (417 steps). */
    double b1_wall = 0, b1_gpu = 0;
    int b1_steps = 0;
    for (int i = 0; i < 4; i++) {
        double w1 = 0, g1 = 0; int s1 = 0;
        run_per_layer_i64(&kernels, &mo, 1, progs + i * MAX_PROG, prog_lens + i, MAX_PROG, BUDGET, ref_steps + i, ref_digests + i, &w1, &g1, &s1);
        b1_wall += w1; b1_gpu += g1; b1_steps += s1;
    }

    /* High-throughput batched GPU rollout (B=216 and B=1080 = 20x54 = 73,840 steps in ONE GPU dispatch!). */
    enum { BIG_B = 1080 };
    coax_t_TmOp *big_progs = (coax_t_TmOp *)malloc((size_t)BIG_B * MAX_PROG * sizeof(coax_t_TmOp));
    int big_lens[BIG_B], big_ref_s[BIG_B];
    uint8_t (*big_ref_d)[32] = (uint8_t (*)[32])malloc((size_t)BIG_B * 32);
    for (int b = 0; b < BIG_B; b++) {
        int src_i = b % N_PROGS;
        memcpy(big_progs + b * MAX_PROG, progs + src_i * MAX_PROG, MAX_PROG * sizeof(coax_t_TmOp));
        big_lens[b] = prog_lens[src_i];
        big_ref_s[b] = ref_steps[src_i];
        memcpy(big_ref_d[b], ref_digests[src_i], 32);
    }
    double b216_i_wall = 0, b216_i_gpu = 0, b216_f_wall = 0, b216_f_gpu = 0;
    int b216_i_steps = 0, b216_f_steps = 0;
    run_rollout_gpu(&kernels, &mo, 0, (double)evg_exact_max(prof_i64), 216, big_progs, big_lens, MAX_PROG, BUDGET, big_ref_s, big_ref_d, &b216_i_wall, &b216_i_gpu, &b216_i_steps);
    run_rollout_gpu(&kernels, &mo, 1, (double)evg_exact_max(prof_f64), 216, big_progs, big_lens, MAX_PROG, BUDGET, big_ref_s, big_ref_d, &b216_f_wall, &b216_f_gpu, &b216_f_steps);

    double big_i_wall = 0, big_i_gpu = 0, big_f_wall = 0, big_f_gpu = 0;
    int big_i_steps = 0, big_f_steps = 0;
    run_rollout_gpu(&kernels, &mo, 0, (double)evg_exact_max(prof_i64), BIG_B, big_progs, big_lens, MAX_PROG, BUDGET, big_ref_s, big_ref_d, &big_i_wall, &big_i_gpu, &big_i_steps);
    run_rollout_gpu(&kernels, &mo, 1, (double)evg_exact_max(prof_f64), BIG_B, big_progs, big_lens, MAX_PROG, BUDGET, big_ref_s, big_ref_d, &big_f_wall, &big_f_gpu, &big_f_steps);

    printf("\n[3] Honest throughput & latency table (CPU vs real GPU compiled weights):\n");
    printf("  %-44s | %8s | %10s | %10s | %10s\n", "path", "steps", "steps/s", "us/step", "GPU ms");
    printf("  %-44s | %8d | %10.0f | %10.2f | %10s\n", "Native CPU interpreter (tm_run)", nat_steps, nat_steps / nat_s, 1e6 * nat_s / nat_steps, "-");
    printf("  %-44s | %8d | %10.0f | %10.2f | %10s\n", "CPU dense weights (W1.1 greedy, 41013 w)", cg_steps, cg_steps / cg_s, 1e6 * cg_s / cg_steps, "-");
    printf("  %-44s | %8d | %10.0f | %10.2f | %10s\n", "CPU dense weights (W1.3 optimal, 30192 w)", co_steps, co_steps / co_s, 1e6 * co_s / co_steps, "-");
    printf("  %-44s | %8d | %10.0f | %10.2f | %10.2f\n", "GPU per-layer i64 (B=1, 1 batch/step)", b1_steps, b1_steps / b1_wall, 1e6 * b1_wall / b1_steps, b1_gpu * 1e3);
    printf("  %-44s | %8d | %10.0f | %10.2f | %10.2f\n", "GPU per-layer i64 (B=54 batched)", pl_steps, pl_steps / pl_wall, 1e6 * pl_wall / pl_steps, pl_gpu * 1e3);
    printf("  %-44s | %8d | %10.0f | %10.2f | %10.2f\n", "GPU persistent rollout i64 (B=54)", r_i64_steps, r_i64_steps / r_i64_wall, 1e6 * r_i64_wall / r_i64_steps, r_i64_gpu * 1e3);
    printf("  %-44s | %8d | %10.0f | %10.2f | %10.2f\n", "GPU persistent rollout i64 (B=216)", b216_i_steps, b216_i_steps / b216_i_wall, 1e6 * b216_i_wall / b216_i_steps, b216_i_gpu * 1e3);
    printf("  %-44s | %8d | %10.0f | %10.2f | %10.2f\n", "GPU persistent rollout f64/ff (B=216)", b216_f_steps, b216_f_steps / b216_f_wall, 1e6 * b216_f_wall / b216_f_steps, b216_f_gpu * 1e3);
    printf("  %-44s | %8d | %10.0f | %10.2f | %10.2f\n", "GPU persistent rollout i64 (B=1080)", big_i_steps, big_i_steps / big_i_wall, 1e6 * big_i_wall / big_i_steps, big_i_gpu * 1e3);
    printf("  %-44s | %8d | %10.0f | %10.2f | %10.2f\n", "GPU persistent rollout f64/ff (B=1080)", big_f_steps, big_f_steps / big_f_wall, 1e6 * big_f_wall / big_f_steps, big_f_gpu * 1e3);
    printf("  CPU optimal vs greedy speedup: %.2fx | Note: bare CPU interpreter remains faster for raw loops.\n", (co_steps / co_s) / (cg_steps / cg_s));

    /* 4. Ch 233 device-level softmax precision-cliff sweep on real GPU. */
    run_ch233_gpu_sweep(&kernels);

    free(big_ref_d); free(big_progs); free(kv_cpu); free(sf); free(wg); free(wo); free(wo2);
#ifdef EXEC_GPU_METAL
    pm_unload(mod);
#else
    pc_destroy_event(g_ev0); pc_destroy_event(g_ev1); pc_destroy_stream(g_stream);
    pc_unload(mod);
#endif
    printf("\nALL %s EXACTNESS, PREFLIGHT, BENCHMARK & CH 233 SWEEP CHECKS PASSED!\n", backend_name);
    return 0;
}
