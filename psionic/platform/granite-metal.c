/* See granite-metal.h. Compile with -DGRANITE_COAX_HEADER='"host.h"'. */
#include <dispatch/dispatch.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include GRANITE_COAX_HEADER
#include "granite-metal.h"
#include "metal-host.h"

/* The Coax constants this host relies on (psionic/lib/granite/kernels.coax
   and model.coax). */
#define GR_B 8 /* tokens a matrix-vector launch carries */
#define GR_P 512 /* tokens a prefill launch carries (GRANITE_CHUNK overrides it) */
#define GR_D 8 /* rows a decode group writes */
#define GR_F32 0
#define GR_Q8_0 8
#define GR_Q4_K 12
#define GR_Q6_K 14
#define gr_index(layer, slot) (3 + (layer) * 9 + (slot))

enum { K_MV_Q4K, K_MV_Q6K, K_MV_Q8, K_EMBED_Q4K, K_EMBED_Q6K, K_EMBED_Q8, K_NORM, K_ROPE, K_STORE, K_SCORES, K_SOFTMAX,
       K_CONTEXT_PART, K_CONTEXT, K_SWIGLU, K_ARGMAX_PART, K_ARGMAX, K_DV_Q4K, K_DV_Q6K, K_DV_Q8, K_DV3_Q4K, K_DV3_Q6K, K_DV3_Q8, K_GLU_Q4K, K_GLU_Q6K, K_GLU_Q8,
       K_ATT_SCORES, K_ATT_SOFTMAX, K_ATT_VALUES, K_WIDEN_Q4K, K_WIDEN_Q6K, K_WIDEN_Q8, K_PAIR, K_ADD, K_TILE, K_COUNT };
static const char *const kernel_names[K_COUNT] = {"gr_mv_q4k", "gr_mv_q6k", "gr_mv_q8", "gr_embed_q4k", "gr_embed_q6k", "gr_embed_q8",
    "gr_norm", "gr_rope", "gr_store", "gr_scores", "gr_softmax", "gr_context_part", "gr_context", "gr_swiglu", "gr_argmax_part", "gr_argmax", "gr_dv_q4k", "gr_dv_q6k", "gr_dv_q8", "gr_dv3_q4k", "gr_dv3_q6k", "gr_dv3_q8", "gr_dvglu_q4k", "gr_dvglu_q6k", "gr_dvglu_q8",
    "gr_att_scores", "gr_att_softmax", "gr_att_values", "gr_widen_q4k", "gr_widen_q6k", "gr_widen_q8", "gr_pair", "gr_add", "gr_tile"};
/* Threads per work item: tensor-op tiles are groups of 256, prefill
   attention tiles of 128, other products of 64, norms and softmax of 256. */
static int kernel_lanes(int k) {
    if (k == K_TILE) return 256;
    if (k >= K_WIDEN_Q4K) return 1;
    if (k == K_ATT_SCORES || k == K_ATT_VALUES) return 128;
    if (k == K_ATT_SOFTMAX) return 256;
    return k <= K_MV_Q8 || k >= K_DV_Q4K ? 64 : (k == K_NORM || k == K_SOFTMAX) ? 256 : 1; }

/* Activation buffers. */
enum { B_X, B_XN, B_Q, B_K, B_V, B_ATT, B_GATE, B_UP, B_ACT, B_SCORES, B_PARTIAL, B_CACHE, B_PAGES, B_ROPE, B_IDS, B_LOGITS,
       B_BEST, B_INDEX, B_CONTROL, B_W2, B_X2, B_TMP, B_WEIGHTS, B_COUNT };
typedef struct { PmBuffer metal; uint8_t *cpu; size_t bytes; } GrBuffer;
typedef struct { int kernel; int64_t items; int count; PmArg args[16]; } GrOp;

struct GrSession {
    GrBackend backend; GrFile file; coax_t_GrShape shape; int64_t capacity, pages_per_layer, kv_width, position, parts, chunk, att_rows;
    GrBuffer buffers[B_COUNT]; PmModule module; PmKernel kernels[K_COUNT];
    GrOp *ops; int64_t op_count, op_max; char error[512];
    double gpu_seconds, wait_seconds, encode_seconds; int profile, skip; double kernel_seconds[K_COUNT]; int64_t kernel_launches[K_COUNT];
};

static uint8_t *bytes_of(GrSession *s, int b) { return s->buffers[b].cpu; }
static int fail(GrSession *s, const char *what) { snprintf(s->error, sizeof s->error, "%s", what); return 1; }
static int alloc_buffer(GrSession *s, int b, size_t bytes) {
    GrBuffer *x = &s->buffers[b]; x->bytes = bytes;
    if (s->backend == GR_METAL) {
        if (pm_alloc(bytes, &x->metal)) return fail(s, pm_log());
        x->cpu = pm_contents(x->metal);
    } else {
        x->cpu = calloc(1, bytes ? bytes : 16);
        if (!x->cpu) return fail(s, "out of memory");
    }
    return 0;
}

/* ---- Recording launches. */
static PmArg arg_int(int64_t v) { return (PmArg){.kind = PM_ARG_INT, .i = v}; }
static PmArg arg_view(GrSession *s, int b, int64_t offset, int64_t count) {
    return (PmArg){.kind = PM_ARG_VIEW, .buffer = s->buffers[b].metal, .offset = offset, .count = count, .i = b};
}
/* A view of n f32 starting at element first. */
static PmArg f32s(GrSession *s, int b, int64_t first, int64_t n) { return arg_view(s, b, first * 4, n); }
static PmArg all_f32(GrSession *s, int b) { return arg_view(s, b, 0, (int64_t)(s->buffers[b].bytes / 4)); }
static PmArg all_s64(GrSession *s, int b) { return arg_view(s, b, 0, (int64_t)(s->buffers[b].bytes / 8)); }
static PmArg all_u16(GrSession *s, int b) { return arg_view(s, b, 0, (int64_t)(s->buffers[b].bytes / 2)); }
static PmArg all_u32(GrSession *s, int b) { return arg_view(s, b, 0, (int64_t)(s->buffers[b].bytes / 4)); }
static PmArg weights(GrSession *s, const coax_t_GrTensor *t) {
    int64_t width = t->kind == GR_Q4_K ? 8 : t->kind == GR_F32 ? 4 : 2;
    return arg_view(s, B_WEIGHTS, t->begin, t->bytes / width);
}
static void record(GrSession *s, int kernel, int64_t items, int count, const PmArg *args) {
    if (s->op_count == s->op_max) { s->op_max = s->op_max ? 2 * s->op_max : 1024; s->ops = realloc(s->ops, (size_t)s->op_max * sizeof *s->ops); }
    GrOp *op = &s->ops[s->op_count++]; op->kernel = kernel; op->items = items; op->count = count; memcpy(op->args, args, (size_t)count * sizeof *args);
}
#define RECORD(s, kernel, items, ...) do { PmArg a_[] = {__VA_ARGS__}; record(s, kernel, items, (int)(sizeof a_ / sizeof a_[0]), a_); } while (0)

static const coax_t_GrTensor *tensor(GrSession *s, int index) { return &s->file.table[index]; }
static int format(const coax_t_GrTensor *w) { return w->kind == GR_Q4_K ? 0 : w->kind == GR_Q6_K ? 1 : 2; }
/* Decode: q, k and v (those of q's format) in one launch. */
static void product3(GrSession *s, const coax_t_GrTensor *a, const coax_t_GrTensor *b, const coax_t_GrTensor *c, PmArg x, PmArg oa, PmArg ob, PmArg oc) {
    int f = format(a), with_c = format(c) == f;
    int64_t rc = with_c ? c->rows : 0, items = (a->rows + GR_D - 1) / GR_D + (b->rows + GR_D - 1) / GR_D + (rc + GR_D - 1) / GR_D;
    RECORD(s, K_DV3_Q4K + f, items, arg_int(a->rows), arg_int(b->rows), arg_int(rc), arg_int(a->cols), weights(s, a), weights(s, b), weights(s, with_c ? c : b), x, oa, ob, oc);
}
/* Decode: silu(gate) * up in one launch. */
static void product_glu(GrSession *s, const coax_t_GrTensor *g, const coax_t_GrTensor *u, PmArg x, PmArg out) {
    RECORD(s, K_GLU_Q4K + format(g), (g->rows + 3) / 4, arg_int(g->rows), arg_int(g->cols), weights(s, g), weights(s, u), x, out);
}
/* pair 0 reuses the paired inputs of the previous prefill product, which
   read the same x. */
static void product_paired(GrSession *s, const coax_t_GrTensor *w, int64_t tokens, int add, PmArg x, PmArg out, int pair) {
    if (tokens == 1) {
        int k1 = w->kind == GR_Q4_K ? K_DV_Q4K : w->kind == GR_Q6_K ? K_DV_Q6K : K_DV_Q8;
        RECORD(s, k1, (w->rows + GR_D - 1) / GR_D, arg_int(w->rows), arg_int(w->cols), arg_int(add), weights(s, w), x, out); return;
    }
    if (tokens > GR_B) {
        /* Prefill: the weights widened to BF16 pairs, then tensor-op tiles. */
        int64_t n2 = tokens * w->cols * 2; PmArg dst = add ? f32s(s, B_TMP, 0, tokens * w->rows) : out;
        RECORD(s, K_WIDEN_Q4K + format(w), w->rows * w->cols / (w->kind == GR_Q4_K ? 16 : 8), arg_int(w->cols), weights(s, w), all_u32(s, B_W2));
        if (pair) RECORD(s, K_PAIR, n2, x, f32s(s, B_X2, 0, n2));
        RECORD(s, K_TILE, (w->rows + 127) / 128 * ((tokens + 63) / 64), arg_int(w->rows), arg_int(2 * w->cols), arg_int(tokens), arg_view(s, B_W2, 0, w->rows * w->cols), f32s(s, B_X2, 0, n2), dst);
        if (add) RECORD(s, K_ADD, tokens * w->rows, out, f32s(s, B_TMP, 0, tokens * w->rows));
        return;
    }
    int k = w->kind == GR_Q4_K ? K_MV_Q4K : w->kind == GR_Q6_K ? K_MV_Q6K : K_MV_Q8;
    RECORD(s, k, (w->rows + 3) / 4, arg_int(w->rows), arg_int(w->cols), arg_int(tokens), arg_int(add), weights(s, w), x, out);
}

static void product(GrSession *s, const coax_t_GrTensor *w, int64_t tokens, int add, PmArg x, PmArg out) { product_paired(s, w, tokens, add, x, out, 1); }

/* One layer for n tokens at position p. */
static void record_layer(GrSession *s, int layer, int64_t n, int64_t p) {
    coax_t_GrShape h = s->shape; int64_t width = h.width, kvw = s->kv_width, span = p + n, chunks = (span + 127) / 128;
    int64_t eps = (int64_t)(*(uint32_t *)&h.eps), scale = (int64_t)(*(uint32_t *)&h.attention_scale), lp = layer * s->pages_per_layer;
    RECORD(s, K_NORM, n, arg_int(width), arg_int(eps), all_f32(s, B_X), weights(s, tensor(s, gr_index(layer, 0))), all_f32(s, B_XN));
    const coax_t_GrTensor *wq = tensor(s, gr_index(layer, 1)), *wk = tensor(s, gr_index(layer, 2)), *wv = tensor(s, gr_index(layer, 3));
    if (n == 1 && format(wk) == format(wq)) {
        product3(s, wq, wk, wv, all_f32(s, B_XN), all_f32(s, B_Q), all_f32(s, B_K), all_f32(s, B_V));
        if (format(wv) != format(wq)) product(s, wv, n, 0, all_f32(s, B_XN), all_f32(s, B_V));
    } else {
        product(s, wq, n, 0, all_f32(s, B_XN), all_f32(s, B_Q));
        product_paired(s, wk, n, 0, all_f32(s, B_XN), all_f32(s, B_K), wk->cols != wq->cols);
        product_paired(s, wv, n, 0, all_f32(s, B_XN), all_f32(s, B_V), wv->cols != wq->cols);
    }
    RECORD(s, K_ROPE, n * (h.heads + h.kv_heads) * 64, arg_int(h.heads), arg_int(h.kv_heads), arg_int(p), all_f32(s, B_ROPE), all_f32(s, B_Q), all_f32(s, B_K));
    RECORD(s, K_STORE, n * kvw, arg_int(h.kv_heads), arg_int(p), arg_int(lp), all_s64(s, B_PAGES), all_f32(s, B_K), all_f32(s, B_V), all_u16(s, B_CACHE));
    if (n > GR_B) {
        /* Prefill: attention on simdgroup matrices, att_rows tokens at a time. */
        for (int64_t u = 0; u < n; u += s->att_rows) {
            int64_t m = n - u < s->att_rows ? n - u : s->att_rows, at = p + u, sp = at + m, tiles = (m + 63) / 64;
            RECORD(s, K_ATT_SCORES, h.heads * tiles * ((sp + 31) / 32), arg_int(h.heads), arg_int(h.kv_heads), arg_int(at), arg_int(m), arg_int(sp), arg_int(s->capacity), arg_int(scale),
                   arg_int(lp), all_s64(s, B_PAGES), f32s(s, B_Q, u * width, m * width), all_u16(s, B_CACHE), all_f32(s, B_SCORES));
            RECORD(s, K_ATT_SOFTMAX, m * h.heads, arg_int(h.heads), arg_int(at), arg_int(sp), arg_int(s->capacity), all_f32(s, B_SCORES));
            RECORD(s, K_ATT_VALUES, h.heads * tiles * 4, arg_int(h.heads), arg_int(h.kv_heads), arg_int(at), arg_int(m), arg_int(sp), arg_int(s->capacity),
                   arg_int(lp), all_s64(s, B_PAGES), all_f32(s, B_SCORES), all_u16(s, B_CACHE), f32s(s, B_ATT, u * width, m * width));
        }
    } else {
    RECORD(s, K_SCORES, n * h.heads * span, arg_int(h.heads), arg_int(h.kv_heads), arg_int(p), arg_int(span), arg_int(s->capacity), arg_int(scale),
           arg_int(lp), all_s64(s, B_PAGES), all_f32(s, B_Q), all_u16(s, B_CACHE), all_f32(s, B_SCORES));
    RECORD(s, K_SOFTMAX, n * h.heads, arg_int(span), arg_int(s->capacity), all_f32(s, B_SCORES));
    RECORD(s, K_CONTEXT_PART, n * h.heads * chunks * 128, arg_int(h.heads), arg_int(h.kv_heads), arg_int(span), arg_int(s->capacity), arg_int(chunks),
           arg_int(lp), all_s64(s, B_PAGES), all_f32(s, B_SCORES), all_u16(s, B_CACHE), all_f32(s, B_PARTIAL));
    RECORD(s, K_CONTEXT, n * width, arg_int(chunks), all_f32(s, B_PARTIAL), all_f32(s, B_ATT));
    }
    product(s, tensor(s, gr_index(layer, 4)), n, 1, all_f32(s, B_ATT), all_f32(s, B_X));
    RECORD(s, K_NORM, n, arg_int(width), arg_int(eps), all_f32(s, B_X), weights(s, tensor(s, gr_index(layer, 5))), all_f32(s, B_XN));
    const coax_t_GrTensor *wg = tensor(s, gr_index(layer, 6)), *wu = tensor(s, gr_index(layer, 7));
    if (n == 1 && format(wg) == format(wu)) product_glu(s, wg, wu, all_f32(s, B_XN), all_f32(s, B_ACT));
    else {
        product(s, wg, n, 0, all_f32(s, B_XN), all_f32(s, B_GATE));
        product_paired(s, wu, n, 0, all_f32(s, B_XN), all_f32(s, B_UP), wu->cols != wg->cols);
        RECORD(s, K_SWIGLU, n * h.hidden, all_f32(s, B_GATE), all_f32(s, B_UP), all_f32(s, B_ACT));
    }
    product(s, tensor(s, gr_index(layer, 8)), n, 1, all_f32(s, B_ACT), all_f32(s, B_X));
}
static void record_embed(GrSession *s, int64_t n) {
    const coax_t_GrTensor *e = tensor(s, 0); int k = e->kind == GR_Q4_K ? K_EMBED_Q4K : e->kind == GR_Q6_K ? K_EMBED_Q6K : K_EMBED_Q8;
    RECORD(s, k, n * s->shape.width, arg_int(s->shape.width), arg_view(s, B_IDS, 0, n), weights(s, e), all_f32(s, B_X));
}
static void record_head(GrSession *s, int64_t n) {
    int64_t width = s->shape.width, eps = (int64_t)(*(uint32_t *)&s->shape.eps);
    /* Only the last token's logits are needed: norm and project its row. */
    RECORD(s, K_NORM, 1, arg_int(width), arg_int(eps), f32s(s, B_X, (n - 1) * width, width), weights(s, tensor(s, 1)), f32s(s, B_XN, 0, width));
    product(s, tensor(s, 2), 1, 0, f32s(s, B_XN, 0, width), all_f32(s, B_LOGITS));
    RECORD(s, K_ARGMAX_PART, s->parts, all_f32(s, B_LOGITS), all_f32(s, B_BEST), all_s64(s, B_INDEX));
    RECORD(s, K_ARGMAX, 1, arg_int(s->parts), all_f32(s, B_BEST), all_s64(s, B_INDEX), all_s64(s, B_CONTROL));
}

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }
/* ---- Running the recorded launches. */
static coax_v_f32 v_f32(GrSession *s, const PmArg *a) { return (coax_v_f32){(float *)(bytes_of(s, (int)a->i) + a->offset), a->count}; }
static coax_v_u64 v_u64(GrSession *s, const PmArg *a) { return (coax_v_u64){(uint64_t *)(bytes_of(s, (int)a->i) + a->offset), a->count}; }
static coax_v_u32 v_u32(GrSession *s, const PmArg *a) { return (coax_v_u32){(uint32_t *)(bytes_of(s, (int)a->i) + a->offset), a->count}; }
static coax_v_u16 v_u16(GrSession *s, const PmArg *a) { return (coax_v_u16){(uint16_t *)(bytes_of(s, (int)a->i) + a->offset), a->count}; }
static coax_v_s64 v_s64(GrSession *s, const PmArg *a) { return (coax_v_s64){(int64_t *)(bytes_of(s, (int)a->i) + a->offset), a->count}; }
static void cpu_item(GrSession *s, const GrOp *op, int64_t item) {
    const PmArg *a = op->args;
#define I(n) a[n].i
    switch (op->kernel) {
    case K_MV_Q4K: gr_mv_q4k(item, I(0), I(1), I(2), I(3), v_u64(s, &a[4]), v_f32(s, &a[5]), v_f32(s, &a[6])); break;
    case K_MV_Q6K: gr_mv_q6k(item, I(0), I(1), I(2), I(3), v_u16(s, &a[4]), v_f32(s, &a[5]), v_f32(s, &a[6])); break;
    case K_MV_Q8: gr_mv_q8(item, I(0), I(1), I(2), I(3), v_u16(s, &a[4]), v_f32(s, &a[5]), v_f32(s, &a[6])); break;
    case K_EMBED_Q4K: gr_embed_q4k(item, I(0), v_s64(s, &a[1]), v_u64(s, &a[2]), v_f32(s, &a[3])); break;
    case K_EMBED_Q6K: gr_embed_q6k(item, I(0), v_s64(s, &a[1]), v_u16(s, &a[2]), v_f32(s, &a[3])); break;
    case K_EMBED_Q8: gr_embed_q8(item, I(0), v_s64(s, &a[1]), v_u16(s, &a[2]), v_f32(s, &a[3])); break;
    case K_NORM: gr_norm(item, I(0), I(1), v_f32(s, &a[2]), v_f32(s, &a[3]), v_f32(s, &a[4])); break;
    case K_ROPE: gr_rope(item, I(0), I(1), I(2), v_f32(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5])); break;
    case K_STORE: gr_store(item, I(0), I(1), I(2), v_s64(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5]), v_u16(s, &a[6])); break;
    case K_SCORES: gr_scores(item, I(0), I(1), I(2), I(3), I(4), I(5), I(6), v_s64(s, &a[7]), v_f32(s, &a[8]), v_u16(s, &a[9]), v_f32(s, &a[10])); break;
    case K_SOFTMAX: gr_softmax(item, I(0), I(1), v_f32(s, &a[2])); break;
    case K_CONTEXT_PART: gr_context_part(item, I(0), I(1), I(2), I(3), I(4), I(5), v_s64(s, &a[6]), v_f32(s, &a[7]), v_u16(s, &a[8]), v_f32(s, &a[9])); break;
    case K_CONTEXT: gr_context(item, I(0), v_f32(s, &a[1]), v_f32(s, &a[2])); break;
    case K_SWIGLU: gr_swiglu(item, v_f32(s, &a[0]), v_f32(s, &a[1]), v_f32(s, &a[2])); break;
    case K_ARGMAX_PART: gr_argmax_part(item, v_f32(s, &a[0]), v_f32(s, &a[1]), v_s64(s, &a[2])); break;
    case K_DV_Q4K: gr_dv_q4k(item, I(0), I(1), I(2), v_u64(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5])); break;
    case K_DV_Q6K: gr_dv_q6k(item, I(0), I(1), I(2), v_u16(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5])); break;
    case K_DV3_Q4K: gr_dv3_q4k(item, I(0), I(1), I(2), I(3), v_u64(s, &a[4]), v_u64(s, &a[5]), v_u64(s, &a[6]), v_f32(s, &a[7]), v_f32(s, &a[8]), v_f32(s, &a[9]), v_f32(s, &a[10])); break;
    case K_DV3_Q6K: gr_dv3_q6k(item, I(0), I(1), I(2), I(3), v_u16(s, &a[4]), v_u16(s, &a[5]), v_u16(s, &a[6]), v_f32(s, &a[7]), v_f32(s, &a[8]), v_f32(s, &a[9]), v_f32(s, &a[10])); break;
    case K_DV3_Q8: gr_dv3_q8(item, I(0), I(1), I(2), I(3), v_u16(s, &a[4]), v_u16(s, &a[5]), v_u16(s, &a[6]), v_f32(s, &a[7]), v_f32(s, &a[8]), v_f32(s, &a[9]), v_f32(s, &a[10])); break;
    case K_GLU_Q4K: gr_dvglu_q4k(item, I(0), I(1), v_u64(s, &a[2]), v_u64(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5])); break;
    case K_GLU_Q6K: gr_dvglu_q6k(item, I(0), I(1), v_u16(s, &a[2]), v_u16(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5])); break;
    case K_GLU_Q8: gr_dvglu_q8(item, I(0), I(1), v_u16(s, &a[2]), v_u16(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5])); break;
    case K_DV_Q8: gr_dv_q8(item, I(0), I(1), I(2), v_u16(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5])); break;
    case K_ATT_SCORES: gr_att_scores(item, I(0), I(1), I(2), I(3), I(4), I(5), I(6), I(7), v_s64(s, &a[8]), v_f32(s, &a[9]), v_u16(s, &a[10]), v_f32(s, &a[11])); break;
    case K_ATT_SOFTMAX: gr_att_softmax(item, I(0), I(1), I(2), I(3), v_f32(s, &a[4])); break;
    case K_ATT_VALUES: gr_att_values(item, I(0), I(1), I(2), I(3), I(4), I(5), I(6), v_s64(s, &a[7]), v_f32(s, &a[8]), v_u16(s, &a[9]), v_f32(s, &a[10])); break;
    case K_WIDEN_Q4K: gr_widen_q4k(item, I(0), v_u64(s, &a[1]), v_u32(s, &a[2])); break;
    case K_WIDEN_Q6K: gr_widen_q6k(item, I(0), v_u16(s, &a[1]), v_u32(s, &a[2])); break;
    case K_WIDEN_Q8: gr_widen_q8(item, I(0), v_u16(s, &a[1]), v_u32(s, &a[2])); break;
    case K_PAIR: gr_pair(item, v_f32(s, &a[0]), v_f32(s, &a[1])); break;
    case K_ADD: gr_add(item, v_f32(s, &a[0]), v_f32(s, &a[1])); break;
    case K_TILE: gr_tile(item, I(0), I(1), I(2), v_u32(s, &a[3]), v_f32(s, &a[4]), v_f32(s, &a[5])); break;
    case K_ARGMAX: gr_argmax(item, I(0), v_f32(s, &a[1]), v_s64(s, &a[2]), v_s64(s, &a[3])); break;
    }
#undef I
}
/* The CPU repeat runs work items on at most four threads: each item writes
   only its own outputs, so the order does not change a result. */
#define GR_CPU_THREADS 4
static int run_ops(GrSession *s) {
    if (s->backend == GR_CPU) {
        for (int64_t i = 0; i < s->op_count; i++) {
            const GrOp *op = &s->ops[i];
            if (op->items < 64) { for (int64_t item = 0; item < op->items; item++) cpu_item(s, op, item); continue; }
            dispatch_apply(GR_CPU_THREADS, dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^(size_t w) {
                for (int64_t item = (int64_t)w; item < op->items; item += GR_CPU_THREADS) cpu_item(s, op, item);
            });
        }
        s->op_count = 0; return 0;
    }
    if (s->profile) {
        /* One batch a launch, to attribute GPU time to kernels. */
        for (int64_t i = 0; i < s->op_count; i++) {
            GrOp *op = &s->ops[i]; int lanes = kernel_lanes(op->kernel); PmBatch one; double gpu = 0;
            if (pm_begin(&one) || pm_launch(one, s->kernels[op->kernel], op->items, lanes, lanes == 1 ? 256 : lanes, op->args, op->count) || pm_wait(one, &gpu)) { s->op_count = 0; return fail(s, pm_log()); }
            s->kernel_seconds[op->kernel] += gpu; s->kernel_launches[op->kernel]++;
        }
        s->op_count = 0; return 0;
    }
    double e0 = now(); PmBatch batch; if (pm_begin(&batch)) { s->op_count = 0; return fail(s, pm_log()); }
    for (int64_t i = 0; i < s->op_count; i++) {
        GrOp *op = &s->ops[i]; int lanes = kernel_lanes(op->kernel);
        int is_product = op->kernel <= K_MV_Q8 || (op->kernel >= K_DV_Q4K && op->kernel <= K_GLU_Q8) || op->kernel >= K_WIDEN_Q4K;
        if (s->skip && ((s->skip == 1) == is_product)) continue;
        if (pm_launch(batch, s->kernels[op->kernel], op->items, lanes, lanes == 1 ? 256 : lanes, op->args, op->count)) { s->op_count = 0; pm_wait(batch, NULL); return fail(s, pm_log()); }
    }
    s->encode_seconds += now() - e0; double t0 = now(); s->op_count = 0; double gpu = 0; PmStatus m = pm_wait(batch, &gpu); s->gpu_seconds += gpu; s->wait_seconds += now() - t0;
    if (m == PM_CHECK) return fail(s, "a Coax check failed inside a kernel");
    return m ? fail(s, pm_log()) : 0;
}

int gr_session_eval(GrSession *s, const int64_t *ids, int64_t count, int64_t *next, GrLayerDump dump, void *context, double *seconds) {
    double start = now();
    if (count < 1 || s->position + count > s->capacity) return fail(s, "the conversation does not fit the cache");
    for (int64_t i = 0; i < count; i++) if (ids[i] < 0 || ids[i] >= s->shape.vocab) return fail(s, "a token id is outside the vocabulary");
    for (int64_t at = 0; at < count; at += s->chunk) {
        int64_t n = count - at < s->chunk ? count - at : s->chunk;
        memcpy(bytes_of(s, B_IDS), ids + at, (size_t)n * 8);
        record_embed(s, n);
        for (int layer = 0; layer < s->shape.layers; layer++) {
            record_layer(s, layer, n, s->position);
            if (dump) { if (run_ops(s)) return 1; dump(context, layer, (const float *)bytes_of(s, B_X), n, s->shape.width); }
        }
        if (at + n == count) record_head(s, n);
        if (run_ops(s)) return 1;
        s->position += n;
    }
    *next = ((int64_t *)bytes_of(s, B_CONTROL))[0];
    if (seconds) *seconds = now() - start;
    return 0;
}

int gr_session_open(const char *path, int64_t capacity, GrBackend backend, const char *metal_source, size_t metal_bytes,
                    GrSession **out, char *error, size_t error_bytes) {
    *out = NULL; GrSession *s = calloc(1, sizeof *s); if (!s) { snprintf(error, error_bytes, "out of memory"); return 1; }
    s->backend = backend; s->profile = getenv("GRANITE_PROFILE") != NULL; s->skip = getenv("GRANITE_SKIP") ? atoi(getenv("GRANITE_SKIP")) : 0;
    if (gr_file_open(path, &s->file, error, error_bytes)) { gr_session_close(s); return 1; }
    s->shape = s->file.binding.shape; coax_t_GrShape h = s->shape;
    if (capacity < 256 || capacity % 256 || capacity > h.context) { snprintf(error, error_bytes, "capacity must be a multiple of 256 up to %lld", (long long)h.context); gr_session_close(s); return 1; }
    s->chunk = getenv("GRANITE_CHUNK") ? atoll(getenv("GRANITE_CHUNK")) : GR_P;
    if (s->chunk < 1 || s->chunk > 4096) { snprintf(error, error_bytes, "GRANITE_CHUNK must be 1 to 4096"); gr_session_close(s); return 1; }
    /* Prefill attention keeps one score row per token and head: at most
       256 MiB of them, in multiples of 64 tokens. */
    s->att_rows = (int64_t)((256u << 20) / ((size_t)h.heads * (size_t)capacity * 4)) / 64 * 64;
    if (s->att_rows < 64) s->att_rows = 64;
    if (s->att_rows > s->chunk) s->att_rows = s->chunk;
    s->capacity = capacity; s->pages_per_layer = capacity / 256; s->kv_width = h.kv_heads * h.head_dim; s->parts = (h.vocab + 255) / 256;
    if (backend == GR_METAL) {
        PmDeviceInfo info;
        if (pm_open(&info)) { snprintf(error, error_bytes, "no Metal device: %s", pm_log()); gr_session_close(s); return 1; }
        double compile = 0;
        if (pm_compile(metal_source, metal_bytes, "granite.metal", &s->module, &compile)) { snprintf(error, error_bytes, "%s", pm_log()); gr_session_close(s); return 1; }
        for (int k = 0; k < K_COUNT; k++) if (pm_kernel(s->module, kernel_names[k], &s->kernels[k])) { snprintf(error, error_bytes, "%s", pm_log()); gr_session_close(s); return 1; }
        /* The GPU reads the weights where the file is mapped. */
        if (s->file.map_size > info.max_buffer_bytes) { snprintf(error, error_bytes, "the file needs one %zu-byte buffer; this GPU allows %zu", s->file.map_size, info.max_buffer_bytes); gr_session_close(s); return 1; }
        if (getenv("GRANITE_COPY")) {
            if (pm_alloc_unset(s->file.size, &s->buffers[B_WEIGHTS].metal)) { snprintf(error, error_bytes, "%s", pm_log()); gr_session_close(s); return 1; }
            memcpy(pm_contents(s->buffers[B_WEIGHTS].metal), s->file.bytes, s->file.size);
        } else if (pm_wrap((void *)s->file.bytes, s->file.map_size, &s->buffers[B_WEIGHTS].metal)) { snprintf(error, error_bytes, "%s", pm_log()); gr_session_close(s); return 1; }
        s->buffers[B_WEIGHTS].cpu = pm_contents(s->buffers[B_WEIGHTS].metal);
    } else s->buffers[B_WEIGHTS].cpu = (uint8_t *)s->file.bytes;
    s->buffers[B_WEIGHTS].bytes = s->file.size;
    size_t f = 4, b = (size_t)(s->chunk > GR_B ? s->chunk : GR_B); int64_t chunks = (capacity + 127) / 128;
    size_t sizes[B_WEIGHTS] = {
        [B_X] = b * h.width * f, [B_XN] = b * h.width * f, [B_Q] = b * h.width * f, [B_K] = b * s->kv_width * f, [B_V] = b * s->kv_width * f,
        [B_ATT] = b * h.width * f, [B_GATE] = b * h.hidden * f, [B_UP] = b * h.hidden * f, [B_ACT] = b * h.hidden * f,
        [B_SCORES] = (size_t)(s->att_rows > GR_B ? s->att_rows : GR_B) * h.heads * capacity * f, [B_PARTIAL] = (size_t)GR_B * h.heads * chunks * 128 * f,
        [B_CACHE] = (size_t)h.layers * s->pages_per_layer * 2 * h.kv_heads * 256 * 128 * 2, [B_PAGES] = (size_t)h.layers * s->pages_per_layer * 8,
        [B_ROPE] = (size_t)capacity * 128 * f, [B_IDS] = b * 8, [B_LOGITS] = (size_t)h.vocab * f, [B_BEST] = (size_t)s->parts * f,
        [B_INDEX] = (size_t)s->parts * 8, [B_CONTROL] = 8, [B_X2] = b * 2 * (size_t)(h.hidden > h.width ? h.hidden : h.width) * f, [B_TMP] = b * h.width * f};
    for (int l = 0; l < h.layers; l++) for (int slot = 1; slot < 9; slot++) {
        const coax_t_GrTensor *t = tensor(s, gr_index(l, slot)); size_t words = (size_t)(t->rows * t->cols) * 4;
        if (slot != 5 && words > sizes[B_W2]) sizes[B_W2] = words;
    }
    for (int i = 0; i < B_WEIGHTS; i++) if (alloc_buffer(s, i, sizes[i])) { snprintf(error, error_bytes, "%s", s->error); gr_session_close(s); return 1; }
    /* Pages are assigned in order: page j of layer l is l * pages_per_layer + j. */
    int64_t *pages = (int64_t *)bytes_of(s, B_PAGES); for (int64_t i = 0; i < h.layers * s->pages_per_layer; i++) pages[i] = i;
    if (!gr_rope_table(h.rope_base, capacity, (coax_v_f32){(float *)bytes_of(s, B_ROPE), capacity * 128})) { snprintf(error, error_bytes, "rotary table refused"); gr_session_close(s); return 1; }
    *out = s; return 0;
}
void gr_session_close(GrSession *s) {
    if (!s) return;
    if (getenv("GRANITE_TIMES")) fprintf(stderr, "encode %.3f s, wait %.3f s, gpu %.3f s\n", s->encode_seconds, s->wait_seconds, s->gpu_seconds);
    if (s->profile) for (int k = 0; k < K_COUNT; k++) if (s->kernel_launches[k]) fprintf(stderr, "%-16s %6lld launches %8.3f ms\n", kernel_names[k], (long long)s->kernel_launches[k], 1e3 * s->kernel_seconds[k]);
    for (int i = 0; i < B_COUNT; i++) {
        if (s->buffers[i].metal) pm_free(s->buffers[i].metal); else if (i != B_WEIGHTS) free(s->buffers[i].cpu);
    }
    for (int k = 0; k < K_COUNT; k++) if (s->kernels[k]) pm_kernel_free(s->kernels[k]);
    if (s->module) pm_unload(s->module);
    gr_file_close(&s->file); free(s->ops); free(s);
}
const GrFile *gr_session_file(const GrSession *s) { return &s->file; }
/* Best GPU seconds of one decode launch of `kernel` (rows `per` a group) on
   tensor `index`, over trials of reps launches. */
double gr_session_bench_kernel(GrSession *s, const char *name, int per, int index, int reps, int trials) {
    int kernel = -1; for (int k = 0; k < K_COUNT; k++) if (!strcmp(kernel_names[k], name)) kernel = k;
    if (kernel < 0) return -1;
    const coax_t_GrTensor *w = tensor(s, index); int big = w->cols > s->shape.width; double best = 1e9;
    for (int trial = 0; trial < trials; trial++) {
        for (int i = 0; i < reps; i++) RECORD(s, kernel, (w->rows + per - 1) / per, arg_int(w->rows), arg_int(w->cols), arg_int(0), weights(s, w), all_f32(s, big ? B_ACT : B_XN), all_f32(s, big ? B_X : B_GATE));
        PmBatch batch; double gpu = 0; pm_begin(&batch);
        for (int64_t i = 0; i < s->op_count; i++) { GrOp *op = &s->ops[i]; int lanes = kernel_lanes(op->kernel); pm_launch(batch, s->kernels[op->kernel], op->items, lanes, lanes, op->args, op->count); }
        s->op_count = 0; pm_wait(batch, &gpu); if (gpu / reps < best) best = gpu / reps;
    }
    return best;
}
/* GPU seconds of reps launches of one projection on `tokens` tokens. */
double gr_session_bench_product(GrSession *s, int index, int64_t tokens, int reps) {
    const coax_t_GrTensor *w = tensor(s, index); int big = w->cols > s->shape.width;
    double best = 1e9;
    for (int trial = 0; trial < 5; trial++) {
        for (int i = 0; i < reps; i++) product(s, w, tokens, 0, all_f32(s, big ? B_ACT : B_XN), all_f32(s, big ? B_X : B_GATE));
        PmBatch batch; double gpu = 0; pm_begin(&batch);
        for (int64_t i = 0; i < s->op_count; i++) { GrOp *op = &s->ops[i]; int lanes = kernel_lanes(op->kernel); pm_launch(batch, s->kernels[op->kernel], op->items, lanes, lanes, op->args, op->count); }
        s->op_count = 0; pm_wait(batch, &gpu); if (gpu < best) best = gpu;
    }
    return best;
}
int64_t gr_session_position(const GrSession *s) { return s->position; }
void gr_session_reset(GrSession *s) { s->position = 0; }
const float *gr_session_logits(const GrSession *s) { return (const float *)s->buffers[B_LOGITS].cpu; }
const char *gr_session_error(const GrSession *s) { return s->error; }
double gr_session_gpu_seconds(const GrSession *s) { return s->gpu_seconds; }
