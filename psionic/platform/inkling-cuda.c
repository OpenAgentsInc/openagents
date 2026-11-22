/* Native Psionic CUDA engine for Inkling 2 Mini (inkling2_loop GGUF) on
   NVIDIA GPUs, Linux and Windows. It implements inkling-engine.h.

   Kernels are Coax (psionic/lib/inkling/kernels-cuda.coax), emitted by
   `coax cuda`, compiled at startup by NVRTC and run through cuda-host.c. No
   arithmetic of the model lives in this file, and nothing here links cuBLAS
   or cuDNN.

   Memory. A discrete GPU holds weights in VRAM, apart from host memory:
     - Every tensor except the MoE experts and the token embedding is copied
       to VRAM once (about 1.2 GiB for Inkling 2 Mini).
     - The token embedding stays in host memory; a step uploads its one row.
     - The MoE experts (about 15 GiB of the 16.4 GiB file) go through a
       per-layer expert cache. When VRAM holds them all, each layer is
       resident and a step never waits on the host. When it does not, each
       layer keeps as many experts as its share of VRAM allows and the rest
       stay in host memory (the mapped GGUF): after a layer's router runs, the
       host reads the chosen expert ids, uploads the ones not in the cache
       over the least recently used slots, and hands the kernels slot numbers
       in place of expert ids. The arithmetic is the same either way.
   The plan is made before any large allocation, so a GPU that cannot hold
   even the smallest cache is refused with a message instead of running out
   of memory. */
#include "inkling-engine.h"
#include "inkling-gguf.h"
#include "cuda-host.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define INK_T   64
#define INK_NT  256
#define INK_MT  1024
#define INK_D   8
#define INK_MAX_BLOCKS 8
#define INK_MAX_EXPERTS 256
/* A layer's cache holds at least this many experts: two steps' worth, so the
   experts one step chose are never evicted while that step still needs them. */
#define INK_MIN_EXPERT_SLOTS 16
/* VRAM left unplanned: the kernel module, driver bookkeeping and whatever
   else the desktop allocates while the model runs. */
#define INK_VRAM_MARGIN_MB 384
#define INK_LAYER_TENSORS ((int)(sizeof(InkLayerTensors) / sizeof(InkTensor)))

/* Tokens a prefill batch may hold. Decode is a batch of one. */
#define INK_MAX_BATCH 32

enum {
    K_EMBED_Q8 = 0,
    K_RMSNORM,
    K_DV_IQ4,
    K_DVQ,
    K_DVR_F32,
    K_DVS_F32,
    K_MOE_GU,
    K_MOE_DOWN,
    K_ROUTER,
    K_MIX,
    K_CTRL_PREP,
    K_CTRL_APPLY,
    K_CONVOLVE,
    K_CLIP,
    K_HEAD_RMSNORM,
    K_KV_STORE,
    K_ATTN,
    K_ADD,
    K_COPY,
    K_SOFTCAP,
    K_Q8_ROUND,
    K_RELAYOUT,
    K_MOE_POST,
    K_MOE_GU_T,
    K_MOE_DOWN_T,
    K_MOE_SUM,
    K_MVQ_P,
    K_MVQ_S,
    K_RMSN,
    K_ATTN_POST,
    K_PROJ_P,
    K_PROJ_S,
    K_KV1,
    K_COUNT
};

static const char *const k_names[K_COUNT] = {
    "ink_embed_q8",
    "ink_rmsnorm",
    "ink_dv_iq4",
    "ink_mvq",
    "ink_dvr_f32",
    "ink_dvs_f32",
    "ink_moe_gu",
    "ink_moe_down",
    "ink_router",
    "ink_mix",
    "ink_ctrl_gate",
    "ink_ctrl_out",
    "ink_convolve",
    "ink_clip",
    "ink_head_rmsnorm",
    "ink_kv_store",
    "ink_attn",
    "ink_add",
    "ink_copy",
    "ink_softcap",
    "ink_q8_round",
    "ink_relayout",
    "ink_moe_post",
    "ink_moe_gu_t",
    "ink_moe_down_t",
    "ink_moe_sum",
    "ink_mvq_p",
    "ink_mvq_s",
    "ink_rmsn",
    "ink_attn_post",
    "ink_proj_p",
    "ink_proj_s",
    "ink_kv1",
};

/* Threads per work item: the T of a group kernel's `for thread: 0..<T`
   loops, or 1 for a kernel that runs one thread per item. */
static const int k_threads[K_COUNT] = {
    INK_NT, /* K_EMBED_Q8 */
    INK_NT, /* K_RMSNORM */
    INK_T,  /* K_DV_IQ4 (the quant-kernel test only) */
    INK_MT, /* K_DVQ */
    INK_NT, /* K_DVR_F32 */
    1,      /* K_DVS_F32 */
    INK_MT, /* K_MOE_GU */
    INK_MT, /* K_MOE_DOWN */
    INK_NT, /* K_ROUTER */
    INK_NT, /* K_MIX */
    INK_MT, /* K_CTRL_PREP: ink_ctrl_gate */
    1,      /* K_CTRL_APPLY: ink_ctrl_out */
    1,      /* K_CONVOLVE */
    INK_T,  /* K_CLIP */
    INK_T,  /* K_HEAD_RMSNORM */
    1,      /* K_KV_STORE */
    1024,   /* K_ATTN */
    1,      /* K_ADD */
    1,      /* K_COPY */
    1,      /* K_SOFTCAP */
    1,      /* K_Q8_ROUND */
    1,      /* K_RELAYOUT */
    1,      /* K_MOE_POST */
    INK_NT, /* K_MOE_GU_T */
    INK_NT, /* K_MOE_DOWN_T */
    INK_NT, /* K_MOE_SUM */
    1,      /* K_MVQ_P */
    1,      /* K_MVQ_S */
    INK_NT, /* K_RMSN */
    1,      /* K_ATTN_POST */
    1,      /* K_PROJ_P */
    1,      /* K_PROJ_S */
    INK_NT, /* K_KV1 */
};

/* One layer's experts on the device. cap == n_expert means every expert is
   resident in file order and ids need no translation. */
typedef struct {
    int      cap;
    size_t   gu_bytes, dn_bytes;          /* one expert's gate_up and down blocks */
    PcBuffer gu, dn;                      /* cap experts each */
    int16_t  slot_of[INK_MAX_EXPERTS];    /* expert -> slot, or -1 */
    int16_t  expert_in[INK_MAX_EXPERTS];  /* slot -> expert, or -1 */
    uint64_t used[INK_MAX_EXPERTS];       /* slot -> tick of its last use */
} ExpertCache;

struct InkSession {
    InkGGUF gguf;
    PcModule module;
    PcKernel kernels[K_COUNT];
    PcStream stream;
    int device_open;

    /* Weights on the device. */
    PcBuffer weights;                                /* every resident tensor, packed */
    PcBuffer stage;                                  /* GGUF-order blocks on their way to device order */
    size_t stage_bytes;
    PcBuffer layer_dev[32][sizeof(InkLayerTensors) / sizeof(InkTensor)];
    PcBuffer out_res_query_dev, out_norm_dev, output_dev, tok_norm_dev;
    PcBuffer embd_row;                               /* the batch's token_embd rows, uploaded each batch */
    int batch;                                       /* tokens a batch holds: scratch is [batch, width] */
    ExpertCache experts[32];
    int streaming;                                   /* some layer's cap < n_expert */
    uint64_t tick, expert_hits, expert_misses, expert_upload_bytes;
    double queue_seconds;                            /* time spent queueing work, before the last wait of each call */

    /* Scratch activations (f32 unless noted), each [batch, width]. The
       block and message stacks are [stack, batch, n_embd]. */
    PcBuffer cur, prelude, bank_out;
    PcBuffer blocks, prelude_blk, bank_blk;          /* [INK_MAX_BLOCKS * n_embd] */
    PcBuffer messages;                               /* [4][batch][n_embd] */
    PcBuffer inp3, inp3b;                            /* a layer's Q, K and V inputs: [3][batch][n_embd] */
    PcBuffer norm_b2;
    PcBuffer ctrl_gate;                              /* [3][batch][gate_rank] */
    size_t weights_bytes;
    PcBuffer proj;                                   /* the projections' outputs; the eight below are places in it */
    PcBuffer q_buf, k_raw, v_raw, r_buf, kdyn_buf, vdyn_buf, sinks_buf, gate_buf;
    PcBuffer k_buf, v_buf;
    size_t proj_floats;
    /* The fused projection launch (ink_proj_p, ink_proj_s): a layer's list of
       matrices, [0] when Q, K and V share one input and [1] when each has
       its own. n_mats == 0: the layer's matrices go one at a time. */
    PcBuffer proj_desc;
    struct { int n_mats; int64_t items, rows; } proj_list[32][2];
    PcBuffer attn_scores, attn_out, o_raw, norm_buf;
    PcBuffer router_logits;
    PcBuffer moe_ids;                                /* [batch, n_expert_used] u32, expert ids from the router */
    PcBuffer moe_slots;                              /* [batch, n_expert_used] u32, cache slots of those experts */
    PcBuffer moe_wts, moe_hid, moe_out;
    PcBuffer mv_part;                                /* one token's matvec shares: [blocks, rows] */
    size_t mv_part_floats;
    PcBuffer gu_part, dn_part;                       /* one token's table-kernel shares */
    int no_tables;                                   /* INK_NO_TABLES: IQ4_NL experts through the direct kernels */
    PcBuffer logits;
    PcBuffer dummy_f32;
    PcBuffer oracle_x;                               /* INK_ORACLE_Q8: a matvec input rounded as ggml rounds it */
    int oracle_q8;

    /* Per logical slot: recurrent shortconv state and KV cache (f16). */
    int n_slots;
    int max_ctx;
    InkMemoryBudget budget;
    int kv_cap[INK_MAX_SLOTS];
    PcBuffer kv_k[INK_MAX_SLOTS], kv_v[INK_MAX_SLOTS];
    PcBuffer conv_k[INK_MAX_SLOTS], conv_v[INK_MAX_SLOTS], conv_ao[INK_MAX_SLOTS], conv_fo[INK_MAX_SLOTS];

    /* Host copies of what the engine contract returns by pointer. */
    float *logits_host;
    float *hidden_host;
    float *zeros;                                    /* the largest shortconv state, all zero */

    int pos;
    int max_pos_written;
    struct {
        int valid;
        int pos;
        int has_logits;
        int32_t *tokens;
        float   *conv_state;
        float   *logits;
    } snaps[2];
    double total_gpu_seconds;
    int total_steps;
    int profile;
    double prof_seconds[K_COUNT];
    uint64_t prof_calls[K_COUNT];
    double prof_stream_seconds;
    struct { uint32_t type; int64_t rows, cols; double seconds; uint64_t calls; } prof_mv[24];
    char error[512];
};

const char *ink_backend_name(const InkSession *s) { (void)s; return "psionic-cuda"; }

static PcArg a_int(int64_t v) { return (PcArg){.kind = PC_ARG_INT, .i = v}; }
static PcArg a_f32(float v)   { return (PcArg){.kind = PC_ARG_F32, .f = v}; }
static PcArg a_view(PcBuffer b, int64_t byte_off, int64_t elem_count) {
    return (PcArg){.kind = PC_ARG_VIEW, .data = b + (uint64_t)byte_off, .count = elem_count};
}
static PcArg a_f32v(PcBuffer b, int64_t first_elem, int64_t count) { return a_view(b, first_elem * 4, count); }
static PcArg a_u16v(PcBuffer b, int64_t first_elem, int64_t count) { return a_view(b, first_elem * 2, count); }
static PcArg a_u32v(PcBuffer b, int64_t first_elem, int64_t count) { return a_view(b, first_elem * 4, count); }

/* Where a resident tensor lives on the device. */
static PcBuffer tensor_dev(const InkSession *s, const InkTensor *t) {
    const InkTensor *first = (const InkTensor *)&s->gguf.layers[0];
    const InkTensor *end = first + 32 * INK_LAYER_TENSORS;
    if (t >= first && t < end) {
        size_t i = (size_t)(t - first);
        return s->layer_dev[i / (size_t)INK_LAYER_TENSORS][i % (size_t)INK_LAYER_TENSORS];
    }
    if (t == &s->gguf.out_res_query) return s->out_res_query_dev;
    if (t == &s->gguf.out_norm) return s->out_norm_dev;
    if (t == &s->gguf.output) return s->output_dev;
    if (t == &s->gguf.tok_norm) return s->tok_norm_dev;
    return 0;
}
static PcArg a_tensor_f32(const InkSession *s, const InkTensor *t) {
    return a_view(tensor_dev(s, t), 0, (int64_t)(t->nbytes / 4));
}

static int call_k(InkSession *s, int k, int64_t items, int count, const PcArg *args) {
    double began = s->profile ? pc_seconds() : 0.0;
    PcStatus st = k_threads[k] > 1
        ? pc_launch_groups(s->kernels[k], items, k_threads[k], args, count, s->stream)
        : pc_launch(s->kernels[k], items, 256, args, count, s->stream);
    if (st == PC_OK && s->profile) {
        st = pc_sync_stream(s->stream);
        s->prof_seconds[k] += pc_seconds() - began;
        s->prof_calls[k]++;
    }
    if (st != PC_OK) {
        snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[k], pc_log());
        return 1;
    }
    return 0;
}

#define CALL(s, k, items, ...) do { \
    PcArg _args[] = { __VA_ARGS__ }; \
    if (call_k((s), (k), (items), (int)(sizeof _args / sizeof _args[0]), _args) != 0) return 1; \
} while (0)

static int host_fail(InkSession *s, const char *what) {
    snprintf(s->error, sizeof s->error, "%s: %s", what, pc_log());
    return 1;
}

/* 32-bit words of codes in a 32-value block: Q8_0 has 32 bytes, IQ4_NL 16. */
static int64_t quant_words(uint32_t type) { return type == INK_GGML_Q8_0 ? 8 : 4; }

/* Put a quantized matrix on the device in device order (see ink_relayout in
   kernels-cuda.coax): `rows` rows of `blocks` GGUF blocks at src become the
   block-major words and scales at dst, the same number of bytes. The rows go
   through the staging buffer a piece at a time. pair > 0 stores an expert's
   gate and up rows as neighbours and needs the whole matrix in one piece. */
static int relayout(InkSession *s, const uint8_t *src, PcBuffer dst, int64_t rows, int64_t blocks, int64_t words, int64_t pair) {
    int64_t row_bytes = blocks * (4 * words + 2);
    int64_t piece = (int64_t)s->stage_bytes / row_bytes;
    if (piece > rows) piece = rows;
    if (piece <= 0 || (pair > 0 && piece < rows)) {
        snprintf(s->error, sizeof s->error, "staging buffer too small for a matrix of %lld-byte rows", (long long)row_bytes);
        return 1;
    }
    for (int64_t row0 = 0; row0 < rows; row0 += piece) {
        int64_t n = rows - row0 < piece ? rows - row0 : piece;
        if (pc_upload(s->stage, src + row0 * row_bytes, (size_t)(n * row_bytes), s->stream) != PC_OK) return host_fail(s, "upload weights");
        CALL(s, K_RELAYOUT, n * blocks * (words + 1), a_int(rows), a_int(row0), a_int(n), a_int(blocks), a_int(words), a_int(pair),
             a_view(s->stage, 0, n * row_bytes / 2),
             a_view(dst, 0, rows * blocks * words), a_view(dst, rows * blocks * words * 4, rows * blocks));
    }
    return 0;
}

/* One expert of layer ip from the GGUF into cache slot `slot`. */
static int load_expert(InkSession *s, int ip, int e, int slot) {
    const InkHParams *hp = &s->gguf.hp;
    const InkLayerTensors *L = &s->gguf.layers[ip];
    ExpertCache *c = &s->experts[ip];
    const uint8_t *gu = s->gguf.map + L->ffn_gate_up_exps.offset + (size_t)e * c->gu_bytes;
    const uint8_t *dn = s->gguf.map + L->ffn_down_exps.offset + (size_t)e * c->dn_bytes;
    return relayout(s, gu, c->gu + (uint64_t)slot * c->gu_bytes, 2 * hp->n_ff_exp, hp->n_embd / 32,
                    quant_words(L->ffn_gate_up_exps.type), hp->n_ff_exp) ||
           relayout(s, dn, c->dn + (uint64_t)slot * c->dn_bytes, hp->n_embd, hp->n_ff_exp / 32,
                    quant_words(L->ffn_down_exps.type), 0);
}

static int dispatch_mv_run(InkSession *s, const InkTensor *w, int64_t n, int64_t rows, int64_t cols, int add,
                           PcBuffer x_buf, int64_t x_off, PcBuffer out_buf, int64_t out_off);

/* out = W x (or out += W x) for n tokens and one weight matrix in its GGUF
   type: x is [n, cols] and out is [n, rows]. */
static int dispatch_mv(InkSession *s, const InkTensor *w, int64_t n, int64_t rows, int64_t cols, int add,
                       PcBuffer x_buf, int64_t x_off, PcBuffer out_buf, int64_t out_off) {
    if (!s->profile) return dispatch_mv_run(s, w, n, rows, cols, add, x_buf, x_off, out_buf, out_off);
    double began = pc_seconds();
    int rc = dispatch_mv_run(s, w, n, rows, cols, add, x_buf, x_off, out_buf, out_off);
    for (size_t i = 0; i < sizeof s->prof_mv / sizeof s->prof_mv[0]; i++) {
        if (s->prof_mv[i].calls == 0) { s->prof_mv[i].type = w->type; s->prof_mv[i].rows = rows; s->prof_mv[i].cols = cols; }
        if (s->prof_mv[i].type != w->type || s->prof_mv[i].rows != rows || s->prof_mv[i].cols != cols) continue;
        s->prof_mv[i].seconds += pc_seconds() - began;
        s->prof_mv[i].calls++;
        break;
    }
    return rc;
}

static int dispatch_mv_run(InkSession *s, const InkTensor *w, int64_t n, int64_t rows, int64_t cols, int add,
                           PcBuffer x_buf, int64_t x_off, PcBuffer out_buf, int64_t out_off) {
    int quantized = w->type == INK_GGML_Q8_0 || w->type == INK_GGML_IQ4_NL;
    if (quantized && cols % 32 != 0) {
        snprintf(s->error, sizeof s->error, "a quantized matrix with %lld columns is not whole blocks", (long long)cols);
        return 1;
    }
    if (quantized && s->oracle_q8) {
        CALL(s, K_Q8_ROUND, n * cols / 32, a_int(n * cols), a_f32v(x_buf, x_off, n * cols), a_f32v(s->oracle_x, 0, n * cols));
        x_buf = s->oracle_x; x_off = 0;
    }
    if (quantized) {
        int64_t words = quant_words(w->type), blocks = cols / 32;
        int64_t lrows = (int64_t)(w->nbytes / (uint64_t)(blocks * (4 * words + 2)));
        PcBuffer dev = tensor_dev(s, w);
        if (w->type == INK_GGML_Q8_0 && !s->no_tables && rows % 4 == 0 && (size_t)(rows * blocks) <= s->mv_part_floats) {
            for (int64_t tok = 0; tok < n; tok++) {
                CALL(s, K_MVQ_P, blocks * (rows / 4), a_int(rows), a_int(lrows), a_int(cols),
                     a_view(dev, 0, lrows * blocks * words), a_view(dev, lrows * blocks * words * 4, lrows * blocks),
                     a_f32v(x_buf, x_off + tok * cols, cols), a_f32v(s->mv_part, 0, rows * blocks));
                CALL(s, K_MVQ_S, rows, a_int(rows), a_int(blocks), a_int(add),
                     a_f32v(s->mv_part, 0, rows * blocks), a_f32v(out_buf, out_off + tok * rows, rows));
            }
            return 0;
        }
        CALL(s, K_DVQ, n * ((rows + 31) / 32), a_int(rows), a_int(lrows), a_int(cols), a_int(add), a_int(w->type == INK_GGML_Q8_0),
             a_view(dev, 0, lrows * blocks * words), a_view(dev, lrows * blocks * words * 4, lrows * blocks),
             a_f32v(x_buf, x_off, n * cols), a_f32v(out_buf, out_off, n * rows));
    } else if (w->type == INK_GGML_F32) {
        /* A wide matrix gets a group per output; a narrow one, a thread. */
        CALL(s, cols >= 1024 ? K_DVR_F32 : K_DVS_F32, n * rows, a_int(rows), a_int(cols), a_int(add),
             a_tensor_f32(s, w), a_f32v(x_buf, x_off, n * cols), a_f32v(out_buf, out_off, n * rows));
    } else {
        snprintf(s->error, sizeof s->error, "unsupported weight type %u for matvec", w->type);
        return 1;
    }
    return 0;
}

/* A resident f32 tensor's place in the weights, in floats: the kernels that
   take several weights of a layer take the whole arena and these. */
static int64_t arena_at(const InkSession *s, const InkTensor *t) {
    return (int64_t)((tensor_dev(s, t) - s->weights) / 4);
}
static PcArg a_arena(const InkSession *s) { return a_view(s->weights, 0, (int64_t)(s->weights_bytes / 4)); }

/* The cross-pass recurrence gate for np tokens and n inputs (messages c0 ..
   c0 + n - 1) at once: out[c] = cur - tanh(forget) cur + tanh(gain) msg, with
   gain and forget projected from (cur, msg). cur is [n][batch][d], or with
   single one vector that every input shares; out is [n][batch][d]. */
static int dispatch_control(InkSession *s, const InkLayerTensors *L, int c0, int n, int single, int64_t np,
                            PcBuffer cur_buf, PcBuffer out_buf) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd, stack = (int64_t)s->batch * d;
    int64_t flags = (single ? 1 : 0) | ((int64_t)c0 << 8);
    int64_t down[3] = {0, 0, 0}, gf[3] = {0, 0, 0};
    for (int c = 0; c < n; c++) {
        down[c] = arena_at(s, &L->loop_ctrl_down[c0 + c]);
        gf[c] = arena_at(s, &L->loop_ctrl_gain[c0 + c]) | (arena_at(s, &L->loop_ctrl_forget[c0 + c]) << 32);
    }
    int64_t cur_n = (single ? 1 : n) * stack;
    CALL(s, K_CTRL_PREP, n * np * hp->gate_rank, a_int(d), a_int(np), a_int(stack), a_int(flags),
         a_int(down[0]), a_int(down[1]), a_int(down[2]), a_arena(s),
         a_f32v(cur_buf, 0, cur_n), a_f32v(s->messages, 0, 4 * stack), a_f32v(s->ctrl_gate, 0, n * np * hp->gate_rank));
    CALL(s, K_CTRL_APPLY, n * np * d, a_int(d), a_int(np), a_int(stack), a_int(flags),
         a_int(gf[0]), a_int(gf[1]), a_int(gf[2]), a_arena(s),
         a_f32v(cur_buf, 0, cur_n), a_f32v(s->messages, 0, 4 * stack), a_f32v(s->ctrl_gate, 0, n * np * hp->gate_rank),
         a_f32v(out_buf, 0, n * stack));
    return 0;
}

/* Make the experts the router chose for physical layer ip present on the
   device and leave their cache slots in moe_slots: n ids, n_expert_used for
   each token of the batch. A resident layer needs nothing: its slots are the
   expert ids. A batch never needs more experts than the cache holds, because
   the batch size is at most cap / n_expert_used. */
static int stream_experts(InkSession *s, int ip, int n, PcBuffer *ids_buf) {
    ExpertCache *c = &s->experts[ip];
    const InkHParams *hp = &s->gguf.hp;
    *ids_buf = s->moe_ids;
    if (c->cap >= hp->n_expert) return 0;
    uint32_t ids[INK_MAX_BATCH * 16], slots[INK_MAX_BATCH * 16];
    double began = s->profile ? pc_seconds() : 0.0;
    if (pc_download(ids, s->moe_ids, (size_t)n * 4, s->stream) != PC_OK ||
        pc_sync_stream(s->stream) != PC_OK) return host_fail(s, "read expert ids");
    if (getenv("INK_EXPERT_TRACE")) { for (int k = 0; k < n; k++) fprintf(stderr, "XT %d %u\n", ip, ids[k]); }
    /* Mark what this batch needs before choosing what to replace. */
    s->tick++;
    for (int k = 0; k < n; k++) {
        if (ids[k] >= (uint32_t)hp->n_expert) {
            snprintf(s->error, sizeof s->error, "router chose expert %u of %lld", ids[k], (long long)hp->n_expert);
            return 1;
        }
        int slot = c->slot_of[ids[k]];
        if (slot >= 0) { c->used[slot] = s->tick; s->expert_hits++; }
    }
    for (int k = 0; k < n; k++) {
        uint32_t e = ids[k];
        int slot = c->slot_of[e];
        if (slot < 0) {
            slot = 0;
            for (int i = 1; i < c->cap; i++) if (c->used[i] < c->used[slot]) slot = i;
            if (c->expert_in[slot] >= 0) c->slot_of[c->expert_in[slot]] = -1;
            if (load_expert(s, ip, (int)e, slot)) return 1;
            c->slot_of[e] = (int16_t)slot;
            c->expert_in[slot] = (int16_t)e;
            c->used[slot] = s->tick;
            s->expert_misses++;
            s->expert_upload_bytes += c->gu_bytes + c->dn_bytes;
        }
        slots[k] = (uint32_t)slot;
    }
    if (pc_upload(s->moe_slots, slots, (size_t)n * 4, s->stream) != PC_OK) return host_fail(s, "write expert slots");
    if (s->profile) s->prof_stream_seconds += pc_seconds() - began;
    *ids_buf = s->moe_slots;
    return 0;
}

/* One logical layer il (physical layer ip) for the np tokens of the batch,
   which sit at positions pos .. pos + np - 1. */
static int dispatch_layer(InkSession *s, int il, int ip, int64_t np, int *n_blocks, int has_messages) {
    const InkHParams *hp = &s->gguf.hp;
    const InkLayerTensors *L = &s->gguf.layers[ip];
    int64_t d = hp->n_embd;
    int64_t nd = np * d;
    int64_t stack = (int64_t)s->batch * d; /* one entry of a block or message stack */
    int64_t n_head = hp->n_head;
    int64_t n_head_kv = hp->n_head_kv;
    int64_t hd = hp->head_dim;
    int64_t n_q = n_head * hd;
    int64_t n_kv = n_head_kv * hd;
    int64_t n_rel = hp->rel_len;
    int64_t n_dyn = n_head_kv * hp->conv_len;
    int is_swa = hp->is_swa[ip];

    /* Attention inputs. SWA mixes one vector for Q, K and V; full attention
       mixes three. With messages each input then passes its own gate. Then
       the input RMSNorms, into inp3b. */
    int one = is_swa || !L->attn_res_query.present; /* Q, K and V start as the same vector */
    if (L->attn_res_query.present) {
        int64_t nq = is_swa ? 1 : 3;
        CALL(s, K_MIX, np * nq, a_int(d), a_int(*n_blocks), a_int(nq), a_int(stack),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * stack),
             a_f32v(s->cur, 0, nd),
             a_tensor_f32(s, &L->attn_res_query),
             a_f32v(s->inp3, 0, 3 * stack));
    } else {
        CALL(s, K_COPY, nd, a_int(nd), a_f32v(s->cur, 0, nd), a_f32v(s->inp3, 0, nd));
    }
    PcBuffer norm_src = s->inp3;
    if (has_messages) {
        if (dispatch_control(s, L, 0, 3, one, np, s->inp3, s->inp3b)) return 1;
        norm_src = s->inp3b;
        one = 0;
    }
    int same = one && is_swa; /* one input and one norm: Q, K and V share a vector */
    {
        const InkTensor *nq_w = is_swa ? &L->attn_norm : &L->attn_q_inp_norm;
        const InkTensor *nk_w = is_swa ? &L->attn_norm : &L->attn_k_inp_norm;
        const InkTensor *nv_w = is_swa ? &L->attn_norm : &L->attn_v_inp_norm;
        CALL(s, K_RMSN, (same ? 1 : 3) * np, a_int(d), a_int(np), a_int(stack), a_int(one), a_f32(hp->rms_eps),
             a_int(arena_at(s, nq_w)), a_int(arena_at(s, nk_w)), a_int(arena_at(s, nv_w)), a_arena(s),
             a_f32v(norm_src, 0, 3 * stack), a_f32v(s->inp3b, 0, 3 * stack));
    }
    PcBuffer xin = s->inp3b;
    int64_t xq = 0, xk = same ? 0 : stack, xv = same ? 0 : 2 * stack;

    /* Q, K, V, R projections + K/V causal shortconv. */
    int variant = same ? 0 : 1;
    int fused = s->proj_list[ip][variant].n_mats > 0 && !s->oracle_q8 && !s->no_tables;
    for (int64_t tok = 0; fused && tok < np; tok++) {
        PcArg desc = a_u32v(s->proj_desc, (int64_t)(ip * 2 + variant) * 80, 80);
        CALL(s, K_PROJ_P, s->proj_list[ip][variant].items, a_int(s->proj_list[ip][variant].n_mats), a_int(tok), a_int(d), desc,
             a_view(s->weights, 0, (int64_t)(s->weights_bytes / 4)), a_view(s->weights, 0, (int64_t)(s->weights_bytes / 2)),
             a_f32v(xin, 0, 3 * stack), a_f32v(s->mv_part, 0, (int64_t)s->mv_part_floats));
        CALL(s, K_PROJ_S, s->proj_list[ip][variant].rows, a_int(s->proj_list[ip][variant].n_mats), a_int(tok), a_int(d / 32), desc,
             a_f32v(s->mv_part, 0, (int64_t)s->mv_part_floats), a_f32v(s->proj, 0, (int64_t)s->proj_floats));
    }
    if (!fused && dispatch_mv(s, &L->attn_q, np, n_q, d, 0, xin, xq, s->q_buf, 0)) return 1;
    if (!fused && dispatch_mv(s, &L->attn_k, np, n_kv, d, 0, xin, xk, s->k_raw, 0)) return 1;
    int k_dyn = !is_swa && L->attn_k_conv_proj.present;
    int v_dyn = !is_swa && L->attn_v_conv_proj.present;
    if (!fused && k_dyn && dispatch_mv(s, &L->attn_k_conv_proj, np, n_dyn, d, 0, xin, xk, s->kdyn_buf, 0)) return 1;
    if (!fused && dispatch_mv(s, &L->attn_v, np, n_kv, d, 0, xin, xv, s->v_raw, 0)) return 1;
    if (!fused && v_dyn && dispatch_mv(s, &L->attn_v_conv_proj, np, n_dyn, d, 0, xin, xv, s->vdyn_buf, 0)) return 1;
    if (!fused && dispatch_mv(s, &L->attn_rel, np, n_head * n_rel, d, 0, xin, xq, s->r_buf, 0)) return 1;

    /* K and V: the causal short convolutions, K's clip and V's RMSNorm, then
       into the KV cache of slot il. One token takes one launch. The
       attention kernel clips Q and R itself. */
    int64_t kv_cap = s->kv_cap[il];
    if (np == 1 && hd == 128 && k_dyn == v_dyn) {
        CALL(s, K_KV1, n_head_kv, a_int(n_kv), a_int(k_dyn), a_int(s->pos), a_int(kv_cap), a_f32(hp->qk_clip), a_f32(hp->rms_eps),
             a_f32v(s->k_raw, 0, n_kv), a_f32v(s->v_raw, 0, n_kv),
             a_tensor_f32(s, &L->attn_k_conv), a_tensor_f32(s, &L->attn_v_conv),
             k_dyn ? a_f32v(s->kdyn_buf, 0, n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             v_dyn ? a_f32v(s->vdyn_buf, 0, n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->conv_v[il], 0, n_kv * 3),
             a_u16v(s->kv_k[il], 0, kv_cap * n_kv), a_u16v(s->kv_v[il], 0, kv_cap * n_kv));
    } else {
        CALL(s, K_CONVOLVE, n_kv, a_int(n_kv), a_int(n_head_kv), a_int(k_dyn), a_int(np),
             a_f32v(s->k_raw, 0, np * n_kv), a_tensor_f32(s, &L->attn_k_conv),
             k_dyn ? a_f32v(s->kdyn_buf, 0, np * n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->k_buf, 0, np * n_kv));
        CALL(s, K_CONVOLVE, n_kv, a_int(n_kv), a_int(n_head_kv), a_int(v_dyn), a_int(np),
             a_f32v(s->v_raw, 0, np * n_kv), a_tensor_f32(s, &L->attn_v_conv),
             v_dyn ? a_f32v(s->vdyn_buf, 0, np * n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_v[il], 0, n_kv * 3), a_f32v(s->v_buf, 0, np * n_kv));
        CALL(s, K_CLIP, np * n_head_kv, a_int(hd), a_int(n_head_kv), a_int(0),
             a_f32(hp->qk_clip), a_f32(0.0f), a_f32(1.0f), a_int(s->pos),
             a_f32v(s->k_buf, 0, np * n_kv), a_f32v(s->dummy_f32, 0, 64));
        CALL(s, K_HEAD_RMSNORM, np * n_head_kv, a_int(hd), a_f32(hp->rms_eps),
             a_f32v(s->v_buf, 0, np * n_kv));
        CALL(s, K_KV_STORE, np * n_kv, a_int(n_kv), a_int(s->pos), a_int(kv_cap),
             a_f32v(s->k_buf, 0, np * n_kv), a_f32v(s->v_buf, 0, np * n_kv),
             a_u16v(s->kv_k[il], 0, kv_cap * n_kv), a_u16v(s->kv_v[il], 0, kv_cap * n_kv));
    }
    /* Full-attention Q and R are scaled by tau = 1 + alpha * log(max(1,
       (position + 1) / floor)), which the attention kernel computes for
       each token's own position. */
    float tau_alpha = is_swa ? 0.0f : hp->log_scale_alpha;
    float tau_floor = (float)hp->log_scale_floor;

    /* Sink logits and the attention output gate. */
    if (!fused && dispatch_mv(s, &L->attn_sink_proj, np, n_head, d, 0, xin, xq, s->sinks_buf, 0)) return 1;
    if (!fused && dispatch_mv(s, &L->attn_gate, np, n_q, d, 0, xin, xq, s->gate_buf, 0)) return 1;

    /* Attention. The kernel unpacks these packed dimensions. */
    int64_t dims0 = n_head | (n_head_kv << 16) | (hd << 32) | (n_rel << 48);
    int64_t dims1 = hp->rel_extent | ((int64_t)is_swa << 24) | (hp->swa << 32);
    int64_t kv_info = kv_cap | ((int64_t)s->max_ctx << 32);
    CALL(s, K_ATTN, np * n_head,
         a_int(dims0), a_int(dims1), a_int(kv_info), a_int(s->pos),
         a_f32v(s->q_buf, 0, np * n_q),
         a_f32v(s->r_buf, 0, np * n_head * n_rel),
         a_tensor_f32(s, &L->attn_rel_bias),
         a_f32v(s->sinks_buf, 0, np * n_head),
         a_f32v(s->gate_buf, 0, np * n_q),
         a_u16v(s->kv_k[il], 0, kv_cap * n_kv),
         a_u16v(s->kv_v[il], 0, kv_cap * n_kv),
         a_f32v(s->attn_scores, 0, np * n_head * s->max_ctx),
         a_f32v(s->attn_out, 0, np * n_q),
         a_int((L->attn_q_clip.present ? 1 : 0) | (L->attn_rel_clip.present ? 2 : 0)),
         a_f32(hp->qk_clip), a_f32(hp->rel_clip), a_f32(tau_alpha), a_f32(tau_floor),
         L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64),
         L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64));

    /* Output projection + post shortconv. */
    if (dispatch_mv(s, &L->attn_output, np, d, n_q, 0, s->attn_out, 0, s->o_raw, 0)) return 1;
    /* The post shortconv and the residual add; a block's first layer starts
       a new residual stream and keeps the old one as the block's entry. */
    int block_start = ip % hp->block_size == 0;
    if (block_start && *n_blocks >= INK_MAX_BLOCKS) {
        snprintf(s->error, sizeof s->error, "block count overflow");
        return 1;
    }
    CALL(s, K_ATTN_POST, d, a_int(d), a_int(np), a_int(block_start),
         a_f32v(s->o_raw, 0, nd), a_tensor_f32(s, &L->attn_post_conv), a_f32v(s->conv_ao[il], 0, d * 3),
         a_f32v(s->cur, 0, nd),
         block_start ? a_f32v(s->blocks, (int64_t)(*n_blocks) * stack, nd) : a_f32v(s->dummy_f32, 0, 64));
    if (block_start) (*n_blocks)++;

    /* MoE input: mix -> optional control -> RMSNorm. */
    CALL(s, K_MIX, np, a_int(d), a_int(*n_blocks), a_int(1), a_int(stack),
         a_f32v(s->blocks, 0, INK_MAX_BLOCKS * stack),
         a_f32v(s->cur, 0, nd),
         a_tensor_f32(s, &L->ffn_res_query),
         a_f32v(s->norm_b2, 0, nd));
    PcBuffer ffn_src = s->norm_b2;
    if (has_messages) {
        if (dispatch_control(s, L, 3, 1, 0, np, s->norm_b2, s->norm_buf)) return 1;
        ffn_src = s->norm_buf;
    }
    CALL(s, K_RMSN, np, a_int(d), a_int(np), a_int(stack), a_int(0), a_f32(hp->rms_eps),
         a_int(arena_at(s, &L->ffn_norm)), a_int(0), a_int(0), a_arena(s),
         a_f32v(ffn_src, 0, nd), a_f32v(s->norm_buf, 0, nd));

    /* MoE router, then the chosen experts onto the device. */
    int64_t n_used = hp->n_expert_used;
    if (dispatch_mv(s, &L->ffn_gate_inp, np, hp->n_expert, d, 0, s->norm_buf, 0, s->router_logits, 0)) return 1;
    CALL(s, K_ROUTER, np, a_int(hp->n_expert), a_int(n_used),
         a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
         a_f32v(s->router_logits, 0, np * hp->n_expert),
         L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
         a_u32v(s->moe_ids, 0, np * n_used),
         a_f32v(s->moe_wts, 0, np * n_used));
    PcBuffer ids = s->moe_ids;
    if (stream_experts(s, ip, (int)(np * n_used), &ids)) return 1;
    const ExpertCache *c = &s->experts[ip];
    uint32_t gu_type = L->ffn_gate_up_exps.type, dn_type = L->ffn_down_exps.type;
    if ((gu_type != INK_GGML_IQ4_NL && gu_type != INK_GGML_Q8_0) || (dn_type != INK_GGML_IQ4_NL && dn_type != INK_GGML_Q8_0)) {
        snprintf(s->error, sizeof s->error, "unsupported expert types %u and %u", gu_type, dn_type);
        return 1;
    }

    /* MoE gate_up + SITU, then down + weighted sum, then the post shortconv
       and the residual add. */
    int64_t n_ff = hp->n_ff_exp;
    int64_t gu_q8 = gu_type == INK_GGML_Q8_0, dn_q8 = dn_type == INK_GGML_Q8_0;
    int64_t gu_words = (int64_t)((size_t)c->cap * c->gu_bytes / 4), dn_words = (int64_t)((size_t)c->cap * c->dn_bytes / 4);
    if (!gu_q8 && !dn_q8 && !s->oracle_q8 && !s->no_tables && d % 32 == 0 && n_ff % 32 == 0) {
        /* IQ4_NL experts: the table kernels, a token at a time. */
        int64_t gu_n = n_used * (d / 32) * 2 * n_ff, n_parts = n_used * (n_ff / 32);
        for (int64_t tok = 0; tok < np; tok++) {
            CALL(s, K_MOE_GU_T, n_used * (d / 32), a_int(n_ff), a_int(d),
                 a_view(c->gu, 0, gu_words), a_view(c->gu, 0, gu_words * 2),
                 a_f32v(s->norm_buf, tok * d, d), a_u32v(ids, tok * n_used, n_used), a_f32v(s->gu_part, 0, gu_n));
            CALL(s, K_MOE_DOWN_T, n_parts, a_int(d), a_int(n_ff), a_int(d),
                 a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
                 a_view(c->dn, 0, dn_words), a_view(c->dn, 0, dn_words * 2),
                 a_f32v(s->gu_part, 0, gu_n), a_u32v(ids, tok * n_used, n_used), a_f32v(s->moe_wts, tok * n_used, n_used),
                 a_f32v(s->dn_part, 0, n_parts * d));
            CALL(s, K_MOE_SUM, (d + 31) / 32, a_int(d), a_int(n_parts),
                 a_f32v(s->dn_part, 0, n_parts * d), a_f32v(s->moe_out, tok * d, d));
        }
    } else {
        PcBuffer moe_x = s->norm_buf;
        if (s->oracle_q8) {
            CALL(s, K_Q8_ROUND, nd / 32, a_int(nd), a_f32v(s->norm_buf, 0, nd), a_f32v(s->oracle_x, 0, nd));
            moe_x = s->oracle_x;
        }
        CALL(s, K_MOE_GU, np * n_used * ((n_ff + 15) / 16),
             a_int(n_ff), a_int(d), a_int(n_used), a_int(gu_q8),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_view(c->gu, 0, (int64_t)((size_t)c->cap * c->gu_bytes / 4)),
             a_view(c->gu, 0, (int64_t)((size_t)c->cap * c->gu_bytes / 2)),
             a_f32v(moe_x, 0, nd),
             a_u32v(ids, 0, np * n_used),
             a_f32v(s->moe_hid, 0, np * n_used * n_ff));
        if (s->oracle_q8) {
            int64_t n = np * n_used * n_ff;
            CALL(s, K_Q8_ROUND, n / 32, a_int(n), a_f32v(s->moe_hid, 0, n), a_f32v(s->moe_hid, 0, n));
        }
        CALL(s, K_MOE_DOWN, np * ((d + 31) / 32),
             a_int(d), a_int(n_ff), a_int(n_used), a_int(dn_q8),
             a_view(c->dn, 0, (int64_t)((size_t)c->cap * c->dn_bytes / 4)),
             a_view(c->dn, 0, (int64_t)((size_t)c->cap * c->dn_bytes / 2)),
             a_f32v(s->moe_hid, 0, np * n_used * n_ff),
             a_u32v(ids, 0, np * n_used),
             a_f32v(s->moe_wts, 0, np * n_used),
             a_f32v(s->moe_out, 0, nd));
    }
    CALL(s, K_MOE_POST, d, a_int(d), a_int(np), a_f32v(s->moe_out, 0, nd),
         a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
         a_f32v(s->cur, 0, nd), a_f32v(s->cur, 0, nd));
    return 0;
}

static char *read_entire_file(const char *path, size_t *out_len) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (n <= 0) { fclose(f); return NULL; }
    char *buf = (char *)malloc((size_t)n + 1);
    if (!buf) { fclose(f); return NULL; }
    if (fread(buf, 1, (size_t)n, f) != (size_t)n) { free(buf); fclose(f); return NULL; }
    buf[n] = '\0';
    fclose(f);
    *out_len = (size_t)n;
    return buf;
}

/* Device memory that reads as zero until a kernel writes it. */
static int alloc_zero(InkSession *s, PcBuffer *b, size_t bytes) {
    bytes = (bytes + 3) & ~(size_t)3;
    if (bytes < 256) bytes = 256;
    if (pc_alloc(bytes, b) != PC_OK) {
        snprintf(s->error, sizeof s->error, "out of VRAM allocating %zu bytes: %s", bytes, pc_log());
        return 1;
    }
    if (pc_fill(*b, 0, bytes / 4, s->stream) != PC_OK) return host_fail(s, "clear");
    return 0;
}

static void free_buf(PcBuffer *b) { if (*b) { pc_free(*b); *b = 0; } }

/* Copy host bytes to the device in pieces, so the driver's staging stays small. */
static int upload_big(InkSession *s, PcBuffer dst, const uint8_t *src, size_t bytes) {
    const size_t piece = (size_t)64 << 20;
    for (size_t at = 0; at < bytes; at += piece) {
        size_t n = bytes - at < piece ? bytes - at : piece;
        if (pc_upload(dst + at, src + at, n, s->stream) != PC_OK) return host_fail(s, "upload weights");
    }
    return 0;
}

static int is_expert_tensor(const InkLayerTensors *L, const InkTensor *t) {
    return t == &L->ffn_gate_up_exps || t == &L->ffn_down_exps;
}

static int64_t env_mb(const char *name) {
    const char *v = getenv(name);
    if (!v || !*v) return -1;
    long long n = atoll(v);
    return n > 0 ? (int64_t)n : -1;
}

static int64_t to_mb(size_t bytes) { return (int64_t)((bytes + 1048575u) / 1048576u); }

int ink_open(const char *gguf_path, const char *kernel_path, int max_ctx, InkSession **out, char *err, size_t err_bytes) {
    return ink_open_ex(gguf_path, kernel_path, max_ctx, 0, out, err, err_bytes);
}

int ink_open_ex(const char *gguf_path, const char *kernel_path, int max_ctx,
                int loop_count_override, InkSession **out, char *err, size_t err_bytes) {
    *out = NULL;
    InkSession *s = (InkSession *)calloc(1, sizeof *s);
    if (!s) { snprintf(err, err_bytes, "out of memory"); return 1; }

    if (ink_gguf_open(gguf_path, &s->gguf, err, err_bytes) != 0) { free(s); return 1; }
    InkHParams *hp = &s->gguf.hp;
    if (loop_count_override < 0 || loop_count_override > 16) {
        snprintf(err, err_bytes, "invalid loop_count_override %d (expected 0 or 1..16)", loop_count_override);
        ink_close(s); return 1;
    }
    if (loop_count_override > 0) {
        hp->loop_count = loop_count_override;
        hp->n_slot = hp->n_phys + (hp->loop_count > 1 ? (hp->loop_count - 1) * (hp->loop_end - hp->loop_start) : 0);
    }
    if (max_ctx <= 0) max_ctx = 4096;
    if (max_ctx > 32768) max_ctx = 32768;
    s->max_ctx = max_ctx;
    s->n_slots = (int)(hp->n_phys + (hp->loop_count - 1) * (hp->loop_end - hp->loop_start));
    if (s->n_slots > INK_MAX_SLOTS || hp->n_phys > 32) {
        snprintf(err, err_bytes, "%d logical layers over %lld physical ones is more than this engine holds", s->n_slots, (long long)hp->n_phys);
        ink_close(s); return 1;
    }
    if (hp->n_expert > INK_MAX_EXPERTS || hp->n_expert_used > 16 || hp->n_expert_used > INK_MIN_EXPERT_SLOTS / 2) {
        snprintf(err, err_bytes, "%lld experts with %lld used is more than this engine holds", (long long)hp->n_expert, (long long)hp->n_expert_used);
        ink_close(s); return 1;
    }
    s->profile = getenv("INK_PROFILE") != NULL;
    if (hp->n_embd > 2304) {
        snprintf(err, err_bytes, "this engine handles a width of at most 2304, not %lld", (long long)hp->n_embd);
        ink_close(s); return 1;
    }
    if (hp->gate_rank != 64) {
        snprintf(err, err_bytes, "this engine handles a recurrence gate of rank 64, not %lld", (long long)hp->gate_rank);
        ink_close(s); return 1;
    }
    s->oracle_q8 = getenv("INK_ORACLE_Q8") != NULL;
    s->no_tables = getenv("INK_NO_TABLES") != NULL;

    PcDeviceInfo info;
    PcStatus opened = pc_open(&info);
    if (opened != PC_OK) {
        snprintf(err, err_bytes, opened == PC_ABSENT ? "no NVIDIA GPU: %s" : "CUDA: %s", pc_log());
        ink_close(s); return 1;
    }
    s->device_open = 1;
    if (pc_stream(&s->stream) != PC_OK) { snprintf(err, err_bytes, "CUDA stream: %s", pc_log()); ink_close(s); return 1; }

    /* ---- The memory plan, before any large allocation. ---- */
    size_t d = (size_t)hp->n_embd;
    size_t n_q = (size_t)hp->n_head * (size_t)hp->head_dim;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    size_t resident_bytes = 0, expert_bytes = 0, smallest_cache = 0;
    for (int ip = 0; ip < hp->n_phys; ip++) {
        const InkLayerTensors *L = &s->gguf.layers[ip];
        const InkTensor *t = (const InkTensor *)L;
        for (int i = 0; i < INK_LAYER_TENSORS; i++) {
            if (!t[i].present || is_expert_tensor(L, &t[i])) continue;
            resident_bytes += (t[i].nbytes + 255) & ~(uint64_t)255;
        }
        ExpertCache *c = &s->experts[ip];
        if (!L->ffn_gate_up_exps.present || !L->ffn_down_exps.present) {
            snprintf(err, err_bytes, "layer %d has no expert tensors", ip);
            ink_close(s); return 1;
        }
        c->gu_bytes = (size_t)(L->ffn_gate_up_exps.nbytes / (uint64_t)hp->n_expert);
        c->dn_bytes = (size_t)(L->ffn_down_exps.nbytes / (uint64_t)hp->n_expert);
        expert_bytes += (c->gu_bytes + c->dn_bytes) * (size_t)hp->n_expert;
        smallest_cache += (c->gu_bytes + c->dn_bytes) * INK_MIN_EXPERT_SLOTS;
    }
    resident_bytes += ((s->gguf.out_res_query.nbytes + 255) & ~(uint64_t)255) + ((s->gguf.out_norm.nbytes + 255) & ~(uint64_t)255) +
                      ((s->gguf.output.nbytes + 255) & ~(uint64_t)255) + ((s->gguf.tok_norm.nbytes + 255) & ~(uint64_t)255);

    int full_slots = 0, swa_slots = 0;
    size_t kv_bytes = 0, kv_full_per_token = 0, kv_fixed = 0;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : (int)(hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start));
        /* A window slot is a ring. It holds the window and a whole batch, so
           storing a batch never overwrites a position the batch still reads. */
        int ring = (int)hp->swa + INK_MAX_BATCH;
        int cap = hp->is_swa[ip] ? (ring < s->max_ctx ? ring : s->max_ctx) : s->max_ctx;
        s->kv_cap[il] = cap;
        size_t conv = (2 * n_kv + 2 * d) * 3 * 4;
        kv_bytes += (size_t)cap * n_kv * 4 + conv;
        if (hp->is_swa[ip]) { swa_slots++; kv_fixed += (size_t)ring * n_kv * 4 + conv; }
        else { full_slots++; kv_full_per_token += n_kv * 4; kv_fixed += conv; }
    }
    size_t scratch_bytes = (size_t)hp->n_vocab * 4 + ((size_t)8 << 20) + (size_t)INK_MAX_BATCH * 4 *
                           ((size_t)hp->n_head * (size_t)s->max_ctx + 48 * d + 5 * n_q + 6 * n_kv +
                            (size_t)hp->n_expert_used * (size_t)hp->n_ff_exp + 4096);

    size_t free_b = info.free_bytes, total_b = info.total_bytes;
    int64_t test_mb = env_mb("INK_VRAM_MB"); /* plan as if this much VRAM were free */
    if (test_mb > 0) { free_b = (size_t)test_mb << 20; if (total_b < free_b) total_b = free_b; }
    size_t margin = (size_t)INK_VRAM_MARGIN_MB << 20;
    size_t fixed = resident_bytes + kv_bytes + scratch_bytes + margin;
    int64_t forced_slots = env_mb("INK_EXPERT_SLOTS"); /* cap every layer's cache, to exercise streaming */

    /* The plan of psionic/lib/inkling-budget.coax (inkling_vram_plan), in the
       same integer arithmetic: one slot is one expert in every layer. */
    size_t slot_bytes = expert_bytes / (size_t)hp->n_expert;
    size_t expert_budget = free_b > fixed ? free_b - fixed : 0;
    int slots = (int)hp->n_expert;
    if (expert_budget / slot_bytes < (size_t)hp->n_expert) {
        slots = (int)(expert_budget / slot_bytes);
        s->streaming = 1;
    }
    if (forced_slots > 0 && forced_slots < slots) { slots = (int)forced_slots; s->streaming = 1; }
    if (slots > hp->n_expert) slots = (int)hp->n_expert;
    size_t cache_bytes = 0;
    for (int ip = 0; ip < hp->n_phys; ip++) {
        ExpertCache *c = &s->experts[ip];
        c->cap = slots;
        cache_bytes += (c->gu_bytes + c->dn_bytes) * (size_t)(slots > 0 ? slots : 0);
    }
    size_t planned = fixed + cache_bytes;
    int64_t ctx_fit = 0;
    {
        size_t least = resident_bytes + scratch_bytes + margin + kv_fixed + smallest_cache;
        if (free_b > least && kv_full_per_token > 0) ctx_fit = (int64_t)((free_b - least) / kv_full_per_token);
        if (ctx_fit > 32768) ctx_fit = 32768;
    }
    s->budget = (InkMemoryBudget){
        .device_ram_mb = to_mb(total_b),
        .os_reserve_mb = to_mb(total_b - free_b) + INK_VRAM_MARGIN_MB,
        .weights_mb = to_mb(resident_bytes + cache_bytes),
        .kv_cache_mb = to_mb(kv_bytes),
        .scratchpad_mb = to_mb(scratch_bytes),
        .total_mb = to_mb(planned) + to_mb(total_b - free_b),
        .headroom_mb = free_b > planned ? to_mb(free_b - planned) : 0,
        .max_context_len = ctx_fit,
        .full_slots = full_slots,
        .swa_slots = swa_slots,
    };
    if (slots < INK_MIN_EXPERT_SLOTS) {
        snprintf(err, err_bytes,
                 "insufficient VRAM: %lld MB free of %lld MB on %s; Inkling 2 Mini at context %d needs %lld MB "
                 "with the smallest expert cache (%lld MB with every expert in VRAM); largest context that fits: %lld",
                 (long long)to_mb(free_b), (long long)to_mb(total_b), info.name, s->max_ctx,
                 (long long)to_mb(fixed + smallest_cache), (long long)to_mb(fixed + expert_bytes), (long long)ctx_fit);
        ink_close(s); return 1;
    }
    fprintf(stderr, "psionic-cuda: %s, %lld MB VRAM free of %lld MB; plan %lld MB (weights %lld, KV %lld, scratch %lld)\n",
            info.name, (long long)to_mb(free_b), (long long)to_mb(total_b), (long long)to_mb(planned),
            (long long)to_mb(resident_bytes + cache_bytes), (long long)to_mb(kv_bytes), (long long)to_mb(scratch_bytes));
    /* A batch's experts must all be in a layer's cache at once. */
    s->batch = slots / (int)hp->n_expert_used;
    if (s->batch > INK_MAX_BATCH) s->batch = INK_MAX_BATCH;
    int64_t forced_batch = env_mb("INK_BATCH"); /* a smaller prefill batch, for tests */
    if (forced_batch > 0 && forced_batch < s->batch) s->batch = (int)forced_batch;
    if (s->streaming)
        fprintf(stderr, "psionic-cuda: experts do not all fit in VRAM: %d of %lld per layer cached (%lld of %lld MB), the rest stream from host memory\n",
                slots, (long long)hp->n_expert, (long long)to_mb(cache_bytes), (long long)to_mb(expert_bytes));

    /* ---- Kernels. ---- */
    size_t src_len = 0;
    char *src = read_entire_file(kernel_path, &src_len);
    if (!src) {
        snprintf(err, err_bytes, "cannot read kernel source %s", kernel_path);
        ink_close(s); return 1;
    }
    double compile_s = 0.0;
    PcStatus cs = pc_compile(src, src_len, "inkling", &s->module, &compile_s);
    free(src);
    if (cs != PC_OK) {
        snprintf(err, err_bytes, "compile %s: %s", kernel_path, pc_log());
        ink_close(s); return 1;
    }
    for (int k = 0; k < K_COUNT; k++) {
        if (pc_kernel(s->module, k_names[k], &s->kernels[k]) != PC_OK) {
            snprintf(err, err_bytes, "kernel %s: %s", k_names[k], pc_log());
            ink_close(s); return 1;
        }
    }

    /* ---- Resident weights. ---- */
    s->weights_bytes = resident_bytes;
    if (pc_alloc(resident_bytes, &s->weights) != PC_OK) {
        snprintf(err, err_bytes, "out of VRAM for %lld MB of weights: %s", (long long)to_mb(resident_bytes), pc_log());
        ink_close(s); return 1;
    }
    s->stage_bytes = (size_t)32 << 20;
    for (int ip = 0; ip < hp->n_phys; ip++)
        if (s->experts[ip].gu_bytes > s->stage_bytes) s->stage_bytes = s->experts[ip].gu_bytes;
    if (pc_alloc(s->stage_bytes, &s->stage) != PC_OK) {
        snprintf(err, err_bytes, "out of VRAM for the staging buffer: %s", pc_log());
        ink_close(s); return 1;
    }
    size_t at = 0;
#define PLACE(tensor, dev) do { \
        (dev) = s->weights + at; \
        int quant_ = (tensor)->type == INK_GGML_Q8_0 || (tensor)->type == INK_GGML_IQ4_NL; \
        int64_t blocks_ = (int64_t)(tensor)->dims[0] / 32, words_ = quant_words((tensor)->type); \
        if (quant_ && ((tensor)->ndims != 2 || (tensor)->dims[0] % 32 != 0 || \
                       (tensor)->nbytes != (tensor)->dims[1] * (uint64_t)(blocks_ * (4 * words_ + 2)))) { \
            snprintf(err, err_bytes, "a quantized tensor is not a matrix of whole blocks"); ink_close(s); return 1; \
        } \
        if (quant_ ? relayout(s, s->gguf.map + (tensor)->offset, (dev), (int64_t)(tensor)->dims[1], blocks_, words_, 0) \
                   : upload_big(s, (dev), s->gguf.map + (tensor)->offset, (size_t)(tensor)->nbytes)) { snprintf(err, err_bytes, "%s", s->error); ink_close(s); return 1; } \
        at += ((size_t)(tensor)->nbytes + 255) & ~(size_t)255; \
    } while (0)
    for (int ip = 0; ip < hp->n_phys; ip++) {
        const InkLayerTensors *L = &s->gguf.layers[ip];
        const InkTensor *t = (const InkTensor *)L;
        for (int i = 0; i < INK_LAYER_TENSORS; i++) {
            if (!t[i].present || is_expert_tensor(L, &t[i])) continue;
            PLACE(&t[i], s->layer_dev[ip][i]);
        }
    }
    PLACE(&s->gguf.out_res_query, s->out_res_query_dev);
    PLACE(&s->gguf.out_norm, s->out_norm_dev);
    PLACE(&s->gguf.output, s->output_dev);
    PLACE(&s->gguf.tok_norm, s->tok_norm_dev);
#undef PLACE

    /* ---- Expert caches. ---- */
    for (int ip = 0; ip < hp->n_phys; ip++) {
        ExpertCache *c = &s->experts[ip];
        if (pc_alloc((size_t)c->cap * c->gu_bytes, &c->gu) != PC_OK || pc_alloc((size_t)c->cap * c->dn_bytes, &c->dn) != PC_OK) {
            snprintf(err, err_bytes, "out of VRAM for layer %d's expert cache: %s", ip, pc_log());
            ink_close(s); return 1;
        }
        for (int e = 0; e < INK_MAX_EXPERTS; e++) { c->slot_of[e] = -1; c->expert_in[e] = -1; c->used[e] = 0; }
        /* Fill the cache with the first cap experts: all of them when the
           layer is resident, and otherwise a start that spares the first
           tokens a miss on nearly every expert they choose. */
        for (int e = 0; e < c->cap; e++) {
            if (load_expert(s, ip, e, e)) {
                snprintf(err, err_bytes, "%s", s->error);
                ink_close(s); return 1;
            }
            c->slot_of[e] = (int16_t)e; c->expert_in[e] = (int16_t)e;
        }
    }

    /* ---- Scratch. ---- */
    size_t row_bytes = (size_t)(s->gguf.tok_embd.nbytes / (uint64_t)hp->n_vocab);
    size_t nb = (size_t)s->batch;
    size_t n_used = (size_t)hp->n_expert_used;
    size_t widest = 2 * d > n_q ? 2 * d : n_q;
    size_t n_rel_all = (size_t)hp->n_head * (size_t)hp->rel_len, n_dyn_all = (size_t)hp->n_head_kv * (size_t)hp->conv_len;
    size_t proj_rows = 2 * n_q + 2 * n_kv + 2 * n_dyn_all + n_rel_all + (size_t)hp->n_head;
    if (alloc_zero(s, &s->embd_row, nb * row_bytes) ||
        alloc_zero(s, &s->cur, nb * d * 4) ||
        alloc_zero(s, &s->prelude, nb * d * 4) ||
        alloc_zero(s, &s->bank_out, nb * d * 4) ||
        alloc_zero(s, &s->blocks, INK_MAX_BLOCKS * nb * d * 4) ||
        alloc_zero(s, &s->prelude_blk, INK_MAX_BLOCKS * nb * d * 4) ||
        alloc_zero(s, &s->bank_blk, INK_MAX_BLOCKS * nb * d * 4) ||
        alloc_zero(s, &s->inp3, 3 * nb * d * 4) ||
        alloc_zero(s, &s->inp3b, 3 * nb * d * 4) ||
        alloc_zero(s, &s->norm_b2, nb * d * 4) ||
        alloc_zero(s, &s->messages, 4 * nb * d * 4) ||
        alloc_zero(s, &s->ctrl_gate, 3 * nb * (size_t)hp->gate_rank * 4) ||
        alloc_zero(s, &s->k_buf, nb * n_kv * 4) ||
        alloc_zero(s, &s->v_buf, nb * n_kv * 4) ||
        alloc_zero(s, &s->attn_scores, nb * (size_t)hp->n_head * (size_t)s->max_ctx * 4) ||
        alloc_zero(s, &s->attn_out, nb * n_q * 4) ||
        alloc_zero(s, &s->o_raw, nb * d * 4) ||
        alloc_zero(s, &s->norm_buf, nb * d * 4) ||
        alloc_zero(s, &s->router_logits, nb * (size_t)hp->n_expert * 4) ||
        alloc_zero(s, &s->moe_ids, nb * n_used * 4) ||
        alloc_zero(s, &s->moe_slots, nb * n_used * 4) ||
        alloc_zero(s, &s->moe_wts, nb * n_used * 4) ||
        alloc_zero(s, &s->moe_hid, nb * n_used * (size_t)hp->n_ff_exp * 4) ||
        alloc_zero(s, &s->moe_out, nb * d * 4) ||
        alloc_zero(s, &s->mv_part, (s->mv_part_floats = (n_q > d ? n_q : d) * ((n_q > d ? n_q : d) / 32) + proj_rows * (d / 32)) * 4) ||
        alloc_zero(s, &s->proj, (s->proj_floats = nb * proj_rows) * 4) ||
        alloc_zero(s, &s->proj_desc, 32 * 2 * 80 * 4) ||
        alloc_zero(s, &s->gu_part, n_used * (d / 32) * 2 * (size_t)hp->n_ff_exp * 4) ||
        alloc_zero(s, &s->dn_part, n_used * ((size_t)hp->n_ff_exp / 32) * d * 4) ||
        alloc_zero(s, &s->logits, (size_t)hp->n_vocab * 4) ||
        alloc_zero(s, &s->oracle_x, nb * widest * 4) ||
        alloc_zero(s, &s->dummy_f32, 64 * 4)) {
        snprintf(err, err_bytes, "%s", s->error);
        ink_close(s); return 1;
    }
    for (int il = 0; il < s->n_slots; il++) {
        size_t cap = (size_t)s->kv_cap[il];
        if (alloc_zero(s, &s->kv_k[il], cap * n_kv * 2) ||
            alloc_zero(s, &s->kv_v[il], cap * n_kv * 2) ||
            alloc_zero(s, &s->conv_k[il], n_kv * 3 * 4) ||
            alloc_zero(s, &s->conv_v[il], n_kv * 3 * 4) ||
            alloc_zero(s, &s->conv_ao[il], d * 3 * 4) ||
            alloc_zero(s, &s->conv_fo[il], d * 3 * 4)) {
            snprintf(err, err_bytes, "%s", s->error);
            ink_close(s); return 1;
        }
    }
    /* The projections' outputs, side by side in s->proj. */
    {
        size_t at_f = 0;
        s->q_buf = s->proj + at_f * 4;     at_f += nb * n_q;
        s->k_raw = s->proj + at_f * 4;     at_f += nb * n_kv;
        s->kdyn_buf = s->proj + at_f * 4;  at_f += nb * n_dyn_all;
        s->v_raw = s->proj + at_f * 4;     at_f += nb * n_kv;
        s->vdyn_buf = s->proj + at_f * 4;  at_f += nb * n_dyn_all;
        s->r_buf = s->proj + at_f * 4;     at_f += nb * n_rel_all;
        s->sinks_buf = s->proj + at_f * 4; at_f += nb * (size_t)hp->n_head;
        s->gate_buf = s->proj + at_f * 4;
    }
    /* Each layer's list for the fused projection launch. */
    for (int ip = 0; ip < hp->n_phys; ip++) {
        const InkLayerTensors *L = &s->gguf.layers[ip];
        int dyn = !hp->is_swa[ip];
        struct { const InkTensor *t; int sel; PcBuffer out; int use; } mats[8] = {
            {&L->attn_q, 0, s->q_buf, 1},
            {&L->attn_k, 1, s->k_raw, 1},
            {&L->attn_k_conv_proj, 1, s->kdyn_buf, dyn && L->attn_k_conv_proj.present},
            {&L->attn_v, 2, s->v_raw, 1},
            {&L->attn_v_conv_proj, 2, s->vdyn_buf, dyn && L->attn_v_conv_proj.present},
            {&L->attn_rel, 0, s->r_buf, 1},
            {&L->attn_sink_proj, 0, s->sinks_buf, 1},
            {&L->attn_gate, 0, s->gate_buf, 1},
        };
        for (int variant = 0; variant < 2; variant++) {
            uint32_t desc[80] = {0};
            int n = 0, ok = 1;
            int64_t items = 0, rows_all = 0, blocks = (int64_t)d / 32;
            for (int m = 0; m < 8 && ok; m++) {
                const InkTensor *t = mats[m].t;
                if (!mats[m].use) continue;
                if (!t->present || t->type != INK_GGML_Q8_0 || t->ndims != 2 || t->dims[0] != d || t->dims[1] % 4 != 0) { ok = 0; break; }
                int64_t rows = (int64_t)t->dims[1];
                PcBuffer dev = tensor_dev(s, t);
                uint32_t *e = desc + n * 10;
                e[0] = (uint32_t)((dev - s->weights) / 4);
                e[1] = (uint32_t)((dev - s->weights + (uint64_t)(rows * blocks * 32)) / 2);
                e[2] = (uint32_t)rows;
                e[3] = (uint32_t)rows;
                e[4] = (uint32_t)(variant ? (size_t)mats[m].sel * nb * d : 0);
                e[5] = (uint32_t)((mats[m].out - s->proj) / 4);
                e[6] = (uint32_t)(rows_all * blocks);
                items += blocks * (rows / 4);
                rows_all += rows;
                e[7] = (uint32_t)items;
                e[8] = (uint32_t)rows_all;
                n++;
            }
            if (!ok || (size_t)(rows_all * blocks) > s->mv_part_floats) n = 0;
            s->proj_list[ip][variant].n_mats = n;
            s->proj_list[ip][variant].items = items;
            s->proj_list[ip][variant].rows = rows_all;
            if (pc_upload(s->proj_desc + (uint64_t)(ip * 2 + variant) * 80 * 4, desc, sizeof desc, s->stream) != PC_OK ||
                pc_sync_stream(s->stream) != PC_OK) {
                snprintf(err, err_bytes, "upload the projection lists: %s", pc_log());
                ink_close(s); return 1;
            }
        }
    }
    size_t zeros = (n_kv > d ? n_kv : d) * 3;
    s->logits_host = (float *)calloc((size_t)hp->n_vocab, sizeof(float));
    s->hidden_host = (float *)calloc(d, sizeof(float));
    s->zeros = (float *)calloc(zeros, sizeof(float));
    if (!s->logits_host || !s->hidden_host || !s->zeros) {
        snprintf(err, err_bytes, "out of memory");
        ink_close(s); return 1;
    }
    if (pc_sync_stream(s->stream) != PC_OK) {
        snprintf(err, err_bytes, "loading weights: %s", pc_log());
        ink_close(s); return 1;
    }
    s->pos = 0;
    if (getenv("INK_LAUNCH_BENCH")) {
        /* What an empty launch costs: queued back to back, and with a wait after each. */
        PcArg args[] = { a_int(1), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->dummy_f32, 0, 64) };
        pc_sync_stream(s->stream);
        double t0 = pc_seconds();
        for (int i = 0; i < 20000; i++) pc_launch(s->kernels[K_COPY], 1, 256, args, 3, s->stream);
        double t1 = pc_seconds();
        pc_sync_stream(s->stream);
        double t2 = pc_seconds();
        for (int i = 0; i < 20000; i++) { pc_launch(s->kernels[K_COPY], 1, 256, args, 3, s->stream); pc_sync_stream(s->stream); }
        double t3 = pc_seconds();
        uint32_t tmp[16];
        for (int i = 0; i < 20000; i++) {
            pc_launch(s->kernels[K_COPY], 1, 256, args, 3, s->stream);
            pc_download(tmp, s->moe_ids, 32, s->stream); pc_sync_stream(s->stream);
            pc_upload(s->moe_slots, tmp, 32, s->stream);
        }
        double t4 = pc_seconds();
        fprintf(stderr, "psionic-cuda: launch + 32-byte download + wait + 32-byte upload: %.2f us\n", (t4 - t3) / 20000 * 1e6);
        fprintf(stderr, "psionic-cuda: an empty launch: %.2f us to queue, %.2f us queued and run back to back, %.2f us with a wait after each\n",
                (t1 - t0) / 20000 * 1e6, (t2 - t0) / 20000 * 1e6, (t3 - t2) / 20000 * 1e6);
    }
    *out = s;
    return 0;
}

void ink_close(InkSession *s) {
    if (!s) return;
    if (s->device_open) {
        if (s->stream) pc_sync_stream(s->stream);
        if (s->profile && s->total_steps > 0) {
            double sum = 0.0;
            for (int k = 0; k < K_COUNT; k++) sum += s->prof_seconds[k];
            sum += s->prof_stream_seconds;
            fprintf(stderr, "\n[INK_PROFILE %d steps, %.2f ms a step with a wait after every launch]\n", s->total_steps, 1000.0 * sum / s->total_steps);
            for (int k = 0; k < K_COUNT; k++) {
                if (!s->prof_calls[k]) continue;
                fprintf(stderr, "  %-18s %8.3f ms a step (%4.1f%%) %6.1f calls a step\n", k_names[k],
                        1000.0 * s->prof_seconds[k] / s->total_steps, 100.0 * s->prof_seconds[k] / sum,
                        (double)s->prof_calls[k] / s->total_steps);
            }
            for (size_t i = 0; i < sizeof s->prof_mv / sizeof s->prof_mv[0] && s->prof_mv[i].calls; i++)
                fprintf(stderr, "  matvec type %2u %6lld x %-6lld %8.3f ms a step, %6.1f us a call, %5.1f calls a step\n",
                        s->prof_mv[i].type, (long long)s->prof_mv[i].rows, (long long)s->prof_mv[i].cols,
                        1000.0 * s->prof_mv[i].seconds / s->total_steps, 1e6 * s->prof_mv[i].seconds / (double)s->prof_mv[i].calls,
                        (double)s->prof_mv[i].calls / s->total_steps);
            fprintf(stderr, "  %-18s %8.3f ms a step (%4.1f%%)\n", "expert streaming",
                    1000.0 * s->prof_stream_seconds / s->total_steps, 100.0 * s->prof_stream_seconds / sum);
        }
        if (getenv("INK_PROFILE") || getenv("INK_TIMES"))
        fprintf(stderr, "psionic-cuda: %.3f s queueing work, %.3f s in all on the device path\n", s->queue_seconds, s->total_gpu_seconds);
    if (s->streaming && s->expert_hits + s->expert_misses > 0)
            fprintf(stderr, "psionic-cuda: expert cache %llu hits, %llu misses (%.1f%% hit), %.1f MB uploaded\n",
                    (unsigned long long)s->expert_hits, (unsigned long long)s->expert_misses,
                    100.0 * (double)s->expert_hits / (double)(s->expert_hits + s->expert_misses),
                    (double)s->expert_upload_bytes / 1048576.0);
        for (int il = 0; il < INK_MAX_SLOTS; il++) {
            free_buf(&s->kv_k[il]); free_buf(&s->kv_v[il]);
            free_buf(&s->conv_k[il]); free_buf(&s->conv_v[il]);
            free_buf(&s->conv_ao[il]); free_buf(&s->conv_fo[il]);
        }
        for (int ip = 0; ip < 32; ip++) { free_buf(&s->experts[ip].gu); free_buf(&s->experts[ip].dn); }
        PcBuffer *scratch[] = {
            &s->weights, &s->embd_row, &s->cur, &s->prelude, &s->bank_out,
            &s->blocks, &s->prelude_blk, &s->bank_blk, &s->messages,
            &s->inp3, &s->inp3b, &s->norm_b2, &s->ctrl_gate,
            &s->proj, &s->proj_desc, &s->k_buf, &s->v_buf,
            &s->attn_scores, &s->attn_out, &s->o_raw, &s->norm_buf, &s->router_logits,
            &s->moe_ids, &s->moe_slots, &s->moe_wts, &s->moe_hid, &s->moe_out, &s->gu_part, &s->dn_part, &s->mv_part, &s->logits, &s->dummy_f32, &s->oracle_x,
        };
        for (size_t i = 0; i < sizeof scratch / sizeof *scratch; i++) free_buf(scratch[i]);
        if (s->module) pc_unload(s->module);
        if (s->stream) pc_destroy_stream(s->stream);
    }
    for (int i = 0; i < 2; i++) { free(s->snaps[i].tokens); free(s->snaps[i].conv_state); free(s->snaps[i].logits); }
    free(s->logits_host); free(s->hidden_host); free(s->zeros);
    ink_gguf_close(&s->gguf);
    free(s);
}

void ink_reset(InkSession *s) {
    if (!s) return;
    const InkHParams *hp = &s->gguf.hp;
    size_t d = (size_t)hp->n_embd;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    for (int il = 0; il < s->n_slots; il++) {
        pc_fill(s->conv_k[il], 0, n_kv * 3, s->stream);
        pc_fill(s->conv_v[il], 0, n_kv * 3, s->stream);
        pc_fill(s->conv_ao[il], 0, d * 3, s->stream);
        pc_fill(s->conv_fo[il], 0, d * 3, s->stream);
    }
    s->pos = 0;
    s->max_pos_written = 0;
    s->snaps[0].valid = 0;
    s->snaps[1].valid = 0;
}

/* The exact shortconv state after n_tokens, so a later request that starts
   with the same tokens resumes there. The KV cache stays on the device:
   positions before the snapshot are not rewritten until the ring wraps. */
void ink_prefix_save(InkSession *s, const int32_t *tokens, int n_tokens, int save_logits) {
    if (!s || !tokens || n_tokens <= 0 || n_tokens != s->pos || n_tokens > s->max_ctx) return;
    const InkHParams *hp = &s->gguf.hp;
    size_t d = (size_t)hp->n_embd;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    size_t conv_elems = (size_t)s->n_slots * (2 * n_kv + 2 * d) * 3;
    int slot = save_logits ? 0 : 1;
    if (!s->snaps[slot].tokens) {
        s->snaps[slot].tokens = (int32_t *)malloc((size_t)s->max_ctx * sizeof(int32_t));
        s->snaps[slot].conv_state = (float *)malloc(conv_elems * sizeof(float));
        s->snaps[slot].logits = (float *)malloc((size_t)hp->n_vocab * sizeof(float));
        if (!s->snaps[slot].tokens || !s->snaps[slot].conv_state || !s->snaps[slot].logits) {
            s->snaps[slot].valid = 0;
            return;
        }
    }
    memcpy(s->snaps[slot].tokens, tokens, (size_t)n_tokens * sizeof(int32_t));
    float *dst = s->snaps[slot].conv_state;
    PcStatus st = PC_OK;
    for (int il = 0; il < s->n_slots && st == PC_OK; il++) {
        st = pc_download(dst, s->conv_k[il], n_kv * 3 * 4, s->stream); dst += n_kv * 3;
        if (st == PC_OK) st = pc_download(dst, s->conv_v[il], n_kv * 3 * 4, s->stream);
        dst += n_kv * 3;
        if (st == PC_OK) st = pc_download(dst, s->conv_ao[il], d * 3 * 4, s->stream);
        dst += d * 3;
        if (st == PC_OK) st = pc_download(dst, s->conv_fo[il], d * 3 * 4, s->stream);
        dst += d * 3;
    }
    if (st == PC_OK) st = pc_sync_stream(s->stream);
    if (st != PC_OK) { s->snaps[slot].valid = 0; return; }
    if (save_logits) memcpy(s->snaps[slot].logits, s->logits_host, (size_t)hp->n_vocab * sizeof(float));
    s->snaps[slot].pos = n_tokens;
    s->snaps[slot].has_logits = save_logits ? 1 : 0;
    s->snaps[slot].valid = 1;
}

int ink_prefix_restore(InkSession *s, const int32_t *tokens, int n_tokens) {
    if (!s || !tokens || n_tokens <= 0) { ink_reset(s); return 0; }
    const InkHParams *hp = &s->gguf.hp;
    int swa_cap = s->max_ctx;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : (int)(hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start));
        if (hp->is_swa[ip] && s->kv_cap[il] < swa_cap) swa_cap = s->kv_cap[il];
    }
    int best = -1;
    for (int i = 0; i < 2; i++) {
        if (!s->snaps[i].valid) continue;
        int sp = s->snaps[i].pos;
        if (sp <= 0 || sp > n_tokens) continue;
        if (sp == n_tokens && !s->snaps[i].has_logits) continue;
        /* The window's keys [oldest_needed .. sp-1] must not have been overwritten by later writes. */
        int oldest_needed = (sp >= hp->swa) ? (sp - (int)hp->swa + 1) : 0;
        if (s->max_pos_written - oldest_needed > swa_cap) continue;
        if (memcmp(s->snaps[i].tokens, tokens, (size_t)sp * sizeof(int32_t)) != 0) continue;
        if (best < 0 || sp > s->snaps[best].pos) best = i;
    }
    if (best < 0) { ink_reset(s); return 0; }
    size_t d = (size_t)hp->n_embd;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    const float *src = s->snaps[best].conv_state;
    PcStatus st = PC_OK;
    for (int il = 0; il < s->n_slots && st == PC_OK; il++) {
        st = pc_upload(s->conv_k[il], src, n_kv * 3 * 4, s->stream); src += n_kv * 3;
        if (st == PC_OK) st = pc_upload(s->conv_v[il], src, n_kv * 3 * 4, s->stream);
        src += n_kv * 3;
        if (st == PC_OK) st = pc_upload(s->conv_ao[il], src, d * 3 * 4, s->stream);
        src += d * 3;
        if (st == PC_OK) st = pc_upload(s->conv_fo[il], src, d * 3 * 4, s->stream);
        src += d * 3;
    }
    if (st == PC_OK) st = pc_sync_stream(s->stream);
    if (st != PC_OK) { ink_reset(s); return 0; }
    int sp = s->snaps[best].pos;
    if (sp == n_tokens && s->snaps[best].has_logits)
        memcpy(s->logits_host, s->snaps[best].logits, (size_t)hp->n_vocab * sizeof(float));
    s->pos = sp;
    s->max_pos_written = sp;
    for (int i = 0; i < 2; i++) {
        if (i != best && s->snaps[i].valid && s->snaps[i].pos > sp) s->snaps[i].valid = 0;
    }
    return sp;
}

/* Run np tokens (1 .. batch) through the model at positions pos .. pos +
   np - 1, and compute the logits after the last of them when asked. */
static int enqueue_batch(InkSession *s, const int32_t *tokens, int np_tokens, int compute_logits) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t np = np_tokens;
    if (s->pos + np_tokens > s->max_ctx) {
        snprintf(s->error, sizeof s->error, "context full (%d + %d of %d)", s->pos, np_tokens, s->max_ctx);
        return 1;
    }
    int64_t d = hp->n_embd;
    int64_t nd = np * d;
    int64_t stack = (int64_t)s->batch * d;

    /* 1. Embedding: the batch's rows from host memory, then token_embd_norm. */
    size_t row_bytes = (size_t)(s->gguf.tok_embd.nbytes / (uint64_t)hp->n_vocab);
    for (int b = 0; b < np_tokens; b++) {
        if (tokens[b] < 0 || tokens[b] >= hp->n_vocab) {
            snprintf(s->error, sizeof s->error, "token id %d out of range [0,%lld)", tokens[b], (long long)hp->n_vocab);
            return 1;
        }
        if (pc_upload(s->embd_row + (uint64_t)b * row_bytes, s->gguf.map + s->gguf.tok_embd.offset + (size_t)tokens[b] * row_bytes,
                      row_bytes, s->stream) != PC_OK)
            return host_fail(s, "upload embedding rows");
    }
    CALL(s, K_EMBED_Q8, np, a_int(0), a_int(d),
         a_u16v(s->embd_row, 0, np * (int64_t)(row_bytes / 2)),
         a_tensor_f32(s, &s->gguf.tok_norm),
         a_f32(hp->rms_eps),
         a_f32v(s->cur, 0, nd));

    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;

    /* 2. Prelude layers [0 .. start). */
    for (int il = 0; il < start; il++) {
        if (dispatch_layer(s, il, il, np, &n_blocks, 0) != 0) return 1;
    }

    /* Save the prelude's output and blocks. */
    CALL(s, K_COPY, nd, a_int(nd), a_f32v(s->cur, 0, nd), a_f32v(s->prelude, 0, nd));
    int prelude_n_blocks = n_blocks;
    if (prelude_n_blocks > 0) {
        int64_t n = (int64_t)prelude_n_blocks * stack;
        CALL(s, K_COPY, n, a_int(n), a_f32v(s->blocks, 0, n), a_f32v(s->prelude_blk, 0, n));
    }

    /* 3. Looped core: loop_count passes over physical layers [start .. end). */
    for (int pass = 0; pass < hp->loop_count; pass++) {
        int bank_n_blocks = 0;
        if (pass > 0) {
            int src_off = prelude_n_blocks + 1;
            bank_n_blocks = n_blocks - src_off;
            if (bank_n_blocks < 0) bank_n_blocks = 0;
            if (bank_n_blocks > 0) {
                int64_t n = (int64_t)bank_n_blocks * stack;
                CALL(s, K_COPY, n, a_int(n), a_f32v(s->blocks, (int64_t)src_off * stack, n), a_f32v(s->bank_blk, 0, n));
            }
            CALL(s, K_COPY, nd, a_int(nd), a_f32v(s->cur, 0, nd), a_f32v(s->bank_out, 0, nd));
            CALL(s, K_COPY, nd, a_int(nd), a_f32v(s->prelude, 0, nd), a_f32v(s->cur, 0, nd));
            n_blocks = prelude_n_blocks;
            if (prelude_n_blocks > 0) {
                int64_t n = (int64_t)prelude_n_blocks * stack;
                CALL(s, K_COPY, n, a_int(n), a_f32v(s->prelude_blk, 0, n), a_f32v(s->blocks, 0, n));
            }
        }
        for (int ip = start; ip < end; ip++) {
            int il = (pass == 0) ? ip : (int)hp->n_phys + (pass - 1) * (end - start) + (ip - start);
            if (pass > 0) {
                /* The four messages of this layer, from the previous pass. */
                const InkLayerTensors *L = &s->gguf.layers[ip];
                CALL(s, K_MIX, np * 4, a_int(d), a_int(bank_n_blocks), a_int(4), a_int(stack),
                     a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * stack),
                     a_f32v(s->bank_out, 0, nd),
                     a_tensor_f32(s, &L->loop_msg_query),
                     a_f32v(s->messages, 0, 4 * stack));
            }
            if (dispatch_layer(s, il, ip, np, &n_blocks, pass > 0) != 0) return 1;
        }
    }

    /* 4. Coda layers [end .. n_phys). */
    for (int il = end; il < hp->n_phys; il++) {
        if (dispatch_layer(s, il, il, np, &n_blocks, 0) != 0) return 1;
    }

    /* 5. For the last token: output mix -> output_norm -> lm_head -> logit softcap. */
    if (compute_logits) {
        int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
        int64_t last = (np - 1) * d;
        CALL(s, K_MIX, 1, a_int(d), a_int(n_blocks), a_int(1), a_int(stack),
             a_f32v(s->blocks, last, INK_MAX_BLOCKS * stack - last),
             a_f32v(s->cur, last, d),
             a_tensor_f32(s, &s->gguf.out_res_query),
             a_f32v(s->norm_buf, 0, d));
        CALL(s, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->norm_buf, 0, d), a_tensor_f32(s, &s->gguf.out_norm), a_f32v(s->norm_buf, 0, d));
        if (dispatch_mv(s, &s->gguf.output, 1, nv, d, 0, s->norm_buf, 0, s->logits, 0) != 0) return 1;
        if (hp->logit_softcap > 0.0f) {
            CALL(s, K_SOFTCAP, nv, a_int(nv), a_f32(hp->logit_softcap), a_f32v(s->logits, 0, nv));
        }
        if (pc_download(s->logits_host, s->logits, (size_t)nv * 4, s->stream) != PC_OK ||
            pc_download(s->hidden_host, s->norm_buf, (size_t)d * 4, s->stream) != PC_OK)
            return host_fail(s, "read logits");
    }

    s->pos += np_tokens;
    if (s->pos > s->max_pos_written) s->max_pos_written = s->pos;
    s->total_steps += np_tokens;
    return 0;
}

int ink_prefill(InkSession *s, const int32_t *tokens, int n_tokens, int compute_last_logits) {
    if (n_tokens <= 0) return 0;
    double began = pc_seconds();
    for (int at = 0; at < n_tokens; at += s->batch) {
        int np = n_tokens - at < s->batch ? n_tokens - at : s->batch;
        int need_logits = compute_last_logits && at + np == n_tokens;
        if (enqueue_batch(s, tokens + at, np, need_logits) != 0) {
            pc_sync_stream(s->stream);
            return 1;
        }
    }
    double queued = pc_seconds();
    if (pc_sync_stream(s->stream) != PC_OK) return host_fail(s, "device");
    s->total_gpu_seconds += pc_seconds() - began;
    s->queue_seconds += queued - began;
    return 0;
}

int ink_step(InkSession *s, int32_t token_id, int compute_logits) {
    return ink_prefill(s, &token_id, 1, compute_logits);
}

const float *ink_logits(const InkSession *s) { return s ? s->logits_host : NULL; }
const float *ink_hidden(const InkSession *s) { return s ? s->hidden_host : NULL; }
const InkHParams *ink_hparams(const InkSession *s) { return s ? &s->gguf.hp : NULL; }
const InkMemoryBudget *ink_memory_budget(const InkSession *s) { return s ? &s->budget : NULL; }
int ink_vocab_size(const InkSession *s) { return s ? (int)s->gguf.hp.n_vocab : 0; }
int ink_vocab_unpadded(const InkSession *s) { return s ? (int)s->gguf.hp.n_vocab_unpadded : 0; }
int ink_pos(const InkSession *s) { return s ? s->pos : 0; }
int ink_max_ctx(const InkSession *s) { return s ? s->max_ctx : 0; }
int32_t ink_bos_token(const InkSession *s) { return s ? ink_gguf_find_token(&s->gguf, "<|startoftext|>") : -1; }
int32_t ink_eos_token(const InkSession *s) { return s ? (int32_t)s->gguf.hp.eos_token : -1; }
int32_t ink_find_token(const InkSession *s, const char *name) { return s ? ink_gguf_find_token(&s->gguf, name) : -1; }
int ink_is_eog(const InkSession *s, int32_t id) {
    if (!s || id < 0) return 0;
    if (id == (int32_t)s->gguf.hp.eos_token) return 1;
    const char *str = ink_token_str(s, id);
    return !strcmp(str, "<|return|>") || !strcmp(str, "<|endoftext|>") || !strcmp(str, "<|call|>");
}

int ink_encode(const InkSession *s, const char *text, size_t len, int32_t *out, int max_out, int parse_special) {
    (void)parse_special;
    return s ? (int)ink_gguf_tokenize(&s->gguf, text, len, out, (int64_t)max_out) : -1;
}
int ink_decode_piece(const InkSession *s, int32_t id, char *out, int cap) {
    return s ? (int)ink_gguf_decode_token(&s->gguf, id, out, (size_t)cap) : -1;
}
const char *ink_token_str(const InkSession *s, int32_t id) {
    if (!s || id < 0 || id >= s->gguf.tok.n_tokens) return "";
    return s->gguf.tok.tokens[id].text;
}

int32_t ink_sample(const InkSession *s, const InkSampleParams *p, uint64_t *rng_state) {
    const float *logits = ink_logits(s);
    int n = ink_vocab_unpadded(s);
    if (!logits || n <= 0) return -1;

    float temp = p ? p->temperature : 0.0f;
    if (temp <= 1e-6f) {
        int best = 0; float bv = logits[0];
        for (int i = 1; i < n; i++) {
            if (logits[i] > bv) { bv = logits[i]; best = i; }
        }
        return (int32_t)best;
    }

    int top_k = (p && p->top_k > 0 && p->top_k < n) ? p->top_k : 40;
    float top_p = (p && p->top_p > 0.0f && p->top_p < 1.0f) ? p->top_p : 0.95f;

    int *idx = (int *)malloc((size_t)top_k * sizeof(int));
    float *val = (float *)malloc((size_t)top_k * sizeof(float));
    if (!idx || !val) { free(idx); free(val); return 0; }
    int k = 0;
    for (int i = 0; i < n; i++) {
        float v = logits[i] / temp;
        if (k < top_k) {
            int j = k++;
            while (j > 0 && val[j - 1] < v) { val[j] = val[j - 1]; idx[j] = idx[j - 1]; j--; }
            val[j] = v; idx[j] = i;
        } else if (v > val[top_k - 1]) {
            int j = top_k - 1;
            while (j > 0 && val[j - 1] < v) { val[j] = val[j - 1]; idx[j] = idx[j - 1]; j--; }
            val[j] = v; idx[j] = i;
        }
    }
    float mx = val[0], sum = 0.0f;
    for (int i = 0; i < k; i++) { val[i] = expf(val[i] - mx); sum += val[i]; }
    float cum = 0.0f;
    int cut = k;
    for (int i = 0; i < k; i++) {
        val[i] /= sum;
        cum += val[i];
        if (cum >= top_p) { cut = i + 1; break; }
    }
    float norm = 0.0f;
    for (int i = 0; i < cut; i++) norm += val[i];

    uint64_t x = rng_state ? *rng_state : 0x9e3779b97f4a7c15ULL;
    x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
    if (rng_state) *rng_state = x;
    float r = (float)((x * 2685821657736338717ULL) >> 40) / (float)(1ULL << 24);

    float acc = 0.0f;
    int chosen = idx[0];
    for (int i = 0; i < cut; i++) {
        acc += val[i] / norm;
        if (r <= acc) { chosen = idx[i]; break; }
    }
    free(idx); free(val);
    return (int32_t)chosen;
}

const char *ink_error(const InkSession *s) { return s ? s->error : "null session"; }
double ink_last_gpu_seconds(const InkSession *s) { return s ? s->total_gpu_seconds : 0.0; }

/* ---- --test-quant-kernels: the device's register-dequantizing matvecs
   against the same arithmetic on the host, in the same order. ---- */
static float cpu_nvfp4_val(uint32_t nib) {
    static const float t[8] = {0.0f, 0.5f, 1.0f, 1.5f, 2.0f, 3.0f, 4.0f, 6.0f};
    float v = t[nib & 7u];
    return (nib & 8u) ? -v : v;
}
static float cpu_fp8_e4m3(uint32_t b) {
    uint32_t e = (b >> 3) & 15u, m = b & 7u;
    float v;
    if (e == 0) {
        v = (float)m * (1.0f / 512.0f);
    } else {
        uint32_t bits = ((e + 120u) << 23) | (m << 20);
        memcpy(&v, &bits, 4);
    }
    return (b & 128u) ? -v : v;
}
static float cpu_ue8m0(uint32_t e) {
    uint32_t bits = (e == 0) ? 0x00400000u : ((e & 255u) << 23);
    float v; memcpy(&v, &bits, 4); return v;
}
static float cpu_iq4_val(uint32_t q) {
    static const int8_t t[16] = {-127,-104,-83,-65,-49,-35,-22,-10,1,13,25,38,53,69,89,113};
    return (float)t[q & 15u];
}

int ink_test_quant_kernels(const char *kernel_path, char *err, size_t err_bytes) {
    enum { ROWS = 8, COLS = 64 }; /* 4 NVFP4 blocks a row, 2 MXFP8 blocks, 2 IQ4_NL blocks */
    PcDeviceInfo info;
    PcStatus opened = pc_open(&info);
    if (opened != PC_OK) { snprintf(err, err_bytes, opened == PC_ABSENT ? "no NVIDIA GPU: %s" : "CUDA: %s", pc_log()); return 1; }
    size_t src_len = 0;
    char *src = read_entire_file(kernel_path, &src_len);
    if (!src) { snprintf(err, err_bytes, "cannot read %s", kernel_path); return 1; }
    PcModule mod = NULL; double compile_s = 0.0;
    PcStatus st = pc_compile(src, src_len, "inkling_quant_test", &mod, &compile_s);
    free(src);
    if (st != PC_OK) { snprintf(err, err_bytes, "compile %s: %s", kernel_path, pc_log()); return 1; }

    int rc = 1;
    PcStream stream = NULL;
    PcKernel k_nv = NULL, k_mx = NULL, k_iq = NULL;
    PcBuffer b_x = 0, b_out = 0, b_nv_p = 0, b_nv_s = 0, b_mx_w = 0, b_mx_s = 0, b_iq_w = 0;
    float x[COLS], got[ROWS];
    uint32_t nv_p[ROWS * (COLS / 16) * 2];
    uint8_t nv_sb[ROWS * (COLS / 16)], mx_wb[ROWS * COLS], mx_sb[ROWS * (COLS / 32) + 3];
    uint16_t iq_w[ROWS * (COLS / 32) * 9];
    float max_nv = 0.0f, max_mx = 0.0f, max_iq = 0.0f;
    memset(mx_sb, 0, sizeof mx_sb);

    if (pc_kernel(mod, "ink_dv_nvfp4", &k_nv) != PC_OK ||
        pc_kernel(mod, "ink_dv_mxfp8", &k_mx) != PC_OK ||
        pc_kernel(mod, "ink_dv_iq4", &k_iq) != PC_OK ||
        pc_stream(&stream) != PC_OK ||
        pc_alloc(sizeof x, &b_x) != PC_OK ||
        pc_alloc(sizeof got, &b_out) != PC_OK ||
        pc_alloc(sizeof nv_p, &b_nv_p) != PC_OK ||
        pc_alloc(sizeof nv_sb, &b_nv_s) != PC_OK ||
        pc_alloc(sizeof mx_wb, &b_mx_w) != PC_OK ||
        pc_alloc(sizeof mx_sb, &b_mx_s) != PC_OK ||
        pc_alloc(sizeof iq_w, &b_iq_w) != PC_OK) {
        snprintf(err, err_bytes, "quant test setup: %s", pc_log());
        goto cleanup;
    }
    for (int c = 0; c < COLS; c++) x[c] = (float)((c % 11) - 5) * 0.25f;

    /* 1. NVFP4: FP4 E2M1 elements, an E4M3 scale per block, one global scale. */
    const float gscale = 0.5f;
    const uint8_t e4m3_scales[4] = {0x38, 0x40, 0x30, 0x04}; /* 1.0, 2.0, 0.5, subnormal 4/512 */
    for (int r = 0; r < ROWS; r++) {
        for (int b = 0; b < COLS / 16; b++) {
            int bi = r * (COLS / 16) + b;
            uint32_t w0 = 0, w1 = 0;
            for (int j = 0; j < 8; j++) {
                w0 |= (uint32_t)((r * 3 + b * 5 + j) & 15) << (j * 4);
                w1 |= (uint32_t)((r * 7 + b * 2 + j + 8) & 15) << (j * 4);
            }
            nv_p[bi * 2] = w0; nv_p[bi * 2 + 1] = w1;
            nv_sb[bi] = e4m3_scales[(r + b) & 3];
        }
    }
    PcArg nv_args[7] = {
        a_int(ROWS), a_int(COLS), a_f32(gscale),
        a_u32v(b_nv_p, 0, ROWS * (COLS / 16) * 2),
        a_u32v(b_nv_s, 0, (ROWS * (COLS / 16)) / 4),
        a_f32v(b_x, 0, COLS), a_f32v(b_out, 0, ROWS)
    };
    if (pc_upload(b_x, x, sizeof x, stream) != PC_OK || pc_upload(b_nv_p, nv_p, sizeof nv_p, stream) != PC_OK ||
        pc_upload(b_nv_s, nv_sb, sizeof nv_sb, stream) != PC_OK ||
        pc_launch_groups(k_nv, ROWS, 32, nv_args, 7, stream) != PC_OK ||
        pc_download(got, b_out, sizeof got, stream) != PC_OK || pc_sync_stream(stream) != PC_OK) {
        snprintf(err, err_bytes, "ink_dv_nvfp4: %s", pc_log()); goto cleanup;
    }
    for (int r = 0; r < ROWS; r++) {
        float ref = 0.0f;
        for (int b = 0; b < COLS / 16; b++) {
            int bi = r * (COLS / 16) + b;
            float sc = cpu_fp8_e4m3(nv_sb[bi]) * gscale;
            float sum = 0.0f;
            for (int j = 0; j < 8; j++) {
                sum += cpu_nvfp4_val((nv_p[bi * 2] >> (j * 4)) & 15u) * x[b * 16 + j];
                sum += cpu_nvfp4_val((nv_p[bi * 2 + 1] >> (j * 4)) & 15u) * x[b * 16 + 8 + j];
            }
            ref += sc * sum;
        }
        float diff = fabsf(got[r] - ref);
        if (diff > max_nv) max_nv = diff;
    }
    if (max_nv > 1e-5f) { snprintf(err, err_bytes, "ink_dv_nvfp4 mismatch: max_abs=%g", max_nv); goto cleanup; }

    /* 2. MXFP8: E4M3 elements and a UE8M0 shared exponent per block. */
    const uint8_t e4m3_vals[8] = {0x38, 0xb8, 0x40, 0xc0, 0x34, 0x06, 0x44, 0xbc};
    for (int r = 0; r < ROWS; r++) {
        for (int b = 0; b < COLS / 32; b++) {
            int bi = r * (COLS / 32) + b;
            mx_sb[bi] = (uint8_t)(125 + ((r + b) % 5));
            for (int j = 0; j < 32; j++) mx_wb[bi * 32 + j] = e4m3_vals[(r + b * 3 + j) & 7];
        }
    }
    PcArg mx_args[6] = {
        a_int(ROWS), a_int(COLS),
        a_u32v(b_mx_w, 0, (ROWS * COLS) / 4),
        a_u32v(b_mx_s, 0, (ROWS * (COLS / 32)) / 4),
        a_f32v(b_x, 0, COLS), a_f32v(b_out, 0, ROWS)
    };
    if (pc_upload(b_mx_w, mx_wb, sizeof mx_wb, stream) != PC_OK || pc_upload(b_mx_s, mx_sb, sizeof mx_sb, stream) != PC_OK ||
        pc_launch_groups(k_mx, ROWS, 32, mx_args, 6, stream) != PC_OK ||
        pc_download(got, b_out, sizeof got, stream) != PC_OK || pc_sync_stream(stream) != PC_OK) {
        snprintf(err, err_bytes, "ink_dv_mxfp8: %s", pc_log()); goto cleanup;
    }
    for (int r = 0; r < ROWS; r++) {
        float ref = 0.0f;
        for (int b = 0; b < COLS / 32; b++) {
            int bi = r * (COLS / 32) + b;
            float sc = cpu_ue8m0(mx_sb[bi]);
            float sum = 0.0f;
            for (int j = 0; j < 8; j++)
                sum += cpu_fp8_e4m3(mx_wb[bi * 32 + j * 4]) * x[b * 32 + j * 4]
                     + cpu_fp8_e4m3(mx_wb[bi * 32 + j * 4 + 1]) * x[b * 32 + j * 4 + 1]
                     + cpu_fp8_e4m3(mx_wb[bi * 32 + j * 4 + 2]) * x[b * 32 + j * 4 + 2]
                     + cpu_fp8_e4m3(mx_wb[bi * 32 + j * 4 + 3]) * x[b * 32 + j * 4 + 3];
            ref += sc * sum;
        }
        float diff = fabsf(got[r] - ref);
        if (diff > max_mx) max_mx = diff;
    }
    if (max_mx > 1e-5f) { snprintf(err, err_bytes, "ink_dv_mxfp8 mismatch: max_abs=%g", max_mx); goto cleanup; }

    /* 3. IQ4_NL: an f16 scale and 32 4-bit codes per block. */
    const uint16_t f16_scales[4] = {0x3800, 0x3c00, 0x3400, 0xbc00}; /* 0.5, 1.0, 0.25, -1.0 */
    const float f16_vals[4] = {0.5f, 1.0f, 0.25f, -1.0f};
    for (int r = 0; r < ROWS; r++) {
        for (int b = 0; b < COLS / 32; b++) {
            uint16_t *blk = iq_w + (r * (COLS / 32) + b) * 9;
            blk[0] = f16_scales[(r + b) & 3];
            for (int k = 0; k < 8; k++) {
                uint32_t q0 = (uint32_t)((r + b + 2 * k) & 15);
                uint32_t q1 = (uint32_t)((r + b + 2 * k + 5) & 15);
                uint32_t q2 = (uint32_t)((r + b + 2 * k + 1) & 15);
                uint32_t q3 = (uint32_t)((r + b + 2 * k + 9) & 15);
                blk[1 + k] = (uint16_t)(q0 | (q1 << 4) | (q2 << 8) | (q3 << 12));
            }
        }
    }
    PcArg iq_args[6] = {
        a_int(ROWS), a_int(COLS), a_int(0),
        a_u16v(b_iq_w, 0, ROWS * (COLS / 32) * 9),
        a_f32v(b_x, 0, COLS), a_f32v(b_out, 0, ROWS)
    };
    if (pc_upload(b_iq_w, iq_w, sizeof iq_w, stream) != PC_OK ||
        pc_launch_groups(k_iq, (ROWS + INK_D - 1) / INK_D, INK_T, iq_args, 6, stream) != PC_OK ||
        pc_download(got, b_out, sizeof got, stream) != PC_OK || pc_sync_stream(stream) != PC_OK) {
        snprintf(err, err_bytes, "ink_dv_iq4: %s", pc_log()); goto cleanup;
    }
    for (int r = 0; r < ROWS; r++) {
        float ref = 0.0f;
        for (int b = 0; b < COLS / 32; b++) {
            const uint16_t *blk = iq_w + (r * (COLS / 32) + b) * 9;
            float sc = f16_vals[(r + b) & 3];
            float sum = 0.0f;
            for (int k = 0; k < 8; k++) {
                uint16_t v = blk[1 + k];
                sum += cpu_iq4_val(v & 15u) * x[b * 32 + 2 * k]
                     + cpu_iq4_val((v >> 4) & 15u) * x[b * 32 + 16 + 2 * k]
                     + cpu_iq4_val((v >> 8) & 15u) * x[b * 32 + 2 * k + 1]
                     + cpu_iq4_val((v >> 12) & 15u) * x[b * 32 + 16 + 2 * k + 1];
            }
            ref += sc * sum;
        }
        float diff = fabsf(got[r] - ref);
        if (diff > max_iq) max_iq = diff;
    }
    if (max_iq > 1e-4f) { snprintf(err, err_bytes, "ink_dv_iq4 mismatch: max_abs=%g", max_iq); goto cleanup; }

    fprintf(stderr,
            "cuda-quant-kernels device='%s' compile=%.2fs nvfp4_max_abs=%.2e mxfp8_max_abs=%.2e iq4_max_abs=%.2e ok=true\n",
            info.name, compile_s, max_nv, max_mx, max_iq);
    rc = 0;

cleanup:
    if (stream) pc_sync_stream(stream);
    free_buf(&b_x); free_buf(&b_out); free_buf(&b_nv_p); free_buf(&b_nv_s);
    free_buf(&b_mx_w); free_buf(&b_mx_s); free_buf(&b_iq_w);
    if (stream) pc_destroy_stream(stream);
    pc_unload(mod);
    return rc;
}
