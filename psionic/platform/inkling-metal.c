#include "inkling-metal.h"
#include "inkling-gguf.h"
#include "metal-host.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define INK_T   64
#define INK_NT  256
#define INK_D   8
#define INK_MAX_BLOCKS 8

enum {
    K_EMBED_Q8 = 0,
    K_RMSNORM,
    K_DV_Q8,
    K_DV_IQ4,
    K_DV_F32,
    K_MOE_GU_IQ4,
    K_MOE_GU_Q8,
    K_MOE_DOWN_IQ4,
    K_MOE_DOWN_Q8,
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
    K_COUNT
};

static const char *const k_names[K_COUNT] = {
    "ink_embed_q8",
    "ink_rmsnorm",
    "ink_dv_q8",
    "ink_dv_iq4",
    "ink_dv_f32",
    "ink_moe_gu_iq4",
    "ink_moe_gu_q8",
    "ink_moe_down_iq4",
    "ink_moe_down_q8",
    "ink_router",
    "ink_mix",
    "ink_ctrl_prep",
    "ink_ctrl_apply",
    "ink_convolve",
    "ink_clip",
    "ink_head_rmsnorm",
    "ink_kv_store",
    "ink_attn",
    "ink_add",
    "ink_copy",
    "ink_softcap",
};

static const int64_t k_lanes[K_COUNT] = {
    INK_NT, /* K_EMBED_Q8 */
    INK_NT, /* K_RMSNORM */
    INK_T,  /* K_DV_Q8 */
    INK_T,  /* K_DV_IQ4 */
    INK_T,  /* K_DV_F32 */
    INK_T,  /* K_MOE_GU_IQ4 */
    INK_T,  /* K_MOE_GU_Q8 */
    INK_T,  /* K_MOE_DOWN_IQ4 */
    INK_T,  /* K_MOE_DOWN_Q8 */
    INK_NT, /* K_ROUTER */
    INK_NT, /* K_MIX */
    INK_NT, /* K_CTRL_PREP */
    1,      /* K_CTRL_APPLY */
    1,      /* K_CONVOLVE */
    INK_T,  /* K_CLIP */
    INK_T,  /* K_HEAD_RMSNORM */
    1,      /* K_KV_STORE */
    INK_NT, /* K_ATTN */
    1,      /* K_ADD */
    1,      /* K_COPY */
    1,      /* K_SOFTCAP */
};

struct InkSession {
    InkGGUF gguf;
    PmModule module;
    PmKernel kernels[K_COUNT];
    PmBuffer weights_buf;

    /* Scratch activation buffers (all f32 except moe_ids u32 and kv_k/kv_v u16) */
    PmBuffer cur;
    PmBuffer inp_sa;
    PmBuffer ffn_inp;
    PmBuffer prelude;
    PmBuffer bank_out;
    PmBuffer blocks;       /* [INK_MAX_BLOCKS * n_embd] */
    PmBuffer prelude_blk;  /* [INK_MAX_BLOCKS * n_embd] */
    PmBuffer bank_blk;     /* [INK_MAX_BLOCKS * n_embd] */
    PmBuffer mix_out;      /* [4 * n_embd] */
    PmBuffer messages;     /* [4 * n_embd] */
    PmBuffer inp_q;
    PmBuffer inp_k;
    PmBuffer inp_v;
    PmBuffer ctrl_cat;     /* [2 * n_embd] */
    PmBuffer ctrl_gate;    /* [gate_rank] */
    PmBuffer ctrl_gain;    /* [n_embd] */
    PmBuffer ctrl_forget;  /* [n_embd] */
    PmBuffer q_buf;        /* [n_head * head_dim] */
    PmBuffer k_raw;        /* [n_head_kv * head_dim] */
    PmBuffer k_buf;        /* [n_head_kv * head_dim] */
    PmBuffer v_raw;        /* [n_head_kv * head_dim] */
    PmBuffer v_buf;        /* [n_head_kv * head_dim] */
    PmBuffer r_buf;        /* [n_head * n_rel] */
    PmBuffer dyn_buf;      /* [n_head_kv * l_cache] */
    PmBuffer sinks_buf;    /* [n_head] */
    PmBuffer gate_buf;     /* [n_head * head_dim] */
    PmBuffer attn_scores;  /* [n_head * max_ctx] */
    PmBuffer attn_out;     /* [n_head * head_dim] */
    PmBuffer o_raw;        /* [n_embd] */
    PmBuffer norm_buf;     /* [n_embd] */
    PmBuffer router_logits;/* [n_expert] */
    PmBuffer moe_ids;      /* [n_expert_used] u32 */
    PmBuffer moe_wts;      /* [n_expert_used] f32 */
    PmBuffer moe_hid;      /* [n_expert_used * n_ff_exp] f32 */
    PmBuffer moe_out;      /* [n_embd] */
    PmBuffer logits;       /* [vocab_size] f32 */
    PmBuffer dummy_f32;    /* [64] zero f32 */

    /* Per logical slot recurrent shortconv state and KV cache */
    int n_slots;
    int max_ctx;
    int kv_cap[INK_MAX_SLOTS];
    PmBuffer kv_k[INK_MAX_SLOTS];   /* [kv_cap * n_kv_elem] u16 */
    PmBuffer kv_v[INK_MAX_SLOTS];   /* [kv_cap * n_kv_elem] u16 */
    PmBuffer conv_k[INK_MAX_SLOTS]; /* [n_kv_elem * 3] f32 */
    PmBuffer conv_v[INK_MAX_SLOTS]; /* [n_kv_elem * 3] f32 */
    PmBuffer conv_ao[INK_MAX_SLOTS];/* [n_embd * 3] f32 */
    PmBuffer conv_fo[INK_MAX_SLOTS];/* [n_embd * 3] f32 */

    int pos;
    double total_gpu_seconds;
    int total_steps;
    char error[512];
};

static PmArg a_int(int64_t v) { return (PmArg){.kind = PM_ARG_INT, .i = v}; }
static PmArg a_f32(float v)   { return (PmArg){.kind = PM_ARG_F32, .f = v}; }
static PmArg a_buf(PmBuffer b, int64_t byte_off, int64_t elem_count) {
    return (PmArg){.kind = PM_ARG_VIEW, .buffer = b, .offset = byte_off, .count = elem_count};
}
static PmArg a_f32v(PmBuffer b, int64_t first_elem, int64_t count) {
    return a_buf(b, first_elem * 4, count);
}
static PmArg a_u16v(PmBuffer b, int64_t first_elem, int64_t count) {
    return a_buf(b, first_elem * 2, count);
}
static PmArg a_u32v(PmBuffer b, int64_t first_elem, int64_t count) {
    return a_buf(b, first_elem * 4, count);
}
static PmArg a_tensor_f32(InkSession *s, const InkTensor *t) {
    return a_buf(s->weights_buf, (int64_t)t->offset, (int64_t)(t->nbytes / 4));
}
static PmArg a_tensor_u16(InkSession *s, const InkTensor *t) {
    return a_buf(s->weights_buf, (int64_t)t->offset, (int64_t)(t->nbytes / 2));
}

static int call_k(InkSession *s, PmBatch batch, int k, int64_t items, int count, const PmArg *args) {
    if (pm_launch(batch, s->kernels[k], items, (int)k_lanes[k], 0, args, count) != PM_OK) {
        snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[k], pm_log());
        return 1;
    }
    return 0;
}

#define CALL(s, b, k, items, ...) do { \
    PmArg _args[] = { __VA_ARGS__ }; \
    if (call_k((s), (b), (k), (items), (int)(sizeof _args / sizeof _args[0]), _args) != 0) return 1; \
} while (0)

static int dispatch_mv(InkSession *s, PmBatch batch, const InkTensor *w, int64_t rows, int64_t cols, int add,
                       PmBuffer x_buf, int64_t x_off, PmBuffer out_buf, int64_t out_off) {
    int64_t items = (rows + INK_D - 1) / INK_D;
    if (w->type == INK_GGML_Q8_0) {
        CALL(s, batch, K_DV_Q8, items, a_int(rows), a_int(cols), a_int(add),
             a_tensor_u16(s, w), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
    } else if (w->type == INK_GGML_IQ4_NL) {
        CALL(s, batch, K_DV_IQ4, items, a_int(rows), a_int(cols), a_int(add),
             a_tensor_u16(s, w), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
    } else if (w->type == INK_GGML_F32) {
        CALL(s, batch, K_DV_F32, items, a_int(rows), a_int(cols), a_int(add),
             a_tensor_f32(s, w), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
    } else {
        snprintf(s->error, sizeof s->error, "unsupported weight type %u for matvec", w->type);
        return 1;
    }
    return 0;
}

static int dispatch_control(InkSession *s, PmBatch batch, const InkLayerTensors *L, int ctrl,
                            PmBuffer cur_buf, int64_t cur_off,
                            PmBuffer msg_buf, int64_t msg_off,
                            PmBuffer out_buf, int64_t out_off) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd;
    int64_t rank = hp->gate_rank;
    CALL(s, batch, K_CTRL_PREP, 1, a_int(d),
         a_f32v(cur_buf, cur_off, d), a_f32v(msg_buf, msg_off, d), a_f32v(s->ctrl_cat, 0, 2 * d));
    if (dispatch_mv(s, batch, &L->loop_ctrl_down[ctrl], rank, 2 * d, 0, s->ctrl_cat, 0, s->ctrl_gate, 0)) return 1;
    if (dispatch_mv(s, batch, &L->loop_ctrl_gain[ctrl], d, rank, 0, s->ctrl_gate, 0, s->ctrl_gain, 0)) return 1;
    if (dispatch_mv(s, batch, &L->loop_ctrl_forget[ctrl], d, rank, 0, s->ctrl_gate, 0, s->ctrl_forget, 0)) return 1;
    CALL(s, batch, K_CTRL_APPLY, d, a_int(d),
         a_f32v(cur_buf, cur_off, d), a_f32v(msg_buf, msg_off, d),
         a_f32v(s->ctrl_gain, 0, d), a_f32v(s->ctrl_forget, 0, d), a_f32v(out_buf, out_off, d));
    return 0;
}

static int dispatch_layer(InkSession *s, PmBatch batch, int il, int ip, int *n_blocks, int has_messages, float tau) {
    const InkHParams *hp = &s->gguf.hp;
    const InkLayerTensors *L = &s->gguf.layers[ip];
    int64_t d = hp->n_embd;
    int64_t n_head = hp->n_head;
    int64_t n_head_kv = hp->n_head_kv;
    int64_t hd = hp->head_dim;
    int64_t n_q = n_head * hd;
    int64_t n_kv = n_head_kv * hd;
    int64_t n_rel = hp->rel_len;
    int is_swa = hp->is_swa[ip];

    /* inpSA = cur */
    CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->inp_sa, 0, d));

    /* Attention inputs: SWA mixes 1 vector, full attention mixes 3 vectors (Q, K, V) */
    if (L->attn_res_query.present) {
        int64_t nq = is_swa ? 1 : 3;
        CALL(s, batch, K_MIX, nq, a_int(d), a_int(*n_blocks),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &L->attn_res_query),
             a_f32v(s->mix_out, 0, nq * d));
        CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->mix_out, 0, d), a_f32v(s->inp_q, 0, d));
        if (is_swa) {
            CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->mix_out, 0, d), a_f32v(s->inp_k, 0, d));
            CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->mix_out, 0, d), a_f32v(s->inp_v, 0, d));
        } else {
            CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->mix_out, d, d), a_f32v(s->inp_k, 0, d));
            CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->mix_out, 2 * d, d), a_f32v(s->inp_v, 0, d));
        }
    } else {
        CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->inp_q, 0, d));
        CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->inp_k, 0, d));
        CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->inp_v, 0, d));
    }

    if (has_messages) {
        if (dispatch_control(s, batch, L, 0, s->inp_q, 0, s->messages, 0 * d, s->inp_q, 0)) return 1;
        if (dispatch_control(s, batch, L, 1, s->inp_k, 0, s->messages, 1 * d, s->inp_k, 0)) return 1;
        if (dispatch_control(s, batch, L, 2, s->inp_v, 0, s->messages, 2 * d, s->inp_v, 0)) return 1;
    }

    /* Input RMSNorms */
    if (is_swa && has_messages) {
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->inp_q, 0, d), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_q, 0, d));
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->inp_k, 0, d), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_k, 0, d));
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->inp_v, 0, d), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_v, 0, d));
    } else if (is_swa) {
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->inp_q, 0, d), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_q, 0, d));
        CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d));
        CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->inp_q, 0, d), a_f32v(s->inp_v, 0, d));
    } else {
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->inp_q, 0, d), a_tensor_f32(s, &L->attn_q_inp_norm), a_f32v(s->inp_q, 0, d));
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->inp_k, 0, d), a_tensor_f32(s, &L->attn_k_inp_norm), a_f32v(s->inp_k, 0, d));
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->inp_v, 0, d), a_tensor_f32(s, &L->attn_v_inp_norm), a_f32v(s->inp_v, 0, d));
    }

    /* Q, K, V, R projections + K/V causal shortconv */
    if (dispatch_mv(s, batch, &L->attn_q, n_q, d, 0, s->inp_q, 0, s->q_buf, 0)) return 1;
    if (dispatch_mv(s, batch, &L->attn_k, n_kv, d, 0, s->inp_k, 0, s->k_raw, 0)) return 1;
    if (!is_swa && L->attn_k_conv_proj.present) {
        if (dispatch_mv(s, batch, &L->attn_k_conv_proj, n_head_kv * hp->conv_len, d, 0,
                        s->inp_k, 0, s->dyn_buf, 0)) return 1;
        CALL(s, batch, K_CONVOLVE, n_kv, a_int(n_kv), a_int(n_head_kv), a_int(1),
             a_f32v(s->k_raw, 0, n_kv), a_tensor_f32(s, &L->attn_k_conv),
             a_f32v(s->dyn_buf, 0, n_head_kv * hp->conv_len),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->k_buf, 0, n_kv));
    } else {
        CALL(s, batch, K_CONVOLVE, n_kv, a_int(n_kv), a_int(n_head_kv), a_int(0),
             a_f32v(s->k_raw, 0, n_kv), a_tensor_f32(s, &L->attn_k_conv),
             a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->k_buf, 0, n_kv));
    }

    if (dispatch_mv(s, batch, &L->attn_v, n_kv, d, 0, s->inp_v, 0, s->v_raw, 0)) return 1;
    if (!is_swa && L->attn_v_conv_proj.present) {
        if (dispatch_mv(s, batch, &L->attn_v_conv_proj, n_head_kv * hp->conv_len, d, 0,
                        s->inp_v, 0, s->dyn_buf, 0)) return 1;
        CALL(s, batch, K_CONVOLVE, n_kv, a_int(n_kv), a_int(n_head_kv), a_int(1),
             a_f32v(s->v_raw, 0, n_kv), a_tensor_f32(s, &L->attn_v_conv),
             a_f32v(s->dyn_buf, 0, n_head_kv * hp->conv_len),
             a_f32v(s->conv_v[il], 0, n_kv * 3), a_f32v(s->v_buf, 0, n_kv));
    } else {
        CALL(s, batch, K_CONVOLVE, n_kv, a_int(n_kv), a_int(n_head_kv), a_int(0),
             a_f32v(s->v_raw, 0, n_kv), a_tensor_f32(s, &L->attn_v_conv),
             a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_v[il], 0, n_kv * 3), a_f32v(s->v_buf, 0, n_kv));
    }

    if (dispatch_mv(s, batch, &L->attn_rel, n_head * n_rel, d, 0, s->inp_q, 0, s->r_buf, 0)) return 1;

    /* Clip Q, K, R; RMSNorm V; apply tau on full-attention Q and R */
    float q_tau = is_swa ? 1.0f : tau;
    CALL(s, batch, K_CLIP, n_head, a_int(hd), a_int(L->attn_q_clip.present ? 1 : 0),
         a_f32(hp->qk_clip), a_f32(q_tau),
         a_f32v(s->q_buf, 0, n_q),
         L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64));
    CALL(s, batch, K_CLIP, n_head_kv, a_int(hd), a_int(0),
         a_f32(hp->qk_clip), a_f32(1.0f),
         a_f32v(s->k_buf, 0, n_kv), a_f32v(s->dummy_f32, 0, 64));
    CALL(s, batch, K_HEAD_RMSNORM, n_head_kv, a_int(hd), a_f32(hp->rms_eps),
         a_f32v(s->v_buf, 0, n_kv));
    CALL(s, batch, K_CLIP, n_head, a_int(n_rel), a_int(L->attn_rel_clip.present ? 1 : 0),
         a_f32(hp->rel_clip), a_f32(q_tau),
         a_f32v(s->r_buf, 0, n_head * n_rel),
         L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64));

    /* Store K, V into KV cache for slot il */
    int64_t kv_cap = s->kv_cap[il];
    int64_t slot_pos = is_swa ? (s->pos % kv_cap) : s->pos;
    CALL(s, batch, K_KV_STORE, n_kv, a_int(n_kv), a_int(slot_pos),
         a_f32v(s->k_buf, 0, n_kv), a_f32v(s->v_buf, 0, n_kv),
         a_u16v(s->kv_k[il], 0, kv_cap * n_kv), a_u16v(s->kv_v[il], 0, kv_cap * n_kv));

    /* Sink logits & attention output gate */
    if (dispatch_mv(s, batch, &L->attn_sink_proj, n_head, d, 0, s->inp_q, 0, s->sinks_buf, 0)) return 1;
    if (dispatch_mv(s, batch, &L->attn_gate, n_q, d, 0, s->inp_q, 0, s->gate_buf, 0)) return 1;

    /* Attention (packed scalar dims keep total args <= 16 for metal-host.m) */
    int64_t dims0 = n_head | (n_head_kv << 16) | (hd << 32) | (n_rel << 48);
    int64_t dims1 = hp->rel_extent | ((int64_t)is_swa << 24) | (hp->swa << 32);
    int64_t kv_info = kv_cap | ((int64_t)s->max_ctx << 32);
    CALL(s, batch, K_ATTN, n_head,
         a_int(dims0), a_int(dims1), a_int(kv_info), a_int(s->pos),
         a_f32v(s->q_buf, 0, n_q),
         a_f32v(s->r_buf, 0, n_head * n_rel),
         a_tensor_f32(s, &L->attn_rel_bias),
         a_f32v(s->sinks_buf, 0, n_head),
         a_f32v(s->gate_buf, 0, n_q),
         a_u16v(s->kv_k[il], 0, kv_cap * n_kv),
         a_u16v(s->kv_v[il], 0, kv_cap * n_kv),
         a_f32v(s->attn_scores, 0, n_head * s->max_ctx),
         a_f32v(s->attn_out, 0, n_q));

    /* Output projection + post shortconv */
    if (dispatch_mv(s, batch, &L->attn_output, d, n_q, 0, s->attn_out, 0, s->o_raw, 0)) return 1;
    CALL(s, batch, K_CONVOLVE, d, a_int(d), a_int(1), a_int(0),
         a_f32v(s->o_raw, 0, d), a_tensor_f32(s, &L->attn_post_conv),
         a_f32v(s->dummy_f32, 0, 64),
         a_f32v(s->conv_ao[il], 0, d * 3), a_f32v(s->cur, 0, d));

    /* Residual stream block boundary vs add */
    if (ip % hp->block_size == 0) {
        if (*n_blocks >= INK_MAX_BLOCKS) {
            snprintf(s->error, sizeof s->error, "block count overflow");
            return 1;
        }
        CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->inp_sa, 0, d),
             a_f32v(s->blocks, (int64_t)(*n_blocks) * d, d));
        (*n_blocks)++;
    } else {
        CALL(s, batch, K_ADD, d, a_int(d), a_f32v(s->inp_sa, 0, d), a_f32v(s->cur, 0, d),
             a_f32v(s->cur, 0, d));
    }

    /* ffn_inp = cur */
    CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->ffn_inp, 0, d));

    /* MoE input: mix -> optional control -> RMSNorm */
    CALL(s, batch, K_MIX, 1, a_int(d), a_int(*n_blocks),
         a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
         a_f32v(s->cur, 0, d),
         a_tensor_f32(s, &L->ffn_res_query),
         a_f32v(s->norm_buf, 0, d));
    if (has_messages) {
        if (dispatch_control(s, batch, L, 3, s->norm_buf, 0, s->messages, 3 * d, s->norm_buf, 0)) return 1;
    }
    CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
         a_f32v(s->norm_buf, 0, d), a_tensor_f32(s, &L->ffn_norm), a_f32v(s->norm_buf, 0, d));

    /* MoE router */
    if (dispatch_mv(s, batch, &L->ffn_gate_inp, hp->n_expert, d, 0, s->norm_buf, 0, s->router_logits, 0)) return 1;
    CALL(s, batch, K_ROUTER, 1, a_int(hp->n_expert), a_int(hp->n_expert_used),
         a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
         a_f32v(s->router_logits, 0, hp->n_expert),
         L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
         a_u32v(s->moe_ids, 0, hp->n_expert_used),
         a_f32v(s->moe_wts, 0, hp->n_expert_used));

    /* MoE gate_up + SITU */
    int64_t n_ff = hp->n_ff_exp;
    int64_t gu_items = (int64_t)hp->n_expert_used * ((n_ff + 3) / 4);
    if (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) {
        CALL(s, batch, K_MOE_GU_IQ4, gu_items, a_int(n_ff), a_int(d),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_tensor_u16(s, &L->ffn_gate_up_exps),
             a_f32v(s->norm_buf, 0, d),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else if (L->ffn_gate_up_exps.type == INK_GGML_Q8_0) {
        CALL(s, batch, K_MOE_GU_Q8, gu_items, a_int(n_ff), a_int(d),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_tensor_u16(s, &L->ffn_gate_up_exps),
             a_f32v(s->norm_buf, 0, d),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else {
        snprintf(s->error, sizeof s->error, "unsupported gate_up type %u", L->ffn_gate_up_exps.type);
        return 1;
    }

    /* MoE down + weighted sum */
    int64_t dn_items = (d + INK_D - 1) / INK_D;
    if (L->ffn_down_exps.type == INK_GGML_IQ4_NL) {
        CALL(s, batch, K_MOE_DOWN_IQ4, dn_items, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_f32v(s->moe_out, 0, d));
    } else if (L->ffn_down_exps.type == INK_GGML_Q8_0) {
        CALL(s, batch, K_MOE_DOWN_Q8, dn_items, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_f32v(s->moe_out, 0, d));
    } else {
        snprintf(s->error, sizeof s->error, "unsupported down type %u", L->ffn_down_exps.type);
        return 1;
    }

    /* MoE post shortconv + residual add */
    CALL(s, batch, K_CONVOLVE, d, a_int(d), a_int(1), a_int(0),
         a_f32v(s->moe_out, 0, d), a_tensor_f32(s, &L->ffn_post_conv),
         a_f32v(s->dummy_f32, 0, 64),
         a_f32v(s->conv_fo[il], 0, d * 3), a_f32v(s->cur, 0, d));
    CALL(s, batch, K_ADD, d, a_int(d), a_f32v(s->ffn_inp, 0, d), a_f32v(s->cur, 0, d),
         a_f32v(s->cur, 0, d));
    return 0;
}

static int alloc_buf(InkSession *s, PmBuffer *b, size_t bytes) {
    if (pm_alloc(bytes ? bytes : 64, b) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_alloc(%zu): %s", bytes, pm_log());
        return 1;
    }
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

int ink_open(const char *gguf_path, const char *metal_path, int max_ctx, InkSession **out, char *err, size_t err_bytes) {
    *out = NULL;
    InkSession *s = (InkSession *)calloc(1, sizeof *s);
    if (!s) { snprintf(err, err_bytes, "out of memory"); return 1; }

    if (ink_gguf_open(gguf_path, &s->gguf, err, err_bytes) != 0) {
        free(s); return 1;
    }
    const InkHParams *hp = &s->gguf.hp;
    if (max_ctx <= 0) max_ctx = 4096;
    if (max_ctx > 32768) max_ctx = 32768;
    s->max_ctx = max_ctx;
    s->n_slots = (int)(hp->n_phys + (hp->loop_count - 1) * (hp->loop_end - hp->loop_start));
    if (s->n_slots > INK_MAX_SLOTS) {
        snprintf(err, err_bytes, "n_slots %d > %d", s->n_slots, INK_MAX_SLOTS);
        ink_close(s); return 1;
    }

    PmDeviceInfo info;
    if (pm_open(&info) != PM_OK) {
        snprintf(err, err_bytes, "pm_open: %s", pm_log());
        ink_close(s); return 1;
    }
    size_t metal_len = 0;
    char *metal_src = read_entire_file(metal_path, &metal_len);
    if (!metal_src) {
        snprintf(err, err_bytes, "cannot read metal source %s", metal_path);
        ink_close(s); return 1;
    }
    double compile_s = 0.0;
    PmStatus cs = pm_compile(metal_src, metal_len, "inkling", &s->module, &compile_s);
    free(metal_src);
    if (cs != PM_OK) {
        snprintf(err, err_bytes, "pm_compile(%s): %s", metal_path, pm_log());
        ink_close(s); return 1;
    }
    for (int k = 0; k < K_COUNT; k++) {
        if (pm_kernel(s->module, k_names[k], &s->kernels[k]) != PM_OK) {
            snprintf(err, err_bytes, "pm_kernel(%s): %s", k_names[k], pm_log());
            ink_close(s); return 1;
        }
    }

    if (pm_wrap(s->gguf.map, s->gguf.map_size, &s->weights_buf) != PM_OK) {
        snprintf(err, err_bytes, "pm_wrap(%zu): %s", s->gguf.map_size, pm_log());
        ink_close(s); return 1;
    }

    size_t d = (size_t)hp->n_embd;
    size_t n_q = (size_t)hp->n_head * (size_t)hp->head_dim;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;

    if (alloc_buf(s, &s->cur, d * 4) ||
        alloc_buf(s, &s->inp_sa, d * 4) ||
        alloc_buf(s, &s->ffn_inp, d * 4) ||
        alloc_buf(s, &s->prelude, d * 4) ||
        alloc_buf(s, &s->bank_out, d * 4) ||
        alloc_buf(s, &s->blocks, INK_MAX_BLOCKS * d * 4) ||
        alloc_buf(s, &s->prelude_blk, INK_MAX_BLOCKS * d * 4) ||
        alloc_buf(s, &s->bank_blk, INK_MAX_BLOCKS * d * 4) ||
        alloc_buf(s, &s->mix_out, 4 * d * 4) ||
        alloc_buf(s, &s->messages, 4 * d * 4) ||
        alloc_buf(s, &s->inp_q, d * 4) ||
        alloc_buf(s, &s->inp_k, d * 4) ||
        alloc_buf(s, &s->inp_v, d * 4) ||
        alloc_buf(s, &s->ctrl_cat, 2 * d * 4) ||
        alloc_buf(s, &s->ctrl_gate, (size_t)hp->gate_rank * 4) ||
        alloc_buf(s, &s->ctrl_gain, d * 4) ||
        alloc_buf(s, &s->ctrl_forget, d * 4) ||
        alloc_buf(s, &s->q_buf, n_q * 4) ||
        alloc_buf(s, &s->k_raw, n_kv * 4) ||
        alloc_buf(s, &s->k_buf, n_kv * 4) ||
        alloc_buf(s, &s->v_raw, n_kv * 4) ||
        alloc_buf(s, &s->v_buf, n_kv * 4) ||
        alloc_buf(s, &s->r_buf, (size_t)hp->n_head * (size_t)hp->rel_len * 4) ||
        alloc_buf(s, &s->dyn_buf, (size_t)hp->n_head_kv * (size_t)hp->conv_len * 4) ||
        alloc_buf(s, &s->sinks_buf, (size_t)hp->n_head * 4) ||
        alloc_buf(s, &s->gate_buf, n_q * 4) ||
        alloc_buf(s, &s->attn_scores, (size_t)hp->n_head * (size_t)s->max_ctx * 4) ||
        alloc_buf(s, &s->attn_out, n_q * 4) ||
        alloc_buf(s, &s->o_raw, d * 4) ||
        alloc_buf(s, &s->norm_buf, d * 4) ||
        alloc_buf(s, &s->router_logits, (size_t)hp->n_expert * 4) ||
        alloc_buf(s, &s->moe_ids, (size_t)hp->n_expert_used * 4) ||
        alloc_buf(s, &s->moe_wts, (size_t)hp->n_expert_used * 4) ||
        alloc_buf(s, &s->moe_hid, (size_t)hp->n_expert_used * (size_t)hp->n_ff_exp * 4) ||
        alloc_buf(s, &s->moe_out, d * 4) ||
        alloc_buf(s, &s->logits, (size_t)hp->n_vocab * 4) ||
        alloc_buf(s, &s->dummy_f32, 64 * 4)) {
        snprintf(err, err_bytes, "%s", s->error);
        ink_close(s); return 1;
    }

    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
        int cap = hp->is_swa[ip] ? (hp->swa < s->max_ctx ? hp->swa : s->max_ctx) : s->max_ctx;
        s->kv_cap[il] = cap;
        if (alloc_buf(s, &s->kv_k[il], (size_t)cap * n_kv * 2) ||
            alloc_buf(s, &s->kv_v[il], (size_t)cap * n_kv * 2) ||
            alloc_buf(s, &s->conv_k[il], n_kv * 3 * 4) ||
            alloc_buf(s, &s->conv_v[il], n_kv * 3 * 4) ||
            alloc_buf(s, &s->conv_ao[il], d * 3 * 4) ||
            alloc_buf(s, &s->conv_fo[il], d * 3 * 4)) {
            snprintf(err, err_bytes, "%s", s->error);
            ink_close(s); return 1;
        }
    }

    s->pos = 0;
    *out = s;
    return 0;
}

void ink_close(InkSession *s) {
    if (!s) return;
    for (int il = 0; il < INK_MAX_SLOTS; il++) {
        pm_free(s->kv_k[il]); pm_free(s->kv_v[il]);
        pm_free(s->conv_k[il]); pm_free(s->conv_v[il]);
        pm_free(s->conv_ao[il]); pm_free(s->conv_fo[il]);
    }
    pm_free(s->cur); pm_free(s->inp_sa); pm_free(s->ffn_inp); pm_free(s->prelude); pm_free(s->bank_out);
    pm_free(s->blocks); pm_free(s->prelude_blk); pm_free(s->bank_blk);
    pm_free(s->mix_out); pm_free(s->messages);
    pm_free(s->inp_q); pm_free(s->inp_k); pm_free(s->inp_v);
    pm_free(s->ctrl_cat); pm_free(s->ctrl_gate); pm_free(s->ctrl_gain); pm_free(s->ctrl_forget);
    pm_free(s->q_buf); pm_free(s->k_raw); pm_free(s->k_buf); pm_free(s->v_raw); pm_free(s->v_buf);
    pm_free(s->r_buf); pm_free(s->dyn_buf); pm_free(s->sinks_buf); pm_free(s->gate_buf);
    pm_free(s->attn_scores); pm_free(s->attn_out); pm_free(s->o_raw); pm_free(s->norm_buf);
    pm_free(s->router_logits); pm_free(s->moe_ids); pm_free(s->moe_wts); pm_free(s->moe_hid); pm_free(s->moe_out);
    pm_free(s->logits); pm_free(s->dummy_f32);
    pm_free(s->weights_buf);
    for (int k = 0; k < K_COUNT; k++) pm_kernel_free(s->kernels[k]);
    pm_unload(s->module);
    ink_gguf_close(&s->gguf);
    free(s);
}

void ink_reset(InkSession *s) {
    if (!s) return;
    const InkHParams *hp = &s->gguf.hp;
    size_t d = (size_t)hp->n_embd;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    for (int il = 0; il < s->n_slots; il++) {
        memset(pm_contents(s->conv_k[il]), 0, n_kv * 3 * 4);
        memset(pm_contents(s->conv_v[il]), 0, n_kv * 3 * 4);
        memset(pm_contents(s->conv_ao[il]), 0, d * 3 * 4);
        memset(pm_contents(s->conv_fo[il]), 0, d * 3 * 4);
    }
    s->pos = 0;
}

static int enqueue_step(InkSession *s, PmBatch batch, int32_t token_id, int compute_logits) {
    const InkHParams *hp = &s->gguf.hp;
    if (s->pos >= s->max_ctx) {
        snprintf(s->error, sizeof s->error, "context full (%d/%d)", s->pos, s->max_ctx);
        return 1;
    }
    if (token_id < 0 || token_id >= hp->n_vocab) {
        snprintf(s->error, sizeof s->error, "token id %d out of range [0,%lld)", token_id, (long long)hp->n_vocab);
        return 1;
    }

    /* tau = 1 + alpha * log(max(1, (pos + 1) / floor)) */
    float ratio = (float)(s->pos + 1) / (float)hp->log_scale_floor;
    if (ratio < 1.0f) ratio = 1.0f;
    float tau = 1.0f + hp->log_scale_alpha * logf(ratio);

    int64_t d = hp->n_embd;
    /* 1. Embedding lookup + token_embd_norm */
    CALL(s, batch, K_EMBED_Q8, 1, a_int(token_id), a_int(d),
         a_tensor_u16(s, &s->gguf.tok_embd),
         a_tensor_f32(s, &s->gguf.tok_norm),
         a_f32(hp->rms_eps),
         a_f32v(s->cur, 0, d));

    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;

    /* 2. Prelude layers [0 .. start) */
    for (int il = 0; il < start; il++) {
        if (dispatch_layer(s, batch, il, il, &n_blocks, 0, tau) != 0) return 1;
    }

    /* Save prelude output and prelude blocks */
    CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->prelude, 0, d));
    int prelude_n_blocks = n_blocks;
    if (prelude_n_blocks > 0) {
        CALL(s, batch, K_COPY, (int64_t)prelude_n_blocks * d, a_int((int64_t)prelude_n_blocks * d),
             a_f32v(s->blocks, 0, (int64_t)prelude_n_blocks * d),
             a_f32v(s->prelude_blk, 0, (int64_t)prelude_n_blocks * d));
    }

    /* 3. Looped core passes [0 .. loop_count) over physical layers [start .. end) */
    for (int pass = 0; pass < hp->loop_count; pass++) {
        int bank_n_blocks = 0;
        if (pass > 0) {
            int src_off = prelude_n_blocks + 1;
            bank_n_blocks = n_blocks - src_off;
            if (bank_n_blocks < 0) bank_n_blocks = 0;
            if (bank_n_blocks > 0) {
                CALL(s, batch, K_COPY, (int64_t)bank_n_blocks * d, a_int((int64_t)bank_n_blocks * d),
                     a_f32v(s->blocks, (int64_t)src_off * d, (int64_t)bank_n_blocks * d),
                     a_f32v(s->bank_blk, 0, (int64_t)bank_n_blocks * d));
            }
            CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->bank_out, 0, d));
            CALL(s, batch, K_COPY, d, a_int(d), a_f32v(s->prelude, 0, d), a_f32v(s->cur, 0, d));
            n_blocks = prelude_n_blocks;
            if (prelude_n_blocks > 0) {
                CALL(s, batch, K_COPY, (int64_t)prelude_n_blocks * d, a_int((int64_t)prelude_n_blocks * d),
                     a_f32v(s->prelude_blk, 0, (int64_t)prelude_n_blocks * d),
                     a_f32v(s->blocks, 0, (int64_t)prelude_n_blocks * d));
            }
        }
        for (int ip = start; ip < end; ip++) {
            int il = (pass == 0) ? ip : (int)hp->n_phys + (pass - 1) * (end - start) + (ip - start);
            if (pass == 0) {
                if (dispatch_layer(s, batch, il, ip, &n_blocks, 0, tau) != 0) return 1;
            } else {
                const InkLayerTensors *L = &s->gguf.layers[ip];
                CALL(s, batch, K_MIX, 4, a_int(d), a_int(bank_n_blocks),
                     a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * d),
                     a_f32v(s->bank_out, 0, d),
                     a_tensor_f32(s, &L->loop_msg_query),
                     a_f32v(s->messages, 0, 4 * d));
                if (dispatch_layer(s, batch, il, ip, &n_blocks, 1, tau) != 0) return 1;
            }
        }
    }

    /* 4. Coda layers [end .. n_phys) */
    for (int il = end; il < hp->n_phys; il++) {
        if (dispatch_layer(s, batch, il, il, &n_blocks, 0, tau) != 0) return 1;
    }

    /* 5. Output mix -> output_norm -> lm_head -> logit softcap */
    if (compute_logits) {
        CALL(s, batch, K_MIX, 1, a_int(d), a_int(n_blocks),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &s->gguf.out_res_query),
             a_f32v(s->norm_buf, 0, d));
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->norm_buf, 0, d), a_tensor_f32(s, &s->gguf.out_norm), a_f32v(s->norm_buf, 0, d));
        if (dispatch_mv(s, batch, &s->gguf.output, hp->n_vocab, d, 0, s->norm_buf, 0, s->logits, 0) != 0) return 1;
        if (hp->logit_softcap > 0.0f) {
            CALL(s, batch, K_SOFTCAP, hp->n_vocab, a_int(hp->n_vocab),
                 a_f32(hp->logit_softcap), a_f32v(s->logits, 0, hp->n_vocab));
        }
    }

    s->pos++;
    s->total_steps++;
    return 0;
}

int ink_prefill(InkSession *s, const int32_t *tokens, int n_tokens, int compute_last_logits) {
    if (n_tokens <= 0) return 0;
    PmBatch batch = NULL;
    if (pm_begin(&batch) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_begin: %s", pm_log());
        return 1;
    }
    for (int i = 0; i < n_tokens; i++) {
        int need_logits = compute_last_logits && (i == n_tokens - 1);
        if (enqueue_step(s, batch, tokens[i], need_logits) != 0) {
            pm_wait(batch, NULL);
            return 1;
        }
    }
    double gpu_s = 0.0;
    if (pm_wait(batch, &gpu_s) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_wait: %s", pm_log());
        return 1;
    }
    s->total_gpu_seconds += gpu_s;
    return 0;
}

int ink_step(InkSession *s, int32_t token_id, int compute_logits) {
    return ink_prefill(s, &token_id, 1, compute_logits);
}

const float *ink_logits(const InkSession *s) {
    return s ? (const float *)pm_contents(s->logits) : NULL;
}
const InkHParams *ink_hparams(const InkSession *s) { return s ? &s->gguf.hp : NULL; }
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

    /* Simple top-k selection */
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
