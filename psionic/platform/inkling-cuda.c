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
#define INK_OA  256
#define INK_NT  256
#define INK_MT  1024
#define INK_D   8
#define INK_MAX_BLOCKS 8
#define INK_MAX_EXPERTS 256
#define INK_MAX_SNAPS 8
#define INK_MAX_SWA_SNAPS 4
/* A layer's cache holds at least this many experts: two steps' worth, so the
   experts one step chose are never evicted while that step still needs them. */
#define INK_MIN_EXPERT_SLOTS 16
/* VRAM left unplanned: the kernel module, driver bookkeeping and whatever
   else the desktop allocates while the model runs. */
#define INK_VRAM_MARGIN_MB 384
#define INK_LIKING_RATE 0.3f   /* weight of the newest router logit in an expert's running mean */
#define INK_LAYER_TENSORS ((int)(sizeof(InkLayerTensors) / sizeof(InkTensor)))

/* Tokens a prefill batch may hold. Decode is a batch of one. */
#define INK_MAX_BATCH 128

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
    K_SUM_POST,
    K_BF16_PACK,
    K_Q8_MMA,
    K_PROJ_MMA,
    K_MOE_GU_IQ4_MMA,
    K_MOE_GU_Q8_MMA,
    K_MOE_SITU_PACK,
    K_MOE_DOWN_IQ4_MMA,
    K_MOE_DOWN_Q8_MMA,
    K_MOE_REDUCE_POST,
    K_ATTN_ONLINE,
    K_CTRL_PACK_BF16,
    K_CTRL_GATE_MMA,
    K_CTRL_OUT_MMA,
    K_Q8_TO_IQ4_RELAYOUT,
    K_ARGMAX_S1,
    K_ARGMAX_S2,
    K_TOPK_FILTER,
    K_MOE_PLAN_NB,
    K_MOE_REDUCE_POST_LOG,
    K_CONV_REWIND_ALL,
    K_Q8_MMA_NB8_SK,
    K_SK_REDUCE,
    K_PROJ_MMA_NB8_SK,
    K_PROJ_SK_REDUCE,
    K_MOE_GU_IQ4_NB8,
    K_MOE_DOWN_IQ4_NB8,
    K_BF16_TO_IQ4_RELAYOUT,
    K_BF16_TO_IQ4_ROW,
    K_F32T_TO_IQ4_ROW,
    K_IQ4_MMA_NB8_SK,
    K_DSPARK_MIX_NORM,
    K_DSPARK_KV_ABSORB,
    K_DSPARK_BLOCK_PREP,
    K_DSPARK_ADD_NORM_BF16,
    K_DSPARK_QKV_ROPE,
    K_DSPARK_ATTN_SK,
    K_DSPARK_ATTN_REDUCE,
    K_DSPARK_SILU_MUL_BF16,
    K_DSPARK_MARKOV_CTX_PREP,
    K_DSPARK_MARKOV_STEP,
    K_DSPARK_FOLD_LOGITS_S1,
    K_DSPARK_LOGITS_S2,
    K_KV_STORE_FP8,
    K_ATTN_ONLINE_SK,
    K_ATTN_SK_REDUCE,
    K_CTRL_RMS,
    K_CTRL_GATE_NB8_SK,
    K_KV_NB,
    K_ATTN_POST_LOG,
    K_DVR_F32_NB8,
    K_PROJ_IQ4_P,
    K_PROJ_IQ4_MMA_NB8_SK,
    K_ABLATE_DIR,
    K_MOE_SUM_DOWN,
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
    "ink_sum_post",
    "ink_bf16_pack",
    "ink_q8_mma",
    "ink_proj_mma",
    "ink_moe_gu_iq4_mma",
    "ink_moe_gu_q8_mma",
    "ink_moe_situ_pack",
    "ink_moe_down_iq4_mma",
    "ink_moe_down_q8_mma",
    "ink_moe_reduce_post",
    "ink_attn_online",
    "ink_ctrl_pack_bf16",
    "ink_ctrl_gate_mma",
    "ink_ctrl_out_mma",
    "ink_q8_to_iq4_relayout",
    "ink_argmax_s1",
    "ink_argmax_s2",
    "ink_topk_filter",
    "ink_moe_plan_nb",
    "ink_moe_reduce_post_log",
    "ink_conv_rewind_all",
    "ink_q8_mma_nb8_sk",
    "ink_sk_reduce",
    "ink_proj_mma_nb8_sk",
    "ink_proj_sk_reduce",
    "ink_moe_gu_iq4_nb8",
    "ink_moe_down_iq4_nb8",
    "ink_bf16_to_iq4_relayout",
    "ink_bf16_to_iq4_row",
    "ink_f32t_to_iq4_row",
    "ink_iq4_mma_nb8_sk",
    "ink_dspark_mix_norm",
    "ink_dspark_kv_absorb",
    "ink_dspark_block_prep",
    "ink_dspark_add_norm_bf16",
    "ink_dspark_qkv_rope",
    "ink_dspark_attn_sk",
    "ink_dspark_attn_reduce",
    "ink_dspark_silu_mul_bf16",
    "ink_dspark_markov_ctx_prep",
    "ink_dspark_markov_step",
    "ink_dspark_fold_logits_s1",
    "ink_dspark_logits_s2",
    "ink_kv_store_fp8",
    "ink_attn_online_sk",
    "ink_attn_sk_reduce",
    "ink_ctrl_rms",
    "ink_ctrl_gate_nb8_sk",
    "ink_kv_nb",
    "ink_attn_post_log",
    "ink_dvr_f32_nb8",
    "ink_proj_iq4_p",
    "ink_proj_iq4_mma_nb8_sk",
    "ink_ablate_dir",
    "ink_moe_sum_down",
};

/* Threads per work item: >1 for group kernels (`for thread: 0..<T`), -1 for
   warp kernels (`for lane: 0..<32`, launched with 4 warps/block = 128 threads),
   or 1 for one-thread-per-item kernels. */
static const int k_threads[K_COUNT] = {
    INK_NT, /* K_EMBED_Q8 */
    INK_NT, /* K_RMSNORM */
    INK_T,  /* K_DV_IQ4 (the quant-kernel test only) */
    INK_MT, /* K_DVQ */
    INK_NT, /* K_DVR_F32 */
    1,      /* K_DVS_F32 */
    INK_MT, /* K_MOE_GU */
    INK_MT, /* K_MOE_DOWN */
    INK_MT, /* K_ROUTER */
    INK_NT, /* K_MIX */
    INK_MT, /* K_CTRL_PREP: ink_ctrl_gate */
    INK_NT, /* K_CTRL_APPLY: ink_ctrl_out */
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
    INK_MT, /* K_SUM_POST */
    1,      /* K_BF16_PACK */
    -1,     /* K_Q8_MMA */
    -1,     /* K_PROJ_MMA */
    -1,     /* K_MOE_GU_IQ4_MMA */
    -1,     /* K_MOE_GU_Q8_MMA */
    1,      /* K_MOE_SITU_PACK */
    -1,     /* K_MOE_DOWN_IQ4_MMA */
    -1,     /* K_MOE_DOWN_Q8_MMA */
    1,      /* K_MOE_REDUCE_POST */
    INK_OA, /* K_ATTN_ONLINE */
    INK_NT, /* K_CTRL_PACK_BF16 */
    -1,     /* K_CTRL_GATE_MMA */
    -1,     /* K_CTRL_OUT_MMA */
    1,      /* K_Q8_TO_IQ4_RELAYOUT */
    INK_NT, /* K_ARGMAX_S1 */
    INK_T,  /* K_ARGMAX_S2 */
    128,    /* K_TOPK_FILTER */
    128,    /* K_MOE_PLAN_NB */
    1,      /* K_MOE_REDUCE_POST_LOG */
    1,      /* K_CONV_REWIND_ALL */
    -1,     /* K_Q8_MMA_NB8_SK */
    1,      /* K_SK_REDUCE */
    -1,     /* K_PROJ_MMA_NB8_SK */
    1,      /* K_PROJ_SK_REDUCE */
    -1,     /* K_MOE_GU_IQ4_NB8 */
    -1,     /* K_MOE_DOWN_IQ4_NB8 */
    1,      /* K_BF16_TO_IQ4_RELAYOUT */
    1,      /* K_BF16_TO_IQ4_ROW */
    1,      /* K_F32T_TO_IQ4_ROW */
    -1,     /* K_IQ4_MMA_NB8_SK */
    INK_NT, /* K_DSPARK_MIX_NORM */
    INK_T,  /* K_DSPARK_KV_ABSORB */
    INK_NT, /* K_DSPARK_BLOCK_PREP */
    INK_NT, /* K_DSPARK_ADD_NORM_BF16 */
    INK_NT, /* K_DSPARK_QKV_ROPE */
    INK_NT, /* K_DSPARK_ATTN_SK */
    128,    /* K_DSPARK_ATTN_REDUCE */
    1,      /* K_DSPARK_SILU_MUL_BF16 */
    1,      /* K_DSPARK_MARKOV_CTX_PREP */
    INK_NT, /* K_DSPARK_MARKOV_STEP */
    INK_NT, /* K_DSPARK_FOLD_LOGITS_S1 */
    INK_NT, /* K_DSPARK_LOGITS_S2 */
    1,      /* K_KV_STORE_FP8 */
    INK_OA, /* K_ATTN_ONLINE_SK */
    128,    /* K_ATTN_SK_REDUCE */
    INK_NT, /* K_CTRL_RMS */
    INK_T,  /* K_CTRL_GATE_NB8_SK */
    INK_NT, /* K_KV_NB */
    1,      /* K_ATTN_POST_LOG */
    INK_NT, /* K_DVR_F32_NB8 */
    1,      /* K_PROJ_IQ4_P */
    -1,     /* K_PROJ_IQ4_MMA_NB8_SK */
    INK_NT, /* K_ABLATE_DIR */
    1,      /* K_MOE_SUM_DOWN */
};

typedef struct {
    int64_t off;
    int64_t bytes;
} MtpSpan;

typedef struct {
    int is_swa, kv_cap;
    PcBuffer kv_k, kv_v;
    PcBuffer dflare_norm, input_ln, post_attn_ln, q_norm, k_norm, k_norm_ctx;
    PcBuffer qkv_q, qkv_d, o_q, o_d, gu_q, gu_d, dn_q, dn_d;
    PcBuffer kctx_q, kctx_d, vctx_q, vctx_d;
} CudaDsparkLayer;

typedef struct {
    int ready;
    int kv_fp8;
    float fuse_alpha[4][4];
    PcBuffer small_buf, weights_buf, scratch_buf;
    PcBuffer mask_embed, norm_w, mh_dflare_norm, mh_rnn_bias, rnn_w_step, inv_freq;
    PcBuffer rnn_ctx_q, rnn_ctx_d, mw1_q, mw1_d, fold_q, fold_d, fold_b;
    CudaDsparkLayer layers[3];
    PcBuffer taps, last_taps, ctx_bf16, ctx_k_f32, ctx_v_f32;
    PcBuffer anchor_emb, f_rnn_bf16, res, x_bf16, qkv_f32, attn_bf16, tmp_f32;
    PcBuffer q_rot, k_rot, v_blk, attn_sk_o, attn_sk_ms;
    PcBuffer gu_f32, mlp_bf16, markov_in_bf16, rnn_ctx;
    PcBuffer rnn_state, rnn_state2, rnn_latent, base_logits;
    PcBuffer s1_m, s1_i, s1_s, draft_toks, draft_probs, dev_pos;
    uint32_t pos_host;
    PcGraph  draft_graph[8];
    PcGraph  absorb_graph[9][9];
    int pos, pending_pos0, pending_nb, pending_got;
    int skip_countdown, skip_mult, req_rounds, req_disabled;
    float ema_acc, util_ema;
    double b1_ema_s, last_draft_s, last_verify_s, draft_gpu_s;
    uint64_t rounds, drafted, accepted;
} CudaDspark;

/* One layer's experts on the device. cap == n_expert means every expert is
   resident in file order and ids need no translation. */
typedef struct {
    int      cap;
    uint32_t gu_type, dn_type;            /* device type (may be IQ4_NL quantized on-load from Q8_0 to fit 16 GB VRAM) */
    size_t   src_gu_bytes, src_dn_bytes;  /* one expert's raw GGUF bytes */
    size_t   gu_bytes, dn_bytes;          /* one expert's device gate_up and down blocks */
    PcBuffer gu, dn;                      /* cap experts each */
    int16_t  slot_of[INK_MAX_EXPERTS];    /* expert -> slot, or -1 */
    int16_t  expert_in[INK_MAX_EXPERTS];  /* slot -> expert, or -1 */
    uint64_t used[INK_MAX_EXPERTS];       /* slot -> tick of its last use */
    float    liking[INK_MAX_EXPERTS];     /* expert -> running mean of the router's logit for it */
    int      liking_set;
} ExpertCache;

struct InkSession {
    InkGGUF gguf;
    InkLora lora;
    int has_ablate;
    int ablate_has_emb;
    uint8_t ablate_has_layer[32];
    float *ablate_emb_dir;
    PcBuffer ablate_dirs;
    int embd_is_f32;
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
    struct { int n_mats, is_iq4; int64_t items, rows, rtiles; } proj_list[32][2];
    PcBuffer attn_scores, attn_out, o_raw, norm_buf;
    PcBuffer attn_sk_o, attn_sk_ms, ctrl_inv;
    int kv_fp8;
    int output_iq4;
    int fast_q4;
    PcBuffer router_logits;
    PcBuffer moe_ids;                                /* [batch, n_expert_used] u32, expert ids from the router */
    PcBuffer moe_slots;                              /* [batch, n_expert_used] u32, cache slots of those experts */
    PcBuffer moe_wts, moe_hid, moe_out;
    PcBuffer x_bf16;                                 /* packed BF16 activations for Tensor Core prefill */
    PcBuffer moe_gu_out, moe_hid_bf16, moe_down_out; /* grouped MoE Tensor Core prefill buffers */
    PcBuffer moe_tasks;                              /* grouped MoE task descriptors (slot/base/cnt/tok/pair) */
    size_t x_bf16_words, moe_tasks_words;
    PcBuffer mv_part;                                /* one token's matvec shares: [blocks, rows] */
    size_t mv_part_floats;
    PcBuffer gu_part, dn_part;                       /* one token's table-kernel shares */
    int no_tables;                                   /* INK_NO_TABLES: IQ4_NL experts through the direct kernels */
    int no_mma;                                      /* INK_NO_MMA: disable Tensor Core prefill */
    int no_graph;                                    /* INK_NO_GRAPH: disable CUDA Graph decode */
    int capturing;                                   /* inside pc_capture_begin..end: kernels read pos from dev_pos */
    int use_online_attn;                             /* single-pass online-softmax attention (zero attn_scores) */
    uint32_t *moe_tasks_host;                        /* host staging buffer for grouped MoE descriptors */
    PcBuffer dev_pos;                                /* [1] u32, device-side token position for graph replay */
    uint32_t pos_host;                               /* stable host storage for dev_pos upload */
    PcGraph  decode_graph[2];                        /* full-step resident decode graphs [compute_logits] */
    PcGraph  verify_graph[9];                        /* full-step resident speculative verify graphs [nb = 2..8] */
    PcGraph  stream_seg[INK_MAX_SLOTS];              /* streaming decode graphs between expert syncs */
    PcGraph  stream_tail[2];                         /* streaming decode final MoE + optional logits [compute_logits] */
    int      seg_ip[INK_MAX_SLOTS];                  /* physical layer ip for each streaming segment */
    int      stream_segs_ready;
    int      last_il, last_ip, final_n_blocks;
    PcBuffer logits;
    PcBuffer argmax_vals, argmax_ids, argmax_out;    /* [64] f32, [64] u32, [1] u32 GPU argmax */
    PcBuffer topk_cnt, topk_ids, topk_vals;          /* [256] u32, [256*16] u32, [256*16] f32 sparse GPU top-k */
    uint32_t argmax_host;
    uint32_t topk_cnt_host[256];
    uint32_t topk_ids_host[256 * 16];
    float    topk_vals_host[256 * 16];
    int      argmax_valid;
    int      logits_host_valid;
    int      hidden_host_valid;

    /* Stage C3A: CUDA Graph speculative verification (nb = 2..8) + GPU shortconv rewind + PLD/Prediction/SessHist. */
    int      can_verify_nb;
    int      spec_recording;                         /* inside verify_graph capture: log raw conv inputs */
    PcBuffer spec_conv_bak;                          /* [conv_all_elems] f32 pre-batch conv snapshot */
    PcBuffer spec_raw_log;                           /* [n_slots * 8 * (2*n_kv + 2*d)] f32 raw conv inputs */
    size_t   spec_raw_log_elems;
    PcBuffer spec_logits;                            /* [8 * n_vocab] f32 */
    PcBuffer spec_hidden;                            /* [8 * d] f32 */
    PcBuffer spec_argmax_out;                        /* [8] u32 */
    PcBuffer spec_topk_cnt;                          /* [8 * 256] u32 */
    PcBuffer spec_topk_ids;                          /* [8 * 256 * 16] u32 */
    PcBuffer spec_topk_vals;                         /* [8 * 256 * 16] f32 */
    uint32_t spec_argmax_host[8];
    uint32_t spec_topk_cnt_host[8 * 256];
    uint32_t spec_topk_ids_host[8 * 256 * 16];
    float    spec_topk_vals_host[8 * 256 * 16];
    int32_t  spec_q[8];
    int      spec_head;
    int      spec_tail;
    int      spec_active_idx;                        /* >= 0: active logits/hidden live in spec_logits/spec_hidden[idx] */
    int      conv_pending_nb;                        /* total tokens run in last verify batch */
    int      conv_pending_n;                         /* how many of those tokens have been consumed */
    int32_t *tok_hist;                               /* [max_ctx] prompt + generated tokens for PLD */
    int      tok_hist_len;
    int32_t *pred_toks;                              /* optional OpenAI prediction token buffer */
    int      pred_len;
    int      pred_cap;
    int32_t *sess_hist;                              /* cross-turn output history ring buffer (SuffixDecoding-lite) */
    int      sess_hist_len;
    int      sess_hist_cap;
    float    pld_ema2, pld_ema3;
    uint64_t spec_rounds, spec_drafted, spec_accepted;
    CudaDspark dspark;

    PcBuffer dummy_f32;
    PcBuffer oracle_x;                               /* INK_ORACLE_Q8: a matvec input rounded as ggml rounds it */
    int oracle_q8;

    /* Per logical slot: recurrent shortconv state and KV cache (f16). */
    int n_slots;
    int max_ctx;
    InkMemoryBudget budget;
    int kv_cap[INK_MAX_SLOTS];
    PcBuffer kv_k[INK_MAX_SLOTS], kv_v[INK_MAX_SLOTS];
    PcBuffer conv_all;                               /* single contiguous [n_slots * (2*n_kv + 2*d) * 3] f32 */
    size_t   conv_all_elems;
    PcBuffer conv_k[INK_MAX_SLOTS], conv_v[INK_MAX_SLOTS], conv_ao[INK_MAX_SLOTS], conv_fo[INK_MAX_SLOTS];

    /* Host copies of what the engine contract returns by pointer. */
    float *logits_host;
    float *hidden_host;
    float *zeros;                                    /* the largest shortconv state, all zero */

    int pos;
    int max_pos_written;
    int swa_cap_min;
    int swa_valid_from;
    int dspark_live_len;
    int dspark_swa_valid_from;
    int32_t *live_tokens;
    int live_len;
    uint64_t snap_clock;
    struct {
        int valid;
        int pos;
        int has_logits;
        int has_dspark;
        int pinned;
        uint64_t last_used;
        int32_t *tokens;
        float   *conv_state;
        float   *logits;
        uint8_t *swa_kv;
        int      swa_win;
        float   *dspark_last_taps;
        uint8_t *dspark_swa_kv;
        int      dspark_swa_win;
    } snaps[INK_MAX_SNAPS];
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
        : (k_threads[k] < 0
            ? pc_launch_warps(s->kernels[k], items, 128, args, count, s->stream)
            : pc_launch(s->kernels[k], items, 256, args, count, s->stream));
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

static uint16_t cuda_f32_to_f16(float x) {
    uint32_t u; memcpy(&u, &x, 4);
    uint32_t s = (u >> 16) & 0x8000u;
    int32_t e = (int32_t)((u >> 23) & 0xFFu) - 127;
    uint32_t m = u & 0x7FFFFFu;
    if (e == 128) return (uint16_t)(s | 0x7C00u | (m ? 1u : 0u));
    if (e > 15) return (uint16_t)(s | 0x7BFFu);
    if (e < -24) return (uint16_t)s;
    if (e < -14) {
        m |= 0x800000u;
        uint32_t sh = (uint32_t)(-14 - e);
        return (uint16_t)(s | ((m >> (sh + 13)) & 0x3FFu));
    }
    uint32_t m10 = (m + 0x1000u) >> 13;
    if (m10 == 0x400u) { m10 = 0; e++; if (e > 15) return (uint16_t)(s | 0x7BFFu); }
    return (uint16_t)(s | ((uint32_t)(e + 15) << 10) | m10);
}

static float cuda_f16_to_f32(uint16_t h) {
    uint32_t s = ((uint32_t)h & 0x8000u) << 16;
    uint32_t e = (h >> 10) & 0x1Fu;
    uint32_t m = h & 0x3FFu;
    uint32_t u;
    if (e == 0) {
        if (m == 0) u = s;
        else {
            float v = (float)m * (1.0f / 16777216.0f);
            return s ? -v : v;
        }
    } else if (e == 31) {
        u = s | 0x7F800000u | (m << 13);
    } else {
        u = s | ((e + 112u) << 23) | (m << 13);
    }
    float out; memcpy(&out, &u, 4); return out;
}

static size_t cuda_ggml_row_bytes(uint32_t type, int64_t cols) {
    int64_t be = ink_ggml_block_elems(type), bb = ink_ggml_block_bytes(type);
    return (be > 0 && bb > 0) ? (size_t)((cols / be) * bb) : 0;
}

/* ---- Rows of a tensor converted on several CPU threads. ----
   A GGUF in a type the kernels do not read (Q4_K, IQ3_S, IQ2_S, ...) is
   converted row by row at load. The rows are independent, and one thread took
   168 s for the 3072 expert matrices of an IQ2_M file, so they are split over
   the CPU's threads. INK_LOAD_THREADS sets the count (1 turns it off). */
typedef void (*RowsFn)(void *ctx, int64_t r0, int64_t r1, float *tmp);
typedef struct { RowsFn fn; void *ctx; int64_t r0, r1, cols; int failed; } RowsJob;

static void rows_job_run(RowsJob *j) {
    float *tmp = (float *)malloc((size_t)(j->cols > 0 ? j->cols : 1) * sizeof(float));
    if (!tmp) { j->failed = 1; return; }
    j->fn(j->ctx, j->r0, j->r1, tmp);
    free(tmp);
}

#ifdef _WIN32
#include <windows.h>
static DWORD WINAPI rows_job_thread(LPVOID p) { rows_job_run((RowsJob *)p); return 0; }
static int rows_cpu_count(void) { SYSTEM_INFO si; GetSystemInfo(&si); return (int)si.dwNumberOfProcessors; }
#else
#include <pthread.h>
#include <unistd.h>
static void *rows_job_thread(void *p) { rows_job_run((RowsJob *)p); return NULL; }
static int rows_cpu_count(void) { long n = sysconf(_SC_NPROCESSORS_ONLN); return n > 0 ? (int)n : 1; }
#endif

#define ROWS_MAX_THREADS 32
static int rows_thread_count(void) {
    static int cached = 0;
    if (cached > 0) return cached;
    int n = rows_cpu_count();
    const char *e = getenv("INK_LOAD_THREADS");
    if (e && e[0]) n = atoi(e);
    if (n < 1) n = 1;
    if (n > ROWS_MAX_THREADS) n = ROWS_MAX_THREADS;
    cached = n;
    return n;
}

/* fn(ctx, r0, r1, tmp) over [0, rows) in contiguous ranges; tmp holds cols
   floats and belongs to the calling thread. 0 on success, 1 when out of memory. */
static int rows_parallel(RowsFn fn, void *ctx, int64_t rows, int64_t cols) {
    int nt = rows_thread_count();
    if (rows < 64) nt = 1;
    if ((int64_t)nt > rows / 16) nt = (int)(rows / 16 > 0 ? rows / 16 : 1);
    RowsJob jobs[ROWS_MAX_THREADS];
    for (int t = 0; t < nt; t++) {
        jobs[t].fn = fn; jobs[t].ctx = ctx; jobs[t].cols = cols; jobs[t].failed = 0;
        jobs[t].r0 = rows * t / nt; jobs[t].r1 = rows * (t + 1) / nt;
    }
#ifdef _WIN32
    HANDLE th[ROWS_MAX_THREADS];
    for (int t = 1; t < nt; t++) th[t] = CreateThread(NULL, 0, rows_job_thread, &jobs[t], 0, NULL);
    rows_job_run(&jobs[0]);
    for (int t = 1; t < nt; t++) {
        if (th[t]) { WaitForSingleObject(th[t], INFINITE); CloseHandle(th[t]); }
        else rows_job_run(&jobs[t]);
    }
#else
    pthread_t th[ROWS_MAX_THREADS];
    int started[ROWS_MAX_THREADS];
    for (int t = 1; t < nt; t++) started[t] = pthread_create(&th[t], NULL, rows_job_thread, &jobs[t]) == 0;
    rows_job_run(&jobs[0]);
    for (int t = 1; t < nt; t++) {
        if (started[t]) pthread_join(th[t], NULL);
        else rows_job_run(&jobs[t]);
    }
#endif
    for (int t = 0; t < nt; t++) if (jobs[t].failed) return 1;
    return 0;
}

typedef struct { uint32_t src_type; const uint8_t *src; size_t src_rb; uint8_t *dst; size_t dst_rb; int64_t cols, nblk; float *out; } RepackCtx;

static void repack_q8_rows(void *raw, int64_t r0, int64_t r1, float *tmp) {
    RepackCtx *x = (RepackCtx *)raw;
    uint32_t src_type = x->src_type; const uint8_t *src = x->src; size_t src_rb = x->src_rb, dst_rb = x->dst_rb;
    uint8_t *dst = x->dst; int64_t cols = x->cols, nblk = x->nblk;
    for (int64_t r = r0; r < r1; r++) {
        ink_ggml_dequant_row(src_type, src + (size_t)r * src_rb, tmp, cols);
        uint8_t *orow = dst + (size_t)r * dst_rb;
        for (int64_t b = 0; b < nblk; b++) {
            const float *fb = tmp + b * 32;
            uint8_t *ob = orow + (size_t)b * 34;
            float max_a = 0.0f;
            for (int i = 0; i < 32; i++) { float a = fabsf(fb[i]); if (a > max_a) max_a = a; }
            float d8 = (max_a > 0.0f) ? (max_a / 127.0f) : 0.0f;
            uint16_t d8u = cuda_f32_to_f16(d8);
            memcpy(ob, &d8u, 2);
            float d8f = cuda_f16_to_f32(d8u);
            float inv = (d8f != 0.0f) ? (1.0f / d8f) : 0.0f;
            int8_t *qs = (int8_t *)(ob + 2);
            for (int i = 0; i < 32; i++) {
                int q = (int)lrintf(fb[i] * inv);
                if (q < -127) q = -127; else if (q > 127) q = 127;
                qs[i] = (int8_t)q;
            }
        }
    }
}

static uint8_t *ggml_repack_to_q8_0(uint32_t src_type, const uint8_t *src, int64_t rows, int64_t cols) {
    RepackCtx x = { .src_type = src_type, .src = src, .src_rb = cuda_ggml_row_bytes(src_type, cols), .cols = cols, .nblk = cols / 32 };
    x.dst_rb = (size_t)x.nblk * 34;
    x.dst = (uint8_t *)malloc((size_t)rows * x.dst_rb);
    if (!x.dst) return NULL;
    if (rows_parallel(repack_q8_rows, &x, rows, cols)) { free(x.dst); return NULL; }
    return x.dst;
}

static void repack_iq4_rows(void *raw, int64_t r0, int64_t r1, float *tmp) {
    static const int8_t kvals[16] = {-127,-104,-83,-65,-49,-35,-22,-10,1,13,25,38,53,69,89,113};
    RepackCtx *x = (RepackCtx *)raw;
    uint32_t src_type = x->src_type; const uint8_t *src = x->src; size_t src_rb = x->src_rb, dst_rb = x->dst_rb;
    uint8_t *dst = x->dst; int64_t cols = x->cols, nblk = x->nblk;
    for (int64_t r = r0; r < r1; r++) {
        ink_ggml_dequant_row(src_type, src + (size_t)r * src_rb, tmp, cols);
        uint8_t *orow = dst + (size_t)r * dst_rb;
        for (int64_t b = 0; b < nblk; b++) {
            const float *fb = tmp + b * 32;
            uint8_t *ob = orow + (size_t)b * 18;
            float max_a = 0.0f, max_v = 0.0f;
            for (int i = 0; i < 32; i++) { float a = fabsf(fb[i]); if (a > max_a) { max_a = a; max_v = fb[i]; } }
            float d4 = (max_a > 0.0f) ? (max_v / -127.0f) : 0.0f;
            uint16_t d4u = cuda_f32_to_f16(d4);
            memcpy(ob, &d4u, 2);
            float d4f = cuda_f16_to_f32(d4u);
            float inv = (d4f != 0.0f) ? (1.0f / d4f) : 0.0f;
            for (int j = 0; j < 16; j++) {
                float v0 = fb[j] * inv, v1 = fb[j + 16] * inv;
                int b0 = 0, b1 = 0;
                float e0 = fabsf(v0 - (float)kvals[0]), e1 = fabsf(v1 - (float)kvals[0]);
                for (int q = 1; q < 16; q++) {
                    float d0 = fabsf(v0 - (float)kvals[q]); if (d0 < e0) { e0 = d0; b0 = q; }
                    float d1 = fabsf(v1 - (float)kvals[q]); if (d1 < e1) { e1 = d1; b1 = q; }
                }
                ob[2 + j] = (uint8_t)(b0 | (b1 << 4));
            }
        }
    }
}

static uint8_t *ggml_repack_to_iq4_nl(uint32_t src_type, const uint8_t *src, int64_t rows, int64_t cols) {
    RepackCtx x = { .src_type = src_type, .src = src, .src_rb = cuda_ggml_row_bytes(src_type, cols), .cols = cols, .nblk = cols / 32 };
    x.dst_rb = (size_t)x.nblk * 18;
    x.dst = (uint8_t *)malloc((size_t)rows * x.dst_rb);
    if (!x.dst) return NULL;
    if (rows_parallel(repack_iq4_rows, &x, rows, cols)) { free(x.dst); return NULL; }
    return x.dst;
}

static void dequant_f32_rows(void *raw, int64_t r0, int64_t r1, float *tmp) {
    RepackCtx *x = (RepackCtx *)raw;
    (void)tmp;
    for (int64_t r = r0; r < r1; r++)
        ink_ggml_dequant_row(x->src_type, x->src + (size_t)r * x->src_rb, x->out + r * x->cols, x->cols);
}

static float *ggml_dequant_tensor_f32(const InkTensor *t, const uint8_t *src) {
    int64_t cols = (int64_t)t->dims[0];
    int64_t rows = (t->ndims >= 2) ? (int64_t)t->dims[1] : 1;
    for (uint32_t d = 2; d < t->ndims; d++) rows *= (int64_t)t->dims[d];
    size_t src_rb = cuda_ggml_row_bytes(t->type, cols);
    float *out = (float *)malloc((size_t)(rows * cols) * sizeof(float));
    if (!out) return NULL;
    RepackCtx x = { .src_type = t->type, .src = src, .src_rb = src_rb, .cols = cols, .out = out };
    if (rows_parallel(dequant_f32_rows, &x, rows, cols)) { free(out); return NULL; }
    return out;
}

static int upload_embd_row(InkSession *s, int slot, int32_t tok_id) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd;
    size_t src_rb = (size_t)(s->gguf.tok_embd.nbytes / (uint64_t)hp->n_vocab);
    const uint8_t *src = s->gguf.map + s->gguf.tok_embd.offset + (size_t)tok_id * src_rb;
    if (!s->embd_is_f32) {
        if (pc_upload(s->embd_row + (uint64_t)slot * src_rb, src, src_rb, s->stream) != PC_OK)
            return host_fail(s, "upload embedding row");
        return 0;
    }
    float row[2304];
    ink_ggml_dequant_row(s->gguf.tok_embd.type, src, row, d);
    if (s->ablate_has_emb && s->ablate_emb_dir) {
        float dot = 0.0f;
        for (int64_t i = 0; i < d; i++) dot += row[i] * s->ablate_emb_dir[i];
        for (int64_t i = 0; i < d; i++) row[i] -= dot * s->ablate_emb_dir[i];
    }
    if (pc_upload(s->embd_row + (uint64_t)slot * (uint64_t)d * 4, row, (size_t)d * 4, s->stream) != PC_OK)
        return host_fail(s, "upload f32 embedding row");
    return 0;
}

static int enqueue_embed(InkSession *s, int64_t np, PcBuffer dst) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd;
    if (!s->embd_is_f32) {
        size_t row_bytes = (size_t)(s->gguf.tok_embd.nbytes / (uint64_t)hp->n_vocab);
        CALL(s, K_EMBED_Q8, np, a_int(0), a_int(d),
             a_u16v(s->embd_row, 0, np * (int64_t)(row_bytes / 2)),
             a_tensor_f32(s, &s->gguf.tok_norm), a_f32(hp->rms_eps),
             a_f32v(dst, 0, np * d));
        return 0;
    }
    CALL(s, K_RMSNORM, np, a_int(d), a_int(1), a_f32(hp->rms_eps),
         a_f32v(s->embd_row, 0, np * d), a_tensor_f32(s, &s->gguf.tok_norm),
         a_f32v(dst, 0, np * d));
    return 0;
}

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

/* Quantize a Q8_0 GGUF matrix on-device directly into block-major IQ4_NL device order. */
static int relayout_q8_to_iq4(InkSession *s, const uint8_t *src, PcBuffer dst, int64_t rows, int64_t blocks, int64_t pair) {
    int64_t row_bytes = blocks * 34;
    int64_t piece = (int64_t)s->stage_bytes / row_bytes;
    if (piece > rows) piece = rows;
    if (piece <= 0 || (pair > 0 && piece < rows)) {
        snprintf(s->error, sizeof s->error, "staging buffer too small for Q8->IQ4 matrix (%lld-byte rows)", (long long)row_bytes);
        return 1;
    }
    for (int64_t row0 = 0; row0 < rows; row0 += piece) {
        int64_t n = rows - row0 < piece ? rows - row0 : piece;
        if (pc_upload(s->stage, src + row0 * row_bytes, (size_t)(n * row_bytes), s->stream) != PC_OK) return host_fail(s, "upload q8 weights");
        CALL(s, K_Q8_TO_IQ4_RELAYOUT, n * blocks, a_int(rows), a_int(row0), a_int(n), a_int(blocks), a_int(pair),
             a_view(s->stage, 0, n * row_bytes / 2),
             a_view(dst, 0, rows * blocks * 4), a_view(dst, rows * blocks * 16, rows * blocks));
    }
    return 0;
}

/* One expert of layer ip from the GGUF into cache slot `slot`. */
static int load_expert(InkSession *s, int ip, int e, int slot) {
    const InkHParams *hp = &s->gguf.hp;
    const InkLayerTensors *L = &s->gguf.layers[ip];
    ExpertCache *c = &s->experts[ip];
    const uint8_t *gu = s->gguf.map + L->ffn_gate_up_exps.offset + (size_t)e * c->src_gu_bytes;
    const uint8_t *dn = s->gguf.map + L->ffn_down_exps.offset + (size_t)e * c->src_dn_bytes;
    int rc_gu;
    if (L->ffn_gate_up_exps.type == INK_GGML_Q8_0 && c->gu_type == INK_GGML_IQ4_NL) {
        rc_gu = relayout_q8_to_iq4(s, gu, c->gu + (uint64_t)slot * c->gu_bytes, 2 * hp->n_ff_exp, hp->n_embd / 32, hp->n_ff_exp);
    } else if (L->ffn_gate_up_exps.type == c->gu_type) {
        rc_gu = relayout(s, gu, c->gu + (uint64_t)slot * c->gu_bytes, 2 * hp->n_ff_exp, hp->n_embd / 32, quant_words(c->gu_type), hp->n_ff_exp);
    } else {
        uint8_t *rep = ggml_repack_to_iq4_nl(L->ffn_gate_up_exps.type, gu, 2 * hp->n_ff_exp, hp->n_embd);
        if (!rep) { snprintf(s->error, sizeof s->error, "out of memory repacking expert gate_up"); return 1; }
        rc_gu = relayout(s, rep, c->gu + (uint64_t)slot * c->gu_bytes, 2 * hp->n_ff_exp, hp->n_embd / 32, 4, hp->n_ff_exp);
        free(rep);
    }
    if (rc_gu) return rc_gu;
    if (L->ffn_down_exps.type == INK_GGML_Q8_0 && c->dn_type == INK_GGML_IQ4_NL) {
        return relayout_q8_to_iq4(s, dn, c->dn + (uint64_t)slot * c->dn_bytes, hp->n_embd, hp->n_ff_exp / 32, 0);
    } else if (L->ffn_down_exps.type == c->dn_type) {
        return relayout(s, dn, c->dn + (uint64_t)slot * c->dn_bytes, hp->n_embd, hp->n_ff_exp / 32, quant_words(c->dn_type), 0);
    } else {
        uint8_t *rep = ggml_repack_to_iq4_nl(L->ffn_down_exps.type, dn, hp->n_embd, hp->n_ff_exp);
        if (!rep) { snprintf(s->error, sizeof s->error, "out of memory repacking expert down"); return 1; }
        int rc_dn = relayout(s, rep, c->dn + (uint64_t)slot * c->dn_bytes, hp->n_embd, hp->n_ff_exp / 32, 4, 0);
        free(rep);
        return rc_dn;
    }
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
        /* Batched wide FP32 matvec (e.g. MoE router 128x2304): read W ONCE for all n <= 8 tokens (8x less DRAM!). */
        if (cols >= 1024 && n >= 2 && n <= 8) {
            CALL(s, K_DVR_F32_NB8, rows, a_int(rows), a_int(cols), a_int(n), a_int(add),
                 a_tensor_f32(s, w), a_f32v(x_buf, x_off, n * cols), a_f32v(out_buf, out_off, n * rows));
        } else {
            /* A wide matrix gets a group per output; a narrow one, a thread. */
            CALL(s, cols >= 1024 ? K_DVR_F32 : K_DVS_F32, n * rows, a_int(rows), a_int(cols), a_int(add),
                 a_tensor_f32(s, w), a_f32v(x_buf, x_off, n * cols), a_f32v(out_buf, out_off, n * rows));
        }
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
    if (np >= 2 && !s->no_mma && !s->oracle_q8 && !s->no_tables && d % 16 == 0 && hp->gate_rank == 64) {
        if (np <= 8 && s->ctrl_inv && (size_t)(4 * n * np * hp->gate_rank) <= s->mv_part_floats) {
            /* Split-K FP32 control gate: reads D[64, 2*d] ONCE across all np tokens (8x less DRAM, 100% FP32 exact!). */
            int64_t sk = 4;
            CALL(s, K_CTRL_RMS, n * np, a_int(d), a_int(np), a_int(stack), a_int(flags),
                 a_f32v(cur_buf, 0, cur_n), a_f32v(s->messages, 0, 4 * stack), a_f32v(s->ctrl_inv, 0, n * np * 2));
            CALL(s, K_CTRL_GATE_NB8_SK, n * hp->gate_rank * sk,
                 a_int(d), a_int(np), a_int(n), a_int(stack), a_int(flags), a_int(sk),
                 a_int(down[0]), a_int(down[1]), a_int(down[2]), a_arena(s),
                 a_f32v(cur_buf, 0, cur_n), a_f32v(s->messages, 0, 4 * stack),
                 a_f32v(s->ctrl_inv, 0, n * np * 2), a_f32v(s->mv_part, 0, sk * n * np * hp->gate_rank));
            CALL(s, K_SK_REDUCE, n * np * hp->gate_rank, a_int(n * np * hp->gate_rank), a_int(sk), a_int(0),
                 a_f32v(s->mv_part, 0, sk * n * np * hp->gate_rank), a_f32v(s->ctrl_gate, 0, n * np * hp->gate_rank));
        } else if (np <= 16) {
            CALL(s, K_CTRL_PREP, n * np * hp->gate_rank, a_int(d), a_int(np), a_int(stack), a_int(flags),
                 a_int(down[0]), a_int(down[1]), a_int(down[2]), a_arena(s),
                 a_f32v(cur_buf, 0, cur_n), a_f32v(s->messages, 0, 4 * stack), a_f32v(s->ctrl_gate, 0, n * np * hp->gate_rank));
        } else {
            int64_t x_words = n * np * d;
            CALL(s, K_CTRL_PACK_BF16, n * np, a_int(d), a_int(np), a_int(stack), a_int(flags),
                 a_f32v(cur_buf, 0, cur_n), a_f32v(s->messages, 0, 4 * stack), a_u32v(s->x_bf16, 0, x_words));
            CALL(s, K_CTRL_GATE_MMA, n * 4 * ((np + 15) / 16), a_int(d), a_int(np),
                 a_int(down[0]), a_int(down[1]), a_int(down[2]), a_arena(s),
                 a_u32v(s->x_bf16, 0, x_words), a_f32v(s->ctrl_gate, 0, n * np * hp->gate_rank));
        }
        int64_t g_pairs = n * np * (hp->gate_rank / 2);
        CALL(s, K_BF16_PACK, g_pairs, a_int(g_pairs),
             a_f32v(s->ctrl_gate, 0, n * np * hp->gate_rank), a_u32v(s->x_bf16, 0, g_pairs));
        CALL(s, K_CTRL_OUT_MMA, n * ((d + 15) / 16) * ((np + 15) / 16), a_int(d), a_int(np), a_int(stack), a_int(flags),
             a_int(gf[0]), a_int(gf[1]), a_int(gf[2]), a_arena(s),
             a_f32v(cur_buf, 0, cur_n), a_f32v(s->messages, 0, 4 * stack),
             a_u32v(s->x_bf16, 0, g_pairs), a_f32v(out_buf, 0, n * stack));
        return 0;
    }
    CALL(s, K_CTRL_PREP, n * np * hp->gate_rank, a_int(d), a_int(np), a_int(stack), a_int(flags),
         a_int(down[0]), a_int(down[1]), a_int(down[2]), a_arena(s),
         a_f32v(cur_buf, 0, cur_n), a_f32v(s->messages, 0, 4 * stack), a_f32v(s->ctrl_gate, 0, n * np * hp->gate_rank));
    CALL(s, K_CTRL_APPLY, n * np * ((d + 31) / 32), a_int(d), a_int(np), a_int(stack), a_int(flags),
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
    static float logits[INK_MAX_BATCH * INK_MAX_EXPERTS];
    int n_tok = n / (int)hp->n_expert_used;
    if (pc_download(ids, s->moe_ids, (size_t)n * 4, s->stream) != PC_OK ||
        pc_download(logits, s->router_logits, (size_t)n_tok * (size_t)hp->n_expert * 4, s->stream) != PC_OK ||
        pc_sync_stream(s->stream) != PC_OK) return host_fail(s, "read expert ids");
    /* How much the router has lately liked each expert, chosen or not: among
       experts that have waited equally long, the least liked one is the one
       to replace. An expert the router keeps scoring high is likely to be
       chosen soon even if it has not been yet. */
    for (int t = 0; t < n_tok; t++) {
        const float *row = logits + (size_t)t * (size_t)hp->n_expert;
        for (int e = 0; e < hp->n_expert; e++)
            c->liking[e] = c->liking_set ? c->liking[e] + INK_LIKING_RATE * (row[e] - c->liking[e]) : row[e];
        c->liking_set = 1;
    }
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
            for (int i = 1; i < c->cap; i++) {
                if (c->used[i] > c->used[slot]) continue;
                if (c->used[i] < c->used[slot] || c->expert_in[slot] < 0) { slot = i; continue; }
                if (c->expert_in[i] < 0 || c->liking[c->expert_in[i]] < c->liking[c->expert_in[slot]]) slot = i;
            }
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

static int dispatch_iq4_mma(InkSession *s, const InkTensor *w, int64_t np, int64_t rows, int64_t cols,
                            PcBuffer xb_buf, int64_t xb_off, PcBuffer out_buf, int64_t out_off) {
    int64_t blocks = cols / 32;
    int64_t lrows = (int64_t)(w->nbytes / (uint64_t)(blocks * 18));
    PcBuffer dev = tensor_dev(s, w);
    int64_t rtiles = (rows + 15) / 16;
    for (int64_t t0 = 0; t0 < np; t0 += 8) {
        int64_t tn = (np - t0 < 8) ? (np - t0) : 8;
        int64_t want_sk = (rows <= 2304) ? 8 : 4;
        int64_t sk = (rows <= 4096 && blocks >= 16 && (size_t)(want_sk * tn * rows) <= s->mv_part_floats) ? want_sk : 1;
        if (sk > 1) {
            CALL(s, K_IQ4_MMA_NB8_SK, rtiles * sk, a_int(rows), a_int(lrows), a_int(cols), a_int(tn), a_int(sk),
                 a_view(dev, 0, lrows * blocks * 4), a_view(dev, lrows * blocks * 16, lrows * blocks),
                 a_u32v(xb_buf, xb_off + t0 * (cols / 2), tn * (cols / 2)), a_f32v(s->mv_part, 0, sk * tn * rows));
            CALL(s, K_SK_REDUCE, tn * rows, a_int(tn * rows), a_int(sk), a_int(0),
                 a_f32v(s->mv_part, 0, sk * tn * rows), a_f32v(out_buf, out_off + t0 * rows, tn * rows));
        } else {
            CALL(s, K_IQ4_MMA_NB8_SK, rtiles, a_int(rows), a_int(lrows), a_int(cols), a_int(tn), a_int(1),
                 a_view(dev, 0, lrows * blocks * 4), a_view(dev, lrows * blocks * 16, lrows * blocks),
                 a_u32v(xb_buf, xb_off + t0 * (cols / 2), tn * (cols / 2)), a_f32v(out_buf, out_off + t0 * rows, tn * rows));
        }
    }
    return 0;
}

static int dispatch_q8_mma(InkSession *s, const InkTensor *w, int64_t np, int64_t rows, int64_t cols, int add,
                           PcBuffer xb_buf, int64_t xb_off, PcBuffer out_buf, int64_t out_off) {
    int64_t blocks = cols / 32;
    int64_t lrows = (int64_t)(w->nbytes / (uint64_t)(blocks * 34));
    PcBuffer dev = tensor_dev(s, w);
    int64_t rtiles = (rows + 15) / 16;
    if (np <= 8 && rows <= 4096 && blocks >= 16 && (size_t)(8 * np * rows) <= s->mv_part_floats) {
        int64_t sk = 8;
        CALL(s, K_Q8_MMA_NB8_SK, rtiles * sk, a_int(rows), a_int(lrows), a_int(cols), a_int(np), a_int(sk),
             a_view(dev, 0, lrows * blocks * 8), a_view(dev, lrows * blocks * 32, lrows * blocks),
             a_u32v(xb_buf, xb_off, np * (cols / 2)), a_f32v(s->mv_part, 0, sk * np * rows));
        CALL(s, K_SK_REDUCE, np * rows, a_int(np * rows), a_int(sk), a_int(add),
             a_f32v(s->mv_part, 0, sk * np * rows), a_f32v(out_buf, out_off, np * rows));
        return 0;
    }
    if (np <= 8 && add == 0) {
        CALL(s, K_Q8_MMA_NB8_SK, rtiles, a_int(rows), a_int(lrows), a_int(cols), a_int(np), a_int(1),
             a_view(dev, 0, lrows * blocks * 8), a_view(dev, lrows * blocks * 32, lrows * blocks),
             a_u32v(xb_buf, xb_off, np * (cols / 2)), a_f32v(out_buf, out_off, np * rows));
        return 0;
    }
    int64_t warps = rtiles * ((np + 15) / 16);
    CALL(s, K_Q8_MMA, warps, a_int(rows), a_int(lrows), a_int(cols), a_int(np), a_int(add),
         a_view(dev, 0, lrows * blocks * 8), a_view(dev, lrows * blocks * 32, lrows * blocks),
         a_u32v(xb_buf, xb_off, np * (cols / 2)), a_f32v(out_buf, out_off, np * rows));
    return 0;
}

static int run_moe_mma_group(InkSession *s, int ip, int64_t gu_q8, int64_t dn_q8,
                             const int *experts, int n_exp, const int *exp_head, const int *pair_next,
                             int64_t n_used) {
    const InkHParams *hp = &s->gguf.hp;
    const ExpertCache *c = &s->experts[ip];
    int64_t d = hp->n_embd, n_ff = hp->n_ff_exp;
    uint32_t h_slot[INK_MAX_BATCH * 16 + INK_MAX_EXPERTS];
    uint32_t h_base[INK_MAX_BATCH * 16 + INK_MAX_EXPERTS];
    uint32_t h_cnt[INK_MAX_BATCH * 16 + INK_MAX_EXPERTS];
    uint32_t h_tok[INK_MAX_BATCH * 16];
    uint32_t h_pair[INK_MAX_BATCH * 16];
    int n_tasks = 0, n_entries = 0;
    for (int i = 0; i < n_exp; i++) {
        int e = experts[i];
        uint32_t slot = (c->cap >= hp->n_expert) ? (uint32_t)e : (uint32_t)c->slot_of[e];
        int p = exp_head[e];
        while (p >= 0) {
            int base = n_entries, cnt = 0;
            while (p >= 0 && cnt < 16) {
                h_tok[n_entries] = (uint32_t)(p / (int)n_used);
                h_pair[n_entries] = (uint32_t)p;
                n_entries++;
                cnt++;
                p = pair_next[p];
            }
            h_slot[n_tasks] = slot;
            h_base[n_tasks] = (uint32_t)base;
            h_cnt[n_tasks] = (uint32_t)cnt;
            n_tasks++;
        }
    }
    if (n_tasks == 0) return 0;
    uint32_t *packed = s->moe_tasks_host;
    size_t off_slot = 0;
    size_t off_base = (size_t)n_tasks;
    size_t off_cnt  = (size_t)n_tasks * 2;
    size_t off_tok  = (size_t)n_tasks * 3;
    size_t off_pair = off_tok + (size_t)n_entries;
    size_t total_w  = off_pair + (size_t)n_entries;
    memcpy(packed + off_slot, h_slot, (size_t)n_tasks * 4);
    memcpy(packed + off_base, h_base, (size_t)n_tasks * 4);
    memcpy(packed + off_cnt,  h_cnt,  (size_t)n_tasks * 4);
    memcpy(packed + off_tok,  h_tok,  (size_t)n_entries * 4);
    memcpy(packed + off_pair, h_pair, (size_t)n_entries * 4);
    if (pc_upload(s->moe_tasks, packed, total_w * 4, s->stream) != PC_OK) return host_fail(s, "upload moe tasks");
    PcArg a_slot = a_u32v(s->moe_tasks, (int64_t)off_slot, n_tasks);
    PcArg a_base = a_u32v(s->moe_tasks, (int64_t)off_base, n_tasks);
    PcArg a_cnt  = a_u32v(s->moe_tasks, (int64_t)off_cnt,  n_tasks);
    PcArg a_tok  = a_u32v(s->moe_tasks, (int64_t)off_tok,  n_entries);
    PcArg a_pair = a_u32v(s->moe_tasks, (int64_t)off_pair, n_entries);
    int64_t gu_words = (int64_t)((size_t)c->cap * c->gu_bytes / 4);
    int64_t dn_words = (int64_t)((size_t)c->cap * c->dn_bytes / 4);
    CALL(s, gu_q8 ? K_MOE_GU_Q8_MMA : K_MOE_GU_IQ4_MMA, (int64_t)n_tasks * ((2 * n_ff + 15) / 16),
         a_int(n_ff), a_int(d),
         a_view(c->gu, 0, gu_words), a_view(c->gu, 0, gu_words * 2),
         a_u32v(s->x_bf16, 0, (int64_t)s->batch * (d / 2)),
         a_slot, a_base, a_cnt, a_tok,
         a_f32v(s->moe_gu_out, 0, (int64_t)n_entries * 2 * n_ff));
    CALL(s, K_MOE_SITU_PACK, (int64_t)n_entries * (n_ff / 2),
         a_int(n_entries), a_int(n_ff), a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
         a_f32v(s->moe_gu_out, 0, (int64_t)n_entries * 2 * n_ff),
         a_pair, a_f32v(s->moe_wts, 0, (int64_t)s->batch * n_used),
         a_u32v(s->moe_hid_bf16, 0, (int64_t)n_entries * (n_ff / 2)));
    CALL(s, dn_q8 ? K_MOE_DOWN_Q8_MMA : K_MOE_DOWN_IQ4_MMA, (int64_t)n_tasks * ((d + 15) / 16),
         a_int(d), a_int(n_ff),
         a_view(c->dn, 0, dn_words), a_view(c->dn, 0, dn_words * 2),
         a_u32v(s->moe_hid_bf16, 0, (int64_t)n_entries * (n_ff / 2)),
         a_slot, a_base, a_cnt, a_pair,
         a_f32v(s->moe_down_out, 0, (int64_t)s->batch * n_used * d));
    return 0;
}

/* Grouped-by-expert Tensor Core MoE for prefill (np >= 2). Handles both fully
   resident layers (zero uploads) and streaming layers (in-cache hits run first,
   then misses stream in chunks of at most c->cap experts without thrashing). */
static int dispatch_moe_mma(InkSession *s, int il, int ip, int64_t np, int64_t gu_q8, int64_t dn_q8) {
    const InkHParams *hp = &s->gguf.hp;
    const InkLayerTensors *L = &s->gguf.layers[ip];
    ExpertCache *c = &s->experts[ip];
    int64_t d = hp->n_embd, n_used = hp->n_expert_used;
    int n = (int)(np * n_used);
    int streaming = c->cap < hp->n_expert;

    /* Pack MoE input activations to BF16 while downloading router ids. */
    CALL(s, K_BF16_PACK, np * (d / 2), a_int(np * (d / 2)),
         a_f32v(s->norm_buf, 0, np * d), a_u32v(s->x_bf16, 0, np * (d / 2)));

    /* Stage C3A: Resident small-batch (np <= 8) zero-sync GPU-planned MoE (100% CUDA-Graph capturable!). */
    if (!streaming && np <= 8 && n_used <= 8 && hp->n_expert <= 128) {
        int64_t n_ff = hp->n_ff_exp;
        int64_t n_entries = (int64_t)n;
        int64_t off_slot = 0;
        int64_t off_base = n_entries;
        int64_t off_cnt  = n_entries * 2;
        int64_t off_tok  = n_entries * 3;
        int64_t off_pair = n_entries * 4;
        PcArg a_slot = a_u32v(s->moe_tasks, off_slot, n_entries);
        PcArg a_base = a_u32v(s->moe_tasks, off_base, n_entries);
        PcArg a_cnt  = a_u32v(s->moe_tasks, off_cnt,  n_entries);
        PcArg a_tok  = a_u32v(s->moe_tasks, off_tok,  n_entries);
        PcArg a_pair = a_u32v(s->moe_tasks, off_pair, n_entries);
        CALL(s, K_MOE_PLAN_NB, 1, a_int(hp->n_expert), a_int(n_used), a_int(np),
             a_u32v(s->moe_ids, 0, n_entries), a_slot, a_base, a_cnt, a_tok, a_pair);
        int64_t gu_words = (int64_t)((size_t)c->cap * c->gu_bytes / 4);
        int64_t dn_words = (int64_t)((size_t)c->cap * c->dn_bytes / 4);
        CALL(s, gu_q8 ? K_MOE_GU_Q8_MMA : K_MOE_GU_IQ4_NB8, n_entries * ((2 * n_ff + 15) / 16),
             a_int(n_ff), a_int(d),
             a_view(c->gu, 0, gu_words), a_view(c->gu, 0, gu_words * 2),
             a_u32v(s->x_bf16, 0, (int64_t)s->batch * (d / 2)),
             a_slot, a_base, a_cnt, a_tok,
             a_f32v(s->moe_gu_out, 0, n_entries * 2 * n_ff));
        CALL(s, K_MOE_SITU_PACK, n_entries * (n_ff / 2),
             a_int(n_entries), a_int(n_ff), a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_f32v(s->moe_gu_out, 0, n_entries * 2 * n_ff),
             a_pair, a_f32v(s->moe_wts, 0, (int64_t)s->batch * n_used),
             a_u32v(s->moe_hid_bf16, 0, n_entries * (n_ff / 2)));
        CALL(s, dn_q8 ? K_MOE_DOWN_Q8_MMA : K_MOE_DOWN_IQ4_NB8, n_entries * ((d + 15) / 16),
             a_int(d), a_int(n_ff),
             a_view(c->dn, 0, dn_words), a_view(c->dn, 0, dn_words * 2),
             a_u32v(s->moe_hid_bf16, 0, n_entries * (n_ff / 2)),
             a_slot, a_base, a_cnt, a_pair,
             a_f32v(s->moe_down_out, 0, (int64_t)s->batch * n_used * d));
        int ablate_ip = (s->has_ablate && ip < 32 && s->ablate_has_layer[ip]);
        if (ablate_ip) {
            CALL(s, K_MOE_SUM_DOWN, np * d, a_int(d), a_int(np), a_int(n_used),
                 a_f32v(s->moe_down_out, 0, np * n_used * d), a_f32v(s->o_raw, 0, np * d));
            CALL(s, K_ABLATE_DIR, np, a_int(d), a_int((int64_t)(1 + ip) * d),
                 a_f32v(s->ablate_dirs, 0, (1 + hp->n_phys) * d), a_f32v(s->o_raw, 0, np * d));
            if (s->spec_recording) {
                int64_t n_kv = hp->n_head_kv * hp->head_dim;
                int64_t slot_ch = 2 * n_kv + 2 * d;
                int64_t log_off = (int64_t)il * 8 * slot_ch + 16 * n_kv + 8 * d;
                CALL(s, K_ATTN_POST_LOG, d, a_int(d), a_int(np), a_int(0),
                     a_f32v(s->o_raw, 0, np * d), a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
                     a_f32v(s->spec_raw_log, log_off, 8 * d), a_f32v(s->cur, 0, np * d), a_f32v(s->dummy_f32, 0, 64));
            } else {
                CALL(s, K_MOE_POST, d, a_int(d), a_int(np), a_f32v(s->o_raw, 0, np * d),
                     a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
                     a_f32v(s->cur, 0, np * d), a_f32v(s->cur, 0, np * d));
            }
        } else if (s->spec_recording) {
            int64_t n_kv = hp->n_head_kv * hp->head_dim;
            int64_t slot_ch = 2 * n_kv + 2 * d;
            int64_t bak_off = ((int64_t)il * slot_ch + 2 * n_kv + d) * 3;
            int64_t log_off = (int64_t)il * 8 * slot_ch + 16 * n_kv + 8 * d;
            CALL(s, K_MOE_REDUCE_POST_LOG, d, a_int(d), a_int(np), a_int(n_used),
                 a_f32v(s->moe_down_out, 0, np * n_used * d),
                 a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
                 a_f32v(s->spec_conv_bak, bak_off, d * 3),
                 a_f32v(s->spec_raw_log, log_off, 8 * d),
                 a_f32v(s->cur, 0, np * d));
        } else {
            CALL(s, K_MOE_REDUCE_POST, d, a_int(d), a_int(np), a_int(n_used),
                 a_f32v(s->moe_down_out, 0, np * n_used * d),
                 a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
                 a_f32v(s->cur, 0, np * d));
        }
        return 0;
    }

    uint32_t ids[INK_MAX_BATCH * 16];
    static float logits[INK_MAX_BATCH * INK_MAX_EXPERTS];
    double began = s->profile ? pc_seconds() : 0.0;
    if (pc_download(ids, s->moe_ids, (size_t)n * 4, s->stream) != PC_OK) return host_fail(s, "read expert ids");
    if (streaming && pc_download(logits, s->router_logits, (size_t)np * (size_t)hp->n_expert * 4, s->stream) != PC_OK)
        return host_fail(s, "read router logits");
    if (pc_sync_stream(s->stream) != PC_OK) return host_fail(s, "sync expert ids");
    if (streaming) {
        for (int t = 0; t < (int)np; t++) {
            const float *row = logits + (size_t)t * (size_t)hp->n_expert;
            for (int e = 0; e < hp->n_expert; e++)
                c->liking[e] = c->liking_set ? c->liking[e] + INK_LIKING_RATE * (row[e] - c->liking[e]) : row[e];
            c->liking_set = 1;
        }
    }

    int exp_head[INK_MAX_EXPERTS], exp_cnt[INK_MAX_EXPERTS], pair_next[INK_MAX_BATCH * 16];
    for (int e = 0; e < hp->n_expert; e++) { exp_head[e] = -1; exp_cnt[e] = 0; }
    for (int p = n - 1; p >= 0; p--) {
        uint32_t e = ids[p];
        if (e >= (uint32_t)hp->n_expert) {
            snprintf(s->error, sizeof s->error, "router chose expert %u of %lld", e, (long long)hp->n_expert);
            return 1;
        }
        pair_next[p] = exp_head[e];
        exp_head[e] = p;
        exp_cnt[e]++;
    }

    int hits[INK_MAX_EXPERTS], misses[INK_MAX_EXPERTS];
    int n_hits = 0, n_misses = 0;
    s->tick++;
    uint64_t hit_tick = s->tick;
    for (int e = 0; e < hp->n_expert; e++) {
        if (exp_head[e] < 0) continue;
        if (!streaming || c->slot_of[e] >= 0) {
            hits[n_hits++] = e;
            if (streaming) { c->used[c->slot_of[e]] = hit_tick; s->expert_hits += (uint64_t)exp_cnt[e]; }
        } else {
            misses[n_misses++] = e;
        }
    }
    if (s->profile && streaming) s->prof_stream_seconds += pc_seconds() - began;

    /* Pass 1: every active expert already in VRAM. */
    int need_sync = 0;
    if (n_hits > 0) {
        if (run_moe_mma_group(s, ip, gu_q8, dn_q8, hits, n_hits, exp_head, pair_next, n_used) != 0) return 1;
        need_sync = 1;
    }

    /* Pass 2+: upload missed experts in chunks of at most c->cap and run them. */
    for (int at = 0; at < n_misses; ) {
        int chunk = n_misses - at;
        if (chunk > c->cap) chunk = c->cap;
        double mbegan = s->profile ? pc_seconds() : 0.0;
        uint64_t prev_tick = s->tick;
        s->tick++;
        for (int m = 0; m < chunk; m++) {
            int e = misses[at + m];
            int slot = 0;
            for (int i = 1; i < c->cap; i++) {
                if (c->used[i] > c->used[slot]) continue;
                if (c->used[i] < c->used[slot] || c->expert_in[slot] < 0) { slot = i; continue; }
                if (c->expert_in[i] < 0 || c->liking[c->expert_in[i]] < c->liking[c->expert_in[slot]]) slot = i;
            }
            if (need_sync && c->used[slot] >= prev_tick) {
                if (pc_sync_stream(s->stream) != PC_OK) return host_fail(s, "sync before evict");
                need_sync = 0;
            }
            if (c->expert_in[slot] >= 0) c->slot_of[c->expert_in[slot]] = -1;
            if (load_expert(s, ip, e, slot)) return 1;
            c->slot_of[e] = (int16_t)slot;
            c->expert_in[slot] = (int16_t)e;
            c->used[slot] = s->tick;
            s->expert_misses++;
            if (exp_cnt[e] > 1) s->expert_hits += (uint64_t)(exp_cnt[e] - 1);
            s->expert_upload_bytes += c->gu_bytes + c->dn_bytes;
        }
        if (s->profile) s->prof_stream_seconds += pc_seconds() - mbegan;
        if (run_moe_mma_group(s, ip, gu_q8, dn_q8, misses + at, chunk, exp_head, pair_next, n_used) != 0) return 1;
        need_sync = 1;
        at += chunk;
    }

    if (s->has_ablate && ip < 32 && s->ablate_has_layer[ip]) {
        CALL(s, K_MOE_SUM_DOWN, np * d, a_int(d), a_int(np), a_int(n_used),
             a_f32v(s->moe_down_out, 0, np * n_used * d), a_f32v(s->o_raw, 0, np * d));
        CALL(s, K_ABLATE_DIR, np, a_int(d), a_int((int64_t)(1 + ip) * d),
             a_f32v(s->ablate_dirs, 0, (1 + hp->n_phys) * d), a_f32v(s->o_raw, 0, np * d));
        CALL(s, K_MOE_POST, d, a_int(d), a_int(np), a_f32v(s->o_raw, 0, np * d),
             a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
             a_f32v(s->cur, 0, np * d), a_f32v(s->cur, 0, np * d));
    } else {
        CALL(s, K_MOE_REDUCE_POST, d, a_int(d), a_int(np), a_int(n_used),
             a_f32v(s->moe_down_out, 0, np * n_used * d),
             a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
             a_f32v(s->cur, 0, np * d));
    }
    return 0;
}

/* First half of one logical layer il (physical layer ip): attention through
   the MoE router. Everything here stays on the device; during CUDA Graph
   capture (`s->capturing`), position-dependent kernels read `s->dev_pos`. */
static int dispatch_layer_pre(InkSession *s, int il, int ip, int64_t np, int *n_blocks, int has_messages) {
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
    int64_t pos_arg = s->capturing ? -1 : (int64_t)s->pos;

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
    int use_mma = np >= 2 && !s->no_mma && !s->oracle_q8 && !s->no_tables && d % 32 == 0;
    int k_dyn = !is_swa && L->attn_k_conv_proj.present;
    int v_dyn = !is_swa && L->attn_v_conv_proj.present;
    int variant = same ? 0 : 1;
    int fused = !use_mma && s->proj_list[ip][variant].n_mats > 0 && !s->oracle_q8 && !s->no_tables;
    if (use_mma) {
        int64_t half_stack = stack / 2;
        CALL(s, K_BF16_PACK, np * (d / 2), a_int(np * (d / 2)),
             a_f32v(xin, xq, np * d), a_u32v(s->x_bf16, 0, np * (d / 2)));
        if (!same) {
            CALL(s, K_BF16_PACK, np * (d / 2), a_int(np * (d / 2)),
                 a_f32v(xin, xk, np * d), a_u32v(s->x_bf16, half_stack, np * (d / 2)));
            CALL(s, K_BF16_PACK, np * (d / 2), a_int(np * (d / 2)),
                 a_f32v(xin, xv, np * d), a_u32v(s->x_bf16, 2 * half_stack, np * (d / 2)));
        }
        if (s->proj_list[ip][variant].n_mats > 0 && (!s->proj_list[ip][variant].is_iq4 || np <= 8)) {
            PcArg desc = a_u32v(s->proj_desc, (int64_t)(ip * 2 + variant) * 80, 80);
            int64_t rtiles = s->proj_list[ip][variant].rtiles;
            int64_t rows_all = s->proj_list[ip][variant].rows;
            if (np <= 8 && (size_t)(4 * np * rows_all) <= s->mv_part_floats) {
                int64_t sk = 4;
                int kern = s->proj_list[ip][variant].is_iq4 ? K_PROJ_IQ4_MMA_NB8_SK : K_PROJ_MMA_NB8_SK;
                CALL(s, kern, rtiles * sk, a_int(s->proj_list[ip][variant].n_mats), a_int(rtiles),
                     a_int(rows_all), a_int(d), a_int(np), a_int(sk), desc,
                     a_view(s->weights, 0, (int64_t)(s->weights_bytes / 4)), a_view(s->weights, 0, (int64_t)(s->weights_bytes / 2)),
                     a_u32v(s->x_bf16, 0, (int64_t)s->x_bf16_words), a_f32v(s->mv_part, 0, sk * np * rows_all));
                CALL(s, K_PROJ_SK_REDUCE, np * rows_all, a_int(s->proj_list[ip][variant].n_mats), a_int(rows_all),
                     a_int(np), a_int(sk), desc, a_f32v(s->mv_part, 0, sk * np * rows_all),
                     a_f32v(s->proj, 0, (int64_t)s->proj_floats));
            } else {
                int64_t warps = rtiles * ((np + 31) / 32);
                CALL(s, K_PROJ_MMA, warps, a_int(s->proj_list[ip][variant].n_mats), a_int(rtiles), a_int(d), a_int(np), desc,
                     a_view(s->weights, 0, (int64_t)(s->weights_bytes / 4)), a_view(s->weights, 0, (int64_t)(s->weights_bytes / 2)),
                     a_u32v(s->x_bf16, 0, (int64_t)s->x_bf16_words), a_f32v(s->proj, 0, (int64_t)s->proj_floats));
            }
        } else {
            int64_t xbq = 0, xbk = same ? 0 : half_stack, xbv = same ? 0 : 2 * half_stack;
            if (L->attn_q.type == INK_GGML_Q8_0) { if (dispatch_q8_mma(s, &L->attn_q, np, n_q, d, 0, s->x_bf16, xbq, s->q_buf, 0)) return 1; }
            else if (L->attn_q.type == INK_GGML_IQ4_NL) { if (dispatch_iq4_mma(s, &L->attn_q, np, n_q, d, s->x_bf16, xbq, s->q_buf, 0)) return 1; }
            else if (dispatch_mv(s, &L->attn_q, np, n_q, d, 0, xin, xq, s->q_buf, 0)) return 1;
            if (L->attn_k.type == INK_GGML_Q8_0) { if (dispatch_q8_mma(s, &L->attn_k, np, n_kv, d, 0, s->x_bf16, xbk, s->k_raw, 0)) return 1; }
            else if (L->attn_k.type == INK_GGML_IQ4_NL) { if (dispatch_iq4_mma(s, &L->attn_k, np, n_kv, d, s->x_bf16, xbk, s->k_raw, 0)) return 1; }
            else if (dispatch_mv(s, &L->attn_k, np, n_kv, d, 0, xin, xk, s->k_raw, 0)) return 1;
            if (k_dyn) {
                if (L->attn_k_conv_proj.type == INK_GGML_Q8_0) { if (dispatch_q8_mma(s, &L->attn_k_conv_proj, np, n_dyn, d, 0, s->x_bf16, xbk, s->kdyn_buf, 0)) return 1; }
                else if (L->attn_k_conv_proj.type == INK_GGML_IQ4_NL) { if (dispatch_iq4_mma(s, &L->attn_k_conv_proj, np, n_dyn, d, s->x_bf16, xbk, s->kdyn_buf, 0)) return 1; }
                else if (dispatch_mv(s, &L->attn_k_conv_proj, np, n_dyn, d, 0, xin, xk, s->kdyn_buf, 0)) return 1;
            }
            if (L->attn_v.type == INK_GGML_Q8_0) { if (dispatch_q8_mma(s, &L->attn_v, np, n_kv, d, 0, s->x_bf16, xbv, s->v_raw, 0)) return 1; }
            else if (L->attn_v.type == INK_GGML_IQ4_NL) { if (dispatch_iq4_mma(s, &L->attn_v, np, n_kv, d, s->x_bf16, xbv, s->v_raw, 0)) return 1; }
            else if (dispatch_mv(s, &L->attn_v, np, n_kv, d, 0, xin, xv, s->v_raw, 0)) return 1;
            if (v_dyn) {
                if (L->attn_v_conv_proj.type == INK_GGML_Q8_0) { if (dispatch_q8_mma(s, &L->attn_v_conv_proj, np, n_dyn, d, 0, s->x_bf16, xbv, s->vdyn_buf, 0)) return 1; }
                else if (L->attn_v_conv_proj.type == INK_GGML_IQ4_NL) { if (dispatch_iq4_mma(s, &L->attn_v_conv_proj, np, n_dyn, d, s->x_bf16, xbv, s->vdyn_buf, 0)) return 1; }
                else if (dispatch_mv(s, &L->attn_v_conv_proj, np, n_dyn, d, 0, xin, xv, s->vdyn_buf, 0)) return 1;
            }
            if (L->attn_rel.type == INK_GGML_Q8_0) { if (dispatch_q8_mma(s, &L->attn_rel, np, n_head * n_rel, d, 0, s->x_bf16, xbq, s->r_buf, 0)) return 1; }
            else if (L->attn_rel.type == INK_GGML_IQ4_NL) { if (dispatch_iq4_mma(s, &L->attn_rel, np, n_head * n_rel, d, s->x_bf16, xbq, s->r_buf, 0)) return 1; }
            else if (dispatch_mv(s, &L->attn_rel, np, n_head * n_rel, d, 0, xin, xq, s->r_buf, 0)) return 1;
            if (L->attn_sink_proj.type == INK_GGML_Q8_0) { if (dispatch_q8_mma(s, &L->attn_sink_proj, np, n_head, d, 0, s->x_bf16, xbq, s->sinks_buf, 0)) return 1; }
            else if (L->attn_sink_proj.type == INK_GGML_IQ4_NL) { if (dispatch_iq4_mma(s, &L->attn_sink_proj, np, n_head, d, s->x_bf16, xbq, s->sinks_buf, 0)) return 1; }
            else if (dispatch_mv(s, &L->attn_sink_proj, np, n_head, d, 0, xin, xq, s->sinks_buf, 0)) return 1;
            if (L->attn_gate.type == INK_GGML_Q8_0) { if (dispatch_q8_mma(s, &L->attn_gate, np, n_q, d, 0, s->x_bf16, xbq, s->gate_buf, 0)) return 1; }
            else if (L->attn_gate.type == INK_GGML_IQ4_NL) { if (dispatch_iq4_mma(s, &L->attn_gate, np, n_q, d, s->x_bf16, xbq, s->gate_buf, 0)) return 1; }
            else if (dispatch_mv(s, &L->attn_gate, np, n_q, d, 0, xin, xq, s->gate_buf, 0)) return 1;
        }
    } else {
        for (int64_t tok = 0; fused && tok < np; tok++) {
            PcArg desc = a_u32v(s->proj_desc, (int64_t)(ip * 2 + variant) * 80, 80);
            int pkern = s->proj_list[ip][variant].is_iq4 ? K_PROJ_IQ4_P : K_PROJ_P;
            CALL(s, pkern, s->proj_list[ip][variant].items, a_int(s->proj_list[ip][variant].n_mats), a_int(tok), a_int(d), desc,
                 a_view(s->weights, 0, (int64_t)(s->weights_bytes / 4)), a_view(s->weights, 0, (int64_t)(s->weights_bytes / 2)),
                 a_f32v(xin, 0, 3 * stack), a_f32v(s->mv_part, 0, (int64_t)s->mv_part_floats));
            CALL(s, K_PROJ_S, s->proj_list[ip][variant].rows, a_int(s->proj_list[ip][variant].n_mats), a_int(tok), a_int(d / 32), desc,
                 a_f32v(s->mv_part, 0, (int64_t)s->mv_part_floats), a_f32v(s->proj, 0, (int64_t)s->proj_floats));
        }
        if (!fused && dispatch_mv(s, &L->attn_q, np, n_q, d, 0, xin, xq, s->q_buf, 0)) return 1;
        if (!fused && dispatch_mv(s, &L->attn_k, np, n_kv, d, 0, xin, xk, s->k_raw, 0)) return 1;
        if (!fused && k_dyn && dispatch_mv(s, &L->attn_k_conv_proj, np, n_dyn, d, 0, xin, xk, s->kdyn_buf, 0)) return 1;
        if (!fused && dispatch_mv(s, &L->attn_v, np, n_kv, d, 0, xin, xv, s->v_raw, 0)) return 1;
        if (!fused && v_dyn && dispatch_mv(s, &L->attn_v_conv_proj, np, n_dyn, d, 0, xin, xv, s->vdyn_buf, 0)) return 1;
        if (!fused && dispatch_mv(s, &L->attn_rel, np, n_head * n_rel, d, 0, xin, xq, s->r_buf, 0)) return 1;
    }

    /* K and V: the causal short convolutions, K's clip and V's RMSNorm, then
       into the KV cache of slot il. One token takes one launch. The
       attention kernel clips Q and R itself. */
    int64_t kv_cap = s->kv_cap[il];
    int64_t kv_u16 = kv_cap * (s->kv_fp8 ? (n_kv / 2) : n_kv);
    if (np == 1 && hd == 128 && k_dyn == v_dyn) {
        CALL(s, K_KV1, n_head_kv, a_int(n_kv), a_int(k_dyn | (s->kv_fp8 ? 2 : 0)), a_int(pos_arg), a_int(kv_cap), a_f32(hp->qk_clip), a_f32(hp->rms_eps),
             a_f32v(s->k_raw, 0, n_kv), a_f32v(s->v_raw, 0, n_kv),
             a_tensor_f32(s, &L->attn_k_conv), a_tensor_f32(s, &L->attn_v_conv),
             k_dyn ? a_f32v(s->kdyn_buf, 0, n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             v_dyn ? a_f32v(s->vdyn_buf, 0, n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->conv_v[il], 0, n_kv * 3),
             a_u16v(s->kv_k[il], 0, kv_u16), a_u16v(s->kv_v[il], 0, kv_u16),
             a_u32v(s->dev_pos, 0, 1));
    } else if (np >= 2 && np <= 8 && hd == 128 && k_dyn == v_dyn) {
        int64_t kv_flags = k_dyn | (s->kv_fp8 ? 2 : 0) | (s->spec_recording ? 4 : 0);
        int64_t slot_ch = 2 * n_kv + 2 * d;
        PcArg log_arg = s->spec_recording
            ? a_f32v(s->spec_raw_log, (int64_t)il * 8 * slot_ch, 16 * n_kv)
            : a_f32v(s->dummy_f32, 0, 64);
        CALL(s, K_KV_NB, n_head_kv, a_int(n_kv), a_int(np), a_int(kv_flags), a_int(pos_arg), a_int(kv_cap),
             a_f32(hp->qk_clip), a_f32(hp->rms_eps),
             a_f32v(s->k_raw, 0, np * n_kv), a_f32v(s->v_raw, 0, np * n_kv),
             a_tensor_f32(s, &L->attn_k_conv), a_tensor_f32(s, &L->attn_v_conv),
             k_dyn ? a_f32v(s->kdyn_buf, 0, np * n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             v_dyn ? a_f32v(s->vdyn_buf, 0, np * n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->conv_v[il], 0, n_kv * 3),
             log_arg,
             a_u16v(s->kv_k[il], 0, kv_u16), a_u16v(s->kv_v[il], 0, kv_u16),
             a_u32v(s->dev_pos, 0, 1));
    } else {
        if (s->spec_recording) {
            int64_t slot_ch = 2 * n_kv + 2 * d;
            int64_t rslot = (int64_t)il * 8 * slot_ch;
            CALL(s, K_COPY, np * n_kv, a_int(np * n_kv), a_f32v(s->k_raw, 0, np * n_kv),
                 a_f32v(s->spec_raw_log, rslot, np * n_kv));
            CALL(s, K_COPY, np * n_kv, a_int(np * n_kv), a_f32v(s->v_raw, 0, np * n_kv),
                 a_f32v(s->spec_raw_log, rslot + 8 * n_kv, np * n_kv));
        }
        CALL(s, K_CONVOLVE, n_kv, a_int(n_kv), a_int(n_head_kv), a_int(k_dyn), a_int(np),
             a_f32v(s->k_raw, 0, np * n_kv), a_tensor_f32(s, &L->attn_k_conv),
             k_dyn ? a_f32v(s->kdyn_buf, 0, np * n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_k[il], 0, n_kv * 3), a_f32v(s->k_buf, 0, np * n_kv));
        CALL(s, K_CONVOLVE, n_kv, a_int(n_kv), a_int(n_head_kv), a_int(v_dyn), a_int(np),
             a_f32v(s->v_raw, 0, np * n_kv), a_tensor_f32(s, &L->attn_v_conv),
             v_dyn ? a_f32v(s->vdyn_buf, 0, np * n_dyn) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->conv_v[il], 0, n_kv * 3), a_f32v(s->v_buf, 0, np * n_kv));
        CALL(s, K_CLIP, np * n_head_kv, a_int(hd), a_int(n_head_kv), a_int(0),
             a_f32(hp->qk_clip), a_f32(0.0f), a_f32(1.0f), a_int(pos_arg),
             a_f32v(s->k_buf, 0, np * n_kv), a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->dev_pos, 0, 1));
        CALL(s, K_HEAD_RMSNORM, np * n_head_kv, a_int(hd), a_f32(hp->rms_eps),
             a_f32v(s->v_buf, 0, np * n_kv));
        if (s->kv_fp8) {
            CALL(s, K_KV_STORE_FP8, np * (n_kv / 2), a_int(n_kv), a_int(pos_arg), a_int(kv_cap),
                 a_f32v(s->k_buf, 0, np * n_kv), a_f32v(s->v_buf, 0, np * n_kv),
                 a_u16v(s->kv_k[il], 0, kv_u16), a_u16v(s->kv_v[il], 0, kv_u16),
                 a_u32v(s->dev_pos, 0, 1));
        } else {
            CALL(s, K_KV_STORE, np * n_kv, a_int(n_kv), a_int(pos_arg), a_int(kv_cap),
                 a_f32v(s->k_buf, 0, np * n_kv), a_f32v(s->v_buf, 0, np * n_kv),
                 a_u16v(s->kv_k[il], 0, kv_u16), a_u16v(s->kv_v[il], 0, kv_u16),
                 a_u32v(s->dev_pos, 0, 1));
        }
    }
    /* Full-attention Q and R are scaled by tau = 1 + alpha * log(max(1,
       (position + 1) / floor)), which the attention kernel computes for
       each token's own position. */
    float tau_alpha = is_swa ? 0.0f : hp->log_scale_alpha;
    float tau_floor = (float)hp->log_scale_floor;

    /* Sink logits and the attention output gate (already computed above when use_mma). */
    if (!fused && !use_mma && dispatch_mv(s, &L->attn_sink_proj, np, n_head, d, 0, xin, xq, s->sinks_buf, 0)) return 1;
    if (!fused && !use_mma && dispatch_mv(s, &L->attn_gate, np, n_q, d, 0, xin, xq, s->gate_buf, 0)) return 1;

    /* Attention:
       - For np == 1 with FP16 KV: K_ATTN (single launch, ~10.85 ms B=1).
       - For np == 2..8 (and np == 1 with FP8 KV): K_ATTN_ONLINE_SK (zero attn_scores DRAM,
         saturates all SMs across context length, 2.4 ms faster than K_ATTN at nb=8, FP8 KV ready!).
       - For prefill (np > 8): K_ATTN_ONLINE (zero attn_scores DRAM, full SM saturation). */
    int64_t dims0 = n_head | (n_head_kv << 16) | (hd << 32) | (n_rel << 48);
    int64_t dims1 = hp->rel_extent | ((int64_t)is_swa << 24) | (hp->swa << 32) | ((int64_t)s->kv_fp8 << 56);
    if (s->use_online_attn && ((np >= 2 && np <= 8) || (np == 1 && s->kv_fp8)) && s->attn_sk_o) {
        int64_t gqa = n_head / n_head_kv;
        int64_t q_in_g = gqa < 8 ? gqa : 8;
        int64_t t_step = 8 / q_in_g;
        int64_t g_chunks = (gqa + q_in_g - 1) / q_in_g;
        int64_t oa_items = ((np + t_step - 1) / t_step) * n_head_kv * g_chunks;
        int64_t sk = 16;
        CALL(s, K_ATTN_ONLINE_SK, oa_items * sk,
             a_int(dims0), a_int(dims1), a_int(kv_cap), a_int(pos_arg), a_int(np | (sk << 16)),
             a_f32v(s->q_buf, 0, np * n_q),
             a_f32v(s->r_buf, 0, np * n_head * n_rel),
             a_tensor_f32(s, &L->attn_rel_bias),
             a_f32v(s->sinks_buf, 0, np * n_head),
             a_u16v(s->kv_k[il], 0, kv_u16),
             a_u16v(s->kv_v[il], 0, kv_u16),
             a_f32v(s->attn_sk_o, 0, np * n_head * sk * hd),
             a_f32v(s->attn_sk_ms, 0, np * n_head * sk * 2),
             a_int((L->attn_q_clip.present ? 1 : 0) | (L->attn_rel_clip.present ? 2 : 0)),
             a_f32(hp->qk_clip), a_f32(hp->rel_clip), a_f32(tau_alpha), a_f32(tau_floor),
             L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64),
             L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->dev_pos, 0, 1));
        CALL(s, K_ATTN_SK_REDUCE, np * n_head,
             a_int(sk),
             a_f32v(s->attn_sk_o, 0, np * n_head * sk * hd),
             a_f32v(s->attn_sk_ms, 0, np * n_head * sk * 2),
             a_f32v(s->gate_buf, 0, np * n_q),
             a_f32v(s->attn_out, 0, np * n_q));
    } else if (np >= 9 && s->use_online_attn) {
        int64_t gqa = n_head / n_head_kv;
        int64_t q_in_g = gqa < 8 ? gqa : 8;
        int64_t t_step = 8 / q_in_g;
        int64_t g_chunks = (gqa + q_in_g - 1) / q_in_g;
        int64_t oa_items = ((np + t_step - 1) / t_step) * n_head_kv * g_chunks;
        CALL(s, K_ATTN_ONLINE, oa_items,
             a_int(dims0), a_int(dims1), a_int(kv_cap), a_int(pos_arg), a_int(np),
             a_f32v(s->q_buf, 0, np * n_q),
             a_f32v(s->r_buf, 0, np * n_head * n_rel),
             a_tensor_f32(s, &L->attn_rel_bias),
             a_f32v(s->sinks_buf, 0, np * n_head),
             a_f32v(s->gate_buf, 0, np * n_q),
             a_u16v(s->kv_k[il], 0, kv_u16),
             a_u16v(s->kv_v[il], 0, kv_u16),
             a_f32v(s->attn_out, 0, np * n_q),
             a_int((L->attn_q_clip.present ? 1 : 0) | (L->attn_rel_clip.present ? 2 : 0)),
             a_f32(hp->qk_clip), a_f32(hp->rel_clip), a_f32(tau_alpha), a_f32(tau_floor),
             L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64),
             L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->dev_pos, 0, 1));
    } else {
        int64_t kv_info = kv_cap | ((int64_t)s->max_ctx << 32);
        CALL(s, K_ATTN, np * n_head,
             a_int(dims0), a_int(dims1), a_int(kv_info), a_int(pos_arg),
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
             L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->dev_pos, 0, 1));
    }

    /* The output projection, the post shortconv and the residual add; a
       block's first layer starts a new residual stream and keeps the old one
       as the block's entry. One token's Q8_0 projection leaves its shares
       for the kernel that also does the rest. */
    int block_start = ip % hp->block_size == 0;
    if (block_start && *n_blocks >= INK_MAX_BLOCKS) {
        snprintf(s->error, sizeof s->error, "block count overflow");
        return 1;
    }
    PcArg blk_arg = block_start ? a_f32v(s->blocks, (int64_t)(*n_blocks) * stack, nd) : a_f32v(s->dummy_f32, 0, 64);
    const InkTensor *wo = &L->attn_output;
    int ablate_ip = (s->has_ablate && ip < 32 && s->ablate_has_layer[ip]);
    if (!ablate_ip && np == 1 && wo->type == INK_GGML_Q8_0 && !s->no_tables && !s->oracle_q8 && d % 4 == 0 && n_q % 32 == 0 &&
        (size_t)(d * (n_q / 32)) <= s->mv_part_floats) {
        int64_t blocks = n_q / 32, lrows = (int64_t)(wo->nbytes / (uint64_t)(blocks * 34));
        PcBuffer dev = tensor_dev(s, wo);
        CALL(s, K_MVQ_P, blocks * (d / 4), a_int(d), a_int(lrows), a_int(n_q),
             a_view(dev, 0, lrows * blocks * 8), a_view(dev, lrows * blocks * 32, lrows * blocks),
             a_f32v(s->attn_out, 0, n_q), a_f32v(s->mv_part, 0, d * blocks));
        CALL(s, K_SUM_POST, (d + 31) / 32, a_int(d), a_int(blocks), a_int(block_start),
             a_f32v(s->mv_part, 0, d * blocks), a_tensor_f32(s, &L->attn_post_conv), a_f32v(s->conv_ao[il], 0, d * 3),
             a_f32v(s->cur, 0, nd), blk_arg);
    } else if (use_mma && (wo->type == INK_GGML_Q8_0 || wo->type == INK_GGML_IQ4_NL) && n_q % 32 == 0) {
        CALL(s, K_BF16_PACK, np * (n_q / 2), a_int(np * (n_q / 2)),
             a_f32v(s->attn_out, 0, np * n_q), a_u32v(s->x_bf16, 0, np * (n_q / 2)));
        if (wo->type == INK_GGML_Q8_0) {
            if (dispatch_q8_mma(s, wo, np, d, n_q, 0, s->x_bf16, 0, s->o_raw, 0)) return 1;
        } else {
            if (dispatch_iq4_mma(s, wo, np, d, n_q, s->x_bf16, 0, s->o_raw, 0)) return 1;
        }
        if (ablate_ip) {
            CALL(s, K_ABLATE_DIR, np, a_int(d), a_int((int64_t)(1 + ip) * d),
                 a_f32v(s->ablate_dirs, 0, (1 + hp->n_phys) * d), a_f32v(s->o_raw, 0, nd));
        }
        if (s->spec_recording) {
            int64_t slot_ch = 2 * n_kv + 2 * d;
            CALL(s, K_ATTN_POST_LOG, d, a_int(d), a_int(np), a_int(block_start),
                 a_f32v(s->o_raw, 0, nd), a_tensor_f32(s, &L->attn_post_conv), a_f32v(s->conv_ao[il], 0, d * 3),
                 a_f32v(s->spec_raw_log, (int64_t)il * 8 * slot_ch + 16 * n_kv, nd),
                 a_f32v(s->cur, 0, nd), blk_arg);
        } else {
            CALL(s, K_ATTN_POST, d, a_int(d), a_int(np), a_int(block_start),
                 a_f32v(s->o_raw, 0, nd), a_tensor_f32(s, &L->attn_post_conv), a_f32v(s->conv_ao[il], 0, d * 3),
                 a_f32v(s->cur, 0, nd), blk_arg);
        }
    } else {
        if (dispatch_mv(s, wo, np, d, n_q, 0, s->attn_out, 0, s->o_raw, 0)) return 1;
        if (ablate_ip) {
            CALL(s, K_ABLATE_DIR, np, a_int(d), a_int((int64_t)(1 + ip) * d),
                 a_f32v(s->ablate_dirs, 0, (1 + hp->n_phys) * d), a_f32v(s->o_raw, 0, nd));
        }
        if (s->spec_recording) {
            int64_t slot_ch = 2 * n_kv + 2 * d;
            CALL(s, K_ATTN_POST_LOG, d, a_int(d), a_int(np), a_int(block_start),
                 a_f32v(s->o_raw, 0, nd), a_tensor_f32(s, &L->attn_post_conv), a_f32v(s->conv_ao[il], 0, d * 3),
                 a_f32v(s->spec_raw_log, (int64_t)il * 8 * slot_ch + 16 * n_kv, nd),
                 a_f32v(s->cur, 0, nd), blk_arg);
        } else {
            CALL(s, K_ATTN_POST, d, a_int(d), a_int(np), a_int(block_start),
                 a_f32v(s->o_raw, 0, nd), a_tensor_f32(s, &L->attn_post_conv), a_f32v(s->conv_ao[il], 0, d * 3),
                 a_f32v(s->cur, 0, nd), blk_arg);
        }
    }
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

    /* MoE router. */
    int64_t n_used = hp->n_expert_used;
    if (dispatch_mv(s, &L->ffn_gate_inp, np, hp->n_expert, d, 0, s->norm_buf, 0, s->router_logits, 0)) return 1;
    CALL(s, K_ROUTER, np, a_int(hp->n_expert), a_int(n_used),
         a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
         a_f32v(s->router_logits, 0, np * hp->n_expert),
         L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
         a_u32v(s->moe_ids, 0, np * n_used),
         a_f32v(s->moe_wts, 0, np * n_used));
    return 0;
}

/* Second half of one logical layer il (physical layer ip): MoE gate_up, down,
   weighted sum, post shortconv and residual add, given expert slot ids on device. */
static int dispatch_layer_moe(InkSession *s, int il, int ip, int64_t np, PcBuffer ids) {
    const InkHParams *hp = &s->gguf.hp;
    const InkLayerTensors *L = &s->gguf.layers[ip];
    const ExpertCache *c = &s->experts[ip];
    int64_t d = hp->n_embd;
    int64_t nd = np * d;
    int64_t n_used = hp->n_expert_used;
    int64_t n_ff = hp->n_ff_exp;
    uint32_t gu_type = c->gu_type, dn_type = c->dn_type;
    if ((gu_type != INK_GGML_IQ4_NL && gu_type != INK_GGML_Q8_0) || (dn_type != INK_GGML_IQ4_NL && dn_type != INK_GGML_Q8_0)) {
        snprintf(s->error, sizeof s->error, "unsupported expert types %u and %u", gu_type, dn_type);
        return 1;
    }
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
            if (np == 1 && !(s->has_ablate && ip < 32 && s->ablate_has_layer[ip])) {
                /* One token: the sum, the post shortconv and the residual add together. */
                CALL(s, K_SUM_POST, (d + 31) / 32, a_int(d), a_int(n_parts), a_int(0),
                     a_f32v(s->dn_part, 0, n_parts * d), a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
                     a_f32v(s->cur, 0, nd), a_f32v(s->dummy_f32, 0, 64));
                return 0;
            }
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
    if (s->has_ablate && ip < 32 && s->ablate_has_layer[ip]) {
        CALL(s, K_ABLATE_DIR, np, a_int(d), a_int((int64_t)(1 + ip) * d),
             a_f32v(s->ablate_dirs, 0, (1 + hp->n_phys) * d), a_f32v(s->moe_out, 0, nd));
    }
    CALL(s, K_MOE_POST, d, a_int(d), a_int(np), a_f32v(s->moe_out, 0, nd),
         a_tensor_f32(s, &L->ffn_post_conv), a_f32v(s->conv_fo[il], 0, d * 3),
         a_f32v(s->cur, 0, nd), a_f32v(s->cur, 0, nd));
    return 0;
}

/* One logical layer il (physical layer ip) for the np tokens of the batch,
   which sit at positions pos .. pos + np - 1. */
static int dispatch_layer(InkSession *s, int il, int ip, int64_t np, int *n_blocks, int has_messages) {
    if (dispatch_layer_pre(s, il, ip, np, n_blocks, has_messages) != 0) return 1;
    const InkHParams *hp = &s->gguf.hp;
    const ExpertCache *c = &s->experts[ip];
    uint32_t gu_type = c->gu_type, dn_type = c->dn_type;
    if ((gu_type != INK_GGML_IQ4_NL && gu_type != INK_GGML_Q8_0) || (dn_type != INK_GGML_IQ4_NL && dn_type != INK_GGML_Q8_0)) {
        snprintf(s->error, sizeof s->error, "unsupported expert types %u and %u", gu_type, dn_type);
        return 1;
    }
    int use_mma = np >= 2 && !s->no_mma && !s->oracle_q8 && !s->no_tables && hp->n_embd % 32 == 0;
    if (use_mma && hp->n_ff_exp % 32 == 0) {
        return dispatch_moe_mma(s, il, ip, np, gu_type == INK_GGML_Q8_0, dn_type == INK_GGML_Q8_0);
    }
    PcBuffer ids = s->moe_ids;
    if (!s->capturing && stream_experts(s, ip, (int)(np * hp->n_expert_used), &ids) != 0) return 1;
    return dispatch_layer_moe(s, il, ip, np, ids);
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

static int is_fast_q4_attn_tensor(const InkLayerTensors *L, const InkTensor *t) {
    return t == &L->attn_q || t == &L->attn_k || t == &L->attn_k_conv_proj ||
           t == &L->attn_v || t == &L->attn_v_conv_proj || t == &L->attn_rel ||
           t == &L->attn_sink_proj || t == &L->attn_gate || t == &L->attn_output;
}

static int is_loop_ctrl_tensor(const InkLayerTensors *L, const InkTensor *t) {
    for (int c = 0; c < 4; c++) {
        if (t == &L->loop_ctrl_down[c] || t == &L->loop_ctrl_gain[c] || t == &L->loop_ctrl_forget[c]) return 1;
    }
    return 0;
}

static uint64_t cuda_dev_tensor_bytes(const InkTensor *t, int force_f32) {
    if (force_f32 || t->ndims == 1) {
        uint64_t ne = 1;
        for (uint32_t d = 0; d < t->ndims; d++) ne *= t->dims[d];
        return ne * 4;
    }
    if (t->type == INK_GGML_F32 || t->type == INK_GGML_Q8_0 || t->type == INK_GGML_IQ4_NL) return t->nbytes;
    if (t->ndims == 2 && (t->dims[0] % 32) == 0) return t->dims[1] * (t->dims[0] / 32) * 34;
    uint64_t ne = 1;
    for (uint32_t d = 0; d < t->ndims; d++) ne *= t->dims[d];
    return ne * 4;
}

static int env_on(const char *name) {
    const char *v = getenv(name);
    return v != NULL && v[0] != '\0' && v[0] != '0';
}

static int64_t env_mb(const char *name) {
    const char *v = getenv(name);
    if (!v || !*v) return -1;
    long long n = atoll(v);
    return n > 0 ? (int64_t)n : -1;
}

static int64_t to_mb(size_t bytes) { return (int64_t)((bytes + 1048575u) / 1048576u); }

/* ---- Stage C3B: Native Coax CUDA DSpark (mtp.safetensors) ---- */
static float bf16_to_f32_cuda_host(uint16_t b) {
    uint32_t u = (uint32_t)b << 16;
    float f;
    memcpy(&f, &u, 4);
    return f;
}

static int mtp_seek(FILE *f, int64_t off) {
#ifdef _WIN32
    return _fseeki64(f, off, SEEK_SET);
#else
    return fseeko(f, (off_t)off, SEEK_SET);
#endif
}

static int mtp_find_span_cuda(const char *hdr, int64_t data_start, size_t file_size,
                              const char *name, int64_t expected_bytes, MtpSpan *out) {
    out->off = -1;
    out->bytes = 0;
    char needle[128];
    snprintf(needle, sizeof needle, "\"%s\"", name);
    const char *p = strstr(hdr, needle);
    if (!p) return 1;
    const char *obj = strchr(p + strlen(needle), '{');
    if (!obj) return 1;
    const char *end = strchr(obj, '}');
    if (!end) return 1;
    const char *doff = strstr(obj, "\"data_offsets\"");
    if (!doff || doff > end) return 1;
    const char *lb = strchr(doff, '[');
    if (!lb || lb > end) return 1;
    char *ep = NULL;
    int64_t s0 = strtoll(lb + 1, &ep, 10);
    if (!ep || ep > end) return 1;
    const char *comma = strchr(ep, ',');
    if (!comma || comma > end) return 1;
    int64_t s1 = strtoll(comma + 1, &ep, 10);
    if (!ep || ep > end) return 1;
    if (s0 < 0 || s1 <= s0 || (s1 - s0) != expected_bytes) return 1;
    int64_t off = data_start + s0;
    if ((off & 3) != 0 || (size_t)(data_start + s1) > file_size) return 1;
    out->off = off;
    out->bytes = s1 - s0;
    return 0;
}

static int dspark_iq4_mm(InkSession *s, PcBuffer wq, PcBuffer wd, int64_t rows, int64_t cols, int64_t np,
                         PcBuffer xb_buf, int64_t xb_word_off, PcBuffer out_buf, int64_t out_elem_off) {
    int64_t blocks = cols / 32;
    int64_t rtiles = (rows + 15) / 16;
    for (int64_t t0 = 0; t0 < np; t0 += 8) {
        int64_t tn = (np - t0 < 8) ? (np - t0) : 8;
        int64_t sk = (rows <= 2560 && blocks >= 32 && (size_t)(4 * tn * rows) <= s->mv_part_floats) ? 4 : 1;
        if (sk > 1) {
            CALL(s, K_IQ4_MMA_NB8_SK, rtiles * sk, a_int(rows), a_int(rows), a_int(cols), a_int(tn), a_int(sk),
                 a_view(wq, 0, rows * blocks * 4), a_view(wd, 0, rows * blocks),
                 a_u32v(xb_buf, xb_word_off + t0 * (cols / 2), tn * (cols / 2)),
                 a_f32v(s->mv_part, 0, sk * tn * rows));
            CALL(s, K_SK_REDUCE, tn * rows, a_int(tn * rows), a_int(sk), a_int(0),
                 a_f32v(s->mv_part, 0, sk * tn * rows), a_f32v(out_buf, out_elem_off + t0 * rows, tn * rows));
        } else {
            CALL(s, K_IQ4_MMA_NB8_SK, rtiles, a_int(rows), a_int(rows), a_int(cols), a_int(tn), a_int(1),
                 a_view(wq, 0, rows * blocks * 4), a_view(wd, 0, rows * blocks),
                 a_u32v(xb_buf, xb_word_off + t0 * (cols / 2), tn * (cols / 2)),
                 a_f32v(out_buf, out_elem_off + t0 * rows, tn * rows));
        }
    }
    return 0;
}

static int dspark_upload_iq4_relayout(InkSession *s, FILE *f, MtpSpan sp, uint8_t *hbuf, size_t hbuf_bytes,
                                      PcBuffer dst_q, PcBuffer dst_d, int64_t total_rows, int64_t row0_base,
                                      int64_t n_rows, int64_t cols) {
    int64_t blocks = cols / 32;
    size_t row_bytes = (size_t)cols * 2;
    size_t cap_bytes = hbuf_bytes < s->stage_bytes ? hbuf_bytes : s->stage_bytes;
    int64_t piece = (int64_t)(cap_bytes / row_bytes);
    if (piece > n_rows) piece = n_rows;
    if (piece <= 0) return 1;
    if (mtp_seek(f, sp.off) != 0) return 1;
    for (int64_t r = 0; r < n_rows; r += piece) {
        int64_t n = (n_rows - r < piece) ? (n_rows - r) : piece;
        size_t nb = (size_t)n * row_bytes;
        if (fread(hbuf, 1, nb, f) != nb) return 1;
        if (pc_upload(s->stage, hbuf, nb, s->stream) != PC_OK) return 1;
        CALL(s, K_BF16_TO_IQ4_RELAYOUT, n * blocks,
             a_int(total_rows), a_int(row0_base + r), a_int(n), a_int(blocks),
             a_u16v(s->stage, 0, n * cols),
             a_view(dst_q, 0, total_rows * blocks * 4), a_view(dst_d, 0, total_rows * blocks));
        if (pc_sync_stream(s->stream) != PC_OK) return 1;
    }
    return 0;
}

static int dspark_upload_iq4_row(InkSession *s, FILE *f, MtpSpan sp, uint8_t *hbuf, size_t hbuf_bytes,
                                 PcBuffer dst_q, PcBuffer dst_d, int64_t n_rows, int64_t cols) {
    int64_t blocks = cols / 32;
    size_t row_bytes = (size_t)cols * 2;
    size_t cap_bytes = hbuf_bytes < s->stage_bytes ? hbuf_bytes : s->stage_bytes;
    int64_t piece = (int64_t)(cap_bytes / row_bytes);
    if (piece > n_rows) piece = n_rows;
    if (piece <= 0) return 1;
    if (mtp_seek(f, sp.off) != 0) return 1;
    for (int64_t r = 0; r < n_rows; r += piece) {
        int64_t n = (n_rows - r < piece) ? (n_rows - r) : piece;
        size_t nb = (size_t)n * row_bytes;
        if (fread(hbuf, 1, nb, f) != nb) return 1;
        if (pc_upload(s->stage, hbuf, nb, s->stream) != PC_OK) return 1;
        CALL(s, K_BF16_TO_IQ4_ROW, n * blocks,
             a_int(r), a_int(n), a_int(blocks),
             a_u16v(s->stage, 0, n * cols),
             a_view(dst_q, 0, n_rows * blocks * 4), a_view(dst_d, 0, n_rows * blocks));
        if (pc_sync_stream(s->stream) != PC_OK) return 1;
    }
    return 0;
}

static void ink_dspark_close(InkSession *s) {
    if (!s) return;
    CudaDspark *d = &s->dspark;
    for (int i = 0; i < 8; i++) {
        if (d->draft_graph[i]) { pc_graph_free(d->draft_graph[i]); d->draft_graph[i] = NULL; }
    }
    for (int i = 0; i < 9; i++) {
        for (int j = 0; j < 9; j++) {
            if (d->absorb_graph[i][j]) { pc_graph_free(d->absorb_graph[i][j]); d->absorb_graph[i][j] = NULL; }
        }
    }
    for (int i = 0; i < 3; i++) {
        free_buf(&d->layers[i].kv_k);
        free_buf(&d->layers[i].kv_v);
    }
    free_buf(&d->small_buf);
    free_buf(&d->weights_buf);
    free_buf(&d->scratch_buf);
    d->ready = 0;
}

static void ink_dspark_open(InkSession *s, const char *gguf_path) {
    CudaDspark *d = &s->dspark;
    d->ready = 0;
    const char *ds_env = getenv("INK_DSPARK");
    if (ds_env && ds_env[0] == '0') return;
    const InkHParams *hp = &s->gguf.hp;
    if (!s->can_verify_nb || s->gguf.output.type != INK_GGML_Q8_0) return;
    if (hp->n_embd != 2304 || hp->loop_count != 3 || hp->loop_start != 6 ||
        hp->loop_end != 18 || hp->n_phys != 24 || hp->n_vocab > 201024) return;

    char path_buf[1024];
    const char *mtp_path = getenv("INK_MTP_PATH");
    if (!mtp_path || !*mtp_path) {
        const char *sl1 = gguf_path ? strrchr(gguf_path, '/') : NULL;
        const char *sl2 = gguf_path ? strrchr(gguf_path, '\\') : NULL;
        const char *slash = (sl1 && sl2) ? (sl1 > sl2 ? sl1 : sl2) : (sl1 ? sl1 : sl2);
        if (slash) {
            size_t dir_len = (size_t)(slash - gguf_path);
            if (dir_len + 17 >= sizeof path_buf) return;
            memcpy(path_buf, gguf_path, dir_len);
            memcpy(path_buf + dir_len, "/mtp.safetensors", 17);
        } else {
            snprintf(path_buf, sizeof path_buf, "mtp.safetensors");
        }
        mtp_path = path_buf;
    }

    FILE *f = fopen(mtp_path, "rb");
    if (!f) return;
    if (mtp_seek(f, 0) != 0) { fclose(f); return; }
#ifdef _WIN32
    _fseeki64(f, 0, SEEK_END);
    int64_t fsize64 = _ftelli64(f);
#else
    fseeko(f, 0, SEEK_END);
    int64_t fsize64 = (int64_t)ftello(f);
#endif
    if (fsize64 < 64) { fclose(f); return; }
    size_t fsize = (size_t)fsize64;
    mtp_seek(f, 0);
    uint64_t hdr_len = 0;
    if (fread(&hdr_len, 1, 8, f) != 8 || hdr_len == 0 || hdr_len > (1ULL << 20) || 8 + hdr_len >= fsize) {
        fclose(f); return;
    }
    int64_t data_start = (int64_t)(8 + hdr_len);
    char *hdr = (char *)malloc((size_t)hdr_len + 1);
    if (!hdr) { fclose(f); return; }
    if (fread(hdr, 1, (size_t)hdr_len, f) != (size_t)hdr_len) { free(hdr); fclose(f); return; }
    hdr[hdr_len] = '\0';

    MtpSpan sp_fuse, sp_mask, sp_norm, sp_mh_dn, sp_mh_hw, sp_mh_hb, sp_mh_mw1, sp_mh_rw, sp_mh_rb;
    MtpSpan sp_l[3][15];
    int err = 0;
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "fuse_weight", 4 * 4 * 2, &sp_fuse);
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "mask_embed", 2304 * 2, &sp_mask);
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "norm.weight", 2304 * 2, &sp_norm);
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "markov_head.dflare_norm.weight", 2304 * 2, &sp_mh_dn);
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "markov_head.hidden_out.weight", 2304LL * 256 * 2, &sp_mh_hw);
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "markov_head.hidden_out.bias", 2304 * 2, &sp_mh_hb);
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "markov_head.markov_w1.weight", 201024LL * 256 * 2, &sp_mh_mw1);
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "markov_head.rnn_proj.weight", 768LL * 5120 * 2, &sp_mh_rw);
    err |= mtp_find_span_cuda(hdr, data_start, fsize, "markov_head.rnn_proj.bias", 768 * 2, &sp_mh_rb);
    static const char *const lsuf[15] = {
        "dflare_norm.weight", "input_layernorm.weight", "post_attention_layernorm.weight",
        "self_attn.q_norm.weight", "self_attn.k_norm.weight", "self_attn.k_norm_ctx.weight",
        "self_attn.q_proj.weight", "self_attn.k_proj.weight", "self_attn.v_proj.weight",
        "self_attn.k_proj_ctx.weight", "self_attn.v_proj_ctx.weight", "self_attn.o_proj.weight",
        "mlp.gate_proj.weight", "mlp.up_proj.weight", "mlp.down_proj.weight"
    };
    static const int64_t lbytes[15] = {
        2304*2, 2304*2, 2304*2, 128*2, 128*2, 128*2,
        3072LL*2304*2, 1024LL*2304*2, 1024LL*2304*2,
        1024LL*2304*2, 1024LL*2304*2, 2304LL*3072*2,
        9216LL*2304*2, 9216LL*2304*2, 2304LL*9216*2
    };
    for (int i = 0; i < 3; i++) {
        char nbuf[96];
        for (int k = 0; k < 15; k++) {
            snprintf(nbuf, sizeof nbuf, "layers.%d.%s", i, lsuf[k]);
            err |= mtp_find_span_cuda(hdr, data_start, fsize, nbuf, lbytes[k], &sp_l[i][k]);
        }
    }
    free(hdr);
    if (err != 0) { fclose(f); return; }

    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
    int64_t pb = s->batch;
    d->kv_fp8 = s->kv_fp8;
    size_t kv_need_bf16 = 0, kv_need_fp8 = 0;
    for (int i = 0; i < 3; i++) {
        d->layers[i].is_swa = (i == 0 || i == 2) ? 1 : 0;
        int ring = 1024 + (int)pb;
        int cap = d->layers[i].is_swa ? (ring < s->max_ctx ? ring : s->max_ctx) : s->max_ctx;
        d->layers[i].kv_cap = cap;
        kv_need_bf16 += (size_t)cap * 1024 * 4;
        kv_need_fp8  += (size_t)cap * 1024 * 2;
    }

    /* Plan contiguous arenas: small_buf (~1 MB), weights_buf (~215 MB IQ4_NL), scratch_buf (~13 MB). */
#define ALIGN256(x) (((x) + 255u) & ~(size_t)255u)
    size_t small_sz = 0;
    size_t off_mask = small_sz; small_sz += ALIGN256(2304 * 2);
    size_t off_norm = small_sz; small_sz += ALIGN256(2304 * 2);
    size_t off_mhdn = small_sz; small_sz += ALIGN256(2304 * 2);
    size_t off_mhrb = small_sz; small_sz += ALIGN256(768 * 2);
    size_t off_rwst = small_sz; small_sz += ALIGN256(768 * 512 * 2);
    size_t off_ifrq = small_sz; small_sz += ALIGN256(64 * 4);
    size_t off_foldb = small_sz; small_sz += ALIGN256((size_t)nv * 4);
    size_t off_l1d[3][6];
    for (int i = 0; i < 3; i++) {
        for (int k = 0; k < 6; k++) {
            off_l1d[i][k] = small_sz;
            small_sz += ALIGN256((size_t)lbytes[k]);
        }
    }

    size_t w_sz = 0;
    size_t off_lq[3][6], off_ld[3][6]; /* 0=qkv, 1=o, 2=gu, 3=dn, 4=kctx, 5=vctx */
    static const int64_t lshape[6][2] = {{5120,2304},{2304,3072},{18432,2304},{2304,9216},{1024,2304},{1024,2304}};
    for (int i = 0; i < 3; i++) {
        for (int k = 0; k < 6; k++) {
            size_t nblk = (size_t)lshape[k][0] * (size_t)(lshape[k][1] / 32);
            off_lq[i][k] = w_sz; w_sz += ALIGN256(nblk * 16);
            off_ld[i][k] = w_sz; w_sz += ALIGN256(nblk * 2);
        }
    }
    size_t off_rc_q = w_sz; w_sz += ALIGN256((size_t)768 * (4608 / 32) * 16);
    size_t off_rc_d = w_sz; w_sz += ALIGN256((size_t)768 * (4608 / 32) * 2);
    size_t off_mw_q = w_sz; w_sz += ALIGN256((size_t)201024 * 8 * 16);
    size_t off_mw_d = w_sz; w_sz += ALIGN256((size_t)201024 * 8 * 2);
    size_t off_fw_q = w_sz; w_sz += ALIGN256((size_t)nv * 8 * 16);
    size_t off_fw_d = w_sz; w_sz += ALIGN256((size_t)nv * 8 * 2);

    size_t sc_sz = 0;
    size_t off_taps = sc_sz; sc_sz += ALIGN256((size_t)4 * (size_t)pb * 2304 * 4);
    size_t off_ltap = sc_sz; sc_sz += ALIGN256((size_t)4 * 2304 * 4);
    size_t off_ctxb = sc_sz; sc_sz += ALIGN256((size_t)pb * 2304 * 2);
    size_t off_ctxk = sc_sz; sc_sz += ALIGN256((size_t)pb * 1024 * 4);
    size_t off_ctxv = sc_sz; sc_sz += ALIGN256((size_t)pb * 1024 * 4);
    size_t off_anc  = sc_sz; sc_sz += ALIGN256(2304 * 4);
    size_t off_frnn = sc_sz; sc_sz += ALIGN256(2304 * 2);
    size_t off_res  = sc_sz; sc_sz += ALIGN256(8 * 2304 * 4);
    size_t off_xb   = sc_sz; sc_sz += ALIGN256(8 * 2304 * 2);
    size_t off_qkv  = sc_sz; sc_sz += ALIGN256(8 * 5120 * 4);
    size_t off_attb = sc_sz; sc_sz += ALIGN256(8 * 3072 * 2);
    size_t off_tmp  = sc_sz; sc_sz += ALIGN256(8 * 2304 * 4);
    size_t off_gu   = sc_sz; sc_sz += ALIGN256(8 * 18432 * 4);
    size_t off_mlpb = sc_sz; sc_sz += ALIGN256(8 * 9216 * 2);
    size_t off_mkin = sc_sz; sc_sz += ALIGN256(7 * 4608 * 2);
    size_t off_rctx = sc_sz; sc_sz += ALIGN256(7 * 768 * 4);
    size_t off_rst1 = sc_sz; sc_sz += ALIGN256(256 * 4);
    size_t off_rst2 = sc_sz; sc_sz += ALIGN256(256 * 4);
    size_t off_rlat = sc_sz; sc_sz += ALIGN256(256 * 4);
    size_t off_qrot = sc_sz; sc_sz += ALIGN256(8 * 3072 * 4);
    size_t off_krot = sc_sz; sc_sz += ALIGN256(8 * 1024 * 4);
    size_t off_vblk = sc_sz; sc_sz += ALIGN256(8 * 1024 * 4);
    size_t off_asko = sc_sz; sc_sz += ALIGN256(24 * 8 * 8 * 128 * 4);
    size_t off_askm = sc_sz; sc_sz += ALIGN256(24 * 8 * 8 * 2 * 4);
    size_t off_s1m  = sc_sz; sc_sz += ALIGN256(256 * 4);
    size_t off_s1i  = sc_sz; sc_sz += ALIGN256(256 * 4);
    size_t off_s1s  = sc_sz; sc_sz += ALIGN256(256 * 4);
    size_t off_dtok = sc_sz; sc_sz += ALIGN256(8 * 4);
    size_t off_dprb = sc_sz; sc_sz += ALIGN256(8 * 4);
    size_t off_dpos = sc_sz; sc_sz += ALIGN256(256);
#undef ALIGN256

    size_t kv_need = d->kv_fp8 ? kv_need_fp8 : kv_need_bf16;
    size_t stage_reclaim = (!s->streaming && s->stage) ? s->stage_bytes : 0;
    size_t total_need = small_sz + w_sz + sc_sz + kv_need;
    size_t free_now = 0, total_now = 0;
    if (pc_memory(&free_now, &total_now) != PC_OK) { fclose(f); return; }
    if (!d->kv_fp8 && free_now + stage_reclaim < total_need + ((size_t)48 << 20) &&
        free_now + stage_reclaim >= small_sz + w_sz + sc_sz + kv_need_fp8 + ((size_t)8 << 20)) {
        d->kv_fp8 = 1;
        kv_need = kv_need_fp8;
        total_need = small_sz + w_sz + sc_sz + kv_need;
    }
    if (free_now + stage_reclaim < total_need + ((size_t)8 << 20)) {
        fprintf(stderr, "psionic-cuda: DSpark skipped (need %lld MB, %lld MB VRAM free)\n",
                (long long)to_mb(total_need), (long long)to_mb(free_now));
        fclose(f);
        return;
    }

    if (alloc_zero(s, &d->small_buf, small_sz) ||
        alloc_zero(s, &d->weights_buf, w_sz) ||
        alloc_zero(s, &d->scratch_buf, sc_sz)) {
        s->error[0] = '\0';
        ink_dspark_close(s); fclose(f); return;
    }

    d->mask_embed = d->small_buf + off_mask;
    d->norm_w = d->small_buf + off_norm;
    d->mh_dflare_norm = d->small_buf + off_mhdn;
    d->mh_rnn_bias = d->small_buf + off_mhrb;
    d->rnn_w_step = d->small_buf + off_rwst;
    d->inv_freq = d->small_buf + off_ifrq;
    d->fold_b = d->small_buf + off_foldb;
    for (int i = 0; i < 3; i++) {
        d->layers[i].dflare_norm  = d->small_buf + off_l1d[i][0];
        d->layers[i].input_ln     = d->small_buf + off_l1d[i][1];
        d->layers[i].post_attn_ln = d->small_buf + off_l1d[i][2];
        d->layers[i].q_norm       = d->small_buf + off_l1d[i][3];
        d->layers[i].k_norm       = d->small_buf + off_l1d[i][4];
        d->layers[i].k_norm_ctx   = d->small_buf + off_l1d[i][5];
        d->layers[i].qkv_q  = d->weights_buf + off_lq[i][0]; d->layers[i].qkv_d  = d->weights_buf + off_ld[i][0];
        d->layers[i].o_q    = d->weights_buf + off_lq[i][1]; d->layers[i].o_d    = d->weights_buf + off_ld[i][1];
        d->layers[i].gu_q   = d->weights_buf + off_lq[i][2]; d->layers[i].gu_d   = d->weights_buf + off_ld[i][2];
        d->layers[i].dn_q   = d->weights_buf + off_lq[i][3]; d->layers[i].dn_d   = d->weights_buf + off_ld[i][3];
        d->layers[i].kctx_q = d->weights_buf + off_lq[i][4]; d->layers[i].kctx_d = d->weights_buf + off_ld[i][4];
        d->layers[i].vctx_q = d->weights_buf + off_lq[i][5]; d->layers[i].vctx_d = d->weights_buf + off_ld[i][5];
    }
    d->rnn_ctx_q = d->weights_buf + off_rc_q; d->rnn_ctx_d = d->weights_buf + off_rc_d;
    d->mw1_q = d->weights_buf + off_mw_q; d->mw1_d = d->weights_buf + off_mw_d;
    d->fold_q = d->weights_buf + off_fw_q; d->fold_d = d->weights_buf + off_fw_d;

    d->taps = d->scratch_buf + off_taps;
    d->last_taps = d->scratch_buf + off_ltap;
    d->ctx_bf16 = d->scratch_buf + off_ctxb;
    d->ctx_k_f32 = d->scratch_buf + off_ctxk;
    d->ctx_v_f32 = d->scratch_buf + off_ctxv;
    d->anchor_emb = d->scratch_buf + off_anc;
    d->f_rnn_bf16 = d->scratch_buf + off_frnn;
    d->res = d->scratch_buf + off_res;
    d->x_bf16 = d->scratch_buf + off_xb;
    d->qkv_f32 = d->scratch_buf + off_qkv;
    d->attn_bf16 = d->scratch_buf + off_attb;
    d->tmp_f32 = d->scratch_buf + off_tmp;
    d->gu_f32 = d->scratch_buf + off_gu;
    d->mlp_bf16 = d->scratch_buf + off_mlpb;
    d->markov_in_bf16 = d->scratch_buf + off_mkin;
    d->rnn_ctx = d->scratch_buf + off_rctx;
    d->rnn_state = d->scratch_buf + off_rst1;
    d->rnn_state2 = d->scratch_buf + off_rst2;
    d->rnn_latent = d->scratch_buf + off_rlat;
    d->base_logits = s->spec_logits; /* Aliased safely: draft_graph & verify_graph never overlap! Saves 5.6 MB VRAM! */
    d->q_rot = d->scratch_buf + off_qrot;
    d->k_rot = d->scratch_buf + off_krot;
    d->v_blk = d->scratch_buf + off_vblk;
    d->attn_sk_o = d->scratch_buf + off_asko;
    d->attn_sk_ms = d->scratch_buf + off_askm;
    d->s1_m = d->scratch_buf + off_s1m;
    d->s1_i = d->scratch_buf + off_s1i;
    d->s1_s = d->scratch_buf + off_s1s;
    d->draft_toks = d->scratch_buf + off_dtok;
    d->draft_probs = d->scratch_buf + off_dprb;
    d->dev_pos = d->scratch_buf + off_dpos;

    size_t hbuf_bytes = (size_t)20 << 20;
    uint8_t *hbuf = (uint8_t *)malloc(hbuf_bytes);
    if (!hbuf) { ink_dspark_close(s); fclose(f); return; }

    /* 1. fuse_weight softmax -> fuse_alpha[4][4] + NeoX inv_freq */
    uint16_t fw_raw[16];
    if (mtp_seek(f, sp_fuse.off) != 0 || fread(fw_raw, 2, 16, f) != 16) goto fail_open;
    for (int r = 0; r < 4; r++) {
        float v[4], mx = -1e30f, sum = 0.0f;
        for (int c = 0; c < 4; c++) { v[c] = bf16_to_f32_cuda_host(fw_raw[r * 4 + c]); if (v[c] > mx) mx = v[c]; }
        for (int c = 0; c < 4; c++) { v[c] = expf(v[c] - mx); sum += v[c]; }
        for (int c = 0; c < 4; c++) d->fuse_alpha[r][c] = v[c] / sum;
    }
    {
        float ifreq[64];
        for (int i = 0; i < 64; i++) ifreq[i] = powf(1000000.0f, -(float)(2 * i) / 128.0f);
        if (pc_upload(d->inv_freq, ifreq, sizeof ifreq, s->stream) != PC_OK) goto fail_open;
    }

    /* 2. Upload 1D BF16 norms/biases */
#define UP_SMALL(SPAN, DST) do { \
    if (mtp_seek(f, (SPAN).off) != 0 || fread(hbuf, 1, (size_t)(SPAN).bytes, f) != (size_t)(SPAN).bytes) goto fail_open; \
    if (pc_upload((DST), hbuf, (size_t)(SPAN).bytes, s->stream) != PC_OK) goto fail_open; \
} while (0)
    UP_SMALL(sp_mask, d->mask_embed);
    UP_SMALL(sp_norm, d->norm_w);
    UP_SMALL(sp_mh_dn, d->mh_dflare_norm);
    UP_SMALL(sp_mh_rb, d->mh_rnn_bias);
    for (int i = 0; i < 3; i++) {
        UP_SMALL(sp_l[i][0], d->layers[i].dflare_norm);
        UP_SMALL(sp_l[i][1], d->layers[i].input_ln);
        UP_SMALL(sp_l[i][2], d->layers[i].post_attn_ln);
        UP_SMALL(sp_l[i][3], d->layers[i].q_norm);
        UP_SMALL(sp_l[i][4], d->layers[i].k_norm);
        UP_SMALL(sp_l[i][5], d->layers[i].k_norm_ctx);
    }
#undef UP_SMALL

    /* 3. Split rnn_proj.weight [768, 5120] -> BF16 rnn_w_step [768, 512] + IQ4_NL rnn_ctx [768, 4608] */
    {
        if (mtp_seek(f, sp_mh_rw.off) != 0 || fread(hbuf, 1, (size_t)sp_mh_rw.bytes, f) != (size_t)sp_mh_rw.bytes) goto fail_open;
        uint16_t *rw_step = (uint16_t *)malloc(768 * 512 * 2);
        uint16_t *rw_ctx  = (uint16_t *)malloc(768 * 4608 * 2);
        if (!rw_step || !rw_ctx) { free(rw_step); free(rw_ctx); goto fail_open; }
        const uint16_t *rw = (const uint16_t *)hbuf;
        for (int r = 0; r < 768; r++) {
            memcpy(rw_step + (size_t)r * 512, rw + (size_t)r * 5120, 512 * 2);
            memcpy(rw_ctx  + (size_t)r * 4608, rw + (size_t)r * 5120 + 512, 4608 * 2);
        }
        int ok = (pc_upload(d->rnn_w_step, rw_step, 768 * 512 * 2, s->stream) == PC_OK &&
                  pc_upload(s->stage, rw_ctx, 768 * 4608 * 2, s->stream) == PC_OK);
        free(rw_step); free(rw_ctx);
        if (!ok) goto fail_open;
        int64_t rblk = 4608 / 32;
        PcArg ra[] = { a_int(768), a_int(0), a_int(768), a_int(rblk),
                       a_u16v(s->stage, 0, 768 * 4608),
                       a_view(d->rnn_ctx_q, 0, 768 * rblk * 4), a_view(d->rnn_ctx_d, 0, 768 * rblk) };
        if (call_k(s, K_BF16_TO_IQ4_RELAYOUT, 768 * rblk, 7, ra) != 0 || pc_sync_stream(s->stream) != PC_OK) goto fail_open;
    }

    /* 4. Quantize 3 DraftLayers + ctx projections -> block-major IQ4_NL on GPU */
    for (int i = 0; i < 3; i++) {
        CudaDsparkLayer *L = &d->layers[i];
        if (dspark_upload_iq4_relayout(s, f, sp_l[i][6], hbuf, hbuf_bytes, L->qkv_q, L->qkv_d, 5120, 0, 3072, 2304) ||
            dspark_upload_iq4_relayout(s, f, sp_l[i][7], hbuf, hbuf_bytes, L->qkv_q, L->qkv_d, 5120, 3072, 1024, 2304) ||
            dspark_upload_iq4_relayout(s, f, sp_l[i][8], hbuf, hbuf_bytes, L->qkv_q, L->qkv_d, 5120, 4096, 1024, 2304) ||
            dspark_upload_iq4_relayout(s, f, sp_l[i][11], hbuf, hbuf_bytes, L->o_q, L->o_d, 2304, 0, 2304, 3072) ||
            dspark_upload_iq4_relayout(s, f, sp_l[i][12], hbuf, hbuf_bytes, L->gu_q, L->gu_d, 18432, 0, 9216, 2304) ||
            dspark_upload_iq4_relayout(s, f, sp_l[i][13], hbuf, hbuf_bytes, L->gu_q, L->gu_d, 18432, 9216, 9216, 2304) ||
            dspark_upload_iq4_relayout(s, f, sp_l[i][14], hbuf, hbuf_bytes, L->dn_q, L->dn_d, 2304, 0, 2304, 9216) ||
            dspark_upload_iq4_relayout(s, f, sp_l[i][9],  hbuf, hbuf_bytes, L->kctx_q, L->kctx_d, 1024, 0, 1024, 2304) ||
            dspark_upload_iq4_relayout(s, f, sp_l[i][10], hbuf, hbuf_bytes, L->vctx_q, L->vctx_d, 1024, 0, 1024, 2304))
            goto fail_open;
    }

    /* 5. Quantize markov_w1 [201024, 256] -> row-major IQ4_NL on GPU */
    if (dspark_upload_iq4_row(s, f, sp_mh_mw1, hbuf, hbuf_bytes, d->mw1_q, d->mw1_d, 201024, 256) != 0) goto fail_open;

    /* 6. Fold fold_b = lm_head(hidden_out.bias) and fold_w = IQ4_NL(lm_head @ hidden_out.weight) on GPU Tensor Cores! */
    {
        uint16_t hb_raw[2304];
        float hb_f32[2304];
        if (mtp_seek(f, sp_mh_hb.off) != 0 || fread(hb_raw, 2, 2304, f) != 2304) goto fail_open;
        for (int i = 0; i < 2304; i++) hb_f32[i] = bf16_to_f32_cuda_host(hb_raw[i]);
        if (pc_upload(d->anchor_emb, hb_f32, sizeof hb_f32, s->stream) != PC_OK) goto fail_open;
        int64_t out_blocks = 2304 / 32;
        int64_t lrows = (int64_t)(s->gguf.output.nbytes / (uint64_t)(out_blocks * 34));
        if (s->output_iq4) {
            PcArg fa[] = {
                a_int(nv), a_int(lrows), a_int(2304), a_int(0), a_int(0),
                a_view(s->output_dev, 0, lrows * out_blocks * 4),
                a_view(s->output_dev, lrows * out_blocks * 16, lrows * out_blocks),
                a_f32v(d->anchor_emb, 0, 2304), a_f32v(d->fold_b, 0, nv)
            };
            if (call_k(s, K_DVQ, (nv + 31) / 32, 9, fa) != 0) goto fail_open;
        } else {
            if (dispatch_mv(s, &s->gguf.output, 1, nv, 2304, 0, d->anchor_emb, 0, d->fold_b, 0) != 0) goto fail_open;
        }

        if (mtp_seek(f, sp_mh_hw.off) != 0 || fread(hbuf, 1, (size_t)sp_mh_hw.bytes, f) != (size_t)sp_mh_hw.bytes) goto fail_open;
        uint16_t *hw_t = (uint16_t *)malloc(256 * 2304 * 2);
        if (!hw_t) goto fail_open;
        const uint16_t *hw = (const uint16_t *)hbuf;
        for (int r = 0; r < 2304; r++) {
            for (int c = 0; c < 256; c++) hw_t[(size_t)c * 2304 + r] = hw[(size_t)r * 256 + c];
        }
        int ok = (pc_upload(s->stage, hw_t, 256 * 2304 * 2, s->stream) == PC_OK);
        free(hw_t);
        if (!ok) goto fail_open;

        PcBuffer out_tmp = s->stage + ((size_t)2 << 20); /* 4 MB chunk buffer inside s->stage */
        for (int64_t row0 = 0; row0 < nv; row0 += 4096) {
            int64_t vc = (nv - row0 < 4096) ? (nv - row0) : 4096;
            if (s->output_iq4) {
                int64_t rtiles = (vc + 15) / 16;
                for (int64_t c0 = 0; c0 < 256; c0 += 8) {
                    PcArg ma[] = {
                        a_int(vc), a_int(lrows), a_int(2304), a_int(8), a_int(1),
                        a_view(s->output_dev, row0 * 4, lrows * out_blocks * 4 - row0),
                        a_view(s->output_dev, lrows * out_blocks * 16 + row0 * 2, lrows * out_blocks - row0),
                        a_u32v(s->stage, c0 * 1152, 8 * 1152),
                        a_f32v(out_tmp, c0 * vc, 8 * vc)
                    };
                    if (call_k(s, K_IQ4_MMA_NB8_SK, rtiles, 9, ma) != 0) goto fail_open;
                }
            } else {
                int64_t warps = ((vc + 15) / 16) * (256 / 16);
                PcArg ma[] = {
                    a_int(vc), a_int(lrows), a_int(2304), a_int(256), a_int(0),
                    a_view(s->output_dev, row0 * 4, lrows * out_blocks * 8 - row0),
                    a_view(s->output_dev, lrows * out_blocks * 32 + row0 * 2, lrows * out_blocks - row0),
                    a_u32v(s->stage, 0, 256 * 1152),
                    a_f32v(out_tmp, 0, 256 * vc)
                };
                if (call_k(s, K_Q8_MMA, warps, 9, ma) != 0) goto fail_open;
            }
            PcArg qa[] = {
                a_int(vc), a_int(row0), a_int(8),
                a_f32v(out_tmp, 0, 256 * vc),
                a_view(d->fold_q, 0, nv * 8 * 4), a_view(d->fold_d, 0, nv * 8)
            };
            if (call_k(s, K_F32T_TO_IQ4_ROW, vc * 8, 6, qa) != 0) goto fail_open;
        }
        if (pc_sync_stream(s->stream) != PC_OK) goto fail_open;
    }

    free(hbuf);
    fclose(f);
    /* Reclaim staging buffer on resident GPUs BEFORE allocating DSpark KV caches so 32K FP8 KV + DSpark fit in 16 GB! */
    if (!s->streaming && s->stage) { free_buf(&s->stage); s->stage_bytes = 0; }
    {
        size_t dkv_b = d->kv_fp8 ? 1024 : 2048;
        for (int i = 0; i < 3; i++) {
            size_t cap = (size_t)d->layers[i].kv_cap;
            if (alloc_zero(s, &d->layers[i].kv_k, cap * dkv_b) ||
                alloc_zero(s, &d->layers[i].kv_v, cap * dkv_b)) {
                s->error[0] = '\0';
                ink_dspark_close(s);
                return;
            }
        }
    }
    d->pos = 0;
    d->pending_pos0 = 0;
    d->pending_nb = 0;
    d->pending_got = 0;
    d->skip_countdown = 0;
    d->skip_mult = 1;
    d->req_rounds = 0;
    d->req_disabled = 0;
    d->ema_acc = 3.0f;
    d->util_ema = 1.10f;
    d->ready = 1;
    fprintf(stderr, "psionic-cuda: DSpark ready (%lld MB VRAM, default ON; INK_DSPARK=0 to disable)\n",
            (long long)to_mb(total_need));
    return;

fail_open:
    free(hbuf);
    fclose(f);
    s->error[0] = '\0';
    ink_dspark_close(s);
}

static int enqueue_dspark_absorb_body(InkSession *s, int stride_nb, int n_tok, int64_t pos_arg) {
    CudaDspark *d = &s->dspark;
    int64_t d_dim = 2304;
    int64_t kv_u16_stride = d->kv_fp8 ? 512 : 1024;
    for (int k = 0; k < 4; k++) {
        int64_t src_off = ((int64_t)k * stride_nb + (n_tok - 1)) * d_dim;
        CALL(s, K_COPY, d_dim, a_int(d_dim),
             a_f32v(d->taps, src_off, d_dim), a_f32v(d->last_taps, (int64_t)k * d_dim, d_dim));
    }
    for (int li = 0; li < 3; li++) {
        CudaDsparkLayer *L = &d->layers[li];
        CALL(s, K_DSPARK_MIX_NORM, n_tok,
             a_int(stride_nb), a_int(d_dim),
             a_f32(d->fuse_alpha[li][0]), a_f32(d->fuse_alpha[li][1]),
             a_f32(d->fuse_alpha[li][2]), a_f32(d->fuse_alpha[li][3]),
             a_f32(1e-6f),
             a_f32v(d->taps, 0, (int64_t)4 * stride_nb * d_dim),
             a_u16v(L->dflare_norm, 0, d_dim),
             a_u16v(d->ctx_bf16, 0, (int64_t)n_tok * d_dim));
        if (dspark_iq4_mm(s, L->kctx_q, L->kctx_d, 1024, d_dim, n_tok, d->ctx_bf16, 0, d->ctx_k_f32, 0) != 0) return 1;
        if (dspark_iq4_mm(s, L->vctx_q, L->vctx_d, 1024, d_dim, n_tok, d->ctx_bf16, 0, d->ctx_v_f32, 0) != 0) return 1;
        CALL(s, K_DSPARK_KV_ABSORB, (int64_t)n_tok * 8,
             a_int(pos_arg), a_int(L->kv_cap), a_int(L->is_swa | (d->kv_fp8 ? 2 : 0)), a_f32(1e-6f),
             a_f32v(d->ctx_k_f32, 0, (int64_t)n_tok * 1024),
             a_f32v(d->ctx_v_f32, 0, (int64_t)n_tok * 1024),
             a_u16v(L->k_norm_ctx, 0, 128),
             a_f32v(d->inv_freq, 0, 64),
             a_u16v(L->kv_k, 0, (int64_t)L->kv_cap * kv_u16_stride),
             a_u16v(L->kv_v, 0, (int64_t)L->kv_cap * kv_u16_stride),
             a_u32v(d->dev_pos, 0, 1));
    }
    return 0;
}

static int ink_dspark_enqueue_absorb(InkSession *s, int stride_nb, int n_tok, int pos0) {
    CudaDspark *d = &s->dspark;
    if (!d->ready || n_tok <= 0 || n_tok > stride_nb) return 0;
    if (stride_nb <= 8 && n_tok <= 8 && !s->no_graph && !s->profile) {
        d->pos_host = (uint32_t)pos0;
        if (pc_upload(d->dev_pos, &d->pos_host, 4, s->stream) != PC_OK) return 1;
        if (!d->absorb_graph[stride_nb][n_tok]) {
            if (pc_capture_begin(s->stream) != PC_OK) return 1;
            s->capturing = 1;
            if (enqueue_dspark_absorb_body(s, stride_nb, n_tok, -1) != 0) {
                PcGraph tmp = NULL;
                pc_capture_end(s->stream, &tmp);
                if (tmp) pc_graph_free(tmp);
                s->capturing = 0;
                return 1;
            }
            s->capturing = 0;
            if (pc_capture_end(s->stream, &d->absorb_graph[stride_nb][n_tok]) != PC_OK) return 1;
        }
        if (pc_replay(d->absorb_graph[stride_nb][n_tok], s->stream) != PC_OK) return 1;
    } else {
        if (enqueue_dspark_absorb_body(s, stride_nb, n_tok, (int64_t)pos0) != 0) return 1;
    }
    d->pos = pos0 + n_tok;
    s->dspark_live_len = d->pos;
    int ds_cap = d->layers[0].kv_cap;
    if (ds_cap > 0 && d->pos - ds_cap > s->dspark_swa_valid_from) {
        s->dspark_swa_valid_from = d->pos - ds_cap;
    }
    return 0;
}

static int ink_dspark_flush_pending(InkSession *s) {
    CudaDspark *d = &s->dspark;
    if (!d->ready || d->pending_got <= 0) return 0;
    int n_tok = d->pending_got;
    int stride_nb = d->pending_nb;
    int pos0 = d->pending_pos0;
    d->pending_got = 0;
    d->pending_nb = 0;
    if (pos0 != d->pos) return 0;
    return ink_dspark_enqueue_absorb(s, stride_nb, n_tok, pos0);
}

static int enqueue_dspark_draft_body(InkSession *s, int count) {
    CudaDspark *d = &s->dspark;
    const InkHParams *hp = &s->gguf.hp;
    int64_t d_dim = 2304;
    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
    int64_t pos_arg = s->capturing ? -1 : (int64_t)d->pos;

    /* 1. Anchor embedding + MarkovHead f_rnn mix/norm */
    if (enqueue_embed(s, 1, d->anchor_emb) != 0) return 1;
    CALL(s, K_DSPARK_MIX_NORM, 1,
         a_int(1), a_int(d_dim),
         a_f32(d->fuse_alpha[3][0]), a_f32(d->fuse_alpha[3][1]),
         a_f32(d->fuse_alpha[3][2]), a_f32(d->fuse_alpha[3][3]),
         a_f32(1e-6f),
         a_f32v(d->last_taps, 0, 4 * d_dim),
         a_u16v(d->mh_dflare_norm, 0, d_dim),
         a_u16v(d->f_rnn_bf16, 0, d_dim));

    /* 2. Prepare 8-token block [anchor_emb, mask_embed x 7] + layer-0 input_layernorm */
    CALL(s, K_DSPARK_BLOCK_PREP, 8, a_int(d_dim), a_f32(1e-6f),
         a_f32v(d->anchor_emb, 0, d_dim),
         a_u16v(d->mask_embed, 0, d_dim),
         a_u16v(d->layers[0].input_ln, 0, d_dim),
         a_f32v(d->res, 0, 8 * d_dim),
         a_u16v(d->x_bf16, 0, 8 * d_dim));

    /* 3. Three IQ4_NL DraftLayers (139 MB total, 0 spills, split-K attention!) */
    int64_t dkv_u16_stride = d->kv_fp8 ? 512 : 1024;
    for (int li = 0; li < 3; li++) {
        CudaDsparkLayer *L = &d->layers[li];
        PcBuffer next_ln = (li < 2) ? d->layers[li + 1].input_ln : d->norm_w;
        if (dspark_iq4_mm(s, L->qkv_q, L->qkv_d, 5120, d_dim, 8, d->x_bf16, 0, d->qkv_f32, 0) != 0) return 1;
        CALL(s, K_DSPARK_QKV_ROPE, 24,
             a_int(pos_arg), a_f32(1e-6f),
             a_f32v(d->qkv_f32, 0, 8 * 5120),
             a_u16v(L->q_norm, 0, 128), a_u16v(L->k_norm, 0, 128),
             a_f32v(d->inv_freq, 0, 64),
             a_f32v(d->q_rot, 0, 8 * 3072), a_f32v(d->k_rot, 0, 8 * 1024), a_f32v(d->v_blk, 0, 8 * 1024),
             a_u32v(d->dev_pos, 0, 1));
        CALL(s, K_DSPARK_ATTN_SK, 24 * 8,
             a_int(pos_arg), a_int(L->kv_cap), a_int(L->is_swa | (d->kv_fp8 ? 2 : 0)), a_int(1024), a_int(8),
             a_f32v(d->q_rot, 0, 8 * 3072), a_f32v(d->k_rot, 0, 8 * 1024), a_f32v(d->v_blk, 0, 8 * 1024),
             a_u16v(L->kv_k, 0, (int64_t)L->kv_cap * dkv_u16_stride),
             a_u16v(L->kv_v, 0, (int64_t)L->kv_cap * dkv_u16_stride),
             a_f32v(d->attn_sk_o, 0, 24 * 8 * 8 * 128),
             a_f32v(d->attn_sk_ms, 0, 24 * 8 * 8 * 2),
             a_u32v(d->dev_pos, 0, 1));
        CALL(s, K_DSPARK_ATTN_REDUCE, 24 * 8,
             a_int(8),
             a_f32v(d->attn_sk_o, 0, 24 * 8 * 8 * 128),
             a_f32v(d->attn_sk_ms, 0, 24 * 8 * 8 * 2),
             a_u16v(d->attn_bf16, 0, 8 * 3072));
        if (dspark_iq4_mm(s, L->o_q, L->o_d, d_dim, 3072, 8, d->attn_bf16, 0, d->tmp_f32, 0) != 0) return 1;
        CALL(s, K_DSPARK_ADD_NORM_BF16, 8, a_int(d_dim), a_f32(1e-6f),
             a_f32v(d->tmp_f32, 0, 8 * d_dim), a_f32v(d->res, 0, 8 * d_dim),
             a_u16v(L->post_attn_ln, 0, d_dim), a_u16v(d->x_bf16, 0, 8 * d_dim));
        if (dspark_iq4_mm(s, L->gu_q, L->gu_d, 18432, d_dim, 8, d->x_bf16, 0, d->gu_f32, 0) != 0) return 1;
        CALL(s, K_DSPARK_SILU_MUL_BF16, 8 * 9216,
             a_f32v(d->gu_f32, 0, 8 * 18432), a_u16v(d->mlp_bf16, 0, 8 * 9216));
        if (dspark_iq4_mm(s, L->dn_q, L->dn_d, d_dim, 9216, 8, d->mlp_bf16, 0, d->tmp_f32, 0) != 0) return 1;
        CALL(s, K_DSPARK_ADD_NORM_BF16, 8, a_int(d_dim), a_f32(1e-6f),
             a_f32v(d->tmp_f32, 0, 8 * d_dim), a_f32v(d->res, 0, 8 * d_dim),
             a_u16v(next_ln, 0, d_dim), a_u16v(d->x_bf16, 0, 8 * d_dim));
    }

    /* 4. Base lm_head logits for block positions 1..count + MarkovHead context prep */
    CALL(s, K_DSPARK_MARKOV_CTX_PREP, (int64_t)count * d_dim, a_int(count), a_int(d_dim),
         a_u16v(d->x_bf16, 0, 8 * d_dim), a_u16v(d->f_rnn_bf16, 0, d_dim),
         a_u16v(d->markov_in_bf16, 0, (int64_t)count * 4608));
    if (s->output_iq4) {
        int64_t blocks = d_dim / 32;
        int64_t lrows = (int64_t)(s->gguf.output.nbytes / (uint64_t)(blocks * 34));
        CALL(s, K_IQ4_MMA_NB8_SK, (nv + 15) / 16, a_int(nv), a_int(lrows), a_int(d_dim), a_int(count), a_int(1),
             a_view(s->output_dev, 0, lrows * blocks * 4),
             a_view(s->output_dev, lrows * blocks * 16, lrows * blocks),
             a_u32v(d->x_bf16, d_dim / 2, count * (d_dim / 2)),
             a_f32v(d->base_logits, 0, (int64_t)count * nv));
    } else {
        if (dispatch_q8_mma(s, &s->gguf.output, count, nv, d_dim, 0, d->x_bf16, d_dim / 2, d->base_logits, 0) != 0) return 1;
    }
    if (dspark_iq4_mm(s, d->rnn_ctx_q, d->rnn_ctx_d, 768, 4608, count, d->markov_in_bf16, 0, d->rnn_ctx, 0) != 0) return 1;

    /* 5. GPU-chained autoregressive MarkovHead: 0 CPU syncs across all count steps! */
    for (int step = 0; step < count; step++) {
        PcBuffer st_in = (step & 1) ? d->rnn_state2 : d->rnn_state;
        PcBuffer st_out = (step & 1) ? d->rnn_state : d->rnn_state2;
        CALL(s, K_DSPARK_MARKOV_STEP, 32, a_int(step),
             a_u32v(d->draft_toks, 0, 8),
             a_view(d->mw1_q, 0, 201024LL * 8 * 4), a_view(d->mw1_d, 0, 201024LL * 8),
             a_u16v(d->rnn_w_step, 0, 768 * 512),
             a_f32v(d->rnn_ctx, 0, (int64_t)count * 768),
             a_u16v(d->mh_rnn_bias, 0, 768),
             a_f32v(st_in, 0, 256), a_f32v(st_out, 0, 256), a_f32v(d->rnn_latent, 0, 256));
        CALL(s, K_DSPARK_FOLD_LOGITS_S1, 256, a_int(nv), a_int(step), a_f32(hp->logit_softcap),
             a_view(d->fold_q, 0, nv * 8 * 4), a_view(d->fold_d, 0, nv * 8),
             a_f32v(d->rnn_latent, 0, 256),
             a_f32v(d->base_logits, 0, (int64_t)count * nv),
             a_f32v(d->fold_b, 0, nv),
             a_f32v(d->s1_m, 0, 256), a_u32v(d->s1_i, 0, 256), a_f32v(d->s1_s, 0, 256));
        CALL(s, K_DSPARK_LOGITS_S2, 1, a_int(step),
             a_f32v(d->s1_m, 0, 256), a_u32v(d->s1_i, 0, 256), a_f32v(d->s1_s, 0, 256),
             a_u32v(d->draft_toks, 0, 8), a_f32v(d->draft_probs, 0, 8));
    }
    return 0;
}

static int ink_dspark_draft(InkSession *s, int32_t anchor, int count, int32_t *out_toks, float *out_probs) {
    CudaDspark *d = &s->dspark;
    if (!d->ready || count < 1 || count > 7) return 1;
    if (d->pos + d->pending_got != s->pos || s->pos < 1) return 1;
    double began = pc_seconds();
    if (d->pending_got > 0) {
        if (ink_dspark_flush_pending(s) != 0) return 1;
    }
    if (d->pos != s->pos) return 1;

    uint32_t anc_u32 = (uint32_t)anchor;
    d->pos_host = (uint32_t)d->pos;
    if (upload_embd_row(s, 0, anchor) != 0 ||
        pc_upload(d->draft_toks, &anc_u32, 4, s->stream) != PC_OK ||
        pc_upload(d->dev_pos, &d->pos_host, 4, s->stream) != PC_OK)
        return 1;

    if (!s->no_graph && !s->profile) {
        if (!d->draft_graph[count]) {
            if (pc_capture_begin(s->stream) != PC_OK) return 1;
            s->capturing = 1;
            if (enqueue_dspark_draft_body(s, count) != 0) {
                PcGraph tmp = NULL;
                pc_capture_end(s->stream, &tmp);
                if (tmp) pc_graph_free(tmp);
                s->capturing = 0;
                return 1;
            }
            s->capturing = 0;
            if (pc_capture_end(s->stream, &d->draft_graph[count]) != PC_OK) return 1;
        }
        if (pc_replay(d->draft_graph[count], s->stream) != PC_OK) return 1;
    } else {
        if (enqueue_dspark_draft_body(s, count) != 0) return 1;
    }

    uint32_t h_toks[8] = {0};
    float h_probs[8] = {0};
    if (pc_download(h_toks, d->draft_toks, 8 * 4, s->stream) != PC_OK ||
        pc_download(h_probs, d->draft_probs, 8 * 4, s->stream) != PC_OK ||
        pc_sync_stream(s->stream) != PC_OK)
        return 1;
    double dt = pc_seconds() - began;
    s->total_gpu_seconds += dt;
    d->draft_gpu_s += dt;
    d->last_draft_s = dt;
    for (int i = 0; i < count; i++) {
        out_toks[i] = (int32_t)h_toks[i + 1];
        out_probs[i] = h_probs[i];
    }
    return 0;
}

int ink_open(const char *gguf_path, const char *kernel_path, int max_ctx, InkSession **out, char *err, size_t err_bytes) {
    return ink_open_lora(gguf_path, kernel_path, NULL, max_ctx, 0, out, err, err_bytes);
}

int ink_open_ex(const char *gguf_path, const char *kernel_path, int max_ctx,
                int loop_count_override, InkSession **out, char *err, size_t err_bytes) {
    return ink_open_lora(gguf_path, kernel_path, NULL, max_ctx, loop_count_override, out, err, err_bytes);
}

int ink_open_lora(const char *gguf_path, const char *kernel_path, const char *lora_path, int max_ctx,
                  int loop_count_override, InkSession **out, char *err, size_t err_bytes) {
    *out = NULL;
    InkSession *s = (InkSession *)calloc(1, sizeof *s);
    if (!s) { snprintf(err, err_bytes, "out of memory"); return 1; }

    if (ink_gguf_open(gguf_path, &s->gguf, err, err_bytes) != 0) { free(s); return 1; }
    InkHParams *hp = &s->gguf.hp;
    float *ablate_host = NULL;
    if (lora_path && lora_path[0]) {
        if (ink_lora_open(lora_path, &s->lora, err, err_bytes) != 0) {
            ink_close(s); return 1;
        }
        size_t ab_elems = (size_t)(1 + hp->n_phys) * (size_t)hp->n_embd;
        ablate_host = (float *)calloc(ab_elems, sizeof(float));
        if (!ablate_host) {
            snprintf(err, err_bytes, "out of memory allocating ablation directions");
            ink_close(s); return 1;
        }
        if (ink_lora_extract_ablate(&s->lora, (int)hp->n_phys, (int)hp->n_embd, (int)hp->n_expert, ablate_host,
                                    &s->ablate_has_emb, s->ablate_has_layer, err, err_bytes) != 0) {
            free(ablate_host);
            ink_close(s); return 1;
        }
        s->has_ablate = s->ablate_has_emb ? 1 : 0;
        for (int ip = 0; ip < hp->n_phys && ip < 32; ip++) {
            if (s->ablate_has_layer[ip]) s->has_ablate = 1;
        }
        if (s->ablate_has_emb) {
            s->ablate_emb_dir = (float *)malloc((size_t)hp->n_embd * sizeof(float));
            if (!s->ablate_emb_dir) {
                snprintf(err, err_bytes, "out of memory allocating embedding ablation vector");
                free(ablate_host);
                ink_close(s); return 1;
            }
            memcpy(s->ablate_emb_dir, ablate_host, (size_t)hp->n_embd * sizeof(float));
        }
    }
    s->embd_is_f32 = (s->gguf.tok_embd.type != INK_GGML_Q8_0 || s->ablate_has_emb) ? 1 : 0;
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
    if (hp->n_expert > 128 || hp->n_expert_used > 16) {
        snprintf(err, err_bytes, "this engine routes among at most 128 experts and 16 chosen, not %lld and %lld",
                 (long long)hp->n_expert, (long long)hp->n_expert_used);
        ink_close(s); return 1;
    }
    if (hp->gate_rank != 64) {
        snprintf(err, err_bytes, "this engine handles a recurrence gate of rank 64, not %lld", (long long)hp->gate_rank);
        ink_close(s); return 1;
    }
    s->oracle_q8 = getenv("INK_ORACLE_Q8") != NULL;
    s->no_tables = getenv("INK_NO_TABLES") != NULL;
    s->no_mma    = getenv("INK_NO_MMA") != NULL;
    s->no_graph  = getenv("INK_NO_GRAPH") != NULL;

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
    s->fast_q4 = (env_on("INK_FAST_Q4") && !s->oracle_q8);
    size_t fast_q4_orig_attn = 0, fast_q4_new_attn = 0;
    for (int ip = 0; ip < hp->n_phys; ip++) {
        const InkLayerTensors *L = &s->gguf.layers[ip];
        const InkTensor *t = (const InkTensor *)L;
        for (int i = 0; i < INK_LAYER_TENSORS; i++) {
            if (!t[i].present || is_expert_tensor(L, &t[i])) continue;
            int force_f32 = is_loop_ctrl_tensor(L, &t[i]);
            uint64_t nb_dev = cuda_dev_tensor_bytes(&t[i], force_f32);
            if (!force_f32 && s->fast_q4 && is_fast_q4_attn_tensor(L, &t[i]) &&
                t[i].type == INK_GGML_Q8_0 && t[i].ndims == 2 &&
                (t[i].dims[0] % 32) == 0 && (t[i].nbytes % 34) == 0) {
                fast_q4_orig_attn += (size_t)t[i].nbytes;
                nb_dev = (t[i].nbytes / 34) * 18;
                fast_q4_new_attn += (size_t)nb_dev;
            }
            resident_bytes += (nb_dev + 255) & ~(uint64_t)255;
        }
        ExpertCache *c = &s->experts[ip];
        if (!L->ffn_gate_up_exps.present || !L->ffn_down_exps.present) {
            snprintf(err, err_bytes, "layer %d has no expert tensors", ip);
            free(ablate_host);
            ink_close(s); return 1;
        }
        c->src_gu_bytes = (size_t)(L->ffn_gate_up_exps.nbytes / (uint64_t)hp->n_expert);
        c->src_dn_bytes = (size_t)(L->ffn_down_exps.nbytes / (uint64_t)hp->n_expert);
        if (L->ffn_gate_up_exps.type == INK_GGML_Q8_0 || L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) {
            c->gu_type = L->ffn_gate_up_exps.type;
            c->gu_bytes = c->src_gu_bytes;
        } else {
            c->gu_type = INK_GGML_IQ4_NL;
            c->gu_bytes = (size_t)(2 * hp->n_ff_exp * (hp->n_embd / 32) * 18);
        }
        if (L->ffn_down_exps.type == INK_GGML_Q8_0 || L->ffn_down_exps.type == INK_GGML_IQ4_NL) {
            c->dn_type = L->ffn_down_exps.type;
            c->dn_bytes = c->src_dn_bytes;
        } else {
            c->dn_type = INK_GGML_IQ4_NL;
            c->dn_bytes = (size_t)(hp->n_embd * (hp->n_ff_exp / 32) * 18);
        }
        expert_bytes += (c->gu_bytes + c->dn_bytes) * (size_t)hp->n_expert;
        smallest_cache += (c->gu_bytes + c->dn_bytes) * INK_MIN_EXPERT_SLOTS;
    }
    size_t out_nbytes_dev = (size_t)cuda_dev_tensor_bytes(&s->gguf.output, 0);
    s->output_iq4 = (!s->oracle_q8 && s->gguf.output.type == INK_GGML_Q8_0 && (s->gguf.output.nbytes % 34) == 0 &&
                     (s->fast_q4 || env_on("INK_Q4_LM_HEAD") ||
                      (info.free_bytes < ((size_t)18432 << 20) && s->max_ctx > 8192 && !env_on("INK_EXACT_LM_HEAD"))));
    if (s->output_iq4) out_nbytes_dev = (size_t)(s->gguf.output.nbytes / 34) * 18;
    resident_bytes += ((cuda_dev_tensor_bytes(&s->gguf.out_res_query, 1) + 255) & ~(uint64_t)255) +
                      ((cuda_dev_tensor_bytes(&s->gguf.out_norm, 1) + 255) & ~(uint64_t)255) +
                      ((out_nbytes_dev + 255) & ~(uint64_t)255) +
                      ((cuda_dev_tensor_bytes(&s->gguf.tok_norm, 1) + 255) & ~(uint64_t)255);

    size_t free_b = info.free_bytes, total_b = info.total_bytes;
    int64_t test_mb = env_mb("INK_VRAM_MB"); /* plan as if this much VRAM were free */
    if (test_mb > 0) { free_b = (size_t)test_mb << 20; if (total_b < free_b) total_b = free_b; }
    s->use_online_attn = !s->no_tables && hp->head_dim == 128 && hp->rel_len == 16;
    /* Choose prefill batch size based on VRAM headroom: online-softmax attention
       eliminates attn_scores, so batch=128 fits comfortably at both 4K and 32K
       on >=12 GiB GPUs (64 on 8..12 GiB, 16 on <8 GiB). */
    int plan_batch = INK_MAX_BATCH;
    if (free_b < ((size_t)8192 << 20)) plan_batch = 16;
    else if (free_b < ((size_t)18432 << 20)) plan_batch = 64;
    if (!s->use_online_attn && s->max_ctx > 8192 && plan_batch > 32) plan_batch = 32;
    if (s->no_mma || s->oracle_q8 || s->no_tables) { if (plan_batch > 32) plan_batch = 32; }

    int full_slots = 0, swa_slots = 0;
    size_t kv_bytes = 0, kv_full_per_token = 0, kv_fixed = 0;
    size_t kv_bytes_fp16 = 0;
    s->kv_fp8 = (getenv("INK_FP8_KV") != NULL && s->use_online_attn && !s->oracle_q8 && (n_kv % 2) == 0);
    /* First compute FP16 KV footprint; if 128/128 IQ4 experts don't fit at long context (e.g. 16K..32K on 16 GB RTX 4080),
       auto-enable FP8 E4M3 KV (opt-out via INK_FP16_KV=1) to keep 128/128 experts + verify_graph + DSpark resident! */
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : (int)(hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start));
        int ring = (int)hp->swa + plan_batch;
        int cap = hp->is_swa[ip] ? (ring < s->max_ctx ? ring : s->max_ctx) : s->max_ctx;
        size_t conv = (2 * n_kv + 2 * d) * 3 * 4;
        kv_bytes_fp16 += (size_t)cap * n_kv * 4 + conv;
    }
    if (!s->kv_fp8 && s->use_online_attn && !s->oracle_q8 && (n_kv % 2) == 0 && getenv("INK_FP16_KV") == NULL) {
        size_t q4_exp_est = 0;
        for (int ip = 0; ip < hp->n_phys; ip++) {
            ExpertCache *c = &s->experts[ip];
            size_t gb = (c->gu_type == INK_GGML_Q8_0 && (c->src_gu_bytes % 34) == 0 && getenv("INK_EXACT_Q8_EXPS") == NULL)
                        ? (c->src_gu_bytes / 34) * 18 : c->gu_bytes;
            size_t db = (c->dn_type == INK_GGML_Q8_0 && (c->src_dn_bytes % 34) == 0 && getenv("INK_EXACT_Q8_EXPS") == NULL)
                        ? (c->src_dn_bytes / 34) * 18 : c->dn_bytes;
            q4_exp_est += (gb + db) * (size_t)hp->n_expert;
        }
        size_t est_fixed_fp16 = resident_bytes + kv_bytes_fp16 + ((size_t)70 << 20) + ((size_t)INK_VRAM_MARGIN_MB << 20);
        if (free_b < est_fixed_fp16 + q4_exp_est) {
            s->kv_fp8 = 1;
            if (free_b < ((size_t)18432 << 20) && s->max_ctx > 8192 && plan_batch > 32) plan_batch = 32;
        }
    }
    size_t kv_elem_pair = s->kv_fp8 ? 2 : 4; /* K+V bytes per element (2B FP8 or 4B FP16) */
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : (int)(hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start));
        /* A window slot is a ring. It holds the window and a whole batch, so
           storing a batch never overwrites a position the batch still reads. */
        int ring = (int)hp->swa + plan_batch;
        int cap = hp->is_swa[ip] ? (ring < s->max_ctx ? ring : s->max_ctx) : s->max_ctx;
        s->kv_cap[il] = cap;
        size_t conv = (2 * n_kv + 2 * d) * 3 * 4;
        kv_bytes += (size_t)cap * n_kv * kv_elem_pair + conv;
        if (hp->is_swa[ip]) { swa_slots++; kv_fixed += (size_t)ring * n_kv * kv_elem_pair + conv; }
        else { full_slots++; kv_full_per_token += n_kv * kv_elem_pair; kv_fixed += conv; }
    }
    size_t attn_score_floats = s->use_online_attn
        ? (s->kv_fp8 ? 64 : (size_t)hp->n_head * (size_t)s->max_ctx)
        : (size_t)plan_batch * (size_t)hp->n_head * (size_t)s->max_ctx;
    size_t spec_scratch_bytes = (!s->no_mma && !s->oracle_q8 && !s->no_tables && !s->no_graph &&
                                 free_b >= ((size_t)12288 << 20))
        ? ((size_t)s->n_slots * 11 * (2 * n_kv + 2 * d) * 4 + 8 * (size_t)hp->n_vocab * 4 + ((size_t)3 << 20))
        : ((size_t)2 << 20);
    size_t scratch_bytes = ((size_t)hp->n_vocab + attn_score_floats) * 4 + ((size_t)8 << 20) + spec_scratch_bytes +
                           (size_t)plan_batch * 4 *
                           (44 * d + 3 * n_q + 4 * n_kv +
                            (size_t)hp->n_expert_used * (5 * (size_t)hp->n_ff_exp / 2 + d) + 1024);

    size_t margin = (size_t)INK_VRAM_MARGIN_MB << 20;
    if (s->kv_fp8 && free_b < resident_bytes + kv_bytes + scratch_bytes + margin + ((size_t)13122 << 20)) {
        margin = (size_t)96 << 20;
    }
    size_t fixed = resident_bytes + kv_bytes + scratch_bytes + margin;
    int64_t forced_slots = env_mb("INK_EXPERT_SLOTS"); /* cap every layer's cache, to exercise streaming */

    /* The plan of psionic/lib/inkling-budget.coax (inkling_vram_plan), in the
       same integer arithmetic: one slot is one expert in every layer.
       On 16 GB GPUs (e.g. RTX 4080), quantizing the 4 Q8_0 coda MoE expert layers
       to IQ4_NL on-load saves ~1.90 GB so all 128/128 experts fit 100% resident in
       VRAM (zero PCIe streaming + single decode CUDA Graph; opt-out via INK_EXACT_Q8_EXPS=1). */
    size_t expert_budget = free_b > fixed ? free_b - fixed : 0;
    int q8_exps_quantized = 0;
    size_t orig_expert_bytes = expert_bytes;
    if (expert_budget < expert_bytes && forced_slots <= 0 && !s->oracle_q8 &&
        getenv("INK_EXACT_Q8_EXPS") == NULL) {
        size_t q4_expert_bytes = 0;
        int n_q8_layers = 0;
        for (int ip = 0; ip < hp->n_phys; ip++) {
            ExpertCache *c = &s->experts[ip];
            size_t gb = (c->gu_type == INK_GGML_Q8_0 && (c->src_gu_bytes % 34) == 0) ? (c->src_gu_bytes / 34) * 18 : c->gu_bytes;
            size_t db = (c->dn_type == INK_GGML_Q8_0 && (c->src_dn_bytes % 34) == 0) ? (c->src_dn_bytes / 34) * 18 : c->dn_bytes;
            if (gb != c->gu_bytes || db != c->dn_bytes) n_q8_layers++;
            q4_expert_bytes += (gb + db) * (size_t)hp->n_expert;
        }
        if (n_q8_layers > 0 && q4_expert_bytes <= expert_budget) {
            expert_bytes = 0;
            smallest_cache = 0;
            for (int ip = 0; ip < hp->n_phys; ip++) {
                ExpertCache *c = &s->experts[ip];
                if (c->gu_type == INK_GGML_Q8_0 && (c->src_gu_bytes % 34) == 0) {
                    c->gu_type = INK_GGML_IQ4_NL;
                    c->gu_bytes = (c->src_gu_bytes / 34) * 18;
                }
                if (c->dn_type == INK_GGML_Q8_0 && (c->src_dn_bytes % 34) == 0) {
                    c->dn_type = INK_GGML_IQ4_NL;
                    c->dn_bytes = (c->src_dn_bytes / 34) * 18;
                }
                expert_bytes += (c->gu_bytes + c->dn_bytes) * (size_t)hp->n_expert;
                smallest_cache += (c->gu_bytes + c->dn_bytes) * INK_MIN_EXPERT_SLOTS;
            }
            q8_exps_quantized = n_q8_layers;
        }
    }
    size_t slot_bytes = expert_bytes / (size_t)hp->n_expert;
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
    /* With Tensor Core grouped-expert streaming, a batch can exceed slots / n_expert_used
       because misses run in chunks of at most slots experts. Legacy single-pass streaming
       still requires every token's experts in the cache at once. */
    if (!s->no_mma && !s->oracle_q8 && !s->no_tables && info.major >= 8) {
        s->batch = plan_batch;
    } else {
        s->no_mma = 1;
        s->batch = slots / (int)hp->n_expert_used;
        if (s->batch > plan_batch) s->batch = plan_batch;
    }
    int64_t forced_batch = env_mb("INK_BATCH"); /* a smaller prefill batch, for tests */
    if (forced_batch > 0 && forced_batch < s->batch) s->batch = (int)forced_batch;
    if (s->fast_q4 && fast_q4_orig_attn > 0)
        fprintf(stderr, "psionic-cuda: INK_FAST_Q4=1 quantized Q8_0 attn projections -> IQ4_NL (%lld -> %lld MB)\n",
                (long long)to_mb(fast_q4_orig_attn), (long long)to_mb(fast_q4_new_attn));
    if (s->output_iq4)
        fprintf(stderr, "psionic-cuda: quantized lm_head Q8_0 -> IQ4_NL on-load (%lld -> %lld MB; INK_EXACT_LM_HEAD=1 for Q8_0)\n",
                (long long)to_mb((size_t)s->gguf.output.nbytes), (long long)to_mb(out_nbytes_dev));
    if (s->kv_fp8)
        fprintf(stderr, "psionic-cuda: enabled FP8 E4M3 KV cache (%lld -> %lld MB, ctx=%d; INK_FP16_KV=1 for FP16 KV)\n",
                (long long)to_mb(kv_bytes_fp16), (long long)to_mb(kv_bytes), s->max_ctx);
    if (q8_exps_quantized > 0)
        fprintf(stderr, "psionic-cuda: quantized %d Q8_0 MoE expert layers -> IQ4_NL on-load (%lld -> %lld MB, %d/%lld resident in VRAM; INK_EXACT_Q8_EXPS=1 for streaming Q8_0)\n",
                q8_exps_quantized, (long long)to_mb(orig_expert_bytes), (long long)to_mb(expert_bytes),
                slots, (long long)hp->n_expert);
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
    s->stage_bytes = (s->kv_fp8 && !s->streaming) ? ((size_t)8 << 20) : ((size_t)32 << 20);
    for (int ip = 0; ip < hp->n_phys; ip++)
        if (s->experts[ip].src_gu_bytes > s->stage_bytes) s->stage_bytes = s->experts[ip].src_gu_bytes;
    if (pc_alloc(s->stage_bytes, &s->stage) != PC_OK) {
        snprintf(err, err_bytes, "out of VRAM for the staging buffer: %s", pc_log());
        ink_close(s); return 1;
    }
    size_t at = 0;
#define PLACE_EX(tensor, dev, force_f32) do { \
        (dev) = s->weights + at; \
        if (((force_f32) || (tensor)->ndims == 1) && (tensor)->type != INK_GGML_F32) { \
            float *_f32 = ggml_dequant_tensor_f32((tensor), s->gguf.map + (tensor)->offset); \
            if (!_f32) { snprintf(err, err_bytes, "out of memory dequantizing tensor to f32"); free(ablate_host); ink_close(s); return 1; } \
            uint64_t _nb = cuda_dev_tensor_bytes((tensor), 1); \
            int _rc = upload_big(s, (dev), (const uint8_t *)_f32, (size_t)_nb); \
            free(_f32); \
            if (_rc) { snprintf(err, err_bytes, "%s", s->error); free(ablate_host); ink_close(s); return 1; } \
            (tensor)->type = INK_GGML_F32; \
            (tensor)->nbytes = _nb; \
            at += ((size_t)_nb + 255) & ~(size_t)255; \
        } else if ((tensor)->type != INK_GGML_F32 && (tensor)->type != INK_GGML_Q8_0 && (tensor)->type != INK_GGML_IQ4_NL) { \
            if ((tensor)->ndims == 2 && ((tensor)->dims[0] % 32) == 0) { \
                uint8_t *_q8 = ggml_repack_to_q8_0((tensor)->type, s->gguf.map + (tensor)->offset, (int64_t)(tensor)->dims[1], (int64_t)(tensor)->dims[0]); \
                if (!_q8) { snprintf(err, err_bytes, "out of memory repacking tensor to q8_0"); free(ablate_host); ink_close(s); return 1; } \
                int64_t _blks = (int64_t)(tensor)->dims[0] / 32; \
                int _rc = relayout(s, _q8, (dev), (int64_t)(tensor)->dims[1], _blks, 8, 0); \
                free(_q8); \
                if (_rc) { snprintf(err, err_bytes, "%s", s->error); free(ablate_host); ink_close(s); return 1; } \
                (tensor)->type = INK_GGML_Q8_0; \
                (tensor)->nbytes = (tensor)->dims[1] * (uint64_t)_blks * 34; \
                at += ((size_t)(tensor)->nbytes + 255) & ~(size_t)255; \
            } else { \
                float *_f32 = ggml_dequant_tensor_f32((tensor), s->gguf.map + (tensor)->offset); \
                if (!_f32) { snprintf(err, err_bytes, "out of memory dequantizing tensor"); free(ablate_host); ink_close(s); return 1; } \
                uint64_t _nb = cuda_dev_tensor_bytes((tensor), 1); \
                int _rc = upload_big(s, (dev), (const uint8_t *)_f32, (size_t)_nb); \
                free(_f32); \
                if (_rc) { snprintf(err, err_bytes, "%s", s->error); free(ablate_host); ink_close(s); return 1; } \
                (tensor)->type = INK_GGML_F32; \
                (tensor)->nbytes = _nb; \
                at += ((size_t)_nb + 255) & ~(size_t)255; \
            } \
        } else { \
            int quant_ = (tensor)->type == INK_GGML_Q8_0 || (tensor)->type == INK_GGML_IQ4_NL; \
            int64_t blocks_ = (int64_t)(tensor)->dims[0] / 32, words_ = quant_words((tensor)->type); \
            if (quant_ && ((tensor)->ndims != 2 || (tensor)->dims[0] % 32 != 0 || \
                           (tensor)->nbytes != (tensor)->dims[1] * (uint64_t)(blocks_ * (4 * words_ + 2)))) { \
                snprintf(err, err_bytes, "a quantized tensor is not a matrix of whole blocks"); free(ablate_host); ink_close(s); return 1; \
            } \
            if (quant_ ? relayout(s, s->gguf.map + (tensor)->offset, (dev), (int64_t)(tensor)->dims[1], blocks_, words_, 0) \
                       : upload_big(s, (dev), s->gguf.map + (tensor)->offset, (size_t)(tensor)->nbytes)) { snprintf(err, err_bytes, "%s", s->error); free(ablate_host); ink_close(s); return 1; } \
            at += ((size_t)(tensor)->nbytes + 255) & ~(size_t)255; \
        } \
    } while (0)
#define PLACE(tensor, dev) PLACE_EX((tensor), (dev), 0)
    for (int ip = 0; ip < hp->n_phys; ip++) {
        InkLayerTensors *L = &s->gguf.layers[ip];
        InkTensor *t = (InkTensor *)L;
        for (int i = 0; i < INK_LAYER_TENSORS; i++) {
            if (!t[i].present || is_expert_tensor(L, &t[i])) continue;
            int force_f32 = is_loop_ctrl_tensor(L, &t[i]);
            if (!force_f32 && s->fast_q4 && is_fast_q4_attn_tensor(L, &t[i]) &&
                t[i].type == INK_GGML_Q8_0 && t[i].ndims == 2 &&
                (t[i].dims[0] % 32) == 0 && (t[i].nbytes % 34) == 0) {
                s->layer_dev[ip][i] = s->weights + at;
                if (relayout_q8_to_iq4(s, s->gguf.map + t[i].offset, s->layer_dev[ip][i],
                                       (int64_t)t[i].dims[1], (int64_t)t[i].dims[0] / 32, 0)) {
                    snprintf(err, err_bytes, "%s", s->error); free(ablate_host); ink_close(s); return 1;
                }
                t[i].type = INK_GGML_IQ4_NL;
                t[i].nbytes = (t[i].nbytes / 34) * 18;
                at += ((size_t)t[i].nbytes + 255) & ~(size_t)255;
            } else {
                PLACE_EX(&t[i], s->layer_dev[ip][i], force_f32);
            }
        }
    }
    PLACE_EX(&s->gguf.out_res_query, s->out_res_query_dev, 1);
    PLACE_EX(&s->gguf.out_norm, s->out_norm_dev, 1);
    if (s->output_iq4) {
        s->output_dev = s->weights + at;
        if (relayout_q8_to_iq4(s, s->gguf.map + s->gguf.output.offset, s->output_dev,
                               (int64_t)s->gguf.output.dims[1], (int64_t)s->gguf.output.dims[0] / 32, 0)) {
            snprintf(err, err_bytes, "%s", s->error); free(ablate_host); ink_close(s); return 1;
        }
        at += (out_nbytes_dev + 255) & ~(size_t)255;
    } else {
        PLACE(&s->gguf.output, s->output_dev);
    }
    PLACE_EX(&s->gguf.tok_norm, s->tok_norm_dev, 1);
#undef PLACE
#undef PLACE_EX

    if (s->has_ablate && ablate_host) {
        size_t ab_bytes = (size_t)(1 + hp->n_phys) * d * sizeof(float);
        if (pc_alloc(ab_bytes, &s->ablate_dirs) != PC_OK ||
            pc_upload(s->ablate_dirs, ablate_host, ab_bytes, s->stream) != PC_OK) {
            snprintf(err, err_bytes, "upload ablation directions: %s", pc_log());
            free(ablate_host);
            ink_close(s); return 1;
        }
    }
    free(ablate_host);
    ablate_host = NULL;

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
    size_t row_bytes = s->embd_is_f32 ? (d * 4) : (size_t)(s->gguf.tok_embd.nbytes / (uint64_t)hp->n_vocab);
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
        alloc_zero(s, &s->attn_scores, (s->use_online_attn ? (s->kv_fp8 ? 64 : ((size_t)hp->n_head * (size_t)s->max_ctx)) : (nb * (size_t)hp->n_head * (size_t)s->max_ctx)) * 4) ||
        alloc_zero(s, &s->attn_sk_o, (size_t)8 * (size_t)hp->n_head * 16 * (size_t)hp->head_dim * 4) ||
        alloc_zero(s, &s->attn_sk_ms, (size_t)8 * (size_t)hp->n_head * 16 * 2 * 4) ||
        alloc_zero(s, &s->ctrl_inv, 256) ||
        alloc_zero(s, &s->attn_out, nb * n_q * 4) ||
        alloc_zero(s, &s->o_raw, nb * d * 4) ||
        alloc_zero(s, &s->norm_buf, nb * d * 4) ||
        alloc_zero(s, &s->router_logits, nb * (size_t)hp->n_expert * 4) ||
        alloc_zero(s, &s->moe_ids, nb * n_used * 4) ||
        alloc_zero(s, &s->moe_slots, nb * n_used * 4) ||
        alloc_zero(s, &s->moe_wts, nb * n_used * 4) ||
        alloc_zero(s, &s->moe_hid, ((!s->no_mma && !s->oracle_q8 && !s->no_tables) ? 1 : nb) * n_used * (size_t)hp->n_ff_exp * 4) ||
        alloc_zero(s, &s->moe_out, ((!s->no_mma && !s->oracle_q8 && !s->no_tables) ? 1 : nb) * d * 4) ||
        alloc_zero(s, &s->x_bf16, (s->x_bf16_words = nb * (3 * d > n_q ? 3 * d : n_q)) * 4) ||
        alloc_zero(s, &s->moe_gu_out, nb * n_used * 2 * (size_t)hp->n_ff_exp * 4) ||
        alloc_zero(s, &s->moe_hid_bf16, nb * n_used * ((size_t)hp->n_ff_exp / 2) * 4) ||
        alloc_zero(s, &s->moe_down_out, nb * n_used * d * 4) ||
        alloc_zero(s, &s->moe_tasks, (s->moe_tasks_words = 5 * (INK_MAX_BATCH * 16 + INK_MAX_EXPERTS)) * 4) ||
        alloc_zero(s, &s->mv_part, (s->mv_part_floats = (n_q > d ? n_q : d) * ((n_q > d ? n_q : d) / 32) + proj_rows * (d / 32)) * 4) ||
        alloc_zero(s, &s->proj, (s->proj_floats = nb * proj_rows) * 4) ||
        alloc_zero(s, &s->proj_desc, 32 * 2 * 80 * 4) ||
        alloc_zero(s, &s->gu_part, n_used * (d / 32) * 2 * (size_t)hp->n_ff_exp * 4) ||
        alloc_zero(s, &s->dn_part, n_used * ((size_t)hp->n_ff_exp / 32) * d * 4) ||
        alloc_zero(s, &s->logits, (size_t)hp->n_vocab * 4) ||
        alloc_zero(s, &s->argmax_vals, 64 * 4) ||
        alloc_zero(s, &s->argmax_ids, 64 * 4) ||
        alloc_zero(s, &s->argmax_out, 4) ||
        alloc_zero(s, &s->topk_cnt, 256 * 4) ||
        alloc_zero(s, &s->topk_ids, 256 * 16 * 4) ||
        alloc_zero(s, &s->topk_vals, 256 * 16 * 4) ||
        alloc_zero(s, &s->dev_pos, 4) ||
        alloc_zero(s, &s->oracle_x, s->oracle_q8 ? nb * widest * 4 : 256) ||
        alloc_zero(s, &s->dummy_f32, 64 * 4)) {
        snprintf(err, err_bytes, "%s", s->error);
        ink_close(s); return 1;
    }
    size_t conv_slot_elems = (2 * n_kv + 2 * d) * 3;
    s->conv_all_elems = (size_t)s->n_slots * conv_slot_elems;
    if (alloc_zero(s, &s->conv_all, s->conv_all_elems * 4)) {
        snprintf(err, err_bytes, "%s", s->error);
        ink_close(s); return 1;
    }
    size_t kv_b_per_tok = s->kv_fp8 ? n_kv : (n_kv * 2);
    for (int il = 0; il < s->n_slots; il++) {
        size_t cap = (size_t)s->kv_cap[il];
        if (alloc_zero(s, &s->kv_k[il], cap * kv_b_per_tok) ||
            alloc_zero(s, &s->kv_v[il], cap * kv_b_per_tok)) {
            snprintf(err, err_bytes, "%s", s->error);
            ink_close(s); return 1;
        }
        PcBuffer cb = s->conv_all + (uint64_t)((size_t)il * conv_slot_elems * 4);
        s->conv_k[il]  = cb;
        s->conv_v[il]  = cb + (uint64_t)(n_kv * 3 * 4);
        s->conv_ao[il] = cb + (uint64_t)(2 * n_kv * 3 * 4);
        s->conv_fo[il] = cb + (uint64_t)((2 * n_kv + d) * 3 * 4);
    }
    s->can_verify_nb = (!s->streaming && !s->no_mma && !s->oracle_q8 && !s->no_tables && !s->no_graph &&
                        s->batch >= 8 && hp->n_embd % 32 == 0 && hp->n_ff_exp % 32 == 0 &&
                        hp->n_expert <= 128 && hp->n_expert_used <= 8);
    if (s->can_verify_nb) {
        s->spec_raw_log_elems = (size_t)s->n_slots * 8 * (2 * n_kv + 2 * d);
        if (alloc_zero(s, &s->spec_conv_bak, s->conv_all_elems * 4) ||
            alloc_zero(s, &s->spec_raw_log, s->spec_raw_log_elems * 4) ||
            alloc_zero(s, &s->spec_logits, 8 * (size_t)hp->n_vocab * 4) ||
            alloc_zero(s, &s->spec_hidden, 8 * d * 4) ||
            alloc_zero(s, &s->spec_argmax_out, 8 * 4) ||
            alloc_zero(s, &s->spec_topk_cnt, 8 * 256 * 4) ||
            alloc_zero(s, &s->spec_topk_ids, 8 * 256 * 16 * 4) ||
            alloc_zero(s, &s->spec_topk_vals, 8 * 256 * 16 * 4)) {
            s->can_verify_nb = 0;
            s->error[0] = '\0';
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
            uint32_t want_type = L->attn_q.type;
            int is_iq4 = (want_type == INK_GGML_IQ4_NL);
            int n = 0, ok = (want_type == INK_GGML_Q8_0 || want_type == INK_GGML_IQ4_NL);
            uint64_t q_bytes_per_block = is_iq4 ? 16 : 32;
            int64_t items = 0, rows_all = 0, rtiles_all = 0, blocks = (int64_t)d / 32;
            for (int m = 0; m < 8 && ok; m++) {
                const InkTensor *t = mats[m].t;
                if (!mats[m].use) continue;
                if (!t->present || t->type != want_type || t->ndims != 2 || t->dims[0] != d || t->dims[1] % 4 != 0) { ok = 0; break; }
                int64_t rows = (int64_t)t->dims[1];
                PcBuffer dev = tensor_dev(s, t);
                uint32_t *e = desc + n * 10;
                e[0] = (uint32_t)((dev - s->weights) / 4);
                e[1] = (uint32_t)((dev - s->weights + (uint64_t)(rows * blocks * q_bytes_per_block)) / 2);
                e[2] = (uint32_t)rows;
                e[3] = (uint32_t)rows;
                e[4] = (uint32_t)(variant ? (size_t)mats[m].sel * nb * d : 0);
                e[5] = (uint32_t)((mats[m].out - s->proj) / 4);
                e[6] = (uint32_t)(rows_all * blocks);
                items += blocks * (rows / 4);
                rows_all += rows;
                rtiles_all += (rows + 15) / 16;
                e[7] = (uint32_t)items;
                e[8] = (uint32_t)rows_all;
                e[9] = (uint32_t)rtiles_all;
                n++;
            }
            if (!ok || (size_t)(rows_all * blocks) > s->mv_part_floats) n = 0;
            s->proj_list[ip][variant].n_mats = n;
            s->proj_list[ip][variant].is_iq4 = is_iq4;
            s->proj_list[ip][variant].items = items;
            s->proj_list[ip][variant].rows = rows_all;
            s->proj_list[ip][variant].rtiles = rtiles_all;
            if (pc_upload(s->proj_desc + (uint64_t)(ip * 2 + variant) * 80 * 4, desc, sizeof desc, s->stream) != PC_OK ||
                pc_sync_stream(s->stream) != PC_OK) {
                snprintf(err, err_bytes, "upload the projection lists: %s", pc_log());
                ink_close(s); return 1;
            }
        }
    }
    s->swa_cap_min = s->max_ctx;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : (int)(hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start));
        if (hp->is_swa[ip] && s->kv_cap[il] < s->swa_cap_min) s->swa_cap_min = s->kv_cap[il];
    }
    size_t zeros = (n_kv > d ? n_kv : d) * 3;
    s->logits_host = (float *)calloc((size_t)hp->n_vocab, sizeof(float));
    s->hidden_host = (float *)calloc(d, sizeof(float));
    s->zeros = (float *)calloc(zeros, sizeof(float));
    s->moe_tasks_host = (uint32_t *)calloc(s->moe_tasks_words, sizeof(uint32_t));
    s->tok_hist = (int32_t *)calloc((size_t)s->max_ctx, sizeof(int32_t));
    s->live_tokens = (int32_t *)calloc((size_t)s->max_ctx, sizeof(int32_t));
    if (!s->logits_host || !s->hidden_host || !s->zeros || !s->moe_tasks_host || !s->tok_hist || !s->live_tokens) {
        snprintf(err, err_bytes, "out of memory");
        ink_close(s); return 1;
    }
    if (pc_sync_stream(s->stream) != PC_OK) {
        snprintf(err, err_bytes, "loading weights: %s", pc_log());
        ink_close(s); return 1;
    }
    memset(s->prof_seconds, 0, sizeof s->prof_seconds);
    memset(s->prof_calls, 0, sizeof s->prof_calls);
    s->pos = 0;
    s->live_len = 0;
    s->swa_valid_from = 0;
    s->dspark_live_len = 0;
    s->dspark_swa_valid_from = 0;
    s->spec_active_idx = -1;
    s->pld_ema2 = 0.25f;
    s->pld_ema3 = 0.50f;
    ink_dspark_open(s, gguf_path);
    if (!s->streaming) { free_buf(&s->stage); s->stage_bytes = 0; }
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
        for (int i = 0; i < 2; i++) {
            if (s->decode_graph[i]) { pc_graph_free(s->decode_graph[i]); s->decode_graph[i] = NULL; }
            if (s->stream_tail[i]) { pc_graph_free(s->stream_tail[i]); s->stream_tail[i] = NULL; }
        }
        for (int i = 0; i < 9; i++) {
            if (s->verify_graph[i]) { pc_graph_free(s->verify_graph[i]); s->verify_graph[i] = NULL; }
        }
        for (int il = 0; il < INK_MAX_SLOTS; il++) {
            if (s->stream_seg[il]) { pc_graph_free(s->stream_seg[il]); s->stream_seg[il] = NULL; }
            free_buf(&s->kv_k[il]); free_buf(&s->kv_v[il]);
        }
        for (int ip = 0; ip < 32; ip++) { free_buf(&s->experts[ip].gu); free_buf(&s->experts[ip].dn); }
        PcBuffer *scratch[] = {
            &s->weights, &s->stage, &s->embd_row, &s->cur, &s->prelude, &s->bank_out,
            &s->blocks, &s->prelude_blk, &s->bank_blk, &s->messages,
            &s->inp3, &s->inp3b, &s->norm_b2, &s->ctrl_gate,
            &s->proj, &s->proj_desc, &s->k_buf, &s->v_buf,
            &s->attn_scores, &s->attn_sk_o, &s->attn_sk_ms, &s->ctrl_inv, &s->attn_out, &s->o_raw, &s->norm_buf, &s->router_logits,
            &s->moe_ids, &s->moe_slots, &s->moe_wts, &s->moe_hid, &s->moe_out,
            &s->x_bf16, &s->moe_gu_out, &s->moe_hid_bf16, &s->moe_down_out, &s->moe_tasks,
            &s->gu_part, &s->dn_part, &s->mv_part, &s->logits,
            &s->argmax_vals, &s->argmax_ids, &s->argmax_out,
            &s->topk_cnt, &s->topk_ids, &s->topk_vals,
            &s->conv_all, &s->spec_conv_bak, &s->spec_raw_log,
            &s->spec_logits, &s->spec_hidden, &s->spec_argmax_out,
            &s->spec_topk_cnt, &s->spec_topk_ids, &s->spec_topk_vals,
            &s->dev_pos, &s->dummy_f32, &s->oracle_x, &s->ablate_dirs,
        };
        ink_dspark_close(s);
        for (size_t i = 0; i < sizeof scratch / sizeof *scratch; i++) free_buf(scratch[i]);
        if (s->module) pc_unload(s->module);
        if (s->stream) pc_destroy_stream(s->stream);
    }
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        free(s->snaps[i].tokens); free(s->snaps[i].conv_state);
        free(s->snaps[i].logits); free(s->snaps[i].swa_kv);
        free(s->snaps[i].dspark_last_taps); free(s->snaps[i].dspark_swa_kv);
    }
    free(s->ablate_emb_dir);
    free(s->logits_host); free(s->hidden_host); free(s->zeros); free(s->moe_tasks_host);
    free(s->tok_hist); free(s->live_tokens); free(s->pred_toks); free(s->sess_hist);
    ink_lora_close(&s->lora);
    ink_gguf_close(&s->gguf);
    free(s);
}

static int ink_flush_spec(InkSession *s);

static inline void ink_mark_written_cuda(InkSession *s, int p_end) {
    if (p_end > s->max_pos_written) s->max_pos_written = p_end;
    if (s->swa_cap_min > 0 && p_end - s->swa_cap_min > s->swa_valid_from) {
        s->swa_valid_from = p_end - s->swa_cap_min;
    }
}

void ink_reset(InkSession *s) {
    if (!s) return;
    if (s->conv_all && s->conv_all_elems > 0) {
        pc_fill(s->conv_all, 0, s->conv_all_elems, s->stream);
    }
    s->pos = 0;
    s->live_len = 0;
    s->max_pos_written = 0;
    s->swa_valid_from = 0;
    s->dspark_live_len = 0;
    s->dspark_swa_valid_from = 0;
    s->tok_hist_len = 0;
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;
    s->conv_pending_nb = 0;
    s->conv_pending_n = 0;
    s->argmax_valid = 0;
    s->logits_host_valid = 0;
    s->hidden_host_valid = 0;
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        s->snaps[i].valid = 0;
        s->snaps[i].pinned = 0;
        s->snaps[i].swa_win = 0;
        s->snaps[i].dspark_swa_win = 0;
    }
    if (s->dspark.ready) {
        s->dspark.pos = 0;
        s->dspark.pending_pos0 = 0;
        s->dspark.pending_nb = 0;
        s->dspark.pending_got = 0;
        s->dspark.skip_countdown = 0;
        s->dspark.skip_mult = 1;
        s->dspark.req_rounds = 0;
        s->dspark.req_disabled = 0;
        s->dspark.util_ema = 1.10f;
    }
}

/* The exact shortconv state after n_tokens, so a later request that starts
   with the same tokens resumes there. The KV cache stays on the device:
   positions before the snapshot are not rewritten until the ring wraps. */
void ink_set_prediction(InkSession *s, const int32_t *tokens, int n_tokens) {
    if (!s) return;
    if (!tokens || n_tokens <= 0) { s->pred_len = 0; return; }
    if (n_tokens > 16384) n_tokens = 16384;
    if (n_tokens > s->pred_cap) {
        int32_t *p = (int32_t *)realloc(s->pred_toks, (size_t)n_tokens * sizeof(int32_t));
        if (!p) { s->pred_len = 0; return; }
        s->pred_toks = p;
        s->pred_cap = n_tokens;
    }
    memcpy(s->pred_toks, tokens, (size_t)n_tokens * sizeof(int32_t));
    s->pred_len = n_tokens;
}

void ink_prefix_save(InkSession *s, const int32_t *tokens, int n_tokens, int save_logits) {
    if (!s || !tokens || n_tokens <= 0 || n_tokens != s->pos || n_tokens > s->max_ctx) return;
    if (ink_flush_spec(s) != 0) return;
    if (s->dspark.ready && s->dspark.pending_got > 0) ink_dspark_flush_pending(s);
    const InkHParams *hp = &s->gguf.hp;
    int is_pinned = (save_logits == 2);
    int has_lg = (save_logits == 1);

    /* Choose slot: reuse matching (pos, pinned), else free slot, else LRU non-pinned (or LRU pinned if is_pinned). */
    int slot = -1;
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        if (s->snaps[i].valid && s->snaps[i].pos == n_tokens && s->snaps[i].pinned == is_pinned) {
            slot = i;
            break;
        }
    }
    if (slot < 0) {
        for (int i = 0; i < INK_MAX_SNAPS; i++) {
            if (!s->snaps[i].valid) { slot = i; break; }
        }
    }
    if (slot < 0) {
        for (int i = 0; i < INK_MAX_SNAPS; i++) {
            if (!is_pinned && s->snaps[i].pinned) continue;
            if (slot < 0 || s->snaps[i].last_used < s->snaps[slot].last_used) slot = i;
        }
        if (slot < 0) slot = 0;
    }

    if (!s->snaps[slot].tokens) {
        s->snaps[slot].tokens = (int32_t *)malloc((size_t)s->max_ctx * sizeof(int32_t));
        s->snaps[slot].conv_state = (float *)malloc(s->conv_all_elems * sizeof(float));
        s->snaps[slot].logits = (float *)malloc((size_t)hp->n_vocab * sizeof(float));
        s->snaps[slot].dspark_last_taps = (float *)malloc((size_t)4 * 2304 * sizeof(float));
        if (!s->snaps[slot].tokens || !s->snaps[slot].conv_state || !s->snaps[slot].logits || !s->snaps[slot].dspark_last_taps) {
            s->snaps[slot].valid = 0;
            return;
        }
    }
    if (has_lg) {
        const float *lg = ink_logits(s);
        if (!lg) { s->snaps[slot].valid = 0; return; }
        memcpy(s->snaps[slot].logits, lg, (size_t)hp->n_vocab * sizeof(float));
    }
    memcpy(s->snaps[slot].tokens, tokens, (size_t)n_tokens * sizeof(int32_t));
    PcStatus st = pc_download(s->snaps[slot].conv_state, s->conv_all, s->conv_all_elems * 4, s->stream);
    int save_ds = (s->dspark.ready && s->dspark.pos == n_tokens);
    if (st == PC_OK && save_ds) {
        st = pc_download(s->snaps[slot].dspark_last_taps, s->dspark.last_taps, 4 * 2304 * 4, s->stream);
    }

    /* Optional host-RAM SWA KV window backup so pinned system/early checkpoints survive SWA ring wraps. */
    s->snaps[slot].swa_win = 0;
    s->snaps[slot].dspark_swa_win = 0;
    int swa_slots = 0;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : (int)(hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start));
        if (hp->is_swa[ip]) swa_slots++;
    }
    int n_swa_alloc = 0;
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        if (s->snaps[i].swa_kv) n_swa_alloc++;
    }
    int want_swa = (st == PC_OK && swa_slots > 0 && hp->swa > 0 &&
                    (is_pinned || s->snaps[slot].swa_kv != NULL || n_swa_alloc < INK_MAX_SWA_SNAPS));
    if (want_swa) {
        size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
        size_t kv_b_per_tok = s->kv_fp8 ? n_kv : (n_kv * 2);
        size_t swa_bytes = (size_t)swa_slots * (size_t)hp->swa * kv_b_per_tok * 2;
        if (!s->snaps[slot].swa_kv && (is_pinned || n_swa_alloc < INK_MAX_SWA_SNAPS)) {
            s->snaps[slot].swa_kv = (uint8_t *)malloc(swa_bytes);
        }
        if (s->snaps[slot].swa_kv) {
            int win = (n_tokens < (int)hp->swa) ? n_tokens : (int)hp->swa;
            int p0 = n_tokens - win;
            uint8_t *sd = s->snaps[slot].swa_kv;
            for (int il = 0; st == PC_OK && il < s->n_slots; il++) {
                int ip = (il < hp->n_phys) ? il : (int)(hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start));
                if (!hp->is_swa[ip]) continue;
                int cap = s->kv_cap[il];
                int r0 = p0 % cap;
                int first = (r0 + win <= cap) ? win : (cap - r0);
                int second = win - first;
                size_t b1 = (size_t)first * kv_b_per_tok;
                size_t b2 = (size_t)second * kv_b_per_tok;
                st = pc_download(sd, s->kv_k[il] + (uint64_t)((size_t)r0 * kv_b_per_tok), b1, s->stream);
                if (st == PC_OK && b2 > 0) st = pc_download(sd + b1, s->kv_k[il], b2, s->stream);
                sd += (size_t)win * kv_b_per_tok;
                if (st == PC_OK) st = pc_download(sd, s->kv_v[il] + (uint64_t)((size_t)r0 * kv_b_per_tok), b1, s->stream);
                if (st == PC_OK && b2 > 0) st = pc_download(sd + b1, s->kv_v[il], b2, s->stream);
                sd += (size_t)win * kv_b_per_tok;
            }
            if (st == PC_OK) s->snaps[slot].swa_win = win;
        }
        if (st == PC_OK && save_ds && s->snaps[slot].swa_kv) {
            size_t dkv_b = s->dspark.kv_fp8 ? 1024 : 2048;
            if (!s->snaps[slot].dspark_swa_kv) {
                s->snaps[slot].dspark_swa_kv = (uint8_t *)malloc((size_t)2 * 2 * 1024 * dkv_b);
            }
            if (s->snaps[slot].dspark_swa_kv) {
                int ds_win = (n_tokens < 1024) ? n_tokens : 1024;
                int ds_p0 = n_tokens - ds_win;
                uint8_t *sd = s->snaps[slot].dspark_swa_kv;
                for (int li = 0; st == PC_OK && li < 3; li++) {
                    const CudaDsparkLayer *L = &s->dspark.layers[li];
                    if (!L->is_swa) continue;
                    int cap = L->kv_cap;
                    int r0 = ds_p0 % cap;
                    int first = (r0 + ds_win <= cap) ? ds_win : (cap - r0);
                    int second = ds_win - first;
                    size_t b1 = (size_t)first * dkv_b;
                    size_t b2 = (size_t)second * dkv_b;
                    st = pc_download(sd, L->kv_k + (uint64_t)((size_t)r0 * dkv_b), b1, s->stream);
                    if (st == PC_OK && b2 > 0) st = pc_download(sd + b1, L->kv_k, b2, s->stream);
                    sd += (size_t)ds_win * dkv_b;
                    if (st == PC_OK) st = pc_download(sd, L->kv_v + (uint64_t)((size_t)r0 * dkv_b), b1, s->stream);
                    if (st == PC_OK && b2 > 0) st = pc_download(sd + b1, L->kv_v, b2, s->stream);
                    sd += (size_t)ds_win * dkv_b;
                }
                if (st == PC_OK) s->snaps[slot].dspark_swa_win = ds_win;
            }
        }
    }

    if (st == PC_OK) st = pc_sync_stream(s->stream);
    if (st != PC_OK) { s->snaps[slot].valid = 0; return; }
    s->snaps[slot].pos = n_tokens;
    s->snaps[slot].has_logits = has_lg ? 1 : 0;
    s->snaps[slot].has_dspark = save_ds;
    s->snaps[slot].pinned = is_pinned ? 1 : 0;
    s->snaps[slot].last_used = ++s->snap_clock;
    s->snaps[slot].valid = 1;
}

int ink_prefix_restore(InkSession *s, const int32_t *tokens, int n_tokens) {
    if (!s || !tokens || n_tokens <= 0) { ink_reset(s); return 0; }
    const InkHParams *hp = &s->gguf.hp;
    int best = -1;
    int best_need_swa = 0;
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        if (!s->snaps[i].valid) continue;
        int sp = s->snaps[i].pos;
        if (sp <= 0 || sp > n_tokens) continue;
        if (sp == n_tokens && !s->snaps[i].has_logits) continue;
        if (sp > s->live_len || !s->live_tokens) continue;
        if (memcmp(s->live_tokens, s->snaps[i].tokens, (size_t)sp * sizeof(int32_t)) != 0) continue;
        if (memcmp(s->snaps[i].tokens, tokens, (size_t)sp * sizeof(int32_t)) != 0) continue;
        int oldest_needed = (sp >= (int)hp->swa) ? (sp - (int)hp->swa + 1) : 0;
        int swa_still_live = (oldest_needed >= s->swa_valid_from);
        int need_win = (sp < (int)hp->swa) ? sp : (int)hp->swa;
        int has_swa_backup = (s->snaps[i].swa_kv != NULL && s->snaps[i].swa_win >= need_win);
        if (!swa_still_live && !has_swa_backup) continue;
        if (best < 0 || sp > s->snaps[best].pos) {
            best = i;
            best_need_swa = !swa_still_live;
        }
    }
    if (best < 0) { ink_reset(s); return 0; }
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;
    s->conv_pending_nb = 0;
    s->conv_pending_n = 0;
    int sp = s->snaps[best].pos;
    PcStatus st = pc_upload(s->conv_all, s->snaps[best].conv_state, s->conv_all_elems * 4, s->stream);
    if (st == PC_OK && best_need_swa && s->snaps[best].swa_kv) {
        size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
        size_t kv_b_per_tok = s->kv_fp8 ? n_kv : (n_kv * 2);
        int win = s->snaps[best].swa_win;
        int p0 = sp - win;
        const uint8_t *ss = s->snaps[best].swa_kv;
        for (int il = 0; st == PC_OK && il < s->n_slots; il++) {
            int ip = (il < hp->n_phys) ? il : (int)(hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start));
            if (!hp->is_swa[ip]) continue;
            int cap = s->kv_cap[il];
            int r0 = p0 % cap;
            int first = (r0 + win <= cap) ? win : (cap - r0);
            int second = win - first;
            size_t b1 = (size_t)first * kv_b_per_tok;
            size_t b2 = (size_t)second * kv_b_per_tok;
            st = pc_upload(s->kv_k[il] + (uint64_t)((size_t)r0 * kv_b_per_tok), ss, b1, s->stream);
            if (st == PC_OK && b2 > 0) st = pc_upload(s->kv_k[il], ss + b1, b2, s->stream);
            ss += (size_t)win * kv_b_per_tok;
            if (st == PC_OK) st = pc_upload(s->kv_v[il] + (uint64_t)((size_t)r0 * kv_b_per_tok), ss, b1, s->stream);
            if (st == PC_OK && b2 > 0) st = pc_upload(s->kv_v[il], ss + b1, b2, s->stream);
            ss += (size_t)win * kv_b_per_tok;
        }
        if (st == PC_OK) {
            s->swa_valid_from = p0;
            s->max_pos_written = sp;
        }
    }
    int ds_restored = 0;
    int ds_new_swa_valid_from = s->dspark_swa_valid_from;
    if (st == PC_OK && s->dspark.ready && s->snaps[best].has_dspark &&
        s->snaps[best].dspark_last_taps && sp <= s->dspark_live_len) {
        int ds_oldest = (sp > 1024) ? (sp - 1024) : 0;
        int ds_need_win = (sp < 1024) ? sp : 1024;
        int ds_swa_live = (ds_oldest >= s->dspark_swa_valid_from);
        int ds_has_backup = (s->snaps[best].dspark_swa_kv != NULL && s->snaps[best].dspark_swa_win >= ds_need_win);
        if (ds_swa_live || ds_has_backup) {
            if (!ds_swa_live) {
                size_t dkv_b = s->dspark.kv_fp8 ? 1024 : 2048;
                int ds_win = s->snaps[best].dspark_swa_win;
                int ds_p0 = sp - ds_win;
                const uint8_t *ss = s->snaps[best].dspark_swa_kv;
                for (int li = 0; st == PC_OK && li < 3; li++) {
                    const CudaDsparkLayer *L = &s->dspark.layers[li];
                    if (!L->is_swa) continue;
                    int cap = L->kv_cap;
                    int r0 = ds_p0 % cap;
                    int first = (r0 + ds_win <= cap) ? ds_win : (cap - r0);
                    int second = ds_win - first;
                    size_t b1 = (size_t)first * dkv_b;
                    size_t b2 = (size_t)second * dkv_b;
                    st = pc_upload(L->kv_k + (uint64_t)((size_t)r0 * dkv_b), ss, b1, s->stream);
                    if (st == PC_OK && b2 > 0) st = pc_upload(L->kv_k, ss + b1, b2, s->stream);
                    ss += (size_t)ds_win * dkv_b;
                    if (st == PC_OK) st = pc_upload(L->kv_v + (uint64_t)((size_t)r0 * dkv_b), ss, b1, s->stream);
                    if (st == PC_OK && b2 > 0) st = pc_upload(L->kv_v, ss + b1, b2, s->stream);
                    ss += (size_t)ds_win * dkv_b;
                }
                ds_new_swa_valid_from = ds_p0;
            }
            if (st == PC_OK) {
                st = pc_upload(s->dspark.last_taps, s->snaps[best].dspark_last_taps, 4 * 2304 * 4, s->stream);
                if (st == PC_OK) ds_restored = 1;
            }
        }
    }
    if (st == PC_OK) st = pc_sync_stream(s->stream);
    if (st != PC_OK) { ink_reset(s); return 0; }
    if (s->dspark.ready) {
        s->dspark.pos = ds_restored ? sp : -1;
        s->dspark_live_len = ds_restored ? sp : 0;
        s->dspark_swa_valid_from = ds_restored ? ds_new_swa_valid_from : 0;
        s->dspark.pending_pos0 = 0;
        s->dspark.pending_nb = 0;
        s->dspark.pending_got = 0;
        s->dspark.skip_countdown = 0;
        s->dspark.skip_mult = 1;
        s->dspark.req_rounds = 0;
        s->dspark.req_disabled = 0;
        s->dspark.util_ema = 1.10f;
    }
    if (sp == n_tokens && s->snaps[best].has_logits) {
        memcpy(s->logits_host, s->snaps[best].logits, (size_t)hp->n_vocab * sizeof(float));
        s->logits_host_valid = 1;
    } else {
        s->logits_host_valid = 0;
    }
    s->argmax_valid = 0;
    s->hidden_host_valid = 0;
    if (s->tok_hist && sp <= s->max_ctx) {
        memcpy(s->tok_hist, tokens, (size_t)sp * sizeof(int32_t));
        s->tok_hist_len = sp;
    }
    s->pos = sp;
    s->live_len = sp;
    s->snaps[best].last_used = ++s->snap_clock;
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        if (i != best && s->snaps[i].valid && s->snaps[i].pos > sp) s->snaps[i].valid = 0;
    }
    return sp;
}

static void abort_capture(InkSession *s) {
    if (!s->capturing) return;
    PcGraph tmp = NULL;
    pc_capture_end(s->stream, &tmp);
    if (tmp) pc_graph_free(tmp);
    s->capturing = 0;
    s->spec_recording = 0;
}

static int dispatch_output_logits(InkSession *s, int64_t np, int n_blocks) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd;
    int64_t stack = (int64_t)s->batch * d;
    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
    int64_t last = (np - 1) * d;
    CALL(s, K_MIX, 1, a_int(d), a_int(n_blocks), a_int(1), a_int(stack),
         a_f32v(s->blocks, last, INK_MAX_BLOCKS * stack - last),
         a_f32v(s->cur, last, d),
         a_tensor_f32(s, &s->gguf.out_res_query),
         a_f32v(s->norm_buf, 0, d));
    CALL(s, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
         a_f32v(s->norm_buf, 0, d), a_tensor_f32(s, &s->gguf.out_norm), a_f32v(s->norm_buf, 0, d));
    if (s->output_iq4) {
        int64_t blocks = d / 32;
        int64_t lrows = (int64_t)(s->gguf.output.nbytes / (uint64_t)(blocks * 34));
        CALL(s, K_DVQ, (nv + 31) / 32, a_int(nv), a_int(lrows), a_int(d), a_int(0), a_int(0),
             a_view(s->output_dev, 0, lrows * blocks * 4),
             a_view(s->output_dev, lrows * blocks * 16, lrows * blocks),
             a_f32v(s->norm_buf, 0, d), a_f32v(s->logits, 0, nv));
    } else {
        if (dispatch_mv(s, &s->gguf.output, 1, nv, d, 0, s->norm_buf, 0, s->logits, 0) != 0) return 1;
    }
    if (hp->logit_softcap > 0.0f) {
        CALL(s, K_SOFTCAP, nv, a_int(nv), a_f32(hp->logit_softcap), a_f32v(s->logits, 0, nv));
    }
    CALL(s, K_ARGMAX_S1, 64, a_int(nv), a_f32v(s->logits, 0, nv),
         a_f32v(s->argmax_vals, 0, 64), a_u32v(s->argmax_ids, 0, 64));
    CALL(s, K_ARGMAX_S2, 1, a_int(64), a_f32v(s->argmax_vals, 0, 64),
         a_u32v(s->argmax_ids, 0, 64), a_u32v(s->argmax_out, 0, 1));
    CALL(s, K_TOPK_FILTER, 256, a_int(nv), a_f32(16.0f), a_f32v(s->argmax_vals, 0, 64),
         a_f32v(s->logits, 0, nv), a_u32v(s->topk_cnt, 0, 256),
         a_u32v(s->topk_ids, 0, 256 * 16), a_f32v(s->topk_vals, 0, 256 * 16));
    return 0;
}

/* Multi-token output mix -> out_norm -> BF16 Tensor Core lm_head -> softcap -> per-token GPU argmax + sparse top-k. */
static int dispatch_spec_logits(InkSession *s, int64_t np, int n_blocks) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd;
    int64_t stack = (int64_t)s->batch * d;
    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
    CALL(s, K_MIX, np, a_int(d), a_int(n_blocks), a_int(1), a_int(stack),
         a_f32v(s->blocks, 0, INK_MAX_BLOCKS * stack),
         a_f32v(s->cur, 0, np * d),
         a_tensor_f32(s, &s->gguf.out_res_query),
         a_f32v(s->spec_hidden, 0, np * d));
    CALL(s, K_RMSNORM, np, a_int(d), a_int(1), a_f32(hp->rms_eps),
         a_f32v(s->spec_hidden, 0, np * d), a_tensor_f32(s, &s->gguf.out_norm), a_f32v(s->spec_hidden, 0, np * d));
    if (s->output_iq4 && !s->no_mma && !s->oracle_q8 && d % 32 == 0) {
        int64_t blocks = d / 32;
        int64_t lrows = (int64_t)(s->gguf.output.nbytes / (uint64_t)(blocks * 34));
        CALL(s, K_BF16_PACK, np * (d / 2), a_int(np * (d / 2)),
             a_f32v(s->spec_hidden, 0, np * d), a_u32v(s->x_bf16, 0, np * (d / 2)));
        CALL(s, K_IQ4_MMA_NB8_SK, (nv + 15) / 16, a_int(nv), a_int(lrows), a_int(d), a_int(np), a_int(1),
             a_view(s->output_dev, 0, lrows * blocks * 4),
             a_view(s->output_dev, lrows * blocks * 16, lrows * blocks),
             a_u32v(s->x_bf16, 0, np * (d / 2)), a_f32v(s->spec_logits, 0, np * nv));
    } else if (s->gguf.output.type == INK_GGML_Q8_0 && !s->no_mma && !s->oracle_q8 && d % 32 == 0) {
        CALL(s, K_BF16_PACK, np * (d / 2), a_int(np * (d / 2)),
             a_f32v(s->spec_hidden, 0, np * d), a_u32v(s->x_bf16, 0, np * (d / 2)));
        if (dispatch_q8_mma(s, &s->gguf.output, np, nv, d, 0, s->x_bf16, 0, s->spec_logits, 0) != 0) return 1;
    } else {
        if (dispatch_mv(s, &s->gguf.output, np, nv, d, 0, s->spec_hidden, 0, s->spec_logits, 0) != 0) return 1;
    }
    if (hp->logit_softcap > 0.0f) {
        CALL(s, K_SOFTCAP, np * nv, a_int(np * nv), a_f32(hp->logit_softcap), a_f32v(s->spec_logits, 0, np * nv));
    }
    for (int64_t t = 0; t < np; t++) {
        CALL(s, K_ARGMAX_S1, 64, a_int(nv), a_f32v(s->spec_logits, t * nv, nv),
             a_f32v(s->argmax_vals, 0, 64), a_u32v(s->argmax_ids, 0, 64));
        CALL(s, K_ARGMAX_S2, 1, a_int(64), a_f32v(s->argmax_vals, 0, 64),
             a_u32v(s->argmax_ids, 0, 64), a_u32v(s->spec_argmax_out, t, 1));
        CALL(s, K_TOPK_FILTER, 256, a_int(nv), a_f32(16.0f), a_f32v(s->argmax_vals, 0, 64),
             a_f32v(s->spec_logits, t * nv, nv), a_u32v(s->spec_topk_cnt, t * 256, 256),
             a_u32v(s->spec_topk_ids, t * 256 * 16, 256 * 16), a_f32v(s->spec_topk_vals, t * 256 * 16, 256 * 16));
    }
    return 0;
}

/* Device-side body of a batch (embedding norm through optional logit softcap).
   Safe to capture into one CUDA Graph when !s->streaming and (np == 1 || np <= 8). */
static int enqueue_body(InkSession *s, int64_t np, int compute_logits) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd;
    int64_t nd = np * d;
    int64_t stack = (int64_t)s->batch * d;

    if (s->spec_recording) {
        CALL(s, K_COPY, (int64_t)s->conv_all_elems, a_int((int64_t)s->conv_all_elems),
             a_f32v(s->conv_all, 0, (int64_t)s->conv_all_elems),
             a_f32v(s->spec_conv_bak, 0, (int64_t)s->conv_all_elems));
    }

    if (enqueue_embed(s, np, s->cur) != 0) return 1;

    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;
    int cap_taps = s->dspark.ready ? 1 : 0;

    /* 2. Prelude layers [0 .. start). */
    for (int il = 0; il < start; il++) {
        if (cap_taps && il == 5) {
            CALL(s, K_COPY, nd, a_int(nd), a_f32v(s->cur, 0, nd), a_f32v(s->dspark.taps, 0, nd));
        }
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
            if (cap_taps && pass == 1 && ip == 11) {
                CALL(s, K_COPY, nd, a_int(nd), a_f32v(s->cur, 0, nd), a_f32v(s->dspark.taps, nd, nd));
            } else if (cap_taps && pass == 2 && ip == 17) {
                CALL(s, K_COPY, nd, a_int(nd), a_f32v(s->cur, 0, nd), a_f32v(s->dspark.taps, 2 * nd, nd));
            }
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

    /* 5. Output mix -> output_norm -> lm_head -> logit softcap (+ DSpark tap 3). */
    if (compute_logits == 2) {
        if (dispatch_spec_logits(s, np, n_blocks) != 0) return 1;
        if (cap_taps) {
            CALL(s, K_COPY, nd, a_int(nd), a_f32v(s->spec_hidden, 0, nd), a_f32v(s->dspark.taps, 3 * nd, nd));
        }
    } else if (compute_logits) {
        if (cap_taps && np > 1) {
            CALL(s, K_MIX, np, a_int(d), a_int(n_blocks), a_int(1), a_int(stack),
                 a_f32v(s->blocks, 0, INK_MAX_BLOCKS * stack), a_f32v(s->cur, 0, nd),
                 a_tensor_f32(s, &s->gguf.out_res_query), a_f32v(s->dspark.taps, 3 * nd, nd));
            CALL(s, K_RMSNORM, np, a_int(d), a_int(1), a_f32(hp->rms_eps),
                 a_f32v(s->dspark.taps, 3 * nd, nd), a_tensor_f32(s, &s->gguf.out_norm),
                 a_f32v(s->dspark.taps, 3 * nd, nd));
        }
        if (dispatch_output_logits(s, np, n_blocks) != 0) return 1;
        if (cap_taps && np == 1) {
            CALL(s, K_COPY, d, a_int(d), a_f32v(s->norm_buf, 0, d), a_f32v(s->dspark.taps, 3 * d, d));
        }
    } else if (cap_taps) {
        CALL(s, K_MIX, np, a_int(d), a_int(n_blocks), a_int(1), a_int(stack),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * stack), a_f32v(s->cur, 0, nd),
             a_tensor_f32(s, &s->gguf.out_res_query), a_f32v(s->dspark.taps, 3 * nd, nd));
        CALL(s, K_RMSNORM, np, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->dspark.taps, 3 * nd, nd), a_tensor_f32(s, &s->gguf.out_norm),
             a_f32v(s->dspark.taps, 3 * nd, nd));
    }
    return 0;
}

/* Segmented CUDA Graph decode for streaming GPUs (experts do not all fit in
   VRAM): each segment captures all device launches between consecutive
   stream_experts host syncs (~18 launches -> 1 cuGraphLaunch per layer). */
static int enqueue_streaming_graph(InkSession *s, int compute_logits) {
    const InkHParams *hp = &s->gguf.hp;
    int ci = compute_logits ? 1 : 0;

    if (s->stream_segs_ready) {
        for (int k = 0; k < s->n_slots; k++) {
            if (pc_replay(s->stream_seg[k], s->stream) != PC_OK) return host_fail(s, "replay segment");
            PcBuffer ids = 0;
            if (stream_experts(s, s->seg_ip[k], (int)hp->n_expert_used, &ids) != 0) return 1;
        }
        if (!s->stream_tail[ci]) {
            if (pc_capture_begin(s->stream) != PC_OK) return host_fail(s, "capture tail begin");
            s->capturing = 1;
            PcBuffer moe_ids_b = (s->experts[s->last_ip].cap < hp->n_expert) ? s->moe_slots : s->moe_ids;
            if (dispatch_layer_moe(s, s->last_il, s->last_ip, 1, moe_ids_b) != 0 ||
                (compute_logits && dispatch_output_logits(s, 1, s->final_n_blocks) != 0)) {
                abort_capture(s);
                return 1;
            }
            s->capturing = 0;
            if (pc_capture_end(s->stream, &s->stream_tail[ci]) != PC_OK) return host_fail(s, "capture tail end");
        }
        if (pc_replay(s->stream_tail[ci], s->stream) != PC_OK) return host_fail(s, "replay tail");
        return 0;
    }

    int64_t d = hp->n_embd;
    int64_t stack = (int64_t)s->batch * d;
    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;
    int seg = 0, prev_il = -1, prev_ip = -1, rc = 1;

    if (pc_capture_begin(s->stream) != PC_OK) return host_fail(s, "capture seg 0 begin");
    s->capturing = 1;
    if (enqueue_embed(s, 1, s->cur) != 0) goto fail;

#define BEGIN_SEG_IF_PREV() do { \
        if (prev_il >= 0) { \
            if (pc_capture_begin(s->stream) != PC_OK) { host_fail(s, "capture seg begin"); goto fail; } \
            s->capturing = 1; \
            PcBuffer moe_ids_b_ = (s->experts[prev_ip].cap < hp->n_expert) ? s->moe_slots : s->moe_ids; \
            if (dispatch_layer_moe(s, prev_il, prev_ip, 1, moe_ids_b_) != 0) goto fail; \
            prev_il = -1; \
        } \
    } while (0)

#define END_SEG_AND_STREAM(ip_cur) do { \
        s->capturing = 0; \
        if (pc_capture_end(s->stream, &s->stream_seg[seg]) != PC_OK) { host_fail(s, "capture seg end"); goto fail; } \
        s->seg_ip[seg] = (ip_cur); \
        if (pc_replay(s->stream_seg[seg], s->stream) != PC_OK) { host_fail(s, "replay seg"); goto fail; } \
        seg++; \
        PcBuffer ids_dummy_ = 0; \
        if (stream_experts(s, (ip_cur), (int)hp->n_expert_used, &ids_dummy_) != 0) goto fail; \
    } while (0)

    for (int il = 0; il < start; il++) {
        BEGIN_SEG_IF_PREV();
        if (dispatch_layer_pre(s, il, il, 1, &n_blocks, 0) != 0) goto fail;
        END_SEG_AND_STREAM(il);
        prev_il = il; prev_ip = il;
    }

    BEGIN_SEG_IF_PREV();
    {
        PcArg _a1[] = { a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->prelude, 0, d) };
        if (call_k(s, K_COPY, d, 3, _a1) != 0) goto fail;
    }
    int prelude_n_blocks = n_blocks;
    if (prelude_n_blocks > 0) {
        int64_t n = (int64_t)prelude_n_blocks * stack;
        PcArg _a2[] = { a_int(n), a_f32v(s->blocks, 0, n), a_f32v(s->prelude_blk, 0, n) };
        if (call_k(s, K_COPY, n, 3, _a2) != 0) goto fail;
    }

    for (int pass = 0; pass < hp->loop_count; pass++) {
        int bank_n_blocks = 0;
        if (pass > 0) {
            BEGIN_SEG_IF_PREV();
            int src_off = prelude_n_blocks + 1;
            bank_n_blocks = n_blocks - src_off;
            if (bank_n_blocks < 0) bank_n_blocks = 0;
            if (bank_n_blocks > 0) {
                int64_t n = (int64_t)bank_n_blocks * stack;
                PcArg _ab[] = { a_int(n), a_f32v(s->blocks, (int64_t)src_off * stack, n), a_f32v(s->bank_blk, 0, n) };
                if (call_k(s, K_COPY, n, 3, _ab) != 0) goto fail;
            }
            {
                PcArg _ao[] = { a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->bank_out, 0, d) };
                if (call_k(s, K_COPY, d, 3, _ao) != 0) goto fail;
                PcArg _ap[] = { a_int(d), a_f32v(s->prelude, 0, d), a_f32v(s->cur, 0, d) };
                if (call_k(s, K_COPY, d, 3, _ap) != 0) goto fail;
            }
            n_blocks = prelude_n_blocks;
            if (prelude_n_blocks > 0) {
                int64_t n = (int64_t)prelude_n_blocks * stack;
                PcArg _ar[] = { a_int(n), a_f32v(s->prelude_blk, 0, n), a_f32v(s->blocks, 0, n) };
                if (call_k(s, K_COPY, n, 3, _ar) != 0) goto fail;
            }
        }
        for (int ip = start; ip < end; ip++) {
            int il = (pass == 0) ? ip : (int)hp->n_phys + (pass - 1) * (end - start) + (ip - start);
            BEGIN_SEG_IF_PREV();
            if (pass > 0) {
                const InkLayerTensors *L = &s->gguf.layers[ip];
                PcArg _am[] = { a_int(d), a_int(bank_n_blocks), a_int(4), a_int(stack),
                                a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * stack),
                                a_f32v(s->bank_out, 0, d),
                                a_tensor_f32(s, &L->loop_msg_query),
                                a_f32v(s->messages, 0, 4 * stack) };
                if (call_k(s, K_MIX, 4, 8, _am) != 0) goto fail;
            }
            if (dispatch_layer_pre(s, il, ip, 1, &n_blocks, pass > 0) != 0) goto fail;
            END_SEG_AND_STREAM(ip);
            prev_il = il; prev_ip = ip;
        }
    }

    for (int il = end; il < hp->n_phys; il++) {
        BEGIN_SEG_IF_PREV();
        if (dispatch_layer_pre(s, il, il, 1, &n_blocks, 0) != 0) goto fail;
        END_SEG_AND_STREAM(il);
        prev_il = il; prev_ip = il;
    }

#undef BEGIN_SEG_IF_PREV
#undef END_SEG_AND_STREAM

    if (seg != s->n_slots || prev_il < 0) {
        snprintf(s->error, sizeof s->error, "streaming graph segment count mismatch (%d vs %d)", seg, s->n_slots);
        goto fail;
    }
    s->last_il = prev_il;
    s->last_ip = prev_ip;
    s->final_n_blocks = n_blocks;
    s->stream_segs_ready = 1;

    if (pc_capture_begin(s->stream) != PC_OK) return host_fail(s, "capture tail begin");
    s->capturing = 1;
    {
        PcBuffer moe_ids_b = (s->experts[s->last_ip].cap < hp->n_expert) ? s->moe_slots : s->moe_ids;
        if (dispatch_layer_moe(s, s->last_il, s->last_ip, 1, moe_ids_b) != 0 ||
            (compute_logits && dispatch_output_logits(s, 1, s->final_n_blocks) != 0)) {
            abort_capture(s);
            return 1;
        }
    }
    s->capturing = 0;
    if (pc_capture_end(s->stream, &s->stream_tail[ci]) != PC_OK) return host_fail(s, "capture tail end");
    if (pc_replay(s->stream_tail[ci], s->stream) != PC_OK) return host_fail(s, "replay tail");
    return 0;

fail:
    abort_capture(s);
    for (int k = 0; k < seg; k++) {
        if (s->stream_seg[k]) { pc_graph_free(s->stream_seg[k]); s->stream_seg[k] = NULL; }
    }
    return rc;
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

    /* 1. Upload the batch's embedding rows from host memory. */
    for (int b = 0; b < np_tokens; b++) {
        if (tokens[b] < 0 || tokens[b] >= hp->n_vocab) {
            snprintf(s->error, sizeof s->error, "token id %d out of range [0,%lld)", tokens[b], (long long)hp->n_vocab);
            return 1;
        }
        if (upload_embd_row(s, b, tokens[b]) != 0) return 1;
    }

    if (np_tokens == 1 && !s->no_graph && !s->profile) {
        s->pos_host = (uint32_t)s->pos;
        if (pc_upload(s->dev_pos, &s->pos_host, 4, s->stream) != PC_OK)
            return host_fail(s, "upload position");
        if (!s->streaming) {
            int ci = compute_logits ? 1 : 0;
            if (!s->decode_graph[ci]) {
                if (pc_capture_begin(s->stream) != PC_OK) return host_fail(s, "capture decode begin");
                s->capturing = 1;
                if (enqueue_body(s, 1, compute_logits) != 0) {
                    abort_capture(s);
                    return 1;
                }
                s->capturing = 0;
                if (pc_capture_end(s->stream, &s->decode_graph[ci]) != PC_OK) return host_fail(s, "capture decode end");
            }
            if (pc_replay(s->decode_graph[ci], s->stream) != PC_OK) return host_fail(s, "replay decode");
        } else {
            if (enqueue_streaming_graph(s, compute_logits) != 0) return 1;
        }
    } else {
        if (enqueue_body(s, np, compute_logits) != 0) return 1;
    }

    if (compute_logits) {
        /* Download ONLY 4-byte GPU argmax + 33 KB sparse top-k bins (eliminates 804 KB full-vocab D2H copy per step!).
           Full logits_host and hidden_host are downloaded lazily if ink_logits()/ink_hidden() are called. */
        if (pc_download(&s->argmax_host, s->argmax_out, 4, s->stream) != PC_OK ||
            pc_download(s->topk_cnt_host, s->topk_cnt, 256 * 4, s->stream) != PC_OK ||
            pc_download(s->topk_ids_host, s->topk_ids, 256 * 16 * 4, s->stream) != PC_OK ||
            pc_download(s->topk_vals_host, s->topk_vals, 256 * 16 * 4, s->stream) != PC_OK)
            return host_fail(s, "read argmax/topk");
        s->argmax_valid = 1;
        s->logits_host_valid = 0;
        s->hidden_host_valid = 0;
    } else {
        s->argmax_valid = 0;
        s->logits_host_valid = 0;
        s->hidden_host_valid = 0;
    }

    s->pos += np_tokens;
    ink_mark_written_cuda(s, s->pos);
    s->total_steps += np_tokens;
    return 0;
}

static int ink_flush_spec(InkSession *s) {
    if (!s || s->conv_pending_n <= 0) return 0;
    int got = s->conv_pending_n;
    int nb = s->conv_pending_nb;
    s->conv_pending_n = 0;
    s->conv_pending_nb = 0;
    if (got >= nb) return 0;
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd;
    int64_t n_kv = hp->n_head_kv * hp->head_dim;
    int64_t items = (int64_t)s->n_slots * (2 * n_kv + 2 * d);
    CALL(s, K_CONV_REWIND_ALL, items,
         a_int(s->n_slots), a_int(n_kv), a_int(d), a_int(got),
         a_f32v(s->spec_conv_bak, 0, (int64_t)s->conv_all_elems),
         a_f32v(s->spec_raw_log, 0, (int64_t)s->spec_raw_log_elems),
         a_f32v(s->conv_all, 0, (int64_t)s->conv_all_elems));
    return 0;
}

static void ink_sess_hist_push(InkSession *s, int32_t tok) {
    if (!s || tok < 0) return;
    if (!s->sess_hist) {
        s->sess_hist_cap = 16384;
        s->sess_hist = (int32_t *)malloc((size_t)s->sess_hist_cap * sizeof(int32_t));
        if (!s->sess_hist) { s->sess_hist_cap = 0; return; }
        s->sess_hist_len = 0;
    }
    if (s->sess_hist_len >= s->sess_hist_cap) {
        int keep = s->sess_hist_cap / 2;
        memmove(s->sess_hist, s->sess_hist + (s->sess_hist_cap - keep), (size_t)keep * sizeof(int32_t));
        s->sess_hist_len = keep;
    }
    s->sess_hist[s->sess_hist_len++] = tok;
}

static int pld_search_buf_cuda(const int32_t *ctx, int ctx_len, int32_t cur_tok,
                               const int32_t *src, int src_len, int min_n, int max_n,
                               int *out_found) {
    if (!src || src_len < min_n + 1 || ctx_len < min_n) return 0;
    int lo = src_len - 8192;
    if (lo < 0) lo = 0;
    for (int n = max_n; n >= min_n; n--) {
        if (ctx_len < n || src_len < n + 1) continue;
        for (int j = src_len - n - 1; j >= lo; j--) {
            if (src[j + n - 1] != cur_tok) continue;
            int ok = 1;
            for (int k = 0; k < n - 1; k++) {
                if (src[j + k] != ctx[ctx_len - n + k]) { ok = 0; break; }
            }
            if (ok) { *out_found = j + n; return n; }
        }
    }
    return 0;
}

static int ink_pld_draft_cuda(const InkSession *s, int32_t cur_tok, int32_t *out_draft, int *out_match_n) {
    *out_match_n = 0;
    if (!s->tok_hist || s->pos < 2 || s->pos + 2 >= s->max_ctx) return 0;
    const int32_t *h = s->tok_hist;
    int len = s->pos + 1;
    int min_n = (s->pld_ema2 >= 0.25f) ? 2 : 3;
    const int32_t *src = NULL;
    int src_len = 0, found = -1, match_n = 0;
    if (s->pred_toks && s->pred_len >= 4) {
        int pn = pld_search_buf_cuda(h, len, cur_tok, s->pred_toks, s->pred_len, 3, 10, &found);
        if (pn >= 3) { src = s->pred_toks; src_len = s->pred_len; match_n = pn; }
    }
    if (!src) {
        int lo = len - 4096; if (lo < 0) lo = 0;
        for (int n = 10; n >= min_n && found < 0; n--) {
            if (len < n + 1) continue;
            for (int j = len - n - 1; j >= lo; j--) {
                if (h[j + n - 1] != cur_tok) continue;
                int ok = 1;
                for (int k = 0; k < n - 1; k++) {
                    if (h[j + k] != h[len - n + k]) { ok = 0; break; }
                }
                if (ok) { found = j + n; match_n = n; src = h; src_len = len - 1; break; }
            }
        }
    }
    if (!src && s->sess_hist && s->sess_hist_len >= 5) {
        int sn = pld_search_buf_cuda(h, len, cur_tok, s->sess_hist, s->sess_hist_len, 4, 10, &found);
        if (sn >= 4) { src = s->sess_hist; src_len = s->sess_hist_len; match_n = sn; }
    }
    if (!src || found < 0) return 0;
    int m = (match_n >= 6) ? 7
          : (match_n == 5) ? 6
          : (match_n == 4) ? 5
          : (match_n == 3) ? ((s->pld_ema3 >= 0.55f) ? 4 : (s->pld_ema3 >= 0.35f) ? 3 : 2)
          : ((s->pld_ema2 >= 0.45f) ? 2 : 1);
    int max_m = s->max_ctx - s->pos - 1;
    if (max_m > 7) max_m = 7;
    if (m > max_m) m = max_m;
    int avail = src_len - found;
    if (avail < 1) return 0;
    if (m > avail) m = avail;
    for (int k = 0; k < m; k++) {
        int32_t tk = src[found + k];
        if (tk < 0 || tk >= s->gguf.hp.n_vocab) { m = k; break; }
        out_draft[k] = tk;
    }
    *out_match_n = match_n;
    return m;
}

static int ink_verify_batch_cuda(InkSession *s, const int32_t *tokens, int nb) {
    const InkHParams *hp = &s->gguf.hp;
    if (nb < 2 || nb > 8 || !s->can_verify_nb) return 1;
    if (s->pos + nb > s->max_ctx) {
        snprintf(s->error, sizeof s->error, "context full (%d + %d of %d)", s->pos, nb, s->max_ctx);
        return 1;
    }
    double began = pc_seconds();
    if (ink_flush_spec(s) != 0) return 1;
    for (int b = 0; b < nb; b++) {
        if (tokens[b] < 0 || tokens[b] >= hp->n_vocab) {
            snprintf(s->error, sizeof s->error, "token id %d out of range [0,%lld)", tokens[b], (long long)hp->n_vocab);
            return 1;
        }
        if (upload_embd_row(s, b, tokens[b]) != 0) return 1;
    }
    s->pos_host = (uint32_t)s->pos;
    if (pc_upload(s->dev_pos, &s->pos_host, 4, s->stream) != PC_OK)
        return host_fail(s, "upload verify position");

    if (!s->no_graph && !s->profile) {
        if (!s->verify_graph[nb]) {
            if (pc_capture_begin(s->stream) != PC_OK) return host_fail(s, "capture verify begin");
            s->capturing = 1;
            s->spec_recording = 1;
            if (enqueue_body(s, nb, 2) != 0) {
                abort_capture(s);
                return 1;
            }
            s->spec_recording = 0;
            s->capturing = 0;
            if (pc_capture_end(s->stream, &s->verify_graph[nb]) != PC_OK) return host_fail(s, "capture verify end");
        }
        if (pc_replay(s->verify_graph[nb], s->stream) != PC_OK) return host_fail(s, "replay verify");
    } else {
        s->spec_recording = 1;
        int rc = enqueue_body(s, nb, 2);
        s->spec_recording = 0;
        if (rc != 0) return 1;
    }

    if (pc_download(s->spec_argmax_host, s->spec_argmax_out, (size_t)nb * 4, s->stream) != PC_OK ||
        pc_download(s->spec_topk_cnt_host, s->spec_topk_cnt, (size_t)nb * 256 * 4, s->stream) != PC_OK ||
        pc_download(s->spec_topk_ids_host, s->spec_topk_ids, (size_t)nb * 256 * 16 * 4, s->stream) != PC_OK ||
        pc_download(s->spec_topk_vals_host, s->spec_topk_vals, (size_t)nb * 256 * 16 * 4, s->stream) != PC_OK)
        return host_fail(s, "read verify argmax/topk");
    double queued = pc_seconds();
    if (pc_sync_stream(s->stream) != PC_OK) return host_fail(s, "verify sync");
    double dt = pc_seconds() - began;
    s->total_gpu_seconds += dt;
    s->queue_seconds += queued - began;
    s->dspark.last_verify_s = dt;
    ink_mark_written_cuda(s, s->pos + nb);
    return 0;
}

static float next_u01_cuda(uint64_t *rng_state) {
    uint64_t x = rng_state ? *rng_state : 0x9e3779b97f4a7c15ULL;
    x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
    if (rng_state) *rng_state = x;
    return (float)((x * 2685821657736338717ULL) >> 40) / (float)(1ULL << 24);
}

static int32_t sample_logits_ex_cuda(const float *logits, int n, const InkSampleParams *p, uint64_t *rng_state,
                                     int32_t verify_draft, int *out_accepted) {
    if (!logits || n <= 0) return -1;
    float temp = p ? p->temperature : 0.0f;
    if (temp <= 1e-6f) {
        int best = 0; float bv = logits[0];
        for (int i = 1; i < n; i++) {
            if (logits[i] > bv) { bv = logits[i]; best = i; }
        }
        if (out_accepted) *out_accepted = (verify_draft >= 0 && best == verify_draft) ? 1 : 0;
        return (int32_t)best;
    }
    int top_k = (p && p->top_k > 0 && p->top_k < n) ? p->top_k : 40;
    float top_p = (p && p->top_p > 0.0f && p->top_p < 1.0f) ? p->top_p : 0.95f;
    int idx_stack[64];
    float val_stack[64];
    int *idx = (top_k <= 64) ? idx_stack : (int *)malloc((size_t)top_k * sizeof(int));
    float *val = (top_k <= 64) ? val_stack : (float *)malloc((size_t)top_k * sizeof(float));
    if (!idx || !val) { if (top_k > 64) { free(idx); free(val); } return 0; }
    int k = 0;
    float inv_t = 1.0f / temp;
    for (int i = 0; i < n; i++) {
        float v = logits[i] * inv_t;
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
    for (int i = 0; i < cut; i++) val[i] /= norm;

    int chosen = idx[0];
    if (verify_draft >= 0) {
        float p_draft = 0.0f;
        int draft_pos = -1;
        for (int i = 0; i < cut; i++) {
            if (idx[i] == verify_draft) { p_draft = val[i]; draft_pos = i; break; }
        }
        float u = next_u01_cuda(rng_state);
        if (u < p_draft) {
            if (out_accepted) *out_accepted = 1;
            chosen = verify_draft;
        } else {
            if (out_accepted) *out_accepted = 0;
            float rem = 1.0f - p_draft;
            if (rem <= 1e-7f) {
                chosen = idx[0];
            } else {
                float r = next_u01_cuda(rng_state);
                float acc = 0.0f;
                chosen = (draft_pos == 0 && cut > 1) ? idx[1] : idx[0];
                for (int i = 0; i < cut; i++) {
                    if (i == draft_pos) continue;
                    acc += val[i] / rem;
                    if (r <= acc) { chosen = idx[i]; break; }
                }
            }
        }
    } else {
        float r = next_u01_cuda(rng_state);
        float acc = 0.0f;
        for (int i = 0; i < cut; i++) {
            acc += val[i];
            if (r <= acc) { chosen = idx[i]; break; }
        }
    }
    if (top_k > 64) { free(idx); free(val); }
    return (int32_t)chosen;
}

int ink_prefill(InkSession *s, const int32_t *tokens, int n_tokens, int compute_last_logits) {
    if (n_tokens <= 0) return 0;
    if (n_tokens == 1) return ink_step(s, tokens[0], compute_last_logits);
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;
    if (ink_flush_spec(s) != 0) return 1;
    if (s->dspark.ready && s->dspark.pending_got > 0) {
        if (ink_dspark_flush_pending(s) != 0) return 1;
    }
    if (s->tok_hist && s->pos + n_tokens <= s->max_ctx) {
        memcpy(s->tok_hist + s->pos, tokens, (size_t)n_tokens * sizeof(int32_t));
        if (s->pos + n_tokens > s->tok_hist_len) s->tok_hist_len = s->pos + n_tokens;
    }
    if (s->live_tokens && s->pos + n_tokens <= s->max_ctx) {
        memcpy(s->live_tokens + s->pos, tokens, (size_t)n_tokens * sizeof(int32_t));
        s->live_len = s->pos + n_tokens;
    }
    double began = pc_seconds();
    for (int at = 0; at < n_tokens; at += s->batch) {
        int np = n_tokens - at < s->batch ? n_tokens - at : s->batch;
        int need_logits = compute_last_logits && at + np == n_tokens;
        int chunk_pos0 = s->pos;
        if (enqueue_batch(s, tokens + at, np, need_logits) != 0) {
            pc_sync_stream(s->stream);
            return 1;
        }
        if (s->dspark.ready && s->dspark.pos == chunk_pos0) {
            if (ink_dspark_enqueue_absorb(s, np, np, chunk_pos0) != 0) {
                pc_sync_stream(s->stream);
                return 1;
            }
        }
    }
    double queued = pc_seconds();
    if (pc_sync_stream(s->stream) != PC_OK) return host_fail(s, "device");
    s->total_gpu_seconds += pc_seconds() - began;
    s->queue_seconds += queued - began;
    return 0;
}

int ink_step(InkSession *s, int32_t token_id, int compute_logits) {
    if (!s) return 1;
    const InkHParams *hp = &s->gguf.hp;
    if (s->pos >= s->max_ctx) {
        snprintf(s->error, sizeof s->error, "context full (%d/%d)", s->pos, s->max_ctx);
        return 1;
    }
    if (token_id < 0 || token_id >= hp->n_vocab) {
        snprintf(s->error, sizeof s->error, "token id %d out of range [0,%lld)", token_id, (long long)hp->n_vocab);
        return 1;
    }
    /* Stage C3A/C3B: 0.000 ms speculative queue hit (exact for BOTH greedy temp=0 and sampled temp>0!) */
    if (s->spec_head < s->spec_tail && compute_logits && token_id == s->spec_q[s->spec_head]) {
        int idx = s->spec_head++;
        ink_sess_hist_push(s, token_id);
        if (s->tok_hist && s->pos < s->max_ctx) {
            s->tok_hist[s->pos] = token_id;
            if (s->pos + 1 > s->tok_hist_len) s->tok_hist_len = s->pos + 1;
        }
        if (s->live_tokens && s->pos < s->max_ctx) {
            s->live_tokens[s->pos] = token_id;
            s->live_len = s->pos + 1;
        }
        s->pos++;
        s->total_steps++;
        s->conv_pending_n++;
        if (s->dspark.ready && s->dspark.pending_got > 0) s->dspark.pending_got++;
        s->spec_active_idx = idx;
        s->argmax_valid = 1;
        s->logits_host_valid = 0;
        s->hidden_host_valid = 0;
        return 0;
    }
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;

    int allow_spec = (s->can_verify_nb && compute_logits && !s->profile && getenv("INK_NO_SPEC") == NULL);
    if (allow_spec) {
        int32_t draft[7];
        float ds_probs[7] = {0};
        int match_n = 0;
        int m = ink_pld_draft_cuda(s, token_id, draft, &match_n);
        int used_dspark = 0;

        /* Prefer zero-cost PLD only on high-confidence >=4-gram matches; otherwise run Native Coax CUDA DSpark! */
        if (!(m >= 1 && match_n >= 4) &&
            s->dspark.ready && !s->dspark.req_disabled &&
            s->dspark.pos + s->dspark.pending_got == s->pos &&
            s->pos >= 1 && s->pos + 1 < s->max_ctx && s->dspark.skip_countdown == 0) {
            int max_c = s->max_ctx - s->pos - 1;
            const char *k_env = getenv("INK_DSPARK_K");
            int k_cap = (k_env && atoi(k_env) >= 1 && atoi(k_env) <= 7) ? atoi(k_env) : 7;
            if (max_c > k_cap) max_c = k_cap;
            int32_t ds_toks[7];
            if (ink_dspark_draft(s, token_id, max_c, ds_toks, ds_probs) == 0) {
                /* Decision-theoretic CUDA MoE pruning:
                   Include token k while cumulative prob >= 0.12 with E[acc] >= 1.35! */
                const char *mp_env = getenv("INK_DSPARK_MIN_P");
                float min_cum = mp_env ? strtof(mp_env, NULL) : 0.12f;
                const char *ea_env = getenv("INK_DSPARK_MIN_EACC");
                float min_eacc = ea_env ? strtof(ea_env, NULL) : 1.35f;
                int dm = 0;
                float cum = 1.0f, e_acc = 0.0f;
                while (dm < max_c) {
                    cum *= ds_probs[dm];
                    if (cum < min_cum) break;
                    e_acc += cum;
                    dm++;
                }
                if (dm >= 2 && e_acc >= min_eacc) {
                    for (int k = 0; k < dm; k++) draft[k] = ds_toks[k];
                    m = dm;
                    used_dspark = 1;
                } else {
                    /* Low-confidence token: skip verify and run fast 10.8 ms B=1 instead of losing on m=1! */
                    s->dspark.rounds++;
                    s->dspark.req_rounds++;
                    s->dspark.ema_acc = 0.80f * s->dspark.ema_acc;
                    double b1 = s->dspark.b1_ema_s > 1e-4 ? s->dspark.b1_ema_s : 0.0109;
                    double denom = s->dspark.last_draft_s + b1;
                    float util = (denom > 1e-6) ? (float)(b1 / denom) : 0.8f;
                    s->dspark.util_ema = 0.70f * s->dspark.util_ema + 0.30f * util;
                    if (s->dspark.req_rounds >= 3 && s->dspark.util_ema < 0.94f) {
                        s->dspark.skip_countdown = 6 * s->dspark.skip_mult;
                        if (s->dspark.skip_mult < 4) s->dspark.skip_mult *= 2;
                    }
                }
            }
        }

        if (m >= 1 && (used_dspark || m >= 2)) {
            if (s->dspark.ready && s->dspark.pending_got > 0) {
                if (ink_dspark_flush_pending(s) != 0) return 1;
            }
            int32_t batch_toks[8];
            batch_toks[0] = token_id;
            for (int k = 0; k < m; k++) batch_toks[k + 1] = draft[k];
            int nb = m + 1;
            int pos0 = s->pos;
            if (ink_verify_batch_cuda(s, batch_toks, nb) != 0) return 1;
            int n_acc = 0;
            for (int k = 0; k < m; k++) {
                if ((int32_t)s->spec_argmax_host[k] == draft[k]) n_acc++;
                else break;
            }
            if (used_dspark) {
                s->dspark.rounds++;
                s->dspark.req_rounds++;
                s->dspark.drafted += (uint64_t)m;
                s->dspark.accepted += (uint64_t)n_acc;
                s->dspark.ema_acc = 0.75f * s->dspark.ema_acc + 0.25f * (float)n_acc;
                double b1 = s->dspark.b1_ema_s > 1e-4 ? s->dspark.b1_ema_s : 0.0109;
                double round_s = s->dspark.last_draft_s + s->dspark.last_verify_s;
                float util = (round_s > 1e-6) ? (float)((double)(1 + n_acc) * b1 / round_s) : 1.0f;
                s->dspark.util_ema = 0.70f * s->dspark.util_ema + 0.30f * util;
                if (util >= 1.05f) {
                    s->dspark.skip_mult = 1;
                } else if (s->dspark.req_rounds >= 3 && s->dspark.util_ema < 0.94f) {
                    s->dspark.skip_countdown = 6 * s->dspark.skip_mult;
                    if (s->dspark.skip_mult < 4) s->dspark.skip_mult *= 2;
                }
                if (getenv("INK_DSPARK_TRACE") != NULL) {
                    fprintf(stderr, "[CUDA-DSPARK pos=%d m=%d acc=%d avg=%.2f ema=%.2f util=%.2f(ema=%.2f) sk=%d dr=%.2fms vf=%.2fms b1=%.2fms]\n",
                            pos0, m, n_acc, (double)s->dspark.accepted / (double)s->dspark.rounds,
                            (double)s->dspark.ema_acc, util, s->dspark.util_ema, s->dspark.skip_countdown,
                            s->dspark.last_draft_s * 1000.0, s->dspark.last_verify_s * 1000.0, b1 * 1000.0);
                }
            } else {
                float rate = (float)n_acc / (float)m;
                if (match_n == 2) s->pld_ema2 = 0.80f * s->pld_ema2 + 0.20f * rate;
                else if (match_n == 3) s->pld_ema3 = 0.80f * s->pld_ema3 + 0.20f * rate;
            }
            s->spec_rounds++;
            s->spec_drafted += (uint64_t)m;
            s->spec_accepted += (uint64_t)n_acc;

            ink_sess_hist_push(s, token_id);
            if (s->tok_hist && pos0 < s->max_ctx) {
                s->tok_hist[pos0] = token_id;
                if (pos0 + 1 > s->tok_hist_len) s->tok_hist_len = pos0 + 1;
            }
            if (s->live_tokens && pos0 < s->max_ctx) {
                s->live_tokens[pos0] = token_id;
                s->live_len = pos0 + 1;
            }
            for (int t = 0; t < nb; t++) s->spec_q[t] = batch_toks[t];
            s->spec_head = 1;
            s->spec_tail = nb; /* Queue ALL verified drafts so temp>0 hits match seamlessly too! */
            s->spec_active_idx = 0;
            s->conv_pending_nb = nb;
            s->conv_pending_n = 1;
            if (s->dspark.ready && s->dspark.pos == pos0) {
                s->dspark.pending_pos0 = pos0;
                s->dspark.pending_nb = nb;
                s->dspark.pending_got = 1;
            }
            s->pos = pos0 + 1;
            s->total_steps++;
            s->argmax_valid = 1;
            s->logits_host_valid = 0;
            s->hidden_host_valid = 0;
            return 0;
        }
    }

    if (ink_flush_spec(s) != 0) return 1;
    int spec_allowed_env = (getenv("INK_NO_SPEC") == NULL);
    if (spec_allowed_env && s->dspark.ready && s->dspark.pending_got > 0) {
        if (ink_dspark_flush_pending(s) != 0) return 1;
    }
    if (s->dspark.ready && s->dspark.skip_countdown > 0) s->dspark.skip_countdown--;
    ink_sess_hist_push(s, token_id);
    int step_pos0 = s->pos;
    if (s->tok_hist && step_pos0 < s->max_ctx) {
        s->tok_hist[step_pos0] = token_id;
        if (step_pos0 + 1 > s->tok_hist_len) s->tok_hist_len = step_pos0 + 1;
    }
    if (s->live_tokens && step_pos0 < s->max_ctx) {
        s->live_tokens[step_pos0] = token_id;
        s->live_len = step_pos0 + 1;
    }
    double began = pc_seconds();
    if (enqueue_batch(s, &token_id, 1, compute_logits) != 0) {
        pc_sync_stream(s->stream);
        return 1;
    }
    double queued = pc_seconds();
    if (pc_sync_stream(s->stream) != PC_OK) return host_fail(s, "device");
    double dt = pc_seconds() - began;
    s->total_gpu_seconds += dt;
    s->queue_seconds += queued - began;
    if (compute_logits && dt > 0.002 && dt < 0.25) {
        s->dspark.b1_ema_s = (s->dspark.b1_ema_s > 1e-4) ? (0.8 * s->dspark.b1_ema_s + 0.2 * dt) : dt;
    }
    if (spec_allowed_env && s->dspark.ready && s->dspark.pos == step_pos0) {
        s->dspark.pending_pos0 = step_pos0;
        s->dspark.pending_nb = 1;
        s->dspark.pending_got = 1;
    }
    return 0;
}

int ink_verify_draft(InkSession *s, const int32_t *tokens, int n_tokens,
                     const InkSampleParams *p, uint64_t *rng_state,
                     int32_t *out_next_token) {
    if (!s || !tokens || n_tokens < 1 || !out_next_token) return -1;
    if (n_tokens == 1 || !s->can_verify_nb || n_tokens > 8) {
        if (ink_step(s, tokens[0], 1) != 0) return -1;
        *out_next_token = ink_sample(s, p, rng_state);
        return 0;
    }
    const InkHParams *hp = &s->gguf.hp;
    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;
    if (s->dspark.ready && s->dspark.pending_got > 0) {
        if (ink_dspark_flush_pending(s) != 0) return -1;
    }
    int pos0 = s->pos;
    if (ink_verify_batch_cuda(s, tokens, n_tokens) != 0) return -1;
    float temp = p ? p->temperature : 0.0f;
    int accepted = 0;
    int32_t next_tok = -1;
    if (temp <= 1e-6f) {
        for (int j = 0; j < n_tokens - 1; j++) {
            int32_t best = (int32_t)s->spec_argmax_host[j];
            if (best == tokens[j + 1]) accepted++;
            else { next_tok = best; break; }
        }
        if (accepted == n_tokens - 1) next_tok = (int32_t)s->spec_argmax_host[n_tokens - 1];
    } else {
        float *vlog = (float *)malloc((size_t)n_tokens * (size_t)nv * sizeof(float));
        if (!vlog) return -1;
        if (pc_download(vlog, s->spec_logits, (size_t)n_tokens * (size_t)nv * 4, s->stream) != PC_OK ||
            pc_sync_stream(s->stream) != PC_OK) {
            free(vlog);
            return -1;
        }
        for (int j = 0; j < n_tokens - 1; j++) {
            int ok = 0;
            int32_t sampled = sample_logits_ex_cuda(vlog + (size_t)j * (size_t)nv, (int)nv, p, rng_state, tokens[j + 1], &ok);
            if (ok) accepted++;
            else { next_tok = sampled; break; }
        }
        if (accepted == n_tokens - 1) {
            next_tok = sample_logits_ex_cuda(vlog + (size_t)(n_tokens - 1) * (size_t)nv, (int)nv, p, rng_state, -1, NULL);
        }
        free(vlog);
    }
    int a = accepted + 1;
    for (int k = 0; k < a; k++) ink_sess_hist_push(s, tokens[k]);
    if (s->tok_hist && pos0 + a <= s->max_ctx) {
        memcpy(s->tok_hist + pos0, tokens, (size_t)a * sizeof(int32_t));
        if (pos0 + a > s->tok_hist_len) s->tok_hist_len = pos0 + a;
    }
    if (s->live_tokens && pos0 + a <= s->max_ctx) {
        memcpy(s->live_tokens + pos0, tokens, (size_t)a * sizeof(int32_t));
        s->live_len = pos0 + a;
    }
    s->conv_pending_nb = n_tokens;
    s->conv_pending_n = a;
    if (s->dspark.ready && s->dspark.pos == pos0) {
        s->dspark.pending_pos0 = pos0;
        s->dspark.pending_nb = n_tokens;
        s->dspark.pending_got = a;
    }
    s->spec_active_idx = accepted;
    s->argmax_valid = 1;
    s->logits_host_valid = 0;
    s->hidden_host_valid = 0;
    s->pos = pos0 + a;
    s->total_steps += a;
    *out_next_token = next_tok;
    return accepted;
}

const float *ink_logits(const InkSession *s) {
    if (!s) return NULL;
    InkSession *ms = (InkSession *)s;
    if (!ms->logits_host_valid && ms->logits_host) {
        int64_t nv = ms->gguf.hp.n_vocab_unpadded > 0 ? ms->gguf.hp.n_vocab_unpadded : ms->gguf.hp.n_vocab;
        PcBuffer src = (ms->spec_active_idx >= 0 && ms->spec_logits)
            ? (ms->spec_logits + (uint64_t)((size_t)ms->spec_active_idx * (size_t)nv * 4))
            : ms->logits;
        if (src && pc_download(ms->logits_host, src, (size_t)nv * 4, ms->stream) == PC_OK &&
            pc_sync_stream(ms->stream) == PC_OK) {
            ms->logits_host_valid = 1;
        }
    }
    return s->logits_host;
}
const float *ink_hidden(const InkSession *s) {
    if (!s) return NULL;
    InkSession *ms = (InkSession *)s;
    if (!ms->hidden_host_valid && ms->hidden_host) {
        int64_t d = ms->gguf.hp.n_embd;
        PcBuffer src = (ms->spec_active_idx >= 0 && ms->spec_hidden)
            ? (ms->spec_hidden + (uint64_t)((size_t)ms->spec_active_idx * (size_t)d * 4))
            : ms->norm_buf;
        if (src && pc_download(ms->hidden_host, src, (size_t)d * 4, ms->stream) == PC_OK &&
            pc_sync_stream(ms->stream) == PC_OK) {
            ms->hidden_host_valid = 1;
        }
    }
    return s->hidden_host;
}
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

static int32_t sample_topk_sparse_cuda(const uint32_t *cnts, const uint32_t *ids, const float *vals,
                                       int n, const InkSampleParams *p, uint64_t *rng_state, int *used_fast) {
    *used_fast = 0;
    float temp = p ? p->temperature : 0.0f;
    if (temp <= 1e-6f) return -1;
    int top_k = (p && p->top_k > 0 && p->top_k < n) ? p->top_k : 40;
    float top_p = (p && p->top_p > 0.0f && p->top_p < 1.0f) ? p->top_p : 0.95f;
    int total = 0;
    for (int g = 0; g < 256; g++) {
        uint32_t c = cnts[g];
        if (c > 16u) return -1;
        total += (int)c;
    }
    if (total <= 0 || (total < top_k && temp > 0.8f)) return -1;
    int idx_stack[64];
    float val_stack[64];
    int *idx = (top_k <= 64) ? idx_stack : (int *)malloc((size_t)top_k * sizeof(int));
    float *val = (top_k <= 64) ? val_stack : (float *)malloc((size_t)top_k * sizeof(float));
    if (!idx || !val) { if (top_k > 64) { free(idx); free(val); } return -1; }
    int k = 0;
    float inv_t = 1.0f / temp;
    for (int g = 0; g < 256; g++) {
        uint32_t c = cnts[g];
        if (c == 0u) continue;
        int base = g * 16;
        for (uint32_t j0 = 0; j0 < c; j0++) {
            int id = (int)ids[base + (int)j0];
            float v = vals[base + (int)j0] * inv_t;
            if (k < top_k) {
                int j = k++;
                while (j > 0 && (val[j - 1] < v || (val[j - 1] == v && idx[j - 1] > id))) {
                    val[j] = val[j - 1]; idx[j] = idx[j - 1]; j--;
                }
                val[j] = v; idx[j] = id;
            } else if (v > val[top_k - 1] || (v == val[top_k - 1] && id < idx[top_k - 1])) {
                int j = top_k - 1;
                while (j > 0 && (val[j - 1] < v || (val[j - 1] == v && idx[j - 1] > id))) {
                    val[j] = val[j - 1]; idx[j] = idx[j - 1]; j--;
                }
                val[j] = v; idx[j] = id;
            }
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
    if (top_k > 64) { free(idx); free(val); }
    *used_fast = 1;
    return (int32_t)chosen;
}

int32_t ink_sample(const InkSession *s, const InkSampleParams *p, uint64_t *rng_state) {
    if (!s) return -1;
    int n = ink_vocab_unpadded(s);
    if (n <= 0) return -1;
    float temp = p ? p->temperature : 0.0f;
    if (temp <= 1e-6f && s->argmax_valid) {
        if (s->spec_active_idx >= 0) return (int32_t)s->spec_argmax_host[s->spec_active_idx];
        return (int32_t)s->argmax_host;
    }
    if (temp > 1e-6f && s->argmax_valid) {
        int used_fast = 0;
        const uint32_t *cnts = s->spec_active_idx >= 0 ? &s->spec_topk_cnt_host[s->spec_active_idx * 256] : s->topk_cnt_host;
        const uint32_t *ids  = s->spec_active_idx >= 0 ? &s->spec_topk_ids_host[s->spec_active_idx * 256 * 16] : s->topk_ids_host;
        const float    *vals = s->spec_active_idx >= 0 ? &s->spec_topk_vals_host[s->spec_active_idx * 256 * 16] : s->topk_vals_host;
        int32_t tok = sample_topk_sparse_cuda(cnts, ids, vals, n, p, rng_state, &used_fast);
        if (used_fast) return tok;
    }
    const float *logits = ink_logits(s);
    if (!logits) return -1;
    return sample_logits_ex_cuda(logits, n, p, rng_state, -1, NULL);
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
