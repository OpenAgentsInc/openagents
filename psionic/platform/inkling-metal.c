#include "inkling-metal.h"
#include "inkling-gguf.h"
#include "metal-host.h"

#include <math.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/sysctl.h>
#include <time.h>

#define INK_T   64
#define INK_NT  256
#define INK_D   8
#define INK_MAX_BLOCKS 8
#define INK_PREFILL_B 3072

enum {
    K_EMBED_Q8 = 0,
    K_RMSNORM,
    K_DV_Q8,
    K_DV_IQ4,
    K_DV_F32,
    K_DV_F32_SMALL,
    K_CTRL_UP_APPLY,
    K_MOE_GU_IQ4,
    K_MOE_GU_Q8,
    K_MOE_DOWN_IQ4,
    K_MOE_DOWN_Q8,
    K_ROUTER,
    K_MIX,
    K_CTRL_PREP,
    K_CTRL_APPLY,
    K_CONVOLVE,
    K_CONVOLVE_ADD,
    K_CLIP,
    K_HEAD_RMSNORM,
    K_KV_STORE,
    K_ATTN,
    K_ADD,
    K_COPY,
    K_SOFTCAP,
    /* Batched prefill kernels (B <= 64) */
    K_MM_Q8,
    K_MM_F32,
    K_EMBED_Q8_BATCH,
    K_MIX_BATCH,
    K_CTRL_PREP_BATCH,
    K_CTRL_UP_APPLY_BATCH,
    K_CONVOLVE_BATCH,
    K_CONVOLVE_ADD_BATCH,
    K_CLIP_BATCH,
    K_KV_STORE_BATCH,
    K_ROUTER_BATCH,
    K_MOE_GROUP_BATCH,
    K_MOE_BUILD_TILES,
    K_MOE_GU_BATCH_IQ4,
    K_MOE_GU_BATCH_Q8,
    K_SITU_BATCH,
    K_MOE_DOWN_BATCH_IQ4,
    K_MOE_DOWN_BATCH_Q8,
    K_MOE_REDUCE_BATCH,
    K_ATTN_SCORES_MMA,
    K_ATTN_SOFTMAX_MMA,
    K_ATTN_VALUES_MMA,
    K_ATTN_GQA_SPLIT,
    K_ATTN_GQA_REDUCE,
    K_DEQUANT_Q8_BF16,
    K_MM_BF16,
    K_MM_BF16_16,
    K_MOE_EXP_OFF,
    K_MOE_CHUNK_TILES,
    K_MOE_GATHER_X,
    K_DEQUANT_IQ4_EXP_BF16,
    K_DEQUANT_Q8_EXP_BF16,
    K_MOE_MM_BF16,
    K_MOE_MM_BF16_16,
    K_MOE_SCATTER_WT,
    K_CLIP_KV,
    K_HEAD_RMSNORM_KV,
    K_MIX_RMSNORM,
    K_MOE_GU_IQ4_M4,
    K_MOE_DOWN_IQ4_M4,
    K_MM_F32_M4,
    K_DEQUANT_F32_BF16,
    K_MM_BF16_32,
    K_MOE_MM_IQ4,
    K_MOE_MM_IQ4_16,
    K_DV_Q8_CONV,
    K_CONV_CLIP_KV,
    K_CONV_HEAD_RMSNORM_KV,
    K_MIX_CTRL_PREP,
    K_ARGMAX_S1,
    K_ARGMAX_S2,
    K_MOE_SUM_BATCH,
    K_DV_Q4,
    K_DV_Q4_CONV,
    K_DV_F16_SMALL,
    K_CTRL_UP_APPLY_F16,
    K_MOE_GU_Q4,
    K_MOE_DOWN_Q4,
    K_ATTN_GQA_1PASS,
    K_MIX_CTRL_PREP_QKV,
    K_DEQUANT_Q4_BF16,
    K_MOE_MM_IQ4_128,
    K_MOE_MM_IQ4_256,
    K_DV_Q4_QRG,
    K_RMSNORM_3,
    K_CONV_KV_BOTH,
    K_DV_Q4_CTRL_DOWN_3,
    K_CTRL_UP_APPLY_F16_3,
    K_MOE_GATHER_BF16,
    K_SITU_BATCH_BF16,
    K_F32_TO_BF16,
    K_COUNT
};

static const char *const k_names[K_COUNT] = {
    "ink_embed_q8",
    "ink_rmsnorm",
    "ink_dv_q8",
    "ink_dv_iq4",
    "ink_dv_f32",
    "ink_dv_f32_small",
    "ink_ctrl_up_apply",
    "ink_moe_gu_iq4",
    "ink_moe_gu_q8",
    "ink_moe_down_iq4",
    "ink_moe_down_q8",
    "ink_router",
    "ink_mix",
    "ink_ctrl_prep",
    "ink_ctrl_apply",
    "ink_convolve",
    "ink_convolve_add",
    "ink_clip",
    "ink_head_rmsnorm",
    "ink_kv_store",
    "ink_attn",
    "ink_add",
    "ink_copy",
    "ink_softcap",
    "ink_mm_q8",
    "ink_mm_f32",
    "ink_embed_q8_batch",
    "ink_mix_batch",
    "ink_ctrl_prep_batch",
    "ink_ctrl_up_apply_batch",
    "ink_convolve_batch",
    "ink_convolve_add_batch",
    "ink_clip_batch",
    "ink_kv_store_batch",
    "ink_router_batch",
    "ink_moe_group_batch",
    "ink_moe_build_tiles",
    "ink_moe_gu_batch_iq4",
    "ink_moe_gu_batch_q8",
    "ink_situ_batch",
    "ink_moe_down_batch_iq4",
    "ink_moe_down_batch_q8",
    "ink_moe_reduce_batch",
    "ink_attn_scores_mma",
    "ink_attn_softmax_mma",
    "ink_attn_values_mma",
    "ink_attn_gqa_split",
    "ink_attn_gqa_reduce",
    "ink_dequant_q8_bf16",
    "ink_mm_bf16",
    "ink_mm_bf16_16",
    "ink_moe_exp_off",
    "ink_moe_chunk_tiles",
    "ink_moe_gather_x",
    "ink_dequant_iq4_exp_bf16",
    "ink_dequant_q8_exp_bf16",
    "ink_moe_mm_bf16",
    "ink_moe_mm_bf16_16",
    "ink_moe_scatter_wt",
    "ink_clip_kv",
    "ink_head_rmsnorm_kv",
    "ink_mix_rmsnorm",
    "ink_moe_gu_iq4_m4",
    "ink_moe_down_iq4_m4",
    "ink_mm_f32_m4",
    "ink_dequant_f32_bf16",
    "ink_mm_bf16_32",
    "ink_moe_mm_iq4",
    "ink_moe_mm_iq4_16",
    "ink_dv_q8_conv",
    "ink_conv_clip_kv",
    "ink_conv_head_rmsnorm_kv",
    "ink_mix_ctrl_prep",
    "ink_argmax_s1",
    "ink_argmax_s2",
    "ink_moe_sum_batch",
    "ink_dv_q4",
    "ink_dv_q4_conv",
    "ink_dv_f16_small",
    "ink_ctrl_up_apply_f16",
    "ink_moe_gu_q4",
    "ink_moe_down_q4",
    "ink_attn_gqa_1pass",
    "ink_mix_ctrl_prep_qkv",
    "ink_dequant_q4_bf16",
    "ink_moe_mm_iq4_128",
    "ink_moe_mm_iq4_256",
    "ink_dv_q4_qrg",
    "ink_rmsnorm_3",
    "ink_conv_kv_both",
    "ink_dv_q4_ctrl_down_3",
    "ink_ctrl_up_apply_f16_3",
    "ink_moe_gather_bf16",
    "ink_situ_batch_bf16",
    "ink_f32_to_bf16",
};

static const int64_t k_lanes[K_COUNT] = {
    INK_NT, /* K_EMBED_Q8 */
    INK_NT, /* K_RMSNORM */
    32,     /* K_DV_Q8 */
    INK_T,  /* K_DV_IQ4 */
    INK_T,  /* K_DV_F32 */
    INK_NT, /* K_DV_F32_SMALL */
    INK_T,  /* K_CTRL_UP_APPLY */
    128,    /* K_MOE_GU_IQ4 */
    128,    /* K_MOE_GU_Q8 */
    128,    /* K_MOE_DOWN_IQ4 */
    128,    /* K_MOE_DOWN_Q8 */
    32,     /* K_ROUTER */
    INK_NT, /* K_MIX */
    INK_NT, /* K_CTRL_PREP */
    1,      /* K_CTRL_APPLY */
    1,      /* K_CONVOLVE */
    1,      /* K_CONVOLVE_ADD */
    INK_T,  /* K_CLIP */
    INK_T,  /* K_HEAD_RMSNORM */
    1,      /* K_KV_STORE */
    512,    /* K_ATTN */
    INK_NT, /* K_ADD */
    INK_NT, /* K_COPY */
    INK_NT, /* K_SOFTCAP */
    128,    /* K_MM_Q8 */
    128,    /* K_MM_F32 */
    INK_NT, /* K_EMBED_Q8_BATCH */
    INK_NT, /* K_MIX_BATCH */
    INK_NT, /* K_CTRL_PREP_BATCH */
    INK_T,  /* K_CTRL_UP_APPLY_BATCH */
    64,     /* K_CONVOLVE_BATCH */
    64,     /* K_CONVOLVE_ADD_BATCH */
    INK_T,  /* K_CLIP_BATCH */
    128,    /* K_KV_STORE_BATCH */
    INK_NT, /* K_ROUTER_BATCH */
    128,    /* K_MOE_GROUP_BATCH */
    1,      /* K_MOE_BUILD_TILES */
    128,    /* K_MOE_GU_BATCH_IQ4 */
    128,    /* K_MOE_GU_BATCH_Q8 */
    128,    /* K_SITU_BATCH */
    128,    /* K_MOE_DOWN_BATCH_IQ4 */
    128,    /* K_MOE_DOWN_BATCH_Q8 */
    1,      /* K_MOE_REDUCE_BATCH */
    128,    /* K_ATTN_SCORES_MMA */
    32,     /* K_ATTN_SOFTMAX_MMA */
    128,    /* K_ATTN_VALUES_MMA */
    128,    /* K_ATTN_GQA_SPLIT */
    128,    /* K_ATTN_GQA_REDUCE */
    128,    /* K_DEQUANT_Q8_BF16 */
    256,    /* K_MM_BF16 */
    128,    /* K_MM_BF16_16 */
    1,      /* K_MOE_EXP_OFF */
    1,      /* K_MOE_CHUNK_TILES */
    128,    /* K_MOE_GATHER_X */
    96,     /* K_DEQUANT_IQ4_EXP_BF16 */
    128,    /* K_DEQUANT_Q8_EXP_BF16 */
    128,    /* K_MOE_MM_BF16 */
    128,    /* K_MOE_MM_BF16_16 */
    128,    /* K_MOE_SCATTER_WT */
    INK_T,  /* K_CLIP_KV */
    INK_T,  /* K_HEAD_RMSNORM_KV */
    INK_NT, /* K_MIX_RMSNORM */
    128,    /* K_MOE_GU_IQ4_M4 */
    128,    /* K_MOE_DOWN_IQ4_M4 */
    128,    /* K_MM_F32_M4 */
    128,    /* K_DEQUANT_F32_BF16 */
    128,    /* K_MM_BF16_32 */
    128,    /* K_MOE_MM_IQ4 */
    128,    /* K_MOE_MM_IQ4_16 */
    32,     /* K_DV_Q8_CONV */
    INK_T,  /* K_CONV_CLIP_KV */
    INK_T,  /* K_CONV_HEAD_RMSNORM_KV */
    INK_NT, /* K_MIX_CTRL_PREP */
    INK_NT, /* K_ARGMAX_S1 */
    INK_T,  /* K_ARGMAX_S2 */
    128,    /* K_MOE_SUM_BATCH */
    128,    /* K_DV_Q4 */
    128,    /* K_DV_Q4_CONV */
    128,    /* K_DV_F16_SMALL */
    INK_T,  /* K_CTRL_UP_APPLY_F16 */
    128,    /* K_MOE_GU_Q4 */
    128,    /* K_MOE_DOWN_Q4 */
    128,    /* K_ATTN_GQA_1PASS */
    INK_NT, /* K_MIX_CTRL_PREP_QKV */
    96,     /* K_DEQUANT_Q4_BF16 */
    128,    /* K_MOE_MM_IQ4_128 */
    128,    /* K_MOE_MM_IQ4_256 */
    128,    /* K_DV_Q4_QRG */
    INK_NT, /* K_RMSNORM_3 */
    32,     /* K_CONV_KV_BOTH */
    128,    /* K_DV_Q4_CTRL_DOWN_3 */
    INK_T,  /* K_CTRL_UP_APPLY_F16_3 */
    128,    /* K_MOE_GATHER_BF16 */
    128,    /* K_SITU_BATCH_BF16 */
    256,    /* K_F32_TO_BF16 */
};

typedef struct {
    int64_t off;   /* byte offset in fast_buf (-1 if unused) */
    int64_t words; /* u16 element count */
} FastSpan;

typedef struct {
    FastSpan attn_q, attn_k, attn_v, attn_output, attn_gate, attn_rel;
    FastSpan attn_qrg; /* contiguous Q + R + Gate in Q4_0 for fused 1-dispatch Wave 3! */
    FastSpan ffn_gate_inp;
    FastSpan attn_sink_proj, attn_k_conv_proj, attn_v_conv_proj;
    FastSpan moe_gu_q4, moe_dn_q4;
    FastSpan ctrl_down[4], ctrl_gain[4], ctrl_forget[4];
    FastSpan ctrl_down_qkv_q4, ctrl_down3_q4, ctrl_gain_qkv, ctrl_forget_qkv;
} FastLayer;

struct InkSession {
    InkGGUF gguf;
    PmModule module;
    PmKernel kernels[K_COUNT];
    PmBuffer weights_buf;
    PmBuffer fast_buf;
    FastLayer fast_layers[INK_MAX_SLOTS];
    FastSpan fast_output;

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
    PmBuffer moe_hid;      /* [INK_PREFILL_B * n_expert_used * n_ff_exp] f32 */
    PmBuffer moe_gu;       /* [INK_PREFILL_B * n_expert_used * 2 * n_ff_exp] f32 */
    PmBuffer moe_out;      /* [INK_PREFILL_B * n_embd] */
    PmBuffer moe_slot_out; /* [INK_PREFILL_B * n_expert_used * n_embd] */
    PmBuffer exp_toks;     /* [n_expert * INK_PREFILL_B] u32 */
    PmBuffer exp_slots;    /* [n_expert * INK_PREFILL_B] u32 */
    PmBuffer exp_wts;      /* [n_expert * INK_PREFILL_B] f32 */
    PmBuffer exp_cnt;      /* [n_expert] u32 */
    PmBuffer exp_off;      /* [n_expert] u32 */
    PmBuffer row_dst;      /* [INK_PREFILL_B * n_expert_used] u32 */
    PmBuffer row_wt;       /* [INK_PREFILL_B * n_expert_used] f32 */
    PmBuffer tile_exp;     /* [n_expert + INK_PREFILL_B / 8] u32 */
    PmBuffer tile_m0;      /* [n_expert + INK_PREFILL_B / 8] u32 */
    PmBuffer tile_cnt;     /* [16] u32 */
    PmBuffer w_bf16;       /* [220M] u32 (BF16 pairs for Metal 4 am_matmul_bf16: 64-exp GU = 2 chunks, 128-exp Down = 1 chunk) */
    PmBuffer tok_ids_buf;  /* [INK_PREFILL_B] u32 */
    PmBuffer tau_buf;      /* [prefill_b] f32 per-token log-scale tau for 32K batched prefill */
    PmBuffer logits;       /* [vocab_size] f32 */
    PmBuffer argmax_vals;  /* [64] f32 */
    PmBuffer argmax_ids;   /* [64] u32 */
    PmBuffer argmax_out;   /* [1] u32 */
    int argmax_valid;
    PmBuffer dummy_f32;    /* [64] zero f32 */

    /* Per logical slot recurrent shortconv state and KV cache */
    int n_slots;
    int max_ctx;
    int prefill_b;
    InkMemoryBudget budget;
    int kv_cap[INK_MAX_SLOTS];
    PmBuffer kv_k[INK_MAX_SLOTS];   /* [kv_cap * n_kv_elem] u16 */
    PmBuffer kv_v[INK_MAX_SLOTS];   /* [kv_cap * n_kv_elem] u16 */
    PmBuffer conv_k[INK_MAX_SLOTS]; /* [n_kv_elem * 3] f32 */
    PmBuffer conv_v[INK_MAX_SLOTS]; /* [n_kv_elem * 3] f32 */
    PmBuffer conv_ao[INK_MAX_SLOTS];/* [n_embd * 3] f32 */
    PmBuffer conv_fo[INK_MAX_SLOTS];/* [n_embd * 3] f32 */

    int pos;
    int max_pos_written;
    int64_t last_hidden_off;
    /* Exact cross-request KV + shortconv prefix snapshots (slot 0 = prefill end, slot 1 = backup/turn end). */
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
    char error[512];
};

static PmArg a_int(int64_t v) { return (PmArg){.kind = PM_ARG_INT, .i = v}; }
static PmArg a_u32(uint32_t v){ return (PmArg){.kind = PM_ARG_U32, .i = (int64_t)v}; }
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
        CALL(s, batch, K_DV_Q8, (rows + 1) / 2, a_int(rows), a_int(cols), a_int(add),
             a_tensor_u16(s, w), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
    } else if (w->type == INK_GGML_IQ4_NL) {
        CALL(s, batch, K_DV_IQ4, items, a_int(rows), a_int(cols), a_int(add),
             a_tensor_u16(s, w), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
    } else if (w->type == INK_GGML_F32) {
        if (rows <= 128 && cols >= 256) {
            CALL(s, batch, K_DV_F32_SMALL, rows, a_int(rows), a_int(cols), a_int(add),
                 a_tensor_f32(s, w), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
        } else {
            CALL(s, batch, K_DV_F32, items, a_int(rows), a_int(cols), a_int(add),
                 a_tensor_f32(s, w), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
        }
    } else {
        snprintf(s->error, sizeof s->error, "unsupported weight type %u for matvec", w->type);
        return 1;
    }
    return 0;
}

static PmArg a_fast_u16(InkSession *s, const FastSpan *fs) {
    return a_buf(s->fast_buf, fs->off, fs->words);
}

static int dispatch_mv_q4(InkSession *s, PmBatch batch, const InkTensor *w, const FastSpan *fs, int64_t rows, int64_t cols, int add,
                          PmBuffer x_buf, int64_t x_off, PmBuffer out_buf, int64_t out_off) {
    if (s->fast_buf && fs && fs->off >= 0) {
        CALL(s, batch, K_DV_Q4, (rows + 3) / 4, a_int(rows), a_int(cols), a_int(add),
             a_fast_u16(s, fs), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
        return 0;
    }
    return dispatch_mv(s, batch, w, rows, cols, add, x_buf, x_off, out_buf, out_off);
}

static int dispatch_mv_f16_small(InkSession *s, PmBatch batch, const InkTensor *w, const FastSpan *fs, int64_t rows, int64_t cols, int add,
                                 PmBuffer x_buf, int64_t x_off, PmBuffer out_buf, int64_t out_off) {
    if (s->fast_buf && fs && fs->off >= 0) {
        CALL(s, batch, K_DV_F16_SMALL, (rows + 3) / 4, a_int(rows), a_int(cols), a_int(add),
             a_fast_u16(s, fs), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
        return 0;
    }
    return dispatch_mv(s, batch, w, rows, cols, add, x_buf, x_off, out_buf, out_off);
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
    pm_barrier(batch);
    if (dispatch_mv(s, batch, &L->loop_ctrl_down[ctrl], rank, 2 * d, 0, s->ctrl_cat, 0, s->ctrl_gate, 0)) return 1;
    pm_barrier(batch);
    if (L->loop_ctrl_gain[ctrl].type == INK_GGML_F32 && L->loop_ctrl_forget[ctrl].type == INK_GGML_F32 && rank <= 64) {
        int64_t items = (d + INK_T - 1) / INK_T;
        CALL(s, batch, K_CTRL_UP_APPLY, items, a_int(d), a_int(rank),
             a_tensor_f32(s, &L->loop_ctrl_gain[ctrl]),
             a_tensor_f32(s, &L->loop_ctrl_forget[ctrl]),
             a_f32v(s->ctrl_gate, 0, rank),
             a_f32v(cur_buf, cur_off, d),
             a_f32v(msg_buf, msg_off, d),
             a_f32v(out_buf, out_off, d));
    } else {
        if (dispatch_mv(s, batch, &L->loop_ctrl_gain[ctrl], d, rank, 0, s->ctrl_gate, 0, s->ctrl_gain, 0)) return 1;
        if (dispatch_mv(s, batch, &L->loop_ctrl_forget[ctrl], d, rank, 0, s->ctrl_gate, 0, s->ctrl_forget, 0)) return 1;
        pm_barrier(batch);
        CALL(s, batch, K_CTRL_APPLY, d, a_int(d),
             a_f32v(cur_buf, cur_off, d), a_f32v(msg_buf, msg_off, d),
             a_f32v(s->ctrl_gain, 0, d), a_f32v(s->ctrl_forget, 0, d), a_f32v(out_buf, out_off, d));
    }
    pm_barrier(batch);
    return 0;
}

static int prof_active = 0;
static double prof_ms[12];
static void prof_mark(PmBatch *bp, int slot) {
    if (!prof_active) return;
    double dt = 0.0;
    pm_wait(*bp, &dt);
    prof_ms[slot] += dt * 1000.0;
    pm_begin_concurrent(bp);
}

static int dispatch_layer(InkSession *s, PmBatch *bp, int il, int ip, int *n_blocks, int has_messages, int bank_n_blocks, float tau) {
    PmBatch batch = *bp;
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

    /* Wave 1: save inp_sa = cur concurrently with depth-attention mix (+ fused QKV ctrl_prep when pass_messages precomputed!). */
    CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->inp_sa, 0, d));
    int precomp_msgs = (has_messages && bank_n_blocks >= 0 && (hp->loop_end - hp->loop_start) <= s->prefill_b);
    int64_t msg_base = precomp_msgs ? ((int64_t)(ip - hp->loop_start) * 4 * d) : 0;
    if (has_messages && bank_n_blocks >= 0 && !precomp_msgs) {
        CALL(s, batch, K_MIX, 4, a_int(d), a_int(bank_n_blocks),
             a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->bank_out, 0, d),
             a_tensor_f32(s, &L->loop_msg_query),
             a_f32v(s->messages, 0, 4 * d));
    }

    PmBuffer q_src = s->cur, k_src = s->cur, v_src = s->cur;
    int64_t q_off = 0, k_off = 0, v_off = 0;
    int fused_swa_norm = (L->attn_res_query.present && is_swa && !has_messages);
    int fused_qkv_prep = (L->attn_res_query.present && precomp_msgs);
    if (fused_swa_norm) {
        CALL(s, batch, K_MIX_RMSNORM, 1, a_int(d), a_int(*n_blocks), a_f32(hp->rms_eps),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &L->attn_res_query),
             a_tensor_f32(s, &L->attn_norm),
             a_f32v(s->inp_q, 0, d));
    } else if (fused_qkv_prep) {
        CALL(s, batch, K_MIX_CTRL_PREP_QKV, 3, a_int(d), a_int(*n_blocks), a_int(is_swa ? 1 : 0),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &L->attn_res_query),
             a_f32v(s->messages, msg_base, 3 * d),
             a_f32v(s->mix_out, 0, 3 * d),
             a_f32v(s->ctrl_cat, 0, 3 * 2 * d));
        q_src = s->mix_out; q_off = 0;
        k_src = s->mix_out; k_off = d;
        v_src = s->mix_out; v_off = 2 * d;
    } else if (L->attn_res_query.present) {
        int64_t nq = is_swa ? 1 : 3;
        CALL(s, batch, K_MIX, nq, a_int(d), a_int(*n_blocks),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &L->attn_res_query),
             a_f32v(s->mix_out, 0, nq * d));
        q_src = s->mix_out; q_off = 0;
        k_src = s->mix_out; k_off = is_swa ? 0 : d;
        v_src = s->mix_out; v_off = is_swa ? 0 : 2 * d;
    }
    pm_barrier(batch);

    /* Optional loop control gates (Q, K, V run CONCURRENTLY in 2 waves when fused_qkv_prep!). */
    const FastLayer *FL = &s->fast_layers[ip];
    if (has_messages) {
        int64_t rank = hp->gate_rank;
        if (!fused_qkv_prep) {
            CALL(s, batch, K_CTRL_PREP, 1, a_int(d),
                 a_f32v(q_src, q_off, d), a_f32v(s->messages, msg_base + 0 * d, d), a_f32v(s->ctrl_cat, 0 * 2 * d, 2 * d));
            CALL(s, batch, K_CTRL_PREP, 1, a_int(d),
                 a_f32v(k_src, k_off, d), a_f32v(s->messages, msg_base + 1 * d, d), a_f32v(s->ctrl_cat, 1 * 2 * d, 2 * d));
            CALL(s, batch, K_CTRL_PREP, 1, a_int(d),
                 a_f32v(v_src, v_off, d), a_f32v(s->messages, msg_base + 2 * d, d), a_f32v(s->ctrl_cat, 2 * 2 * d, 2 * d));
            pm_barrier(batch);
        }
        if (s->fast_buf && FL->ctrl_down_qkv_q4.off >= 0) {
            CALL(s, batch, K_DV_Q4_CTRL_DOWN_3, (3 * rank + 3) / 4, a_int(rank), a_int(2 * d),
                 a_fast_u16(s, &FL->ctrl_down_qkv_q4), a_f32v(s->ctrl_cat, 0, 6 * d), a_f32v(s->ctrl_gate, 0, 3 * rank));
        } else {
            if (dispatch_mv_f16_small(s, batch, &L->loop_ctrl_down[0], &FL->ctrl_down[0], rank, 2 * d, 0, s->ctrl_cat, 0 * 2 * d, s->ctrl_gate, 0 * rank)) return 1;
            if (dispatch_mv_f16_small(s, batch, &L->loop_ctrl_down[1], &FL->ctrl_down[1], rank, 2 * d, 0, s->ctrl_cat, 1 * 2 * d, s->ctrl_gate, 1 * rank)) return 1;
            if (dispatch_mv_f16_small(s, batch, &L->loop_ctrl_down[2], &FL->ctrl_down[2], rank, 2 * d, 0, s->ctrl_cat, 2 * 2 * d, s->ctrl_gate, 2 * rank)) return 1;
        }
        pm_barrier(batch);
        if (s->fast_buf && FL->ctrl_gain_qkv.off >= 0 && FL->ctrl_forget_qkv.off >= 0 && rank <= 64 && q_src == s->mix_out && k_src == s->mix_out && v_src == s->mix_out) {
            int64_t citems = (d + INK_T - 1) / INK_T;
            CALL(s, batch, K_CTRL_UP_APPLY_F16_3, 3 * citems, a_int(d), a_int(rank), a_int(is_swa ? 1 : 0),
                 a_fast_u16(s, &FL->ctrl_gain_qkv), a_fast_u16(s, &FL->ctrl_forget_qkv),
                 a_f32v(s->ctrl_gate, 0, 3 * rank), a_f32v(s->mix_out, 0, 3 * d), a_f32v(s->messages, msg_base, 3 * d),
                 a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d));
            pm_barrier(batch);
        } else if (s->fast_buf && FL->ctrl_gain[0].off >= 0 && FL->ctrl_forget[0].off >= 0 && rank <= 64) {
            int64_t citems = (d + INK_T - 1) / INK_T;
            CALL(s, batch, K_CTRL_UP_APPLY_F16, citems, a_int(d), a_int(rank),
                 a_fast_u16(s, &FL->ctrl_gain[0]), a_fast_u16(s, &FL->ctrl_forget[0]),
                 a_f32v(s->ctrl_gate, 0 * rank, rank), a_f32v(q_src, q_off, d), a_f32v(s->messages, msg_base + 0 * d, d), a_f32v(s->inp_q, 0, d));
            CALL(s, batch, K_CTRL_UP_APPLY_F16, citems, a_int(d), a_int(rank),
                 a_fast_u16(s, &FL->ctrl_gain[1]), a_fast_u16(s, &FL->ctrl_forget[1]),
                 a_f32v(s->ctrl_gate, 1 * rank, rank), a_f32v(k_src, k_off, d), a_f32v(s->messages, msg_base + 1 * d, d), a_f32v(s->inp_k, 0, d));
            CALL(s, batch, K_CTRL_UP_APPLY_F16, citems, a_int(d), a_int(rank),
                 a_fast_u16(s, &FL->ctrl_gain[2]), a_fast_u16(s, &FL->ctrl_forget[2]),
                 a_f32v(s->ctrl_gate, 2 * rank, rank), a_f32v(v_src, v_off, d), a_f32v(s->messages, msg_base + 2 * d, d), a_f32v(s->inp_v, 0, d));
            pm_barrier(batch);
        } else if (L->loop_ctrl_gain[0].type == INK_GGML_F32 && L->loop_ctrl_forget[0].type == INK_GGML_F32 && rank <= 64) {
            int64_t citems = (d + INK_T - 1) / INK_T;
            CALL(s, batch, K_CTRL_UP_APPLY, citems, a_int(d), a_int(rank),
                 a_tensor_f32(s, &L->loop_ctrl_gain[0]), a_tensor_f32(s, &L->loop_ctrl_forget[0]),
                 a_f32v(s->ctrl_gate, 0 * rank, rank), a_f32v(q_src, q_off, d), a_f32v(s->messages, msg_base + 0 * d, d), a_f32v(s->inp_q, 0, d));
            CALL(s, batch, K_CTRL_UP_APPLY, citems, a_int(d), a_int(rank),
                 a_tensor_f32(s, &L->loop_ctrl_gain[1]), a_tensor_f32(s, &L->loop_ctrl_forget[1]),
                 a_f32v(s->ctrl_gate, 1 * rank, rank), a_f32v(k_src, k_off, d), a_f32v(s->messages, msg_base + 1 * d, d), a_f32v(s->inp_k, 0, d));
            CALL(s, batch, K_CTRL_UP_APPLY, citems, a_int(d), a_int(rank),
                 a_tensor_f32(s, &L->loop_ctrl_gain[2]), a_tensor_f32(s, &L->loop_ctrl_forget[2]),
                 a_f32v(s->ctrl_gate, 2 * rank, rank), a_f32v(v_src, v_off, d), a_f32v(s->messages, msg_base + 2 * d, d), a_f32v(s->inp_v, 0, d));
            pm_barrier(batch);
        } else {
            if (dispatch_control(s, batch, L, 0, q_src, q_off, s->messages, msg_base + 0 * d, s->inp_q, 0)) return 1;
            if (dispatch_control(s, batch, L, 1, k_src, k_off, s->messages, msg_base + 1 * d, s->inp_k, 0)) return 1;
            if (dispatch_control(s, batch, L, 2, v_src, v_off, s->messages, msg_base + 2 * d, s->inp_v, 0)) return 1;
        }
        q_src = s->inp_q; q_off = 0;
        k_src = s->inp_k; k_off = 0;
        v_src = s->inp_v; v_off = 0;
    }

    /* Wave 2: Input RMSNorms (concurrent across Q, K, V; SWA without messages shares one norm!). */
    PmBuffer q_in = s->inp_q, k_in = s->inp_k, v_in = s->inp_v;
    if (fused_swa_norm == 2) {
        /* already normalized by K_CTRL_UP_APPLY_F16_RMSNORM */
    } else if (fused_swa_norm) {
        k_in = s->inp_q;
        v_in = s->inp_q;
    } else if (is_swa && has_messages) {
        CALL(s, batch, K_RMSNORM_3, 3, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(q_src, q_off, d), a_f32v(k_src, k_off, d), a_f32v(v_src, v_off, d),
             a_tensor_f32(s, &L->attn_norm), a_tensor_f32(s, &L->attn_norm), a_tensor_f32(s, &L->attn_norm),
             a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d));
        pm_barrier(batch);
    } else if (is_swa) {
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(q_src, q_off, d), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_q, 0, d));
        k_in = s->inp_q;
        v_in = s->inp_q;
        pm_barrier(batch);
    } else {
        CALL(s, batch, K_RMSNORM_3, 3, a_int(d), a_int(0), a_f32(hp->rms_eps),
             a_f32v(q_src, q_off, d), a_f32v(k_src, k_off, d), a_f32v(v_src, v_off, d),
             a_tensor_f32(s, &L->attn_q_inp_norm), a_tensor_f32(s, &L->attn_k_inp_norm), a_tensor_f32(s, &L->attn_v_inp_norm),
             a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d));
        pm_barrier(batch);
    }
    prof_mark(bp, 0); batch = *bp; /* 0: mix + ctrl + rmsnorm */

    /* Wave 3 (Sprint 50): Fused 128-thread Q+R+Gate (-96 dispatches/tok!) + K/V/Sinks/Dyn concurrently! */
    if (s->fast_buf && FL->attn_qrg.off >= 0) {
        int64_t qrg_rows = n_q + n_head * n_rel + n_q;
        CALL(s, batch, K_DV_Q4_QRG, (qrg_rows + 3) / 4, a_int(n_q), a_int(n_head * n_rel), a_int(d),
             a_fast_u16(s, &FL->attn_qrg), a_f32v(q_in, 0, d),
             a_f32v(s->q_buf, 0, n_q), a_f32v(s->r_buf, 0, n_head * n_rel), a_f32v(s->gate_buf, 0, n_q));
    } else {
        if (dispatch_mv_q4(s, batch, &L->attn_q, &FL->attn_q, n_q, d, 0, q_in, 0, s->q_buf, 0)) return 1;
        if (dispatch_mv_q4(s, batch, &L->attn_rel, &FL->attn_rel, n_head * n_rel, d, 0, q_in, 0, s->r_buf, 0)) return 1;
        if (dispatch_mv_q4(s, batch, &L->attn_gate, &FL->attn_gate, n_q, d, 0, q_in, 0, s->gate_buf, 0)) return 1;
    }
    if (dispatch_mv_q4(s, batch, &L->attn_k, &FL->attn_k, n_kv, d, 0, k_in, 0, s->k_raw, 0)) return 1;
    if (dispatch_mv_q4(s, batch, &L->attn_v, &FL->attn_v, n_kv, d, 0, v_in, 0, s->v_raw, 0)) return 1;
    if (dispatch_mv_f16_small(s, batch, &L->attn_sink_proj, &FL->attn_sink_proj, n_head, d, 0, q_in, 0, s->sinks_buf, 0)) return 1;
    int has_k_dyn = (!is_swa && L->attn_k_conv_proj.present);
    int has_v_dyn = (!is_swa && L->attn_v_conv_proj.present);
    int64_t dyn_sz = n_head_kv * hp->conv_len;
    if (has_k_dyn) {
        if (dispatch_mv_f16_small(s, batch, &L->attn_k_conv_proj, &FL->attn_k_conv_proj, dyn_sz, d, 0,
                                  k_in, 0, s->dyn_buf, 0)) return 1;
    }
    if (has_v_dyn) {
        if (dispatch_mv_f16_small(s, batch, &L->attn_v_conv_proj, &FL->attn_v_conv_proj, dyn_sz, d, 0,
                                  v_in, 0, s->dyn_buf, dyn_sz)) return 1;
    }
    pm_barrier(batch);
    prof_mark(bp, 1); batch = *bp; /* 1: attn input projs (Q/K/V/R/Gate/Sinks/Dyn) */

    /* Wave 4 (Sprint 50): 32-lane ZERO-TGEM fused K+V conv+clip/norm+KV store (-48 dispatches/tok!) + fused Q/R clip in GQA 1pass! */
    float q_tau = is_swa ? 1.0f : tau;
    int64_t kv_cap = s->kv_cap[il];
    int64_t slot_pos = is_swa ? (s->pos % kv_cap) : s->pos;
    int64_t n_keys_attn = (is_swa && s->pos >= hp->swa) ? hp->swa : (s->pos + 1);
    int use_1pass = (n_head == n_head_kv * 3 && hd == 128 && n_keys_attn >= 1 && n_keys_attn <= 256);
    if (hd == 128 && has_k_dyn == has_v_dyn) {
        CALL(s, batch, K_CONV_KV_BOTH, 2 * n_head_kv, a_int(n_head_kv), a_int(n_kv), a_int(slot_pos),
             a_int(has_k_dyn ? 1 : 0), a_f32(hp->qk_clip), a_f32(hp->rms_eps),
             a_f32v(s->k_raw, 0, n_kv), a_f32v(s->v_raw, 0, n_kv),
             a_tensor_f32(s, &L->attn_k_conv), a_tensor_f32(s, &L->attn_v_conv),
             has_k_dyn ? a_f32v(s->dyn_buf, 0, 2 * dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->conv_v[il], 0, n_kv * 3),
             a_u16v(s->kv_k[il], 0, kv_cap * n_kv), a_u16v(s->kv_v[il], 0, kv_cap * n_kv));
    } else {
        CALL(s, batch, K_CONV_CLIP_KV, n_head_kv, a_int(hd), a_int(n_kv), a_int(slot_pos), a_int(has_k_dyn ? 1 : 0),
             a_f32(hp->qk_clip), a_f32v(s->k_raw, 0, n_kv), a_tensor_f32(s, &L->attn_k_conv),
             has_k_dyn ? a_f32v(s->dyn_buf, 0, dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_u16v(s->kv_k[il], 0, kv_cap * n_kv));
        CALL(s, batch, K_CONV_HEAD_RMSNORM_KV, n_head_kv, a_int(hd), a_int(n_kv), a_int(slot_pos), a_int(has_v_dyn ? 1 : 0),
             a_f32(hp->rms_eps), a_f32v(s->v_raw, 0, n_kv), a_tensor_f32(s, &L->attn_v_conv),
             has_v_dyn ? a_f32v(s->dyn_buf, dyn_sz, dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_v[il], 0, n_kv * 3), a_u16v(s->kv_v[il], 0, kv_cap * n_kv));
    }
    if (!use_1pass) {
        CALL(s, batch, K_CLIP, n_head, a_int(hd), a_int(L->attn_q_clip.present ? 1 : 0),
             a_f32(hp->qk_clip), a_f32(q_tau),
             a_f32v(s->q_buf, 0, n_q),
             L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64));
        CALL(s, batch, K_CLIP, n_head, a_int(n_rel), a_int(L->attn_rel_clip.present ? 1 : 0),
             a_f32(hp->rel_clip), a_f32(q_tau),
             a_f32v(s->r_buf, 0, n_head * n_rel),
             L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64));
    }
    pm_barrier(batch);

    /* Attention (Sprint 47: 18-TG 1-pass Flash-Decoding GQA with fused Q-clip & R-clip!) */
    uint32_t dims0 = (uint32_t)n_head | ((uint32_t)n_head_kv << 8) | ((uint32_t)hd << 16) | ((uint32_t)n_rel << 24);
    uint32_t dims1 = (uint32_t)hp->rel_extent | ((uint32_t)(is_swa ? 1 : 0) << 16) | ((uint32_t)hp->swa << 17);
    if (use_1pass) {
        uint32_t kv_info = (uint32_t)kv_cap | ((uint32_t)(L->attn_q_clip.present ? 1 : 0) << 16) | ((uint32_t)(L->attn_rel_clip.present ? 1 : 0) << 17);
        CALL(s, batch, K_ATTN_GQA_1PASS, n_head,
             a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos),
             a_f32(hp->qk_clip), a_f32(hp->rel_clip), a_f32(q_tau),
             a_f32v(s->q_buf, 0, n_q),
             L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->r_buf, 0, n_head * n_rel),
             L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64),
             a_tensor_f32(s, &L->attn_rel_bias),
             a_f32v(s->sinks_buf, 0, n_head),
             a_f32v(s->gate_buf, 0, n_q),
             a_u16v(s->kv_k[il], 0, kv_cap * n_kv),
             a_u16v(s->kv_v[il], 0, kv_cap * n_kv),
             a_f32v(s->attn_out, 0, n_q));
    } else if (n_head == n_head_kv * 3 && hd == 128 && s->max_ctx >= 130 && n_keys_attn >= 1) {
        int64_t n_splits = (n_keys_attn >= 1024) ? 32 : (n_keys_attn >= 256) ? 16 : (n_keys_attn >= 64) ? 8 : (n_keys_attn >= 16) ? 4 : 1;
        int64_t max_sp = s->max_ctx / 130;
        if (n_splits > max_sp) n_splits = max_sp;
        if (n_splits < 1) n_splits = 1;
        uint32_t kv_info = (uint32_t)kv_cap | ((uint32_t)n_splits << 16);
        int64_t split_elems = n_head * n_splits * 130;
        CALL(s, batch, K_ATTN_GQA_SPLIT, n_head_kv * n_splits,
             a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos),
             a_f32v(s->q_buf, 0, n_q),
             a_f32v(s->r_buf, 0, n_head * n_rel),
             a_tensor_f32(s, &L->attn_rel_bias),
             a_f32v(s->sinks_buf, 0, n_head),
             a_u16v(s->kv_k[il], 0, kv_cap * n_kv),
             a_u16v(s->kv_v[il], 0, kv_cap * n_kv),
             a_f32v(s->attn_scores, 0, split_elems));
        pm_barrier(batch);
        CALL(s, batch, K_ATTN_GQA_REDUCE, n_head,
             a_int(n_splits),
             a_f32v(s->attn_scores, 0, split_elems),
             a_f32v(s->gate_buf, 0, n_q),
             a_f32v(s->attn_out, 0, n_q));
    } else {
        uint32_t kv_info = (uint32_t)kv_cap | ((uint32_t)s->max_ctx << 16);
        CALL(s, batch, K_ATTN, n_head,
             a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos),
             a_f32v(s->q_buf, 0, n_q),
             a_f32v(s->r_buf, 0, n_head * n_rel),
             a_tensor_f32(s, &L->attn_rel_bias),
             a_f32v(s->sinks_buf, 0, n_head),
             a_f32v(s->gate_buf, 0, n_q),
             a_u16v(s->kv_k[il], 0, kv_cap * n_kv),
             a_u16v(s->kv_v[il], 0, kv_cap * n_kv),
             a_f32v(s->attn_scores, 0, n_head * s->max_ctx),
             a_f32v(s->attn_out, 0, n_q));
    }
    pm_barrier(batch);
    prof_mark(bp, 2); batch = *bp; /* 2: conv+clip+kv_store + GQA attn */

    /* Output projection + fused post shortconv + residual (Sprint 28: -48 dispatches & -48 barriers/tok!) */
    if (L->attn_output.type == INK_GGML_Q8_0) {
        int is_blk = (ip % hp->block_size == 0);
        if (is_blk) {
            if (*n_blocks >= INK_MAX_BLOCKS) {
                snprintf(s->error, sizeof s->error, "block count overflow");
                return 1;
            }
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->inp_sa, 0, d),
                 a_f32v(s->blocks, (int64_t)(*n_blocks) * d, d));
            (*n_blocks)++;
        }
        if (s->fast_buf && FL->attn_output.off >= 0) {
            CALL(s, batch, K_DV_Q4_CONV, (d + 3) / 4, a_int(d), a_int(n_q), a_int(is_blk ? 0 : 1),
                 a_fast_u16(s, &FL->attn_output), a_f32v(s->attn_out, 0, n_q),
                 a_tensor_f32(s, &L->attn_post_conv), a_f32v(s->conv_ao[il], 0, d * 3),
                 a_f32v(s->inp_sa, 0, d), a_f32v(s->cur, 0, d));
        } else {
            CALL(s, batch, K_DV_Q8_CONV, (d + 1) / 2, a_int(d), a_int(n_q), a_int(is_blk ? 0 : 1),
                 a_tensor_u16(s, &L->attn_output), a_f32v(s->attn_out, 0, n_q),
                 a_tensor_f32(s, &L->attn_post_conv), a_f32v(s->conv_ao[il], 0, d * 3),
                 a_f32v(s->inp_sa, 0, d), a_f32v(s->cur, 0, d));
        }
    } else {
        if (dispatch_mv(s, batch, &L->attn_output, d, n_q, 0, s->attn_out, 0, s->o_raw, 0)) return 1;
        pm_barrier(batch);
        if (ip % hp->block_size == 0) {
            if (*n_blocks >= INK_MAX_BLOCKS) {
                snprintf(s->error, sizeof s->error, "block count overflow");
                return 1;
            }
            CALL(s, batch, K_CONVOLVE, d, a_int(d), a_int(1), a_int(0),
                 a_f32v(s->o_raw, 0, d), a_tensor_f32(s, &L->attn_post_conv),
                 a_f32v(s->dummy_f32, 0, 64),
                 a_f32v(s->conv_ao[il], 0, d * 3), a_f32v(s->cur, 0, d));
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->inp_sa, 0, d),
                 a_f32v(s->blocks, (int64_t)(*n_blocks) * d, d));
            (*n_blocks)++;
        } else {
            CALL(s, batch, K_CONVOLVE_ADD, d, a_int(d),
                 a_f32v(s->o_raw, 0, d), a_tensor_f32(s, &L->attn_post_conv),
                 a_f32v(s->conv_ao[il], 0, d * 3), a_f32v(s->inp_sa, 0, d), a_f32v(s->cur, 0, d));
        }
    }
    pm_barrier(batch);
    prof_mark(bp, 3); batch = *bp; /* 3: attn_output (Q8_0+conv) */

    /* ffn_inp = cur (skip K_COPY when fused MoE down updates cur in-place -> -48 dispatches/tok!) */
    int moe_dn_inplace = ((s->fast_buf && FL->moe_dn_q4.off >= 0) || L->ffn_down_exps.type == INK_GGML_IQ4_NL);
    if (!moe_dn_inplace) {
        CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->ffn_inp, 0, d));
    }
    if (!has_messages) {
        CALL(s, batch, K_MIX_RMSNORM, 1, a_int(d), a_int(*n_blocks), a_f32(hp->rms_eps),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &L->ffn_res_query),
             a_tensor_f32(s, &L->ffn_norm),
             a_f32v(s->norm_buf, 0, d));
        pm_barrier(batch);
    } else {
        int64_t rank = hp->gate_rank;
        if (L->loop_ctrl_gain[3].type == INK_GGML_F32 && L->loop_ctrl_forget[3].type == INK_GGML_F32 && rank <= 64) {
            /* Sprint 29: Fused K_MIX + K_CTRL_PREP (-24 dispatches & barriers/tok)! */
            CALL(s, batch, K_MIX_CTRL_PREP, 1, a_int(d), a_int(*n_blocks),
                 a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
                 a_f32v(s->cur, 0, d),
                 a_tensor_f32(s, &L->ffn_res_query),
                 a_f32v(s->messages, msg_base + 3 * d, d),
                 a_f32v(s->norm_buf, 0, d),
                 a_f32v(s->ctrl_cat, 0, 2 * d));
            pm_barrier(batch);
            if (dispatch_mv_f16_small(s, batch, &L->loop_ctrl_down[3], &FL->ctrl_down[3], rank, 2 * d, 0, s->ctrl_cat, 0, s->ctrl_gate, 0)) return 1;
            pm_barrier(batch);
            int64_t citems = (d + INK_T - 1) / INK_T;
            if (s->fast_buf && FL->ctrl_gain[3].off >= 0 && FL->ctrl_forget[3].off >= 0) {
                CALL(s, batch, K_CTRL_UP_APPLY_F16, citems, a_int(d), a_int(rank),
                     a_fast_u16(s, &FL->ctrl_gain[3]), a_fast_u16(s, &FL->ctrl_forget[3]),
                     a_f32v(s->ctrl_gate, 0, rank), a_f32v(s->norm_buf, 0, d), a_f32v(s->messages, msg_base + 3 * d, d),
                     a_f32v(s->norm_buf, 0, d));
            } else {
                CALL(s, batch, K_CTRL_UP_APPLY, citems, a_int(d), a_int(rank),
                     a_tensor_f32(s, &L->loop_ctrl_gain[3]), a_tensor_f32(s, &L->loop_ctrl_forget[3]),
                     a_f32v(s->ctrl_gate, 0, rank), a_f32v(s->norm_buf, 0, d), a_f32v(s->messages, msg_base + 3 * d, d),
                     a_f32v(s->norm_buf, 0, d));
            }
            pm_barrier(batch);
            CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
                 a_f32v(s->norm_buf, 0, d), a_tensor_f32(s, &L->ffn_norm), a_f32v(s->norm_buf, 0, d));
            pm_barrier(batch);
        } else {
            CALL(s, batch, K_MIX, 1, a_int(d), a_int(*n_blocks),
                 a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
                 a_f32v(s->cur, 0, d),
                 a_tensor_f32(s, &L->ffn_res_query),
                 a_f32v(s->norm_buf, 0, d));
            pm_barrier(batch);
            if (dispatch_control(s, batch, L, 3, s->norm_buf, 0, s->messages, msg_base + 3 * d, s->norm_buf, 0)) return 1;
            CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
                 a_f32v(s->norm_buf, 0, d), a_tensor_f32(s, &L->ffn_norm), a_f32v(s->norm_buf, 0, d));
            pm_barrier(batch);
        }
    }

    /* MoE router */
    if (dispatch_mv_f16_small(s, batch, &L->ffn_gate_inp, &FL->ffn_gate_inp, hp->n_expert, d, 0, s->norm_buf, 0, s->router_logits, 0)) return 1;
    pm_barrier(batch);
    CALL(s, batch, K_ROUTER, 1, a_int(hp->n_expert), a_int(hp->n_expert_used),
         a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
         a_f32v(s->router_logits, 0, hp->n_expert),
         L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
         a_u32v(s->moe_ids, 0, hp->n_expert_used),
         a_f32v(s->moe_wts, 0, hp->n_expert_used));
    pm_barrier(batch);
    prof_mark(bp, 4); batch = *bp; /* 4: moe prep (mix+norm+router) */

    /* MoE gate_up + SITU */
    int64_t n_ff = hp->n_ff_exp;
    int64_t gu_items = (int64_t)hp->n_expert_used * ((n_ff + 3) / 4);
    if (s->fast_buf && FL->moe_gu_q4.off >= 0) {
        CALL(s, batch, K_MOE_GU_Q4, gu_items, a_int(n_ff), a_int(d),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_fast_u16(s, &FL->moe_gu_q4),
             a_f32v(s->norm_buf, 0, d),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else if (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) {
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
    pm_barrier(batch);
    prof_mark(bp, (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) ? 5 : 6); batch = *bp; /* 5: moe_gu_iq4, 6: moe_gu_q8 */

    /* MoE down + weighted sum + fused post shortconv & residual add (eliminates K_CONVOLVE_ADD + 48 barriers/tok!) */
    int64_t dn_items = (d + INK_D - 1) / INK_D;
    if (s->fast_buf && FL->moe_dn_q4.off >= 0) {
        CALL(s, batch, K_MOE_DOWN_Q4, (d + 3) / 4, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_fast_u16(s, &FL->moe_dn_q4),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_f32v(s->conv_fo[il], 0, d * 3),
             a_f32v(s->cur, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, (L->ffn_down_exps.type == INK_GGML_IQ4_NL) ? 7 : 8); batch = *bp;
    } else if (L->ffn_down_exps.type == INK_GGML_IQ4_NL) {
        CALL(s, batch, K_MOE_DOWN_IQ4, (d + 3) / 4, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_f32v(s->conv_fo[il], 0, d * 3),
             a_f32v(s->cur, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, 7); batch = *bp; /* 7: moe_down_iq4 */
    } else if (L->ffn_down_exps.type == INK_GGML_Q8_0) {
        CALL(s, batch, K_MOE_DOWN_Q8, dn_items, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_f32v(s->conv_fo[il], 0, d * 3),
             a_f32v(s->ffn_inp, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, 8); batch = *bp; /* 8: moe_down_q8 */
    } else {
        snprintf(s->error, sizeof s->error, "unsupported down type %u", L->ffn_down_exps.type);
        return 1;
    }
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

static uint16_t f32_to_f16_host(float x) {
    _Float16 h = (_Float16)x;
    uint16_t u;
    memcpy(&u, &h, 2);
    return u;
}

static float f16_to_f32_host(uint16_t u) {
    _Float16 h;
    memcpy(&h, &u, 2);
    return (float)h;
}

static void convert_f32_to_f16(const uint8_t *src, size_t nbytes, uint8_t *dst) {
    size_t n = nbytes / 4;
    const float *f = (const float *)src;
    uint16_t *o = (uint16_t *)dst;
    for (size_t i = 0; i < n; i++) o[i] = f32_to_f16_host(f[i]);
}

static void convert_f32_to_f16_transpose(const uint8_t *src, int64_t rows, int64_t cols, uint8_t *dst) {
    const float *f = (const float *)src;
    uint16_t *o = (uint16_t *)dst;
    for (int64_t r = 0; r < rows; r++) {
        const float *fr = f + r * cols;
        for (int64_t c = 0; c < cols; c++) {
            o[c * rows + r] = f32_to_f16_host(fr[c]);
        }
    }
}

static void convert_q8_to_q4(const uint8_t *src, size_t nbytes, uint8_t *dst) {
    size_t nb = nbytes / 34;
    for (size_t b = 0; b < nb; b++) {
        const uint8_t *sb = src + b * 34;
        uint8_t *db = dst + b * 18;
        uint16_t d8u;
        memcpy(&d8u, sb, 2);
        float d8 = f16_to_f32_host(d8u);
        const int8_t *qs = (const int8_t *)(sb + 2);
        int max_a = 0;
        int max_v = 0;
        for (int i = 0; i < 32; i++) {
            int a = qs[i] < 0 ? -qs[i] : qs[i];
            if (a > max_a) { max_a = a; max_v = qs[i]; }
        }
        float d4 = (max_a > 0) ? (d8 * (float)max_v / -8.0f) : 0.0f;
        float inv = (d4 != 0.0f) ? (d8 / d4) : 0.0f;
        uint16_t d4u = f32_to_f16_host(d4);
        memcpy(db, &d4u, 2);
        for (int j = 0; j < 16; j++) {
            int q0 = (int)(qs[j] * inv + 8.5f);
            if (q0 < 0) q0 = 0; else if (q0 > 15) q0 = 15;
            int q1 = (int)(qs[j + 16] * inv + 8.5f);
            if (q1 < 0) q1 = 0; else if (q1 > 15) q1 = 15;
            db[2 + j] = (uint8_t)(q0 | (q1 << 4));
        }
    }
}

static void convert_f32_to_q4(const uint8_t *src, size_t nbytes, uint8_t *dst) {
    const float *f = (const float *)src;
    size_t nb = (nbytes / 4) / 32;
    for (size_t b = 0; b < nb; b++) {
        const float *fb = f + b * 32;
        uint8_t *db = dst + b * 18;
        float max_a = 0.0f, max_v = 0.0f;
        for (int i = 0; i < 32; i++) {
            float a = fb[i] < 0.0f ? -fb[i] : fb[i];
            if (a > max_a) { max_a = a; max_v = fb[i]; }
        }
        float d4 = (max_a > 0.0f) ? (max_v / -8.0f) : 0.0f;
        float inv = (d4 != 0.0f) ? (1.0f / d4) : 0.0f;
        uint16_t d4u = f32_to_f16_host(d4);
        memcpy(db, &d4u, 2);
        for (int j = 0; j < 16; j++) {
            int q0 = (int)(fb[j] * inv + 8.5f);
            if (q0 < 0) q0 = 0; else if (q0 > 15) q0 = 15;
            int q1 = (int)(fb[j + 16] * inv + 8.5f);
            if (q1 < 0) q1 = 0; else if (q1 > 15) q1 = 15;
            db[2 + j] = (uint8_t)(q0 | (q1 << 4));
        }
    }
}

typedef struct {
    const uint8_t *src;
    size_t src_bytes;
    uint8_t *dst;
    int kind; /* 0 = f32->f16, 1 = q8->q4, 2 = f32->f16 transpose, 3 = f32->q4 */
    int64_t rows, cols;
} FastJob;

typedef struct {
    const FastJob *jobs;
    int n_jobs;
    int tid;
    int n_threads;
} FastWorkerArg;

static void *fast_worker(void *arg) {
    FastWorkerArg *a = (FastWorkerArg *)arg;
    for (int i = a->tid; i < a->n_jobs; i += a->n_threads) {
        const FastJob *j = &a->jobs[i];
        if (j->kind == 1) convert_q8_to_q4(j->src, j->src_bytes, j->dst);
        else if (j->kind == 2) convert_f32_to_f16_transpose(j->src, j->rows, j->cols, j->dst);
        else if (j->kind == 3) convert_f32_to_q4(j->src, j->src_bytes, j->dst);
        else convert_f32_to_f16(j->src, j->src_bytes, j->dst);
    }
    return NULL;
}

static void build_fast_weights(InkSession *s) {
    const InkHParams *hp = &s->gguf.hp;
    const char *q4_env = getenv("INK_FAST_Q4");
    int do_q4 = (q4_env != NULL && q4_env[0] != '0');
    for (int ip = 0; ip < INK_MAX_SLOTS; ip++) {
        FastLayer *fl = &s->fast_layers[ip];
        fl->attn_q.off = fl->attn_k.off = fl->attn_v.off = fl->attn_output.off = fl->attn_gate.off = fl->attn_rel.off = fl->attn_qrg.off = -1;
        fl->moe_gu_q4.off = fl->moe_dn_q4.off = -1;
        fl->ffn_gate_inp.off = -1;
        fl->attn_sink_proj.off = fl->attn_k_conv_proj.off = fl->attn_v_conv_proj.off = -1;
        for (int c = 0; c < 4; c++) fl->ctrl_down[c].off = fl->ctrl_gain[c].off = fl->ctrl_forget[c].off = -1;
        fl->ctrl_down_qkv_q4.off = fl->ctrl_down3_q4.off = fl->ctrl_gain_qkv.off = fl->ctrl_forget_qkv.off = -1;
    }
    s->fast_output.off = -1;

    FastJob jobs[1024];
    int n_jobs = 0;
    int64_t total_bytes = 0;
    const uint8_t *base = (const uint8_t *)s->gguf.map;

#define ADD_FAST_F16(T, SPAN) do { \
    if ((T).present && (T).type == INK_GGML_F32 && n_jobs < 1024) { \
        int64_t bytes = (int64_t)((T).nbytes / 2); \
        (SPAN).off = total_bytes; (SPAN).words = bytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)total_bytes, 0, 0, 0 }; \
        total_bytes = (total_bytes + bytes + 63) & ~63LL; \
    } \
} while (0)

#define ADD_FAST_F16_T(T, SPAN, ROWS, COLS) do { \
    if ((T).present && (T).type == INK_GGML_F32 && n_jobs < 1024) { \
        int64_t bytes = (int64_t)((T).nbytes / 2); \
        (SPAN).off = total_bytes; (SPAN).words = bytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)total_bytes, 2, (ROWS), (COLS) }; \
        total_bytes = (total_bytes + bytes + 63) & ~63LL; \
    } \
} while (0)

#define ADD_FAST_Q4(T, SPAN) do { \
    if (do_q4 && (T).present && (T).type == INK_GGML_Q8_0 && n_jobs < 1024) { \
        int64_t nb = (int64_t)((T).nbytes / 34); \
        int64_t bytes = nb * 18; \
        (SPAN).off = total_bytes; (SPAN).words = bytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)total_bytes, 1, 0, 0 }; \
        total_bytes = (total_bytes + bytes + 63) & ~63LL; \
    } \
} while (0)

    for (int ip = 0; ip < (int)hp->n_phys; ip++) {
        const InkLayerTensors *L = &s->gguf.layers[ip];
        FastLayer *FL = &s->fast_layers[ip];
        ADD_FAST_F16(L->ffn_gate_inp, FL->ffn_gate_inp);
        ADD_FAST_F16(L->attn_sink_proj, FL->attn_sink_proj);
        ADD_FAST_F16(L->attn_k_conv_proj, FL->attn_k_conv_proj);
        ADD_FAST_F16(L->attn_v_conv_proj, FL->attn_v_conv_proj);
        if (L->loop_ctrl_down[0].present && L->loop_ctrl_down[0].type == INK_GGML_F32 &&
            L->loop_ctrl_down[1].present && L->loop_ctrl_down[1].type == INK_GGML_F32 &&
            L->loop_ctrl_down[2].present && L->loop_ctrl_down[2].type == INK_GGML_F32 && n_jobs + 3 <= 1024) {
            int64_t b0 = (int64_t)(L->loop_ctrl_down[0].nbytes / 2);
            int64_t b1 = (int64_t)(L->loop_ctrl_down[1].nbytes / 2);
            int64_t b2 = (int64_t)(L->loop_ctrl_down[2].nbytes / 2);
            FL->ctrl_down_qkv_q4.off = total_bytes;
            FL->ctrl_down_qkv_q4.words = (b0 + b1 + b2) / 2;
            FL->ctrl_down[0].off = total_bytes; FL->ctrl_down[0].words = b0 / 2;
            FL->ctrl_down[1].off = total_bytes + b0; FL->ctrl_down[1].words = b1 / 2;
            FL->ctrl_down[2].off = total_bytes + b0 + b1; FL->ctrl_down[2].words = b2 / 2;
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[0].offset, (size_t)L->loop_ctrl_down[0].nbytes, (uint8_t *)(uintptr_t)(total_bytes), 0, 0, 0 };
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[1].offset, (size_t)L->loop_ctrl_down[1].nbytes, (uint8_t *)(uintptr_t)(total_bytes + b0), 0, 0, 0 };
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[2].offset, (size_t)L->loop_ctrl_down[2].nbytes, (uint8_t *)(uintptr_t)(total_bytes + b0 + b1), 0, 0, 0 };
            total_bytes = (total_bytes + b0 + b1 + b2 + 63) & ~63LL;
            ADD_FAST_F16(L->loop_ctrl_down[3], FL->ctrl_down[3]);
        } else {
            for (int c = 0; c < 4; c++) ADD_FAST_F16(L->loop_ctrl_down[c], FL->ctrl_down[c]);
        }
        if (L->loop_ctrl_gain[0].present && L->loop_ctrl_gain[0].type == INK_GGML_F32 &&
            L->loop_ctrl_gain[1].present && L->loop_ctrl_gain[1].type == INK_GGML_F32 &&
            L->loop_ctrl_gain[2].present && L->loop_ctrl_gain[2].type == INK_GGML_F32 && n_jobs + 6 <= 1024) {
            int64_t bg = (int64_t)(L->loop_ctrl_gain[0].nbytes / 2);
            FL->ctrl_gain_qkv.off = total_bytes; FL->ctrl_gain_qkv.words = (3 * bg) / 2;
            for (int c = 0; c < 3; c++) {
                FL->ctrl_gain[c].off = total_bytes + c * bg; FL->ctrl_gain[c].words = bg / 2;
                jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_gain[c].offset, (size_t)L->loop_ctrl_gain[c].nbytes, (uint8_t *)(uintptr_t)(total_bytes + c * bg), 2, hp->n_embd, hp->gate_rank };
            }
            total_bytes = (total_bytes + 3 * bg + 63) & ~63LL;
            FL->ctrl_forget_qkv.off = total_bytes; FL->ctrl_forget_qkv.words = (3 * bg) / 2;
            for (int c = 0; c < 3; c++) {
                FL->ctrl_forget[c].off = total_bytes + c * bg; FL->ctrl_forget[c].words = bg / 2;
                jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_forget[c].offset, (size_t)L->loop_ctrl_forget[c].nbytes, (uint8_t *)(uintptr_t)(total_bytes + c * bg), 2, hp->n_embd, hp->gate_rank };
            }
            total_bytes = (total_bytes + 3 * bg + 63) & ~63LL;
            ADD_FAST_F16_T(L->loop_ctrl_gain[3], FL->ctrl_gain[3], hp->n_embd, hp->gate_rank);
            ADD_FAST_F16_T(L->loop_ctrl_forget[3], FL->ctrl_forget[3], hp->n_embd, hp->gate_rank);
        } else {
            for (int c = 0; c < 4; c++) {
                ADD_FAST_F16_T(L->loop_ctrl_gain[c], FL->ctrl_gain[c], hp->n_embd, hp->gate_rank);
                ADD_FAST_F16_T(L->loop_ctrl_forget[c], FL->ctrl_forget[c], hp->n_embd, hp->gate_rank);
            }
        }
        if (do_q4 && L->attn_q.present && L->attn_q.type == INK_GGML_Q8_0 &&
            L->attn_rel.present && L->attn_rel.type == INK_GGML_Q8_0 &&
            L->attn_gate.present && L->attn_gate.type == INK_GGML_Q8_0 && n_jobs + 3 <= 1024) {
            int64_t bq = (int64_t)(L->attn_q.nbytes / 34) * 18;
            int64_t br = (int64_t)(L->attn_rel.nbytes / 34) * 18;
            int64_t bg = (int64_t)(L->attn_gate.nbytes / 34) * 18;
            FL->attn_qrg.off = total_bytes;
            FL->attn_qrg.words = (bq + br + bg) / 2;
            FL->attn_q.off = total_bytes;           FL->attn_q.words = bq / 2;
            FL->attn_rel.off = total_bytes + bq;    FL->attn_rel.words = br / 2;
            FL->attn_gate.off = total_bytes + bq + br; FL->attn_gate.words = bg / 2;
            jobs[n_jobs++] = (FastJob){ base + L->attn_q.offset, (size_t)L->attn_q.nbytes, (uint8_t *)(uintptr_t)(total_bytes), 1, 0, 0 };
            jobs[n_jobs++] = (FastJob){ base + L->attn_rel.offset, (size_t)L->attn_rel.nbytes, (uint8_t *)(uintptr_t)(total_bytes + bq), 1, 0, 0 };
            jobs[n_jobs++] = (FastJob){ base + L->attn_gate.offset, (size_t)L->attn_gate.nbytes, (uint8_t *)(uintptr_t)(total_bytes + bq + br), 1, 0, 0 };
            total_bytes = (total_bytes + bq + br + bg + 63) & ~63LL;
        } else {
            ADD_FAST_Q4(L->attn_q, FL->attn_q);
            ADD_FAST_Q4(L->attn_rel, FL->attn_rel);
            ADD_FAST_Q4(L->attn_gate, FL->attn_gate);
        }
        ADD_FAST_Q4(L->attn_k, FL->attn_k);
        ADD_FAST_Q4(L->attn_v, FL->attn_v);
        ADD_FAST_Q4(L->attn_output, FL->attn_output);
        ADD_FAST_Q4(L->ffn_gate_up_exps, FL->moe_gu_q4);
        ADD_FAST_Q4(L->ffn_down_exps, FL->moe_dn_q4);
    }
    ADD_FAST_Q4(s->gguf.output, s->fast_output);
#undef ADD_FAST_F16
#undef ADD_FAST_Q4

    if (total_bytes == 0) return;
    if (pm_alloc_unset((size_t)total_bytes, &s->fast_buf) != PM_OK) {
        s->fast_buf = NULL;
        return;
    }
    uint8_t *dst_base = (uint8_t *)pm_contents(s->fast_buf);
    for (int i = 0; i < n_jobs; i++) jobs[i].dst = dst_base + (uintptr_t)jobs[i].dst;
    const int NT = 8;
    pthread_t th[8];
    FastWorkerArg args[8];
    for (int i = 0; i < NT; i++) {
        args[i] = (FastWorkerArg){ jobs, n_jobs, i, NT };
        pthread_create(&th[i], NULL, fast_worker, &args[i]);
    }
    for (int i = 0; i < NT; i++) pthread_join(th[i], NULL);
    pm_mark_readonly(s->fast_buf);
}

static int64_t ink_host_ram_mb(void) {
    const char *ov = getenv("INK_TEST_RAM_MB");
    if (ov && *ov) {
        long long v = atoll(ov);
        if (v > 0) return (int64_t)v;
    }
    uint64_t mem = 0;
    size_t sz = sizeof(mem);
    if (sysctlbyname("hw.memsize", &mem, &sz, NULL, 0) == 0 && mem > 0) {
        return (int64_t)(mem / (1024ULL * 1024ULL));
    }
    return 0;
}

int ink_open(const char *gguf_path, const char *metal_path, int max_ctx, InkSession **out, char *err, size_t err_bytes) {
    return ink_open_ex(gguf_path, metal_path, max_ctx, 0, out, err, err_bytes);
}

int ink_open_ex(const char *gguf_path, const char *metal_path, int max_ctx,
                int loop_count_override, InkSession **out, char *err, size_t err_bytes) {
    *out = NULL;
    InkSession *s = (InkSession *)calloc(1, sizeof *s);
    if (!s) { snprintf(err, err_bytes, "out of memory"); return 1; }

    if (ink_gguf_open(gguf_path, &s->gguf, err, err_bytes) != 0) {
        free(s); return 1;
    }
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
    if (s->n_slots > INK_MAX_SLOTS) {
        snprintf(err, err_bytes, "n_slots %d > %d", s->n_slots, INK_MAX_SLOTS);
        ink_close(s); return 1;
    }

    PmDeviceInfo info;
    if (pm_open(&info) != PM_OK) {
        snprintf(err, err_bytes, "pm_open: %s", pm_log());
        ink_close(s); return 1;
    }

    /* Unified-memory residency guardrail (>= 32 GB Apple Silicon floor + exact KV & scratch budget). */
    int64_t ram_mb = ink_host_ram_mb();
    if (ram_mb <= 0 && info.working_set_bytes > 0) {
        ram_mb = (int64_t)(info.working_set_bytes / (1024ULL * 1024ULL));
    }
    s->prefill_b = (s->max_ctx <= 8192) ? INK_PREFILL_B
                 : (s->max_ctx <= 16384) ? 2048
                 : (ram_mb > 0 && ram_mb <= 40960) ? 512 : 1024;

    size_t B = (size_t)s->prefill_b;
    size_t d = (size_t)hp->n_embd;
    size_t n_q = (size_t)hp->n_head * (size_t)hp->head_dim;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    int full_slots = 0, swa_slots = 0;
    size_t kv_bytes = 0;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
        int swa_cap = hp->swa + s->prefill_b;
        int cap = hp->is_swa[ip] ? (swa_cap < s->max_ctx ? swa_cap : s->max_ctx) : s->max_ctx;
        if (hp->is_swa[ip]) swa_slots++; else full_slots++;
        kv_bytes += (size_t)cap * n_kv * 4 + (2 * n_kv + 2 * d) * 3 * 4;
    }
    size_t attn_prefill_bytes = B * (size_t)hp->n_head * (size_t)s->max_ctx * 2;
    size_t attn_decode_bytes = (size_t)hp->n_head * (size_t)s->max_ctx * 4;
    size_t attn_bytes = attn_prefill_bytes > attn_decode_bytes ? attn_prefill_bytes : attn_decode_bytes;
    size_t scratch_bytes = attn_bytes + (size_t)44 * 1024 * 1024 * 4 +
                           B * (size_t)(48 * d + 6 * n_q + 4 * n_kv + 3 * (size_t)hp->n_expert_used * (size_t)hp->n_ff_exp) * 4;
    int64_t os_reserve_mb = 4096;
    int64_t weights_mb = (int64_t)((s->gguf.map_size + 1048575ULL) / 1048576ULL);
    int64_t kv_mb = (int64_t)((kv_bytes + 1048575ULL) / 1048576ULL);
    int64_t scratch_mb = (int64_t)((scratch_bytes + 1048575ULL) / 1048576ULL);
    int64_t total_mb = os_reserve_mb + weights_mb + kv_mb + scratch_mb;
    int64_t fixed_mb = os_reserve_mb + weights_mb + scratch_mb;
    int64_t max_ctx_fit = 0;
    if (ram_mb > fixed_mb && full_slots > 0) {
        int64_t swa_ring_bytes = (int64_t)swa_slots * (int64_t)(hp->swa + s->prefill_b) * (int64_t)(n_kv * 4);
        int64_t avail_full = (ram_mb - fixed_mb) * 1048576LL - swa_ring_bytes;
        if (avail_full > 0) max_ctx_fit = avail_full / ((int64_t)full_slots * (int64_t)(n_kv * 4));
    }
    s->budget = (InkMemoryBudget){
        .device_ram_mb = ram_mb,
        .os_reserve_mb = os_reserve_mb,
        .weights_mb = weights_mb,
        .kv_cache_mb = kv_mb,
        .scratchpad_mb = scratch_mb,
        .total_mb = total_mb,
        .headroom_mb = (ram_mb > total_mb) ? (ram_mb - total_mb) : 0,
        .max_context_len = max_ctx_fit,
        .full_slots = full_slots,
        .swa_slots = swa_slots,
    };
    if (ram_mb > 0 && ram_mb < 32768) {
        snprintf(err, err_bytes,
                 "insufficient unified memory: %lld MB detected (requires >= 32768 MB for Inkling 2 Mini; weights=%lld MB)",
                 (long long)ram_mb, (long long)weights_mb);
        ink_close(s); return 1;
    }
    if (ram_mb > 0 && total_mb > ram_mb) {
        snprintf(err, err_bytes,
                 "context %d exceeds unified memory budget (%lld MB required > %lld MB RAM; max_ctx=%lld)",
                 s->max_ctx, (long long)total_mb, (long long)ram_mb, (long long)max_ctx_fit);
        ink_close(s); return 1;
    }

    size_t metal_len = 0;
    char *metal_src = read_entire_file(metal_path, &metal_len);
    if (!metal_src) {
        snprintf(err, err_bytes, "cannot read metal source %s", metal_path);
        ink_close(s); return 1;
    }
    double compile_s = 0.0;
    PmStatus cs = pm_compile_fast(metal_src, metal_len, "inkling", &s->module, &compile_s);
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

    if (alloc_buf(s, &s->cur, B * d * 4) ||
        alloc_buf(s, &s->inp_sa, B * d * 4) ||
        alloc_buf(s, &s->ffn_inp, B * d * 4) ||
        alloc_buf(s, &s->prelude, B * d * 4) ||
        alloc_buf(s, &s->bank_out, B * d * 4) ||
        alloc_buf(s, &s->blocks, INK_MAX_BLOCKS * B * d * 4) ||
        alloc_buf(s, &s->prelude_blk, INK_MAX_BLOCKS * B * d * 4) ||
        alloc_buf(s, &s->bank_blk, INK_MAX_BLOCKS * B * d * 4) ||
        alloc_buf(s, &s->mix_out, 4 * B * d * 4) ||
        alloc_buf(s, &s->messages, 4 * B * d * 4) ||
        alloc_buf(s, &s->inp_q, B * d * 4) ||
        alloc_buf(s, &s->inp_k, B * d * 4) ||
        alloc_buf(s, &s->inp_v, B * d * 4) ||
        alloc_buf(s, &s->ctrl_cat, 3 * 2 * B * d * 4) ||
        alloc_buf(s, &s->ctrl_gate, 3 * B * (size_t)hp->gate_rank * 4) ||
        alloc_buf(s, &s->ctrl_gain, d * 4) ||
        alloc_buf(s, &s->ctrl_forget, d * 4) ||
        alloc_buf(s, &s->q_buf, B * n_q * 4) ||
        alloc_buf(s, &s->k_raw, B * n_kv * 4) ||
        alloc_buf(s, &s->k_buf, B * n_kv * 4) ||
        alloc_buf(s, &s->v_raw, B * n_kv * 4) ||
        alloc_buf(s, &s->v_buf, B * n_kv * 4) ||
        alloc_buf(s, &s->r_buf, B * (size_t)hp->n_head * (size_t)hp->rel_len * 4) ||
        alloc_buf(s, &s->dyn_buf, 2 * B * (size_t)hp->n_head_kv * (size_t)hp->conv_len * 4) ||
        alloc_buf(s, &s->sinks_buf, B * (size_t)hp->n_head * 4) ||
        alloc_buf(s, &s->gate_buf, B * n_q * 4) ||
        alloc_buf(s, &s->attn_scores, attn_bytes) ||
        alloc_buf(s, &s->attn_out, B * n_q * 4) ||
        alloc_buf(s, &s->o_raw, B * d * 4) ||
        alloc_buf(s, &s->norm_buf, B * d * 4) ||
        alloc_buf(s, &s->router_logits, B * (size_t)hp->n_expert * 4) ||
        alloc_buf(s, &s->moe_ids, B * (size_t)hp->n_expert_used * 4) ||
        alloc_buf(s, &s->moe_wts, B * (size_t)hp->n_expert_used * 4) ||
        alloc_buf(s, &s->moe_hid, B * (size_t)hp->n_expert_used * (size_t)hp->n_ff_exp * 4) ||
        alloc_buf(s, &s->moe_gu, B * (size_t)hp->n_expert_used * 2 * (size_t)hp->n_ff_exp * 4) ||
        alloc_buf(s, &s->moe_out, B * d * 4) ||
        alloc_buf(s, &s->moe_slot_out, B * (size_t)hp->n_expert_used * d * 4) ||
        alloc_buf(s, &s->exp_toks, (size_t)hp->n_expert * INK_PREFILL_B * 4) ||
        alloc_buf(s, &s->exp_slots, (size_t)hp->n_expert * INK_PREFILL_B * 4) ||
        alloc_buf(s, &s->exp_wts, (size_t)hp->n_expert * INK_PREFILL_B * 4) ||
        alloc_buf(s, &s->exp_cnt, (size_t)hp->n_expert * 4) ||
        alloc_buf(s, &s->exp_off, (size_t)hp->n_expert * 4) ||
        alloc_buf(s, &s->row_dst, B * (size_t)hp->n_expert_used * 4) ||
        alloc_buf(s, &s->row_wt, B * (size_t)hp->n_expert_used * 4) ||
        alloc_buf(s, &s->tile_exp, ((size_t)hp->n_expert + INK_PREFILL_B / 2 + 16) * 4) ||
        alloc_buf(s, &s->tile_m0, ((size_t)hp->n_expert + INK_PREFILL_B / 2 + 16) * 4) ||
        alloc_buf(s, &s->tile_cnt, 16 * 4) ||
        alloc_buf(s, &s->w_bf16, (size_t)44 * 1024 * 1024 * 4) ||
        alloc_buf(s, &s->tok_ids_buf, B * 4) ||
        alloc_buf(s, &s->tau_buf, B * 4) ||
        alloc_buf(s, &s->logits, (size_t)hp->n_vocab * 4) ||
        alloc_buf(s, &s->argmax_vals, 64 * 4) ||
        alloc_buf(s, &s->argmax_ids, 64 * 4) ||
        alloc_buf(s, &s->argmax_out, 4) ||
        alloc_buf(s, &s->dummy_f32, 64 * 4)) {
        snprintf(err, err_bytes, "%s", s->error);
        ink_close(s); return 1;
    }

    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
        /* SWA ring buffer must hold at least (swa + prefill_b) entries so K_KV_STORE_BATCH never
         * overwrites earlier keys in [k_start, pos + nb) before MMA prefill attention reads them. */
        int swa_cap = hp->swa + s->prefill_b;
        int cap = hp->is_swa[ip] ? (swa_cap < s->max_ctx ? swa_cap : s->max_ctx) : s->max_ctx;
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

    build_fast_weights(s);

    s->pos = 0;
    *out = s;
    return 0;
}

void ink_close(InkSession *s) {
    if (!s) return;
    for (int i = 0; i < 2; i++) {
        free(s->snaps[i].tokens);
        free(s->snaps[i].conv_state);
        free(s->snaps[i].logits);
    }
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
    pm_free(s->router_logits); pm_free(s->moe_ids); pm_free(s->moe_wts); pm_free(s->moe_hid); pm_free(s->moe_gu); pm_free(s->moe_out);
    pm_free(s->moe_slot_out); pm_free(s->exp_toks); pm_free(s->exp_slots); pm_free(s->exp_wts); pm_free(s->exp_cnt);
    pm_free(s->exp_off); pm_free(s->row_dst); pm_free(s->row_wt);
    pm_free(s->tile_exp); pm_free(s->tile_m0); pm_free(s->tile_cnt); pm_free(s->w_bf16);
    pm_free(s->tok_ids_buf); pm_free(s->tau_buf);
    pm_free(s->logits); pm_free(s->argmax_vals); pm_free(s->argmax_ids); pm_free(s->argmax_out); pm_free(s->dummy_f32);
    pm_free(s->fast_buf);
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
    s->max_pos_written = 0;
    s->last_hidden_off = 0;
    s->snaps[0].valid = 0;
    s->snaps[1].valid = 0;
}

void ink_prefix_save(InkSession *s, const int32_t *tokens, int n_tokens, int save_logits) {
    if (!s || !tokens || n_tokens <= 0 || n_tokens != s->pos || n_tokens > s->max_ctx) return;
    const InkHParams *hp = &s->gguf.hp;
    size_t d = (size_t)hp->n_embd;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    size_t conv_per_slot = (2 * n_kv + 2 * d) * 3;
    size_t conv_elems = (size_t)s->n_slots * conv_per_slot;
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
    for (int il = 0; il < s->n_slots; il++) {
        memcpy(dst, pm_contents(s->conv_k[il]), n_kv * 3 * sizeof(float)); dst += n_kv * 3;
        memcpy(dst, pm_contents(s->conv_v[il]), n_kv * 3 * sizeof(float)); dst += n_kv * 3;
        memcpy(dst, pm_contents(s->conv_ao[il]), d * 3 * sizeof(float));   dst += d * 3;
        memcpy(dst, pm_contents(s->conv_fo[il]), d * 3 * sizeof(float));   dst += d * 3;
    }
    if (save_logits) {
        memcpy(s->snaps[slot].logits, pm_contents(s->logits), (size_t)hp->n_vocab * sizeof(float));
    }
    s->snaps[slot].pos = n_tokens;
    s->snaps[slot].has_logits = save_logits ? 1 : 0;
    s->snaps[slot].valid = 1;
}

int ink_prefix_restore(InkSession *s, const int32_t *tokens, int n_tokens) {
    if (!s || !tokens || n_tokens <= 0) { ink_reset(s); return 0; }
    const InkHParams *hp = &s->gguf.hp;
    int swa_cap = s->max_ctx;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
        if (hp->is_swa[ip] && s->kv_cap[il] < swa_cap) swa_cap = s->kv_cap[il];
    }
    int best = -1;
    for (int i = 0; i < 2; i++) {
        if (!s->snaps[i].valid) continue;
        int sp = s->snaps[i].pos;
        if (sp <= 0 || sp > n_tokens) continue;
        if (sp == n_tokens && !s->snaps[i].has_logits) continue;
        /* Verify SWA ring-buffer keys [oldest_needed .. sp-1] were never overwritten by writes up to max_pos_written. */
        int oldest_needed = (sp >= hp->swa) ? (sp - (int)hp->swa + 1) : 0;
        if (s->max_pos_written - oldest_needed > swa_cap) continue;
        if (memcmp(s->snaps[i].tokens, tokens, (size_t)sp * sizeof(int32_t)) != 0) continue;
        if (best < 0 || sp > s->snaps[best].pos) best = i;
    }
    if (best < 0) {
        ink_reset(s);
        return 0;
    }
    size_t d = (size_t)hp->n_embd;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    const float *src = s->snaps[best].conv_state;
    for (int il = 0; il < s->n_slots; il++) {
        memcpy(pm_contents(s->conv_k[il]), src, n_kv * 3 * sizeof(float)); src += n_kv * 3;
        memcpy(pm_contents(s->conv_v[il]), src, n_kv * 3 * sizeof(float)); src += n_kv * 3;
        memcpy(pm_contents(s->conv_ao[il]), src, d * 3 * sizeof(float));   src += d * 3;
        memcpy(pm_contents(s->conv_fo[il]), src, d * 3 * sizeof(float));   src += d * 3;
    }
    int sp = s->snaps[best].pos;
    if (sp == n_tokens && s->snaps[best].has_logits) {
        memcpy(pm_contents(s->logits), s->snaps[best].logits, (size_t)hp->n_vocab * sizeof(float));
    }
    s->argmax_valid = 0;
    s->pos = sp;
    s->max_pos_written = sp;
    for (int i = 0; i < 2; i++) {
        if (i != best && s->snaps[i].valid && s->snaps[i].pos > sp) s->snaps[i].valid = 0;
    }
    return sp;
}

static int enqueue_step(InkSession *s, PmBatch *bp, int32_t token_id, int compute_logits) {
    PmBatch batch = *bp;
    const InkHParams *hp = &s->gguf.hp;
    int do_prof = (getenv("INK_PROFILE") != NULL && s->total_steps == 35);
    if (do_prof) { prof_active = 1; memset(prof_ms, 0, sizeof prof_ms); }
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
    pm_barrier(batch);

    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;

    /* 2. Prelude layers [0 .. start) */
    for (int il = 0; il < start; il++) {
        if (dispatch_layer(s, bp, il, il, &n_blocks, 0, -1, tau) != 0) return 1;
    }
    batch = *bp;

    /* Save prelude output and prelude blocks (concurrent) */
    CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->prelude, 0, d));
    int prelude_n_blocks = n_blocks;
    if (prelude_n_blocks > 0) {
        int64_t pn = (int64_t)prelude_n_blocks * d;
        CALL(s, batch, K_COPY, (pn + INK_NT - 1) / INK_NT, a_int(pn),
             a_f32v(s->blocks, 0, pn),
             a_f32v(s->prelude_blk, 0, pn));
    }
    pm_barrier(batch);

    /* 3. Looped core passes [0 .. loop_count) over physical layers [start .. end) */
    for (int pass = 0; pass < hp->loop_count; pass++) {
        int bank_n_blocks = 0;
        if (pass > 0) {
            int src_off = prelude_n_blocks + 1;
            bank_n_blocks = n_blocks - src_off;
            if (bank_n_blocks < 0) bank_n_blocks = 0;
            if (bank_n_blocks > 0) {
                int64_t bn = (int64_t)bank_n_blocks * d;
                CALL(s, batch, K_COPY, (bn + INK_NT - 1) / INK_NT, a_int(bn),
                     a_f32v(s->blocks, (int64_t)src_off * d, bn),
                     a_f32v(s->bank_blk, 0, bn));
            }
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->bank_out, 0, d));
            pm_barrier(batch);
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->prelude, 0, d), a_f32v(s->cur, 0, d));
            n_blocks = prelude_n_blocks;
            if (prelude_n_blocks > 0) {
                int64_t pn = (int64_t)prelude_n_blocks * d;
                CALL(s, batch, K_COPY, (pn + INK_NT - 1) / INK_NT, a_int(pn),
                     a_f32v(s->prelude_blk, 0, pn),
                     a_f32v(s->blocks, 0, pn));
            }
            /* Sprint 32: Precompute ALL 12 looped layers' messages concurrently in this wave (48 TGs, 0 extra barriers)! */
            if ((end - start) <= s->prefill_b) {
                for (int ip = start; ip < end; ip++) {
                    const InkLayerTensors *ML = &s->gguf.layers[ip];
                    CALL(s, batch, K_MIX, 4, a_int(d), a_int(bank_n_blocks),
                         a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * d),
                         a_f32v(s->bank_out, 0, d),
                         a_tensor_f32(s, &ML->loop_msg_query),
                         a_f32v(s->messages, (int64_t)(ip - start) * 4 * d, 4 * d));
                }
            }
            pm_barrier(batch);
        }
        for (int ip = start; ip < end; ip++) {
            int il = (pass == 0) ? ip : (int)hp->n_phys + (pass - 1) * (end - start) + (ip - start);
            if (pass == 0) {
                if (dispatch_layer(s, bp, il, ip, &n_blocks, 0, -1, tau) != 0) return 1;
            } else {
                if (dispatch_layer(s, bp, il, ip, &n_blocks, 1, bank_n_blocks, tau) != 0) return 1;
            }
            batch = *bp;
        }
    }

    /* 4. Coda layers [end .. n_phys) */
    for (int il = end; il < hp->n_phys; il++) {
        if (dispatch_layer(s, bp, il, il, &n_blocks, 0, -1, tau) != 0) return 1;
    }
    batch = *bp;

    /* 5. Output mix -> output_norm -> lm_head -> logit softcap -> GPU argmax */
    if (compute_logits) {
        prof_mark(bp, 9); batch = *bp;
        CALL(s, batch, K_MIX_RMSNORM, 1, a_int(d), a_int(n_blocks), a_f32(hp->rms_eps),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &s->gguf.out_res_query),
             a_tensor_f32(s, &s->gguf.out_norm),
             a_f32v(s->norm_buf, 0, d));
        pm_barrier(batch);
        int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
        if (dispatch_mv_q4(s, batch, &s->gguf.output, &s->fast_output, nv, d, 0, s->norm_buf, 0, s->logits, 0) != 0) return 1;
        if (hp->logit_softcap > 0.0f) {
            pm_barrier(batch);
            CALL(s, batch, K_SOFTCAP, (nv + INK_NT - 1) / INK_NT, a_int(nv),
                 a_f32(hp->logit_softcap), a_f32v(s->logits, 0, nv));
        }
        pm_barrier(batch);
        CALL(s, batch, K_ARGMAX_S1, 64, a_int(nv),
             a_f32v(s->logits, 0, nv), a_f32v(s->argmax_vals, 0, 64), a_u32v(s->argmax_ids, 0, 64));
        pm_barrier(batch);
        CALL(s, batch, K_ARGMAX_S2, 1, a_int(64),
             a_f32v(s->argmax_vals, 0, 64), a_u32v(s->argmax_ids, 0, 64), a_u32v(s->argmax_out, 0, 1));
        s->argmax_valid = 1;
        s->last_hidden_off = 0;
        prof_mark(bp, 10); batch = *bp;
    } else {
        s->argmax_valid = 0;
    }
    if (do_prof) {
        prof_active = 0;
        double sum = 0;
        for (int i = 0; i <= 10; i++) sum += prof_ms[i];
        fprintf(stderr, "\n[INK_PROFILE step=%lld total_gpu=%.2f ms]\n"
                "  0 mix+ctrl+norm: %6.2f ms (%4.1f%%)\n"
                "  1 attn_in_projs: %6.2f ms (%4.1f%%)\n"
                "  2 conv+kv+attn : %6.2f ms (%4.1f%%)\n"
                "  3 attn_out_conv: %6.2f ms (%4.1f%%)\n"
                "  4 moe_prep+rtr : %6.2f ms (%4.1f%%)\n"
                "  5 moe_gu_iq4   : %6.2f ms (%4.1f%%)\n"
                "  6 moe_gu_q8    : %6.2f ms (%4.1f%%)\n"
                "  7 moe_down_iq4 : %6.2f ms (%4.1f%%)\n"
                "  8 moe_down_q8  : %6.2f ms (%4.1f%%)\n"
                "  9 loop_copy_etc: %6.2f ms (%4.1f%%)\n"
                " 10 lm_head+cap  : %6.2f ms (%4.1f%%)\n",
                (long long)s->total_steps, sum,
                prof_ms[0], 100*prof_ms[0]/sum, prof_ms[1], 100*prof_ms[1]/sum,
                prof_ms[2], 100*prof_ms[2]/sum, prof_ms[3], 100*prof_ms[3]/sum,
                prof_ms[4], 100*prof_ms[4]/sum, prof_ms[5], 100*prof_ms[5]/sum,
                prof_ms[6], 100*prof_ms[6]/sum, prof_ms[7], 100*prof_ms[7]/sum,
                prof_ms[8], 100*prof_ms[8]/sum, prof_ms[9], 100*prof_ms[9]/sum,
                prof_ms[10], 100*prof_ms[10]/sum);
    }

    s->pos++;
    if (s->pos > s->max_pos_written) s->max_pos_written = s->pos;
    s->total_steps++;
    return 0;
}

static int dispatch_mm(InkSession *s, PmBatch batch, const InkTensor *w, int64_t rows, int64_t cols, int64_t nb,
                       PmBuffer x_buf, int64_t x_off, PmBuffer out_buf, int64_t out_off) {
    int64_t t_tiles = (nb + 63) / 64;
    int64_t r_tiles = (rows + 31) / 32;
    int64_t items = t_tiles * r_tiles;
    if (w->type == INK_GGML_Q8_0) {
        CALL(s, batch, K_MM_Q8, items, a_int(rows), a_int(cols), a_int(nb),
             a_tensor_u16(s, w), a_f32v(x_buf, x_off, nb * cols), a_f32v(out_buf, out_off, nb * rows));
    } else if (w->type == INK_GGML_F32) {
        if (nb <= 48 && rows <= 256) {
            int64_t m4_items = ((nb + 3) / 4) * ((rows + 3) / 4);
            CALL(s, batch, K_MM_F32_M4, m4_items, a_int(rows), a_int(cols), a_int(nb),
                 a_tensor_f32(s, w), a_f32v(x_buf, x_off, nb * cols), a_f32v(out_buf, out_off, nb * rows));
        } else {
            CALL(s, batch, K_MM_F32, items, a_int(rows), a_int(cols), a_int(nb),
                 a_tensor_f32(s, w), a_f32v(x_buf, x_off, nb * cols), a_f32v(out_buf, out_off, nb * rows));
        }
    } else {
        snprintf(s->error, sizeof s->error, "unsupported weight type %u for batched matmul", w->type);
        return 1;
    }
    return 0;
}

static int dispatch_dequant_q8_bf16(InkSession *s, PmBatch batch, const InkTensor *w, int64_t rows, int64_t cols, int64_t bf16_off) {
    CALL(s, batch, K_DEQUANT_Q8_BF16, (rows + 3) / 4, a_int(rows), a_int(cols),
         a_tensor_u16(s, w), a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)));
    return 0;
}

static int dispatch_dequant_q4_or_q8_bf16(InkSession *s, PmBatch batch, const InkTensor *w, const FastSpan *fs, int64_t rows, int64_t cols, int64_t bf16_off) {
    if (s->fast_buf && fs && fs->off >= 0) {
        CALL(s, batch, K_DEQUANT_Q4_BF16, (rows + 7) / 8, a_int(rows), a_int(cols),
             a_fast_u16(s, fs), a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)));
        return 0;
    }
    return dispatch_dequant_q8_bf16(s, batch, w, rows, cols, bf16_off);
}

static int dispatch_dequant_f32_bf16(InkSession *s, PmBatch batch, const InkTensor *w, int64_t rows, int64_t cols, int64_t bf16_off) {
    CALL(s, batch, K_DEQUANT_F32_BF16, (rows + 3) / 4, a_int(rows), a_int(cols),
         a_tensor_f32(s, w), a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)));
    return 0;
}

static int dispatch_f32_to_bf16(InkSession *s, PmBatch batch, int64_t n, PmBuffer x_buf, int64_t x_off, int64_t bf16_off) {
    CALL(s, batch, K_F32_TO_BF16, (n + 1023) / 1024, a_int(n),
         a_f32v(x_buf, x_off, n), a_u16v(s->moe_slot_out, bf16_off, n));
    return 0;
}

static int dispatch_mm_bf16(InkSession *s, PmBatch batch, int64_t rows, int64_t cols, int64_t nb, int64_t bf16_off,
                            PmBuffer x_bf16, int64_t x_off, PmBuffer out_buf, int64_t out_off) {
    if (nb >= 33) {
        int64_t t_tiles = (nb + 63) / 64;
        int64_t r_tiles = (rows + 127) / 128;
        CALL(s, batch, K_MM_BF16, t_tiles * r_tiles, a_int(rows), a_int(cols), a_int(nb),
             a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)),
             a_u16v(x_bf16, x_off, nb * cols),
             a_f32v(out_buf, out_off, nb * rows));
    } else if (nb >= 17) {
        int64_t t_tiles = (nb + 31) / 32;
        int64_t r_tiles = (rows + 63) / 64;
        CALL(s, batch, K_MM_BF16_32, t_tiles * r_tiles, a_int(rows), a_int(cols), a_int(nb),
             a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)),
             a_u16v(x_bf16, x_off, nb * cols),
             a_f32v(out_buf, out_off, nb * rows));
    } else {
        int64_t t_tiles = (nb + 15) / 16;
        int64_t r_tiles = (rows + 63) / 64;
        CALL(s, batch, K_MM_BF16_16, t_tiles * r_tiles, a_int(rows), a_int(cols), a_int(nb),
             a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)),
             a_u16v(x_bf16, x_off, nb * cols),
             a_f32v(out_buf, out_off, nb * rows));
    }
    return 0;
}

static int dispatch_layer_batch(InkSession *s, PmBatch *bp, int il, int ip, int *n_blocks, int has_messages, int64_t nb) {
    PmBatch batch = *bp;
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
    int64_t bd = nb * d;

    /* Wave 1: save inp_sa = cur concurrently with depth-attention mix. */
    CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->cur, 0, bd), a_f32v(s->inp_sa, 0, bd));

    PmBuffer q_src = s->cur, k_src = s->cur, v_src = s->cur;
    int64_t q_off = 0, k_off = 0, v_off = 0;
    if (L->attn_res_query.present) {
        int64_t nq = is_swa ? 1 : 3;
        CALL(s, batch, K_MIX_BATCH, nq * nb, a_int(d), a_int(*n_blocks), a_int(nb),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd),
             a_f32v(s->cur, 0, bd),
             a_tensor_f32(s, &L->attn_res_query),
             a_f32v(s->mix_out, 0, nq * bd));
        q_src = s->mix_out; q_off = 0;
        k_src = s->mix_out; k_off = is_swa ? 0 : bd;
        v_src = s->mix_out; v_off = is_swa ? 0 : 2 * bd;
    }
    pm_barrier(batch);

    /* Optional loop control gates (Q, K, V concurrently; F32->BF16 dequant overlaps with K_CTRL_PREP_BATCH!). */
    if (has_messages) {
        int64_t rank = hp->gate_rank;
        int ctrl_bf16 = (nb >= 64 && L->loop_ctrl_down[0].type == INK_GGML_F32 &&
                         L->loop_ctrl_down[1].type == INK_GGML_F32 && L->loop_ctrl_down[2].type == INK_GGML_F32);
        int64_t csz = rank * d;
        CALL(s, batch, K_CTRL_PREP_BATCH, nb, a_int(d),
             a_f32v(q_src, q_off, bd), a_f32v(s->messages, 0 * bd, bd), a_f32v(s->ctrl_cat, 0 * 2 * bd, 2 * bd));
        CALL(s, batch, K_CTRL_PREP_BATCH, nb, a_int(d),
             a_f32v(k_src, k_off, bd), a_f32v(s->messages, 1 * bd, bd), a_f32v(s->ctrl_cat, 1 * 2 * bd, 2 * bd));
        CALL(s, batch, K_CTRL_PREP_BATCH, nb, a_int(d),
             a_f32v(v_src, v_off, bd), a_f32v(s->messages, 2 * bd, bd), a_f32v(s->ctrl_cat, 2 * 2 * bd, 2 * bd));
        if (ctrl_bf16) {
            if (dispatch_dequant_f32_bf16(s, batch, &L->loop_ctrl_down[0], rank, 2 * d, 0 * csz)) return 1;
            if (dispatch_dequant_f32_bf16(s, batch, &L->loop_ctrl_down[1], rank, 2 * d, 1 * csz)) return 1;
            if (dispatch_dequant_f32_bf16(s, batch, &L->loop_ctrl_down[2], rank, 2 * d, 2 * csz)) return 1;
        }
        pm_barrier(batch);
        if (ctrl_bf16) {
            if (dispatch_f32_to_bf16(s, batch, 6 * bd, s->ctrl_cat, 0, 0)) return 1;
            pm_barrier(batch);
            if (dispatch_mm_bf16(s, batch, rank, 2 * d, nb, 0 * csz, s->moe_slot_out, 0 * 2 * bd, s->ctrl_gate, 0 * nb * rank)) return 1;
            if (dispatch_mm_bf16(s, batch, rank, 2 * d, nb, 1 * csz, s->moe_slot_out, 1 * 2 * bd, s->ctrl_gate, 1 * nb * rank)) return 1;
            if (dispatch_mm_bf16(s, batch, rank, 2 * d, nb, 2 * csz, s->moe_slot_out, 2 * 2 * bd, s->ctrl_gate, 2 * nb * rank)) return 1;
        } else {
            if (dispatch_mm(s, batch, &L->loop_ctrl_down[0], rank, 2 * d, nb, s->ctrl_cat, 0 * 2 * bd, s->ctrl_gate, 0 * nb * rank)) return 1;
            if (dispatch_mm(s, batch, &L->loop_ctrl_down[1], rank, 2 * d, nb, s->ctrl_cat, 1 * 2 * bd, s->ctrl_gate, 1 * nb * rank)) return 1;
            if (dispatch_mm(s, batch, &L->loop_ctrl_down[2], rank, 2 * d, nb, s->ctrl_cat, 2 * 2 * bd, s->ctrl_gate, 2 * nb * rank)) return 1;
        }
        pm_barrier(batch);
        int64_t citems = ((nb + 7) / 8) * ((d + INK_T - 1) / INK_T);
        CALL(s, batch, K_CTRL_UP_APPLY_BATCH, citems, a_int(d), a_int(rank), a_int(nb),
             a_tensor_f32(s, &L->loop_ctrl_gain[0]), a_tensor_f32(s, &L->loop_ctrl_forget[0]),
             a_f32v(s->ctrl_gate, 0 * nb * rank, nb * rank), a_f32v(q_src, q_off, bd), a_f32v(s->messages, 0 * bd, bd), a_f32v(s->inp_q, 0, bd));
        CALL(s, batch, K_CTRL_UP_APPLY_BATCH, citems, a_int(d), a_int(rank), a_int(nb),
             a_tensor_f32(s, &L->loop_ctrl_gain[1]), a_tensor_f32(s, &L->loop_ctrl_forget[1]),
             a_f32v(s->ctrl_gate, 1 * nb * rank, nb * rank), a_f32v(k_src, k_off, bd), a_f32v(s->messages, 1 * bd, bd), a_f32v(s->inp_k, 0, bd));
        CALL(s, batch, K_CTRL_UP_APPLY_BATCH, citems, a_int(d), a_int(rank), a_int(nb),
             a_tensor_f32(s, &L->loop_ctrl_gain[2]), a_tensor_f32(s, &L->loop_ctrl_forget[2]),
             a_f32v(s->ctrl_gate, 2 * nb * rank, nb * rank), a_f32v(v_src, v_off, bd), a_f32v(s->messages, 2 * bd, bd), a_f32v(s->inp_v, 0, bd));
        pm_barrier(batch);
        q_src = s->inp_q; q_off = 0;
        k_src = s->inp_k; k_off = 0;
        v_src = s->inp_v; v_off = 0;
    }

    /* Wave 2: Input RMSNorms (ink_rmsnorm already supports nb rows!). */
    /* Wave 2: Input RMSNorms + overlapped Q8/F32 -> BF16 weight dequantization (-48 barriers/prefill!). */
    int has_k_dyn = (!is_swa && L->attn_k_conv_proj.present);
    int has_v_dyn = (!is_swa && L->attn_v_conv_proj.present);
    int qkv_bf16 = (nb >= 8 && L->attn_q.type == INK_GGML_Q8_0 && L->attn_k.type == INK_GGML_Q8_0 &&
                    L->attn_v.type == INK_GGML_Q8_0 && L->attn_rel.type == INK_GGML_Q8_0 && L->attn_gate.type == INK_GGML_Q8_0);
    int64_t off_q = 0;
    int64_t off_k = off_q + n_q * (d / 2);
    int64_t off_v = off_k + n_kv * (d / 2);
    int64_t off_r = off_v + n_kv * (d / 2);
    int64_t off_g = off_r + (n_head * n_rel) * (d / 2);
    int64_t off_s = off_g + n_q * (d / 2);
    int64_t off_kd = off_s + n_head * (d / 2);
    int sink_bf16 = (qkv_bf16 && nb >= 64 && L->attn_sink_proj.type == INK_GGML_F32);
    int kdyn_bf16 = (qkv_bf16 && has_k_dyn && nb >= 64 && L->attn_k_conv_proj.type == INK_GGML_F32);

    PmBuffer q_in = s->inp_q, k_in = s->inp_k, v_in = s->inp_v;
    if (is_swa && has_messages) {
        CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(q_src, q_off, bd), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_q, 0, bd));
        CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(k_src, k_off, bd), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_k, 0, bd));
        CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(v_src, v_off, bd), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_v, 0, bd));
    } else if (is_swa) {
        CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(q_src, q_off, bd), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_q, 0, bd));
        k_in = s->inp_q;
        v_in = s->inp_q;
    } else {
        CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(q_src, q_off, bd), a_tensor_f32(s, &L->attn_q_inp_norm), a_f32v(s->inp_q, 0, bd));
        CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(k_src, k_off, bd), a_tensor_f32(s, &L->attn_k_inp_norm), a_f32v(s->inp_k, 0, bd));
        CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(v_src, v_off, bd), a_tensor_f32(s, &L->attn_v_inp_norm), a_f32v(s->inp_v, 0, bd));
    }
    const FastLayer *FL = &s->fast_layers[ip];
    if (qkv_bf16) {
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_q, &FL->attn_q, n_q, d, off_q)) return 1;
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_k, &FL->attn_k, n_kv, d, off_k)) return 1;
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_v, &FL->attn_v, n_kv, d, off_v)) return 1;
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_rel, &FL->attn_rel, n_head * n_rel, d, off_r)) return 1;
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_gate, &FL->attn_gate, n_q, d, off_g)) return 1;
        if (sink_bf16) {
            if (dispatch_dequant_f32_bf16(s, batch, &L->attn_sink_proj, n_head, d, off_s)) return 1;
        }
        if (kdyn_bf16) {
            if (dispatch_dequant_f32_bf16(s, batch, &L->attn_k_conv_proj, n_head_kv * hp->conv_len, d, off_kd)) return 1;
        }
    }
    pm_barrier(batch);
    prof_mark(bp, 0); batch = *bp; /* 0: mix+ctrl+norm+qkv_deq */

    /* Wave 3: ALL independent attention projections concurrently — Metal 4 Neural Accelerators (native bfloat x bfloat)! */
    if (qkv_bf16) {
        int64_t qx_off = 0, kx_off = 0, vx_off = 0;
        if (dispatch_f32_to_bf16(s, batch, bd, q_in, 0, 0)) return 1;
        if (k_in != q_in) {
            kx_off = bd;
            if (dispatch_f32_to_bf16(s, batch, bd, k_in, 0, kx_off)) return 1;
        }
        if (v_in != q_in) {
            vx_off = (v_in == k_in) ? kx_off : 2 * bd;
            if (v_in != k_in && dispatch_f32_to_bf16(s, batch, bd, v_in, 0, vx_off)) return 1;
        }
        pm_barrier(batch);
        if (dispatch_mm_bf16(s, batch, n_q, d, nb, off_q, s->moe_slot_out, qx_off, s->q_buf, 0)) return 1;
        if (dispatch_mm_bf16(s, batch, n_kv, d, nb, off_k, s->moe_slot_out, kx_off, s->k_raw, 0)) return 1;
        if (dispatch_mm_bf16(s, batch, n_kv, d, nb, off_v, s->moe_slot_out, vx_off, s->v_raw, 0)) return 1;
        if (dispatch_mm_bf16(s, batch, n_head * n_rel, d, nb, off_r, s->moe_slot_out, qx_off, s->r_buf, 0)) return 1;
        if (dispatch_mm_bf16(s, batch, n_q, d, nb, off_g, s->moe_slot_out, qx_off, s->gate_buf, 0)) return 1;
        if (sink_bf16) {
            if (dispatch_mm_bf16(s, batch, n_head, d, nb, off_s, s->moe_slot_out, qx_off, s->sinks_buf, 0)) return 1;
        } else {
            if (dispatch_mm(s, batch, &L->attn_sink_proj, n_head, d, nb, q_in, 0, s->sinks_buf, 0)) return 1;
        }
        if (has_k_dyn) {
            if (kdyn_bf16) {
                if (dispatch_mm_bf16(s, batch, n_head_kv * hp->conv_len, d, nb, off_kd, s->moe_slot_out, kx_off, s->dyn_buf, 0)) return 1;
            } else {
                if (dispatch_mm(s, batch, &L->attn_k_conv_proj, n_head_kv * hp->conv_len, d, nb,
                                k_in, 0, s->dyn_buf, 0)) return 1;
            }
        }
    } else {
        if (dispatch_mm(s, batch, &L->attn_q, n_q, d, nb, q_in, 0, s->q_buf, 0)) return 1;
        if (dispatch_mm(s, batch, &L->attn_k, n_kv, d, nb, k_in, 0, s->k_raw, 0)) return 1;
        if (dispatch_mm(s, batch, &L->attn_v, n_kv, d, nb, v_in, 0, s->v_raw, 0)) return 1;
        if (dispatch_mm(s, batch, &L->attn_rel, n_head * n_rel, d, nb, q_in, 0, s->r_buf, 0)) return 1;
        if (dispatch_mm(s, batch, &L->attn_sink_proj, n_head, d, nb, q_in, 0, s->sinks_buf, 0)) return 1;
        if (dispatch_mm(s, batch, &L->attn_gate, n_q, d, nb, q_in, 0, s->gate_buf, 0)) return 1;
        if (has_k_dyn) {
            if (dispatch_mm(s, batch, &L->attn_k_conv_proj, n_head_kv * hp->conv_len, d, nb,
                            k_in, 0, s->dyn_buf, 0)) return 1;
        }
    }
    pm_barrier(batch);
    prof_mark(bp, 1); batch = *bp; /* 1: QKV+R+G+Sink+Dyn GEMMs */

    /* Wave 4: K shortconv, Q clip, R clip (+ V shortconv if !has_v_dyn).
     * Non-SWA layers apply per-token log-scale tau from s->tau_buf so batched MMA prefill
     * stays exact across the entire 32K context window (including past log_scale_floor=8192). */
    int use_tau_vec = is_swa ? 0 : 1;
    PmArg tau_arg = use_tau_vec ? a_f32v(s->tau_buf, 0, nb) : a_f32v(s->dummy_f32, 0, 64);
    int64_t conv_kv_items = ((nb + 31) / 32) * ((n_kv + 63) / 64);
    CALL(s, batch, K_CONVOLVE_BATCH, conv_kv_items, a_int(n_kv), a_int(n_head_kv), a_int(has_k_dyn ? 1 : 0), a_int(nb),
         a_f32v(s->k_raw, 0, nb * n_kv), a_tensor_f32(s, &L->attn_k_conv),
         has_k_dyn ? a_f32v(s->dyn_buf, 0, nb * n_head_kv * hp->conv_len) : a_f32v(s->dummy_f32, 0, 64),
         a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->k_buf, 0, nb * n_kv));
    CALL(s, batch, K_CLIP_BATCH, nb * n_head, a_int(hd), a_int(n_head), a_int(L->attn_q_clip.present ? 1 : 0),
         a_f32(hp->qk_clip), a_int(use_tau_vec), a_f32(1.0f),
         a_f32v(s->q_buf, 0, nb * n_q),
         L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64),
         tau_arg);
    CALL(s, batch, K_CLIP_BATCH, nb * n_head, a_int(n_rel), a_int(n_head), a_int(L->attn_rel_clip.present ? 1 : 0),
         a_f32(hp->rel_clip), a_int(use_tau_vec), a_f32(1.0f),
         a_f32v(s->r_buf, 0, nb * n_head * n_rel),
         L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64),
         tau_arg);
    if (!has_v_dyn) {
        CALL(s, batch, K_CONVOLVE_BATCH, conv_kv_items, a_int(n_kv), a_int(n_head_kv), a_int(0), a_int(nb),
             a_f32v(s->v_raw, 0, nb * n_kv), a_tensor_f32(s, &L->attn_v_conv),
             a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_v[il], 0, n_kv * 3), a_f32v(s->v_buf, 0, nb * n_kv));
        pm_barrier(batch);
    } else {
        pm_barrier(batch);
        if (dispatch_mm(s, batch, &L->attn_v_conv_proj, n_head_kv * hp->conv_len, d, nb,
                        v_in, 0, s->dyn_buf, 0)) return 1;
        pm_barrier(batch);
        CALL(s, batch, K_CONVOLVE_BATCH, conv_kv_items, a_int(n_kv), a_int(n_head_kv), a_int(1), a_int(nb),
             a_f32v(s->v_raw, 0, nb * n_kv), a_tensor_f32(s, &L->attn_v_conv),
             a_f32v(s->dyn_buf, 0, nb * n_head_kv * hp->conv_len),
             a_f32v(s->conv_v[il], 0, n_kv * 3), a_f32v(s->v_buf, 0, nb * n_kv));
        pm_barrier(batch);
    }

    /* Wave 5: K clip + V head RMSNorm concurrently. */
    CALL(s, batch, K_CLIP_BATCH, nb * n_head_kv, a_int(hd), a_int(n_head_kv), a_int(0),
         a_f32(hp->qk_clip), a_int(0), a_f32(1.0f),
         a_f32v(s->k_buf, 0, nb * n_kv), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->dummy_f32, 0, 64));
    CALL(s, batch, K_HEAD_RMSNORM, nb * n_head_kv, a_int(hd), a_f32(hp->rms_eps),
         a_f32v(s->v_buf, 0, nb * n_kv));
    pm_barrier(batch);

    /* Store K, V for all nb tokens into KV cache */
    int64_t kv_cap = s->kv_cap[il];
    CALL(s, batch, K_KV_STORE_BATCH, nb, a_int(n_kv), a_int(s->pos), a_int(kv_cap), a_int(is_swa),
         a_f32v(s->k_buf, 0, nb * n_kv), a_f32v(s->v_buf, 0, nb * n_kv),
         a_u16v(s->kv_k[il], 0, kv_cap * n_kv), a_u16v(s->kv_v[il], 0, kv_cap * n_kv));
    pm_barrier(batch);

    /* 3-stage MMA Flash-Prefill Attention (packed score_stride = span for 9x smaller SLC-resident scores!) */
    prof_mark(bp, 2); batch = *bp; /* 2: conv+clip+hnorm+kv */
    uint32_t dims0 = (uint32_t)n_head | ((uint32_t)n_head_kv << 8) | ((uint32_t)hd << 16) | ((uint32_t)n_rel << 24);
    uint32_t dims1 = (uint32_t)hp->rel_extent | ((uint32_t)(is_swa ? 1 : 0) << 16) | ((uint32_t)hp->swa << 17);
    int64_t k_start = (is_swa && s->pos >= hp->swa) ? (s->pos - hp->swa + 1) : 0;
    int64_t span = (s->pos + nb) - k_start;
    int64_t sc_stride = span;
    uint32_t kv_info = (uint32_t)kv_cap | ((uint32_t)sc_stride << 16);
    int64_t q_tiles = (nb + 63) / 64;
    CALL(s, batch, K_ATTN_SCORES_MMA, n_head * q_tiles * 4,
         a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos), a_int(nb), a_int(k_start),
         a_f32v(s->q_buf, 0, nb * n_q),
         a_f32v(s->r_buf, 0, nb * n_head * n_rel),
         a_tensor_f32(s, &L->attn_rel_bias),
         a_f32v(s->sinks_buf, 0, nb * n_head),
         a_u16v(s->kv_k[il], 0, kv_cap * n_kv),
         a_u16v(s->attn_scores, 0, nb * n_head * sc_stride));
    pm_barrier(batch);
    prof_mark(bp, 3); batch = *bp; /* 3: attn_scores_mma */
    CALL(s, batch, K_ATTN_SOFTMAX_MMA, nb * n_head,
         a_u32(dims0), a_u32(dims1), a_int(sc_stride), a_int(s->pos), a_int(nb), a_int(k_start),
         a_u16v(s->attn_scores, 0, nb * n_head * sc_stride),
         a_f32v(s->sinks_buf, 0, nb * n_head));
    pm_barrier(batch);
    prof_mark(bp, 4); batch = *bp; /* 4: attn_softmax_mma */
    int o_bf16 = (nb >= 8 && L->attn_output.type == INK_GGML_Q8_0);
    CALL(s, batch, K_ATTN_VALUES_MMA, n_head * q_tiles * 4,
         a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos), a_int(nb), a_int(k_start),
         a_u16v(s->attn_scores, 0, nb * n_head * sc_stride),
         a_f32v(s->sinks_buf, 0, nb * n_head),
         a_u16v(s->kv_v[il], 0, kv_cap * n_kv),
         a_f32v(s->gate_buf, 0, nb * n_q),
         a_f32v(s->attn_out, 0, nb * n_q));
    if (o_bf16) {
        /* Overlapped with K_ATTN_VALUES_MMA (-48 barriers/prefill!). */
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_output, &FL->attn_output, d, n_q, 0)) return 1;
    }
    pm_barrier(batch);
    prof_mark(bp, 5); batch = *bp; /* 5: attn_values_mma+o_deq */

    /* Output projection + post shortconv + residual (fused when non-block-boundary!) */
    if (o_bf16) {
        if (dispatch_f32_to_bf16(s, batch, nb * n_q, s->attn_out, 0, 0)) return 1;
        pm_barrier(batch);
        if (dispatch_mm_bf16(s, batch, d, n_q, nb, 0, s->moe_slot_out, 0, s->o_raw, 0)) return 1;
    } else {
        if (dispatch_mm(s, batch, &L->attn_output, d, n_q, nb, s->attn_out, 0, s->o_raw, 0)) return 1;
    }
    pm_barrier(batch);
    int64_t conv_d_items = ((nb + 31) / 32) * ((d + 63) / 64);
    if (ip % hp->block_size == 0) {
        if (*n_blocks >= INK_MAX_BLOCKS) {
            snprintf(s->error, sizeof s->error, "block count overflow");
            return 1;
        }
        CALL(s, batch, K_CONVOLVE_BATCH, conv_d_items, a_int(d), a_int(1), a_int(0), a_int(nb),
             a_f32v(s->o_raw, 0, bd), a_tensor_f32(s, &L->attn_post_conv),
             a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_ao[il], 0, d * 3), a_f32v(s->cur, 0, bd));
        CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->inp_sa, 0, bd),
             a_f32v(s->blocks, (int64_t)(*n_blocks) * bd, bd));
        (*n_blocks)++;
    } else {
        CALL(s, batch, K_CONVOLVE_ADD_BATCH, conv_d_items, a_int(d), a_int(nb),
             a_f32v(s->o_raw, 0, bd), a_tensor_f32(s, &L->attn_post_conv),
             a_f32v(s->conv_ao[il], 0, d * 3), a_f32v(s->inp_sa, 0, bd), a_f32v(s->cur, 0, bd));
    }
    pm_barrier(batch);

    /* ffn_inp = cur concurrently with MoE input mix */
    CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->cur, 0, bd), a_f32v(s->ffn_inp, 0, bd));
    CALL(s, batch, K_MIX_BATCH, nb, a_int(d), a_int(*n_blocks), a_int(nb),
         a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd),
         a_f32v(s->cur, 0, bd),
         a_tensor_f32(s, &L->ffn_res_query),
         a_f32v(s->norm_buf, 0, bd));
    pm_barrier(batch);
    if (has_messages) {
        int64_t rank = hp->gate_rank;
        int c3_bf16 = (nb >= 64 && L->loop_ctrl_down[3].type == INK_GGML_F32);
        CALL(s, batch, K_CTRL_PREP_BATCH, nb, a_int(d),
             a_f32v(s->norm_buf, 0, bd), a_f32v(s->messages, 3 * bd, bd), a_f32v(s->ctrl_cat, 0, 2 * bd));
        if (c3_bf16) {
            if (dispatch_dequant_f32_bf16(s, batch, &L->loop_ctrl_down[3], rank, 2 * d, 0)) return 1;
        }
        pm_barrier(batch);
        if (c3_bf16) {
            if (dispatch_f32_to_bf16(s, batch, 2 * bd, s->ctrl_cat, 0, 0)) return 1;
            pm_barrier(batch);
            if (dispatch_mm_bf16(s, batch, rank, 2 * d, nb, 0, s->moe_slot_out, 0, s->ctrl_gate, 0)) return 1;
        } else {
            if (dispatch_mm(s, batch, &L->loop_ctrl_down[3], rank, 2 * d, nb, s->ctrl_cat, 0, s->ctrl_gate, 0)) return 1;
        }
        pm_barrier(batch);
        int64_t citems = ((nb + 7) / 8) * ((d + INK_T - 1) / INK_T);
        CALL(s, batch, K_CTRL_UP_APPLY_BATCH, citems, a_int(d), a_int(rank), a_int(nb),
             a_tensor_f32(s, &L->loop_ctrl_gain[3]), a_tensor_f32(s, &L->loop_ctrl_forget[3]),
             a_f32v(s->ctrl_gate, 0, nb * rank), a_f32v(s->norm_buf, 0, bd), a_f32v(s->messages, 3 * bd, bd), a_f32v(s->norm_buf, 0, bd));
        pm_barrier(batch);
    }
    int rtr_bf16 = (nb >= 64 && L->ffn_gate_inp.type == INK_GGML_F32);
    CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
         a_f32v(s->norm_buf, 0, bd), a_tensor_f32(s, &L->ffn_norm), a_f32v(s->norm_buf, 0, bd));
    if (rtr_bf16) {
        if (dispatch_dequant_f32_bf16(s, batch, &L->ffn_gate_inp, hp->n_expert, d, 0)) return 1;
    }
    pm_barrier(batch);

    /* MoE router + expert grouping */
    if (rtr_bf16) {
        if (dispatch_f32_to_bf16(s, batch, bd, s->norm_buf, 0, 0)) return 1;
        pm_barrier(batch);
        if (dispatch_mm_bf16(s, batch, hp->n_expert, d, nb, 0, s->moe_slot_out, 0, s->router_logits, 0)) return 1;
    } else {
        if (dispatch_mm(s, batch, &L->ffn_gate_inp, hp->n_expert, d, nb, s->norm_buf, 0, s->router_logits, 0)) return 1;
    }
    pm_barrier(batch);
    CALL(s, batch, K_ROUTER_BATCH, nb, a_int(hp->n_expert), a_int(hp->n_expert_used),
         a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
         a_f32v(s->router_logits, 0, nb * hp->n_expert),
         L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
         a_u32v(s->moe_ids, 0, nb * hp->n_expert_used),
         a_f32v(s->moe_wts, 0, nb * hp->n_expert_used));
    pm_barrier(batch);
    CALL(s, batch, K_MOE_GROUP_BATCH, hp->n_expert, a_int(nb), a_int(hp->n_expert_used),
         a_u32v(s->moe_ids, 0, nb * hp->n_expert_used),
         a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
         a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
         a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
         a_f32v(s->exp_wts, 0, hp->n_expert * INK_PREFILL_B),
         a_u32v(s->exp_cnt, 0, hp->n_expert));
    pm_barrier(batch);
    prof_mark(bp, 6); batch = *bp; /* 6: O_proj+pconv+ffn_prep+rtr+group */

    int64_t n_ff = hp->n_ff_exp;
    int64_t tile_cap = hp->n_expert + INK_PREFILL_B / 2 + 16;
    int64_t direct_thresh = 16;
    {
        const char *env_t = getenv("INK_DIRECT_THRESH");
        if (env_t && env_t[0]) direct_thresh = strtoll(env_t, NULL, 10);
    }
    if (nb <= direct_thresh &&
        L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL &&
        L->ffn_down_exps.type == INK_GGML_IQ4_NL) {
        /* Direct IQ4_NL tiny-batch MoE (tile_m=4, zero BF16 staging!) */
        int64_t tot_rows = nb * hp->n_expert_used;
        int64_t tm = 4;
        int64_t gu_rg = (n_ff + 3) / 4;
        int64_t dn_rg = (d + 7) / 8;
        CALL(s, batch, K_MOE_EXP_OFF, 1, a_int(hp->n_expert),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert));
        CALL(s, batch, K_MOE_BUILD_TILES, 1, a_int(hp->n_expert), a_int(tm), a_int(gu_rg), a_int(dn_rg),
             a_u32v(s->exp_cnt, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap),
             a_u32v(s->tile_m0, 0, tile_cap),
             a_u32v(s->tile_cnt, 0, 16));
        pm_barrier(batch);
        CALL(s, batch, K_MOE_GATHER_X, hp->n_expert * 8, a_int(d), a_int(hp->n_expert_used),
             a_f32v(s->norm_buf, 0, bd),
             a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
             a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
             a_f32v(s->exp_wts, 0, hp->n_expert * INK_PREFILL_B),
             a_u32v(s->exp_cnt, 0, hp->n_expert),
             a_u32v(s->exp_off, 0, hp->n_expert),
             a_f32v(s->moe_slot_out, 0, tot_rows * d),
             a_u32v(s->row_dst, 0, tot_rows),
             a_f32v(s->row_wt, 0, tot_rows));
        pm_barrier(batch);
        int64_t max_tiles = (tot_rows + (tm - 1) * hp->n_expert) / tm;
        if (max_tiles > tot_rows) max_tiles = tot_rows;
        if (max_tiles > tile_cap) max_tiles = tile_cap;
        CALL(s, batch, K_MOE_GU_IQ4_M4, max_tiles * gu_rg,
             a_int(n_ff), a_int(d), a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_tensor_u16(s, &L->ffn_gate_up_exps),
             a_f32v(s->moe_slot_out, 0, tot_rows * d),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16),
             a_f32v(s->moe_hid, 0, tot_rows * n_ff));
        pm_barrier(batch);
        CALL(s, batch, K_MOE_DOWN_IQ4_M4, max_tiles * dn_rg,
             a_int(d), a_int(n_ff),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_f32v(s->moe_hid, 0, tot_rows * n_ff),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16),
             a_u32v(s->row_dst, 0, tot_rows), a_f32v(s->row_wt, 0, tot_rows),
             a_f32v(s->moe_slot_out, 0, tot_rows * d));
        pm_barrier(batch);
        prof_mark(bp, 3); batch = *bp; /* 3: moe_mm_iq4 (GU+SITU+DN+scatter) */
    } else if (nb >= 8 &&
        L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL &&
        L->ffn_down_exps.type == INK_GGML_IQ4_NL) {
        /* Sprint 42: Metal 4 Neural Accelerator Expert-Grouped MoE with IN-TILE IQ4_NL dequant (TM=128 for nb>=1024 halves dequant!) */
        int64_t tot_rows = nb * hp->n_expert_used;
        int use_256 = (nb >= 2560);
        int use_128 = (nb >= 768 && !use_256);
        int use_64 = (nb >= 384 && !use_128 && !use_256);
        int64_t tm = use_256 ? 256 : (use_128 ? 128 : (use_64 ? 64 : 16));
        int64_t tn = 64;
        int moe_mm_k = use_256 ? K_MOE_MM_IQ4_256 : (use_128 ? K_MOE_MM_IQ4_128 : (use_64 ? K_MOE_MM_IQ4 : K_MOE_MM_IQ4_16));
        int64_t gu_rows = 2 * n_ff;
        int64_t gu_r_tiles = (gu_rows + tn - 1) / tn;
        int64_t dn_r_tiles = (d + tn - 1) / tn;
        int64_t max_tiles = (tot_rows + (tm - 1) * hp->n_expert) / tm;
        if (max_tiles > tot_rows) max_tiles = tot_rows;
        if (max_tiles > tile_cap) max_tiles = tile_cap;
        CALL(s, batch, K_MOE_EXP_OFF, 1, a_int(hp->n_expert),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert));
        CALL(s, batch, K_MOE_BUILD_TILES, 1, a_int(hp->n_expert), a_int(tm), a_int(gu_r_tiles), a_int(dn_r_tiles),
             a_u32v(s->exp_cnt, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16));
        pm_barrier(batch);
        CALL(s, batch, K_MOE_GATHER_BF16, hp->n_expert * 8, a_int(d), a_int(hp->n_expert_used),
             a_f32v(s->norm_buf, 0, bd),
             a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
             a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
             a_f32v(s->exp_wts, 0, hp->n_expert * INK_PREFILL_B),
             a_u32v(s->exp_cnt, 0, hp->n_expert),
             a_u32v(s->exp_off, 0, hp->n_expert),
             a_u16v(s->moe_slot_out, 0, tot_rows * d),
             a_u32v(s->row_dst, 0, tot_rows),
             a_f32v(s->row_wt, 0, tot_rows));
        pm_barrier(batch);
        prof_mark(bp, 7); batch = *bp; /* 7: moe_gather_bf16+tiles */
        /* Gate+Up in ONE single dispatch across all 128 experts (native BF16xBF16 Neural Accelerator)! */
        CALL(s, batch, moe_mm_k, max_tiles * gu_r_tiles, a_int(gu_rows), a_int(d),
             a_tensor_u16(s, &L->ffn_gate_up_exps),
             a_u16v(s->moe_slot_out, 0, tot_rows * d),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16),
             a_f32v(s->moe_gu, 0, tot_rows * gu_rows));
        pm_barrier(batch);
        prof_mark(bp, 8); batch = *bp; /* 8: moe_gu_iq4 */
        CALL(s, batch, K_SITU_BATCH_BF16, tot_rows,
             a_int(n_ff), a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_f32v(s->moe_gu, 0, tot_rows * gu_rows),
             a_u16v(s->moe_hid, 0, tot_rows * n_ff));
        pm_barrier(batch);
        prof_mark(bp, 9); batch = *bp; /* 9: situ_batch_bf16 */
        /* Down in ONE single dispatch across all 128 experts (native BF16xBF16 Neural Accelerator)! */
        CALL(s, batch, moe_mm_k, max_tiles * dn_r_tiles, a_int(d), a_int(n_ff),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_u16v(s->moe_hid, 0, tot_rows * n_ff),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16),
             a_f32v(s->moe_gu, 0, tot_rows * d));
        pm_barrier(batch);
        CALL(s, batch, K_MOE_SCATTER_WT, tot_rows, a_int(d),
             a_f32v(s->moe_gu, 0, tot_rows * d),
             a_u32v(s->row_dst, 0, tot_rows), a_f32v(s->row_wt, 0, tot_rows),
             a_f32v(s->moe_slot_out, 0, tot_rows * d));
        pm_barrier(batch);
        prof_mark(bp, 10); batch = *bp; /* 10: moe_dn_iq4+scatter */
    } else if (nb >= 8 &&
        (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL || L->ffn_gate_up_exps.type == INK_GGML_Q8_0) &&
        (L->ffn_down_exps.type == INK_GGML_IQ4_NL || L->ffn_down_exps.type == INK_GGML_Q8_0)) {
        /* Sprint 9B: Metal 4 Neural Accelerator (am_matmul_bf16) Expert-Grouped MoE (64-exp GU = 2 chunks, 128-exp Down = 1 chunk!) */
        int64_t tot_rows = nb * hp->n_expert_used;
        int use_64 = (nb >= 384);
        int64_t tm = use_64 ? 64 : 16;
        int64_t tn = 64;
        int moe_mm_k = use_64 ? K_MOE_MM_BF16 : K_MOE_MM_BF16_16;
        CALL(s, batch, K_MOE_EXP_OFF, 1, a_int(hp->n_expert),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert));
        pm_barrier(batch);
        /* Gather norm_buf -> contiguous x_exp (reusing moe_slot_out) + record row_dst/row_wt */
        CALL(s, batch, K_MOE_GATHER_X, hp->n_expert * 8, a_int(d), a_int(hp->n_expert_used),
             a_f32v(s->norm_buf, 0, bd),
             a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
             a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
             a_f32v(s->exp_wts, 0, hp->n_expert * INK_PREFILL_B),
             a_u32v(s->exp_cnt, 0, hp->n_expert),
             a_u32v(s->exp_off, 0, hp->n_expert),
             a_f32v(s->moe_slot_out, 0, tot_rows * d),
             a_u32v(s->row_dst, 0, tot_rows),
             a_f32v(s->row_wt, 0, tot_rows));
        pm_barrier(batch);

        int64_t e_chunk_gu = 16;
        int64_t gu_rows = 2 * n_ff;
        int64_t gu_w_u32 = gu_rows * (d / 2);
        int64_t gu_r_tiles = (gu_rows + tn - 1) / tn;
        int64_t gu_deq_rgrps = (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) ? ((gu_rows + 7) / 8) : ((gu_rows + 3) / 4);
        int gu_deq_k = (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) ? K_DEQUANT_IQ4_EXP_BF16 : K_DEQUANT_Q8_EXP_BF16;
        for (int64_t e_lo = 0; e_lo < hp->n_expert; e_lo += e_chunk_gu) {
            int64_t e_hi = e_lo + e_chunk_gu;
            if (e_hi > hp->n_expert) e_hi = hp->n_expert;
            int64_t ec = e_hi - e_lo;
            int64_t max_ct = ec + tot_rows / tm;
            if (max_ct > tile_cap) max_ct = tile_cap;
            CALL(s, batch, K_MOE_CHUNK_TILES, 1, a_int(e_lo), a_int(e_hi), a_int(tm), a_int(gu_r_tiles),
                 a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16));
            CALL(s, batch, gu_deq_k, ec * gu_deq_rgrps, a_int(e_lo), a_int(gu_rows), a_int(d),
                 a_tensor_u16(s, &L->ffn_gate_up_exps), a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->w_bf16, 0, ec * gu_w_u32));
            pm_barrier(batch);
            CALL(s, batch, moe_mm_k, max_ct * gu_r_tiles, a_int(e_lo), a_int(gu_rows), a_int(d),
                 a_u32v(s->w_bf16, 0, ec * gu_w_u32),
                 a_f32v(s->moe_slot_out, 0, tot_rows * d),
                 a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16),
                 a_f32v(s->moe_gu, 0, tot_rows * gu_rows));
            pm_barrier(batch);
        }
        /* Contiguous SITU: moe_gu -> moe_hid */
        CALL(s, batch, K_SITU_BATCH, tot_rows,
             a_int(n_ff), a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_f32v(s->moe_gu, 0, tot_rows * gu_rows),
             a_f32v(s->moe_hid, 0, tot_rows * n_ff));
        pm_barrier(batch);

        /* Down projection (32 experts/chunk fits in 44M u32 w_bf16!) */
        int64_t e_chunk_dn = 32;
        int64_t dn_w_u32 = d * (n_ff / 2);
        int64_t dn_r_tiles = (d + tn - 1) / tn;
        int64_t dn_deq_rgrps = (L->ffn_down_exps.type == INK_GGML_IQ4_NL) ? ((d + 7) / 8) : ((d + 3) / 4);
        int dn_deq_k = (L->ffn_down_exps.type == INK_GGML_IQ4_NL) ? K_DEQUANT_IQ4_EXP_BF16 : K_DEQUANT_Q8_EXP_BF16;
        for (int64_t e_lo = 0; e_lo < hp->n_expert; e_lo += e_chunk_dn) {
            int64_t e_hi = e_lo + e_chunk_dn;
            if (e_hi > hp->n_expert) e_hi = hp->n_expert;
            int64_t ec = e_hi - e_lo;
            int64_t max_ct = ec + tot_rows / tm;
            if (max_ct > tile_cap) max_ct = tile_cap;
            CALL(s, batch, K_MOE_CHUNK_TILES, 1, a_int(e_lo), a_int(e_hi), a_int(tm), a_int(dn_r_tiles),
                 a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16));
            CALL(s, batch, dn_deq_k, ec * dn_deq_rgrps, a_int(e_lo), a_int(d), a_int(n_ff),
                 a_tensor_u16(s, &L->ffn_down_exps), a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->w_bf16, 0, ec * dn_w_u32));
            pm_barrier(batch);
            CALL(s, batch, moe_mm_k, max_ct * dn_r_tiles, a_int(e_lo), a_int(d), a_int(n_ff),
                 a_u32v(s->w_bf16, 0, ec * dn_w_u32),
                 a_f32v(s->moe_hid, 0, tot_rows * n_ff),
                 a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16),
                 a_f32v(s->moe_gu, 0, tot_rows * d));
            pm_barrier(batch);
        }
        /* Weighted scatter back to moe_slot_out -> K_MOE_REDUCE_BATCH */
        CALL(s, batch, K_MOE_SCATTER_WT, tot_rows, a_int(d),
             a_f32v(s->moe_gu, 0, tot_rows * d),
             a_u32v(s->row_dst, 0, tot_rows), a_f32v(s->row_wt, 0, tot_rows),
             a_f32v(s->moe_slot_out, 0, tot_rows * d));
        pm_barrier(batch);
        prof_mark(bp, 3); batch = *bp;
    } else {
        int64_t tile_m = 64;
        CALL(s, batch, K_MOE_BUILD_TILES, 1, a_int(hp->n_expert), a_int(tile_m), a_int((2 * n_ff + 31) / 32), a_int((d + 31) / 32),
             a_u32v(s->exp_cnt, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap),
             a_u32v(s->tile_m0, 0, tile_cap),
             a_u32v(s->tile_cnt, 0, 16));
        pm_barrier(batch);

        /* Expert-grouped MoE gate_up via simdgroup MMA (compacted active tiles + n_a_iters zero-skip) + SITU */
        int64_t max_tiles = hp->n_expert + (nb * hp->n_expert_used) / tile_m;
        if (max_tiles > nb * hp->n_expert_used) max_tiles = nb * hp->n_expert_used;
        int64_t gu_items = max_tiles * ((2 * n_ff + 31) / 32);
        if (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) {
            CALL(s, batch, K_MOE_GU_BATCH_IQ4,
                 gu_items, a_int(2 * n_ff), a_int(d), a_int(hp->n_expert_used),
                 a_tensor_u16(s, &L->ffn_gate_up_exps),
                 a_f32v(s->norm_buf, 0, bd),
                 a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
                 a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
                 a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap),
                 a_u32v(s->tile_m0, 0, tile_cap),
                 a_u32v(s->tile_cnt, 0, 16),
                 a_f32v(s->moe_gu, 0, nb * hp->n_expert_used * 2 * n_ff));
        } else if (L->ffn_gate_up_exps.type == INK_GGML_Q8_0) {
            CALL(s, batch, K_MOE_GU_BATCH_Q8,
                 gu_items, a_int(2 * n_ff), a_int(d), a_int(hp->n_expert_used),
                 a_tensor_u16(s, &L->ffn_gate_up_exps),
                 a_f32v(s->norm_buf, 0, bd),
                 a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
                 a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
                 a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap),
                 a_u32v(s->tile_m0, 0, tile_cap),
                 a_u32v(s->tile_cnt, 0, 16),
                 a_f32v(s->moe_gu, 0, nb * hp->n_expert_used * 2 * n_ff));
        } else {
            snprintf(s->error, sizeof s->error, "unsupported batch gate_up type %u", L->ffn_gate_up_exps.type);
            return 1;
        }
        pm_barrier(batch);
        CALL(s, batch, K_SITU_BATCH, nb * hp->n_expert_used,
             a_int(n_ff), a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_f32v(s->moe_gu, 0, nb * hp->n_expert_used * 2 * n_ff),
             a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff));
        pm_barrier(batch);

        /* Expert-grouped MoE down + slot reduction */
        int64_t dn_items = max_tiles * ((d + 31) / 32);
        if (L->ffn_down_exps.type == INK_GGML_IQ4_NL) {
            CALL(s, batch, K_MOE_DOWN_BATCH_IQ4,
                 dn_items, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
                 a_tensor_u16(s, &L->ffn_down_exps),
                 a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
                 a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
                 a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
                 a_f32v(s->exp_wts, 0, hp->n_expert * INK_PREFILL_B),
                 a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap),
                 a_u32v(s->tile_m0, 0, tile_cap),
                 a_u32v(s->tile_cnt, 0, 16),
                 a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d));
        } else if (L->ffn_down_exps.type == INK_GGML_Q8_0) {
            CALL(s, batch, K_MOE_DOWN_BATCH_Q8,
                 dn_items, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
                 a_tensor_u16(s, &L->ffn_down_exps),
                 a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
                 a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
                 a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
                 a_f32v(s->exp_wts, 0, hp->n_expert * INK_PREFILL_B),
                 a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap),
                 a_u32v(s->tile_m0, 0, tile_cap),
                 a_u32v(s->tile_cnt, 0, 16),
                 a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d));
        } else {
            snprintf(s->error, sizeof s->error, "unsupported batch down type %u", L->ffn_down_exps.type);
            return 1;
        }
        pm_barrier(batch);
    }
    /* Sprint 30: Parallel coalesced MoE slot sum + coalesced post shortconv & residual add (446ms -> ~30ms on 2.4K prefill!) */
    CALL(s, batch, K_MOE_SUM_BATCH, nb * ((d + 127) / 128), a_int(d), a_int(hp->n_expert_used),
         a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d),
         a_f32v(s->moe_out, 0, bd));
    pm_barrier(batch);
    CALL(s, batch, K_CONVOLVE_ADD_BATCH, conv_d_items, a_int(d), a_int(nb),
         a_f32v(s->moe_out, 0, bd),
         a_tensor_f32(s, &L->ffn_post_conv),
         a_f32v(s->conv_fo[il], 0, d * 3),
         a_f32v(s->ffn_inp, 0, bd),
         a_f32v(s->cur, 0, bd));
    pm_barrier(batch);
    prof_mark(bp, 11); batch = *bp; /* 11: moe_reduce_batch */
    return 0;
}

static int run_chunk(InkSession *s, const int32_t *tokens, int nb, int compute_last_logits) {
    const InkHParams *hp = &s->gguf.hp;
    if (s->pos + nb > s->max_ctx) {
        snprintf(s->error, sizeof s->error, "context full (%d+%d/%d)", s->pos, nb, s->max_ctx);
        return 1;
    }
    uint32_t *tids = (uint32_t *)pm_contents(s->tok_ids_buf);
    float *taus = (float *)pm_contents(s->tau_buf);
    for (int i = 0; i < nb; i++) {
        if (tokens[i] < 0 || tokens[i] >= hp->n_vocab) {
            snprintf(s->error, sizeof s->error, "token id %d out of range", tokens[i]);
            return 1;
        }
        tids[i] = (uint32_t)tokens[i];
        float ratio = (float)(s->pos + i + 1) / (float)hp->log_scale_floor;
        if (ratio < 1.0f) ratio = 1.0f;
        taus[i] = 1.0f + hp->log_scale_alpha * logf(ratio);
    }

    PmBatch batch = NULL;
    if (pm_begin_concurrent(&batch) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_begin_concurrent: %s", pm_log());
        return 1;
    }

    int64_t d = hp->n_embd;
    int64_t bd = (int64_t)nb * d;

    /* 1. Batched embedding lookup + token_embd_norm */
    CALL(s, batch, K_EMBED_Q8_BATCH, nb, a_int(d),
         a_u32v(s->tok_ids_buf, 0, nb),
         a_tensor_u16(s, &s->gguf.tok_embd),
         a_tensor_f32(s, &s->gguf.tok_norm),
         a_f32(hp->rms_eps),
         a_f32v(s->cur, 0, bd));
    pm_barrier(batch);

    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;

    int do_pprof = (getenv("INK_PREFILL_PROF") != NULL);
    if (do_pprof) { prof_active = 1; memset(prof_ms, 0, sizeof prof_ms); }

    /* 2. Prelude layers [0 .. start) */
    for (int il = 0; il < start; il++) {
        if (dispatch_layer_batch(s, &batch, il, il, &n_blocks, 0, nb) != 0) { pm_wait(batch, NULL); return 1; }
    }

    /* Save prelude output and prelude blocks */
    CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->cur, 0, bd), a_f32v(s->prelude, 0, bd));
    int prelude_n_blocks = n_blocks;
    if (prelude_n_blocks > 0) {
        int64_t pn = (int64_t)prelude_n_blocks * bd;
        CALL(s, batch, K_COPY, (pn + INK_NT - 1) / INK_NT, a_int(pn),
             a_f32v(s->blocks, 0, pn),
             a_f32v(s->prelude_blk, 0, pn));
    }
    pm_barrier(batch);

    /* 3. Looped core passes */
    for (int pass = 0; pass < hp->loop_count; pass++) {
        int bank_n_blocks = 0;
        if (pass > 0) {
            int src_off = prelude_n_blocks + 1;
            bank_n_blocks = n_blocks - src_off;
            if (bank_n_blocks < 0) bank_n_blocks = 0;
            if (bank_n_blocks > 0) {
                int64_t bn = (int64_t)bank_n_blocks * bd;
                CALL(s, batch, K_COPY, (bn + INK_NT - 1) / INK_NT, a_int(bn),
                     a_f32v(s->blocks, (int64_t)src_off * bd, bn),
                     a_f32v(s->bank_blk, 0, bn));
            }
            CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->cur, 0, bd), a_f32v(s->bank_out, 0, bd));
            pm_barrier(batch);
            CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->prelude, 0, bd), a_f32v(s->cur, 0, bd));
            n_blocks = prelude_n_blocks;
            if (prelude_n_blocks > 0) {
                int64_t pn = (int64_t)prelude_n_blocks * bd;
                CALL(s, batch, K_COPY, (pn + INK_NT - 1) / INK_NT, a_int(pn),
                     a_f32v(s->prelude_blk, 0, pn),
                     a_f32v(s->blocks, 0, pn));
            }
            pm_barrier(batch);
        }
        for (int ip = start; ip < end; ip++) {
            int il = (pass == 0) ? ip : (int)hp->n_phys + (pass - 1) * (end - start) + (ip - start);
            if (pass == 0) {
                if (dispatch_layer_batch(s, &batch, il, ip, &n_blocks, 0, nb) != 0) { pm_wait(batch, NULL); return 1; }
            } else {
                const InkLayerTensors *L = &s->gguf.layers[ip];
                CALL(s, batch, K_MIX_BATCH, 4 * nb, a_int(d), a_int(bank_n_blocks), a_int(nb),
                     a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * bd),
                     a_f32v(s->bank_out, 0, bd),
                     a_tensor_f32(s, &L->loop_msg_query),
                     a_f32v(s->messages, 0, 4 * bd));
                pm_barrier(batch);
                if (dispatch_layer_batch(s, &batch, il, ip, &n_blocks, 1, nb) != 0) { pm_wait(batch, NULL); return 1; }
            }
        }
    }

    /* 4. Coda layers [end .. n_phys) */
    for (int il = end; il < hp->n_phys; il++) {
        if (dispatch_layer_batch(s, &batch, il, il, &n_blocks, 0, nb) != 0) { pm_wait(batch, NULL); return 1; }
    }
    if (do_pprof) {
        prof_active = 0;
        double sum = 0.0;
        for (int i = 0; i < 12; i++) sum += prof_ms[i];
        static const char *const pnames[12] = {
            "0 mix+ctrl+norm", "1 QKV+R+G GEMMs", "2 conv+clip+kv ",
            "3 attn_scores  ", "4 attn_softmax ", "5 attn_values  ",
            "6 O+ffn_prep   ", "7 moe_gather   ", "8 moe_gu_iq4   ",
            "9 situ_bf16    ", "10 moe_dn+scat ", "11 moe_reduce  "
        };
        fprintf(stderr, "\n[INK_PREFILL_PROF nb=%d total_gpu=%.1f ms (%.0f tok/s)]\n",
                nb, sum, nb * 1000.0 / (sum + 1e-9));
        for (int i = 0; i < 12; i++) {
            fprintf(stderr, "  %s: %7.1f ms (%4.1f%%)\n", pnames[i], prof_ms[i], 100.0 * prof_ms[i] / (sum + 1e-9));
        }
    }

    /* 5. Output mix -> output_norm -> lm_head -> logit softcap (ONLY for last token nb-1!) */
    if (compute_last_logits) {
        /* Copy last token's blocks[0..n_blocks-1] and cur[nb-1] or mix all nb and take nb-1: */
        CALL(s, batch, K_MIX_BATCH, nb, a_int(d), a_int(n_blocks), a_int(nb),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd),
             a_f32v(s->cur, 0, bd),
             a_tensor_f32(s, &s->gguf.out_res_query),
             a_f32v(s->norm_buf, 0, bd));
        pm_barrier(batch);
        int64_t last_off = (int64_t)(nb - 1) * d;
        CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->norm_buf, last_off, d), a_tensor_f32(s, &s->gguf.out_norm), a_f32v(s->norm_buf, last_off, d));
        pm_barrier(batch);
        int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
        if (dispatch_mv(s, batch, &s->gguf.output, nv, d, 0, s->norm_buf, last_off, s->logits, 0) != 0) { pm_wait(batch, NULL); return 1; }
        if (hp->logit_softcap > 0.0f) {
            pm_barrier(batch);
            CALL(s, batch, K_SOFTCAP, (nv + INK_NT - 1) / INK_NT, a_int(nv),
                 a_f32(hp->logit_softcap), a_f32v(s->logits, 0, nv));
        }
        s->last_hidden_off = last_off;
        s->argmax_valid = 0;
    }

    double gpu_s = 0.0;
    if (pm_wait(batch, &gpu_s) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_wait: %s", pm_log());
        return 1;
    }
    s->pos += nb;
    if (s->pos > s->max_pos_written) s->max_pos_written = s->pos;
    s->total_steps += nb;
    s->total_gpu_seconds += gpu_s;
    return 0;
}

int ink_prefill(InkSession *s, const int32_t *tokens, int n_tokens, int compute_last_logits) {
    if (n_tokens <= 0) return 0;
    /* Use true batched MMA prefill across the entire 32K context window (per-token tau in tau_buf;
     * kv_info packs kv_cap and sc_stride in 16-bit fields so cap at 65535). */
    if (n_tokens > 1 && s->pos + n_tokens <= 65535 && s->max_ctx <= 65535) {
        int off = 0;
        while (off < n_tokens) {
            int nb = n_tokens - off;
            if (nb > s->prefill_b) nb = s->prefill_b;
            int need_logits = compute_last_logits && (off + nb == n_tokens);
            if (nb == 1) {
                if (ink_step(s, tokens[off], need_logits) != 0) return 1;
            } else {
                if (run_chunk(s, tokens + off, nb, need_logits) != 0) return 1;
            }
            off += nb;
        }
        return 0;
    }
    PmBatch batch = NULL;
    if (pm_begin_concurrent(&batch) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_begin_concurrent: %s", pm_log());
        return 1;
    }
    for (int i = 0; i < n_tokens; i++) {
        int need_logits = compute_last_logits && (i == n_tokens - 1);
        if (enqueue_step(s, &batch, tokens[i], need_logits) != 0) {
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
const float *ink_hidden(const InkSession *s) {
    return s ? ((const float *)pm_contents(s->norm_buf) + s->last_hidden_off) : NULL;
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
        if (s->argmax_valid) {
            return (int32_t)((const uint32_t *)pm_contents(s->argmax_out))[0];
        }
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
const InkMemoryBudget *ink_memory_budget(const InkSession *s) { return s ? &s->budget : NULL; }

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

int ink_test_quant_kernels(const char *metal_path, char *err, size_t err_bytes) {
    PmDeviceInfo info;
    if (pm_open(&info) != PM_OK) { snprintf(err, err_bytes, "pm_open: %s", pm_log()); return 1; }
    size_t metal_len = 0;
    char *src = read_entire_file(metal_path, &metal_len);
    if (!src) { snprintf(err, err_bytes, "cannot read %s", metal_path); return 1; }
    PmModule mod = NULL; double compile_s = 0.0;
    PmStatus st = pm_compile(src, metal_len, "inkling_quant_test", &mod, &compile_s);
    free(src);
    if (st != PM_OK) { snprintf(err, err_bytes, "pm_compile: %s", pm_log()); return 1; }

    PmKernel k_nv = NULL, k_mx = NULL, k_iq = NULL;
    if (pm_kernel(mod, "ink_dv_nvfp4", &k_nv) != PM_OK ||
        pm_kernel(mod, "ink_dv_mxfp8", &k_mx) != PM_OK ||
        pm_kernel(mod, "ink_dv_iq4",   &k_iq) != PM_OK) {
        snprintf(err, err_bytes, "pm_kernel: %s", pm_log());
        if (k_nv) pm_kernel_free(k_nv);
        if (k_mx) pm_kernel_free(k_mx);
        if (k_iq) pm_kernel_free(k_iq);
        pm_unload(mod); return 1;
    }

    int rc = 1;
    const int rows = 8, cols = 64; /* 4 NVFP4 blocks/row, 2 MXFP8 blocks/row, 2 IQ4 blocks/row */
    PmBuffer b_x = NULL, b_out = NULL, b_nv_p = NULL, b_nv_s = NULL, b_mx_w = NULL, b_mx_s = NULL, b_iq_w = NULL;
    if (pm_alloc(cols * 4, &b_x) != PM_OK ||
        pm_alloc(rows * 4, &b_out) != PM_OK ||
        pm_alloc(rows * (cols / 16) * 8, &b_nv_p) != PM_OK ||
        pm_alloc(rows * (cols / 16), &b_nv_s) != PM_OK ||
        pm_alloc(rows * cols, &b_mx_w) != PM_OK ||
        pm_alloc(rows * (cols / 32), &b_mx_s) != PM_OK ||
        pm_alloc(rows * (cols / 32) * 18, &b_iq_w) != PM_OK) {
        snprintf(err, err_bytes, "pm_alloc failed");
        goto cleanup;
    }

    float *x = (float *)pm_contents(b_x);
    for (int c = 0; c < cols; c++) x[c] = (float)((c % 11) - 5) * 0.25f;

    /* 1. Verify Metal ink_dv_nvfp4 against exact CPU FP4 E2M1 + E4M3 + global_scale reference */
    const float gscale = 0.5f;
    uint32_t *nv_p = (uint32_t *)pm_contents(b_nv_p);
    uint8_t  *nv_sb = (uint8_t *)pm_contents(b_nv_s);
    const uint8_t e4m3_scales[4] = {0x38, 0x40, 0x30, 0x04}; /* 1.0, 2.0, 0.5, subnormal 4/512 */
    for (int r = 0; r < rows; r++) {
        for (int b = 0; b < cols / 16; b++) {
            int bi = r * (cols / 16) + b;
            uint32_t w0 = 0, w1 = 0;
            for (int j = 0; j < 8; j++) {
                uint32_t n0 = (uint32_t)((r * 3 + b * 5 + j) & 15);
                uint32_t n1 = (uint32_t)((r * 7 + b * 2 + j + 8) & 15);
                w0 |= n0 << (j * 4);
                w1 |= n1 << (j * 4);
            }
            nv_p[bi * 2] = w0; nv_p[bi * 2 + 1] = w1;
            nv_sb[bi] = e4m3_scales[(r + b) & 3];
        }
    }
    PmBatch batch = NULL;
    if (pm_begin(&batch) != PM_OK) { snprintf(err, err_bytes, "pm_begin: %s", pm_log()); goto cleanup; }
    PmArg nv_args[7] = {
        a_int(rows), a_int(cols), a_f32(gscale),
        a_u32v(b_nv_p, 0, rows * (cols / 16) * 2),
        a_u32v(b_nv_s, 0, (rows * (cols / 16)) / 4),
        a_f32v(b_x, 0, cols), a_f32v(b_out, 0, rows)
    };
    if (pm_launch(batch, k_nv, rows, 32, 32, nv_args, 7) != PM_OK || pm_wait(batch, NULL) != PM_OK) {
        snprintf(err, err_bytes, "ink_dv_nvfp4 launch: %s", pm_log()); goto cleanup;
    }
    const float *got = (const float *)pm_contents(b_out);
    float max_nv = 0.0f;
    for (int r = 0; r < rows; r++) {
        float ref = 0.0f;
        for (int b = 0; b < cols / 16; b++) {
            int bi = r * (cols / 16) + b;
            float sc = cpu_fp8_e4m3(nv_sb[bi]) * gscale;
            float s = 0.0f;
            for (int j = 0; j < 8; j++) {
                s += cpu_nvfp4_val((nv_p[bi * 2] >> (j * 4)) & 15u) * x[b * 16 + j];
                s += cpu_nvfp4_val((nv_p[bi * 2 + 1] >> (j * 4)) & 15u) * x[b * 16 + 8 + j];
            }
            ref += sc * s;
        }
        float d = fabsf(got[r] - ref);
        if (d > max_nv) max_nv = d;
    }
    if (max_nv > 1e-5f) { snprintf(err, err_bytes, "ink_dv_nvfp4 mismatch: max_abs=%g", max_nv); goto cleanup; }

    /* 2. Verify Metal ink_dv_mxfp8 against exact CPU E4M3 + UE8M0 reference */
    uint8_t *mx_wb = (uint8_t *)pm_contents(b_mx_w);
    uint8_t *mx_sb = (uint8_t *)pm_contents(b_mx_s);
    const uint8_t e4m3_vals[8] = {0x38, 0xb8, 0x40, 0xc0, 0x34, 0x06, 0x44, 0xbc};
    for (int r = 0; r < rows; r++) {
        for (int b = 0; b < cols / 32; b++) {
            int bi = r * (cols / 32) + b;
            mx_sb[bi] = (uint8_t)(125 + ((r + b) % 5));
            for (int j = 0; j < 32; j++) mx_wb[bi * 32 + j] = e4m3_vals[(r + b * 3 + j) & 7];
        }
    }
    if (pm_begin(&batch) != PM_OK) { snprintf(err, err_bytes, "pm_begin: %s", pm_log()); goto cleanup; }
    PmArg mx_args[6] = {
        a_int(rows), a_int(cols),
        a_u32v(b_mx_w, 0, (rows * cols) / 4),
        a_u32v(b_mx_s, 0, (rows * (cols / 32)) / 4),
        a_f32v(b_x, 0, cols), a_f32v(b_out, 0, rows)
    };
    if (pm_launch(batch, k_mx, rows, 32, 32, mx_args, 6) != PM_OK || pm_wait(batch, NULL) != PM_OK) {
        snprintf(err, err_bytes, "ink_dv_mxfp8 launch: %s", pm_log()); goto cleanup;
    }
    float max_mx = 0.0f;
    for (int r = 0; r < rows; r++) {
        float ref = 0.0f;
        for (int b = 0; b < cols / 32; b++) {
            int bi = r * (cols / 32) + b;
            float sc = cpu_ue8m0(mx_sb[bi]);
            float s = 0.0f;
            for (int j = 0; j < 32; j++) s += cpu_fp8_e4m3(mx_wb[bi * 32 + j]) * x[b * 32 + j];
            ref += sc * s;
        }
        float d = fabsf(got[r] - ref);
        if (d > max_mx) max_mx = d;
    }
    if (max_mx > 1e-5f) { snprintf(err, err_bytes, "ink_dv_mxfp8 mismatch: max_abs=%g", max_mx); goto cleanup; }

    /* 3. Verify Metal ink_dv_iq4 against exact CPU IQ4_NL reference */
    uint16_t *iq_w = (uint16_t *)pm_contents(b_iq_w);
    const uint16_t f16_scales[4] = {0x3800, 0x3c00, 0x3400, 0xbc00}; /* 0.5, 1.0, 0.25, -1.0 */
    const float f16_vals[4] = {0.5f, 1.0f, 0.25f, -1.0f};
    for (int r = 0; r < rows; r++) {
        for (int b = 0; b < cols / 32; b++) {
            uint16_t *blk = iq_w + (r * (cols / 32) + b) * 9;
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
    if (pm_begin(&batch) != PM_OK) { snprintf(err, err_bytes, "pm_begin: %s", pm_log()); goto cleanup; }
    PmArg iq_args[6] = {
        a_int(rows), a_int(cols), a_int(0),
        a_u16v(b_iq_w, 0, rows * (cols / 32) * 9),
        a_f32v(b_x, 0, cols), a_f32v(b_out, 0, rows)
    };
    if (pm_launch(batch, k_iq, (rows + INK_D - 1) / INK_D, INK_T, INK_T, iq_args, 6) != PM_OK || pm_wait(batch, NULL) != PM_OK) {
        snprintf(err, err_bytes, "ink_dv_iq4 launch: %s", pm_log()); goto cleanup;
    }
    float max_iq = 0.0f;
    for (int r = 0; r < rows; r++) {
        float ref = 0.0f;
        for (int b = 0; b < cols / 32; b++) {
            const uint16_t *blk = iq_w + (r * (cols / 32) + b) * 9;
            float sc = f16_vals[(r + b) & 3];
            float s = 0.0f;
            for (int k = 0; k < 8; k++) {
                uint16_t v = blk[1 + k];
                s += cpu_iq4_val(v & 15u) * x[b * 32 + 2 * k]
                   + cpu_iq4_val((v >> 4) & 15u) * x[b * 32 + 16 + 2 * k]
                   + cpu_iq4_val((v >> 8) & 15u) * x[b * 32 + 2 * k + 1]
                   + cpu_iq4_val((v >> 12) & 15u) * x[b * 32 + 16 + 2 * k + 1];
            }
            ref += sc * s;
        }
        float d = fabsf(got[r] - ref);
        if (d > max_iq) max_iq = d;
    }
    if (max_iq > 1e-4f) { snprintf(err, err_bytes, "ink_dv_iq4 mismatch: max_abs=%g", max_iq); goto cleanup; }

    fprintf(stderr,
            "metal-quant-kernels device='%s' compile=%.2fs nvfp4_max_abs=%.2e mxfp8_max_abs=%.2e iq4_max_abs=%.2e ok=true\n",
            info.name, compile_s, max_nv, max_mx, max_iq);
    rc = 0;

cleanup:
    if (b_x) pm_free(b_x);
    if (b_out) pm_free(b_out);
    if (b_nv_p) pm_free(b_nv_p);
    if (b_nv_s) pm_free(b_nv_s);
    if (b_mx_w) pm_free(b_mx_w);
    if (b_mx_s) pm_free(b_mx_s);
    if (b_iq_w) pm_free(b_iq_w);
    pm_kernel_free(k_nv); pm_kernel_free(k_mx); pm_kernel_free(k_iq);
    pm_unload(mod);
    return rc;
}
