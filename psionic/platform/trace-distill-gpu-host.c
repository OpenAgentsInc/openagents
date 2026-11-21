/* Thin C11 platform host for Executor W3.2 (`trace-distill-v1`, GitHub #13918).
   Compiles cleanly (-Wall -Wextra -Werror) for BOTH:
     - Apple Silicon Metal 4 (-DTD_GPU_METAL + psionic/platform/metal-host.m)
     - NVIDIA CUDA (-DTD_GPU_CUDA + psionic/platform/cuda-host.c)

   All policy, dataset generation, Ch 234 exact replay, Ch 247 exposure-ledger
   split enforcement, Ch 139 RL advantage rules, schedules, initialization,
   and H1/H3/H5 verdicts live in Coax (psionic/lib/trace-distill-policy.coax
   and psionic/lib/trace-distill.coax). All GPU math lives in single-source
   Coax kernels (psionic/lib/trace-distill-kernels.coax).

   ZERO CPU fallback on GPU runs: without a Metal/CUDA device, reports
   PM_ABSENT / PC_ABSENT cleanly and stops.

   Usage: trace-distill-gpu-{metal|cuda} KERNELS.{metal|cu} [--steps N] [--ckpt-dir DIR] */
#define _POSIX_C_SOURCE 200809L
#include "trace-distill-bridge.h"
#ifdef TD_GPU_METAL
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

static int64_t mm_items(int64_t m, int64_t n) {
    return ((m + 63) / 64) * ((n + 63) / 64);
}

/* ---- Unified thin GPU backend adapter (Metal 4 vs CUDA) ---- */

#ifdef TD_GPU_METAL
typedef PmBuffer GpuBuf;
typedef PmModule GpuMod;
typedef PmKernel GpuKern;
typedef PmBatch  GpuBatch;
typedef PmArg    GpuArg;

#define GPU_TRY(x) do { PmStatus s_ = (x); if (s_ != PM_OK) { fprintf(stderr, "%s failed (%d): %s\n", #x, (int)s_, pm_log()); exit(1); } } while (0)

static GpuBuf gpu_alloc(size_t bytes) { GpuBuf b = NULL; GPU_TRY(pm_alloc(bytes ? bytes : 16, &b)); return b; }
static void gpu_free(GpuBuf b) { if (b) GPU_TRY(pm_free(b)); }
static void gpu_upload(GpuBuf dst, size_t off, const void *src, size_t bytes) { memcpy((uint8_t *)pm_contents(dst) + off, src, bytes); }
static void gpu_download(void *dst, GpuBuf src, size_t off, size_t bytes) { memcpy(dst, (uint8_t *)pm_contents(src) + off, bytes); }
static void gpu_zero(GpuBuf dst, size_t bytes) { memset(pm_contents(dst), 0, bytes); }
static void gpu_copy_dev(GpuBuf dst, GpuBuf src, size_t bytes) { memcpy(pm_contents(dst), pm_contents(src), bytes); }
static GpuArg arg_i(int64_t v) { return (GpuArg){PM_ARG_INT, v, 0.0, NULL, 0, 0}; }
static GpuArg arg_f32(float v) { return (GpuArg){PM_ARG_F32, 0, (double)v, NULL, 0, 0}; }
static GpuArg arg_v(GpuBuf b, int64_t byte_off, int64_t count) { return (GpuArg){PM_ARG_VIEW, 0, 0.0, b, byte_off, count}; }
static GpuBatch gpu_begin(void) { GpuBatch b = NULL; GPU_TRY(pm_begin(&b)); return b; }
static void gpu_launch(GpuBatch b, GpuKern k, int64_t items, int lanes, int block, const GpuArg *a, int n) {
    if (items <= 0) return;
    GPU_TRY(pm_launch(b, k, items, lanes, block, a, n));
}
static double gpu_wait(GpuBatch b) { double s = 0.0; GPU_TRY(pm_wait(b, &s)); return s; }
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
static void gpu_free(GpuBuf b) { if (b) GPU_TRY(pc_free(b)); }
static void gpu_upload(GpuBuf dst, size_t off, const void *src, size_t bytes) {
    GPU_TRY(pc_upload(dst + (PcBuffer)off, src, bytes, g_stream));
    GPU_TRY(pc_sync_stream(g_stream));
}
static void gpu_download(void *dst, GpuBuf src, size_t off, size_t bytes) {
    GPU_TRY(pc_download(dst, src + (PcBuffer)off, bytes, g_stream));
    GPU_TRY(pc_sync_stream(g_stream));
}
static void gpu_zero(GpuBuf dst, size_t bytes) {
    GPU_TRY(pc_fill(dst, 0, (bytes + 3) / 4, g_stream));
    GPU_TRY(pc_sync_stream(g_stream));
}
static void gpu_copy_dev(GpuBuf dst, GpuBuf src, size_t bytes) {
    void *tmp = malloc(bytes);
    if (!tmp) { fprintf(stderr, "oom\n"); exit(1); }
    gpu_download(tmp, src, 0, bytes);
    gpu_upload(dst, 0, tmp, bytes);
    free(tmp);
}
static GpuArg arg_i(int64_t v) { return (GpuArg){PC_ARG_INT, v, 0.0, 0, 0}; }
static GpuArg arg_f32(float v) { return (GpuArg){PC_ARG_F32, 0, (double)v, 0, 0}; }
static GpuArg arg_v(GpuBuf b, int64_t byte_off, int64_t count) { return (GpuArg){PC_ARG_VIEW, 0, 0.0, b + (PcBuffer)byte_off, count}; }
static GpuBatch gpu_begin(void) { GPU_TRY(pc_record(g_ev0, g_stream)); return g_stream; }
static void gpu_launch(GpuBatch b, GpuKern k, int64_t items, int lanes, int block, const GpuArg *a, int n) {
    if (items <= 0) return;
    if (lanes > 1) GPU_TRY(pc_launch_warps(k, items * (lanes / 32), lanes, a, n, b));
    else GPU_TRY(pc_launch(k, items, block > 0 ? block : 256, a, n, b));
}
static double gpu_wait(GpuBatch b) {
    double s = 0.0;
    GPU_TRY(pc_record(g_ev1, b));
    GPU_TRY(pc_sync_stream(b));
    GPU_TRY(pc_event_seconds(g_ev0, g_ev1, &s));
    return s;
}
#endif

typedef struct {
    GpuMod mod;
    GpuKern mm_nn, mm_nt, mm_tn;
    GpuKern embed, embed_back, rms, rms_back, rope_qkv, relu2, relu2_back;
    GpuKern attn_scores, margin_loss, add_margin_ds, softmax, softmax_back, attn_out, attn_dp, attn_dqkv;
    GpuKern ce_weighted, aux_ce, kv_write_rope, decode_attn, sum, adamw;
} TdKernels;

static void load_kernels(GpuMod mod, TdKernels *k) {
    k->mod = mod;
#ifdef TD_GPU_METAL
#define K(field, name) GPU_TRY(pm_kernel(mod, name, &k->field))
#else
#define K(field, name) GPU_TRY(pc_kernel(mod, name, &k->field))
#endif
    K(mm_nn,         "tdk_mm_nn");
    K(mm_nt,         "tdk_mm_nt");
    K(mm_tn,         "tdk_mm_tn");
    K(embed,         "tdk_embed");
    K(embed_back,    "tdk_embed_back");
    K(rms,           "tdk_rms");
    K(rms_back,      "tdk_rms_back");
    K(rope_qkv,      "tdk_rope_qkv");
    K(relu2,         "tdk_relu2");
    K(relu2_back,    "tdk_relu2_back");
    K(attn_scores,   "tdk_attn_scores");
    K(margin_loss,   "tdk_margin_loss");
    K(add_margin_ds, "tdk_add_margin_ds");
    K(softmax,       "tdk_softmax");
    K(softmax_back,  "tdk_softmax_back");
    K(attn_out,      "tdk_attn_out");
    K(attn_dp,       "tdk_attn_dp");
    K(attn_dqkv,     "tdk_attn_dqkv");
    K(ce_weighted,   "tdk_ce_weighted");
    K(aux_ce,        "tdk_aux_ce");
    K(kv_write_rope, "tdk_kv_write_rope");
    K(decode_attn,   "tdk_decode_attn");
    K(sum,           "tdk_sum");
    K(adamw,         "tdk_adamw");
#undef K
}

/* ---- GPU Training & Decode State ---- */

typedef struct {
    coax_t_TdShape s;
    int64_t np, nt, st, max_ctx, dec_batch;
    GpuBuf p, p_ref, g, m, v;
    GpuBuf x, h1, r1, qkv, sc, oa, h2, r2, u, a;
    GpuBuf hf, rf, logits, dlog, loss;
    GpuBuf aux_log, daux, aux_loss, mloss, dmargin;
    GpuBuf g_res, dh, da, dout, dp, dqkv, part, scal;
    GpuBuf ids, tg, weights, aux_tg, aux_mask, src_pos, rope;
    /* KV-cached decode buffers */
    GpuBuf k_cache, v_cache;
    GpuBuf dec_ids, dec_x, dec_h, dec_r, dec_qkv, dec_o, dec_u, dec_a, dec_logits;
} TdGpuState;

static void td_gpu_alloc(TdGpuState *st, coax_t_TdShape s, int64_t max_ctx, int64_t dec_batch) {
    memset(st, 0, sizeof *st);
    st->s = s;
    st->np = td_total_params(s);
    st->nt = s.rows * s.train_ctx;
    st->st = s.rows * s.heads * s.train_ctx * s.train_ctx;
    st->max_ctx = max_ctx;
    st->dec_batch = dec_batch;
    int64_t d = s.d, L = s.layers, h3 = 3 * d, hid = s.hidden, nt = st->nt, np = st->np;

    st->p = gpu_alloc((size_t)np * 4);
    st->p_ref = gpu_alloc((size_t)np * 4);
    st->g = gpu_alloc((size_t)np * 4);
    st->m = gpu_alloc((size_t)np * 4);
    st->v = gpu_alloc((size_t)np * 4);

    st->x = gpu_alloc((size_t)(nt * d) * 4);
    st->h1 = gpu_alloc((size_t)(L * nt * d) * 4);
    st->r1 = gpu_alloc((size_t)(L * nt) * 4);
    st->qkv = gpu_alloc((size_t)(L * nt * h3) * 4);
    st->sc = gpu_alloc((size_t)(L * st->st) * 4);
    st->oa = gpu_alloc((size_t)(L * nt * d) * 4);
    st->h2 = gpu_alloc((size_t)(L * nt * d) * 4);
    st->r2 = gpu_alloc((size_t)(L * nt) * 4);
    st->u = gpu_alloc((size_t)(L * nt * hid) * 4);
    st->a = gpu_alloc((size_t)(L * nt * hid) * 4);

    st->hf = gpu_alloc((size_t)(nt * d) * 4);
    st->rf = gpu_alloc((size_t)nt * 4);
    st->logits = gpu_alloc((size_t)(nt * s.vocab) * 4);
    st->dlog = gpu_alloc((size_t)(nt * s.vocab) * 4);
    st->loss = gpu_alloc((size_t)nt * 4);
    st->aux_log = gpu_alloc((size_t)(nt * s.aux_dim) * 4);
    st->daux = gpu_alloc((size_t)(nt * s.aux_dim) * 4);
    st->aux_loss = gpu_alloc((size_t)nt * 4);
    st->mloss = gpu_alloc((size_t)nt * 4);
    st->dmargin = gpu_alloc((size_t)(s.rows * s.train_ctx * s.train_ctx) * 4);

    st->g_res = gpu_alloc((size_t)(nt * d) * 4);
    st->dh = gpu_alloc((size_t)(nt * d) * 4);
    st->da = gpu_alloc((size_t)(nt * hid) * 4);
    st->dout = gpu_alloc((size_t)(nt * d) * 4);
    st->dp = gpu_alloc((size_t)st->st * 4);
    st->dqkv = gpu_alloc((size_t)(nt * h3) * 4);
    st->part = gpu_alloc((size_t)((np + 4095) / 4096 + 4) * 4);
    st->scal = gpu_alloc(32);

    st->ids = gpu_alloc((size_t)nt);
    st->tg = gpu_alloc((size_t)nt);
    st->weights = gpu_alloc((size_t)nt * 4);
    st->aux_tg = gpu_alloc((size_t)(nt * 4));
    st->aux_mask = gpu_alloc((size_t)nt);
    st->src_pos = gpu_alloc((size_t)nt * 8);

    int64_t hd = d / s.heads;
    st->rope = gpu_alloc((size_t)(max_ctx * hd) * 4);
    float *h_rope = (float *)malloc((size_t)(max_ctx * hd) * 4);
    td_rope_table(hd, max_ctx, (coax_v_f32){h_rope, max_ctx * hd});
    gpu_upload(st->rope, 0, h_rope, (size_t)(max_ctx * hd) * 4);
    free(h_rope);

    if (dec_batch > 0) {
        st->k_cache = gpu_alloc((size_t)(L * dec_batch * max_ctx * d) * 4);
        st->v_cache = gpu_alloc((size_t)(L * dec_batch * max_ctx * d) * 4);
        st->dec_ids = gpu_alloc((size_t)dec_batch);
        st->dec_x = gpu_alloc((size_t)(dec_batch * d) * 4);
        st->dec_h = gpu_alloc((size_t)(dec_batch * d) * 4);
        st->dec_r = gpu_alloc((size_t)dec_batch * 4);
        st->dec_qkv = gpu_alloc((size_t)(dec_batch * h3) * 4);
        st->dec_o = gpu_alloc((size_t)(dec_batch * d) * 4);
        st->dec_u = gpu_alloc((size_t)(dec_batch * hid) * 4);
        st->dec_a = gpu_alloc((size_t)(dec_batch * hid) * 4);
        st->dec_logits = gpu_alloc((size_t)(dec_batch * s.vocab) * 4);
    }
}

static void td_gpu_free(TdGpuState *st) {
    gpu_free(st->p); gpu_free(st->p_ref); gpu_free(st->g); gpu_free(st->m); gpu_free(st->v);
    gpu_free(st->x); gpu_free(st->h1); gpu_free(st->r1); gpu_free(st->qkv); gpu_free(st->sc);
    gpu_free(st->oa); gpu_free(st->h2); gpu_free(st->r2); gpu_free(st->u); gpu_free(st->a);
    gpu_free(st->hf); gpu_free(st->rf); gpu_free(st->logits); gpu_free(st->dlog); gpu_free(st->loss);
    gpu_free(st->aux_log); gpu_free(st->daux); gpu_free(st->aux_loss); gpu_free(st->mloss); gpu_free(st->dmargin);
    gpu_free(st->g_res); gpu_free(st->dh); gpu_free(st->da); gpu_free(st->dout); gpu_free(st->dp);
    gpu_free(st->dqkv); gpu_free(st->part); gpu_free(st->scal);
    gpu_free(st->ids); gpu_free(st->tg); gpu_free(st->weights); gpu_free(st->aux_tg);
    gpu_free(st->aux_mask); gpu_free(st->src_pos); gpu_free(st->rope);
    gpu_free(st->k_cache); gpu_free(st->v_cache); gpu_free(st->dec_ids); gpu_free(st->dec_x);
    gpu_free(st->dec_h); gpu_free(st->dec_r); gpu_free(st->dec_qkv); gpu_free(st->dec_o);
    gpu_free(st->dec_u); gpu_free(st->dec_a); gpu_free(st->dec_logits);
}

/* Encodes full forward (+ optional backward + loss/grad-norm reductions) in ONE GPU batch. */
static double gpu_fwd_bwd(
    const TdKernels *k, TdGpuState *st, GpuBuf p_buf, coax_t_TdArm arm,
    float beta_para, int use_aux, float scale_aux, int use_margin, float margin_val, float margin_w,
    int do_bwd
) {
    coax_t_TdShape s = st->s;
    int64_t d = s.d, L = s.layers, heads = s.heads, hd = d / heads, h3 = 3 * d, hid = s.hidden;
    int64_t t = s.train_ctx, rows = s.rows, nt = st->nt, bh = rows * heads, sc_n = st->st, np = st->np;
    int64_t rope_n = st->max_ctx * hd;

    GpuBatch b = gpu_begin();
    /* Embed */
    {
        GpuArg a[] = {arg_i(d), arg_v(st->ids, 0, nt), arg_v(p_buf, 0, s.vocab * d), arg_v(st->x, 0, nt * d)};
        gpu_launch(b, k->embed, nt * d, 1, 256, a, 4);
    }
    /* Forward layers */
    for (int64_t l = 0; l < L; l++) {
        int64_t para = td_para_heads(arm, l);
        int64_t h1_off = l * nt * d * 4, r1_off = l * nt * 4, qkv_off = l * nt * h3 * 4;
        int64_t sc_off = l * sc_n * 4, oa_off = l * nt * d * 4;
        int64_t h2_off = l * nt * d * 4, r2_off = l * nt * 4, u_off = l * nt * hid * 4;
        int64_t oq = td_off_qkv(s, l) * 4, oo = td_off_o(s, l) * 4, ou = td_off_up(s, l) * 4, od = td_off_down(s, l) * 4;

        { GpuArg a[] = {arg_i(d), arg_v(st->x, 0, nt * d), arg_v(st->h1, h1_off, nt * d), arg_v(st->r1, r1_off, nt)};
          gpu_launch(b, k->rms, nt, 32, 32, a, 4); }
        { GpuArg a[] = {arg_i(nt), arg_i(h3), arg_i(d), arg_i(0), arg_v(st->h1, h1_off, nt * d), arg_v(p_buf, oq, d * h3), arg_v(st->qkv, qkv_off, nt * h3)};
          gpu_launch(b, k->mm_nn, mm_items(nt, h3), 256, 256, a, 7); }
        { GpuArg a[] = {arg_i(t), arg_i(d), arg_i(heads), arg_i(para), arg_i(0), arg_v(st->rope, 0, rope_n), arg_v(st->qkv, qkv_off, nt * h3)};
          gpu_launch(b, k->rope_qkv, nt * heads * (hd / 2), 1, 256, a, 7); }
        { GpuArg a[] = {arg_i(t), arg_i(d), arg_i(heads), arg_i(para), arg_f32(beta_para), arg_v(st->qkv, qkv_off, nt * h3), arg_v(st->sc, sc_off, sc_n)};
          gpu_launch(b, k->attn_scores, sc_n, 1, 256, a, 7); }
        if (use_margin && l == 0) {
            GpuArg a[] = {arg_i(t), arg_i(heads), arg_f32(margin_val), arg_f32(margin_w),
                arg_v(st->sc, sc_off, sc_n), arg_v(st->src_pos, 0, nt),
                arg_v(st->mloss, 0, nt), arg_v(st->dmargin, 0, rows * t * t)};
            gpu_launch(b, k->margin_loss, nt, 1, 256, a, 8);
        }
        { GpuArg a[] = {arg_i(t), arg_v(st->sc, sc_off, sc_n)};
          gpu_launch(b, k->softmax, bh * t, 32, 32, a, 2); }
        { GpuArg a[] = {arg_i(t), arg_i(d), arg_i(heads), arg_v(st->sc, sc_off, sc_n), arg_v(st->qkv, qkv_off, nt * h3), arg_v(st->oa, oa_off, nt * d)};
          gpu_launch(b, k->attn_out, nt * d, 1, 256, a, 6); }
        { GpuArg a[] = {arg_i(nt), arg_i(d), arg_i(d), arg_i(1), arg_v(st->oa, oa_off, nt * d), arg_v(p_buf, oo, d * d), arg_v(st->x, 0, nt * d)};
          gpu_launch(b, k->mm_nn, mm_items(nt, d), 256, 256, a, 7); }
        { GpuArg a[] = {arg_i(d), arg_v(st->x, 0, nt * d), arg_v(st->h2, h2_off, nt * d), arg_v(st->r2, r2_off, nt)};
          gpu_launch(b, k->rms, nt, 32, 32, a, 4); }
        { GpuArg a[] = {arg_i(nt), arg_i(hid), arg_i(d), arg_i(0), arg_v(st->h2, h2_off, nt * d), arg_v(p_buf, ou, d * hid), arg_v(st->u, u_off, nt * hid)};
          gpu_launch(b, k->mm_nn, mm_items(nt, hid), 256, 256, a, 7); }
        { GpuArg a[] = {arg_v(st->u, u_off, nt * hid), arg_v(st->a, u_off, nt * hid)};
          gpu_launch(b, k->relu2, nt * hid, 1, 256, a, 2); }
        { GpuArg a[] = {arg_i(nt), arg_i(d), arg_i(hid), arg_i(1), arg_v(st->a, u_off, nt * hid), arg_v(p_buf, od, hid * d), arg_v(st->x, 0, nt * d)};
          gpu_launch(b, k->mm_nn, mm_items(nt, d), 256, 256, a, 7); }
    }
    /* Final norm + LM head + weighted CE */
    { GpuArg a[] = {arg_i(d), arg_v(st->x, 0, nt * d), arg_v(st->hf, 0, nt * d), arg_v(st->rf, 0, nt)};
      gpu_launch(b, k->rms, nt, 32, 32, a, 4); }
    int64_t oh = td_off_head(s) * 4, oa = td_off_aux(s) * 4;
    { GpuArg a[] = {arg_i(nt), arg_i(s.vocab), arg_i(d), arg_i(0), arg_v(st->hf, 0, nt * d), arg_v(p_buf, oh, d * s.vocab), arg_v(st->logits, 0, nt * s.vocab)};
      gpu_launch(b, k->mm_nn, mm_items(nt, s.vocab), 256, 256, a, 7); }
    { GpuArg a[] = {arg_i(s.vocab), arg_v(st->logits, 0, nt * s.vocab), arg_v(st->tg, 0, nt), arg_v(st->weights, 0, nt), arg_v(st->loss, 0, nt), arg_v(st->dlog, 0, nt * s.vocab)};
      gpu_launch(b, k->ce_weighted, nt, 1, 256, a, 6); }
    if (use_aux) {
        { GpuArg a[] = {arg_i(nt), arg_i(s.aux_dim), arg_i(d), arg_i(0), arg_v(st->hf, 0, nt * d), arg_v(p_buf, oa, d * s.aux_dim), arg_v(st->aux_log, 0, nt * s.aux_dim)};
          gpu_launch(b, k->mm_nn, mm_items(nt, s.aux_dim), 256, 256, a, 7); }
        { GpuArg a[] = {arg_f32(scale_aux), arg_v(st->aux_log, 0, nt * s.aux_dim), arg_v(st->aux_tg, 0, nt * 4), arg_v(st->aux_mask, 0, nt), arg_v(st->aux_loss, 0, nt), arg_v(st->daux, 0, nt * s.aux_dim)};
          gpu_launch(b, k->aux_ce, nt, 1, 256, a, 6); }
    }

    if (do_bwd) {
        /* Head & Aux backward */
        { GpuArg a[] = {arg_i(d), arg_i(s.vocab), arg_i(nt), arg_i(0), arg_v(st->hf, 0, nt * d), arg_v(st->dlog, 0, nt * s.vocab), arg_v(st->g, oh, d * s.vocab)};
          gpu_launch(b, k->mm_tn, mm_items(d, s.vocab), 256, 256, a, 7); }
        { GpuArg a[] = {arg_i(nt), arg_i(d), arg_i(s.vocab), arg_i(0), arg_v(st->dlog, 0, nt * s.vocab), arg_v(p_buf, oh, d * s.vocab), arg_v(st->dh, 0, nt * d)};
          gpu_launch(b, k->mm_nt, mm_items(nt, d), 256, 256, a, 7); }
        if (use_aux) {
            { GpuArg a[] = {arg_i(d), arg_i(s.aux_dim), arg_i(nt), arg_i(0), arg_v(st->hf, 0, nt * d), arg_v(st->daux, 0, nt * s.aux_dim), arg_v(st->g, oa, d * s.aux_dim)};
              gpu_launch(b, k->mm_tn, mm_items(d, s.aux_dim), 256, 256, a, 7); }
            { GpuArg a[] = {arg_i(nt), arg_i(d), arg_i(s.aux_dim), arg_i(1), arg_v(st->daux, 0, nt * s.aux_dim), arg_v(p_buf, oa, d * s.aux_dim), arg_v(st->dh, 0, nt * d)};
              gpu_launch(b, k->mm_nt, mm_items(nt, d), 256, 256, a, 7); }
        }
        { GpuArg a[] = {arg_i(d), arg_i(0), arg_v(st->hf, 0, nt * d), arg_v(st->rf, 0, nt), arg_v(st->dh, 0, nt * d), arg_v(st->g_res, 0, nt * d)};
          gpu_launch(b, k->rms_back, nt, 32, 32, a, 6); }

        for (int64_t bk = 0; bk < L; bk++) {
            int64_t l = L - 1 - bk;
            int64_t para = td_para_heads(arm, l);
            int64_t h1_off = l * nt * d * 4, r1_off = l * nt * 4, qkv_off = l * nt * h3 * 4;
            int64_t sc_off = l * sc_n * 4, oa_off = l * nt * d * 4;
            int64_t h2_off = l * nt * d * 4, r2_off = l * nt * 4, u_off = l * nt * hid * 4;
            int64_t oq = td_off_qkv(s, l) * 4, oo = td_off_o(s, l) * 4, ou = td_off_up(s, l) * 4, od = td_off_down(s, l) * 4;

            /* MLP backward */
            { GpuArg a[] = {arg_i(nt), arg_i(hid), arg_i(d), arg_i(0), arg_v(st->g_res, 0, nt * d), arg_v(p_buf, od, hid * d), arg_v(st->da, 0, nt * hid)};
              gpu_launch(b, k->mm_nt, mm_items(nt, hid), 256, 256, a, 7); }
            { GpuArg a[] = {arg_i(hid), arg_i(d), arg_i(nt), arg_i(0), arg_v(st->a, u_off, nt * hid), arg_v(st->g_res, 0, nt * d), arg_v(st->g, od, hid * d)};
              gpu_launch(b, k->mm_tn, mm_items(hid, d), 256, 256, a, 7); }
            { GpuArg a[] = {arg_v(st->u, u_off, nt * hid), arg_v(st->da, 0, nt * hid)};
              gpu_launch(b, k->relu2_back, nt * hid, 1, 256, a, 2); }
            { GpuArg a[] = {arg_i(nt), arg_i(d), arg_i(hid), arg_i(0), arg_v(st->da, 0, nt * hid), arg_v(p_buf, ou, d * hid), arg_v(st->dh, 0, nt * d)};
              gpu_launch(b, k->mm_nt, mm_items(nt, d), 256, 256, a, 7); }
            { GpuArg a[] = {arg_i(d), arg_i(hid), arg_i(nt), arg_i(0), arg_v(st->h2, h2_off, nt * d), arg_v(st->da, 0, nt * hid), arg_v(st->g, ou, d * hid)};
              gpu_launch(b, k->mm_tn, mm_items(d, hid), 256, 256, a, 7); }
            { GpuArg a[] = {arg_i(d), arg_i(1), arg_v(st->h2, h2_off, nt * d), arg_v(st->r2, r2_off, nt), arg_v(st->dh, 0, nt * d), arg_v(st->g_res, 0, nt * d)};
              gpu_launch(b, k->rms_back, nt, 32, 32, a, 6); }

            /* Attention backward */
            { GpuArg a[] = {arg_i(nt), arg_i(d), arg_i(d), arg_i(0), arg_v(st->g_res, 0, nt * d), arg_v(p_buf, oo, d * d), arg_v(st->dout, 0, nt * d)};
              gpu_launch(b, k->mm_nt, mm_items(nt, d), 256, 256, a, 7); }
            { GpuArg a[] = {arg_i(d), arg_i(d), arg_i(nt), arg_i(0), arg_v(st->oa, oa_off, nt * d), arg_v(st->g_res, 0, nt * d), arg_v(st->g, oo, d * d)};
              gpu_launch(b, k->mm_tn, mm_items(d, d), 256, 256, a, 7); }
            { GpuArg a[] = {arg_i(t), arg_i(d), arg_i(heads), arg_v(st->dout, 0, nt * d), arg_v(st->qkv, qkv_off, nt * h3), arg_v(st->dp, 0, sc_n)};
              gpu_launch(b, k->attn_dp, sc_n, 1, 256, a, 6); }
            { GpuArg a[] = {arg_i(t), arg_v(st->sc, sc_off, sc_n), arg_v(st->dp, 0, sc_n)};
              gpu_launch(b, k->softmax_back, bh * t, 32, 32, a, 3); }
            if (use_margin && l == 0) {
                GpuArg a[] = {arg_i(t), arg_i(heads), arg_v(st->dmargin, 0, rows * t * t), arg_v(st->dp, 0, sc_n)};
                gpu_launch(b, k->add_margin_ds, rows * t * t, 1, 256, a, 4);
            }
            { GpuArg a[] = {arg_i(t), arg_i(d), arg_i(heads), arg_i(para), arg_f32(beta_para),
                arg_v(st->sc, sc_off, sc_n), arg_v(st->dp, 0, sc_n), arg_v(st->qkv, qkv_off, nt * h3),
                arg_v(st->dout, 0, nt * d), arg_v(st->dqkv, 0, nt * h3)};
              gpu_launch(b, k->attn_dqkv, nt * h3, 1, 256, a, 10); }
            { GpuArg a[] = {arg_i(t), arg_i(d), arg_i(heads), arg_i(para), arg_i(1), arg_v(st->rope, 0, rope_n), arg_v(st->dqkv, 0, nt * h3)};
              gpu_launch(b, k->rope_qkv, nt * heads * (hd / 2), 1, 256, a, 7); }
            { GpuArg a[] = {arg_i(nt), arg_i(d), arg_i(h3), arg_i(0), arg_v(st->dqkv, 0, nt * h3), arg_v(p_buf, oq, d * h3), arg_v(st->dh, 0, nt * d)};
              gpu_launch(b, k->mm_nt, mm_items(nt, d), 256, 256, a, 7); }
            { GpuArg a[] = {arg_i(d), arg_i(h3), arg_i(nt), arg_i(0), arg_v(st->h1, h1_off, nt * d), arg_v(st->dqkv, 0, nt * h3), arg_v(st->g, oq, d * h3)};
              gpu_launch(b, k->mm_tn, mm_items(d, h3), 256, 256, a, 7); }
            { GpuArg a[] = {arg_i(d), arg_i(1), arg_v(st->h1, h1_off, nt * d), arg_v(st->r1, r1_off, nt), arg_v(st->dh, 0, nt * d), arg_v(st->g_res, 0, nt * d)};
              gpu_launch(b, k->rms_back, nt, 32, 32, a, 6); }
        }
        { GpuArg a[] = {arg_i(d), arg_i(nt), arg_i(0), arg_v(st->ids, 0, nt), arg_v(st->g_res, 0, nt * d), arg_v(st->g, 0, s.vocab * d)};
          gpu_launch(b, k->embed_back, s.vocab * d, 1, 256, a, 6); }

        /* Deterministic reductions: scal[0] = sum(loss), scal[1] = sum(g^2), scal[2] = sum(aux_loss), scal[3] = sum(mloss) */
        int64_t lp = (nt + 4095) / 4096;
        int64_t gp = (np + 4095) / 4096;
        { GpuArg a[] = {arg_i(nt), arg_i(0), arg_v(st->loss, 0, nt), arg_v(st->part, 0, lp)};
          gpu_launch(b, k->sum, lp, 256, 256, a, 4); }
        { GpuArg a[] = {arg_i(lp), arg_i(0), arg_v(st->part, 0, lp), arg_v(st->scal, 0, 1)};
          gpu_launch(b, k->sum, 1, 256, 256, a, 4); }
        { GpuArg a[] = {arg_i(np), arg_i(1), arg_v(st->g, 0, np), arg_v(st->part, 0, gp)};
          gpu_launch(b, k->sum, gp, 256, 256, a, 4); }
        { GpuArg a[] = {arg_i(gp), arg_i(0), arg_v(st->part, 0, gp), arg_v(st->scal, 4, 1)};
          gpu_launch(b, k->sum, 1, 256, 256, a, 4); }
        if (use_aux) {
            { GpuArg a[] = {arg_i(nt), arg_i(0), arg_v(st->aux_loss, 0, nt), arg_v(st->part, 0, lp)};
              gpu_launch(b, k->sum, lp, 256, 256, a, 4); }
            { GpuArg a[] = {arg_i(lp), arg_i(0), arg_v(st->part, 0, lp), arg_v(st->scal, 8, 1)};
              gpu_launch(b, k->sum, 1, 256, 256, a, 4); }
        }
        if (use_margin) {
            { GpuArg a[] = {arg_i(nt), arg_i(0), arg_v(st->mloss, 0, nt), arg_v(st->part, 0, lp)};
              gpu_launch(b, k->sum, lp, 256, 256, a, 4); }
            { GpuArg a[] = {arg_i(lp), arg_i(0), arg_v(st->part, 0, lp), arg_v(st->scal, 12, 1)};
              gpu_launch(b, k->sum, 1, 256, 256, a, 4); }
        }
    }
    return gpu_wait(b);
}

static double gpu_adamw_step(const TdKernels *k, TdGpuState *st, coax_t_TdDecision dec, int64_t active_params) {
    GpuBatch b = gpu_begin();
    GpuArg a[] = {
        arg_f32(dec.keep), arg_f32(dec.b2), arg_f32(dec.wb1), arg_f32(dec.wb2),
        arg_f32(dec.step), arg_f32(dec.root2), arg_f32(dec.eps), arg_f32(dec.scale),
        arg_v(st->p, 0, st->np), arg_v(st->g, 0, st->np), arg_v(st->m, 0, st->np), arg_v(st->v, 0, st->np)
    };
    gpu_launch(b, k->adamw, active_params, 1, 256, a, 12);
    return gpu_wait(b);
}

/* Single KV-cached decode step for B_dec sequences at position `pos` (all 6 layers in ONE command buffer). */
static double gpu_decode_step(
    const TdKernels *k, TdGpuState *st, GpuBuf p_buf, coax_t_TdArm arm,
    float beta_para, int64_t b_dec, int64_t pos, const uint8_t *cur_ids, float *out_logits
) {
    coax_t_TdShape s = st->s;
    int64_t d = s.d, L = s.layers, heads = s.heads, hd = d / heads, h3 = 3 * d, hid = s.hidden;
    int64_t max_ctx = st->max_ctx, rope_n = max_ctx * hd;
    int64_t kv_layer_elems = st->dec_batch * max_ctx * d;

    gpu_upload(st->dec_ids, 0, cur_ids, (size_t)b_dec);
    GpuBatch b = gpu_begin();
    { GpuArg a[] = {arg_i(d), arg_v(st->dec_ids, 0, b_dec), arg_v(p_buf, 0, s.vocab * d), arg_v(st->dec_x, 0, b_dec * d)};
      gpu_launch(b, k->embed, b_dec * d, 1, 256, a, 4); }
    for (int64_t l = 0; l < L; l++) {
        int64_t para = td_para_heads(arm, l);
        int64_t kv_off = l * kv_layer_elems * 4;
        int64_t oq = td_off_qkv(s, l) * 4, oo = td_off_o(s, l) * 4, ou = td_off_up(s, l) * 4, od = td_off_down(s, l) * 4;
        { GpuArg a[] = {arg_i(d), arg_v(st->dec_x, 0, b_dec * d), arg_v(st->dec_h, 0, b_dec * d), arg_v(st->dec_r, 0, b_dec)};
          gpu_launch(b, k->rms, b_dec, 32, 32, a, 4); }
        { GpuArg a[] = {arg_i(b_dec), arg_i(h3), arg_i(d), arg_i(0), arg_v(st->dec_h, 0, b_dec * d), arg_v(p_buf, oq, d * h3), arg_v(st->dec_qkv, 0, b_dec * h3)};
          gpu_launch(b, k->mm_nn, mm_items(b_dec, h3), 256, 256, a, 7); }
        { GpuArg a[] = {arg_i(max_ctx), arg_i(d), arg_i(heads), arg_i(para), arg_i(pos),
            arg_v(st->rope, 0, rope_n), arg_v(st->dec_qkv, 0, b_dec * h3),
            arg_v(st->k_cache, kv_off, kv_layer_elems), arg_v(st->v_cache, kv_off, kv_layer_elems)};
          gpu_launch(b, k->kv_write_rope, b_dec * heads * (hd / 2), 1, 256, a, 9); }
        { GpuArg a[] = {arg_i(max_ctx), arg_i(d), arg_i(heads), arg_i(para), arg_f32(beta_para), arg_i(pos),
            arg_v(st->dec_qkv, 0, b_dec * h3), arg_v(st->k_cache, kv_off, kv_layer_elems),
            arg_v(st->v_cache, kv_off, kv_layer_elems), arg_v(st->dec_o, 0, b_dec * d)};
          gpu_launch(b, k->decode_attn, b_dec * heads, 32, 32, a, 10); }
        { GpuArg a[] = {arg_i(b_dec), arg_i(d), arg_i(d), arg_i(1), arg_v(st->dec_o, 0, b_dec * d), arg_v(p_buf, oo, d * d), arg_v(st->dec_x, 0, b_dec * d)};
          gpu_launch(b, k->mm_nn, mm_items(b_dec, d), 256, 256, a, 7); }
        { GpuArg a[] = {arg_i(d), arg_v(st->dec_x, 0, b_dec * d), arg_v(st->dec_h, 0, b_dec * d), arg_v(st->dec_r, 0, b_dec)};
          gpu_launch(b, k->rms, b_dec, 32, 32, a, 4); }
        { GpuArg a[] = {arg_i(b_dec), arg_i(hid), arg_i(d), arg_i(0), arg_v(st->dec_h, 0, b_dec * d), arg_v(p_buf, ou, d * hid), arg_v(st->dec_u, 0, b_dec * hid)};
          gpu_launch(b, k->mm_nn, mm_items(b_dec, hid), 256, 256, a, 7); }
        { GpuArg a[] = {arg_v(st->dec_u, 0, b_dec * hid), arg_v(st->dec_a, 0, b_dec * hid)};
          gpu_launch(b, k->relu2, b_dec * hid, 1, 256, a, 2); }
        { GpuArg a[] = {arg_i(b_dec), arg_i(d), arg_i(hid), arg_i(1), arg_v(st->dec_a, 0, b_dec * hid), arg_v(p_buf, od, hid * d), arg_v(st->dec_x, 0, b_dec * d)};
          gpu_launch(b, k->mm_nn, mm_items(b_dec, d), 256, 256, a, 7); }
    }
    int64_t oh = td_off_head(s) * 4;
    { GpuArg a[] = {arg_i(d), arg_v(st->dec_x, 0, b_dec * d), arg_v(st->dec_h, 0, b_dec * d), arg_v(st->dec_r, 0, b_dec)};
      gpu_launch(b, k->rms, b_dec, 32, 32, a, 4); }
    { GpuArg a[] = {arg_i(b_dec), arg_i(s.vocab), arg_i(d), arg_i(0), arg_v(st->dec_h, 0, b_dec * d), arg_v(p_buf, oh, d * s.vocab), arg_v(st->dec_logits, 0, b_dec * s.vocab)};
      gpu_launch(b, k->mm_nn, mm_items(b_dec, s.vocab), 256, 256, a, 7); }
    double gs = gpu_wait(b);
    gpu_download(out_logits, st->dec_logits, 0, (size_t)(b_dec * s.vocab) * 4);
    return gs;
}

/* ---- [1] Live GPU-vs-CPU Forward, Backward, & KV-Decode Parity Verification ---- */

static void verify_gpu_cpu_parity(const TdKernels *k) {
    coax_t_TdShape tiny = td_tiny_shape();
    int64_t np = td_total_params(tiny), nt = tiny.rows * tiny.train_ctx;
    TdGpuState st;
    td_gpu_alloc(&st, tiny, 64, tiny.rows);

    float *hp = (float *)calloc((size_t)np, 4);
    float *hg_cpu = (float *)calloc((size_t)np, 4);
    float *hg_gpu = (float *)calloc((size_t)np, 4);
    float *h_rope = (float *)calloc(1024, 4);
    td_rope_table(tiny.d / tiny.heads, 64, (coax_v_f32){h_rope, 1024});

    uint8_t ids[64], tg[64], aux_tg[256], aux_mask[64];
    int64_t src_pos[64];
    float w_pos[64], w_rl[64];
    for (int i = 0; i < 64; i++) {
        ids[i] = (uint8_t)(64 + ((i * 7 + 3) % 32));
        tg[i] = (uint8_t)(64 + ((i * 5 + 1) % 32));
        w_pos[i] = (i % 32) >= 8 ? 0.02f : 0.0f;
        w_rl[i] = (i % 32) >= 8 ? (i < 32 ? 0.02f : -0.015f) : 0.0f;
        aux_mask[i] = (i % 6) == 0 ? 1 : 0;
        aux_tg[4 * i] = (uint8_t)(i % 32);
        aux_tg[4 * i + 1] = (uint8_t)((i + 1) % 32);
        aux_tg[4 * i + 2] = (uint8_t)((i * 3) % 64);
        aux_tg[4 * i + 3] = (uint8_t)((i * 5) % 64);
        int pos = i % 32;
        src_pos[i] = (pos >= 4 && (pos % 5 == 0)) ? pos - 3 : -1;
    }
    coax_t_TdTinyWork *work = (coax_t_TdTinyWork *)calloc(1, sizeof *work);
    float *full_logits = (float *)calloc((size_t)(nt * tiny.vocab), 4);
    float *step_logits = (float *)calloc((size_t)(tiny.rows * tiny.vocab), 4);

    printf("\n[1] Live GPU vs CPU Reference Parity (td_tiny_shape, all 3 arms + KV-decode):\n");
    const char *arm_names[3] = {"Baseline A (SFT)", "Variant B (Aux+RL)", "Variant C (Parabolic+Margin)"};
    for (int ai = 0; ai < 3; ai++) {
        coax_t_TdArm arm = (coax_t_TdArm)ai;
        if (td_init_weights(tiny, arm, (coax_v_f32){hp, np}) != COAX_TdStatus_ok) { fprintf(stderr, "init failed\n"); exit(1); }
        float beta = (arm == COAX_TdArm_variant_c) ? 2.5f : 1.0f;
        float aux_w = (arm == COAX_TdArm_variant_b) ? 0.25f : 0.0f;
        float mar_w = (arm == COAX_TdArm_variant_c) ? 0.05f : 0.0f;
        float *w_use = (arm == COAX_TdArm_variant_b) ? w_rl : w_pos;
        int n_aux = 0;
        for (int i = 0; i < 64; i++) if (aux_mask[i]) n_aux++;
        float scale_aux = (n_aux > 0) ? aux_w / (float)(4 * n_aux) : 0.0f;

        td_tiny_cpu_step(arm, beta, aux_w, 2.0f, mar_w,
            (coax_v_f32){hp, np}, (coax_v_f32){h_rope, 1024},
            (coax_v_u8){ids, 64}, (coax_v_u8){tg, 64}, (coax_v_f32){w_use, 64},
            (coax_v_u8){aux_tg, 256}, (coax_v_u8){aux_mask, 64}, (coax_v_s64){src_pos, 64},
            (coax_v_t_TdTinyWork){work, 1}, (coax_v_f32){hg_cpu, np}, 1);

        gpu_upload(st.p, 0, hp, (size_t)np * 4);
        gpu_zero(st.g, (size_t)np * 4);
        gpu_upload(st.ids, 0, ids, 64);
        gpu_upload(st.tg, 0, tg, 64);
        gpu_upload(st.weights, 0, w_use, 64 * 4);
        gpu_upload(st.aux_tg, 0, aux_tg, 256);
        gpu_upload(st.aux_mask, 0, aux_mask, 64);
        gpu_upload(st.src_pos, 0, src_pos, 64 * 8);

        double gs = gpu_fwd_bwd(k, &st, st.p, arm, beta, arm == COAX_TdArm_variant_b, scale_aux, arm == COAX_TdArm_variant_c, 2.0f, mar_w, 1);
        gpu_download(hg_gpu, st.g, 0, (size_t)np * 4);
        gpu_download(full_logits, st.logits, 0, (size_t)(nt * tiny.vocab) * 4);

        double max_gdiff = 0.0;
        for (int64_t i = 0; i < np; i++) {
            double d_val = fabs((double)hg_cpu[i] - (double)hg_gpu[i]);
            if (d_val > max_gdiff) max_gdiff = d_val;
        }
        /* Check GPU KV-cached decode matches GPU full forward across all 32 steps. */
        double max_dec_diff = 0.0;
        for (int64_t pos = 0; pos < tiny.train_ctx; pos++) {
            uint8_t cur[2] = {ids[pos], ids[tiny.train_ctx + pos]};
            gpu_decode_step(k, &st, st.p, arm, beta, 2, pos, cur, step_logits);
            for (int r = 0; r < 2; r++) {
                for (int v = 0; v < tiny.vocab; v++) {
                    double dv = fabs((double)step_logits[r * tiny.vocab + v] - (double)full_logits[(r * tiny.train_ctx + pos) * tiny.vocab + v]);
                    if (dv > max_dec_diff) max_dec_diff = dv;
                }
            }
        }
        printf("  %-30s | max |grad_gpu - grad_cpu| = %.3e | max |kv_dec - full| = %.3e | gpu %.2f ms\n",
               arm_names[ai], max_gdiff, max_dec_diff, gs * 1e3);
        if (max_gdiff > 1e-4 || max_dec_diff > 1e-4) {
            fprintf(stderr, "GPU vs CPU parity FAILED on %s!\n", arm_names[ai]);
            exit(1);
        }
    }
    free(hp); free(hg_cpu); free(hg_gpu); free(h_rope); free(work); free(full_logits); free(step_logits);
    td_gpu_free(&st);
}

/* ---- Training batch assembly from verified corpus ---- */

typedef struct {
    uint8_t *ids;
    uint8_t *tg;
    uint8_t *mask;
    uint8_t *aux_tg;
    uint8_t *aux_mask;
    int64_t *src_pos;
    int64_t n_traces;
    int64_t ctx;
} Corpus;

static void fill_sft_batch(
    const Corpus *c, int64_t step, int64_t rows,
    uint8_t *b_ids, uint8_t *b_tg, float *b_w, uint8_t *b_aux_tg, uint8_t *b_aux_mask, int64_t *b_src,
    int64_t *out_active_tok, int64_t *out_active_aux, int64_t *out_active_src
) {
    int64_t ctx = c->ctx, nt = rows * ctx;
    int64_t act_tok = 0, act_aux = 0, act_src = 0;
    for (int64_t r = 0; r < rows; r++) {
        int64_t idx = td_rng_index(7001ULL, (uint64_t)(step * rows + r), c->n_traces);
        const uint8_t *si = c->ids + idx * ctx;
        const uint8_t *st = c->tg + idx * ctx;
        const uint8_t *sm = c->mask + idx * ctx;
        const uint8_t *sa = c->aux_tg + idx * ctx * 4;
        const uint8_t *sam = c->aux_mask + idx * ctx;
        const int64_t *ss = c->src_pos + idx * ctx;
        memcpy(b_ids + r * ctx, si, (size_t)ctx);
        memcpy(b_tg + r * ctx, st, (size_t)ctx);
        memcpy(b_aux_tg + r * ctx * 4, sa, (size_t)ctx * 4);
        memcpy(b_aux_mask + r * ctx, sam, (size_t)ctx);
        memcpy(b_src + r * ctx, ss, (size_t)ctx * 8);
        for (int64_t t = 0; t < ctx; t++) {
            if (sm[t]) act_tok++;
            if (sam[t]) act_aux++;
            if (ss[t] >= 0) act_src++;
        }
    }
    float inv_tok = act_tok > 0 ? 1.0f / (float)act_tok : 0.0f;
    for (int64_t r = 0; r < rows; r++) {
        int64_t idx = td_rng_index(7001ULL, (uint64_t)(step * rows + r), c->n_traces);
        const uint8_t *sm = c->mask + idx * ctx;
        for (int64_t t = 0; t < ctx; t++) {
            b_w[r * ctx + t] = sm[t] ? inv_tok : 0.0f;
        }
    }
    (void)nt;
    *out_active_tok = act_tok;
    *out_active_aux = act_aux;
    *out_active_src = act_src;
}

static void compute_ckpt_digest(TdGpuState *st, coax_t_TdArm arm, int64_t step, int64_t active_p, uint8_t out_dig[32]) {
    float *buf = (float *)malloc((size_t)active_p * 4);
    uint8_t sp[32], sm[32], sv[32];
    gpu_download(buf, st->p, 0, (size_t)active_p * 4);
    td_sha256_f32((coax_v_f32){buf, active_p}, (coax_v_u8){sp, 32});
    gpu_download(buf, st->m, 0, (size_t)active_p * 4);
    td_sha256_f32((coax_v_f32){buf, active_p}, (coax_v_u8){sm, 32});
    gpu_download(buf, st->v, 0, (size_t)active_p * 4);
    td_sha256_f32((coax_v_f32){buf, active_p}, (coax_v_u8){sv, 32});
    free(buf);
    coax_a32_u8 ap, am, av;
    memcpy(ap.e, sp, 32); memcpy(am.e, sm, 32); memcpy(av.e, sv, 32);
    coax_a32_u8 res = tdp_ckpt_digest(arm, step, active_p, ap, am, av);
    memcpy(out_dig, res.e, 32);
}

/* ---- [2] Exact 10.72M-Parameter GPU Checkpoint Save/Resume Bit-Parity Check ---- */

static void verify_exact_resume_10m(const TdKernels *k, TdGpuState *st, const Corpus *corpus, const char *ckpt_dir) {
    coax_t_TdShape s = st->s;
    int64_t np = st->np, bp = td_backbone_params(s), nt = st->nt;
    float *hp = (float *)calloc((size_t)np, 4);
    uint8_t *b_ids = (uint8_t *)malloc((size_t)nt), *b_tg = (uint8_t *)malloc((size_t)nt);
    float *b_w = (float *)malloc((size_t)nt * 4);
    uint8_t *b_atg = (uint8_t *)malloc((size_t)nt * 4), *b_am = (uint8_t *)malloc((size_t)nt);
    int64_t *b_src = (int64_t *)malloc((size_t)nt * 8);

    /* Uninterrupted steps 0..4 */
    td_init_weights(s, COAX_TdArm_baseline_a, (coax_v_f32){hp, np});
    gpu_upload(st->p, 0, hp, (size_t)np * 4);
    gpu_zero(st->m, (size_t)np * 4);
    gpu_zero(st->v, (size_t)np * 4);
    for (int64_t step = 0; step < 4; step++) {
        int64_t atok = 0, aaux = 0, asrc = 0;
        fill_sft_batch(corpus, step, s.rows, b_ids, b_tg, b_w, b_atg, b_am, b_src, &atok, &aaux, &asrc);
        gpu_upload(st->ids, 0, b_ids, (size_t)nt);
        gpu_upload(st->tg, 0, b_tg, (size_t)nt);
        gpu_upload(st->weights, 0, b_w, (size_t)nt * 4);
        gpu_zero(st->g, (size_t)np * 4);
        gpu_fwd_bwd(k, st, st->p, COAX_TdArm_baseline_a, 1.0f, 0, 0.0f, 0, 0.0f, 0.0f, 1);
        uint32_t sc[2]; gpu_download(sc, st->scal, 0, 8);
        coax_t_TdDecision dec = tdp_decide(step, 100, 10, 0.0006, 0.00006, 1.0f, 0.9, 0.99, 1e-8, 0.01, atok, sc[0], sc[1]);
        if (dec.status != COAX_TdStatus_ok || !dec.apply) { fprintf(stderr, "resume test step failed\n"); exit(1); }
        gpu_adamw_step(k, st, dec, bp);
    }
    uint8_t dig_unint[32];
    compute_ckpt_digest(st, COAX_TdArm_baseline_a, 4, bp, dig_unint);
    float *p_unint = (float *)malloc((size_t)bp * 4);
    gpu_download(p_unint, st->p, 0, (size_t)bp * 4);

    /* Split run: 0..2 -> save checkpoint to disk -> wipe GPU -> load & verify -> 2..4 */
    td_init_weights(s, COAX_TdArm_baseline_a, (coax_v_f32){hp, np});
    gpu_upload(st->p, 0, hp, (size_t)np * 4);
    gpu_zero(st->m, (size_t)np * 4);
    gpu_zero(st->v, (size_t)np * 4);
    for (int64_t step = 0; step < 2; step++) {
        int64_t atok = 0, aaux = 0, asrc = 0;
        fill_sft_batch(corpus, step, s.rows, b_ids, b_tg, b_w, b_atg, b_am, b_src, &atok, &aaux, &asrc);
        gpu_upload(st->ids, 0, b_ids, (size_t)nt);
        gpu_upload(st->tg, 0, b_tg, (size_t)nt);
        gpu_upload(st->weights, 0, b_w, (size_t)nt * 4);
        gpu_zero(st->g, (size_t)np * 4);
        gpu_fwd_bwd(k, st, st->p, COAX_TdArm_baseline_a, 1.0f, 0, 0.0f, 0, 0.0f, 0.0f, 1);
        uint32_t sc[2]; gpu_download(sc, st->scal, 0, 8);
        coax_t_TdDecision dec = tdp_decide(step, 100, 10, 0.0006, 0.00006, 1.0f, 0.9, 0.99, 1e-8, 0.01, atok, sc[0], sc[1]);
        gpu_adamw_step(k, st, dec, bp);
    }
    uint8_t dig_mid[32];
    compute_ckpt_digest(st, COAX_TdArm_baseline_a, 2, bp, dig_mid);

    char cpath[512];
    snprintf(cpath, sizeof cpath, "%s/resume-step2.ckpt", ckpt_dir);
    float *ck = (float *)malloc((size_t)bp * 12);
    gpu_download(ck, st->p, 0, (size_t)bp * 4);
    gpu_download(ck + bp, st->m, 0, (size_t)bp * 4);
    gpu_download(ck + 2 * bp, st->v, 0, (size_t)bp * 4);
    FILE *cf = fopen(cpath, "wb");
    if (!cf || fwrite(dig_mid, 1, 32, cf) != 32 || fwrite(ck, 4, (size_t)bp * 3, cf) != (size_t)bp * 3 || fclose(cf) != 0) {
        fprintf(stderr, "failed writing resume checkpoint %s\n", cpath); exit(1);
    }
    /* Wipe GPU buffers completely to prove resume loads strictly from disk. */
    gpu_zero(st->p, (size_t)np * 4);
    gpu_zero(st->m, (size_t)np * 4);
    gpu_zero(st->v, (size_t)np * 4);
    memset(ck, 0, (size_t)bp * 12);

    uint8_t disk_dig[32];
    cf = fopen(cpath, "rb");
    if (!cf || fread(disk_dig, 1, 32, cf) != 32 || fread(ck, 4, (size_t)bp * 3, cf) != (size_t)bp * 3 || fclose(cf) != 0) {
        fprintf(stderr, "failed reading resume checkpoint\n"); exit(1);
    }
    gpu_upload(st->p, 0, ck, (size_t)bp * 4);
    gpu_upload(st->m, 0, ck + bp, (size_t)bp * 4);
    gpu_upload(st->v, 0, ck + 2 * bp, (size_t)bp * 4);
    uint8_t dig_reload[32];
    compute_ckpt_digest(st, COAX_TdArm_baseline_a, 2, bp, dig_reload);
    if (memcmp(disk_dig, dig_reload, 32) != 0) { fprintf(stderr, "checkpoint reload SHA-256 mismatch!\n"); exit(1); }

    for (int64_t step = 2; step < 4; step++) {
        int64_t atok = 0, aaux = 0, asrc = 0;
        fill_sft_batch(corpus, step, s.rows, b_ids, b_tg, b_w, b_atg, b_am, b_src, &atok, &aaux, &asrc);
        gpu_upload(st->ids, 0, b_ids, (size_t)nt);
        gpu_upload(st->tg, 0, b_tg, (size_t)nt);
        gpu_upload(st->weights, 0, b_w, (size_t)nt * 4);
        gpu_zero(st->g, (size_t)np * 4);
        gpu_fwd_bwd(k, st, st->p, COAX_TdArm_baseline_a, 1.0f, 0, 0.0f, 0, 0.0f, 0.0f, 1);
        uint32_t sc[2]; gpu_download(sc, st->scal, 0, 8);
        coax_t_TdDecision dec = tdp_decide(step, 100, 10, 0.0006, 0.00006, 1.0f, 0.9, 0.99, 1e-8, 0.01, atok, sc[0], sc[1]);
        gpu_adamw_step(k, st, dec, bp);
    }
    uint8_t dig_resumed[32];
    compute_ckpt_digest(st, COAX_TdArm_baseline_a, 4, bp, dig_resumed);
    float *p_resumed = (float *)malloc((size_t)bp * 4);
    gpu_download(p_resumed, st->p, 0, (size_t)bp * 4);

    int64_t bit_diffs = 0;
    for (int64_t i = 0; i < bp; i++) {
        if (memcmp(&p_unint[i], &p_resumed[i], 4) != 0) bit_diffs++;
    }
    char h1[65], h2[65];
    fmt_hex(dig_unint, h1); fmt_hex(dig_resumed, h2);
    printf("\n[2] Live 10.72M-Parameter GPU Exact Save/Resume Verification:\n");
    printf("  uninterrupted step 4 sha256: %s\n", h1);
    printf("  resumed       step 4 sha256: %s\n", h2);
    printf("  parameter bit differences across %lld params: %lld\n", (long long)bp, (long long)bit_diffs);
    if (bit_diffs != 0 || memcmp(dig_unint, dig_resumed, 32) != 0) {
        fprintf(stderr, "EXACT RESUME FAILED!\n"); exit(1);
    }
    free(hp); free(b_ids); free(b_tg); free(b_w); free(b_atg); free(b_am); free(b_src);
    free(p_unint); free(p_resumed); free(ck);
}

/* ---- Diagnostic-Only Teacher-Forced Accuracy (NEVER counted as pass) ---- */

static double diag_teacher_forced_acc(const TdKernels *k, TdGpuState *st, const Corpus *corpus, coax_t_TdArm arm, float beta_para) {
    int64_t nt = st->nt, ctx = st->s.train_ctx, vocab = st->s.vocab;
    uint8_t *b_ids = (uint8_t *)malloc((size_t)nt), *b_tg = (uint8_t *)malloc((size_t)nt);
    float *b_w = (float *)malloc((size_t)nt * 4);
    uint8_t *b_atg = (uint8_t *)malloc((size_t)nt * 4), *b_am = (uint8_t *)malloc((size_t)nt);
    int64_t *b_src = (int64_t *)malloc((size_t)nt * 8);
    int64_t atok = 0, aaux = 0, asrc = 0;
    fill_sft_batch(corpus, 0, st->s.rows, b_ids, b_tg, b_w, b_atg, b_am, b_src, &atok, &aaux, &asrc);
    gpu_upload(st->ids, 0, b_ids, (size_t)nt);
    gpu_upload(st->tg, 0, b_tg, (size_t)nt);
    gpu_upload(st->weights, 0, b_w, (size_t)nt * 4);
    gpu_fwd_bwd(k, st, st->p, arm, beta_para, 0, 0.0f, 0, 0.0f, 0.0f, 0);
    float *hlog = (float *)malloc((size_t)(nt * vocab) * 4);
    gpu_download(hlog, st->logits, 0, (size_t)(nt * vocab) * 4);
    int64_t right = 0, tot = 0;
    for (int64_t i = 0; i < nt; i++) {
        if (b_w[i] > 0.0f) {
            int pred = (int)td_greedy_token((coax_v_f32){hlog + i * vocab, vocab});
            if (pred == (int)b_tg[i]) right++;
            tot++;
        }
    }
    (void)ctx;
    free(b_ids); free(b_tg); free(b_w); free(b_atg); free(b_am); free(b_src); free(hlog);
    return tot > 0 ? (double)right / (double)tot : 0.0;
}

/* ---- Standalone Autoregressive Rollout + Ch 234 Replay Evaluation ---- */

typedef struct {
    int n_total, n_exact, n_out_match, n_syntax_err;
    int div_steps[4];
} CellStats;

typedef struct {
    CellStats cell[6][4]; /* [family][bucket] */
    double seen_pass[4], unseen_pass[4], all_pass[4];
    double seen_out[4], unseen_out[4], all_out[4];
    int train_diag_exact, train_diag_total;
    double eval_wall_s, eval_gpu_s;
    int64_t total_gen_tokens;
    coax_t_TdHypVerdict verdict;
} ArmEval;

static int cmp_int(const void *a, const void *b) {
    int ia = *(const int *)a, ib = *(const int *)b;
    return (ia > ib) - (ia < ib);
}

static ArmEval eval_arm_autoregressive(const TdKernels *k, TdGpuState *st, coax_t_TdArm arm, float beta_para) {
    ArmEval ev;
    memset(&ev, 0, sizeof ev);
    int64_t vocab = st->s.vocab;
    float *step_logits = (float *)malloc((size_t)(16 * vocab) * 4);

    /* Evaluate all 96 held-out items grouped by length bucket (4 batches of 24? Use batch=16:
       6 batches of 16, grouping same-length buckets together for tight GPU utilization!). */
    /* Build 6 batches of 16: each batch has 4 families x 4 items, or 1 bucket (24 items = 16 + 8). */
    int order[96];
    int oi = 0;
    for (int bi = 0; bi < 4; bi++) {
        for (int fi = 0; fi < 6; fi++) {
            for (int pi = 0; pi < 4; pi++) {
                order[oi++] = fi * 16 + bi * 4 + pi;
            }
        }
    }

    double t0 = now_s();
    for (int base = 0; base < 96; base += 16) {
        int b_n = (96 - base < 16) ? (96 - base) : 16;
        uint8_t prompts[16][128];
        coax_t_TmOp progs[16][32];
        coax_t_RxClaim claims[16];
        int64_t meta[16][5]; /* k, plen, ref_steps, fi, bi */
        int64_t max_len = 0;

        for (int r = 0; r < b_n; r++) {
            claims[r] = td_eval_item_prompt(order[base + r],
                (coax_v_u8){prompts[r], 128}, (coax_v_t_TmOp){progs[r], 32}, (coax_v_s64){meta[r], 5});
            int64_t need = meta[r][1] + meta[r][2] * 6 + 16;
            if (need > st->max_ctx) need = st->max_ctx;
            if (need > max_len) max_len = need;
        }

        uint8_t *seqs = (uint8_t *)calloc((size_t)b_n * (size_t)max_len, 1);
        int done[16] = {0};
        int gen_len[16] = {0};
        for (int r = 0; r < b_n; r++) {
            memcpy(seqs + (size_t)r * (size_t)max_len, prompts[r], (size_t)meta[r][1]);
        }

        for (int64_t pos = 0; pos < max_len - 1; pos++) {
            int any_active = 0;
            uint8_t cur_ids[16] = {0};
            for (int r = 0; r < b_n; r++) {
                cur_ids[r] = seqs[(size_t)r * (size_t)max_len + (size_t)pos];
                if (!done[r]) any_active = 1;
            }
            if (!any_active) break;
            ev.eval_gpu_s += gpu_decode_step(k, st, st->p, arm, beta_para, b_n, pos, cur_ids, step_logits);
            for (int r = 0; r < b_n; r++) {
                if (done[r]) continue;
                if (pos + 1 < meta[r][1]) continue; /* still feeding program prompt */
                uint8_t next_tok = (uint8_t)td_greedy_token((coax_v_f32){step_logits + r * vocab, vocab});
                seqs[(size_t)r * (size_t)max_len + (size_t)(pos + 1)] = next_tok;
                gen_len[r]++;
                ev.total_gen_tokens++;
                if (next_tok == 5 /* TDP_EOS */ || gen_len[r] >= meta[r][2] * 6 + 12) {
                    done[r] = 1;
                }
            }
        }

        /* Score EVERY generated trace strictly through Ch 234 (tdp_score_rollout -> rx_check). */
        for (int r = 0; r < b_n; r++) {
            int fi = (int)meta[r][3], bi = (int)meta[r][4];
            int64_t plen = meta[r][1];
            uint8_t *gen_ptr = seqs + (size_t)r * (size_t)max_len + (size_t)plen;
            coax_t_TdScore sc = tdp_score_rollout(
                (coax_v_t_TmOp){progs[r], meta[r][0]},
                (coax_v_u8){gen_ptr, gen_len[r]},
                claims[r], meta[r][2], 512);
            CellStats *cs = &ev.cell[fi][bi];
            int slot = cs->n_total;
            if (slot < 4) cs->div_steps[slot] = sc.exact ? (int)meta[r][2] : (int)sc.first_div_step;
            cs->n_total++;
            if (sc.exact) cs->n_exact++;
            if (sc.output_match) cs->n_out_match++;
            if (sc.syntax_err) cs->n_syntax_err++;
        }
        free(seqs);
    }
    /* Also measure standalone autoregressive Ch 234 exact replay on 12 training-split prompts
       (4 per training family) strictly as a convergence sanity check — NEVER included in held-out scores! */
    {
        int train_sample[12] = {0, 29, 58, 70, 84, 113, 142, 154, 168, 197, 226, 238};
        uint8_t prompts[12][128];
        coax_t_TmOp progs[12][32];
        coax_t_RxClaim claims[12];
        int64_t meta[12][4];
        int64_t max_len = 0;
        for (int r = 0; r < 12; r++) {
            claims[r] = td_train_item_prompt(train_sample[r],
                (coax_v_u8){prompts[r], 128}, (coax_v_t_TmOp){progs[r], 32}, (coax_v_s64){meta[r], 4});
            int64_t need = meta[r][1] + meta[r][2] * 6 + 16;
            if (need > st->max_ctx) need = st->max_ctx;
            if (need > max_len) max_len = need;
        }
        uint8_t *seqs = (uint8_t *)calloc((size_t)12 * (size_t)max_len, 1);
        int done[12] = {0}, gen_len[12] = {0};
        for (int r = 0; r < 12; r++) memcpy(seqs + (size_t)r * (size_t)max_len, prompts[r], (size_t)meta[r][1]);
        for (int64_t pos = 0; pos < max_len - 1; pos++) {
            int any = 0;
            uint8_t cur_ids[16] = {0};
            for (int r = 0; r < 12; r++) { cur_ids[r] = seqs[(size_t)r * (size_t)max_len + (size_t)pos]; if (!done[r]) any = 1; }
            if (!any) break;
            gpu_decode_step(k, st, st->p, arm, beta_para, 12, pos, cur_ids, step_logits);
            for (int r = 0; r < 12; r++) {
                if (done[r] || pos + 1 < meta[r][1]) continue;
                uint8_t nxt = (uint8_t)td_greedy_token((coax_v_f32){step_logits + r * vocab, vocab});
                seqs[(size_t)r * (size_t)max_len + (size_t)(pos + 1)] = nxt;
                gen_len[r]++;
                if (nxt == 5 || gen_len[r] >= meta[r][2] * 6 + 12) done[r] = 1;
            }
        }
        ev.train_diag_total = 12;
        ev.train_diag_exact = 0;
        for (int r = 0; r < 12; r++) {
            coax_t_TdScore sc = tdp_score_rollout((coax_v_t_TmOp){progs[r], meta[r][0]},
                (coax_v_u8){seqs + (size_t)r * (size_t)max_len + (size_t)meta[r][1], gen_len[r]}, claims[r], meta[r][2], 512);
            if (sc.exact) ev.train_diag_exact++;
        }
        free(seqs);
    }

    ev.eval_wall_s = now_s() - t0;
    free(step_logits);

    for (int bi = 0; bi < 4; bi++) {
        int s_ex = 0, s_out = 0, s_tot = 0, u_ex = 0, u_out = 0, u_tot = 0;
        for (int fi = 0; fi < 6; fi++) {
            if (fi < 3) { s_ex += ev.cell[fi][bi].n_exact; s_out += ev.cell[fi][bi].n_out_match; s_tot += ev.cell[fi][bi].n_total; }
            else { u_ex += ev.cell[fi][bi].n_exact; u_out += ev.cell[fi][bi].n_out_match; u_tot += ev.cell[fi][bi].n_total; }
        }
        ev.seen_pass[bi] = (double)s_ex / (double)s_tot;
        ev.unseen_pass[bi] = (double)u_ex / (double)u_tot;
        ev.all_pass[bi] = (double)(s_ex + u_ex) / (double)(s_tot + u_tot);
        ev.seen_out[bi] = (double)s_out / (double)s_tot;
        ev.unseen_out[bi] = (double)u_out / (double)u_tot;
        ev.all_out[bi] = (double)(s_out + u_out) / (double)(s_tot + u_tot);
    }
    ev.verdict = tdp_eval_verdict(ev.all_pass[1], ev.all_pass[2], ev.all_pass[3], ev.unseen_pass[0]);
    return ev;
}

/* ---- Train One Arm (Baseline A / Variant B / Variant C) on Live GPU ---- */

typedef struct {
    coax_t_TdArm arm;
    int64_t total_steps, sft_steps, rl_steps, total_tokens, active_params;
    double wall_s, gpu_s, tok_per_s;
    double loss_start, loss_mid, loss_end, aux_loss_end, margin_loss_end;
    double diag_tf_acc;
    double rl_mean_reward, rl_mean_kl;
    int64_t rl_exact_count, rl_total_rollouts, rl_mixed_groups;
    uint8_t ckpt_digest[32];
    ArmEval eval;
} ArmResult;

static ArmResult train_and_eval_arm(
    const TdKernels *k, TdGpuState *st, const Corpus *corpus, coax_t_TdArm arm, int64_t total_steps
) {
    ArmResult res;
    memset(&res, 0, sizeof res);
    res.arm = arm;
    res.total_steps = total_steps;

    coax_t_TdShape s = st->s;
    int64_t np = st->np, bp = td_backbone_params(s), nt = st->nt, ctx = s.train_ctx, vocab = s.vocab;
    int64_t active_p = (arm == COAX_TdArm_variant_b) ? np : bp;
    res.active_params = active_p;
    int64_t warmup = total_steps >= 20 ? total_steps / 10 : 2;
    int64_t sft_steps = (arm == COAX_TdArm_variant_b) ? (total_steps * 3) / 4 : total_steps;
    int64_t rl_steps = total_steps - sft_steps;
    res.sft_steps = sft_steps;
    res.rl_steps = rl_steps;

    float *hp = (float *)calloc((size_t)np, 4);
    if (td_init_weights(s, arm, (coax_v_f32){hp, np}) != COAX_TdStatus_ok) { fprintf(stderr, "init failed\n"); exit(1); }
    gpu_upload(st->p, 0, hp, (size_t)np * 4);
    gpu_zero(st->m, (size_t)np * 4);
    gpu_zero(st->v, (size_t)np * 4);
    free(hp);

    uint8_t *b_ids = (uint8_t *)malloc((size_t)nt), *b_tg = (uint8_t *)malloc((size_t)nt);
    float *b_w = (float *)malloc((size_t)nt * 4);
    uint8_t *b_atg = (uint8_t *)malloc((size_t)nt * 4), *b_am = (uint8_t *)malloc((size_t)nt);
    int64_t *b_src = (int64_t *)malloc((size_t)nt * 8);

    const char *arm_label = (arm == COAX_TdArm_baseline_a) ? "Baseline A (H1: Standard SFT)" :
                            (arm == COAX_TdArm_variant_b)  ? "Variant B (H5: Aux State + Ch 139 RL)" :
                                                             "Variant C (H3: Parabolic + Max-Margin + Beta)";
    printf("\n=== Training %s | params=%lld (backbone=%lld) | steps=%lld (sft=%lld, rl=%lld) | tokens=%lld ===\n",
           arm_label, (long long)active_p, (long long)bp, (long long)total_steps,
           (long long)sft_steps, (long long)rl_steps, (long long)(total_steps * nt));

    double t0 = now_s();
    double gpu_total = 0.0;

    /* Phase 1: SFT (A), SFT + Aux State Supervision (B), or SFT + Parabolic Max-Margin + Beta Annealing (C) */
    for (int64_t step = 0; step < sft_steps; step++) {
        int64_t atok = 0, aaux = 0, asrc = 0;
        fill_sft_batch(corpus, step, s.rows, b_ids, b_tg, b_w, b_atg, b_am, b_src, &atok, &aaux, &asrc);
        gpu_upload(st->ids, 0, b_ids, (size_t)nt);
        gpu_upload(st->tg, 0, b_tg, (size_t)nt);
        gpu_upload(st->weights, 0, b_w, (size_t)nt * 4);
        if (arm == COAX_TdArm_variant_b) {
            gpu_upload(st->aux_tg, 0, b_atg, (size_t)nt * 4);
            gpu_upload(st->aux_mask, 0, b_am, (size_t)nt);
        }
        if (arm == COAX_TdArm_variant_c) {
            gpu_upload(st->src_pos, 0, b_src, (size_t)nt * 8);
        }
        gpu_zero(st->g, (size_t)np * 4);
        gpu_zero(st->scal, 32);

        float beta = (float)tdp_beta(arm, step, total_steps, 1.0, 10.0);
        float scale_aux = (arm == COAX_TdArm_variant_b && aaux > 0) ? (0.25f / (float)(4 * aaux)) : 0.0f;
        float mar_w = (arm == COAX_TdArm_variant_c && asrc > 0) ? (0.25f / (float)asrc) : 0.0f;

        gpu_total += gpu_fwd_bwd(k, st, st->p, arm, beta, arm == COAX_TdArm_variant_b, scale_aux,
                                 arm == COAX_TdArm_variant_c, 2.0f, mar_w, 1);

        uint32_t sc_bits[4] = {0};
        gpu_download(sc_bits, st->scal, 0, 16);
        coax_t_TdDecision dec = tdp_decide(step, total_steps, warmup, 0.0006, 0.00006, 1.0f,
                                           0.9, 0.99, 1e-8, 0.01, atok, sc_bits[0], sc_bits[1]);
        if (dec.status != COAX_TdStatus_ok || !dec.apply) {
            fprintf(stderr, "step %lld refused (status=%d)\n", (long long)step, (int)dec.status);
            exit(1);
        }
        gpu_total += gpu_adamw_step(k, st, dec, active_p);
        res.total_tokens += nt;

        float aux_sum = 0.0f, mar_sum = 0.0f;
        if (arm == COAX_TdArm_variant_b) memcpy(&aux_sum, &sc_bits[2], 4);
        if (arm == COAX_TdArm_variant_c) memcpy(&mar_sum, &sc_bits[3], 4);
        double aux_l = (arm == COAX_TdArm_variant_b && aaux > 0) ? (double)aux_sum / (double)aaux : 0.0;
        double mar_l = (arm == COAX_TdArm_variant_c) ? (double)mar_sum : 0.0;

        if (step == 0) res.loss_start = dec.loss;
        if (step == total_steps / 2) res.loss_mid = dec.loss;
        res.loss_end = dec.loss;
        res.aux_loss_end = aux_l;
        res.margin_loss_end = mar_l;

        if (step == 0 || (step + 1) % (total_steps >= 8 ? total_steps / 4 : 1) == 0 || step + 1 == sft_steps) {
            printf("  step %4lld/%lld | ce=%.4f | norm=%.3f | lr=%.5f | beta=%.2f | aux=%.4f | margin=%.4f\n",
                   (long long)(step + 1), (long long)total_steps, dec.loss, (double)dec.norm, dec.rate,
                   (double)beta, aux_l, mar_l);
        }
    }

    /* Phase 2 (Variant B ONLY): Ch 139 On-Policy Group-Relative RL with Binary Ch 234 Exact-Replay Reward */
    if (arm == COAX_TdArm_variant_b && rl_steps > 0) {
        gpu_copy_dev(st->p_ref, st->p, (size_t)np * 4);
        float *step_logits = (float *)malloc((size_t)(16 * vocab) * 4);
        float *loss_pi = (float *)malloc((size_t)nt * 4);
        float *loss_ref = (float *)malloc((size_t)nt * 4);
        double kl_acc = 0.0;

        for (int64_t rstep = 0; rstep < rl_steps; rstep++) {
            int64_t step = sft_steps + rstep;
            /* Pick 4 training prompts strictly from train split [0..251], G = 4 rollouts each = 16 rows. */
            uint8_t prompts[4][128];
            coax_t_TmOp progs[4][32];
            coax_t_RxClaim claims[4];
            int64_t meta[4][4];
            for (int g = 0; g < 4; g++) {
                int64_t tidx = td_rng_index(8001ULL, (uint64_t)(rstep * 4 + g), corpus->n_traces);
                claims[g] = td_train_item_prompt(tidx, (coax_v_u8){prompts[g], 128},
                                                 (coax_v_t_TmOp){progs[g], 32}, (coax_v_s64){meta[g], 4});
            }

            memset(b_ids, 0, (size_t)nt);
            memset(b_tg, 0, (size_t)nt);
            int done[16] = {0};
            int gen_len[16] = {0};
            for (int r = 0; r < 16; r++) {
                int g = r / 4;
                memcpy(b_ids + r * ctx, prompts[g], (size_t)meta[g][1]);
            }
            /* Live Metal GPU KV-cached on-policy temperature sampling (temp = 0.7). */
            for (int64_t pos = 0; pos < ctx - 1; pos++) {
                int any = 0;
                uint8_t cur_ids[16];
                for (int r = 0; r < 16; r++) {
                    cur_ids[r] = b_ids[r * ctx + pos];
                    if (!done[r]) any = 1;
                }
                if (!any) break;
                gpu_total += gpu_decode_step(k, st, st->p, arm, 1.0f, 16, pos, cur_ids, step_logits);
                for (int r = 0; r < 16; r++) {
                    if (done[r]) continue;
                    int g = r / 4;
                    if (pos + 1 < meta[g][1]) continue;
                    uint64_t seed = 90000ULL + (uint64_t)rstep * 32ULL + (uint64_t)r;
                    uint8_t nxt = (uint8_t)td_sample_token((coax_v_f32){step_logits + r * vocab, vocab}, 0.7, seed, (uint64_t)pos);
                    b_ids[r * ctx + pos + 1] = nxt;
                    gen_len[r]++;
                    if (nxt == 5 /* TDP_EOS */) done[r] = 1;
                }
            }

            /* Score every sampled rollout strictly with Ch 234 exact replay! */
            double rewards[16] = {0}, logp[16] = {0}, logr[16] = {0}, adv[16] = {0};
            int64_t total_samp = 0;
            for (int r = 0; r < 16; r++) {
                int g = r / 4;
                int64_t plen = meta[g][1];
                coax_t_TdScore sc = tdp_score_rollout(
                    (coax_v_t_TmOp){progs[g], meta[g][0]},
                    (coax_v_u8){b_ids + r * ctx + plen, gen_len[r]},
                    claims[g], meta[g][2], 512);
                rewards[r] = sc.exact ? 1.0 : 0.0;
                if (sc.exact) res.rl_exact_count++;
                res.rl_total_rollouts++;
                for (int64_t t = 0; t < ctx - 1; t++) {
                    b_tg[r * ctx + t] = b_ids[r * ctx + t + 1];
                    int active = (t >= plen - 1 && t < plen - 1 + gen_len[r]);
                    b_w[r * ctx + t] = active ? 1.0f : 0.0f;
                    if (active) total_samp++;
                }
                b_w[r * ctx + ctx - 1] = 0.0f;
            }
            for (int g = 0; g < 4; g++) {
                double s_r = rewards[4 * g] + rewards[4 * g + 1] + rewards[4 * g + 2] + rewards[4 * g + 3];
                if (s_r > 0.0 && s_r < 4.0) res.rl_mixed_groups++;
            }

            /* Forward pass under current policy and frozen SFT reference to compute exact log-prob ratio KL. */
            gpu_upload(st->ids, 0, b_ids, (size_t)nt);
            gpu_upload(st->tg, 0, b_tg, (size_t)nt);
            gpu_upload(st->weights, 0, b_w, (size_t)nt * 4);
            gpu_total += gpu_fwd_bwd(k, st, st->p, arm, 1.0f, 0, 0.0f, 0, 0.0f, 0.0f, 0);
            gpu_download(loss_pi, st->loss, 0, (size_t)nt * 4);
            gpu_total += gpu_fwd_bwd(k, st, st->p_ref, arm, 1.0f, 0, 0.0f, 0, 0.0f, 0.0f, 0);
            gpu_download(loss_ref, st->loss, 0, (size_t)nt * 4);

            for (int r = 0; r < 16; r++) {
                double lp = 0.0, lr = 0.0;
                for (int64_t t = 0; t < ctx; t++) {
                    if (b_w[r * ctx + t] > 0.0f) {
                        lp -= (double)loss_pi[r * ctx + t];
                        lr -= (double)loss_ref[r * ctx + t];
                    }
                }
                logp[r] = lp;
                logr[r] = lr;
                kl_acc += (lp - lr);
            }

            /* Ch 139 group-relative advantages with KL beta = 0.02 */
            if (tdp_rl_advantages(4, 0.02, (coax_v_f64){rewards, 16}, (coax_v_f64){logp, 16}, (coax_v_f64){logr, 16}, (coax_v_f64){adv, 16}) != COAX_TdStatus_ok) {
                fprintf(stderr, "tdp_rl_advantages failed\n"); exit(1);
            }
            float inv_s = total_samp > 0 ? 1.0f / (float)total_samp : 0.0f;
            for (int r = 0; r < 16; r++) {
                for (int64_t t = 0; t < ctx; t++) {
                    if (b_w[r * ctx + t] > 0.0f) b_w[r * ctx + t] = (float)adv[r] * inv_s;
                }
            }

            /* Policy-gradient forward+backward & AdamW update */
            gpu_upload(st->weights, 0, b_w, (size_t)nt * 4);
            gpu_zero(st->g, (size_t)np * 4);
            gpu_zero(st->scal, 32);
            gpu_total += gpu_fwd_bwd(k, st, st->p, arm, 1.0f, 0, 0.0f, 0, 0.0f, 0.0f, 1);
            uint32_t sc_bits[2] = {0};
            gpu_download(sc_bits, st->scal, 0, 8);
            coax_t_TdDecision dec = tdp_decide(step, total_steps, warmup, 0.0002, 0.00002, 1.0f,
                                               0.9, 0.99, 1e-8, 0.01, total_samp > 0 ? total_samp : 1, sc_bits[0], sc_bits[1]);
            if (dec.status == COAX_TdStatus_ok && dec.apply) {
                gpu_total += gpu_adamw_step(k, st, dec, bp);
            }
            res.total_tokens += nt;

            if (rstep == 0 || rstep + 1 == rl_steps || (rstep + 1) % (rl_steps >= 4 ? rl_steps / 2 : 1) == 0) {
                double rew_step = 0.0;
                for (int r = 0; r < 16; r++) rew_step += rewards[r];
                printf("  RL step %4lld/%lld | ch234_reward=%.3f (%d/16) | grad_norm=%.4f | mean_kl=%.4f\n",
                       (long long)(step + 1), (long long)total_steps, rew_step / 16.0, (int)rew_step,
                       (double)dec.norm, kl_acc / (double)res.rl_total_rollouts);
            }
        }
        res.rl_mean_reward = res.rl_total_rollouts > 0 ? (double)res.rl_exact_count / (double)res.rl_total_rollouts : 0.0;
        res.rl_mean_kl = res.rl_total_rollouts > 0 ? kl_acc / (double)res.rl_total_rollouts : 0.0;
        free(step_logits); free(loss_pi); free(loss_ref);
    }

    res.wall_s = now_s() - t0;
    res.gpu_s = gpu_total;
    res.tok_per_s = (res.wall_s > 0.0) ? (double)res.total_tokens / res.wall_s : 0.0;
    float final_beta = (float)tdp_beta(arm, total_steps, total_steps, 1.0, 10.0);
    res.diag_tf_acc = diag_teacher_forced_acc(k, st, corpus, arm, final_beta);
    compute_ckpt_digest(st, arm, total_steps, bp, res.ckpt_digest);

    char hex[65];
    fmt_hex(res.ckpt_digest, hex);
    printf("  -> Trained %lld tokens in %.2f s (GPU %.2f s, %.0f tok/s)\n",
           (long long)res.total_tokens, res.wall_s, res.gpu_s, res.tok_per_s);
    printf("  -> Final ckpt sha256: %s\n", hex);
    printf("  -> [DIAGNOSTIC ONLY] train TF-acc=%.1f%% (NOT counted as pass)\n", res.diag_tf_acc * 100.0);

    /* Standalone Autoregressive Rollout + Ch 234 Exact Replay Evaluation across 1x/2x/4x/8x */
    res.eval = eval_arm_autoregressive(k, st, arm, final_beta);
    printf("  -> [DIAGNOSTIC ONLY] train-split 1x Ch 234 autoregressive rollout: %d/%d (%.1f%%)\n",
           res.eval.train_diag_exact, res.eval.train_diag_total,
           100.0 * (double)res.eval.train_diag_exact / (double)res.eval.train_diag_total);
    printf("  -> Ch 234 Held-Out Autoregressive Eval (%lld gen tokens in %.2f s, GPU %.2f s): "
           "seen 1x=%.2f 2x=%.2f 4x=%.2f 8x=%.2f | unseen 1x=%.2f 2x=%.2f 4x=%.2f 8x=%.2f\n",
           (long long)res.eval.total_gen_tokens, res.eval.eval_wall_s, res.eval.eval_gpu_s,
           res.eval.seen_pass[0], res.eval.seen_pass[1], res.eval.seen_pass[2], res.eval.seen_pass[3],
           res.eval.unseen_pass[0], res.eval.unseen_pass[1], res.eval.unseen_pass[2], res.eval.unseen_pass[3]);

    free(b_ids); free(b_tg); free(b_w); free(b_atg); free(b_am); free(b_src);
    return res;
}

static const char *verdict_str(coax_t_TdHypVerdict v) {
    if (v == COAX_TdHypVerdict_confirmed) return "CONFIRMED";
    if (v == COAX_TdHypVerdict_partial) return "PARTIAL (2x only; fails 4x/8x/unseen)";
    return "FALSIFIED";
}

static void print_detailed_table(const char *title, const ArmResult *r) {
    static const char *fnames[6] = {"countdown   (seen)  ", "prefix_sum  (seen)  ", "mem_accum   (seen)  ",
                                    "stride_sub  (UNSEEN)", "squares_mem (UNSEEN)", "tri_accum   (UNSEEN)"};
    static const char *bnames[4] = {"1x", "2x", "4x", "8x"};
    printf("\n--- %s ---\n", title);
    printf("  Family               | Len | Pass@1 (Ch234) | OutDigest | SynErr | FirstDivStep (min/med/max)\n");
    for (int fi = 0; fi < 6; fi++) {
        for (int bi = 0; bi < 4; bi++) {
            const CellStats *c = &r->eval.cell[fi][bi];
            int d[4];
            for (int i = 0; i < c->n_total && i < 4; i++) d[i] = c->div_steps[i];
            qsort(d, (size_t)c->n_total, sizeof(int), cmp_int);
            int dmin = d[0], dmed = d[c->n_total / 2], dmax = d[c->n_total - 1];
            printf("  %s | %s  | %d/%d (%5.1f%%)    | %d/%d       | %d/%d    | %3d / %3d / %3d\n",
                   fnames[fi], bnames[bi],
                   c->n_exact, c->n_total, 100.0 * (double)c->n_exact / (double)c->n_total,
                   c->n_out_match, c->n_total, c->n_syntax_err, c->n_total, dmin, dmed, dmax);
        }
    }
    printf("  SUMMARY Seen   pass@1: 1x=%.1f%%  2x=%.1f%%  4x=%.1f%%  8x=%.1f%% | OutDigest: 1x=%.1f%% 2x=%.1f%% 4x=%.1f%% 8x=%.1f%%\n",
           100.0 * r->eval.seen_pass[0], 100.0 * r->eval.seen_pass[1], 100.0 * r->eval.seen_pass[2], 100.0 * r->eval.seen_pass[3],
           100.0 * r->eval.seen_out[0], 100.0 * r->eval.seen_out[1], 100.0 * r->eval.seen_out[2], 100.0 * r->eval.seen_out[3]);
    printf("  SUMMARY Unseen pass@1: 1x=%.1f%%  2x=%.1f%%  4x=%.1f%%  8x=%.1f%% | OutDigest: 1x=%.1f%% 2x=%.1f%% 4x=%.1f%% 8x=%.1f%%\n",
           100.0 * r->eval.unseen_pass[0], 100.0 * r->eval.unseen_pass[1], 100.0 * r->eval.unseen_pass[2], 100.0 * r->eval.unseen_pass[3],
           100.0 * r->eval.unseen_out[0], 100.0 * r->eval.unseen_out[1], 100.0 * r->eval.unseen_out[2], 100.0 * r->eval.unseen_out[3]);
    printf("  SUMMARY All    pass@1: 1x=%.1f%%  2x=%.1f%%  4x=%.1f%%  8x=%.1f%% | Verdict: %s\n",
           100.0 * r->eval.all_pass[0], 100.0 * r->eval.all_pass[1], 100.0 * r->eval.all_pass[2], 100.0 * r->eval.all_pass[3],
           verdict_str(r->eval.verdict));
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: %s KERNELS.{metal|cu} [--steps N] [--ckpt-dir DIR]\n", argv[0]); return 2; }
    const char *kernel_path = argv[1];
    int64_t total_steps = 320;
    const char *ckpt_dir = "/tmp";
    for (int i = 2; i + 1 < argc; i += 2) {
        if (!strcmp(argv[i], "--steps")) total_steps = atoll(argv[i + 1]);
        else if (!strcmp(argv[i], "--ckpt-dir")) ckpt_dir = argv[i + 1];
    }

#ifdef TD_GPU_METAL
    PmDeviceInfo info;
    PmStatus os = pm_open(&info);
    if (os == PM_ABSENT) { puts("SKIP (PM_ABSENT): no Metal GPU device present"); return 0; }
    if (os != PM_OK) { fprintf(stderr, "pm_open failed: %s\n", pm_log()); return 1; }
    printf("Backend: Metal 4 (trace-distill-metal-v1, fast math OFF)\n");
    printf("Device:  %s (Apple GPU family %d, unified=%d, working set %.1f GiB)\n",
           info.name, info.apple_family, info.unified_memory, (double)info.working_set_bytes / 1073741824.0);
#else
    PcDeviceInfo info;
    PcStatus os = pc_open(&info);
    if (os == PC_ABSENT || os == PC_NO_NVRTC) {
        printf("SKIP (%s): no NVIDIA CUDA GPU/NVRTC present on this host (%s)\n",
               os == PC_ABSENT ? "PC_ABSENT" : "PC_NO_NVRTC", pc_log());
        puts("trace-distill-cuda-v1 host compiled cleanly & Coax CUDA kernels emitted; run on NVIDIA GPU host for live CUDA execution.");
        return 0;
    }
    if (os != PC_OK) { fprintf(stderr, "pc_open failed: %s\n", pc_log()); return 1; }
    printf("Backend: CUDA (trace-distill-cuda-v1, --fmad=false)\n");
    printf("Device:  %s (SM %d.%d, %d SMs, %.1f GiB free / %.1f GiB total)\n",
           info.name, info.major, info.minor, info.sms,
           (double)info.free_bytes / 1073741824.0, (double)info.total_bytes / 1073741824.0);
    GPU_TRY(pc_stream(&g_stream));
    GPU_TRY(pc_event(&g_ev0));
    GPU_TRY(pc_event(&g_ev1));
#endif

    size_t src_len = 0;
    char *src = read_text_file(kernel_path, &src_len);
    if (!src) return 2;
    GpuMod mod = NULL;
    double compile_s = 0.0;
#ifdef TD_GPU_METAL
    GPU_TRY(pm_compile(src, src_len, "trace-distill-kernels.metal", &mod, &compile_s));
#else
    GPU_TRY(pc_compile(src, src_len, "trace-distill-kernels.cu", &mod, &compile_s));
#endif
    free(src);
    TdKernels kernels;
    load_kernels(mod, &kernels);
    printf("Compiled 24 Coax GPU kernels in %.3f s\n", compile_s);

    /* [0] W1.2 wasm-exec-v1 dense-weight parity & Ch 234 + Ch 247 verified corpus admission. */
    int64_t wasm_wc[1] = {0};
    uint8_t wasm_art[32] = {0};
    if (!td_verify_wasm_parity((coax_v_s64){wasm_wc, 1}, (coax_v_u8){wasm_art, 32})) {
        fprintf(stderr, "W1.2 wasm-exec-v1 parity check FAILED\n"); return 1;
    }
    char hex_art[65]; fmt_hex(wasm_art, hex_art);
    printf("\n[0] W1.2 wasm-exec-v1 & Ch 234/247 Verified Corpus Admission:\n");
    printf("  wasm-exec-v1 core artifact sha256: %.16s... | dense weight calls across 6 families: %lld (6/6 EXACT)\n",
           hex_art, (long long)wasm_wc[0]);

    coax_t_TdShape full = td_full_shape();
    int64_t n_tr = 252, ctx = full.train_ctx;
    Corpus corpus;
    corpus.n_traces = n_tr; corpus.ctx = ctx;
    corpus.ids = (uint8_t *)calloc((size_t)(n_tr * ctx), 1);
    corpus.tg = (uint8_t *)calloc((size_t)(n_tr * ctx), 1);
    corpus.mask = (uint8_t *)calloc((size_t)(n_tr * ctx), 1);
    corpus.aux_tg = (uint8_t *)calloc((size_t)(n_tr * ctx * 4), 1);
    corpus.aux_mask = (uint8_t *)calloc((size_t)(n_tr * ctx), 1);
    corpus.src_pos = (int64_t *)calloc((size_t)(n_tr * ctx), 8);
    uint8_t ledger_dig[32] = {0};
    int64_t ledger_cnt[4] = {0};
    if (!td_build_verified_corpus(ctx,
            (coax_v_u8){corpus.ids, n_tr * ctx}, (coax_v_u8){corpus.tg, n_tr * ctx},
            (coax_v_u8){corpus.mask, n_tr * ctx}, (coax_v_u8){corpus.aux_tg, n_tr * ctx * 4},
            (coax_v_u8){corpus.aux_mask, n_tr * ctx}, (coax_v_s64){corpus.src_pos, n_tr * ctx},
            (coax_v_u8){ledger_dig, 32}, (coax_v_s64){ledger_cnt, 4})) {
        fprintf(stderr, "Ch 247 verified corpus admission FAILED\n"); return 1;
    }
    char hex_led[65]; fmt_hex(ledger_dig, hex_led);
    printf("  Ch 247 exposure ledger: train=%lld heldout=%lld groups=%lld leakage_probes_blocked=%lld | sha256=%s\n",
           (long long)ledger_cnt[0], (long long)ledger_cnt[1], (long long)ledger_cnt[2], (long long)ledger_cnt[3], hex_led);

    /* [1] Live GPU-vs-CPU parity on tiny shape across all 3 arms + KV decode. */
    verify_gpu_cpu_parity(&kernels);

    /* Allocate REAL 10.72M-parameter decoder on GPU. */
    TdGpuState st;
    td_gpu_alloc(&st, full, 2048, 16);
    printf("\nModel: Psionic 6-layer Causal Decoder (d=%lld, L=%lld, H=%lld, d_ff=%lld, V=%lld, T_train=%lld, T_eval_max=2048)\n",
           (long long)full.d, (long long)full.layers, (long long)full.heads, (long long)full.hidden,
           (long long)full.vocab, (long long)full.train_ctx);
    printf("Parameters: backbone=%lld (>= 10M verified), aux_head=%lld (Variant B only)\n",
           (long long)td_backbone_params(full), (long long)(full.d * full.aux_dim));

    /* [2] Live 10.72M exact save/resume verification. */
    verify_exact_resume_10m(&kernels, &st, &corpus, ckpt_dir);

    /* [3] Run all three pre-registered experimental arms under identical token budgets. */
    ArmResult res_a = train_and_eval_arm(&kernels, &st, &corpus, COAX_TdArm_baseline_a, total_steps);
    ArmResult res_b = train_and_eval_arm(&kernels, &st, &corpus, COAX_TdArm_variant_b, total_steps);
    ArmResult res_c = train_and_eval_arm(&kernels, &st, &corpus, COAX_TdArm_variant_c, total_steps);

    if (!tdp_arm_budget_ok(res_a.total_tokens, res_b.total_tokens, res_c.total_tokens, total_steps * full.rows * full.train_ctx)) {
        fprintf(stderr, "Token budget parity violated!\n"); return 1;
    }

    /* [4] Full unedited Ch 234 standalone autoregressive rollout tables & H1/H3/H5 verdicts. */
    print_detailed_table("Baseline A (H1: Standard Next-Token SFT)", &res_a);
    print_detailed_table("Variant B  (H5: Aux State Supervision + Ch 139 RL)", &res_b);
    print_detailed_table("Variant C  (H3: Parabolic Init + Max-Margin + Beta Annealing)", &res_c);

    coax_t_TdW5Lane w5 = tdp_w5_recommend(res_a.eval.verdict, res_b.eval.verdict, res_c.eval.verdict);
    printf("\n================ FINAL PRE-REGISTERED VERDICTS ================\n");
    printf("  Identical Token Budget: %lld tokens / arm (VERIFIED)\n", (long long)res_a.total_tokens);
    printf("  Arm A (H1: Pure SFT):               %s | throughput=%.0f tok/s | wall=%.2fs\n", verdict_str(res_a.eval.verdict), res_a.tok_per_s, res_a.wall_s);
    printf("  Arm B (H5: Aux + Ch 139 RL):        %s | throughput=%.0f tok/s | wall=%.2fs | RL reward=%.3f (%lld/%lld exact, %lld mixed groups)\n",
           verdict_str(res_b.eval.verdict), res_b.tok_per_s, res_b.wall_s, res_b.rl_mean_reward,
           (long long)res_b.rl_exact_count, (long long)res_b.rl_total_rollouts, (long long)res_b.rl_mixed_groups);
    printf("  Arm C (H3: Parabolic+Margin+Beta):  %s | throughput=%.0f tok/s | wall=%.2fs | final margin_loss=%.4f\n",
           verdict_str(res_c.eval.verdict), res_c.tok_per_s, res_c.wall_s, res_c.margin_loss_end);
    printf("  W5 Decentralized Pylon Recommendation: %s\n",
           w5 == COAX_TdW5Lane_h2_routing_only ? "H2_ROUTING_ONLY (keep Pylon training on routing/interface adapters around frozen exact cores)"
                                               : "H2_PLUS_STUDENT_BACKBONE");
    printf("===============================================================\n");

    td_gpu_free(&st);
    free(corpus.ids); free(corpus.tg); free(corpus.mask); free(corpus.aux_tg); free(corpus.aux_mask); free(corpus.src_pos);
    return 0;
}
