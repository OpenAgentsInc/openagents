#include "inkling-metal.h"
#include "inkling-gguf.h"
#include "metal-host.h"

#include <dirent.h>
#include <fcntl.h>
#include <math.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/sysctl.h>
#include <time.h>
#include <unistd.h>

#define INK_T   64
#define INK_NT  256
#define INK_D   8
#define INK_MAX_BLOCKS 8
#define INK_PREFILL_B 3072
#define INK_MAX_SNAPS 8
#define INK_MAX_SWA_SNAPS 3
#define INK_SPEC_MAX_NB 8

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
    K_DV_F16_NORM,
    K_KV_ATTN_GQA_1PASS,
    K_CTRL_UP_APPLY_GF,
    K_CTRL_UP_APPLY_GF_3,
    K_CTRL_UP_APPLY_GF_3_NORM,
    K_MIX_RMSNORM_3,
    K_DV_Q4_QRGKV,
    K_DV_Q4_RMS,
    K_DV_F16_SMALL_RMS,
    K_CTRL_UP_APPLY_GF_BATCH,
    K_ATTN_SCORES_M4,
    K_ATTN_VALUES_M4,
    K_CTRL_UP_APPLY_GF_NORM,
    K_DV_Q4S_QRGKV_SDYN,
    K_DV_Q4S_CONV,
    K_DV_Q4S_RMS,
    K_DV_Q4S_SMALL_RMS,
    K_DV_Q4S_CTRL_DOWN_3,
    K_DEQUANT_Q4S_BF16,
    K_EMBED_Q8_INDIRECT,
    K_CTRL_UP_APPLY_GF_RM_NORM,
    K_CTRL_UP_APPLY_GF_RM_3_NORM,
    K_MOE_GU_Q4S,
    K_MOE_DOWN_Q4S,
    K_MOE_GU_IQ4S,
    K_MOE_DOWN_IQ4S,
    K_DV_Q4S_NB,
    K_DV_Q4S_MMA32_NB,
    K_DV_Q8_MMA32_NB,
    K_SCALE_RMS_PARTS_NB,
    K_DV_F16_SMALL_NB,
    K_POST_CONV_RES_NB,
    K_DV_Q4S_QRGKV_SDYN_NB,
    K_KV_ATTN_GQA_1PASS_NB,
    K_DV_Q4S_CONV_NB,
    K_MOE_PLAN_NB,
    K_MOE_GU_IQ4_NB,
    K_MOE_GU_Q4S_NB,
    K_MOE_GU_IQ4S_NB,
    K_MOE_GU_Q8_NB,
    K_MOE_DOWN_IQ4_NB,
    K_MOE_DOWN_Q4S_NB,
    K_MOE_DOWN_IQ4S_NB,
    K_MOE_DOWN_P1_IQ4S_NB,
    K_MOE_DOWN_P1_Q4S_NB,
    K_MOE_DOWN_P1_Q8_NB,
    K_MOE_DOWN_P2_NB,
    K_TOPK_FILTER,
    K_CONV_COMMIT_NB,
    K_DV_Q4S_SMALL_NB,
    K_MIX_RMSNORM_NB,
    K_MIX_CTRL_PREP_NB,
    K_CTRL_UP_APPLY_GF_RM_NB,
    K_CTRL_UP_APPLY_GF_RM_3_NB,
    K_RMSNORM_3_NB,
    K_DSPARK_MIX_NORM,
    K_DSPARK_KV_ABSORB,
    K_DSPARK_BLOCK_PREP,
    K_DSPARK_ADD_NORM_BF16,
    K_DSPARK_QKV_ROPE_ATTN,
    K_DSPARK_QKV_ROPE_ATTN_F32,
    K_DSPARK_SILU_MUL_BF16,
    K_DSPARK_SILU_MUL_F32,
    K_DSPARK_MARKOV_CTX_PREP,
    K_DSPARK_MARKOV_STEP,
    K_DSPARK_LOGITS_S1,
    K_DSPARK_FOLD_LOGITS_S1,
    K_DSPARK_LOGITS_S2,
    K_DV_Q8S_QRGKV_SDYN,
    K_DV_Q8S_CONV,
    K_DV_Q8S_RMS,
    K_DV_Q8S_SMALL_RMS,
    K_DV_Q8S_CTRL_DOWN_3,
    K_MOE_GU_Q8S,
    K_MOE_DOWN_Q8S,
    K_DV_Q8S_NB,
    K_DV_Q8S_QRGKV_SDYN_NB,
    K_DV_Q8S_CONV_NB,
    K_DV_Q8S_SMALL_NB,
    K_MOE_GU_Q8S_NB,
    K_MOE_DOWN_Q8S_NB,
    K_KV_ATTN_GQA3_SPLIT,
    K_KV_ATTN_GQA3_SPLIT_NB,
    K_ATTN_GQA3_REDUCE_NB,
    K_KV_STORE_BATCH_Q8,
    K_TOPK64_S1,
    K_TOPK64_S2,
    K_FLASH_ATTN_M4,
    K_MM_Q8S_BF16_16,
    K_MM_Q8S_BF16_64,
    K_MM_Q8S_BF16_128,
    K_MOE_MM_IQ4S_16,
    K_MOE_MM_IQ4S_64,
    K_MOE_MM_IQ4S_128,
    K_MOE_MM_IQ4S_256,
    K_MOE_MM_Q8S_16,
    K_MOE_MM_Q8S_64,
    K_MOE_MM_Q8S_128,
    K_SITU_BATCH_BF16_V2,
    K_MOE_GATHER_BF16_V2,
    K_MOE_SUM_GATHER_BATCH,
    K_DEQUANT_Q8S_BF16,
    K_EMBED_GGML,
    K_EMBED_GGML_INDIRECT,
    K_EMBED_GGML_BATCH,
    K_DV_GGML,
    K_MM_GGML,
    K_DEQUANT_GGML_BF16,
    K_DEQUANT_GGML_EXP_BF16,
    K_MOE_GU_GGML,
    K_MOE_DOWN_GGML,
    K_MOE_GU_GGML_NB,
    K_MOE_DOWN_P1_GGML_NB,
    K_MOE_DOWN_P1_Q8S_NB,
    K_ABLATE_DIR,
    K_POST_CONV_ABLATE_RES_NB,
    K_MOE_SUM_ABLATE_CONV_NB,
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
    "ink_dv_f16_norm",
    "ink_kv_attn_gqa_1pass",
    "ink_ctrl_up_apply_gf",
    "ink_ctrl_up_apply_gf_3",
    "ink_ctrl_up_apply_gf_3_norm",
    "ink_mix_rmsnorm_3",
    "ink_dv_q4_qrgkv",
    "ink_dv_q4_rms",
    "ink_dv_f16_small_rms",
    "ink_ctrl_up_apply_gf_batch",
    "ink_attn_scores_m4",
    "ink_attn_values_m4",
    "ink_ctrl_up_apply_gf_norm",
    "ink_dv_q4s_qrgkv_sdyn",
    "ink_dv_q4s_conv",
    "ink_dv_q4s_rms",
    "ink_dv_q4s_small_rms",
    "ink_dv_q4s_ctrl_down_3",
    "ink_dequant_q4s_bf16",
    "ink_embed_q8_indirect",
    "ink_ctrl_up_apply_gf_rm_norm",
    "ink_ctrl_up_apply_gf_rm_3_norm",
    "ink_moe_gu_q4s",
    "ink_moe_down_q4s",
    "ink_moe_gu_iq4s",
    "ink_moe_down_iq4s",
    "ink_dv_q4s_nb",
    "ink_dv_q4s_mma32_nb",
    "ink_dv_q8_mma32_nb",
    "ink_scale_rms_parts_nb",
    "ink_dv_f16_small_nb",
    "ink_post_conv_res_nb",
    "ink_dv_q4s_qrgkv_sdyn_nb",
    "ink_kv_attn_gqa_1pass_nb",
    "ink_dv_q4s_conv_nb",
    "ink_moe_plan_nb",
    "ink_moe_gu_iq4_nb",
    "ink_moe_gu_q4s_nb",
    "ink_moe_gu_iq4s_nb",
    "ink_moe_gu_q8_nb",
    "ink_moe_down_iq4_nb",
    "ink_moe_down_q4s_nb",
    "ink_moe_down_iq4s_nb",
    "ink_moe_down_p1_iq4s_nb",
    "ink_moe_down_p1_q4s_nb",
    "ink_moe_down_p1_q8_nb",
    "ink_moe_down_p2_nb",
    "ink_topk_filter",
    "ink_conv_commit_nb",
    "ink_dv_q4s_small_nb",
    "ink_mix_rmsnorm_nb",
    "ink_mix_ctrl_prep_nb",
    "ink_ctrl_up_apply_gf_rm_nb",
    "ink_ctrl_up_apply_gf_rm_3_nb",
    "ink_rmsnorm_3_nb",
    "ink_dspark_mix_norm",
    "ink_dspark_kv_absorb",
    "ink_dspark_block_prep",
    "ink_dspark_add_norm_bf16",
    "ink_dspark_qkv_rope_attn",
    "ink_dspark_qkv_rope_attn_f32",
    "ink_dspark_silu_mul_bf16",
    "ink_dspark_silu_mul_f32",
    "ink_dspark_markov_ctx_prep",
    "ink_dspark_markov_step",
    "ink_dspark_logits_s1",
    "ink_dspark_fold_logits_s1",
    "ink_dspark_logits_s2",
    "ink_dv_q8s_qrgkv_sdyn",
    "ink_dv_q8s_conv",
    "ink_dv_q8s_rms",
    "ink_dv_q8s_small_rms",
    "ink_dv_q8s_ctrl_down_3",
    "ink_moe_gu_q8s",
    "ink_moe_down_q8s",
    "ink_dv_q8s_nb",
    "ink_dv_q8s_qrgkv_sdyn_nb",
    "ink_dv_q8s_conv_nb",
    "ink_dv_q8s_small_nb",
    "ink_moe_gu_q8s_nb",
    "ink_moe_down_q8s_nb",
    "ink_kv_attn_gqa3_split",
    "ink_kv_attn_gqa3_split_nb",
    "ink_attn_gqa3_reduce_nb",
    "ink_kv_store_batch_q8",
    "ink_topk64_s1",
    "ink_topk64_s2",
    "ink_flash_attn_m4",
    "ink_mm_q8s_bf16_16",
    "ink_mm_q8s_bf16_64",
    "ink_mm_q8s_bf16_128",
    "ink_moe_mm_iq4s_16",
    "ink_moe_mm_iq4s_64",
    "ink_moe_mm_iq4s_128",
    "ink_moe_mm_iq4s_256",
    "ink_moe_mm_q8s_16",
    "ink_moe_mm_q8s_64",
    "ink_moe_mm_q8s_128",
    "ink_situ_batch_bf16_v2",
    "ink_moe_gather_bf16_v2",
    "ink_moe_sum_gather_batch",
    "ink_dequant_q8s_bf16",
    "ink_embed_ggml",
    "ink_embed_ggml_indirect",
    "ink_embed_ggml_batch",
    "ink_dv_ggml",
    "ink_mm_ggml",
    "ink_dequant_ggml_bf16",
    "ink_dequant_ggml_exp_bf16",
    "ink_moe_gu_ggml",
    "ink_moe_down_ggml",
    "ink_moe_gu_ggml_nb",
    "ink_moe_down_p1_ggml_nb",
    "ink_moe_down_p1_q8s_nb",
    "ink_ablate_dir",
    "ink_post_conv_ablate_res_nb",
    "ink_moe_sum_ablate_conv_nb",
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
    128,    /* K_DV_F16_NORM */
    256,    /* K_KV_ATTN_GQA_1PASS */
    32,     /* K_CTRL_UP_APPLY_GF */
    32,     /* K_CTRL_UP_APPLY_GF_3 */
    32,     /* K_CTRL_UP_APPLY_GF_3_NORM */
    INK_NT, /* K_MIX_RMSNORM_3 */
    128,    /* K_DV_Q4_QRGKV */
    128,    /* K_DV_Q4_RMS */
    128,    /* K_DV_F16_SMALL_RMS */
    128,    /* K_CTRL_UP_APPLY_GF_BATCH */
    128,    /* K_ATTN_SCORES_M4 */
    128,    /* K_ATTN_VALUES_M4 */
    32,     /* K_CTRL_UP_APPLY_GF_NORM */
    128,    /* K_DV_Q4S_QRGKV_SDYN */
    128,    /* K_DV_Q4S_CONV */
    128,    /* K_DV_Q4S_RMS */
    128,    /* K_DV_Q4S_SMALL_RMS */
    128,    /* K_DV_Q4S_CTRL_DOWN_3 */
    96,     /* K_DEQUANT_Q4S_BF16 */
    INK_NT, /* K_EMBED_Q8_INDIRECT */
    128,    /* K_CTRL_UP_APPLY_GF_RM_NORM */
    128,    /* K_CTRL_UP_APPLY_GF_RM_3_NORM */
    128,    /* K_MOE_GU_Q4S */
    128,    /* K_MOE_DOWN_Q4S */
    128,    /* K_MOE_GU_IQ4S */
    128,    /* K_MOE_DOWN_IQ4S */
    128,    /* K_DV_Q4S_NB */
    128,    /* K_DV_Q4S_MMA32_NB */
    128,    /* K_DV_Q8_MMA32_NB */
    256,    /* K_SCALE_RMS_PARTS_NB */
    128,    /* K_DV_F16_SMALL_NB */
    64,     /* K_POST_CONV_RES_NB */
    128,    /* K_DV_Q4S_QRGKV_SDYN_NB */
    256,    /* K_KV_ATTN_GQA_1PASS_NB */
    128,    /* K_DV_Q4S_CONV_NB */
    256,    /* K_MOE_PLAN_NB */
    128,    /* K_MOE_GU_IQ4_NB */
    128,    /* K_MOE_GU_Q4S_NB */
    128,    /* K_MOE_GU_IQ4S_NB */
    128,    /* K_MOE_GU_Q8_NB */
    128,    /* K_MOE_DOWN_IQ4_NB */
    128,    /* K_MOE_DOWN_Q4S_NB */
    128,    /* K_MOE_DOWN_IQ4S_NB */
    128,    /* K_MOE_DOWN_P1_IQ4S_NB */
    128,    /* K_MOE_DOWN_P1_Q4S_NB */
    128,    /* K_MOE_DOWN_P1_Q8_NB */
    64,     /* K_MOE_DOWN_P2_NB */
    128,    /* K_TOPK_FILTER */
    1,      /* K_CONV_COMMIT_NB */
    128,    /* K_DV_Q4S_SMALL_NB */
    INK_NT, /* K_MIX_RMSNORM_NB */
    INK_NT, /* K_MIX_CTRL_PREP_NB */
    128,    /* K_CTRL_UP_APPLY_GF_RM_NB */
    128,    /* K_CTRL_UP_APPLY_GF_RM_3_NB */
    INK_NT, /* K_RMSNORM_3_NB */
    256,    /* K_DSPARK_MIX_NORM */
    64,     /* K_DSPARK_KV_ABSORB */
    256,    /* K_DSPARK_BLOCK_PREP */
    256,    /* K_DSPARK_ADD_NORM_BF16 */
    256,    /* K_DSPARK_QKV_ROPE_ATTN */
    256,    /* K_DSPARK_QKV_ROPE_ATTN_F32 */
    256,    /* K_DSPARK_SILU_MUL_BF16 */
    256,    /* K_DSPARK_SILU_MUL_F32 */
    256,    /* K_DSPARK_MARKOV_CTX_PREP */
    256,    /* K_DSPARK_MARKOV_STEP */
    256,    /* K_DSPARK_LOGITS_S1 */
    256,    /* K_DSPARK_FOLD_LOGITS_S1 */
    64,     /* K_DSPARK_LOGITS_S2 */
    128,    /* K_DV_Q8S_QRGKV_SDYN */
    128,    /* K_DV_Q8S_CONV */
    128,    /* K_DV_Q8S_RMS */
    128,    /* K_DV_Q8S_SMALL_RMS */
    128,    /* K_DV_Q8S_CTRL_DOWN_3 */
    128,    /* K_MOE_GU_Q8S */
    128,    /* K_MOE_DOWN_Q8S */
    128,    /* K_DV_Q8S_NB */
    128,    /* K_DV_Q8S_QRGKV_SDYN_NB */
    128,    /* K_DV_Q8S_CONV_NB */
    128,    /* K_DV_Q8S_SMALL_NB */
    128,    /* K_MOE_GU_Q8S_NB */
    128,    /* K_MOE_DOWN_Q8S_NB */
    256,    /* K_KV_ATTN_GQA3_SPLIT */
    256,    /* K_KV_ATTN_GQA3_SPLIT_NB */
    128,    /* K_ATTN_GQA3_REDUCE_NB */
    128,    /* K_KV_STORE_BATCH_Q8 */
    256,    /* K_TOPK64_S1 */
    256,    /* K_TOPK64_S2 */
    128,    /* K_FLASH_ATTN_M4 */
    128,    /* K_MM_Q8S_BF16_16 */
    128,    /* K_MM_Q8S_BF16_64 */
    128,    /* K_MM_Q8S_BF16_128 */
    128,    /* K_MOE_MM_IQ4S_16 */
    128,    /* K_MOE_MM_IQ4S_64 */
    128,    /* K_MOE_MM_IQ4S_128 */
    128,    /* K_MOE_MM_IQ4S_256 */
    128,    /* K_MOE_MM_Q8S_16 */
    128,    /* K_MOE_MM_Q8S_64 */
    128,    /* K_MOE_MM_Q8S_128 */
    128,    /* K_SITU_BATCH_BF16_V2 */
    128,    /* K_MOE_GATHER_BF16_V2 */
    128,    /* K_MOE_SUM_GATHER_BATCH */
    96,     /* K_DEQUANT_Q8S_BF16 */
    INK_NT, /* K_EMBED_GGML */
    INK_NT, /* K_EMBED_GGML_INDIRECT */
    INK_NT, /* K_EMBED_GGML_BATCH */
    128,    /* K_DV_GGML */
    128,    /* K_MM_GGML */
    128,    /* K_DEQUANT_GGML_BF16 */
    128,    /* K_DEQUANT_GGML_EXP_BF16 */
    128,    /* K_MOE_GU_GGML */
    128,    /* K_MOE_DOWN_GGML */
    128,    /* K_MOE_GU_GGML_NB */
    128,    /* K_MOE_DOWN_P1_GGML_NB */
    128,    /* K_MOE_DOWN_P1_Q8S_NB */
    INK_NT, /* K_ABLATE_DIR */
    INK_NT, /* K_POST_CONV_ABLATE_RES_NB */
    INK_NT, /* K_MOE_SUM_ABLATE_CONV_NB */
};

typedef struct {
    int64_t off;   /* byte offset in fast_buf (-1 if unused) */
    int64_t words; /* u16 element count */
} FastSpan;

typedef struct {
    FastSpan qs; /* 64B-aligned packed nibbles (u32 words = qs.words / 2) */
    FastSpan ds; /* 64B-aligned separate half scales (u16 words = ds.words) */
} FastQ4S;

typedef struct {
    FastSpan attn_q, attn_k, attn_v, attn_output, attn_gate, attn_rel;
    FastSpan attn_qrg; /* contiguous Q + R + Gate in Q4_0 for fused 1-dispatch Wave 3! */
    FastQ4S attn_qrgkv_s; /* Sprint 68: contiguous Q4_B32_SPLIT [Q|R|G|K|V|sinks|(k_dyn|v_dyn)] */
    FastQ4S attn_q_s, attn_rel_s, attn_gate_s, attn_k_s, attn_v_s, attn_output_s;
    FastQ4S attn_sink_s, attn_k_dyn_s, attn_v_dyn_s;
    FastQ4S ctrl_down_qkv_s, ctrl_down3_s, ffn_gate_inp_s;
    FastQ4S moe_gu_q4s, moe_dn_q4s; /* Sprint 70: Q4_B32_SPLIT MoE for Q8_0 MoE layers! */
    FastQ4S moe_gu_iq4s, moe_dn_iq4s; /* Sprint 71C: 100% lossless aligned IQ4S MoE (in iq4s_buf)! */
    FastSpan ffn_gate_inp, ffn_gate_inp_normed;
    FastSpan attn_sink_proj, attn_k_conv_proj, attn_v_conv_proj;
    FastSpan moe_gu_q4, moe_dn_q4;
    FastSpan ctrl_down[4], ctrl_gain[4], ctrl_forget[4];
    FastSpan ctrl_down_qkv_q4, ctrl_down3_q4, ctrl_gain_qkv, ctrl_forget_qkv;
    FastSpan ctrl_gf_qkv, ctrl_gf3; /* col-major [rank, dim] interleaved half2 for prefill */
    FastSpan ctrl_gf_rm_qkv, ctrl_gf_rm3; /* Sprint 70: row-major [dim, rank] half2 for 256t decode! */
} FastLayer;

typedef struct {
    int64_t off;    /* byte offset in d->mtp_buf (-1 if missing) */
    int64_t bytes;  /* byte length */
} MtpSpan;

typedef struct {
    MtpSpan dflare_norm;     /* [2304] bf16 */
    MtpSpan input_ln;        /* [2304] bf16 */
    MtpSpan post_attn_ln;    /* [2304] bf16 */
    MtpSpan q_norm;          /* [128] bf16 */
    MtpSpan k_norm;          /* [128] bf16 */
    MtpSpan k_norm_ctx;      /* [128] bf16 */
    MtpSpan q_proj;          /* [3072, 2304] bf16 */
    MtpSpan k_proj;          /* [1024, 2304] bf16 */
    MtpSpan v_proj;          /* [1024, 2304] bf16 */
    MtpSpan k_proj_ctx;      /* [1024, 2304] bf16 */
    MtpSpan v_proj_ctx;      /* [1024, 2304] bf16 */
    MtpSpan o_proj;          /* [2304, 3072] bf16 */
    MtpSpan gate_proj;       /* [9216, 2304] bf16 */
    MtpSpan up_proj;         /* [9216, 2304] bf16 */
    MtpSpan down_proj;       /* [2304, 9216] bf16 */
    FastQ4S qkv_s;           /* [5120, 2304] Q4S in draft_q4s_buf */
    FastQ4S o_s;             /* [2304, 3072] Q4S in draft_q4s_buf */
    FastQ4S gu_s;            /* [18432, 2304] Q4S in draft_q4s_buf */
    FastQ4S dn_s;            /* [2304, 9216] Q4S in draft_q4s_buf */
    int is_swa;              /* 1 for layers 0 & 2, 0 for layer 1 */
    int kv_cap;
    PmBuffer kv_k;           /* [kv_cap * 1024] bf16 (u16) */
    PmBuffer kv_v;           /* [kv_cap * 1024] bf16 (u16) */
} DsparkLayer;

typedef struct {
    int ready;
    int use_q4s_draft;
    int fd;
    void *map;
    size_t map_size;
    PmBuffer mtp_buf;        /* zero-copy pm_wrap of mtp.safetensors */
    PmBuffer draft_q4s_buf;  /* quantized Q4S 3x DraftLayer weights (139 MB vs 495 MB BF16) */
    MtpSpan mask_embed;      /* [2304] bf16 */
    MtpSpan norm_w;          /* [2304] bf16 */
    MtpSpan mh_dflare_norm;  /* [2304] bf16 */
    MtpSpan mh_markov_w1;    /* [201024, 256] bf16 */
    MtpSpan mh_rnn_bias;     /* [768] bf16 */
    MtpSpan mh_hidden_w;     /* [2304, 256] bf16 */
    MtpSpan mh_hidden_b;     /* [2304] bf16 */
    MtpSpan mh_rnn_w;        /* [768, 5120] bf16 */
    MtpSpan fuse_weight;     /* [4, 4] bf16 */
    float fuse_alpha[4][4];  /* softmax(fuse_weight, axis=-1) */
    DsparkLayer layers[3];

    PmBuffer inv_freq;       /* [64] f32 RoPE inverse frequencies (theta=1e6) */
    PmBuffer rnn_w_step;     /* [768 * 512] bf16 (u16) */
    PmBuffer rnn_w_ctx;      /* [768 * 4608] bf16 (u32 pairs) */
    PmBuffer fold_w_qs;      /* [nv * 8 * 4] u32 (Q4S packed nibbles for [nv, 256]) */
    PmBuffer fold_w_ds;      /* [nv * 8] u16 (Q4S scales for [nv, 256]) */
    PmBuffer fold_b;         /* [nv] f32 = lm_head(hidden_out.bias) */

    PmBuffer taps;           /* [4 * prefill_b * 2304] f32 */
    PmBuffer last_taps;      /* [4 * 2304] f32 */
    PmBuffer ctx_bf16;       /* [prefill_b * 2304] bf16 (u16) */
    PmBuffer ctx_k_f32;      /* [prefill_b * 1024] f32 */
    PmBuffer ctx_v_f32;      /* [prefill_b * 1024] f32 */

    PmBuffer anchor_emb;     /* [2304] f32 */
    PmBuffer f_rnn_bf16;     /* [2304] bf16 (u16) */
    PmBuffer res;            /* [8 * 2304] f32 */
    PmBuffer x_bf16;         /* [8 * 2304] bf16 (u16) */
    PmBuffer x_f32;          /* [8 * 2304] f32 */
    PmBuffer q_f32;          /* [8 * 3072] f32 */
    PmBuffer k_f32;          /* [8 * 1024] f32 */
    PmBuffer v_f32;          /* [8 * 1024] f32 */
    PmBuffer qkv_f32;        /* [8 * 5120] f32 packed Q|K|V for Q4S draft */
    PmBuffer attn_bf16;      /* [8 * 3072] bf16 (u16) */
    PmBuffer attn_f32;       /* [8 * 3072] f32 */
    PmBuffer tmp_f32;        /* [8 * 2304] f32 */
    PmBuffer gate_f32;       /* [8 * 9216] f32 */
    PmBuffer up_f32;         /* [8 * 9216] f32 */
    PmBuffer gu_f32;         /* [8 * 18432] f32 packed gate|up for Q4S draft */
    PmBuffer mlp_bf16;       /* [8 * 9216] bf16 (u16) */
    PmBuffer mlp_f32;        /* [8 * 9216] f32 */
    PmBuffer markov_in_bf16; /* [7 * 4608] bf16 (u16) */
    PmBuffer rnn_ctx;        /* [7 * 768] f32 */
    PmBuffer rnn_state;      /* [256] f32 */
    PmBuffer rnn_state2;     /* [256] f32 ping-pong buffer for 32-TG markov_step */
    PmBuffer rnn_latent;     /* [256] f32 */
    PmBuffer base_logits;    /* [7 * nv] f32 */
    PmBuffer delta_logits;   /* [nv] f32 */
    PmBuffer s1_m;           /* [64] f32 */
    PmBuffer s1_i;           /* [64] u32 */
    PmBuffer s1_s;           /* [64] f32 */
    PmBuffer draft_toks;     /* [8] u32 */
    PmBuffer draft_probs;    /* [8] f32 */

    int pos;
    int pending_pos0;
    int pending_nb;
    int pending_got;
    int skip_countdown;
    int req_disabled;
    int req_rounds;
    int skip_mult;
    int consec_unprof;
    float ema_acc;
    float util_ema;
    double b1_ema_s;
    double last_draft_s;
    double last_verify_s;
    float ema_draft_ms;
    float ema_cal;
    float ema_net_speedup;
    float last_draft_ms;
    int rounds;
    int drafted;
    int accepted;
    double draft_gpu_s;
} DsparkState;

struct InkSession {
    InkGGUF gguf;
    InkLora lora;
    int has_ablate;
    int ablate_has_emb;
    uint8_t ablate_has_layer[32];
    PmBuffer ablate_dirs;  /* [(1 + n_phys) * n_embd] f32: slot 0 = tok_embd, slot 1+ip = layer ip */
    PmBuffer ggml_grids;   /* [INK_GRID_TOTAL_U32] u32 universal GGML IQ lookup tables */
    PmModule module;
    PmKernel kernels[K_COUNT];
    PmBuffer weights_buf;
    PmBuffer fast_buf;
    PmBuffer iq4s_buf;     /* Sprint 71C: 100% lossless aligned IQ4S MoE weights */
    FastLayer fast_layers[INK_MAX_SLOTS];
    FastSpan fast_output;
    FastQ4S fast_output_s;
    int fast_is_q8s;       /* 1 when *_s spans store lossless 32B/block Q8S instead of 16B/block Q4S */
    int fast_output_is_q4s;/* 1 when fast_output_s stores 16B/block Q4S (INK_FAST_Q4=1 or INK_Q4_LM_HEAD=1) */

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
    PmBuffer logits_buf[2];     /* [vocab_size] f32 double-buffered for 2-in-flight GPU chaining */
    PmBuffer hidden_out[2];     /* [n_embd] f32 double-buffered decode output hidden state */
    PmBuffer argmax_vals;       /* [64] f32 */
    PmBuffer argmax_ids;        /* [64] u32 */
    PmBuffer argmax_out_buf[2]; /* [1] u32 double-buffered GPU argmax */
    PmBuffer topk_s1_vals;      /* [8 * 32 * 64] f32 Sprint D GPU exact Top-64 stage 1 */
    PmBuffer topk_s1_ids;       /* [8 * 32 * 64] u32 */
    PmBuffer topk_vals[2];      /* [64] f32 sorted top-64 values per decode slot */
    PmBuffer topk_ids[2];       /* [64] u32 sorted top-64 token ids per decode slot */
    PmBuffer spec_topk_vals;    /* [8 * 64] f32 sorted top-64 for B=2..8 verify slots */
    PmBuffer spec_topk_ids;     /* [8 * 64] u32 */
    int sampled_mode;           /* 1 when ink_sample called with temp > 1e-6f */
    float ema_base_ms;          /* Ch 102 online EMA of B=1 decode step GPU time (ms) */
    float ema_vratio[9];        /* Ch 102 online EMA of t_verify(nb) / ema_base_ms for nb=2..8 */
    float last_verify_ms;
    int spec_round_open;        /* sampled-mode deferred telemetry for open spec_q */
    int spec_round_dspark;
    int spec_round_m;
    int spec_round_match_n;
    int spec_round_hits;
    float spec_round_pred;
    float spec_round_cost_ms;
    int active_out_slot;
    int hidden_is_prefill;
    int argmax_valid;
    int topk_valid;
    PmBuffer dummy_f32;         /* [64] zero f32 */
    PmBuffer dummy_ones;        /* [8] 1.0f32 for SWA nb tau_buf */
    PmBuffer rms_parts;         /* [3 * 128] f32 deferred RMSNorm tile sum-of-squares */
    /* Metal 4 double-buffered lookahead pre-encoded next decode step (non-greedy) */
    PmBatch prep_batch;
    int prep_pos;
    int prep_logits;
    int prep_slot;
    int next_m4_slot;
    /* Sprint 69: 2-in-flight GPU-to-GPU chained greedy decode (zero CPU wakeup gap!) */
    int inflight_active;
    uint64_t inflight_ticket;
    int inflight_slot;
    int inflight_pos;
    int inflight_logits;
    int32_t inflight_expected_tok;
    int has_prev_argmax;
    int32_t prev_argmax;
    int greedy_streak;
    /* Sprint 71B: 100% lossless bit-exact tiny-batch (B=2..8) grouped-MoE speculative verify + adaptive PLD */
    PmBuffer conv_raw_log;      /* [n_slots * 8 * (2*n_kv + 2*d)] f32 read-only verify raw log */
    int64_t raw_slot_stride;
    PmBuffer spec_u_eids;       /* [64] u32 */
    PmBuffer spec_u_mask;       /* [64] u32 */
    PmBuffer spec_u_slot;       /* [64 * 8] u32 */
    PmBuffer spec_u_cnt;        /* [4] u32 (cnt[0]=nu, cnt[1..3]={nu*gpe, 1, 1} for indirect dispatch) */
    PmBuffer spec_logits;       /* [8 * n_vocab] f32 */
    PmBuffer spec_hidden;       /* [8 * n_embd] f32 */
    PmBuffer spec_argmax_out;   /* [8] u32 */
    int can_verify_nb;
    int32_t *tok_hist;          /* [max_ctx] exact prompt + generated token history */
    int tok_hist_len;
    int32_t *pred_toks;         /* optional explicit predicted-output tokens (Cursor Instant Apply / OpenAI prediction) */
    int pred_len;
    int pred_cap;
    int32_t *sess_hist;         /* cross-turn output history ring buffer (SuffixDecoding-lite, 16K tokens) */
    int sess_hist_len;
    int sess_hist_cap;
    int32_t spec_q[8];          /* pre-verified draft tokens (indices 1..spec_tail-1) */
    int32_t spec_argmax[8];     /* exact argmax for verified slots 0..spec_tail-1 */
    int spec_head;              /* next slot in spec_q to consume on ink_step */
    int spec_tail;              /* number of verified tokens in this batch (1..8) */
    int spec_active_idx;        /* -1 = normal logits_buf; 0..7 = index into spec_logits/hidden/argmax */
    int conv_pending_pos0;      /* starting pos0 of uncommitted conv_raw_log */
    int conv_pending_n;         /* number of consumed tokens (0..8) pending K_CONV_COMMIT_NB */
    float pld_ema2;             /* EMA acceptance rate for n=2 bigram matches */
    float pld_ema3;             /* EMA acceptance rate for n=3 trigram matches */
    int spec_rounds;
    int spec_drafted;
    int spec_accepted;
    DsparkState dspark;

    /* Per logical slot recurrent shortconv state (packed contiguous for 1-copy backup) and KV cache */
    int n_slots;
    int max_ctx;
    int prefill_b;
    int kv_q8;
    int64_t kv_row_u16;
    uint64_t model_fp;            /* Ch 176 DiskPromptCache model + KV-format fingerprint */
    size_t attn_scores_bytes;
    InkMemoryBudget budget;
    int kv_cap[INK_MAX_SLOTS];
    PmBuffer kv_k[INK_MAX_SLOTS]; /* [kv_cap * n_kv_elem] u16 */
    PmBuffer kv_v[INK_MAX_SLOTS]; /* [kv_cap * n_kv_elem] u16 */
    PmBuffer conv_all;            /* [n_slots * (2*n_kv_elem + 2*n_embd) * 3] f32 */
    PmBuffer conv_backup;         /* [n_slots * (2*n_kv_elem + 2*n_embd) * 3] f32 */
    int64_t conv_slot_stride;
    int64_t conv_all_elems;

    int pos;
    int max_pos_written;
    int swa_cap_min;
    int swa_valid_from;
    int dspark_live_len;
    int dspark_swa_valid_from;
    int64_t last_hidden_off;
    int32_t *live_tokens;
    int live_len;
    /* Lazy-allocated scratch for batched speculative draft verification (ink_verify_draft). */
    PmBuffer verify_logits; /* [INK_SPEC_MAX_NB * n_vocab] f32 */
    PmBuffer spec_hist;     /* [n_slots * INK_SPEC_MAX_NB * (2*n_kv + 2*d)] f32 */
    float   *spec_old_conv; /* [conv_all_elems] f32 */
    /* Exact cross-request KV + shortconv prefix snapshots (LRU + pinned SWA-backed checkpoints). */
    uint64_t snap_clock;
    struct {
        int valid;
        int pos;
        int has_logits;
        int pinned;
        uint64_t last_used;
        int32_t *tokens;
        float   *conv_state;
        float   *logits;
        uint16_t *swa_kv;
        int swa_win;
        int dspark_pos;
        float   *dspark_last_taps;
        uint16_t *dspark_swa_kv;
        int dspark_swa_win;
    } snaps[INK_MAX_SNAPS];
    /* Ch 176 async background NVMe writer for DiskPromptCache (zero TTFT/decode stall) */
    pthread_t disk_thr;
    pthread_mutex_t disk_mu;
    pthread_cond_t disk_cv;
    int disk_thr_started;
    int disk_stop;
    struct {
        char dir[512];
        char final_path[640];
        char tmp_path[680];
        uint8_t *payload;
        size_t payload_bytes;
    } disk_q[4];
    int disk_q_head;
    int disk_q_tail;
    int disk_q_len;
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
static PmArg a_tensor_ggml_u16(InkSession *s, const InkTensor *t) {
    return a_buf(s->weights_buf, (int64_t)t->offset, (int64_t)((t->nbytes + 1) / 2));
}
static PmArg a_ggml_grids(const InkSession *s) {
    return a_u32v(s->ggml_grids, 0, INK_GRID_TOTAL_U32);
}
static PmArg a_ablate_dirs(const InkSession *s) {
    if (s->ablate_dirs) {
        return a_f32v(s->ablate_dirs, 0, (int64_t)(1 + s->gguf.hp.n_phys) * s->gguf.hp.n_embd);
    }
    return a_f32v(s->dummy_f32, 0, 64);
}
static PmArg a_conv_k(const InkSession *s, int il) {
    int64_t n_kv = s->gguf.hp.n_head_kv * s->gguf.hp.head_dim;
    return a_f32v(s->conv_all, (int64_t)il * s->conv_slot_stride, n_kv * 4);
}
static PmArg a_conv_v(const InkSession *s, int il) {
    int64_t n_kv = s->gguf.hp.n_head_kv * s->gguf.hp.head_dim;
    return a_f32v(s->conv_all, (int64_t)il * s->conv_slot_stride + n_kv * 4, n_kv * 4);
}
static PmArg a_conv_ao(const InkSession *s, int il) {
    int64_t n_kv = s->gguf.hp.n_head_kv * s->gguf.hp.head_dim;
    int64_t d = s->gguf.hp.n_embd;
    return a_f32v(s->conv_all, (int64_t)il * s->conv_slot_stride + 2 * n_kv * 4, d * 3);
}
static PmArg a_conv_fo(const InkSession *s, int il) {
    int64_t n_kv = s->gguf.hp.n_head_kv * s->gguf.hp.head_dim;
    int64_t d = s->gguf.hp.n_embd;
    return a_f32v(s->conv_all, (int64_t)il * s->conv_slot_stride + 2 * n_kv * 4 + d * 3, d * 3);
}
static PmArg a_raw_k(const InkSession *s, int il) {
    int64_t n_kv = s->gguf.hp.n_head_kv * s->gguf.hp.head_dim;
    return a_f32v(s->conv_raw_log, (int64_t)il * s->raw_slot_stride, 8 * n_kv);
}
static PmArg a_raw_v(const InkSession *s, int il) {
    int64_t n_kv = s->gguf.hp.n_head_kv * s->gguf.hp.head_dim;
    return a_f32v(s->conv_raw_log, (int64_t)il * s->raw_slot_stride + 8 * n_kv, 8 * n_kv);
}
static PmArg a_raw_ao(const InkSession *s, int il) {
    int64_t n_kv = s->gguf.hp.n_head_kv * s->gguf.hp.head_dim;
    int64_t d = s->gguf.hp.n_embd;
    return a_f32v(s->conv_raw_log, (int64_t)il * s->raw_slot_stride + 16 * n_kv, 8 * d);
}
static PmArg a_raw_fo(const InkSession *s, int il) {
    int64_t n_kv = s->gguf.hp.n_head_kv * s->gguf.hp.head_dim;
    int64_t d = s->gguf.hp.n_embd;
    return a_f32v(s->conv_raw_log, (int64_t)il * s->raw_slot_stride + 16 * n_kv + 8 * d, 8 * d);
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
    } else if (ink_ggml_type_supported(w->type)) {
        CALL(s, batch, K_DV_GGML, (rows + 3) / 4, a_int(rows), a_int(cols), a_int(add), a_int((int64_t)w->type),
             a_tensor_ggml_u16(s, w), a_ggml_grids(s), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
    } else {
        snprintf(s->error, sizeof s->error, "unsupported weight type %u (%s) for matvec", w->type, ink_ggml_type_name(w->type));
        return 1;
    }
    return 0;
}

static PmArg a_fast_u16(InkSession *s, const FastSpan *fs) {
    return a_buf(s->fast_buf, fs->off, fs->words);
}
static PmArg a_fast_u32(InkSession *s, const FastSpan *fs) {
    return a_buf(s->fast_buf, fs->off, fs->words / 2);
}
static PmArg a_iq4s_u16(InkSession *s, const FastSpan *fs) {
    return a_buf(s->iq4s_buf, fs->off, fs->words);
}
static PmArg a_iq4s_u32(InkSession *s, const FastSpan *fs) {
    return a_buf(s->iq4s_buf, fs->off, fs->words / 2);
}

static int dispatch_mv_q4(InkSession *s, PmBatch batch, const InkTensor *w, const FastSpan *fs, int64_t rows, int64_t cols, int add,
                          PmBuffer x_buf, int64_t x_off, PmBuffer out_buf, int64_t out_off) {
    if (s->fast_buf && fs && fs->off >= 0) {
        CALL(s, batch, K_DV_Q4, (rows + 7) / 8, a_int(rows), a_int(cols), a_int(add),
             a_fast_u16(s, fs), a_f32v(x_buf, x_off, cols), a_f32v(out_buf, out_off, rows));
        return 0;
    }
    return dispatch_mv(s, batch, w, rows, cols, add, x_buf, x_off, out_buf, out_off);
}

static int dispatch_mv_f16_small(InkSession *s, PmBatch batch, const InkTensor *w, const FastSpan *fs, int64_t rows, int64_t cols, int add,
                                 PmBuffer x_buf, int64_t x_off, PmBuffer out_buf, int64_t out_off) {
    if (s->fast_buf && fs && fs->off >= 0) {
        CALL(s, batch, K_DV_F16_SMALL, rows, a_int(rows), a_int(cols), a_int(add),
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

static int64_t ink_choose_attn_splits(const InkSession *s, int64_t n_keys, int64_t nb) {
    int64_t n_head = s->gguf.hp.n_head;
    int64_t sp = (n_keys <= 384) ? 1 : (n_keys <= 1024) ? 8 : (n_keys <= 4096) ? 16 : 32;
    int64_t max_sp = (int64_t)(s->attn_scores_bytes / (size_t)(nb * n_head * 132 * 4));
    if (max_sp > 32) max_sp = 32;
    if (sp > max_sp) sp = max_sp;
    if (sp < 1) sp = 1;
    return sp;
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

    /* Wave 1: depth-attention mix (+ fused QKV ctrl_prep when pass_messages precomputed!).
     * Sprint 63: attn_output updates cur in-place -> delete all 48 K_COPY(cur -> inp_sa);
     * only the 8 block-boundary layers copy cur -> blocks[*n_blocks] directly in Wave 1! */
    int attn_o_inplace = (L->attn_output.type == INK_GGML_Q8_0);
    int is_blk = (ip % hp->block_size == 0);
    if (!attn_o_inplace) {
        CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->inp_sa, 0, d));
    } else if (is_blk) {
        if (*n_blocks >= INK_MAX_BLOCKS) {
            snprintf(s->error, sizeof s->error, "block count overflow");
            return 1;
        }
        CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->cur, 0, d),
             a_f32v(s->blocks, (int64_t)(*n_blocks) * d, d));
    }
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
    int fused_full_norm = (L->attn_res_query.present && !is_swa && !has_messages);
    int fused_qkv_prep = (L->attn_res_query.present && precomp_msgs);
    if (fused_swa_norm) {
        CALL(s, batch, K_MIX_RMSNORM, 1, a_int(d), a_int(*n_blocks), a_f32(hp->rms_eps),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &L->attn_res_query),
             a_tensor_f32(s, &L->attn_norm),
             a_f32v(s->inp_q, 0, d));
    } else if (fused_full_norm) {
        CALL(s, batch, K_MIX_RMSNORM_3, 3, a_int(d), a_int(*n_blocks), a_f32(hp->rms_eps),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &L->attn_res_query),
             a_tensor_f32(s, &L->attn_q_inp_norm),
             a_tensor_f32(s, &L->attn_k_inp_norm),
             a_tensor_f32(s, &L->attn_v_inp_norm),
             a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d));
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
    int has_k_dyn = (!is_swa && L->attn_k_conv_proj.present);
    int has_v_dyn = (!is_swa && L->attn_v_conv_proj.present);
    int64_t citems = (d + INK_T - 1) / INK_T;
    int64_t citems_rm = (d + 31) / 32;
    int64_t qkv_rms_parts = citems;
    int deferred_rms = 0;
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
        int use_mma_b1 = (s->can_verify_nb && getenv("INK_NO_MMA32") == NULL);
        if (s->fast_buf && FL->ctrl_down_qkv_s.qs.off >= 0) {
            CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_CTRL_DOWN_3 : K_DV_Q4S_CTRL_DOWN_3, 3 * rank, a_int(rank), a_int(2 * d),
                 a_fast_u32(s, &FL->ctrl_down_qkv_s.qs), a_fast_u16(s, &FL->ctrl_down_qkv_s.ds),
                 a_f32v(s->ctrl_cat, 0, 6 * d), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->ctrl_gate, 0, 3 * rank));
        } else if (use_mma_b1 && s->fast_buf && FL->ctrl_down_qkv_q4.off >= 0) {
            CALL(s, batch, K_DV_F16_SMALL_NB, 3 * rank, a_int(3 * rank), a_int(rank), a_int(2 * d), a_int(1),
                 a_fast_u16(s, &FL->ctrl_down_qkv_q4),
                 a_f32v(s->ctrl_cat, 0, 6 * d), a_f32v(s->ctrl_gate, 0, 3 * rank));
        } else if (s->fast_buf && FL->ctrl_down_qkv_q4.off >= 0) {
            CALL(s, batch, K_DV_Q4_CTRL_DOWN_3, 3 * rank, a_int(rank), a_int(2 * d),
                 a_fast_u16(s, &FL->ctrl_down_qkv_q4), a_f32v(s->ctrl_cat, 0, 6 * d), a_f32v(s->ctrl_gate, 0, 3 * rank));
        } else {
            if (dispatch_mv_f16_small(s, batch, &L->loop_ctrl_down[0], &FL->ctrl_down[0], rank, 2 * d, 0, s->ctrl_cat, 0 * 2 * d, s->ctrl_gate, 0 * rank)) return 1;
            if (dispatch_mv_f16_small(s, batch, &L->loop_ctrl_down[1], &FL->ctrl_down[1], rank, 2 * d, 0, s->ctrl_cat, 1 * 2 * d, s->ctrl_gate, 1 * rank)) return 1;
            if (dispatch_mv_f16_small(s, batch, &L->loop_ctrl_down[2], &FL->ctrl_down[2], rank, 2 * d, 0, s->ctrl_cat, 2 * 2 * d, s->ctrl_gate, 2 * rank)) return 1;
        }
        pm_barrier(batch);
        int has_q4s_qrg = (FL->attn_qrgkv_s.qs.off >= 0 && has_k_dyn == has_v_dyn &&
                           (n_head % 2 == 0) && (!has_k_dyn || ((n_head_kv * hp->conv_len) % 2 == 0)));
        int has_q4_qrg = (FL->attn_qrg.off >= 0 && FL->attn_k.off >= 0 && FL->attn_v.off >= 0 &&
                          FL->attn_sink_proj.off >= 0 &&
                          (!has_k_dyn || FL->attn_k_conv_proj.off >= 0) &&
                          (!has_v_dyn || FL->attn_v_conv_proj.off >= 0));
        int has_q8_mma = (use_mma_b1 && FL->attn_q_s.qs.off < 0 && L->attn_q.type == INK_GGML_Q8_0 &&
                          FL->attn_sink_proj.off >= 0 && has_k_dyn == has_v_dyn);
        int can_defer_rms = (s->fast_buf && FL->ctrl_gf_qkv.off >= 0 && rank <= 64 &&
                             q_src == s->mix_out && k_src == s->mix_out && v_src == s->mix_out &&
                             (has_q4s_qrg || has_q4_qrg || has_q8_mma) &&
                             (n_q % 2 == 0) && ((n_head * n_rel) % 2 == 0) && (n_kv % 2 == 0) && citems_rm <= 128);
        if (can_defer_rms) {
            /* Sprint 70/71: Row-major [dim, rank] half2 128t lane-parallel tanh ctrl_up_apply_gf_rm_3_norm (-27%!) */
            if (FL->ctrl_gf_rm_qkv.off >= 0) {
                qkv_rms_parts = citems_rm;
                CALL(s, batch, K_CTRL_UP_APPLY_GF_RM_3_NORM, 3 * citems_rm, a_int(d), a_int(rank), a_int(is_swa ? 1 : 0),
                     a_fast_u32(s, &FL->ctrl_gf_rm_qkv),
                     a_f32v(s->ctrl_gate, 0, 3 * rank), a_f32v(s->mix_out, 0, 3 * d), a_f32v(s->messages, msg_base, 3 * d),
                     is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_q_inp_norm),
                     is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_k_inp_norm),
                     is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_v_inp_norm),
                     a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d),
                     a_f32v(s->rms_parts, 0, 3 * 128));
            } else {
                qkv_rms_parts = citems;
                CALL(s, batch, K_CTRL_UP_APPLY_GF_3_NORM, 3 * citems, a_int(d), a_int(rank), a_int(is_swa ? 1 : 0),
                     a_fast_u32(s, &FL->ctrl_gf_qkv),
                     a_f32v(s->ctrl_gate, 0, 3 * rank), a_f32v(s->mix_out, 0, 3 * d), a_f32v(s->messages, msg_base, 3 * d),
                     is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_q_inp_norm),
                     is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_k_inp_norm),
                     is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_v_inp_norm),
                     a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d),
                     a_f32v(s->rms_parts, 0, 3 * 128));
            }
            pm_barrier(batch);
            deferred_rms = 1;
        } else if (s->fast_buf && FL->ctrl_gf_qkv.off >= 0 && rank <= 64 && q_src == s->mix_out && k_src == s->mix_out && v_src == s->mix_out) {
            CALL(s, batch, K_CTRL_UP_APPLY_GF_3, 3 * citems, a_int(d), a_int(rank), a_int(is_swa ? 1 : 0),
                 a_fast_u32(s, &FL->ctrl_gf_qkv),
                 a_f32v(s->ctrl_gate, 0, 3 * rank), a_f32v(s->mix_out, 0, 3 * d), a_f32v(s->messages, msg_base, 3 * d),
                 a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d));
            pm_barrier(batch);
        } else if (s->fast_buf && FL->ctrl_gain_qkv.off >= 0 && FL->ctrl_forget_qkv.off >= 0 && rank <= 64 && q_src == s->mix_out && k_src == s->mix_out && v_src == s->mix_out) {
            CALL(s, batch, K_CTRL_UP_APPLY_F16_3, 3 * citems, a_int(d), a_int(rank), a_int(is_swa ? 1 : 0),
                 a_fast_u16(s, &FL->ctrl_gain_qkv), a_fast_u16(s, &FL->ctrl_forget_qkv),
                 a_f32v(s->ctrl_gate, 0, 3 * rank), a_f32v(s->mix_out, 0, 3 * d), a_f32v(s->messages, msg_base, 3 * d),
                 a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d));
            pm_barrier(batch);
        } else if (s->fast_buf && FL->ctrl_gain[0].off >= 0 && FL->ctrl_forget[0].off >= 0 && rank <= 64) {
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

    /* Wave 2: Input RMSNorms (completely eliminated on all 44 fused/deferred layers!). */
    PmBuffer q_in = s->inp_q, k_in = s->inp_k, v_in = s->inp_v;
    if (deferred_rms || fused_full_norm) {
        /* already normalized (fused_full_norm) or w_norm-scaled with rms_parts deferred to Wave 3! */
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

    /* Wave 3 (Sprint 50/63): Fused 128-thread Q+R+Gate + K/V/Sinks/Dyn concurrently (with deferred RMSNorm when looped!). */
    int64_t kv_cap = s->kv_cap[il];
    int64_t slot_pos = is_swa ? (s->pos % kv_cap) : s->pos;
    int64_t n_keys_attn = (is_swa && s->pos >= hp->swa) ? hp->swa : (s->pos + 1);
    int use_mma_b1 = (s->can_verify_nb && getenv("INK_NO_MMA32") == NULL);
    int use_1pass = (n_head == n_head_kv * 3 && hd == 128 && n_keys_attn >= 1 && (is_swa || n_keys_attn <= 4096 || s->can_verify_nb));
    int64_t dyn_sz = n_head_kv * hp->conv_len;
    if (use_mma_b1 && s->fast_buf && !s->fast_is_q8s && FL->attn_q_s.qs.off >= 0 && FL->attn_sink_s.qs.off >= 0 && has_k_dyn == has_v_dyn) {
        if (deferred_rms) {
            CALL(s, batch, K_SCALE_RMS_PARTS_NB, 3, a_int(d), a_int(1), a_int(qkv_rms_parts), a_f32(hp->rms_eps),
                 a_f32v(s->rms_parts, 0, 3 * 128),
                 a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d));
            pm_barrier(batch);
        }
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_q + 31) / 32, a_int(n_q), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_q_s.qs), a_fast_u16(s, &FL->attn_q_s.ds),
             a_f32v(q_in, 0, d), a_f32v(s->q_buf, 0, n_q));
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_head * n_rel + 31) / 32, a_int(n_head * n_rel), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_rel_s.qs), a_fast_u16(s, &FL->attn_rel_s.ds),
             a_f32v(q_in, 0, d), a_f32v(s->r_buf, 0, n_head * n_rel));
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_q + 31) / 32, a_int(n_q), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_gate_s.qs), a_fast_u16(s, &FL->attn_gate_s.ds),
             a_f32v(q_in, 0, d), a_f32v(s->gate_buf, 0, n_q));
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_kv + 31) / 32, a_int(n_kv), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_k_s.qs), a_fast_u16(s, &FL->attn_k_s.ds),
             a_f32v(k_in, 0, d), a_f32v(s->k_raw, 0, n_kv));
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_kv + 31) / 32, a_int(n_kv), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_v_s.qs), a_fast_u16(s, &FL->attn_v_s.ds),
             a_f32v(v_in, 0, d), a_f32v(s->v_raw, 0, n_kv));
        CALL(s, batch, K_DV_Q4S_SMALL_NB, n_head, a_int(n_head), a_int(n_head), a_int(d), a_int(1),
             a_int(0), a_f32(hp->rms_eps),
             a_fast_u32(s, &FL->attn_sink_s.qs), a_fast_u16(s, &FL->attn_sink_s.ds),
             a_f32v(q_in, 0, d), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->sinks_buf, 0, n_head));
        if (has_k_dyn) {
            CALL(s, batch, K_DV_Q4S_SMALL_NB, dyn_sz, a_int(dyn_sz), a_int(dyn_sz), a_int(d), a_int(1),
                 a_int(0), a_f32(hp->rms_eps),
                 a_fast_u32(s, &FL->attn_k_dyn_s.qs), a_fast_u16(s, &FL->attn_k_dyn_s.ds),
                 a_f32v(k_in, 0, d), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->dyn_buf, 0, dyn_sz));
            CALL(s, batch, K_DV_Q4S_SMALL_NB, dyn_sz, a_int(dyn_sz), a_int(dyn_sz), a_int(d), a_int(1),
                 a_int(0), a_f32(hp->rms_eps),
                 a_fast_u32(s, &FL->attn_v_dyn_s.qs), a_fast_u16(s, &FL->attn_v_dyn_s.ds),
                 a_f32v(v_in, 0, d), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->dyn_buf, dyn_sz, dyn_sz));
        }
    } else if (use_mma_b1 && FL->attn_q_s.qs.off < 0 && L->attn_q.type == INK_GGML_Q8_0 && FL->attn_sink_proj.off >= 0 && has_k_dyn == has_v_dyn) {
        if (deferred_rms) {
            CALL(s, batch, K_SCALE_RMS_PARTS_NB, 3, a_int(d), a_int(1), a_int(qkv_rms_parts), a_f32(hp->rms_eps),
                 a_f32v(s->rms_parts, 0, 3 * 128),
                 a_f32v(s->inp_q, 0, d), a_f32v(s->inp_k, 0, d), a_f32v(s->inp_v, 0, d));
            pm_barrier(batch);
        }
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_q + 31) / 32, a_int(n_q), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_q), a_f32v(q_in, 0, d), a_f32v(s->q_buf, 0, n_q));
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_head * n_rel + 31) / 32, a_int(n_head * n_rel), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_rel), a_f32v(q_in, 0, d), a_f32v(s->r_buf, 0, n_head * n_rel));
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_q + 31) / 32, a_int(n_q), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_gate), a_f32v(q_in, 0, d), a_f32v(s->gate_buf, 0, n_q));
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_kv + 31) / 32, a_int(n_kv), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_k), a_f32v(k_in, 0, d), a_f32v(s->k_raw, 0, n_kv));
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_kv + 31) / 32, a_int(n_kv), a_int(d), a_int(1),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_v), a_f32v(v_in, 0, d), a_f32v(s->v_raw, 0, n_kv));
        CALL(s, batch, K_DV_F16_SMALL_NB, n_head, a_int(n_head), a_int(n_head), a_int(d), a_int(1),
             a_fast_u16(s, &FL->attn_sink_proj), a_f32v(q_in, 0, d), a_f32v(s->sinks_buf, 0, n_head));
        if (has_k_dyn) {
            CALL(s, batch, K_DV_F16_SMALL_NB, dyn_sz, a_int(dyn_sz), a_int(dyn_sz), a_int(d), a_int(1),
                 a_fast_u16(s, &FL->attn_k_conv_proj), a_f32v(k_in, 0, d), a_f32v(s->dyn_buf, 0, dyn_sz));
            CALL(s, batch, K_DV_F16_SMALL_NB, dyn_sz, a_int(dyn_sz), a_int(dyn_sz), a_int(d), a_int(1),
                 a_fast_u16(s, &FL->attn_v_conv_proj), a_f32v(v_in, 0, d), a_f32v(s->dyn_buf, dyn_sz, dyn_sz));
        }
    } else if (s->fast_buf && FL->attn_qrgkv_s.qs.off >= 0 && has_k_dyn == has_v_dyn &&
        (n_q % 2 == 0) && ((n_head * n_rel) % 2 == 0) && (n_kv % 2 == 0) && (n_head % 2 == 0) && (!has_k_dyn || (dyn_sz % 2 == 0))) {
        /* Sprint 68: 1-dispatch Q4_B32_SPLIT [Q|R|G|K|V|sinks|(k_dyn|v_dyn)] -> deletes 72-96 small F16 dispatches/tok! */
        int64_t n_dyn_use = has_k_dyn ? dyn_sz : 0;
        int64_t tot_rows = n_q + n_head * n_rel + n_q + 2 * n_kv + n_head + 2 * n_dyn_use;
        uint32_t sd0 = (uint32_t)n_q | ((uint32_t)(n_head * n_rel) << 16);
        uint32_t sd1 = (uint32_t)n_kv | ((uint32_t)n_head << 16);
        uint32_t sd2 = (uint32_t)n_dyn_use | ((uint32_t)(deferred_rms ? qkv_rms_parts : 0) << 16);
        CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_QRGKV_SDYN : K_DV_Q4S_QRGKV_SDYN, (tot_rows + 7) / 8,
             a_u32(sd0), a_u32(sd1), a_u32(sd2), a_int(d), a_f32(hp->rms_eps),
             a_fast_u32(s, &FL->attn_qrgkv_s.qs), a_fast_u16(s, &FL->attn_qrgkv_s.ds),
             a_f32v(q_in, 0, d), a_f32v(k_in, 0, d), a_f32v(v_in, 0, d),
             a_f32v(s->rms_parts, 0, 3 * 128),
             a_f32v(s->q_buf, 0, n_q), a_f32v(s->r_buf, 0, n_head * n_rel), a_f32v(s->gate_buf, 0, n_q),
             a_f32v(s->k_raw, 0, n_kv), a_f32v(s->v_raw, 0, n_kv),
             a_f32v(s->sinks_buf, 0, n_head),
             has_k_dyn ? a_f32v(s->dyn_buf, 0, 2 * dyn_sz) : a_f32v(s->dummy_f32, 0, 64));
    } else if (s->fast_buf && FL->attn_qrg.off >= 0 && (n_q % 2 == 0) && ((n_head * n_rel) % 2 == 0) && (n_kv % 2 == 0)) {
        int64_t qrgkv_rows = n_q + n_head * n_rel + n_q + 2 * n_kv;
        CALL(s, batch, K_DV_Q4_QRGKV, (qrgkv_rows + 7) / 8,
             a_int(n_q), a_int(n_head * n_rel), a_int(n_kv), a_int(d),
             a_int(deferred_rms ? qkv_rms_parts : 0), a_f32(hp->rms_eps),
             a_fast_u16(s, &FL->attn_qrg),
             a_f32v(q_in, 0, d), a_f32v(k_in, 0, d), a_f32v(v_in, 0, d),
             a_f32v(s->rms_parts, 0, 3 * 128),
             a_f32v(s->q_buf, 0, n_q), a_f32v(s->r_buf, 0, n_head * n_rel), a_f32v(s->gate_buf, 0, n_q),
             a_f32v(s->k_raw, 0, n_kv), a_f32v(s->v_raw, 0, n_kv));
        if (deferred_rms) {
            CALL(s, batch, K_DV_F16_SMALL_RMS, n_head, a_int(n_head), a_int(d),
                 a_int(0), a_int(qkv_rms_parts), a_f32(hp->rms_eps),
                 a_fast_u16(s, &FL->attn_sink_proj), a_f32v(q_in, 0, d), a_f32v(s->rms_parts, 0, 3 * 128), a_f32v(s->sinks_buf, 0, n_head));
            if (has_k_dyn) {
                CALL(s, batch, K_DV_F16_SMALL_RMS, dyn_sz, a_int(dyn_sz), a_int(d),
                     a_int(qkv_rms_parts), a_int(qkv_rms_parts), a_f32(hp->rms_eps),
                     a_fast_u16(s, &FL->attn_k_conv_proj), a_f32v(k_in, 0, d), a_f32v(s->rms_parts, 0, 3 * 128), a_f32v(s->dyn_buf, 0, dyn_sz));
            }
            if (has_v_dyn) {
                CALL(s, batch, K_DV_F16_SMALL_RMS, dyn_sz, a_int(dyn_sz), a_int(d),
                     a_int(2 * qkv_rms_parts), a_int(qkv_rms_parts), a_f32(hp->rms_eps),
                     a_fast_u16(s, &FL->attn_v_conv_proj), a_f32v(v_in, 0, d), a_f32v(s->rms_parts, 0, 3 * 128), a_f32v(s->dyn_buf, dyn_sz, dyn_sz));
            }
        } else {
            if (dispatch_mv_f16_small(s, batch, &L->attn_sink_proj, &FL->attn_sink_proj, n_head, d, 0, q_in, 0, s->sinks_buf, 0)) return 1;
            if (has_k_dyn) {
                if (dispatch_mv_f16_small(s, batch, &L->attn_k_conv_proj, &FL->attn_k_conv_proj, dyn_sz, d, 0,
                                          k_in, 0, s->dyn_buf, 0)) return 1;
            }
            if (has_v_dyn) {
                if (dispatch_mv_f16_small(s, batch, &L->attn_v_conv_proj, &FL->attn_v_conv_proj, dyn_sz, d, 0,
                                          v_in, 0, s->dyn_buf, dyn_sz)) return 1;
            }
        }
    } else {
        if (dispatch_mv_q4(s, batch, &L->attn_q, &FL->attn_q, n_q, d, 0, q_in, 0, s->q_buf, 0)) return 1;
        if (dispatch_mv_q4(s, batch, &L->attn_rel, &FL->attn_rel, n_head * n_rel, d, 0, q_in, 0, s->r_buf, 0)) return 1;
        if (dispatch_mv_q4(s, batch, &L->attn_gate, &FL->attn_gate, n_q, d, 0, q_in, 0, s->gate_buf, 0)) return 1;
        if (dispatch_mv_q4(s, batch, &L->attn_k, &FL->attn_k, n_kv, d, 0, k_in, 0, s->k_raw, 0)) return 1;
        if (dispatch_mv_q4(s, batch, &L->attn_v, &FL->attn_v, n_kv, d, 0, v_in, 0, s->v_raw, 0)) return 1;
        if (dispatch_mv_f16_small(s, batch, &L->attn_sink_proj, &FL->attn_sink_proj, n_head, d, 0, q_in, 0, s->sinks_buf, 0)) return 1;
        if (has_k_dyn) {
            if (dispatch_mv_f16_small(s, batch, &L->attn_k_conv_proj, &FL->attn_k_conv_proj, dyn_sz, d, 0,
                                      k_in, 0, s->dyn_buf, 0)) return 1;
        }
        if (has_v_dyn) {
            if (dispatch_mv_f16_small(s, batch, &L->attn_v_conv_proj, &FL->attn_v_conv_proj, dyn_sz, d, 0,
                                      v_in, 0, s->dyn_buf, dyn_sz)) return 1;
        }
    }
    pm_barrier(batch);
    prof_mark(bp, 1); batch = *bp; /* 1: attn input projs (Q/K/V/R/Gate/Sinks/Dyn) */

    /* Wave 4+5 (Sprint C): Fused 3-head GQA Split-K + Q8_0 KV cache (5.65x fewer KV DRAM bytes + 40-core saturation!) */
    float q_tau = is_swa ? 1.0f : tau;
    uint32_t dims0 = (uint32_t)n_head | ((uint32_t)n_head_kv << 8) | ((uint32_t)hd << 16) | ((uint32_t)n_rel << 24);
    uint32_t dims1 = (uint32_t)hp->rel_extent | ((uint32_t)(is_swa ? 1 : 0) << 16) | ((uint32_t)hp->swa << 17);
    if (n_head == n_head_kv * 3 && hd == 128 && has_k_dyn == has_v_dyn && n_keys_attn >= 1) {
        int64_t n_splits = ink_choose_attn_splits(s, n_keys_attn, 1);
        int64_t split_elems = n_head * n_splits * 132;
        uint32_t kv_info = (uint32_t)kv_cap
                         | ((uint32_t)(L->attn_q_clip.present ? 1 : 0) << 16)
                         | ((uint32_t)(L->attn_rel_clip.present ? 1 : 0) << 17)
                         | ((uint32_t)(has_k_dyn ? 1 : 0) << 18)
                         | ((uint32_t)(s->kv_q8 ? 1 : 0) << 19)
                         | ((uint32_t)n_splits << 20);
        CALL(s, batch, K_KV_ATTN_GQA3_SPLIT, n_head_kv * n_splits,
             a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos), a_int(slot_pos),
             a_f32(hp->qk_clip), a_f32(hp->rel_clip), a_f32(q_tau), a_f32(hp->rms_eps),
             a_f32v(s->k_raw, 0, n_kv), a_f32v(s->v_raw, 0, n_kv),
             a_tensor_f32(s, &L->attn_k_conv), a_tensor_f32(s, &L->attn_v_conv),
             has_k_dyn ? a_f32v(s->dyn_buf, 0, 2 * dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
             a_conv_k(s, il), a_conv_v(s, il),
             a_f32v(s->q_buf, 0, n_q),
             L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64),
             a_f32v(s->r_buf, 0, n_head * n_rel),
             L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64),
             a_tensor_f32(s, &L->attn_rel_bias),
             a_f32v(s->sinks_buf, 0, n_head),
             a_f32v(s->gate_buf, 0, n_q),
             a_u16v(s->kv_k[il], 0, kv_cap * s->kv_row_u16),
             a_u16v(s->kv_v[il], 0, kv_cap * s->kv_row_u16),
             a_f32v(s->attn_scores, 0, split_elems),
             a_f32v(s->attn_out, 0, n_q));
        if (n_splits > 1) {
            pm_barrier(batch);
            CALL(s, batch, K_ATTN_GQA3_REDUCE_NB, n_head,
                 a_int(n_splits),
                 a_f32v(s->attn_scores, 0, split_elems),
                 a_f32v(s->gate_buf, 0, n_q),
                 a_f32v(s->attn_out, 0, n_q));
        }
    } else if (use_1pass && has_k_dyn == has_v_dyn) {
        uint32_t kv_info = (uint32_t)kv_cap | ((uint32_t)(L->attn_q_clip.present ? 1 : 0) << 16)
                         | ((uint32_t)(L->attn_rel_clip.present ? 1 : 0) << 17)
                         | ((uint32_t)(has_k_dyn ? 1 : 0) << 18);
        CALL(s, batch, K_KV_ATTN_GQA_1PASS, n_head,
             a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos), a_int(slot_pos),
             a_f32(hp->qk_clip), a_f32(hp->rel_clip), a_f32(q_tau), a_f32(hp->rms_eps),
             a_f32v(s->k_raw, 0, n_kv), a_f32v(s->v_raw, 0, n_kv),
             a_tensor_f32(s, &L->attn_k_conv), a_tensor_f32(s, &L->attn_v_conv),
             has_k_dyn ? a_f32v(s->dyn_buf, 0, 2 * dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
             a_conv_k(s, il), a_conv_v(s, il),
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
    } else {
        if (hd == 128 && has_k_dyn == has_v_dyn) {
            CALL(s, batch, K_CONV_KV_BOTH, 2 * n_head_kv, a_int(n_head_kv), a_int(n_kv), a_int(s->pos), a_int(slot_pos),
                 a_int(has_k_dyn ? 1 : 0), a_f32(hp->qk_clip), a_f32(hp->rms_eps),
                 a_f32v(s->k_raw, 0, n_kv), a_f32v(s->v_raw, 0, n_kv),
                 a_tensor_f32(s, &L->attn_k_conv), a_tensor_f32(s, &L->attn_v_conv),
                 has_k_dyn ? a_f32v(s->dyn_buf, 0, 2 * dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
                 a_conv_k(s, il), a_conv_v(s, il),
                 a_u16v(s->kv_k[il], 0, kv_cap * n_kv), a_u16v(s->kv_v[il], 0, kv_cap * n_kv));
        } else {
            CALL(s, batch, K_CONV_CLIP_KV, n_head_kv, a_int(hd), a_int(n_kv), a_int(s->pos), a_int(slot_pos), a_int(has_k_dyn ? 1 : 0),
                 a_f32(hp->qk_clip), a_f32v(s->k_raw, 0, n_kv), a_tensor_f32(s, &L->attn_k_conv),
                 has_k_dyn ? a_f32v(s->dyn_buf, 0, dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
                 a_conv_k(s, il), a_u16v(s->kv_k[il], 0, kv_cap * n_kv));
            CALL(s, batch, K_CONV_HEAD_RMSNORM_KV, n_head_kv, a_int(hd), a_int(n_kv), a_int(s->pos), a_int(slot_pos), a_int(has_v_dyn ? 1 : 0),
                 a_f32(hp->rms_eps), a_f32v(s->v_raw, 0, n_kv), a_tensor_f32(s, &L->attn_v_conv),
                 has_v_dyn ? a_f32v(s->dyn_buf, dyn_sz, dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
                 a_conv_v(s, il), a_u16v(s->kv_v[il], 0, kv_cap * n_kv));
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
    }
    pm_barrier(batch);
    prof_mark(bp, 2); batch = *bp; /* 2: conv+clip+kv_store + GQA attn */

    /* Output projection + fused post shortconv + residual (Sprint 62/63/68: Q4_B32_SPLIT uint2 in-place cur!) */
    int ablate_ip = (s->has_ablate && ip < 32 && s->ablate_has_layer[ip]);
    if (attn_o_inplace) {
        if (is_blk) (*n_blocks)++;
        if (ablate_ip) {
            if (s->fast_buf && s->fast_is_q8s && FL->attn_output_s.qs.off >= 0) {
                CALL(s, batch, K_DV_Q8S_NB, (d + 3) / 4, a_int(d), a_int(n_q), a_int(1),
                     a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                     a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
                     a_f32v(s->attn_out, 0, n_q), a_f32v(s->o_raw, 0, d));
            } else if (use_mma_b1 && s->fast_buf && !s->fast_is_q8s && FL->attn_output_s.qs.off >= 0) {
                CALL(s, batch, K_DV_Q4S_MMA32_NB, (d + 31) / 32, a_int(d), a_int(n_q), a_int(1),
                     a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                     a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
                     a_f32v(s->attn_out, 0, n_q), a_f32v(s->o_raw, 0, d));
            } else if (s->fast_buf && !s->fast_is_q8s && FL->attn_output_s.qs.off >= 0) {
                CALL(s, batch, K_DV_Q4S_NB, (d + 3) / 4, a_int(d), a_int(n_q), a_int(1),
                     a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                     a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
                     a_f32v(s->attn_out, 0, n_q), a_f32v(s->o_raw, 0, d));
            } else {
                if (dispatch_mv(s, batch, &L->attn_output, d, n_q, 0, s->attn_out, 0, s->o_raw, 0)) return 1;
            }
            pm_barrier(batch);
            CALL(s, batch, K_POST_CONV_ABLATE_RES_NB, 1, a_int(d), a_int(1), a_int(is_blk ? 0 : 1), a_int(1),
                 a_f32v(s->o_raw, 0, d), a_int((int64_t)(1 + ip) * d), a_ablate_dirs(s),
                 a_tensor_f32(s, &L->attn_post_conv), a_conv_ao(s, il),
                 a_f32v(s->cur, 0, d), a_raw_ao(s, il), a_f32v(s->cur, 0, d));
        } else if (use_mma_b1 && s->fast_buf && !s->fast_is_q8s && FL->attn_output_s.qs.off >= 0) {
            CALL(s, batch, K_DV_Q4S_MMA32_NB, (d + 31) / 32, a_int(d), a_int(n_q), a_int(1),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
                 a_f32v(s->attn_out, 0, n_q), a_f32v(s->o_raw, 0, d));
            pm_barrier(batch);
            CALL(s, batch, K_POST_CONV_RES_NB, (d + 63) / 64, a_int(d), a_int(1), a_int(is_blk ? 0 : 1), a_int(1),
                 a_f32v(s->o_raw, 0, d), a_tensor_f32(s, &L->attn_post_conv),
                 a_conv_ao(s, il), a_f32v(s->cur, 0, d), a_raw_ao(s, il), a_f32v(s->cur, 0, d));
        } else if (use_mma_b1 && FL->attn_output_s.qs.off < 0 && L->attn_output.type == INK_GGML_Q8_0) {
            CALL(s, batch, K_DV_Q8_MMA32_NB, (d + 31) / 32, a_int(d), a_int(n_q), a_int(1),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_tensor_u16(s, &L->attn_output),
                 a_f32v(s->attn_out, 0, n_q), a_f32v(s->o_raw, 0, d));
            pm_barrier(batch);
            CALL(s, batch, K_POST_CONV_RES_NB, (d + 63) / 64, a_int(d), a_int(1), a_int(is_blk ? 0 : 1), a_int(1),
                 a_f32v(s->o_raw, 0, d), a_tensor_f32(s, &L->attn_post_conv),
                 a_conv_ao(s, il), a_f32v(s->cur, 0, d), a_raw_ao(s, il), a_f32v(s->cur, 0, d));
        } else if (s->fast_buf && FL->attn_output_s.qs.off >= 0) {
            CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_CONV : K_DV_Q4S_CONV, (d + 7) / 8, a_int(d), a_int(n_q), a_int(is_blk ? 0 : 1),
                 a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
                 a_f32v(s->attn_out, 0, n_q),
                 a_tensor_f32(s, &L->attn_post_conv), a_conv_ao(s, il),
                 a_f32v(s->cur, 0, d), a_f32v(s->cur, 0, d));
        } else if (s->fast_buf && FL->attn_output.off >= 0) {
            CALL(s, batch, K_DV_Q4_CONV, (d + 7) / 8, a_int(d), a_int(n_q), a_int(is_blk ? 0 : 1),
                 a_fast_u16(s, &FL->attn_output), a_f32v(s->attn_out, 0, n_q),
                 a_tensor_f32(s, &L->attn_post_conv), a_conv_ao(s, il),
                 a_f32v(s->cur, 0, d), a_f32v(s->cur, 0, d));
        } else {
            CALL(s, batch, K_DV_Q8_CONV, (d + 1) / 2, a_int(d), a_int(n_q), a_int(is_blk ? 0 : 1),
                 a_tensor_u16(s, &L->attn_output), a_f32v(s->attn_out, 0, n_q),
                 a_tensor_f32(s, &L->attn_post_conv), a_conv_ao(s, il),
                 a_f32v(s->cur, 0, d), a_f32v(s->cur, 0, d));
        }
    } else {
        if (dispatch_mv(s, batch, &L->attn_output, d, n_q, 0, s->attn_out, 0, s->o_raw, 0)) return 1;
        pm_barrier(batch);
        if (ablate_ip) {
            CALL(s, batch, K_ABLATE_DIR, 1, a_int(d), a_int((int64_t)(1 + ip) * d),
                 a_ablate_dirs(s), a_f32v(s->o_raw, 0, d));
            pm_barrier(batch);
        }
        if (ip % hp->block_size == 0) {
            if (*n_blocks >= INK_MAX_BLOCKS) {
                snprintf(s->error, sizeof s->error, "block count overflow");
                return 1;
            }
            CALL(s, batch, K_CONVOLVE, d, a_int(d), a_int(1), a_int(0),
                 a_f32v(s->o_raw, 0, d), a_tensor_f32(s, &L->attn_post_conv),
                 a_f32v(s->dummy_f32, 0, 64),
                 a_conv_ao(s, il), a_f32v(s->cur, 0, d));
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->inp_sa, 0, d),
                 a_f32v(s->blocks, (int64_t)(*n_blocks) * d, d));
            (*n_blocks)++;
        } else {
            CALL(s, batch, K_CONVOLVE_ADD, d, a_int(d),
                 a_f32v(s->o_raw, 0, d), a_tensor_f32(s, &L->attn_post_conv),
                 a_conv_ao(s, il), a_f32v(s->inp_sa, 0, d), a_f32v(s->cur, 0, d));
        }
    }
    pm_barrier(batch);
    prof_mark(bp, 3); batch = *bp; /* 3: attn_output (Q8_0+conv) */

    /* ffn_inp = cur (skip K_COPY when fused MoE down updates cur in-place -> -48 dispatches/tok!) */
    int moe_dn_inplace = ((s->fast_buf && (FL->moe_dn_q4s.qs.off >= 0 || FL->moe_dn_q4.off >= 0)) ||
                          (s->iq4s_buf && FL->moe_dn_iq4s.qs.off >= 0) || L->ffn_down_exps.type == INK_GGML_IQ4_NL);
    if (!moe_dn_inplace) {
        CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d), a_f32v(s->cur, 0, d), a_f32v(s->ffn_inp, 0, d));
    }
    int router_done = 0;
    int ffn_deferred_rms = 0;
    int use_fused_rtr = ((s->fast_buf && (FL->moe_gu_q4s.qs.off >= 0 || FL->moe_gu_q4.off >= 0)) ||
                         (s->iq4s_buf && FL->moe_gu_iq4s.qs.off >= 0) || L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL);
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
        if (((s->fast_buf && FL->ctrl_gf3.off >= 0) ||
             (L->loop_ctrl_gain[3].type == INK_GGML_F32 && L->loop_ctrl_forget[3].type == INK_GGML_F32)) && rank <= 64) {
            /* Sprint 29: Fused K_MIX + K_CTRL_PREP (-24 dispatches & barriers/tok)! */
            CALL(s, batch, K_MIX_CTRL_PREP, 1, a_int(d), a_int(*n_blocks),
                 a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
                 a_f32v(s->cur, 0, d),
                 a_tensor_f32(s, &L->ffn_res_query),
                 a_f32v(s->messages, msg_base + 3 * d, d),
                 a_f32v(s->norm_buf, 0, d),
                 a_f32v(s->ctrl_cat, 0, 2 * d));
            pm_barrier(batch);
            if (s->fast_buf && FL->ctrl_down3_s.qs.off >= 0) {
                CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_SMALL_RMS : K_DV_Q4S_SMALL_RMS, rank, a_int(rank), a_int(2 * d),
                     a_int(0), a_int(0), a_f32(hp->rms_eps),
                     a_fast_u32(s, &FL->ctrl_down3_s.qs), a_fast_u16(s, &FL->ctrl_down3_s.ds),
                     a_f32v(s->ctrl_cat, 0, 2 * d), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->ctrl_gate, 0, rank));
            } else if (use_mma_b1 && s->fast_buf && FL->ctrl_down[3].off >= 0) {
                CALL(s, batch, K_DV_F16_SMALL_NB, rank, a_int(rank), a_int(rank), a_int(2 * d), a_int(1),
                     a_fast_u16(s, &FL->ctrl_down[3]),
                     a_f32v(s->ctrl_cat, 0, 2 * d), a_f32v(s->ctrl_gate, 0, rank));
            } else {
                if (dispatch_mv_f16_small(s, batch, &L->loop_ctrl_down[3], &FL->ctrl_down[3], rank, 2 * d, 0, s->ctrl_cat, 0, s->ctrl_gate, 0)) return 1;
            }
            pm_barrier(batch);
            int64_t citems = (d + INK_T - 1) / INK_T;
            int64_t ffn_rms_parts = citems;
            int can_ffn_defer = (s->fast_buf && FL->ctrl_gf3.off >= 0 && use_fused_rtr && citems_rm <= 128 &&
                                 (FL->ffn_gate_inp_s.qs.off >= 0 || FL->ffn_gate_inp.off >= 0));
            if (can_ffn_defer) {
                /* Sprint 70/71: Row-major [dim, rank] half2 128t lane-parallel tanh ctrl_up_apply_gf_rm_norm! */
                if (FL->ctrl_gf_rm3.off >= 0) {
                    ffn_rms_parts = citems_rm;
                    CALL(s, batch, K_CTRL_UP_APPLY_GF_RM_NORM, citems_rm, a_int(d), a_int(rank),
                         a_fast_u32(s, &FL->ctrl_gf_rm3),
                         a_f32v(s->ctrl_gate, 0, rank), a_f32v(s->norm_buf, 0, d), a_f32v(s->messages, msg_base + 3 * d, d),
                         a_tensor_f32(s, &L->ffn_norm), a_f32v(s->o_raw, 0, d), a_f32v(s->rms_parts, 0, 3 * 128));
                } else {
                    ffn_rms_parts = citems;
                    CALL(s, batch, K_CTRL_UP_APPLY_GF_NORM, citems, a_int(d), a_int(rank),
                         a_fast_u32(s, &FL->ctrl_gf3),
                         a_f32v(s->ctrl_gate, 0, rank), a_f32v(s->norm_buf, 0, d), a_f32v(s->messages, msg_base + 3 * d, d),
                         a_tensor_f32(s, &L->ffn_norm), a_f32v(s->o_raw, 0, d), a_f32v(s->rms_parts, 0, 3 * 128));
                }
                pm_barrier(batch);
                ffn_deferred_rms = (int)ffn_rms_parts;
                if (FL->ffn_gate_inp_s.qs.off >= 0) {
                    CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_SMALL_RMS : K_DV_Q4S_SMALL_RMS, hp->n_expert, a_int(hp->n_expert), a_int(d),
                         a_int(0), a_int(ffn_rms_parts), a_f32(hp->rms_eps),
                         a_fast_u32(s, &FL->ffn_gate_inp_s.qs), a_fast_u16(s, &FL->ffn_gate_inp_s.ds),
                         a_f32v(s->o_raw, 0, d), a_f32v(s->rms_parts, 0, 3 * 128), a_f32v(s->router_logits, 0, hp->n_expert));
                } else if (use_mma_b1) {
                    CALL(s, batch, K_SCALE_RMS_PARTS_NB, 1, a_int(d), a_int(1), a_int(ffn_rms_parts), a_f32(hp->rms_eps),
                         a_f32v(s->rms_parts, 0, 3 * 128),
                         a_f32v(s->o_raw, 0, d), a_f32v(s->o_raw, 0, d), a_f32v(s->o_raw, 0, d));
                    pm_barrier(batch);
                    CALL(s, batch, K_DV_F16_SMALL_NB, hp->n_expert, a_int(hp->n_expert), a_int(hp->n_expert), a_int(d), a_int(1),
                         a_fast_u16(s, &FL->ffn_gate_inp), a_f32v(s->o_raw, 0, d), a_f32v(s->router_logits, 0, hp->n_expert));
                    ffn_deferred_rms = -1; /* already scaled in s->o_raw; pass n_parts=0 to MoE GU */
                } else {
                    CALL(s, batch, K_DV_F16_SMALL_RMS, hp->n_expert, a_int(hp->n_expert), a_int(d),
                         a_int(0), a_int(ffn_rms_parts), a_f32(hp->rms_eps),
                         a_fast_u16(s, &FL->ffn_gate_inp), a_f32v(s->o_raw, 0, d),
                         a_f32v(s->rms_parts, 0, 3 * 128), a_f32v(s->router_logits, 0, hp->n_expert));
                }
                router_done = 1;
                pm_barrier(batch);
            } else {
                int conc_rtr = (s->fast_buf && FL->ffn_gate_inp_normed.off >= 0);
                PmBuffer cu_out = conc_rtr ? s->o_raw : s->norm_buf;
                if (s->fast_buf && FL->ctrl_gf3.off >= 0) {
                    CALL(s, batch, K_CTRL_UP_APPLY_GF, citems, a_int(d), a_int(rank),
                         a_fast_u32(s, &FL->ctrl_gf3),
                         a_f32v(s->ctrl_gate, 0, rank), a_f32v(s->norm_buf, 0, d), a_f32v(s->messages, msg_base + 3 * d, d),
                         a_f32v(cu_out, 0, d));
                } else if (s->fast_buf && FL->ctrl_gain[3].off >= 0 && FL->ctrl_forget[3].off >= 0) {
                    CALL(s, batch, K_CTRL_UP_APPLY_F16, citems, a_int(d), a_int(rank),
                         a_fast_u16(s, &FL->ctrl_gain[3]), a_fast_u16(s, &FL->ctrl_forget[3]),
                         a_f32v(s->ctrl_gate, 0, rank), a_f32v(s->norm_buf, 0, d), a_f32v(s->messages, msg_base + 3 * d, d),
                         a_f32v(cu_out, 0, d));
                } else {
                    CALL(s, batch, K_CTRL_UP_APPLY, citems, a_int(d), a_int(rank),
                         a_tensor_f32(s, &L->loop_ctrl_gain[3]), a_tensor_f32(s, &L->loop_ctrl_forget[3]),
                         a_f32v(s->ctrl_gate, 0, rank), a_f32v(s->norm_buf, 0, d), a_f32v(s->messages, msg_base + 3 * d, d),
                         a_f32v(cu_out, 0, d));
                }
                pm_barrier(batch);
                CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
                     a_f32v(cu_out, 0, d), a_tensor_f32(s, &L->ffn_norm), a_f32v(s->norm_buf, 0, d));
                if (conc_rtr) {
                    CALL(s, batch, K_DV_F16_NORM, hp->n_expert, a_int(hp->n_expert), a_int(d), a_f32(hp->rms_eps),
                         a_fast_u16(s, &FL->ffn_gate_inp_normed), a_f32v(cu_out, 0, d), a_f32v(s->router_logits, 0, hp->n_expert));
                    router_done = 1;
                }
                pm_barrier(batch);
            }
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

    /* MoE router + gate_up + SITU (Sprint 67/68: 1-warp noinline fused Router top-8 + Warp-1 deferred rms_parts!) */
    if (!router_done) {
        if (s->fast_buf && FL->ffn_gate_inp_s.qs.off >= 0) {
            CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_SMALL_RMS : K_DV_Q4S_SMALL_RMS, hp->n_expert, a_int(hp->n_expert), a_int(d),
                 a_int(0), a_int(0), a_f32(hp->rms_eps),
                 a_fast_u32(s, &FL->ffn_gate_inp_s.qs), a_fast_u16(s, &FL->ffn_gate_inp_s.ds),
                 a_f32v(s->norm_buf, 0, d), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->router_logits, 0, hp->n_expert));
        } else if (use_mma_b1 && s->fast_buf && FL->ffn_gate_inp.off >= 0) {
            CALL(s, batch, K_DV_F16_SMALL_NB, hp->n_expert, a_int(hp->n_expert), a_int(hp->n_expert), a_int(d), a_int(1),
                 a_fast_u16(s, &FL->ffn_gate_inp), a_f32v(s->norm_buf, 0, d), a_f32v(s->router_logits, 0, hp->n_expert));
        } else {
            if (dispatch_mv_f16_small(s, batch, &L->ffn_gate_inp, &FL->ffn_gate_inp, hp->n_expert, d, 0, s->norm_buf, 0, s->router_logits, 0)) return 1;
        }
        pm_barrier(batch);
    }
    int64_t n_ff = hp->n_ff_exp;
    int64_t gu_items_2r = (int64_t)hp->n_expert_used * ((n_ff + 7) / 8);
    int64_t gu_items_q8 = (int64_t)hp->n_expert_used * ((n_ff + 3) / 4);
    if (!use_fused_rtr) {
        CALL(s, batch, K_ROUTER, 1, a_int(hp->n_expert), a_int(hp->n_expert_used),
             a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
             a_f32v(s->router_logits, 0, hp->n_expert),
             L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used));
        pm_barrier(batch);
    }
    prof_mark(bp, 4); batch = *bp; /* 4: moe prep (mix+norm+router_gemv) */

    PmBuffer moe_x_in = (ffn_deferred_rms != 0) ? s->o_raw : s->norm_buf;
    int64_t ffn_n_parts = (ffn_deferred_rms > 0) ? (int64_t)ffn_deferred_rms : 0;
    if (s->fast_buf && FL->moe_gu_q4s.qs.off >= 0) {
        CALL(s, batch, s->fast_is_q8s ? K_MOE_GU_Q8S : K_MOE_GU_Q4S, gu_items_2r, a_int(n_ff), a_int(d),
             a_int(hp->n_expert), a_int(hp->n_expert_used),
             a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_int(ffn_n_parts), a_f32(hp->rms_eps),
             a_fast_u32(s, &FL->moe_gu_q4s.qs), a_fast_u16(s, &FL->moe_gu_q4s.ds),
             a_f32v(moe_x_in, 0, d),
             a_f32v(s->rms_parts, 0, 3 * 128),
             a_f32v(s->router_logits, 0, hp->n_expert),
             L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else if (s->fast_buf && FL->moe_gu_q4.off >= 0) {
        CALL(s, batch, K_MOE_GU_Q4, gu_items_2r, a_int(n_ff), a_int(d),
             a_int(hp->n_expert), a_int(hp->n_expert_used),
             a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_int(ffn_n_parts), a_f32(hp->rms_eps),
             a_fast_u16(s, &FL->moe_gu_q4),
             a_f32v(moe_x_in, 0, d),
             a_f32v(s->rms_parts, 0, 3 * 128),
             a_f32v(s->router_logits, 0, hp->n_expert),
             L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else if (s->iq4s_buf && FL->moe_gu_iq4s.qs.off >= 0) {
        CALL(s, batch, K_MOE_GU_IQ4S, gu_items_2r, a_int(n_ff), a_int(d),
             a_int(hp->n_expert), a_int(hp->n_expert_used),
             a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_int(ffn_n_parts), a_f32(hp->rms_eps),
             a_iq4s_u32(s, &FL->moe_gu_iq4s.qs), a_iq4s_u16(s, &FL->moe_gu_iq4s.ds),
             a_f32v(moe_x_in, 0, d),
             a_f32v(s->rms_parts, 0, 3 * 128),
             a_f32v(s->router_logits, 0, hp->n_expert),
             L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else if (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) {
        CALL(s, batch, K_MOE_GU_IQ4, gu_items_2r, a_int(n_ff), a_int(d),
             a_int(hp->n_expert), a_int(hp->n_expert_used),
             a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_int(ffn_n_parts), a_f32(hp->rms_eps),
             a_tensor_u16(s, &L->ffn_gate_up_exps),
             a_f32v(moe_x_in, 0, d),
             a_f32v(s->rms_parts, 0, 3 * 128),
             a_f32v(s->router_logits, 0, hp->n_expert),
             L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else if (use_mma_b1 && L->ffn_gate_up_exps.type == INK_GGML_Q8_0) {
        int64_t gpe = (n_ff + 7) / 8;
        CALL(s, batch, K_MOE_PLAN_NB, 1, a_int(hp->n_expert), a_int(hp->n_expert_used), a_int(1), a_int(gpe),
             a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
             a_f32v(s->router_logits, 0, hp->n_expert),
             L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
             a_u32v(s->moe_ids, 0, hp->n_expert_used), a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
             a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4));
        pm_barrier(batch);
        CALL(s, batch, K_MOE_GU_Q8_NB, hp->n_expert_used * gpe, a_int(n_ff), a_int(d), a_int(1), a_int(hp->n_expert_used),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_tensor_u16(s, &L->ffn_gate_up_exps), a_f32v(s->norm_buf, 0, d),
             a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
             a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else if (L->ffn_gate_up_exps.type == INK_GGML_Q8_0) {
        CALL(s, batch, K_MOE_GU_Q8, gu_items_q8, a_int(n_ff), a_int(d),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_tensor_u16(s, &L->ffn_gate_up_exps),
             a_f32v(s->norm_buf, 0, d),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else if (ink_ggml_type_supported(L->ffn_gate_up_exps.type)) {
        CALL(s, batch, K_MOE_GU_GGML, gu_items_q8, a_int(n_ff), a_int(d), a_int((int64_t)L->ffn_gate_up_exps.type),
             a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_tensor_ggml_u16(s, &L->ffn_gate_up_exps), a_ggml_grids(s),
             a_f32v(s->norm_buf, 0, d),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff));
    } else {
        snprintf(s->error, sizeof s->error, "unsupported gate_up type %u (%s)", L->ffn_gate_up_exps.type, ink_ggml_type_name(L->ffn_gate_up_exps.type));
        return 1;
    }
    pm_barrier(batch);
    prof_mark(bp, (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) ? 5 : 6); batch = *bp; /* 5: moe_gu_iq4, 6: moe_gu_q8 */

    /* MoE down + weighted sum + fused post shortconv & residual add (Sprint 69/70: 288 TGs 8r/TG md_8r_nobar, Q4_B32_SPLIT for Q8_0 layers!) */
    int64_t dn_items = (d + INK_D - 1) / INK_D;
    PmBuffer ffn_res = moe_dn_inplace ? s->cur : s->ffn_inp;
    if (ablate_ip) {
        int has_p1_fast = ((s->fast_buf && FL->moe_dn_q4s.qs.off >= 0) ||
                           (s->iq4s_buf && FL->moe_dn_iq4s.qs.off >= 0));
        if (has_p1_fast) {
            int64_t dn_gpe = (d + 15) / 16;
            CALL(s, batch, K_MOE_PLAN_NB, 1, a_int(hp->n_expert), a_int(hp->n_expert_used), a_int(1), a_int(dn_gpe),
                 a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
                 a_f32v(s->router_logits, 0, hp->n_expert),
                 L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
                 a_u32v(s->moe_ids, 0, hp->n_expert_used), a_f32v(s->moe_wts, 0, hp->n_expert_used),
                 a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                 a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4));
            pm_barrier(batch);
            if (s->fast_buf && FL->moe_dn_q4s.qs.off >= 0 && s->fast_is_q8s) {
                CALL(s, batch, K_MOE_DOWN_P1_Q8S_NB, hp->n_expert_used * ((d + 3) / 4), a_int(d), a_int(n_ff), a_int(1), a_int(hp->n_expert_used),
                     a_fast_u32(s, &FL->moe_dn_q4s.qs), a_fast_u16(s, &FL->moe_dn_q4s.ds),
                     a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
                     a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                     a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
                     a_f32v(s->moe_slot_out, 0, hp->n_expert_used * d));
            } else if (s->fast_buf && FL->moe_dn_q4s.qs.off >= 0) {
                CALL(s, batch, K_MOE_DOWN_P1_Q4S_NB, hp->n_expert_used * dn_gpe, a_int(d), a_int(n_ff), a_int(1), a_int(hp->n_expert_used),
                     a_fast_u32(s, &FL->moe_dn_q4s.qs), a_fast_u16(s, &FL->moe_dn_q4s.ds),
                     a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
                     a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                     a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
                     a_f32v(s->moe_slot_out, 0, hp->n_expert_used * d));
            } else {
                CALL(s, batch, K_MOE_DOWN_P1_IQ4S_NB, hp->n_expert_used * dn_gpe, a_int(d), a_int(n_ff), a_int(1), a_int(hp->n_expert_used),
                     a_iq4s_u32(s, &FL->moe_dn_iq4s.qs), a_iq4s_u16(s, &FL->moe_dn_iq4s.ds),
                     a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
                     a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                     a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
                     a_f32v(s->moe_slot_out, 0, hp->n_expert_used * d));
            }
            pm_barrier(batch);
            CALL(s, batch, K_MOE_SUM_ABLATE_CONV_NB, 1, a_int(d), a_int(1), a_int(hp->n_expert_used), a_int(1),
                 a_f32v(s->moe_slot_out, 0, hp->n_expert_used * d),
                 a_f32v(s->moe_wts, 0, hp->n_expert_used),
                 a_int((int64_t)(1 + ip) * d), a_ablate_dirs(s),
                 a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
                 a_f32v(ffn_res, 0, d), a_raw_fo(s, il), a_f32v(s->cur, 0, d));
        } else if (ink_ggml_type_supported(L->ffn_down_exps.type)) {
            CALL(s, batch, K_MOE_DOWN_GGML, (d + 3) / 4, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
                 a_int((int64_t)L->ffn_down_exps.type), a_int(0),
                 a_tensor_ggml_u16(s, &L->ffn_down_exps), a_ggml_grids(s),
                 a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
                 a_u32v(s->moe_ids, 0, hp->n_expert_used),
                 a_f32v(s->moe_wts, 0, hp->n_expert_used),
                 a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
                 a_f32v(ffn_res, 0, d), a_f32v(s->moe_out, 0, d));
            pm_barrier(batch);
            CALL(s, batch, K_POST_CONV_ABLATE_RES_NB, 1, a_int(d), a_int(1), a_int(1), a_int(1),
                 a_f32v(s->moe_out, 0, d), a_int((int64_t)(1 + ip) * d), a_ablate_dirs(s),
                 a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
                 a_f32v(ffn_res, 0, d), a_raw_fo(s, il), a_f32v(s->cur, 0, d));
        } else {
            snprintf(s->error, sizeof s->error, "unsupported down type %u (%s)", L->ffn_down_exps.type, ink_ggml_type_name(L->ffn_down_exps.type));
            return 1;
        }
        pm_barrier(batch);
        prof_mark(bp, (L->ffn_down_exps.type == INK_GGML_IQ4_NL) ? 7 : 8); batch = *bp;
    } else if (s->fast_buf && FL->moe_dn_q4s.qs.off >= 0) {
        CALL(s, batch, s->fast_is_q8s ? K_MOE_DOWN_Q8S : K_MOE_DOWN_Q4S, (d + 7) / 8, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_fast_u32(s, &FL->moe_dn_q4s.qs), a_fast_u16(s, &FL->moe_dn_q4s.ds),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_conv_fo(s, il),
             a_f32v(s->cur, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, (L->ffn_down_exps.type == INK_GGML_IQ4_NL) ? 7 : 8); batch = *bp;
    } else if (s->fast_buf && FL->moe_dn_q4.off >= 0) {
        CALL(s, batch, K_MOE_DOWN_Q4, (d + 7) / 8, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_fast_u16(s, &FL->moe_dn_q4),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_conv_fo(s, il),
             a_f32v(s->cur, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, (L->ffn_down_exps.type == INK_GGML_IQ4_NL) ? 7 : 8); batch = *bp;
    } else if (s->iq4s_buf && FL->moe_dn_iq4s.qs.off >= 0) {
        CALL(s, batch, K_MOE_DOWN_IQ4S, (d + 7) / 8, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_iq4s_u32(s, &FL->moe_dn_iq4s.qs), a_iq4s_u16(s, &FL->moe_dn_iq4s.ds),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_conv_fo(s, il),
             a_f32v(s->cur, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, 7); batch = *bp; /* 7: moe_down_iq4s */
    } else if (L->ffn_down_exps.type == INK_GGML_IQ4_NL) {
        CALL(s, batch, K_MOE_DOWN_IQ4, (d + 7) / 8, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_conv_fo(s, il),
             a_f32v(s->cur, 0, d),
             a_f32v(s->moe_out, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, 7); batch = *bp; /* 7: moe_down_iq4 */
    } else if (use_mma_b1 && L->ffn_down_exps.type == INK_GGML_Q8_0) {
        int64_t dn_gpe = (d + 15) / 16;
        CALL(s, batch, K_MOE_DOWN_P1_Q8_NB, hp->n_expert_used * dn_gpe, a_int(d), a_int(n_ff), a_int(1), a_int(hp->n_expert_used),
             a_tensor_u16(s, &L->ffn_down_exps), a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
             a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_f32v(s->moe_slot_out, 0, hp->n_expert_used * d));
        pm_barrier(batch);
        CALL(s, batch, K_MOE_DOWN_P2_NB, (d + 63) / 64, a_int(d), a_int(1), a_int(hp->n_expert_used), a_int(1),
             a_f32v(s->moe_slot_out, 0, hp->n_expert_used * d),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_u32v(s->spec_u_mask, 0, 64), a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
             a_f32v(s->ffn_inp, 0, d), a_raw_fo(s, il), a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, 8); batch = *bp; /* 8: moe_down_q8 */
    } else if (L->ffn_down_exps.type == INK_GGML_Q8_0) {
        CALL(s, batch, K_MOE_DOWN_Q8, dn_items, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_conv_fo(s, il),
             a_f32v(s->ffn_inp, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, 8); batch = *bp; /* 8: moe_down_q8 */
    } else if (ink_ggml_type_supported(L->ffn_down_exps.type)) {
        CALL(s, batch, K_MOE_DOWN_GGML, (d + 3) / 4, a_int(d), a_int(n_ff), a_int(hp->n_expert_used),
             a_int((int64_t)L->ffn_down_exps.type), a_int(1),
             a_tensor_ggml_u16(s, &L->ffn_down_exps), a_ggml_grids(s),
             a_f32v(s->moe_hid, 0, hp->n_expert_used * n_ff),
             a_u32v(s->moe_ids, 0, hp->n_expert_used),
             a_f32v(s->moe_wts, 0, hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv),
             a_conv_fo(s, il),
             a_f32v(ffn_res, 0, d),
             a_f32v(s->cur, 0, d));
        pm_barrier(batch);
        prof_mark(bp, 8); batch = *bp;
    } else {
        snprintf(s->error, sizeof s->error, "unsupported down type %u (%s)", L->ffn_down_exps.type, ink_ggml_type_name(L->ffn_down_exps.type));
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

static int alloc_buf_unset(InkSession *s, PmBuffer *b, size_t bytes) {
    if (pm_alloc_unset(bytes ? bytes : 64, b) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_alloc_unset(%zu): %s", bytes, pm_log());
        return 1;
    }
    return 0;
}

static double open_now_s(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}

static int fast_worker_threads(void) {
    int ncpu = 8;
    size_t sz = sizeof(ncpu);
    if (sysctlbyname("hw.ncpu", &ncpu, &sz, NULL, 0) != 0 || ncpu < 1) ncpu = 8;
    if (ncpu > 8) ncpu = 8;
    return ncpu;
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

static void convert_q8_to_q4s(const uint8_t *src, size_t nbytes, uint8_t *dst_qs, uint8_t *dst_ds) {
    size_t nb = nbytes / 34;
    uint16_t *ds = (uint16_t *)dst_ds;
    for (size_t b = 0; b < nb; b++) {
        const uint8_t *sb = src + b * 34;
        uint8_t *db = dst_qs + b * 16;
        uint16_t d8u;
        memcpy(&d8u, sb, 2);
        float d8 = f16_to_f32_host(d8u);
        const int8_t *qs = (const int8_t *)(sb + 2);
        int max_a = 0, max_v = 0;
        for (int i = 0; i < 32; i++) {
            int a = qs[i] < 0 ? -qs[i] : qs[i];
            if (a > max_a) { max_a = a; max_v = qs[i]; }
        }
        float d4 = (max_a > 0) ? (d8 * (float)max_v / -8.0f) : 0.0f;
        float inv = (d4 != 0.0f) ? (d8 / d4) : 0.0f;
        ds[b] = f32_to_f16_host(d4);
        for (int j = 0; j < 16; j++) {
            int q0 = (int)(qs[j] * inv + 8.5f);
            if (q0 < 0) q0 = 0; else if (q0 > 15) q0 = 15;
            int q1 = (int)(qs[j + 16] * inv + 8.5f);
            if (q1 < 0) q1 = 0; else if (q1 > 15) q1 = 15;
            db[j] = (uint8_t)(q0 | (q1 << 4));
        }
    }
}

static void convert_f32_to_q4s(const uint8_t *src, size_t nbytes, uint8_t *dst_qs, uint8_t *dst_ds) {
    const float *f = (const float *)src;
    size_t nb = (nbytes / 4) / 32;
    uint16_t *ds = (uint16_t *)dst_ds;
    for (size_t b = 0; b < nb; b++) {
        const float *fb = f + b * 32;
        uint8_t *db = dst_qs + b * 16;
        float max_a = 0.0f, max_v = 0.0f;
        for (int i = 0; i < 32; i++) {
            float a = fb[i] < 0.0f ? -fb[i] : fb[i];
            if (a > max_a) { max_a = a; max_v = fb[i]; }
        }
        float d4 = (max_a > 0.0f) ? (max_v / -8.0f) : 0.0f;
        float inv = (d4 != 0.0f) ? (1.0f / d4) : 0.0f;
        ds[b] = f32_to_f16_host(d4);
        for (int j = 0; j < 16; j++) {
            int q0 = (int)(fb[j] * inv + 8.5f);
            if (q0 < 0) q0 = 0; else if (q0 > 15) q0 = 15;
            int q1 = (int)(fb[j + 16] * inv + 8.5f);
            if (q1 < 0) q1 = 0; else if (q1 > 15) q1 = 15;
            db[j] = (uint8_t)(q0 | (q1 << 4));
        }
    }
}

static void convert_bf16_to_q4s(const uint8_t *src, size_t nbytes, uint8_t *dst_qs, uint8_t *dst_ds) {
    const uint16_t *bf = (const uint16_t *)src;
    size_t nb = (nbytes / 2) / 32;
    uint16_t *ds = (uint16_t *)dst_ds;
    float fb[32];
    for (size_t b = 0; b < nb; b++) {
        const uint16_t *sb = bf + b * 32;
        uint8_t *db = dst_qs + b * 16;
        float max_a = 0.0f, max_v = 0.0f;
        for (int i = 0; i < 32; i++) {
            uint32_t bits = (uint32_t)sb[i] << 16;
            float v;
            memcpy(&v, &bits, sizeof v);
            fb[i] = v;
            float a = v < 0.0f ? -v : v;
            if (a > max_a) { max_a = a; max_v = v; }
        }
        float d4 = (max_a > 0.0f) ? (max_v / -8.0f) : 0.0f;
        float inv = (d4 != 0.0f) ? (1.0f / d4) : 0.0f;
        ds[b] = f32_to_f16_host(d4);
        for (int j = 0; j < 16; j++) {
            int q0 = (int)(fb[j] * inv + 8.5f);
            if (q0 < 0) q0 = 0; else if (q0 > 15) q0 = 15;
            int q1 = (int)(fb[j + 16] * inv + 8.5f);
            if (q1 < 0) q1 = 0; else if (q1 > 15) q1 = 15;
            db[j] = (uint8_t)(q0 | (q1 << 4));
        }
    }
}

typedef struct {
    const uint8_t *src;
    size_t src_bytes;
    uint8_t *dst;
    uint8_t *dst2; /* separate ds[] buffer for kind 7 (q8->q4s) and kind 8 (f32->q4s) */
    int kind; /* 0 = f32->f16, 1 = q8->q4, 2 = f32->f16 transpose, 3 = f32->q4, 4 = f32*norm->f16, 5 = q8->f16, 6 = gf, 7 = q8->q4s, 8 = f32->q4s, 9 = gf_rm, 10 = iq4->iq4s, 11 = q8->q8s, 12 = f32->q8s, 13 = ggml_gf_T, 14 = ggml_gf_rm, 15 = ggml->f16 */
    int64_t rows, cols;
    const float *norm_w;
    uint32_t wtype1, wtype2;
} FastJob;

static void convert_f32_norm_to_f16(const uint8_t *src, const float *nw, int64_t rows, int64_t cols, uint8_t *dst) {
    const float *f = (const float *)src;
    uint16_t *h = (uint16_t *)dst;
    for (int64_t r = 0; r < rows; r++) {
        const float *fr = f + r * cols;
        uint16_t *hr = h + r * cols;
        for (int64_t c = 0; c < cols; c++) hr[c] = f32_to_f16_host(fr[c] * nw[c]);
    }
}

static void convert_q8_to_f16(const uint8_t *src, size_t nbytes, uint8_t *dst) {
    size_t nb = nbytes / 34;
    uint16_t *o = (uint16_t *)dst;
    for (size_t b = 0; b < nb; b++) {
        const uint8_t *sb = src + b * 34;
        uint16_t d8u;
        memcpy(&d8u, sb, 2);
        float d8 = f16_to_f32_host(d8u);
        const int8_t *qs = (const int8_t *)(sb + 2);
        uint16_t *ob = o + b * 32;
        for (int i = 0; i < 32; i++) ob[i] = f32_to_f16_host(d8 * (float)qs[i]);
    }
}

static void convert_f32_gf_transpose(const uint8_t *g_src, const float *f_src, int64_t rows, int64_t cols, uint8_t *dst) {
    const float *g = (const float *)g_src;
    uint32_t *o = (uint32_t *)dst;
    for (int64_t r = 0; r < rows; r++) {
        const float *gr = g + r * cols;
        const float *fr = f_src + r * cols;
        for (int64_t c = 0; c < cols; c++) {
            uint16_t hg = f32_to_f16_host(gr[c]);
            uint16_t hf = f32_to_f16_host(fr[c]);
            o[c * rows + r] = (uint32_t)hg | ((uint32_t)hf << 16);
        }
    }
}

static void convert_f32_gf_rowmaj(const uint8_t *g_src, const float *f_src, int64_t rows, int64_t cols, uint8_t *dst) {
    const float *g = (const float *)g_src;
    uint32_t *o = (uint32_t *)dst;
    int64_t n = rows * cols;
    for (int64_t i = 0; i < n; i++) {
        uint16_t hg = f32_to_f16_host(g[i]);
        uint16_t hf = f32_to_f16_host(f_src[i]);
        o[i] = (uint32_t)hg | ((uint32_t)hf << 16);
    }
}

static size_t ggml_row_bytes_host(uint32_t wtype, int64_t cols) {
    int64_t be = ink_ggml_block_elems(wtype);
    int64_t bb = ink_ggml_block_bytes(wtype);
    return (be > 0 && bb > 0) ? (size_t)((cols / be) * bb) : 0;
}

static void convert_ggml_gf_transpose(const uint8_t *g_src, uint32_t gt, const uint8_t *f_src, uint32_t ft, int64_t rows, int64_t cols, uint8_t *dst) {
    uint32_t *o = (uint32_t *)dst;
    size_t grb = ggml_row_bytes_host(gt, cols);
    size_t frb = ggml_row_bytes_host(ft, cols);
    float stack_g[256], stack_f[256];
    float *gr = (cols <= 256) ? stack_g : (float *)malloc((size_t)cols * sizeof(float));
    float *fr = (cols <= 256) ? stack_f : (float *)malloc((size_t)cols * sizeof(float));
    if (!gr || !fr) { if (cols > 256) { free(gr); free(fr); } return; }
    for (int64_t r = 0; r < rows; r++) {
        ink_ggml_dequant_row(gt, g_src + (size_t)r * grb, gr, cols);
        ink_ggml_dequant_row(ft, f_src + (size_t)r * frb, fr, cols);
        for (int64_t c = 0; c < cols; c++) {
            uint16_t hg = f32_to_f16_host(gr[c]);
            uint16_t hf = f32_to_f16_host(fr[c]);
            o[c * rows + r] = (uint32_t)hg | ((uint32_t)hf << 16);
        }
    }
    if (cols > 256) { free(gr); free(fr); }
}

static void convert_ggml_gf_rowmaj(const uint8_t *g_src, uint32_t gt, const uint8_t *f_src, uint32_t ft, int64_t rows, int64_t cols, uint8_t *dst) {
    uint32_t *o = (uint32_t *)dst;
    size_t grb = ggml_row_bytes_host(gt, cols);
    size_t frb = ggml_row_bytes_host(ft, cols);
    float stack_g[256], stack_f[256];
    float *gr = (cols <= 256) ? stack_g : (float *)malloc((size_t)cols * sizeof(float));
    float *fr = (cols <= 256) ? stack_f : (float *)malloc((size_t)cols * sizeof(float));
    if (!gr || !fr) { if (cols > 256) { free(gr); free(fr); } return; }
    for (int64_t r = 0; r < rows; r++) {
        ink_ggml_dequant_row(gt, g_src + (size_t)r * grb, gr, cols);
        ink_ggml_dequant_row(ft, f_src + (size_t)r * frb, fr, cols);
        uint32_t *orow = o + r * cols;
        for (int64_t c = 0; c < cols; c++) {
            uint16_t hg = f32_to_f16_host(gr[c]);
            uint16_t hf = f32_to_f16_host(fr[c]);
            orow[c] = (uint32_t)hg | ((uint32_t)hf << 16);
        }
    }
    if (cols > 256) { free(gr); free(fr); }
}

static void convert_ggml_to_f16(const uint8_t *src, uint32_t wtype, int64_t rows, int64_t cols, uint8_t *dst) {
    uint16_t *o = (uint16_t *)dst;
    size_t rb = ggml_row_bytes_host(wtype, cols);
    float *tmp = (float *)malloc((size_t)cols * sizeof(float));
    if (!tmp) return;
    for (int64_t r = 0; r < rows; r++) {
        ink_ggml_dequant_row(wtype, src + (size_t)r * rb, tmp, cols);
        uint16_t *orow = o + r * cols;
        for (int64_t c = 0; c < cols; c++) orow[c] = f32_to_f16_host(tmp[c]);
    }
    free(tmp);
}

/* Sprint 71C: 100% lossless bit-exact split of 18B IQ4_NL (d:f16 + qs:16B) -> aligned 16B qs + 2B ds! */
static void convert_iq4_to_iq4s(const uint8_t *src, size_t nbytes, uint8_t *dst_qs, uint8_t *dst_ds) {
    size_t nb = nbytes / 18;
    uint16_t *ds = (uint16_t *)dst_ds;
    for (size_t b = 0; b < nb; b++) {
        const uint8_t *sb = src + b * 18;
        memcpy(&ds[b], sb, 2);
        memcpy(dst_qs + b * 16, sb + 2, 16);
    }
}

/* Sprint 73: 100% lossless bit-exact split of 34B Q8_0 (d:f16 + qs:32B) -> aligned 32B qs + 2B ds! */
static void convert_q8_to_q8s(const uint8_t *src, size_t nbytes, uint8_t *dst_qs, uint8_t *dst_ds) {
    size_t nb = nbytes / 34;
    uint16_t *ds = (uint16_t *)dst_ds;
    for (size_t b = 0; b < nb; b++) {
        const uint8_t *sb = src + b * 34;
        memcpy(&ds[b], sb, 2);
        memcpy(dst_qs + b * 32, sb + 2, 32);
    }
}

/* High-precision F32 -> Q8S (block-32 int8 + f16 scale) for tiny F32 control/router/sink/dyn projs. */
static void convert_f32_to_q8s(const uint8_t *src, size_t nbytes, uint8_t *dst_qs, uint8_t *dst_ds) {
    const float *f = (const float *)src;
    size_t nb = (nbytes / 4) / 32;
    uint16_t *ds = (uint16_t *)dst_ds;
    for (size_t b = 0; b < nb; b++) {
        const float *fb = f + b * 32;
        int8_t *db = (int8_t *)(dst_qs + b * 32);
        float max_a = 0.0f;
        for (int i = 0; i < 32; i++) {
            float a = fb[i] < 0.0f ? -fb[i] : fb[i];
            if (a > max_a) max_a = a;
        }
        float d8 = (max_a > 0.0f) ? (max_a / 127.0f) : 0.0f;
        uint16_t d8u = f32_to_f16_host(d8);
        ds[b] = d8u;
        float d8_f16 = f16_to_f32_host(d8u);
        float inv = (d8_f16 != 0.0f) ? (1.0f / d8_f16) : 0.0f;
        for (int i = 0; i < 32; i++) {
            int q = (int)lrintf(fb[i] * inv);
            if (q < -127) q = -127; else if (q > 127) q = 127;
            db[i] = (int8_t)q;
        }
    }
}

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
        else if (j->kind == 4) convert_f32_norm_to_f16(j->src, j->norm_w, j->rows, j->cols, j->dst);
        else if (j->kind == 5) convert_q8_to_f16(j->src, j->src_bytes, j->dst);
        else if (j->kind == 6) convert_f32_gf_transpose(j->src, j->norm_w, j->rows, j->cols, j->dst);
        else if (j->kind == 7) convert_q8_to_q4s(j->src, j->src_bytes, j->dst, j->dst2);
        else if (j->kind == 8) convert_f32_to_q4s(j->src, j->src_bytes, j->dst, j->dst2);
        else if (j->kind == 9) convert_f32_gf_rowmaj(j->src, j->norm_w, j->rows, j->cols, j->dst);
        else if (j->kind == 10) convert_iq4_to_iq4s(j->src, j->src_bytes, j->dst, j->dst2);
        else if (j->kind == 11) convert_q8_to_q8s(j->src, j->src_bytes, j->dst, j->dst2);
        else if (j->kind == 12) convert_f32_to_q8s(j->src, j->src_bytes, j->dst, j->dst2);
        else if (j->kind == 13) convert_ggml_gf_transpose(j->src, j->wtype1, (const uint8_t *)j->norm_w, j->wtype2, j->rows, j->cols, j->dst);
        else if (j->kind == 14) convert_ggml_gf_rowmaj(j->src, j->wtype1, (const uint8_t *)j->norm_w, j->wtype2, j->rows, j->cols, j->dst);
        else if (j->kind == 15) convert_ggml_to_f16(j->src, j->wtype1, j->rows, j->cols, j->dst);
        else convert_f32_to_f16(j->src, j->src_bytes, j->dst);
        /* Streamingly drop finished large repacked mmap pages so peak ru_maxrss stays ~18 GB instead of 32.7 GB! */
        if (j->src_bytes >= 262144 && (j->kind == 7 || j->kind == 10 || j->kind == 11)) {
            size_t ps = (size_t)getpagesize();
            uintptr_t lo = ((uintptr_t)j->src + ps - 1) & ~(uintptr_t)(ps - 1);
            uintptr_t hi = ((uintptr_t)j->src + j->src_bytes) & ~(uintptr_t)(ps - 1);
            if (hi > lo) madvise((void *)lo, hi - lo, MADV_DONTNEED);
        }
    }
    return NULL;
}

static void init_q4s(FastQ4S *q) { q->qs.off = q->ds.off = -1; q->qs.words = q->ds.words = 0; }

static void build_fast_weights(InkSession *s) {
    const InkHParams *hp = &s->gguf.hp;
    const char *q4_env = getenv("INK_FAST_Q4");
    int do_q4 = (q4_env != NULL && q4_env[0] != '0');
    const char *q8s_env = getenv("INK_FAST_Q8S");
    int do_q8s = (!do_q4) && (q8s_env ? (q8s_env[0] != '0') : (s->budget.headroom_mb >= 8192));
    int do_fast_s = (do_q4 || do_q8s);
    int64_t qblk_bytes = do_q8s ? 32 : 16;
    int64_t qblk_u16 = qblk_bytes / 2;
    s->fast_is_q8s = do_q8s ? 1 : 0;
    const char *iq4s_env = getenv("INK_FAST_IQ4S");
    int do_iq4s = iq4s_env ? (iq4s_env[0] != '0') : (s->budget.headroom_mb >= 16384);
    for (int ip = 0; ip < INK_MAX_SLOTS; ip++) {
        FastLayer *fl = &s->fast_layers[ip];
        fl->attn_q.off = fl->attn_k.off = fl->attn_v.off = fl->attn_output.off = fl->attn_gate.off = fl->attn_rel.off = fl->attn_qrg.off = -1;
        init_q4s(&fl->attn_qrgkv_s); init_q4s(&fl->attn_q_s); init_q4s(&fl->attn_rel_s); init_q4s(&fl->attn_gate_s);
        init_q4s(&fl->attn_k_s); init_q4s(&fl->attn_v_s); init_q4s(&fl->attn_output_s);
        init_q4s(&fl->attn_sink_s); init_q4s(&fl->attn_k_dyn_s); init_q4s(&fl->attn_v_dyn_s);
        init_q4s(&fl->ctrl_down_qkv_s); init_q4s(&fl->ctrl_down3_s); init_q4s(&fl->ffn_gate_inp_s);
        init_q4s(&fl->moe_gu_q4s); init_q4s(&fl->moe_dn_q4s);
        init_q4s(&fl->moe_gu_iq4s); init_q4s(&fl->moe_dn_iq4s);
        fl->moe_gu_q4.off = fl->moe_dn_q4.off = -1;
        fl->ffn_gate_inp.off = fl->ffn_gate_inp_normed.off = -1;
        fl->attn_sink_proj.off = fl->attn_k_conv_proj.off = fl->attn_v_conv_proj.off = -1;
        for (int c = 0; c < 4; c++) fl->ctrl_down[c].off = fl->ctrl_gain[c].off = fl->ctrl_forget[c].off = -1;
        fl->ctrl_down_qkv_q4.off = fl->ctrl_down3_q4.off = fl->ctrl_gain_qkv.off = fl->ctrl_forget_qkv.off = -1;
        fl->ctrl_gf_qkv.off = fl->ctrl_gf3.off = -1;
        fl->ctrl_gf_rm_qkv.off = fl->ctrl_gf_rm3.off = -1;
    }
    s->fast_output.off = -1;
    init_q4s(&s->fast_output_s);

    FastJob jobs[2048];
    int n_jobs = 0;
    int64_t total_bytes = 0;
    const uint8_t *base = (const uint8_t *)s->gguf.map;

#define ADD_FAST_F16(T, SPAN) do { \
    if ((T).present && (T).type == INK_GGML_F32 && n_jobs < 2048) { \
        int64_t bytes = (int64_t)((T).nbytes / 2); \
        (SPAN).off = total_bytes; (SPAN).words = bytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)total_bytes, NULL, 0, 0, 0, NULL, 0, 0 }; \
        total_bytes = (total_bytes + bytes + 63) & ~63LL; \
    } else if ((T).present && (T).type == INK_GGML_Q8_0 && n_jobs < 2048) { \
        int64_t nb = (int64_t)((T).nbytes / 34); \
        int64_t bytes = nb * 32 * 2; \
        (SPAN).off = total_bytes; (SPAN).words = bytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)total_bytes, NULL, 5, 0, 0, NULL, 0, 0 }; \
        total_bytes = (total_bytes + bytes + 63) & ~63LL; \
    } else if ((T).present && ink_ggml_type_supported((T).type) && (T).ndims >= 2 && n_jobs < 2048) { \
        int64_t _cols = (int64_t)(T).dims[0], _rows = (int64_t)(T).dims[1]; \
        int64_t bytes = _rows * _cols * 2; \
        (SPAN).off = total_bytes; (SPAN).words = bytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)total_bytes, NULL, 15, _rows, _cols, NULL, (T).type, 0 }; \
        total_bytes = (total_bytes + bytes + 63) & ~63LL; \
    } \
} while (0)

#define ADD_FAST_F16_T(T, SPAN, ROWS, COLS) do { \
    if ((T).present && (T).type == INK_GGML_F32 && n_jobs < 2048) { \
        int64_t bytes = (int64_t)((T).nbytes / 2); \
        (SPAN).off = total_bytes; (SPAN).words = bytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)total_bytes, NULL, 2, (ROWS), (COLS), NULL, 0, 0 }; \
        total_bytes = (total_bytes + bytes + 63) & ~63LL; \
    } \
} while (0)

#define ADD_FAST_Q4(T, SPAN) do { \
    if (do_q4 && (T).present && (T).type == INK_GGML_Q8_0 && n_jobs < 2048) { \
        int64_t nb = (int64_t)((T).nbytes / 34); \
        int64_t bytes = nb * 18; \
        (SPAN).off = total_bytes; (SPAN).words = bytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)total_bytes, NULL, 1, 0, 0, NULL, 0, 0 }; \
        total_bytes = (total_bytes + bytes + 63) & ~63LL; \
    } \
} while (0)

#define Q4S_OK(T) ((T).present && ((T).type == INK_GGML_Q8_0 || (T).type == INK_GGML_F32))
#define Q4S_NB(T) ((T).type == INK_GGML_Q8_0 ? (int64_t)((T).nbytes / 34) : (int64_t)((T).nbytes / 128))
#define Q4S_KIND(T) (do_q8s ? ((T).type == INK_GGML_Q8_0 ? 11 : 12) : ((T).type == INK_GGML_Q8_0 ? 7 : 8))

#define ADD_FAST_Q4S(T, Q4S) do { \
    if (do_fast_s && Q4S_OK(T) && n_jobs < 2048) { \
        int64_t nb = Q4S_NB(T); \
        int64_t qbytes = nb * qblk_bytes, dbytes = nb * 2; \
        int64_t qoff = total_bytes; \
        total_bytes = (total_bytes + qbytes + 63) & ~63LL; \
        int64_t doff = total_bytes; \
        total_bytes = (total_bytes + dbytes + 63) & ~63LL; \
        (Q4S).qs.off = qoff; (Q4S).qs.words = qbytes / 2; \
        (Q4S).ds.off = doff; (Q4S).ds.words = dbytes / 2; \
        jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, (uint8_t *)(uintptr_t)qoff, (uint8_t *)(uintptr_t)doff, Q4S_KIND(T), 0, 0, NULL, 0, 0 }; \
    } \
} while (0)

    for (int ip = 0; ip < (int)hp->n_phys; ip++) {
        const InkLayerTensors *L = &s->gguf.layers[ip];
        FastLayer *FL = &s->fast_layers[ip];
        if (do_fast_s && Q4S_OK(L->ffn_gate_inp)) {
            ADD_FAST_Q4S(L->ffn_gate_inp, FL->ffn_gate_inp_s);
        } else {
            ADD_FAST_F16(L->ffn_gate_inp, FL->ffn_gate_inp);
        }
        if (do_fast_s && Q4S_OK(L->loop_ctrl_down[0]) && Q4S_OK(L->loop_ctrl_down[1]) && Q4S_OK(L->loop_ctrl_down[2]) && n_jobs + 4 <= 2048) {
            int64_t nb0 = Q4S_NB(L->loop_ctrl_down[0]), nb1 = Q4S_NB(L->loop_ctrl_down[1]), nb2 = Q4S_NB(L->loop_ctrl_down[2]);
            int64_t tot_nb = nb0 + nb1 + nb2;
            int64_t qoff = total_bytes;
            total_bytes = (total_bytes + tot_nb * qblk_bytes + 63) & ~63LL;
            int64_t doff = total_bytes;
            total_bytes = (total_bytes + tot_nb * 2 + 63) & ~63LL;
            FL->ctrl_down_qkv_s.qs.off = qoff; FL->ctrl_down_qkv_s.qs.words = tot_nb * qblk_u16;
            FL->ctrl_down_qkv_s.ds.off = doff; FL->ctrl_down_qkv_s.ds.words = tot_nb;
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[0].offset, (size_t)L->loop_ctrl_down[0].nbytes,
                                        (uint8_t *)(uintptr_t)(qoff), (uint8_t *)(uintptr_t)(doff), Q4S_KIND(L->loop_ctrl_down[0]), 0, 0, NULL, 0, 0 };
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[1].offset, (size_t)L->loop_ctrl_down[1].nbytes,
                                        (uint8_t *)(uintptr_t)(qoff + nb0 * qblk_bytes), (uint8_t *)(uintptr_t)(doff + nb0 * 2), Q4S_KIND(L->loop_ctrl_down[1]), 0, 0, NULL, 0, 0 };
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[2].offset, (size_t)L->loop_ctrl_down[2].nbytes,
                                        (uint8_t *)(uintptr_t)(qoff + (nb0 + nb1) * qblk_bytes), (uint8_t *)(uintptr_t)(doff + (nb0 + nb1) * 2), Q4S_KIND(L->loop_ctrl_down[2]), 0, 0, NULL, 0, 0 };
            ADD_FAST_Q4S(L->loop_ctrl_down[3], FL->ctrl_down3_s);
        } else if (L->loop_ctrl_down[0].present && L->loop_ctrl_down[0].type == INK_GGML_F32 &&
                   L->loop_ctrl_down[1].present && L->loop_ctrl_down[1].type == INK_GGML_F32 &&
                   L->loop_ctrl_down[2].present && L->loop_ctrl_down[2].type == INK_GGML_F32 && n_jobs + 4 <= 2048) {
            int64_t b0 = (int64_t)(L->loop_ctrl_down[0].nbytes / 2);
            int64_t b1 = (int64_t)(L->loop_ctrl_down[1].nbytes / 2);
            int64_t b2 = (int64_t)(L->loop_ctrl_down[2].nbytes / 2);
            FL->ctrl_down_qkv_q4.off = total_bytes;
            FL->ctrl_down_qkv_q4.words = (b0 + b1 + b2) / 2;
            FL->ctrl_down[0].off = total_bytes; FL->ctrl_down[0].words = b0 / 2;
            FL->ctrl_down[1].off = total_bytes + b0; FL->ctrl_down[1].words = b1 / 2;
            FL->ctrl_down[2].off = total_bytes + b0 + b1; FL->ctrl_down[2].words = b2 / 2;
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[0].offset, (size_t)L->loop_ctrl_down[0].nbytes, (uint8_t *)(uintptr_t)(total_bytes), NULL, 0, 0, 0, NULL, 0, 0 };
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[1].offset, (size_t)L->loop_ctrl_down[1].nbytes, (uint8_t *)(uintptr_t)(total_bytes + b0), NULL, 0, 0, 0, NULL, 0, 0 };
            jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_down[2].offset, (size_t)L->loop_ctrl_down[2].nbytes, (uint8_t *)(uintptr_t)(total_bytes + b0 + b1), NULL, 0, 0, 0, NULL, 0, 0 };
            total_bytes = (total_bytes + b0 + b1 + b2 + 63) & ~63LL;
            ADD_FAST_F16(L->loop_ctrl_down[3], FL->ctrl_down[3]);
        } else {
            for (int c = 0; c < 4; c++) ADD_FAST_F16(L->loop_ctrl_down[c], FL->ctrl_down[c]);
        }
        if (L->loop_ctrl_gain[0].present && ink_ggml_type_supported(L->loop_ctrl_gain[0].type) &&
            L->loop_ctrl_gain[1].present && ink_ggml_type_supported(L->loop_ctrl_gain[1].type) &&
            L->loop_ctrl_gain[2].present && ink_ggml_type_supported(L->loop_ctrl_gain[2].type) &&
            L->loop_ctrl_forget[0].present && ink_ggml_type_supported(L->loop_ctrl_forget[0].type) &&
            L->loop_ctrl_forget[1].present && ink_ggml_type_supported(L->loop_ctrl_forget[1].type) &&
            L->loop_ctrl_forget[2].present && ink_ggml_type_supported(L->loop_ctrl_forget[2].type) && n_jobs + 8 <= 2048) {
            int64_t bgf = hp->n_embd * hp->gate_rank * 4; /* gain(f16)+forget(f16) = 4 bytes/elem */
            FL->ctrl_gf_qkv.off = total_bytes; FL->ctrl_gf_qkv.words = (3 * bgf) / 2;
            for (int c = 0; c < 3; c++) {
                int is_f32_pair = (L->loop_ctrl_gain[c].type == INK_GGML_F32 && L->loop_ctrl_forget[c].type == INK_GGML_F32);
                jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_gain[c].offset, (size_t)L->loop_ctrl_gain[c].nbytes,
                                            (uint8_t *)(uintptr_t)(total_bytes + c * bgf), NULL,
                                            is_f32_pair ? 6 : 13, hp->n_embd, hp->gate_rank,
                                            (const float *)(base + L->loop_ctrl_forget[c].offset),
                                            L->loop_ctrl_gain[c].type, L->loop_ctrl_forget[c].type };
            }
            total_bytes = (total_bytes + 3 * bgf + 63) & ~63LL;
            /* Sprint 70: Row-major [dim, rank] half2 copy for 256t decode (-33%, 28 MB total)! */
            FL->ctrl_gf_rm_qkv.off = total_bytes; FL->ctrl_gf_rm_qkv.words = (3 * bgf) / 2;
            for (int c = 0; c < 3; c++) {
                int is_f32_pair = (L->loop_ctrl_gain[c].type == INK_GGML_F32 && L->loop_ctrl_forget[c].type == INK_GGML_F32);
                jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_gain[c].offset, (size_t)L->loop_ctrl_gain[c].nbytes,
                                            (uint8_t *)(uintptr_t)(total_bytes + c * bgf), NULL,
                                            is_f32_pair ? 9 : 14, hp->n_embd, hp->gate_rank,
                                            (const float *)(base + L->loop_ctrl_forget[c].offset),
                                            L->loop_ctrl_gain[c].type, L->loop_ctrl_forget[c].type };
            }
            total_bytes = (total_bytes + 3 * bgf + 63) & ~63LL;
            if (L->loop_ctrl_gain[3].present && ink_ggml_type_supported(L->loop_ctrl_gain[3].type) &&
                L->loop_ctrl_forget[3].present && ink_ggml_type_supported(L->loop_ctrl_forget[3].type)) {
                int is_f32_pair = (L->loop_ctrl_gain[3].type == INK_GGML_F32 && L->loop_ctrl_forget[3].type == INK_GGML_F32);
                FL->ctrl_gf3.off = total_bytes; FL->ctrl_gf3.words = bgf / 2;
                jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_gain[3].offset, (size_t)L->loop_ctrl_gain[3].nbytes,
                                            (uint8_t *)(uintptr_t)total_bytes, NULL,
                                            is_f32_pair ? 6 : 13, hp->n_embd, hp->gate_rank,
                                            (const float *)(base + L->loop_ctrl_forget[3].offset),
                                            L->loop_ctrl_gain[3].type, L->loop_ctrl_forget[3].type };
                total_bytes = (total_bytes + bgf + 63) & ~63LL;
                FL->ctrl_gf_rm3.off = total_bytes; FL->ctrl_gf_rm3.words = bgf / 2;
                jobs[n_jobs++] = (FastJob){ base + L->loop_ctrl_gain[3].offset, (size_t)L->loop_ctrl_gain[3].nbytes,
                                            (uint8_t *)(uintptr_t)total_bytes, NULL,
                                            is_f32_pair ? 9 : 14, hp->n_embd, hp->gate_rank,
                                            (const float *)(base + L->loop_ctrl_forget[3].offset),
                                            L->loop_ctrl_gain[3].type, L->loop_ctrl_forget[3].type };
                total_bytes = (total_bytes + bgf + 63) & ~63LL;
            }
        } else {
            for (int c = 0; c < 4; c++) {
                ADD_FAST_F16_T(L->loop_ctrl_gain[c], FL->ctrl_gain[c], hp->n_embd, hp->gate_rank);
                ADD_FAST_F16_T(L->loop_ctrl_forget[c], FL->ctrl_forget[c], hp->n_embd, hp->gate_rank);
            }
        }
        int has_dyn_pair = (!hp->is_swa[ip] && Q4S_OK(L->attn_k_conv_proj) && Q4S_OK(L->attn_v_conv_proj));
        int dyn_ok = (hp->is_swa[ip] || has_dyn_pair);
        if (do_fast_s && Q4S_OK(L->attn_q) && Q4S_OK(L->attn_rel) && Q4S_OK(L->attn_gate) &&
            Q4S_OK(L->attn_k) && Q4S_OK(L->attn_v) && Q4S_OK(L->attn_sink_proj) && dyn_ok && n_jobs + 8 <= 2048) {
            int64_t nbq = Q4S_NB(L->attn_q), nbr = Q4S_NB(L->attn_rel), nbg = Q4S_NB(L->attn_gate);
            int64_t nbk = Q4S_NB(L->attn_k), nbv = Q4S_NB(L->attn_v), nbs = Q4S_NB(L->attn_sink_proj);
            int64_t nbkd = has_dyn_pair ? Q4S_NB(L->attn_k_conv_proj) : 0;
            int64_t nbvd = has_dyn_pair ? Q4S_NB(L->attn_v_conv_proj) : 0;
            int64_t tot_nb = nbq + nbr + nbg + nbk + nbv + nbs + nbkd + nbvd;
            int64_t qoff = total_bytes;
            total_bytes = (total_bytes + tot_nb * qblk_bytes + 63) & ~63LL;
            int64_t doff = total_bytes;
            total_bytes = (total_bytes + tot_nb * 2 + 63) & ~63LL;
            FL->attn_qrgkv_s.qs.off = qoff; FL->attn_qrgkv_s.qs.words = tot_nb * qblk_u16;
            FL->attn_qrgkv_s.ds.off = doff; FL->attn_qrgkv_s.ds.words = tot_nb;
            int64_t cur_nb = 0;
#define PACK_Q4S_SUB(T, NB, SUB) do { \
    (SUB).qs.off = qoff + cur_nb * qblk_bytes; (SUB).qs.words = (NB) * qblk_u16; \
    (SUB).ds.off = doff + cur_nb * 2;  (SUB).ds.words = (NB); \
    jobs[n_jobs++] = (FastJob){ base + (T).offset, (size_t)(T).nbytes, \
                                (uint8_t *)(uintptr_t)(qoff + cur_nb * qblk_bytes), (uint8_t *)(uintptr_t)(doff + cur_nb * 2), \
                                Q4S_KIND(T), 0, 0, NULL, 0, 0 }; \
    cur_nb += (NB); \
} while (0)
            PACK_Q4S_SUB(L->attn_q, nbq, FL->attn_q_s);
            PACK_Q4S_SUB(L->attn_rel, nbr, FL->attn_rel_s);
            PACK_Q4S_SUB(L->attn_gate, nbg, FL->attn_gate_s);
            PACK_Q4S_SUB(L->attn_k, nbk, FL->attn_k_s);
            PACK_Q4S_SUB(L->attn_v, nbv, FL->attn_v_s);
            PACK_Q4S_SUB(L->attn_sink_proj, nbs, FL->attn_sink_s);
            if (has_dyn_pair) {
                PACK_Q4S_SUB(L->attn_k_conv_proj, nbkd, FL->attn_k_dyn_s);
                PACK_Q4S_SUB(L->attn_v_conv_proj, nbvd, FL->attn_v_dyn_s);
            }
#undef PACK_Q4S_SUB
        } else {
            ADD_FAST_F16(L->attn_sink_proj, FL->attn_sink_proj);
            ADD_FAST_F16(L->attn_k_conv_proj, FL->attn_k_conv_proj);
            ADD_FAST_F16(L->attn_v_conv_proj, FL->attn_v_conv_proj);
            ADD_FAST_Q4(L->attn_q, FL->attn_q);
            ADD_FAST_Q4(L->attn_rel, FL->attn_rel);
            ADD_FAST_Q4(L->attn_gate, FL->attn_gate);
            ADD_FAST_Q4(L->attn_k, FL->attn_k);
            ADD_FAST_Q4(L->attn_v, FL->attn_v);
        }
        ADD_FAST_Q4S(L->attn_output, FL->attn_output_s);
        ADD_FAST_Q4S(L->ffn_gate_up_exps, FL->moe_gu_q4s);
        ADD_FAST_Q4S(L->ffn_down_exps, FL->moe_dn_q4s);
    }
    {
        const char *qlm_env = getenv("INK_Q4_LM_HEAD");
        int do_q4_lm = do_q4 || (qlm_env != NULL && qlm_env[0] != '0');
        int _dq = do_q4, _dq8 = do_q8s, _dfs = do_fast_s;
        int64_t _qbb = qblk_bytes, _qbu = qblk_u16;
        if (do_q4_lm) {
            do_q4 = 1; do_q8s = 0; do_fast_s = 1; qblk_bytes = 16; qblk_u16 = 8;
            s->fast_output_is_q4s = 1;
        } else {
            s->fast_output_is_q4s = 0;
        }
        ADD_FAST_Q4S(s->gguf.output, s->fast_output_s);
        do_q4 = _dq; do_q8s = _dq8; do_fast_s = _dfs; qblk_bytes = _qbb; qblk_u16 = _qbu;
    }
#undef ADD_FAST_F16
#undef ADD_FAST_F16_T
#undef ADD_FAST_Q4
#undef ADD_FAST_Q4S
#undef Q4S_OK
#undef Q4S_NB
#undef Q4S_KIND

    if (total_bytes > 0) {
        if (pm_alloc_unset((size_t)total_bytes, &s->fast_buf) != PM_OK) {
            s->fast_buf = NULL;
        } else {
            uint8_t *dst_base = (uint8_t *)pm_contents(s->fast_buf);
            /* Chunk large Q4S/Q8S jobs (> 16 MB) so all worker threads finish with zero tail stragglers. */
            FastJob exp_jobs[4096];
            int n_exp = 0;
            for (int i = 0; i < n_jobs; i++) {
                FastJob j = jobs[i];
                j.dst = dst_base + (uintptr_t)j.dst;
                int is_split = (j.kind == 7 || j.kind == 8 || j.kind == 11 || j.kind == 12);
                if (is_split) j.dst2 = dst_base + (uintptr_t)j.dst2;
                if (is_split && j.src_bytes > 16ULL * 1024 * 1024 && n_exp + 8 <= 4096) {
                    size_t bsz = (j.kind == 7 || j.kind == 11) ? 34 : 128;
                    size_t qstep = (j.kind == 11 || j.kind == 12) ? 32 : 16;
                    size_t nb = j.src_bytes / bsz;
                    int chunks = 8;
                    size_t per = (nb + (size_t)chunks - 1) / (size_t)chunks;
                    for (int c = 0; c < chunks; c++) {
                        size_t b0 = (size_t)c * per;
                        if (b0 >= nb) break;
                        size_t bc = (b0 + per <= nb) ? per : (nb - b0);
                        FastJob sub = j;
                        sub.src = j.src + b0 * bsz;
                        sub.src_bytes = bc * bsz;
                        sub.dst = j.dst + b0 * qstep;
                        sub.dst2 = j.dst2 + b0 * 2;
                        exp_jobs[n_exp++] = sub;
                    }
                } else if (n_exp < 4096) {
                    exp_jobs[n_exp++] = j;
                }
            }
            int nt = fast_worker_threads();
            pthread_t th[16];
            FastWorkerArg args[16];
            for (int i = 0; i < nt; i++) {
                args[i] = (FastWorkerArg){ exp_jobs, n_exp, i, nt };
                pthread_create(&th[i], NULL, fast_worker, &args[i]);
            }
            for (int i = 0; i < nt; i++) pthread_join(th[i], NULL);
            pm_mark_readonly(s->fast_buf);
        }
    }

    /* Sprint 71C: 100% lossless aligned IQ4S MoE repack into dedicated iq4s_buf (2.21x SLC hit BW, +11-21% DRAM BW!). */
    if (do_iq4s) {
        FastJob iq_jobs[512];
        int n_iq = 0;
        int64_t iq_bytes = 0;
#define ADD_IQ4S(T, Q4S) do { \
    if ((T).present && (T).type == INK_GGML_IQ4_NL && ((T).nbytes % 18) == 0 && n_iq + 8 <= 512) { \
        int64_t nb = (int64_t)((T).nbytes / 18); \
        int64_t qbytes = nb * 16, dbytes = nb * 2; \
        int64_t qoff = iq_bytes; iq_bytes = (iq_bytes + qbytes + 63) & ~63LL; \
        int64_t doff = iq_bytes; iq_bytes = (iq_bytes + dbytes + 63) & ~63LL; \
        (Q4S).qs.off = qoff; (Q4S).qs.words = qbytes / 2; \
        (Q4S).ds.off = doff; (Q4S).ds.words = dbytes / 2; \
        int chunks = (nb >= 65536) ? 8 : 1; \
        int64_t per = (nb + chunks - 1) / chunks; \
        for (int _c = 0; _c < chunks; _c++) { \
            int64_t _b0 = (int64_t)_c * per; \
            if (_b0 >= nb) break; \
            int64_t _bc = (_b0 + per <= nb) ? per : (nb - _b0); \
            iq_jobs[n_iq++] = (FastJob){ base + (T).offset + _b0 * 18, (size_t)(_bc * 18), \
                                         (uint8_t *)(uintptr_t)(qoff + _b0 * 16), (uint8_t *)(uintptr_t)(doff + _b0 * 2), 10, 0, 0, NULL, 0, 0 }; \
        } \
    } \
} while (0)
        for (int ip = 0; ip < (int)hp->n_phys; ip++) {
            const InkLayerTensors *L = &s->gguf.layers[ip];
            FastLayer *FL = &s->fast_layers[ip];
            if (FL->moe_gu_q4s.qs.off < 0) ADD_IQ4S(L->ffn_gate_up_exps, FL->moe_gu_iq4s);
            if (FL->moe_dn_q4s.qs.off < 0) ADD_IQ4S(L->ffn_down_exps, FL->moe_dn_iq4s);
        }
#undef ADD_IQ4S
        if (iq_bytes > 0) {
            if (pm_alloc_unset((size_t)iq_bytes, &s->iq4s_buf) == PM_OK) {
                uint8_t *iq_base = (uint8_t *)pm_contents(s->iq4s_buf);
                for (int i = 0; i < n_iq; i++) {
                    iq_jobs[i].dst = iq_base + (uintptr_t)iq_jobs[i].dst;
                    iq_jobs[i].dst2 = iq_base + (uintptr_t)iq_jobs[i].dst2;
                }
                int nt = fast_worker_threads();
                pthread_t th[16];
                FastWorkerArg args[16];
                for (int i = 0; i < nt; i++) {
                    args[i] = (FastWorkerArg){ iq_jobs, n_iq, i, nt };
                    pthread_create(&th[i], NULL, fast_worker, &args[i]);
                }
                for (int i = 0; i < nt; i++) pthread_join(th[i], NULL);
                pm_mark_readonly(s->iq4s_buf);
            } else {
                s->iq4s_buf = NULL;
                for (int ip = 0; ip < (int)hp->n_phys; ip++) {
                    init_q4s(&s->fast_layers[ip].moe_gu_iq4s);
                    init_q4s(&s->fast_layers[ip].moe_dn_iq4s);
                }
            }
        }
    }
    if (!s->fast_buf) return;

    /* Sprint 71B/C + Phase A2: verify all layers support fast tiny-batch (B=2..8) grouped-MoE verification
       for BOTH INK_FAST_Q4=1 AND default lossless Q8_0 (K_DV_Q8_MMA32_NB + K_MOE_*_Q8_NB)! */
    int ok_nb = ((s->fast_output_s.qs.off >= 0 || s->gguf.output.type == INK_GGML_Q8_0) &&
                 hp->n_head == hp->n_head_kv * 3 && hp->head_dim == 128 &&
                 hp->n_expert <= 128 && hp->n_expert_used <= 8 && hp->gate_rank <= 64 &&
                 ((hp->n_embd + 31) / 32) <= 128 &&
                 (hp->loop_end - hp->loop_start) * 8 <= s->prefill_b &&
                 ((hp->n_head * hp->head_dim) % 2 == 0) && ((hp->n_head * hp->rel_len) % 2 == 0) &&
                 ((hp->n_head_kv * hp->head_dim) % 2 == 0) && (hp->n_head % 2 == 0));
    for (int ip = 0; ip < hp->n_phys && ok_nb; ip++) {
        const InkLayerTensors *L = &s->gguf.layers[ip];
        const FastLayer *FL = &s->fast_layers[ip];
        int is_swa = hp->is_swa[ip];
        int has_k_dyn = (!is_swa && L->attn_k_conv_proj.present);
        int has_v_dyn = (!is_swa && L->attn_v_conv_proj.present);
        if (!L->ffn_res_query.present) ok_nb = 0;
        int attn_q4s_ok = (FL->attn_qrgkv_s.qs.off >= 0 && FL->attn_q_s.qs.off >= 0 && FL->attn_sink_s.qs.off >= 0 &&
                           FL->attn_output_s.qs.off >= 0 && FL->ffn_gate_inp_s.qs.off >= 0 &&
                           (!has_k_dyn || (FL->attn_k_dyn_s.qs.off >= 0 && FL->attn_v_dyn_s.qs.off >= 0)));
        int attn_q8_ok = (L->attn_q.type == INK_GGML_Q8_0 && L->attn_rel.type == INK_GGML_Q8_0 &&
                          L->attn_gate.type == INK_GGML_Q8_0 && L->attn_k.type == INK_GGML_Q8_0 &&
                          L->attn_v.type == INK_GGML_Q8_0 && L->attn_output.type == INK_GGML_Q8_0 &&
                          FL->attn_sink_proj.off >= 0 && FL->ffn_gate_inp.off >= 0 &&
                          (!has_k_dyn || (FL->attn_k_conv_proj.off >= 0 && FL->attn_v_conv_proj.off >= 0)));
        if (!attn_q4s_ok && !attn_q8_ok) ok_nb = 0;
        if (has_k_dyn != has_v_dyn) ok_nb = 0;
        if (has_k_dyn && ((hp->n_head_kv * hp->conv_len) % 2 != 0)) ok_nb = 0;
        int moe_ok = (FL->moe_gu_q4s.qs.off >= 0 && FL->moe_dn_q4s.qs.off >= 0) ||
                     (ink_ggml_type_supported(L->ffn_gate_up_exps.type) && ink_ggml_type_supported(L->ffn_down_exps.type));
        if (!moe_ok) ok_nb = 0;
        if (ip >= hp->loop_start && ip < hp->loop_end && hp->loop_count > 1) {
            int ctrl_q4s_ok = (FL->ctrl_down_qkv_s.qs.off >= 0 && FL->ctrl_down3_s.qs.off >= 0);
            int ctrl_f16_ok = (FL->ctrl_down_qkv_q4.off >= 0 && FL->ctrl_down[3].off >= 0);
            if (!L->attn_res_query.present || (!ctrl_q4s_ok && !ctrl_f16_ok) ||
                FL->ctrl_gf_rm_qkv.off < 0 || FL->ctrl_gf_rm3.off < 0) ok_nb = 0;
        }
    }
    s->can_verify_nb = ok_nb;
}

/* Compact only the non-repacked tensors (~560 MB: tok_embd + small F32 norms/convs/projs) into
 * s->weights_buf and unmap the 16.4 GiB GGUF file when 100% of large Q8_0/IQ4_NL tensors live in
 * fast_buf + iq4s_buf. Cuts peak RSS & Metal 4 residency footprint by ~15.4 GB! */
static int compact_or_wrap_weights(InkSession *s, char *err, size_t err_bytes) {
    const InkHParams *hp = &s->gguf.hp;
    int can_compact = (s->fast_buf != NULL && s->fast_is_q8s && s->iq4s_buf != NULL &&
                       s->fast_output_s.qs.off >= 0 && hp->head_dim == 128 && hp->rel_len == 16 &&
                       !getenv("INK_KEEP_GGUF") && !getenv("INK_SPEC_FP32") && !getenv("INK_VERIFY") &&
                       !getenv("INK_M4_BATCH_MIN") && !getenv("INK_DIRECT_THRESH") && !getenv("INK_OLD_ATTN"));
    for (int ip = 0; can_compact && ip < hp->n_phys; ip++) {
        const FastLayer *FL = &s->fast_layers[ip];
        if (FL->attn_q_s.qs.off < 0 || FL->attn_k_s.qs.off < 0 || FL->attn_v_s.qs.off < 0 ||
            FL->attn_rel_s.qs.off < 0 || FL->attn_gate_s.qs.off < 0 || FL->attn_output_s.qs.off < 0) {
            can_compact = 0;
        }
        int moe_ok = (FL->moe_gu_iq4s.qs.off >= 0 && FL->moe_dn_iq4s.qs.off >= 0) ||
                     (FL->moe_gu_q4s.qs.off >= 0 && FL->moe_dn_q4s.qs.off >= 0);
        if (!moe_ok) can_compact = 0;
    }
    if (!can_compact) {
        if (pm_wrap(s->gguf.map, s->gguf.map_size, &s->weights_buf) != PM_OK) {
            snprintf(err, err_bytes, "pm_wrap(%zu): %s", s->gguf.map_size, pm_log());
            return 1;
        }
        return 0;
    }

    InkTensor *keep[1024];
    int n_keep = 0;
    size_t total_bytes = 0;
#define KEEP_T(T) do { \
    if ((T).present && (T).nbytes > 0 && n_keep < 1024) { \
        keep[n_keep++] = &(T); \
        total_bytes = (total_bytes + (size_t)(T).nbytes + 63ULL) & ~63ULL; \
    } \
} while (0)
    KEEP_T(s->gguf.tok_embd);
    KEEP_T(s->gguf.tok_norm);
    KEEP_T(s->gguf.out_norm);
    KEEP_T(s->gguf.out_res_query);
    for (int ip = 0; ip < hp->n_phys; ip++) {
        InkLayerTensors *L = &s->gguf.layers[ip];
        KEEP_T(L->attn_norm);
        KEEP_T(L->attn_q_inp_norm);
        KEEP_T(L->attn_k_inp_norm);
        KEEP_T(L->attn_v_inp_norm);
        KEEP_T(L->attn_q_clip);
        KEEP_T(L->attn_rel_clip);
        KEEP_T(L->attn_rel_bias);
        KEEP_T(L->attn_k_conv);
        KEEP_T(L->attn_v_conv);
        KEEP_T(L->attn_post_conv);
        KEEP_T(L->attn_res_query);
        KEEP_T(L->attn_sink_proj);
        KEEP_T(L->attn_k_conv_proj);
        KEEP_T(L->attn_v_conv_proj);
        KEEP_T(L->ffn_norm);
        KEEP_T(L->ffn_post_conv);
        KEEP_T(L->ffn_res_query);
        KEEP_T(L->ffn_exp_probs_b);
        KEEP_T(L->ffn_gate_inp);
        KEEP_T(L->loop_msg_query);
        for (int c = 0; c < 4; c++) {
            KEEP_T(L->loop_ctrl_down[c]);
            KEEP_T(L->loop_ctrl_gain[c]);
            KEEP_T(L->loop_ctrl_forget[c]);
        }
    }
#undef KEEP_T

    if (total_bytes == 0 || pm_alloc_unset(total_bytes, &s->weights_buf) != PM_OK) {
        if (pm_wrap(s->gguf.map, s->gguf.map_size, &s->weights_buf) != PM_OK) {
            snprintf(err, err_bytes, "pm_wrap(%zu): %s", s->gguf.map_size, pm_log());
            return 1;
        }
        return 0;
    }
    uint8_t *dst = (uint8_t *)pm_contents(s->weights_buf);
    const uint8_t *src = s->gguf.map;
    size_t cur = 0;
    for (int i = 0; i < n_keep; i++) {
        memcpy(dst + cur, src + keep[i]->offset, (size_t)keep[i]->nbytes);
        keep[i]->offset = (uint64_t)cur;
        cur = (cur + (size_t)keep[i]->nbytes + 63ULL) & ~63ULL;
    }
    pm_mark_readonly(s->weights_buf);
    munmap(s->gguf.map, s->gguf.map_size);
    s->gguf.map = NULL;
    s->gguf.map_size = 0;
    return 0;
}

/* ---- Sprint 72: Native Coax + Metal 4 DSpark (mtp.safetensors) speculative drafter ---- */

static float bf16_to_f32_host(uint16_t u) {
    uint32_t b = (uint32_t)u << 16;
    float f;
    memcpy(&f, &b, sizeof f);
    return f;
}

static PmArg a_mtp_u16(const DsparkState *d, const MtpSpan *sp) {
    return a_buf(d->mtp_buf, sp->off, sp->bytes / 2);
}

static int dspark_mm_bf16(InkSession *s, PmBatch batch, int64_t rows, int64_t cols, int64_t nb,
                          PmBuffer w_buf, int64_t w_byte_off,
                          PmBuffer x_buf, int64_t x_u16_off,
                          PmBuffer out_buf, int64_t out_f32_off) {
    PmArg w_arg = a_buf(w_buf, w_byte_off, rows * (cols / 2));
    PmArg x_arg = a_u16v(x_buf, x_u16_off, nb * cols);
    PmArg o_arg = a_f32v(out_buf, out_f32_off, nb * rows);
    if (nb >= 33) {
        int64_t items = ((nb + 63) / 64) * ((rows + 127) / 128);
        CALL(s, batch, K_MM_BF16, items, a_int(rows), a_int(cols), a_int(nb), w_arg, x_arg, o_arg);
    } else if (nb >= 17) {
        int64_t items = ((nb + 31) / 32) * ((rows + 63) / 64);
        CALL(s, batch, K_MM_BF16_32, items, a_int(rows), a_int(cols), a_int(nb), w_arg, x_arg, o_arg);
    } else {
        int64_t items = ((nb + 15) / 16) * ((rows + 63) / 64);
        CALL(s, batch, K_MM_BF16_16, items, a_int(rows), a_int(cols), a_int(nb), w_arg, x_arg, o_arg);
    }
    return 0;
}

static int mtp_find_span(const char *hdr, int64_t data_start, size_t file_size,
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

static void ink_dspark_close(InkSession *s) {
    if (!s) return;
    DsparkState *d = &s->dspark;
    for (int i = 0; i < 3; i++) {
        pm_free(d->layers[i].kv_k); d->layers[i].kv_k = NULL;
        pm_free(d->layers[i].kv_v); d->layers[i].kv_v = NULL;
    }
    pm_free(d->inv_freq); d->inv_freq = NULL;
    pm_free(d->rnn_w_step); d->rnn_w_step = NULL;
    pm_free(d->rnn_w_ctx); d->rnn_w_ctx = NULL;
    pm_free(d->fold_w_qs); d->fold_w_qs = NULL;
    pm_free(d->fold_w_ds); d->fold_w_ds = NULL;
    pm_free(d->fold_b); d->fold_b = NULL;
    pm_free(d->taps); d->taps = NULL;
    pm_free(d->last_taps); d->last_taps = NULL;
    pm_free(d->ctx_bf16); d->ctx_bf16 = NULL;
    pm_free(d->ctx_k_f32); d->ctx_k_f32 = NULL;
    pm_free(d->ctx_v_f32); d->ctx_v_f32 = NULL;
    pm_free(d->anchor_emb); d->anchor_emb = NULL;
    pm_free(d->f_rnn_bf16); d->f_rnn_bf16 = NULL;
    pm_free(d->res); d->res = NULL;
    pm_free(d->x_bf16); d->x_bf16 = NULL;
    pm_free(d->x_f32); d->x_f32 = NULL;
    pm_free(d->q_f32); d->q_f32 = NULL;
    pm_free(d->k_f32); d->k_f32 = NULL;
    pm_free(d->v_f32); d->v_f32 = NULL;
    pm_free(d->qkv_f32); d->qkv_f32 = NULL;
    pm_free(d->attn_bf16); d->attn_bf16 = NULL;
    pm_free(d->attn_f32); d->attn_f32 = NULL;
    pm_free(d->tmp_f32); d->tmp_f32 = NULL;
    pm_free(d->gate_f32); d->gate_f32 = NULL;
    pm_free(d->up_f32); d->up_f32 = NULL;
    pm_free(d->gu_f32); d->gu_f32 = NULL;
    pm_free(d->mlp_bf16); d->mlp_bf16 = NULL;
    pm_free(d->mlp_f32); d->mlp_f32 = NULL;
    pm_free(d->markov_in_bf16); d->markov_in_bf16 = NULL;
    pm_free(d->rnn_ctx); d->rnn_ctx = NULL;
    pm_free(d->rnn_state); d->rnn_state = NULL;
    pm_free(d->rnn_state2); d->rnn_state2 = NULL;
    pm_free(d->rnn_latent); d->rnn_latent = NULL;
    pm_free(d->base_logits); d->base_logits = NULL;
    pm_free(d->delta_logits); d->delta_logits = NULL;
    pm_free(d->s1_m); d->s1_m = NULL;
    pm_free(d->s1_i); d->s1_i = NULL;
    pm_free(d->s1_s); d->s1_s = NULL;
    pm_free(d->draft_toks); d->draft_toks = NULL;
    pm_free(d->draft_probs); d->draft_probs = NULL;
    pm_free(d->draft_q4s_buf); d->draft_q4s_buf = NULL;
    pm_free(d->mtp_buf); d->mtp_buf = NULL;
    if (d->map && d->map != MAP_FAILED) {
        munmap(d->map, d->map_size);
        d->map = NULL;
    }
    if (d->fd >= 0) {
        close(d->fd);
        d->fd = -1;
    }
    d->ready = 0;
}

static void ink_dspark_open(InkSession *s, const char *gguf_path) {
    DsparkState *d = &s->dspark;
    d->ready = 0;
    const char *ds_env = getenv("INK_DSPARK");
    /* Default ON whenever mtp.safetensors is present (Cascade utility gate prevents open-prose slowdowns); opt-out via INK_DSPARK=0 */
    if (ds_env && ds_env[0] == '0') return;
    const InkHParams *hp = &s->gguf.hp;
    if (!s->can_verify_nb || !s->fast_buf || (s->fast_output_s.qs.off < 0 && s->gguf.output.type != INK_GGML_Q8_0)) return;
    if (hp->n_embd != 2304 || hp->loop_count != 3 || hp->loop_start != 6 ||
        hp->loop_end != 18 || hp->n_phys != 24 || hp->n_vocab > 201024) return;
    if (s->budget.device_ram_mb > 0 && s->budget.headroom_mb < 2048) return;

    char path_buf[1024];
    const char *mtp_path = getenv("INK_MTP_PATH");
    if (!mtp_path || !*mtp_path) {
        const char *slash = gguf_path ? strrchr(gguf_path, '/') : NULL;
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

    int fd = open(mtp_path, O_RDONLY);
    if (fd < 0) return;
    struct stat st;
    if (fstat(fd, &st) != 0 || st.st_size < 64) { close(fd); return; }
    size_t fsize = (size_t)st.st_size;
    size_t page = (size_t)getpagesize();
    size_t msize = (fsize + page - 1) & ~(page - 1);
    void *map = mmap(NULL, msize, PROT_READ, MAP_PRIVATE, fd, 0);
    if (map == MAP_FAILED) { close(fd); return; }
    d->fd = fd;
    d->map = map;
    d->map_size = msize;

    uint64_t hdr_len = 0;
    memcpy(&hdr_len, map, 8);
    if (hdr_len == 0 || hdr_len > (1ULL << 20) || 8 + hdr_len >= fsize) {
        ink_dspark_close(s); return;
    }
    int64_t data_start = (int64_t)(8 + hdr_len);
    char *hdr = (char *)malloc((size_t)hdr_len + 1);
    if (!hdr) { ink_dspark_close(s); return; }
    memcpy(hdr, (const uint8_t *)map + 8, (size_t)hdr_len);
    hdr[hdr_len] = '\0';

    int err = 0;
    err |= mtp_find_span(hdr, data_start, fsize, "fuse_weight", 4 * 4 * 2, &d->fuse_weight);
    err |= mtp_find_span(hdr, data_start, fsize, "mask_embed", 2304 * 2, &d->mask_embed);
    err |= mtp_find_span(hdr, data_start, fsize, "norm.weight", 2304 * 2, &d->norm_w);
    err |= mtp_find_span(hdr, data_start, fsize, "markov_head.dflare_norm.weight", 2304 * 2, &d->mh_dflare_norm);
    err |= mtp_find_span(hdr, data_start, fsize, "markov_head.hidden_out.weight", 2304 * 256 * 2, &d->mh_hidden_w);
    err |= mtp_find_span(hdr, data_start, fsize, "markov_head.hidden_out.bias", 2304 * 2, &d->mh_hidden_b);
    err |= mtp_find_span(hdr, data_start, fsize, "markov_head.markov_w1.weight", 201024LL * 256 * 2, &d->mh_markov_w1);
    err |= mtp_find_span(hdr, data_start, fsize, "markov_head.rnn_proj.weight", 768LL * 5120 * 2, &d->mh_rnn_w);
    err |= mtp_find_span(hdr, data_start, fsize, "markov_head.rnn_proj.bias", 768 * 2, &d->mh_rnn_bias);
    for (int i = 0; i < 3; i++) {
        DsparkLayer *L = &d->layers[i];
        L->is_swa = (i == 0 || i == 2) ? 1 : 0;
        char nbuf[96];
#define FIND_L(FLD, SUF, BYTES) do { \
    snprintf(nbuf, sizeof nbuf, "layers.%d." SUF, i); \
    err |= mtp_find_span(hdr, data_start, fsize, nbuf, (BYTES), &L->FLD); \
} while (0)
        FIND_L(dflare_norm, "dflare_norm.weight", 2304 * 2);
        FIND_L(input_ln, "input_layernorm.weight", 2304 * 2);
        FIND_L(post_attn_ln, "post_attention_layernorm.weight", 2304 * 2);
        FIND_L(q_norm, "self_attn.q_norm.weight", 128 * 2);
        FIND_L(k_norm, "self_attn.k_norm.weight", 128 * 2);
        FIND_L(k_norm_ctx, "self_attn.k_norm_ctx.weight", 128 * 2);
        FIND_L(q_proj, "self_attn.q_proj.weight", 3072LL * 2304 * 2);
        FIND_L(k_proj, "self_attn.k_proj.weight", 1024LL * 2304 * 2);
        FIND_L(v_proj, "self_attn.v_proj.weight", 1024LL * 2304 * 2);
        FIND_L(k_proj_ctx, "self_attn.k_proj_ctx.weight", 1024LL * 2304 * 2);
        FIND_L(v_proj_ctx, "self_attn.v_proj_ctx.weight", 1024LL * 2304 * 2);
        FIND_L(o_proj, "self_attn.o_proj.weight", 2304LL * 3072 * 2);
        FIND_L(gate_proj, "mlp.gate_proj.weight", 9216LL * 2304 * 2);
        FIND_L(up_proj, "mlp.up_proj.weight", 9216LL * 2304 * 2);
        FIND_L(down_proj, "mlp.down_proj.weight", 2304LL * 9216 * 2);
#undef FIND_L
    }
    free(hdr);
    if (err != 0 || pm_wrap(map, msize, &d->mtp_buf) != PM_OK) {
        ink_dspark_close(s); return;
    }

    /* Compute softmax(fuse_weight, axis=-1) -> fuse_alpha[4][4] */
    const uint16_t *fw = (const uint16_t *)((const uint8_t *)map + d->fuse_weight.off);
    for (int r = 0; r < 4; r++) {
        float v[4], mx = -1e30f, sum = 0.0f;
        for (int c = 0; c < 4; c++) {
            v[c] = bf16_to_f32_host(fw[r * 4 + c]);
            if (v[c] > mx) mx = v[c];
        }
        for (int c = 0; c < 4; c++) { v[c] = expf(v[c] - mx); sum += v[c]; }
        for (int c = 0; c < 4; c++) d->fuse_alpha[r][c] = v[c] / sum;
    }

    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
    int64_t pb = s->prefill_b;
    int64_t d_dim = 2304;
    for (int i = 0; i < 3; i++) {
        int swa_cap = 1024 + (int)pb;
        int cap = d->layers[i].is_swa ? (swa_cap < s->max_ctx ? swa_cap : s->max_ctx) : s->max_ctx;
        d->layers[i].kv_cap = cap;
        if (alloc_buf_unset(s, &d->layers[i].kv_k, (size_t)cap * 1024 * 2) ||
            alloc_buf_unset(s, &d->layers[i].kv_v, (size_t)cap * 1024 * 2)) {
            ink_dspark_close(s); return;
        }
    }
    if (alloc_buf_unset(s, &d->inv_freq, 64 * 4) ||
        alloc_buf_unset(s, &d->rnn_w_step, 768 * 512 * 2) ||
        alloc_buf_unset(s, &d->rnn_w_ctx, 768 * 4608 * 2) ||
        alloc_buf_unset(s, &d->fold_w_qs, (size_t)nv * 8 * 16) ||
        alloc_buf_unset(s, &d->fold_w_ds, (size_t)nv * 8 * 2) ||
        alloc_buf_unset(s, &d->fold_b, (size_t)nv * 4) ||
        alloc_buf_unset(s, &d->taps, (size_t)4 * (size_t)pb * (size_t)d_dim * 4) ||
        alloc_buf(s, &d->last_taps, (size_t)4 * (size_t)d_dim * 4) ||
        alloc_buf_unset(s, &d->ctx_bf16, (size_t)pb * (size_t)d_dim * 2) ||
        alloc_buf_unset(s, &d->ctx_k_f32, (size_t)pb * 1024 * 4) ||
        alloc_buf_unset(s, &d->ctx_v_f32, (size_t)pb * 1024 * 4) ||
        alloc_buf_unset(s, &d->anchor_emb, (size_t)d_dim * 4) ||
        alloc_buf_unset(s, &d->f_rnn_bf16, (size_t)d_dim * 2) ||
        alloc_buf_unset(s, &d->res, 8 * (size_t)d_dim * 4) ||
        alloc_buf_unset(s, &d->x_bf16, 8 * (size_t)d_dim * 2) ||
        alloc_buf_unset(s, &d->x_f32, 8 * (size_t)d_dim * 4) ||
        alloc_buf_unset(s, &d->q_f32, 8 * 3072 * 4) ||
        alloc_buf_unset(s, &d->k_f32, 8 * 1024 * 4) ||
        alloc_buf_unset(s, &d->v_f32, 8 * 1024 * 4) ||
        alloc_buf_unset(s, &d->qkv_f32, 8 * 5120 * 4) ||
        alloc_buf_unset(s, &d->attn_bf16, 8 * 3072 * 2) ||
        alloc_buf_unset(s, &d->attn_f32, 8 * 3072 * 4) ||
        alloc_buf_unset(s, &d->tmp_f32, 8 * (size_t)d_dim * 4) ||
        alloc_buf_unset(s, &d->gate_f32, 8 * 9216 * 4) ||
        alloc_buf_unset(s, &d->up_f32, 8 * 9216 * 4) ||
        alloc_buf_unset(s, &d->gu_f32, 8 * 18432 * 4) ||
        alloc_buf_unset(s, &d->mlp_bf16, 8 * 9216 * 2) ||
        alloc_buf_unset(s, &d->mlp_f32, 8 * 9216 * 4) ||
        alloc_buf_unset(s, &d->markov_in_bf16, 7 * 4608 * 2) ||
        alloc_buf_unset(s, &d->rnn_ctx, 7 * 768 * 4) ||
        alloc_buf(s, &d->rnn_state, 256 * 4) ||
        alloc_buf(s, &d->rnn_state2, 256 * 4) ||
        alloc_buf_unset(s, &d->rnn_latent, 256 * 4) ||
        alloc_buf_unset(s, &d->base_logits, (size_t)7 * (size_t)nv * 4) ||
        alloc_buf_unset(s, &d->delta_logits, (size_t)nv * 4) ||
        alloc_buf_unset(s, &d->s1_m, 64 * 4) ||
        alloc_buf_unset(s, &d->s1_i, 64 * 4) ||
        alloc_buf_unset(s, &d->s1_s, 64 * 4) ||
        alloc_buf(s, &d->draft_toks, 8 * 4) ||
        alloc_buf(s, &d->draft_probs, 8 * 4)) {
        ink_dspark_close(s); return;
    }

    /* Populate NeoX RoPE inv_freq (theta = 1e6, head_dim = 128) */
    float *ifreq = (float *)pm_contents(d->inv_freq);
    for (int i = 0; i < 64; i++) {
        ifreq[i] = powf(1000000.0f, -(float)(2 * i) / 128.0f);
    }

    /* Split rnn_proj.weight [768, 5120] -> rnn_w_step [768, 512] + rnn_w_ctx [768, 4608] */
    const uint16_t *rw = (const uint16_t *)((const uint8_t *)map + d->mh_rnn_w.off);
    uint16_t *rw_step = (uint16_t *)pm_contents(d->rnn_w_step);
    uint16_t *rw_ctx = (uint16_t *)pm_contents(d->rnn_w_ctx);
    for (int r = 0; r < 768; r++) {
        memcpy(rw_step + (size_t)r * 512, rw + (size_t)r * 5120, 512 * 2);
        memcpy(rw_ctx + (size_t)r * 4608, rw + (size_t)r * 5120 + 512, 4608 * 2);
    }

    /* Load-time GPU fold: fold_b = lm_head(hidden_out.bias), fold_w = Q4S(lm_head @ hidden_out.weight) */
    const uint16_t *hb = (const uint16_t *)((const uint8_t *)map + d->mh_hidden_b.off);
    float *hb_f32 = (float *)pm_contents(d->anchor_emb);
    for (int i = 0; i < 2304; i++) hb_f32[i] = bf16_to_f32_host(hb[i]);

    const uint16_t *hw = (const uint16_t *)((const uint8_t *)map + d->mh_hidden_w.off);
    uint16_t *hw_t = (uint16_t *)pm_contents(s->w_bf16);
    for (int r = 0; r < 2304; r++) {
        const uint16_t *src_r = hw + (size_t)r * 256;
        for (int c = 0; c < 256; c++) hw_t[(size_t)c * 2304 + r] = src_r[c];
    }

    {
        PmBatch b = NULL;
        if (pm_begin_concurrent(&b) != PM_OK) { ink_dspark_close(s); return; }
        if (s->fast_output_s.qs.off >= 0 && s->fast_output_is_q4s) {
            PmArg args[] = {
                a_int(nv), a_int(d_dim), a_int(1), a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
                a_f32v(d->anchor_emb, 0, d_dim), a_f32v(d->fold_b, 0, nv)
            };
            if (call_k(s, b, K_DV_Q4S_MMA32_NB, (nv + 31) / 32, 11, args) != 0 || pm_wait(b, NULL) != PM_OK) {
                ink_dspark_close(s); return;
            }
        } else if (s->fast_output_s.qs.off >= 0) {
            PmArg args[] = {
                a_int(nv), a_int(d_dim), a_int(1), a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
                a_f32v(d->anchor_emb, 0, d_dim), a_f32v(d->fold_b, 0, nv)
            };
            if (call_k(s, b, K_DV_Q8S_NB, (nv + 3) / 4, 11, args) != 0 || pm_wait(b, NULL) != PM_OK) {
                ink_dspark_close(s); return;
            }
        } else {
            PmArg args[] = {
                a_int(nv), a_int(d_dim), a_int(1), a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                a_tensor_u16(s, &s->gguf.output),
                a_f32v(d->anchor_emb, 0, d_dim), a_f32v(d->fold_b, 0, nv)
            };
            if (call_k(s, b, K_DV_Q8_MMA32_NB, (nv + 31) / 32, 10, args) != 0 || pm_wait(b, NULL) != PM_OK) {
                ink_dspark_close(s); return;
            }
        }
    }

    /* Sprint 73: Repack 3x DSpark DraftLayer BF16 weights (495 MB) -> fused Q4S (139 MB, 3.56x less DRAM traffic!) */
    d->use_q4s_draft = (getenv("INK_DSPARK_BF16") == NULL) ? 1 : 0;
    if (d->use_q4s_draft) {
        size_t qoff = 0;
        for (int i = 0; i < 3; i++) {
            DsparkLayer *L = &d->layers[i];
            int64_t specs[4][2] = {{5120, 2304}, {2304, 3072}, {18432, 2304}, {2304, 9216}};
            FastQ4S *dsts[4] = {&L->qkv_s, &L->o_s, &L->gu_s, &L->dn_s};
            for (int k = 0; k < 4; k++) {
                size_t nblk = (size_t)specs[k][0] * (size_t)(specs[k][1] / 32);
                qoff = (qoff + 63) & ~(size_t)63;
                dsts[k]->qs.off = (int64_t)qoff; dsts[k]->qs.words = (int64_t)(nblk * 8);
                qoff += nblk * 16;
                qoff = (qoff + 63) & ~(size_t)63;
                dsts[k]->ds.off = (int64_t)qoff; dsts[k]->ds.words = (int64_t)nblk;
                qoff += nblk * 2;
            }
        }
        qoff = (qoff + 63) & ~(size_t)63;
        if (alloc_buf(s, &d->draft_q4s_buf, qoff) != 0) {
            ink_dspark_close(s); return;
        }
        uint8_t *qb = (uint8_t *)pm_contents(d->draft_q4s_buf);
        const uint8_t *mb = (const uint8_t *)map;
        for (int i = 0; i < 3; i++) {
            DsparkLayer *L = &d->layers[i];
            /* qkv_s: [q 3072 | k 1024 | v 1024] x 2304 (72 blocks/row) */
            convert_bf16_to_q4s(mb + L->q_proj.off, (size_t)L->q_proj.bytes,
                                qb + L->qkv_s.qs.off, qb + L->qkv_s.ds.off);
            convert_bf16_to_q4s(mb + L->k_proj.off, (size_t)L->k_proj.bytes,
                                qb + L->qkv_s.qs.off + 3072ULL * 72 * 16, qb + L->qkv_s.ds.off + 3072ULL * 72 * 2);
            convert_bf16_to_q4s(mb + L->v_proj.off, (size_t)L->v_proj.bytes,
                                qb + L->qkv_s.qs.off + 4096ULL * 72 * 16, qb + L->qkv_s.ds.off + 4096ULL * 72 * 2);
            /* o_s: [2304, 3072] */
            convert_bf16_to_q4s(mb + L->o_proj.off, (size_t)L->o_proj.bytes,
                                qb + L->o_s.qs.off, qb + L->o_s.ds.off);
            /* gu_s: [gate 9216 | up 9216] x 2304 (72 blocks/row) */
            convert_bf16_to_q4s(mb + L->gate_proj.off, (size_t)L->gate_proj.bytes,
                                qb + L->gu_s.qs.off, qb + L->gu_s.ds.off);
            convert_bf16_to_q4s(mb + L->up_proj.off, (size_t)L->up_proj.bytes,
                                qb + L->gu_s.qs.off + 9216ULL * 72 * 16, qb + L->gu_s.ds.off + 9216ULL * 72 * 2);
            /* dn_s: [2304, 9216] */
            convert_bf16_to_q4s(mb + L->down_proj.off, (size_t)L->down_proj.bytes,
                                qb + L->dn_s.qs.off, qb + L->dn_s.ds.off);
        }
        pm_mark_readonly(d->draft_q4s_buf);
    }

    uint8_t *dst_qs = (uint8_t *)pm_contents(d->fold_w_qs);
    uint8_t *dst_ds = (uint8_t *)pm_contents(d->fold_w_ds);
    const int64_t w_off = 327680; /* u32 offset in s->w_bf16 (> 256 * 1152 = 294912) */
    const int64_t blk_per_row = d_dim / 32; /* 72 */
    for (int64_t row0 = 0; row0 < nv; row0 += 4096) {
        int64_t vc = (nv - row0 < 4096) ? (nv - row0) : 4096;
        PmBatch b = NULL;
        if (pm_begin_concurrent(&b) != PM_OK) { ink_dspark_close(s); return; }
        if (s->fast_output_s.qs.off >= 0 && !s->fast_output_is_q4s) {
            PmArg dq_args[] = {
                a_int(vc), a_int(d_dim),
                a_buf(s->fast_buf, s->fast_output_s.qs.off + row0 * blk_per_row * 32, vc * blk_per_row * 8),
                a_buf(s->fast_buf, s->fast_output_s.ds.off + row0 * blk_per_row * 2, vc * blk_per_row),
                a_u32v(s->w_bf16, w_off, vc * (d_dim / 2))
            };
            if (call_k(s, b, K_DEQUANT_Q8S_BF16, (vc + 7) / 8, 5, dq_args) != 0) {
                pm_abort(b); ink_dspark_close(s); return;
            }
        } else if (s->fast_output_s.qs.off >= 0) {
            PmArg dq_args[] = {
                a_int(vc), a_int(d_dim),
                a_buf(s->fast_buf, s->fast_output_s.qs.off + row0 * blk_per_row * 16, vc * blk_per_row * 4),
                a_buf(s->fast_buf, s->fast_output_s.ds.off + row0 * blk_per_row * 2, vc * blk_per_row),
                a_u32v(s->w_bf16, w_off, vc * (d_dim / 2))
            };
            if (call_k(s, b, K_DEQUANT_Q4S_BF16, (vc + 7) / 8, 5, dq_args) != 0) {
                pm_abort(b); ink_dspark_close(s); return;
            }
        } else {
            PmArg dq_args[] = {
                a_int(vc), a_int(d_dim),
                a_buf(s->weights_buf, (int64_t)s->gguf.output.offset + row0 * blk_per_row * 34, vc * blk_per_row * 17),
                a_u32v(s->w_bf16, w_off, vc * (d_dim / 2))
            };
            if (call_k(s, b, K_DEQUANT_Q8_BF16, (vc + 3) / 4, 4, dq_args) != 0) {
                pm_abort(b); ink_dspark_close(s); return;
            }
        }
        pm_barrier(b);
        int64_t mm_items = ((vc + 63) / 64) * ((256 + 127) / 128);
        PmArg mm_args[] = {
            a_int(256), a_int(d_dim), a_int(vc),
            a_u32v(s->w_bf16, 0, 256 * (d_dim / 2)),
            a_u16v(s->w_bf16, w_off * 2, vc * d_dim),
            a_f32v(s->spec_logits, 0, vc * 256)
        };
        if (call_k(s, b, K_MM_BF16, mm_items, 6, mm_args) != 0 || pm_wait(b, NULL) != PM_OK) {
            ink_dspark_close(s); return;
        }
        convert_f32_to_q4s((const uint8_t *)pm_contents(s->spec_logits), (size_t)vc * 256 * 4,
                           dst_qs + (size_t)row0 * 8 * 16, dst_ds + (size_t)row0 * 8 * 2);
    }

    pm_mark_readonly(d->inv_freq);
    pm_mark_readonly(d->rnn_w_step);
    pm_mark_readonly(d->rnn_w_ctx);
    pm_mark_readonly(d->fold_w_qs);
    pm_mark_readonly(d->fold_w_ds);
    pm_mark_readonly(d->fold_b);

    d->pos = 0;
    d->pending_pos0 = 0;
    d->pending_nb = 0;
    d->pending_got = 0;
    d->skip_countdown = 0;
    d->consec_unprof = 0;
    d->ema_acc = 3.0f;
    d->ema_draft_ms = 4.2f;
    d->ema_cal = 0.90f;
    d->ema_net_speedup = 1.15f;
    d->last_draft_ms = 4.2f;
    d->ready = 1;
}

static int ink_dspark_enqueue_absorb(InkSession *s, PmBatch batch, int stride_nb, int n_tok, int pos0) {
    DsparkState *d = &s->dspark;
    if (!d->ready || n_tok <= 0 || n_tok > stride_nb) return 0;
    int64_t d_dim = 2304;
    int64_t cp_items = (d_dim + INK_NT - 1) / INK_NT;
    /* Copy last absorbed token's 4 taps into d->last_taps for MarkovHead f_rnn */
    for (int k = 0; k < 4; k++) {
        int64_t src_off = ((int64_t)k * stride_nb + (n_tok - 1)) * d_dim;
        CALL(s, batch, K_COPY, cp_items, a_int(d_dim),
             a_f32v(d->taps, src_off, d_dim), a_f32v(d->last_taps, (int64_t)k * d_dim, d_dim));
    }
    for (int li = 0; li < 3; li++) {
        DsparkLayer *L = &d->layers[li];
        CALL(s, batch, K_DSPARK_MIX_NORM, n_tok,
             a_int(stride_nb), a_int(d_dim),
             a_f32(d->fuse_alpha[li][0]), a_f32(d->fuse_alpha[li][1]),
             a_f32(d->fuse_alpha[li][2]), a_f32(d->fuse_alpha[li][3]),
             a_f32(1e-6f),
             a_f32v(d->taps, 0, (int64_t)4 * stride_nb * d_dim),
             a_mtp_u16(d, &L->dflare_norm),
             a_u16v(d->ctx_bf16, 0, (int64_t)n_tok * d_dim));
        pm_barrier(batch);
        if (dspark_mm_bf16(s, batch, 1024, d_dim, n_tok, d->mtp_buf, L->k_proj_ctx.off,
                           d->ctx_bf16, 0, d->ctx_k_f32, 0) != 0) return 1;
        if (dspark_mm_bf16(s, batch, 1024, d_dim, n_tok, d->mtp_buf, L->v_proj_ctx.off,
                           d->ctx_bf16, 0, d->ctx_v_f32, 0) != 0) return 1;
        pm_barrier(batch);
        CALL(s, batch, K_DSPARK_KV_ABSORB, (int64_t)n_tok * 8,
             a_int(pos0), a_int(L->kv_cap), a_int(L->is_swa), a_f32(1e-6f),
             a_f32v(d->ctx_k_f32, 0, (int64_t)n_tok * 1024),
             a_f32v(d->ctx_v_f32, 0, (int64_t)n_tok * 1024),
             a_mtp_u16(d, &L->k_norm_ctx),
             a_f32v(d->inv_freq, 0, 64),
             a_u16v(L->kv_k, 0, (int64_t)L->kv_cap * 1024),
             a_u16v(L->kv_v, 0, (int64_t)L->kv_cap * 1024));
        pm_barrier(batch);
    }
    d->pos = pos0 + n_tok;
    s->dspark_live_len = d->pos;
    int ds_cap = d->layers[0].kv_cap;
    if (ds_cap > 0 && d->pos - ds_cap > s->dspark_swa_valid_from) {
        s->dspark_swa_valid_from = d->pos - ds_cap;
    }
    return 0;
}

static int ink_dspark_flush_pending(InkSession *s, PmBatch batch) {
    DsparkState *d = &s->dspark;
    if (!d->ready || d->pending_got <= 0) return 0;
    int n_tok = d->pending_got;
    int stride_nb = d->pending_nb;
    int pos0 = d->pending_pos0;
    d->pending_got = 0;
    d->pending_nb = 0;
    if (pos0 != d->pos) return 0;
    return ink_dspark_enqueue_absorb(s, batch, stride_nb, n_tok, pos0);
}

static int ink_dspark_draft(InkSession *s, int32_t anchor, int count, int32_t *out_toks, float *out_probs) {
    DsparkState *d = &s->dspark;
    const InkHParams *hp = &s->gguf.hp;
    if (!d->ready || count < 1 || count > 7) return 1;
    if (d->pos + d->pending_got != s->pos || s->pos < 1) return 1;
    int64_t d_dim = 2304;
    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;

    ((uint32_t *)pm_contents(d->draft_toks))[0] = (uint32_t)anchor;

    PmBatch batch = NULL;
    if (pm_begin_concurrent(&batch) != PM_OK) return 1;

    /* 0. Flush any pending accepted taps from the previous step into DSpark's 3 KV caches (same CB!) */
    if (d->pending_got > 0) {
        if (ink_dspark_flush_pending(s, batch) != 0) { pm_abort(batch); return 1; }
    }
    if (d->pos != s->pos) { pm_abort(batch); return 1; }

    /* 1. Anchor embedding + MarkovHead f_rnn mix/norm (concurrent) */
    if (s->gguf.tok_embd.type == INK_GGML_Q8_0 && !s->ablate_has_emb) {
        CALL(s, batch, K_EMBED_Q8, 1, a_int(anchor), a_int(d_dim),
             a_tensor_u16(s, &s->gguf.tok_embd),
             a_tensor_f32(s, &s->gguf.tok_norm),
             a_f32(hp->rms_eps),
             a_f32v(d->anchor_emb, 0, d_dim));
    } else {
        CALL(s, batch, K_EMBED_GGML, 1, a_int(anchor), a_int(d_dim), a_int((int64_t)s->gguf.tok_embd.type),
             a_tensor_ggml_u16(s, &s->gguf.tok_embd), a_ggml_grids(s),
             a_tensor_f32(s, &s->gguf.tok_norm), a_f32(hp->rms_eps),
             a_int(s->ablate_has_emb ? 1 : 0), a_ablate_dirs(s),
             a_f32v(d->anchor_emb, 0, d_dim));
    }
    CALL(s, batch, K_DSPARK_MIX_NORM, 1,
         a_int(1), a_int(d_dim),
         a_f32(d->fuse_alpha[3][0]), a_f32(d->fuse_alpha[3][1]),
         a_f32(d->fuse_alpha[3][2]), a_f32(d->fuse_alpha[3][3]),
         a_f32(1e-6f),
         a_f32v(d->last_taps, 0, 4 * d_dim),
         a_mtp_u16(d, &d->mh_dflare_norm),
         a_u16v(d->f_rnn_bf16, 0, d_dim));
    pm_barrier(batch);

    /* 2. Prepare 8-token block [anchor_emb, mask_embed x 7] + layer-0 input_layernorm */
    CALL(s, batch, K_DSPARK_BLOCK_PREP, 8, a_int(d_dim), a_f32(1e-6f),
         a_f32v(d->anchor_emb, 0, d_dim),
         a_mtp_u16(d, &d->mask_embed),
         a_mtp_u16(d, &d->layers[0].input_ln),
         a_f32v(d->res, 0, 8 * d_dim),
         a_u16v(d->x_bf16, 0, 8 * d_dim),
         a_f32v(d->x_f32, 0, 8 * d_dim));
    pm_barrier(batch);

    /* 3. Three DraftLayers: fused Q4S (139 MB, 3.56x less DRAM traffic!) or BF16 fallback */
    for (int li = 0; li < 3; li++) {
        DsparkLayer *L = &d->layers[li];
        const MtpSpan *next_ln = (li < 2) ? &d->layers[li + 1].input_ln : &d->norm_w;
        if (d->use_q4s_draft) {
            CALL(s, batch, K_DV_Q4S_MMA32_NB, (5120 + 31) / 32, a_int(5120), a_int(d_dim), a_int(8),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_buf(d->draft_q4s_buf, L->qkv_s.qs.off, L->qkv_s.qs.words / 2),
                 a_buf(d->draft_q4s_buf, L->qkv_s.ds.off, L->qkv_s.ds.words),
                 a_f32v(d->x_f32, 0, 8 * d_dim), a_f32v(d->qkv_f32, 0, 8 * 5120));
            pm_barrier(batch);
            CALL(s, batch, K_DSPARK_QKV_ROPE_ATTN_F32, 24,
                 a_int(d->pos), a_int(L->kv_cap), a_int(L->is_swa), a_int(1024), a_f32(1e-6f),
                 a_f32v(d->qkv_f32, 0, 8 * 5120),
                 a_mtp_u16(d, &L->q_norm), a_mtp_u16(d, &L->k_norm),
                 a_f32v(d->inv_freq, 0, 64),
                 a_u16v(L->kv_k, 0, (int64_t)L->kv_cap * 1024),
                 a_u16v(L->kv_v, 0, (int64_t)L->kv_cap * 1024),
                 a_f32v(d->attn_f32, 0, 8 * 3072));
            pm_barrier(batch);
            CALL(s, batch, K_DV_Q4S_MMA32_NB, (d_dim + 31) / 32, a_int(d_dim), a_int(3072), a_int(8),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_buf(d->draft_q4s_buf, L->o_s.qs.off, L->o_s.qs.words / 2),
                 a_buf(d->draft_q4s_buf, L->o_s.ds.off, L->o_s.ds.words),
                 a_f32v(d->attn_f32, 0, 8 * 3072), a_f32v(d->tmp_f32, 0, 8 * d_dim));
            pm_barrier(batch);
            CALL(s, batch, K_DSPARK_ADD_NORM_BF16, 8, a_int(d_dim), a_f32(1e-6f), a_int(1),
                 a_f32v(d->tmp_f32, 0, 8 * d_dim), a_f32v(d->res, 0, 8 * d_dim),
                 a_mtp_u16(d, &L->post_attn_ln),
                 a_u16v(d->x_bf16, 0, 8 * d_dim), a_f32v(d->x_f32, 0, 8 * d_dim));
            pm_barrier(batch);
            CALL(s, batch, K_DV_Q4S_MMA32_NB, (18432 + 31) / 32, a_int(18432), a_int(d_dim), a_int(8),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_buf(d->draft_q4s_buf, L->gu_s.qs.off, L->gu_s.qs.words / 2),
                 a_buf(d->draft_q4s_buf, L->gu_s.ds.off, L->gu_s.ds.words),
                 a_f32v(d->x_f32, 0, 8 * d_dim), a_f32v(d->gu_f32, 0, 8 * 18432));
            pm_barrier(batch);
            CALL(s, batch, K_DSPARK_SILU_MUL_F32, (8 * 9216 + 255) / 256,
                 a_f32v(d->gu_f32, 0, 8 * 18432), a_f32v(d->mlp_f32, 0, 8 * 9216));
            pm_barrier(batch);
            CALL(s, batch, K_DV_Q4S_MMA32_NB, (d_dim + 31) / 32, a_int(d_dim), a_int(9216), a_int(8),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_buf(d->draft_q4s_buf, L->dn_s.qs.off, L->dn_s.qs.words / 2),
                 a_buf(d->draft_q4s_buf, L->dn_s.ds.off, L->dn_s.ds.words),
                 a_f32v(d->mlp_f32, 0, 8 * 9216), a_f32v(d->tmp_f32, 0, 8 * d_dim));
            pm_barrier(batch);
            CALL(s, batch, K_DSPARK_ADD_NORM_BF16, 8, a_int(d_dim), a_f32(1e-6f), a_int(1),
                 a_f32v(d->tmp_f32, 0, 8 * d_dim), a_f32v(d->res, 0, 8 * d_dim),
                 a_mtp_u16(d, next_ln),
                 a_u16v(d->x_bf16, 0, 8 * d_dim), a_f32v(d->x_f32, 0, 8 * d_dim));
            pm_barrier(batch);
        } else {
            if (dspark_mm_bf16(s, batch, 3072, d_dim, 8, d->mtp_buf, L->q_proj.off, d->x_bf16, 0, d->q_f32, 0) != 0 ||
                dspark_mm_bf16(s, batch, 1024, d_dim, 8, d->mtp_buf, L->k_proj.off, d->x_bf16, 0, d->k_f32, 0) != 0 ||
                dspark_mm_bf16(s, batch, 1024, d_dim, 8, d->mtp_buf, L->v_proj.off, d->x_bf16, 0, d->v_f32, 0) != 0) {
                pm_abort(batch); return 1;
            }
            pm_barrier(batch);
            CALL(s, batch, K_DSPARK_QKV_ROPE_ATTN, 24,
                 a_int(d->pos), a_int(L->kv_cap), a_int(L->is_swa), a_int(1024), a_f32(1e-6f),
                 a_f32v(d->q_f32, 0, 8 * 3072), a_f32v(d->k_f32, 0, 8 * 1024), a_f32v(d->v_f32, 0, 8 * 1024),
                 a_mtp_u16(d, &L->q_norm), a_mtp_u16(d, &L->k_norm),
                 a_f32v(d->inv_freq, 0, 64),
                 a_u16v(L->kv_k, 0, (int64_t)L->kv_cap * 1024),
                 a_u16v(L->kv_v, 0, (int64_t)L->kv_cap * 1024),
                 a_u16v(d->attn_bf16, 0, 8 * 3072));
            pm_barrier(batch);
            if (dspark_mm_bf16(s, batch, d_dim, 3072, 8, d->mtp_buf, L->o_proj.off, d->attn_bf16, 0, d->tmp_f32, 0) != 0) {
                pm_abort(batch); return 1;
            }
            pm_barrier(batch);
            CALL(s, batch, K_DSPARK_ADD_NORM_BF16, 8, a_int(d_dim), a_f32(1e-6f), a_int(0),
                 a_f32v(d->tmp_f32, 0, 8 * d_dim), a_f32v(d->res, 0, 8 * d_dim),
                 a_mtp_u16(d, &L->post_attn_ln),
                 a_u16v(d->x_bf16, 0, 8 * d_dim), a_f32v(d->x_f32, 0, 8 * d_dim));
            pm_barrier(batch);
            if (dspark_mm_bf16(s, batch, 9216, d_dim, 8, d->mtp_buf, L->gate_proj.off, d->x_bf16, 0, d->gate_f32, 0) != 0 ||
                dspark_mm_bf16(s, batch, 9216, d_dim, 8, d->mtp_buf, L->up_proj.off, d->x_bf16, 0, d->up_f32, 0) != 0) {
                pm_abort(batch); return 1;
            }
            pm_barrier(batch);
            CALL(s, batch, K_DSPARK_SILU_MUL_BF16, (8 * 9216 + 255) / 256, a_int(8 * 9216),
                 a_f32v(d->gate_f32, 0, 8 * 9216), a_f32v(d->up_f32, 0, 8 * 9216),
                 a_u16v(d->mlp_bf16, 0, 8 * 9216));
            pm_barrier(batch);
            if (dspark_mm_bf16(s, batch, d_dim, 9216, 8, d->mtp_buf, L->down_proj.off, d->mlp_bf16, 0, d->tmp_f32, 0) != 0) {
                pm_abort(batch); return 1;
            }
            pm_barrier(batch);
            CALL(s, batch, K_DSPARK_ADD_NORM_BF16, 8, a_int(d_dim), a_f32(1e-6f), a_int(li == 2 ? 1 : 0),
                 a_f32v(d->tmp_f32, 0, 8 * d_dim), a_f32v(d->res, 0, 8 * d_dim),
                 a_mtp_u16(d, next_ln),
                 a_u16v(d->x_bf16, 0, 8 * d_dim), a_f32v(d->x_f32, 0, 8 * d_dim));
            pm_barrier(batch);
        }
    }

    /* 4. Base lm_head logits for block positions 1..count + MarkovHead context prep (concurrent) */
    CALL(s, batch, K_DSPARK_MARKOV_CTX_PREP, count, a_int(d_dim),
         a_u16v(d->x_bf16, 0, 8 * d_dim), a_u16v(d->f_rnn_bf16, 0, d_dim),
         a_u16v(d->markov_in_bf16, 0, (int64_t)count * 4608));
    if (s->fast_output_s.qs.off >= 0 && s->fast_output_is_q4s) {
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (nv + 31) / 32, a_int(nv), a_int(d_dim), a_int(count),
             a_int(0), a_int(d_dim), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
             a_f32v(d->x_f32, 0, 8 * d_dim), a_f32v(d->base_logits, 0, (int64_t)count * nv));
    } else if (s->fast_output_s.qs.off >= 0) {
        CALL(s, batch, K_DV_Q8S_NB, (nv + 3) / 4, a_int(nv), a_int(d_dim), a_int(count),
             a_int(0), a_int(d_dim), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
             a_f32v(d->x_f32, 0, 8 * d_dim), a_f32v(d->base_logits, 0, (int64_t)count * nv));
    } else {
        CALL(s, batch, K_DV_Q8_MMA32_NB, (nv + 31) / 32, a_int(nv), a_int(d_dim), a_int(count),
             a_int(0), a_int(d_dim), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &s->gguf.output),
             a_f32v(d->x_f32, 0, 8 * d_dim), a_f32v(d->base_logits, 0, (int64_t)count * nv));
    }
    pm_barrier(batch);
    if (dspark_mm_bf16(s, batch, 768, 4608, count, d->rnn_w_ctx, 0, d->markov_in_bf16, 0, d->rnn_ctx, 0) != 0) {
        pm_abort(batch); return 1;
    }
    pm_barrier(batch);

    /* 5. GPU-chained autoregressive MarkovHead: 32-TG markov_step + fused 0%-idle fold_logits_s1 (0 CPU syncs!) */
    for (int step = 0; step < count; step++) {
        PmBuffer st_in = (step & 1) ? d->rnn_state2 : d->rnn_state;
        PmBuffer st_out = (step & 1) ? d->rnn_state : d->rnn_state2;
        CALL(s, batch, K_DSPARK_MARKOV_STEP, 32, a_int(step),
             a_u32v(d->draft_toks, 0, 8),
             a_mtp_u16(d, &d->mh_markov_w1),
             a_u16v(d->rnn_w_step, 0, 768 * 512),
             a_f32v(d->rnn_ctx, 0, (int64_t)count * 768),
             a_mtp_u16(d, &d->mh_rnn_bias),
             a_f32v(st_in, 0, 256), a_f32v(st_out, 0, 256),
             a_f32v(d->rnn_latent, 0, 256));
        pm_barrier(batch);
        CALL(s, batch, K_DSPARK_FOLD_LOGITS_S1, 64, a_int(nv), a_int(step), a_f32(hp->logit_softcap),
             a_u32v(d->fold_w_qs, 0, nv * 32), a_u16v(d->fold_w_ds, 0, nv * 8),
             a_f32v(d->rnn_latent, 0, 256),
             a_f32v(d->base_logits, 0, (int64_t)count * nv),
             a_f32v(d->fold_b, 0, nv),
             a_f32v(d->s1_m, 0, 64), a_u32v(d->s1_i, 0, 64), a_f32v(d->s1_s, 0, 64));
        pm_barrier(batch);
        CALL(s, batch, K_DSPARK_LOGITS_S2, 1, a_int(step),
             a_f32v(d->s1_m, 0, 64), a_u32v(d->s1_i, 0, 64), a_f32v(d->s1_s, 0, 64),
             a_u32v(d->draft_toks, 0, 8), a_f32v(d->draft_probs, 0, 8));
        pm_barrier(batch);
    }

    double gpu_s = 0.0;
    if (pm_wait(batch, &gpu_s) != PM_OK) return 1;
    s->total_gpu_seconds += gpu_s;
    d->draft_gpu_s += gpu_s;
    d->last_draft_s = gpu_s;
    d->last_draft_ms = (float)(gpu_s * 1000.0);
    if (d->last_draft_ms > 0.1f && d->last_draft_ms < 200.0f) {
        d->ema_draft_ms = 0.75f * d->ema_draft_ms + 0.25f * d->last_draft_ms;
    }

    const uint32_t *dt = (const uint32_t *)pm_contents(d->draft_toks);
    const float *dp = (const float *)pm_contents(d->draft_probs);
    for (int i = 0; i < count; i++) {
        out_toks[i] = (int32_t)dt[i + 1];
        out_probs[i] = dp[i];
    }
    return 0;
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
    return ink_open_lora(gguf_path, metal_path, NULL, max_ctx, 0, out, err, err_bytes);
}

int ink_open_ex(const char *gguf_path, const char *metal_path, int max_ctx,
                int loop_count_override, InkSession **out, char *err, size_t err_bytes) {
    return ink_open_lora(gguf_path, metal_path, NULL, max_ctx, loop_count_override, out, err, err_bytes);
}

int ink_open_lora(const char *gguf_path, const char *metal_path, const char *lora_path, int max_ctx,
                  int loop_count_override, InkSession **out, char *err, size_t err_bytes) {
    *out = NULL;
    InkSession *s = (InkSession *)calloc(1, sizeof *s);
    if (!s) { snprintf(err, err_bytes, "out of memory"); return 1; }
    s->dspark.fd = -1;

    int is_sf_overlay = (lora_path && lora_path[0] && ink_file_is_safetensors(lora_path));
    if (ink_gguf_open_ex(gguf_path, is_sf_overlay, &s->gguf, err, err_bytes) != 0) {
        free(s); return 1;
    }
    InkHParams *hp = &s->gguf.hp;
    float *ablate_host = NULL;
    if (is_sf_overlay) {
        if (ink_gguf_apply_safetensors_attn_overlay(&s->gguf, lora_path, &s->lora.fp, err, err_bytes) != 0) {
            ink_close(s); return 1;
        }
    } else if (lora_path && lora_path[0]) {
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
    }
    if (loop_count_override < 0 || loop_count_override > 16) {
        snprintf(err, err_bytes, "invalid loop_count_override %d (expected 0 or 1..16)", loop_count_override);
        free(ablate_host);
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
        free(ablate_host);
        ink_close(s); return 1;
    }

    PmDeviceInfo info;
    if (pm_open(&info) != PM_OK) {
        snprintf(err, err_bytes, "pm_open: %s", pm_log());
        free(ablate_host);
        ink_close(s); return 1;
    }

    /* Unified-memory residency guardrail (dynamic >= 12 GB floor + exact weights/KV/scratch budget). */
    int64_t ram_mb = ink_host_ram_mb();
    if (ram_mb <= 0 && info.working_set_bytes > 0) {
        ram_mb = (int64_t)(info.working_set_bytes / (1024ULL * 1024ULL));
    }
    int use_flash_attn = (hp->head_dim == 128 && hp->rel_len == 16 && !getenv("INK_OLD_ATTN"));
    if (ram_mb > 0 && ram_mb <= 18432) {
        s->prefill_b = 256;
    } else if (ram_mb > 0 && ram_mb <= 28672) {
        s->prefill_b = 512;
    } else {
        s->prefill_b = (use_flash_attn && (ram_mb == 0 || ram_mb > 40960)) ? INK_PREFILL_B
                     : (s->max_ctx <= 8192) ? INK_PREFILL_B
                     : (s->max_ctx <= 16384) ? 2048
                     : (ram_mb > 0 && ram_mb <= 40960) ? 512 : 1024;
    }

    size_t B = (size_t)s->prefill_b;
    size_t d = (size_t)hp->n_embd;
    size_t n_q = (size_t)hp->n_head * (size_t)hp->head_dim;
    size_t n_kv = (size_t)hp->n_head_kv * (size_t)hp->head_dim;
    /* Sprint C: Block-32 Q8_0 KV cache (68 u16/head = 1.882x smaller KV RAM & DRAM; toggle INK_KV_Q8=0 for FP16). */
    {
        const char *env_kv8 = getenv("INK_KV_Q8");
        int can_q8 = (hp->head_dim == 128 && hp->rel_len == 16 && hp->n_head == hp->n_head_kv * 3 &&
                      s->max_ctx <= 65535 && (!env_kv8 || strcmp(env_kv8, "0") != 0));
        for (int ip = 0; can_q8 && ip < hp->n_phys; ip++) {
            int kd = (!hp->is_swa[ip] && s->gguf.layers[ip].attn_k_conv_proj.present);
            int vd = (!hp->is_swa[ip] && s->gguf.layers[ip].attn_v_conv_proj.present);
            if (kd != vd) can_q8 = 0;
        }
        s->kv_q8 = can_q8;
        s->kv_row_u16 = can_q8 ? ((int64_t)hp->n_head_kv * 68) : (int64_t)n_kv;
    }
    {
        uint64_t fp = 14695981039346656037ULL;
        uint64_t ks[11] = {
            (uint64_t)hp->n_phys, (uint64_t)hp->n_embd, (uint64_t)hp->n_head, (uint64_t)hp->n_head_kv,
            (uint64_t)hp->n_ff_exp, (uint64_t)hp->n_expert, (uint64_t)hp->n_vocab, (uint64_t)hp->swa,
            (uint64_t)s->gguf.file_size, (uint64_t)s->kv_q8, s->lora.fp
        };
        for (int ki = 0; ki < 11; ki++) { fp ^= ks[ki]; fp *= 1099511628211ULL; }
        s->model_fp = fp;
    }
    size_t kv_row = (size_t)s->kv_row_u16;
    int full_slots = 0, swa_slots = 0;
    size_t kv_bytes = 0;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
        int swa_cap = hp->swa + s->prefill_b;
        int cap = hp->is_swa[ip] ? (swa_cap < s->max_ctx ? swa_cap : s->max_ctx) : s->max_ctx;
        if (hp->is_swa[ip]) swa_slots++; else full_slots++;
        kv_bytes += (size_t)cap * kv_row * 4 + (2 * n_kv + 2 * d) * 3 * 4;
    }
    size_t attn_prefill_bytes = use_flash_attn ? 0 : (B * (size_t)hp->n_head * (size_t)s->max_ctx * 2);
    size_t attn_decode_bytes = (size_t)hp->n_head * (size_t)s->max_ctx * 4;
    size_t attn_split_min = (size_t)8 * (size_t)hp->n_head * 32 * 132 * 4;
    size_t attn_bytes = attn_prefill_bytes > attn_decode_bytes ? attn_prefill_bytes : attn_decode_bytes;
    if (attn_bytes < attn_split_min) attn_bytes = attn_split_min;
    s->attn_scores_bytes = attn_bytes;
    size_t w_bf16_u32 = (ram_mb > 0 && ram_mb <= 28672) ? (size_t)(24 * 1024 * 1024) : (size_t)(44 * 1024 * 1024);
    size_t scratch_bytes = attn_bytes + w_bf16_u32 * 4 +
                           B * (size_t)(48 * d + 6 * n_q + 4 * n_kv + 3 * (size_t)hp->n_expert_used * (size_t)hp->n_ff_exp) * 4;
    int64_t os_reserve_mb = (ram_mb > 0 && ram_mb <= 18432) ? 2048 : (ram_mb > 0 && ram_mb <= 28672) ? 3072 : 4096;
    int64_t weights_mb = (int64_t)((s->gguf.map_size + 1048575ULL) / 1048576ULL);
    int64_t kv_mb = (int64_t)((kv_bytes + 1048575ULL) / 1048576ULL);
    int64_t scratch_mb = (int64_t)((scratch_bytes + 1048575ULL) / 1048576ULL);
    int64_t total_mb = os_reserve_mb + weights_mb + kv_mb + scratch_mb;
    int64_t fixed_mb = os_reserve_mb + weights_mb + scratch_mb;
    int64_t max_ctx_fit = 0;
    if (ram_mb > fixed_mb && full_slots > 0) {
        int64_t swa_ring_bytes = (int64_t)swa_slots * (int64_t)(hp->swa + s->prefill_b) * (int64_t)(kv_row * 4);
        int64_t avail_full = (ram_mb - fixed_mb) * 1048576LL - swa_ring_bytes;
        if (avail_full > 0) max_ctx_fit = avail_full / ((int64_t)full_slots * (int64_t)(kv_row * 4));
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
    if (ram_mb > 0 && ram_mb < 12288) {
        snprintf(err, err_bytes,
                 "insufficient unified memory: %lld MB detected (requires >= 12288 MB for Inkling 2 Mini; weights=%lld MB)",
                 (long long)ram_mb, (long long)weights_mb);
        free(ablate_host);
        ink_close(s); return 1;
    }
    if (ram_mb > 0 && total_mb > ram_mb) {
        snprintf(err, err_bytes,
                 "context %d exceeds unified memory budget (%lld MB required > %lld MB RAM; weights=%lld MB, max_ctx=%lld)",
                 s->max_ctx, (long long)total_mb, (long long)ram_mb, (long long)weights_mb, (long long)max_ctx_fit);
        free(ablate_host);
        ink_close(s); return 1;
    }

    double t_open0 = open_now_s();
    size_t metal_len = 0;
    char *metal_src = read_entire_file(metal_path, &metal_len);
    if (!metal_src) {
        snprintf(err, err_bytes, "cannot read metal source %s", metal_path);
        free(ablate_host);
        ink_close(s); return 1;
    }
    double compile_s = 0.0;
    PmStatus cs = pm_compile_fast(metal_src, metal_len, "inkling", &s->module, &compile_s);
    free(metal_src);
    if (cs != PM_OK) {
        snprintf(err, err_bytes, "pm_compile(%s): %s", metal_path, pm_log());
        free(ablate_host);
        ink_close(s); return 1;
    }
    double t_pso0 = open_now_s();
    if (pm_kernels_batch(s->module, k_names, K_COUNT, s->kernels) != PM_OK) {
        snprintf(err, err_bytes, "pm_kernels_batch: %s", pm_log());
        free(ablate_host);
        ink_close(s); return 1;
    }
    double t_pso1 = open_now_s();
    madvise(s->gguf.map, s->gguf.map_size, MADV_SEQUENTIAL);

    if (alloc_buf_unset(s, &s->ggml_grids, (size_t)INK_GRID_TOTAL_U32 * sizeof(uint32_t))) {
        snprintf(err, err_bytes, "%s", s->error);
        free(ablate_host);
        ink_close(s); return 1;
    }
    memcpy(pm_contents(s->ggml_grids), ink_ggml_grid_table(NULL), (size_t)INK_GRID_TOTAL_U32 * sizeof(uint32_t));
    pm_mark_readonly(s->ggml_grids);

    if (s->has_ablate && ablate_host) {
        size_t ab_bytes = (size_t)(1 + hp->n_phys) * d * sizeof(float);
        if (alloc_buf_unset(s, &s->ablate_dirs, ab_bytes)) {
            snprintf(err, err_bytes, "%s", s->error);
            free(ablate_host);
            ink_close(s); return 1;
        }
        memcpy(pm_contents(s->ablate_dirs), ablate_host, ab_bytes);
        pm_mark_readonly(s->ablate_dirs);
    }
    free(ablate_host);
    ablate_host = NULL;

    if (alloc_buf_unset(s, &s->cur, B * d * 4) ||
        alloc_buf_unset(s, &s->inp_sa, B * d * 4) ||
        alloc_buf_unset(s, &s->ffn_inp, B * d * 4) ||
        alloc_buf_unset(s, &s->prelude, B * d * 4) ||
        alloc_buf_unset(s, &s->bank_out, B * d * 4) ||
        alloc_buf_unset(s, &s->blocks, INK_MAX_BLOCKS * B * d * 4) ||
        alloc_buf_unset(s, &s->prelude_blk, INK_MAX_BLOCKS * B * d * 4) ||
        alloc_buf_unset(s, &s->bank_blk, INK_MAX_BLOCKS * B * d * 4) ||
        alloc_buf_unset(s, &s->mix_out, 4 * B * d * 4) ||
        alloc_buf_unset(s, &s->messages, ((4 * B > (size_t)(hp->loop_end - hp->loop_start) * 4 * 8) ? (4 * B) : ((size_t)(hp->loop_end - hp->loop_start) * 4 * 8)) * d * 4) ||
        alloc_buf_unset(s, &s->inp_q, B * d * 4) ||
        alloc_buf_unset(s, &s->inp_k, B * d * 4) ||
        alloc_buf_unset(s, &s->inp_v, B * d * 4) ||
        alloc_buf_unset(s, &s->ctrl_cat, 3 * 2 * B * d * 4) ||
        alloc_buf_unset(s, &s->ctrl_gate, 3 * B * (size_t)hp->gate_rank * 4) ||
        alloc_buf_unset(s, &s->ctrl_gain, d * 4) ||
        alloc_buf_unset(s, &s->ctrl_forget, d * 4) ||
        alloc_buf_unset(s, &s->q_buf, B * n_q * 4) ||
        alloc_buf_unset(s, &s->k_raw, B * n_kv * 4) ||
        alloc_buf_unset(s, &s->k_buf, B * n_kv * 4) ||
        alloc_buf_unset(s, &s->v_raw, B * n_kv * 4) ||
        alloc_buf_unset(s, &s->v_buf, B * n_kv * 4) ||
        alloc_buf_unset(s, &s->r_buf, B * (size_t)hp->n_head * (size_t)hp->rel_len * 4) ||
        alloc_buf_unset(s, &s->dyn_buf, 2 * B * (size_t)hp->n_head_kv * (size_t)hp->conv_len * 4) ||
        alloc_buf_unset(s, &s->sinks_buf, B * (size_t)hp->n_head * 4) ||
        alloc_buf_unset(s, &s->gate_buf, B * n_q * 4) ||
        alloc_buf_unset(s, &s->attn_scores, attn_bytes) ||
        alloc_buf_unset(s, &s->attn_out, B * n_q * 4) ||
        alloc_buf_unset(s, &s->o_raw, B * d * 4) ||
        alloc_buf_unset(s, &s->norm_buf, B * d * 4) ||
        alloc_buf_unset(s, &s->router_logits, B * (size_t)hp->n_expert * 4) ||
        alloc_buf_unset(s, &s->moe_ids, B * (size_t)hp->n_expert_used * 4) ||
        alloc_buf_unset(s, &s->moe_wts, B * (size_t)hp->n_expert_used * 4) ||
        alloc_buf_unset(s, &s->moe_hid, B * (size_t)hp->n_expert_used * (size_t)hp->n_ff_exp * 4) ||
        alloc_buf_unset(s, &s->moe_gu, B * (size_t)hp->n_expert_used * 2 * (size_t)hp->n_ff_exp * 4) ||
        alloc_buf_unset(s, &s->moe_out, B * d * 4) ||
        alloc_buf_unset(s, &s->moe_slot_out, B * (size_t)hp->n_expert_used * d * 4) ||
        alloc_buf_unset(s, &s->exp_toks, (size_t)hp->n_expert * INK_PREFILL_B * 4) ||
        alloc_buf_unset(s, &s->exp_slots, (size_t)hp->n_expert * INK_PREFILL_B * 4) ||
        alloc_buf_unset(s, &s->exp_wts, (size_t)hp->n_expert * INK_PREFILL_B * 4) ||
        alloc_buf(s, &s->exp_cnt, (size_t)hp->n_expert * 4) ||
        alloc_buf_unset(s, &s->exp_off, (size_t)hp->n_expert * 4) ||
        alloc_buf_unset(s, &s->row_dst, B * (size_t)hp->n_expert_used * 4) ||
        alloc_buf_unset(s, &s->row_wt, B * (size_t)hp->n_expert_used * 4) ||
        alloc_buf_unset(s, &s->tile_exp, ((size_t)hp->n_expert + INK_PREFILL_B / 2 + 16) * 4) ||
        alloc_buf_unset(s, &s->tile_m0, ((size_t)hp->n_expert + INK_PREFILL_B / 2 + 16) * 4) ||
        alloc_buf(s, &s->tile_cnt, 16 * 4) ||
        alloc_buf_unset(s, &s->w_bf16, w_bf16_u32 * 4) ||
        alloc_buf_unset(s, &s->tok_ids_buf, B * 4) ||
        alloc_buf_unset(s, &s->tau_buf, B * 4) ||
        alloc_buf_unset(s, &s->logits_buf[0], (size_t)hp->n_vocab * 4) ||
        alloc_buf_unset(s, &s->logits_buf[1], (size_t)hp->n_vocab * 4) ||
        alloc_buf_unset(s, &s->hidden_out[0], d * 4) ||
        alloc_buf_unset(s, &s->hidden_out[1], d * 4) ||
        alloc_buf_unset(s, &s->argmax_vals, 8 * 64 * 4) ||
        alloc_buf_unset(s, &s->argmax_ids, 8 * 64 * 4) ||
        alloc_buf(s, &s->argmax_out_buf[0], 4) ||
        alloc_buf(s, &s->argmax_out_buf[1], 4) ||
        alloc_buf_unset(s, &s->topk_s1_vals, 8 * 32 * 64 * 4) ||
        alloc_buf_unset(s, &s->topk_s1_ids, 8 * 32 * 64 * 4) ||
        alloc_buf_unset(s, &s->topk_vals[0], 64 * 4) ||
        alloc_buf_unset(s, &s->topk_vals[1], 64 * 4) ||
        alloc_buf_unset(s, &s->topk_ids[0], 64 * 4) ||
        alloc_buf_unset(s, &s->topk_ids[1], 64 * 4) ||
        alloc_buf_unset(s, &s->spec_topk_vals, 8 * 64 * 4) ||
        alloc_buf_unset(s, &s->spec_topk_ids, 8 * 64 * 4) ||
        alloc_buf(s, &s->dummy_f32, 64 * 4) ||
        alloc_buf(s, &s->dummy_ones, 8 * 4) ||
        alloc_buf(s, &s->rms_parts, 8 * 3 * 128 * 4) ||
        alloc_buf_unset(s, &s->spec_u_eids, 64 * 4) ||
        alloc_buf_unset(s, &s->spec_u_mask, 64 * 4) ||
        alloc_buf_unset(s, &s->spec_u_slot, 64 * 8 * 4) ||
        alloc_buf(s, &s->spec_u_cnt, 4 * 4) ||
        alloc_buf_unset(s, &s->spec_logits, (size_t)8 * (size_t)hp->n_vocab * 4) ||
        alloc_buf_unset(s, &s->spec_hidden, (size_t)8 * d * 4) ||
        alloc_buf(s, &s->spec_argmax_out, 8 * 4)) {
        snprintf(err, err_bytes, "%s", s->error);
        ink_close(s); return 1;
    }
    {
        float *ones = (float *)pm_contents(s->dummy_ones);
        for (int i = 0; i < 8; i++) ones[i] = 1.0f;
    }
    s->tok_hist = (int32_t *)malloc((size_t)s->max_ctx * sizeof(int32_t));
    if (!s->tok_hist) {
        snprintf(err, err_bytes, "out of memory (tok_hist)");
        ink_close(s); return 1;
    }
    s->spec_active_idx = -1;
    s->pld_ema2 = 0.35f;
    s->pld_ema3 = 0.55f;

    s->conv_slot_stride = (int64_t)2 * n_kv * 4 + (int64_t)2 * d * 3;
    s->conv_all_elems = (int64_t)s->n_slots * s->conv_slot_stride;
    s->raw_slot_stride = (int64_t)8 * ((int64_t)2 * n_kv + (int64_t)2 * d);
    if (alloc_buf(s, &s->conv_all, (size_t)s->conv_all_elems * 4) ||
        alloc_buf(s, &s->conv_backup, (size_t)s->conv_all_elems * 4) ||
        alloc_buf_unset(s, &s->conv_raw_log, (size_t)s->n_slots * (size_t)s->raw_slot_stride * 4)) {
        snprintf(err, err_bytes, "%s", s->error);
        ink_close(s); return 1;
    }

    s->swa_cap_min = s->max_ctx;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
        /* SWA ring buffer must hold at least (swa + prefill_b) entries so K_KV_STORE_BATCH never
         * overwrites earlier keys in [k_start, pos + nb) before MMA prefill attention reads them. */
        int swa_cap = hp->swa + s->prefill_b;
        int cap = hp->is_swa[ip] ? (swa_cap < s->max_ctx ? swa_cap : s->max_ctx) : s->max_ctx;
        s->kv_cap[il] = cap;
        if (hp->is_swa[ip] && cap < s->swa_cap_min) s->swa_cap_min = cap;
        if (alloc_buf_unset(s, &s->kv_k[il], (size_t)cap * kv_row * 2) ||
            alloc_buf_unset(s, &s->kv_v[il], (size_t)cap * kv_row * 2)) {
            snprintf(err, err_bytes, "%s", s->error);
            ink_close(s); return 1;
        }
    }

    s->live_tokens = (int32_t *)malloc((size_t)s->max_ctx * sizeof(int32_t));
    if (!s->live_tokens) {
        snprintf(err, err_bytes, "out of memory allocating live_tokens");
        ink_close(s); return 1;
    }

    double t_fw0 = open_now_s();
    build_fast_weights(s);
    if (compact_or_wrap_weights(s, err, err_bytes) != 0) {
        ink_close(s); return 1;
    }
    s->ema_base_ms = s->fast_is_q8s ? 10.5f : 9.2f;
    {
        static const float init_vr[9] = {1.0f, 1.0f, 1.55f, 1.95f, 2.30f, 2.60f, 2.90f, 3.18f, 3.45f};
        for (int i = 0; i < 9; i++) s->ema_vratio[i] = init_vr[i];
    }
    double t_fw1 = open_now_s();
    ink_dspark_open(s, gguf_path);
    double t_ds1 = open_now_s();
    pm_commit_residency();
    double t_res1 = open_now_s();
    if (getenv("INK_OPEN_TIMING") != NULL) {
        fprintf(stderr, "[INK_OPEN_TIMING compile=%.3fs pso=%.3fs alloc=%.3fs fast_w=%.3fs dspark=%.3fs residency=%.3fs total=%.3fs]\n",
                compile_s, t_pso1 - t_pso0, t_fw0 - t_pso1, t_fw1 - t_fw0, t_ds1 - t_fw1, t_res1 - t_ds1, t_res1 - t_open0);
    }

    s->pos = 0;
    s->live_len = 0;
    s->swa_valid_from = 0;
    s->dspark_live_len = 0;
    s->dspark_swa_valid_from = 0;
    *out = s;
    return 0;
}

static inline void ink_mark_written(InkSession *s, int p_end) {
    if (p_end > s->max_pos_written) s->max_pos_written = p_end;
    if (s->swa_cap_min > 0 && p_end - s->swa_cap_min > s->swa_valid_from) {
        s->swa_valid_from = p_end - s->swa_cap_min;
    }
}

static void ink_discard_prep(InkSession *s) {
    if (s && s->prep_batch) {
        pm_abort(s->prep_batch);
        s->prep_batch = NULL;
    }
}

static void ink_cancel_inflight(InkSession *s) {
    if (!s) return;
    ink_discard_prep(s);
    if (s->inflight_active) {
        pm_wait_ticket(s->inflight_ticket, s->inflight_slot, NULL);
        if (s->conv_all && s->conv_backup && s->conv_all_elems > 0) {
            memcpy(pm_contents(s->conv_all), pm_contents(s->conv_backup), (size_t)s->conv_all_elems * sizeof(float));
        }
        s->next_m4_slot = s->inflight_slot ^ 1;
        s->inflight_active = 0;
    }
}

void ink_close(InkSession *s) {
    if (!s) return;
    ink_cancel_inflight(s);
    if (s->disk_thr_started) {
        pthread_mutex_lock(&s->disk_mu);
        s->disk_stop = 1;
        pthread_cond_signal(&s->disk_cv);
        pthread_mutex_unlock(&s->disk_mu);
        pthread_join(s->disk_thr, NULL);
        pthread_mutex_destroy(&s->disk_mu);
        pthread_cond_destroy(&s->disk_cv);
        s->disk_thr_started = 0;
    }
    free(s->live_tokens);
    free(s->spec_old_conv);
    free(s->pred_toks);
    free(s->sess_hist);
    pm_free(s->verify_logits);
    pm_free(s->spec_hist);
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        free(s->snaps[i].tokens);
        free(s->snaps[i].conv_state);
        free(s->snaps[i].logits);
        free(s->snaps[i].swa_kv);
        free(s->snaps[i].dspark_last_taps);
        free(s->snaps[i].dspark_swa_kv);
    }
    ink_dspark_close(s);
    for (int il = 0; il < INK_MAX_SLOTS; il++) {
        pm_free(s->kv_k[il]); pm_free(s->kv_v[il]);
    }
    pm_free(s->conv_all); pm_free(s->conv_backup); pm_free(s->conv_raw_log);
    free(s->tok_hist);
    pm_free(s->spec_u_eids); pm_free(s->spec_u_mask); pm_free(s->spec_u_slot); pm_free(s->spec_u_cnt);
    pm_free(s->spec_logits); pm_free(s->spec_hidden); pm_free(s->spec_argmax_out);
    pm_free(s->dummy_ones);
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
    pm_free(s->logits_buf[0]); pm_free(s->logits_buf[1]);
    pm_free(s->hidden_out[0]); pm_free(s->hidden_out[1]);
    pm_free(s->argmax_vals); pm_free(s->argmax_ids);
    pm_free(s->argmax_out_buf[0]); pm_free(s->argmax_out_buf[1]);
    pm_free(s->topk_s1_vals); pm_free(s->topk_s1_ids);
    pm_free(s->topk_vals[0]); pm_free(s->topk_vals[1]);
    pm_free(s->topk_ids[0]); pm_free(s->topk_ids[1]);
    pm_free(s->spec_topk_vals); pm_free(s->spec_topk_ids);
    pm_free(s->dummy_f32);
    pm_free(s->rms_parts);
    pm_free(s->iq4s_buf);
    pm_free(s->fast_buf);
    pm_free(s->weights_buf);
    pm_free(s->ablate_dirs);
    pm_free(s->ggml_grids);
    for (int k = 0; k < K_COUNT; k++) pm_kernel_free(s->kernels[k]);
    pm_unload(s->module);
    ink_lora_close(&s->lora);
    ink_gguf_close(&s->gguf);
    free(s);
}

static int ink_enqueue_conv_commit(InkSession *s, PmBatch batch) {
    if (!s || s->conv_pending_n <= 0) return 0;
    const InkHParams *hp = &s->gguf.hp;
    int64_t n_kv = hp->n_head_kv * hp->head_dim;
    int64_t d = hp->n_embd;
    int64_t total_ch = (int64_t)s->n_slots * (2 * n_kv + 2 * d);
    CALL(s, batch, K_CONV_COMMIT_NB, total_ch,
         a_int(s->n_slots), a_int(n_kv), a_int(d),
         a_int(s->conv_pending_pos0), a_int(s->conv_pending_n),
         a_f32v(s->conv_all, 0, s->conv_all_elems),
         a_f32v(s->conv_raw_log, 0, (int64_t)s->n_slots * s->raw_slot_stride));
    pm_barrier(batch);
    s->conv_pending_n = 0;
    return 0;
}

static void ink_flush_spec(InkSession *s) {
    if (!s) return;
    if (s->conv_pending_n > 0 || (s->dspark.ready && s->dspark.pending_got > 0)) {
        PmBatch batch = NULL;
        if (pm_begin_concurrent(&batch) == PM_OK) {
            int ok = 1;
            if (s->conv_pending_n > 0 && ink_enqueue_conv_commit(s, batch) != 0) ok = 0;
            if (ok && s->dspark.ready && s->dspark.pending_got > 0 && ink_dspark_flush_pending(s, batch) != 0) ok = 0;
            if (ok) pm_wait(batch, NULL);
            else pm_abort(batch);
        }
        s->conv_pending_n = 0;
        s->dspark.pending_got = 0;
        s->dspark.pending_nb = 0;
    }
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;
}

void ink_reset(InkSession *s) {
    if (!s) return;
    ink_cancel_inflight(s);
    s->conv_pending_n = 0;
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;
    s->tok_hist_len = 0;
    s->pld_ema2 = 0.35f;
    s->pld_ema3 = 0.55f;
    s->spec_rounds = 0;
    s->spec_drafted = 0;
    s->spec_accepted = 0;
    s->dspark.pos = 0;
    s->dspark.pending_pos0 = 0;
    s->dspark.pending_nb = 0;
    s->dspark.pending_got = 0;
    s->dspark.skip_countdown = 0;
    s->dspark.req_disabled = 0;
    s->dspark.req_rounds = 0;
    s->dspark.skip_mult = 1;
    s->dspark.consec_unprof = 0;
    s->dspark.ema_acc = 3.0f;
    s->dspark.util_ema = 1.15f;
    s->dspark.ema_cal = 0.90f;
    s->dspark.ema_net_speedup = 1.15f;
    if (s->dspark.b1_ema_s <= 1e-4) s->dspark.b1_ema_s = (s->fast_output_s.qs.off >= 0 && s->fast_output_is_q4s) ? 0.012 : 0.016;
    s->dspark.rounds = 0;
    s->dspark.drafted = 0;
    s->dspark.accepted = 0;
    s->dspark.draft_gpu_s = 0.0;
    s->sampled_mode = 0;
    s->spec_round_open = 0;
    s->ema_base_ms = s->fast_is_q8s ? 10.5f : 9.2f;
    {
        static const float init_vr[9] = {1.0f, 1.0f, 1.55f, 1.95f, 2.30f, 2.60f, 2.90f, 3.18f, 3.45f};
        for (int i = 0; i < 9; i++) s->ema_vratio[i] = init_vr[i];
    }
    if (s->conv_all && s->conv_all_elems > 0) {
        memset(pm_contents(s->conv_all), 0, (size_t)s->conv_all_elems * sizeof(float));
    }
    s->pos = 0;
    s->live_len = 0;
    s->max_pos_written = 0;
    s->swa_valid_from = 0;
    s->dspark_live_len = 0;
    s->dspark_swa_valid_from = 0;
    s->last_hidden_off = 0;
    s->active_out_slot = 0;
    s->hidden_is_prefill = 0;
    s->argmax_valid = 0;
    s->has_prev_argmax = 0;
    s->greedy_streak = 0;
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        s->snaps[i].valid = 0;
        s->snaps[i].pinned = 0;
        s->snaps[i].swa_win = 0;
        s->snaps[i].dspark_swa_win = 0;
    }
}

/* ---- Ch 176: Bounded NVMe Cold-Tier DiskPromptCache (~/.openagents/cache/psionic/prompts/) ---- */
typedef struct {
    char     magic[4];       /* "INKP" */
    uint32_t version;        /* 2 (v2 persists DSpark last_taps + 3-layer KV) */
    uint64_t model_fp;
    uint64_t tok_hash;
    int32_t  n_tokens;
    int32_t  kv_q8;
    int32_t  kv_row_u16;
    int32_t  n_slots;
    int32_t  full_slots;
    int32_t  swa_slots;
    int32_t  swa_win;
    int32_t  conv_all_elems;
    int32_t  has_logits;
    int32_t  n_vocab;
    int32_t  has_dspark;
    int32_t  dspark_swa_win;
} InkDiskSnapHdr;

static uint64_t ink_fnv1a_tokens(const int32_t *tokens, int n) {
    uint64_t h = 14695981039346656037ULL;
    for (int i = 0; i < n; i++) {
        h ^= (uint32_t)tokens[i];
        h *= 1099511628211ULL;
    }
    return h;
}

static int ink_disk_cache_dir(char *out, size_t cap) {
    const char *home = getenv("HOME");
    if (!home || !home[0]) return 1;
    char d1[512], d2[512], d3[512];
    if (snprintf(d1, sizeof d1, "%s/.openagents", home) >= (int)sizeof d1) return 1;
    if (snprintf(d2, sizeof d2, "%s/.openagents/cache", home) >= (int)sizeof d2) return 1;
    if (snprintf(d3, sizeof d3, "%s/.openagents/cache/psionic", home) >= (int)sizeof d3) return 1;
    if (snprintf(out, cap, "%s/.openagents/cache/psionic/prompts", home) >= (int)cap) return 1;
    mkdir(d1, 0755); mkdir(d2, 0755); mkdir(d3, 0755); mkdir(out, 0755);
    return 0;
}

static int ink_write_all(int fd, const void *buf, size_t n) {
    const uint8_t *p = (const uint8_t *)buf;
    while (n > 0) {
        size_t chunk = n > (16ULL << 20) ? (16ULL << 20) : n;
        ssize_t w = write(fd, p, chunk);
        if (w <= 0) return 1;
        p += (size_t)w;
        n -= (size_t)w;
    }
    return 0;
}

static int ink_read_all(int fd, void *buf, size_t n) {
    uint8_t *p = (uint8_t *)buf;
    while (n > 0) {
        size_t chunk = n > (16ULL << 20) ? (16ULL << 20) : n;
        ssize_t r = read(fd, p, chunk);
        if (r <= 0) return 1;
        p += (size_t)r;
        n -= (size_t)r;
    }
    return 0;
}

static void ink_disk_cache_prune(const char *dir, int max_files) {
    DIR *dp = opendir(dir);
    if (!dp) return;
    typedef struct { char path[640]; time_t mt; } Ent;
    Ent ents[64];
    int n = 0;
    struct dirent *de;
    while ((de = readdir(dp)) != NULL && n < 64) {
        size_t nl = strlen(de->d_name);
        if (nl < 6 || strcmp(de->d_name + nl - 5, ".inkp") != 0) continue;
        char full[640];
        if (snprintf(full, sizeof full, "%s/%s", dir, de->d_name) >= (int)sizeof full) continue;
        struct stat st;
        if (stat(full, &st) == 0) {
            memcpy(ents[n].path, full, sizeof full);
            ents[n].mt = st.st_mtime;
            n++;
        }
    }
    closedir(dp);
    while (n > max_files) {
        int oldest = 0;
        for (int i = 1; i < n; i++) if (ents[i].mt < ents[oldest].mt) oldest = i;
        unlink(ents[oldest].path);
        ents[oldest] = ents[--n];
    }
}

static void *ink_disk_worker(void *arg) {
    InkSession *s = (InkSession *)arg;
    for (;;) {
        pthread_mutex_lock(&s->disk_mu);
        while (s->disk_q_len == 0 && !s->disk_stop) {
            pthread_cond_wait(&s->disk_cv, &s->disk_mu);
        }
        if (s->disk_q_len == 0 && s->disk_stop) {
            pthread_mutex_unlock(&s->disk_mu);
            break;
        }
        char dir[512], final_path[640], tmp_path[680];
        memcpy(dir, s->disk_q[s->disk_q_head].dir, sizeof dir);
        memcpy(final_path, s->disk_q[s->disk_q_head].final_path, sizeof final_path);
        memcpy(tmp_path, s->disk_q[s->disk_q_head].tmp_path, sizeof tmp_path);
        uint8_t *payload = s->disk_q[s->disk_q_head].payload;
        size_t payload_bytes = s->disk_q[s->disk_q_head].payload_bytes;
        s->disk_q[s->disk_q_head].payload = NULL;
        s->disk_q_head = (s->disk_q_head + 1) & 3;
        s->disk_q_len--;
        pthread_mutex_unlock(&s->disk_mu);

        if (payload) {
            struct stat st;
            if (stat(final_path, &st) != 0) {
                int fd = open(tmp_path, O_CREAT | O_WRONLY | O_TRUNC, 0644);
                if (fd >= 0) {
                    int fail = ink_write_all(fd, payload, payload_bytes);
                    close(fd);
                    if (fail || rename(tmp_path, final_path) != 0) {
                        unlink(tmp_path);
                    } else {
                        ink_disk_cache_prune(dir, 8);
                    }
                }
            }
            free(payload);
        }
    }
    return NULL;
}

static void ink_disk_cache_save(InkSession *s, const int32_t *tokens, int n_tokens, int has_lg, const float *lg_src) {
    const char *noc = getenv("INK_NO_DISK_CACHE");
    if ((noc && noc[0] != '0' && noc[0] != '\0') || n_tokens < 1024 || n_tokens > s->max_ctx) return;
    char dir[512];
    if (ink_disk_cache_dir(dir, sizeof dir) != 0) return;
    uint64_t th = ink_fnv1a_tokens(tokens, n_tokens);
    char final_path[640], tmp_path[680];
    if (snprintf(final_path, sizeof final_path, "%s/%016llx_%016llx_%d.inkp",
                 dir, (unsigned long long)s->model_fp, (unsigned long long)th, n_tokens) >= (int)sizeof final_path) return;
    struct stat st;
    if (stat(final_path, &st) == 0) return; /* already cached on SSD */
    if (snprintf(tmp_path, sizeof tmp_path, "%s.tmp.%d", final_path, (int)getpid()) >= (int)sizeof tmp_path) return;

    const InkHParams *hp = &s->gguf.hp;
    int win = (hp->swa > 0) ? (n_tokens < (int)hp->swa ? n_tokens : (int)hp->swa) : 0;
    int save_ds = (s->dspark.ready && s->dspark.pos == n_tokens);
    int ds_win = save_ds ? (n_tokens < 1024 ? n_tokens : 1024) : 0;
    InkDiskSnapHdr hdr;
    memset(&hdr, 0, sizeof hdr);
    memcpy(hdr.magic, "INKP", 4);
    hdr.version = 2;
    hdr.model_fp = s->model_fp;
    hdr.tok_hash = th;
    hdr.n_tokens = n_tokens;
    hdr.kv_q8 = s->kv_q8;
    hdr.kv_row_u16 = (int32_t)s->kv_row_u16;
    hdr.n_slots = s->n_slots;
    hdr.full_slots = s->budget.full_slots;
    hdr.swa_slots = s->budget.swa_slots;
    hdr.swa_win = win;
    hdr.conv_all_elems = (int32_t)s->conv_all_elems;
    hdr.has_logits = (has_lg && lg_src) ? 1 : 0;
    hdr.n_vocab = (int32_t)hp->n_vocab;
    hdr.has_dspark = save_ds;
    hdr.dspark_swa_win = ds_win;

    size_t kv_row = (size_t)s->kv_row_u16;
    size_t ds_bytes = 0;
    if (save_ds) {
        ds_bytes = (size_t)4 * 2304 * sizeof(float)
                 + (size_t)2 * 2 * (size_t)ds_win * 1024 * sizeof(uint16_t)
                 + (size_t)1 * 2 * (size_t)n_tokens * 1024 * sizeof(uint16_t);
    }
    size_t total_bytes = sizeof hdr
                       + (size_t)n_tokens * sizeof(int32_t)
                       + (size_t)s->conv_all_elems * sizeof(float)
                       + (size_t)s->budget.full_slots * 2 * (size_t)n_tokens * kv_row * sizeof(uint16_t)
                       + (size_t)s->budget.swa_slots * 2 * (size_t)win * kv_row * sizeof(uint16_t)
                       + (hdr.has_logits ? (size_t)hp->n_vocab * sizeof(float) : 0)
                       + ds_bytes;
    uint8_t *payload = (uint8_t *)malloc(total_bytes);
    if (!payload) return;
    uint8_t *dst = payload;
    memcpy(dst, &hdr, sizeof hdr); dst += sizeof hdr;
    memcpy(dst, tokens, (size_t)n_tokens * sizeof(int32_t)); dst += (size_t)n_tokens * sizeof(int32_t);
    memcpy(dst, pm_contents(s->conv_all), (size_t)s->conv_all_elems * sizeof(float)); dst += (size_t)s->conv_all_elems * sizeof(float);
    int p0 = n_tokens - win;
    for (int il = 0; il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
        const uint16_t *kk = (const uint16_t *)pm_contents(s->kv_k[il]);
        const uint16_t *kv = (const uint16_t *)pm_contents(s->kv_v[il]);
        if (!hp->is_swa[ip]) {
            size_t nb_k = (size_t)n_tokens * kv_row * sizeof(uint16_t);
            memcpy(dst, kk, nb_k); dst += nb_k;
            memcpy(dst, kv, nb_k); dst += nb_k;
        } else if (win > 0) {
            int cap = s->kv_cap[il];
            int r0 = p0 % cap;
            int first = (r0 + win <= cap) ? win : (cap - r0);
            int second = win - first;
            size_t b1 = (size_t)first * kv_row * sizeof(uint16_t);
            size_t b2 = (size_t)second * kv_row * sizeof(uint16_t);
            memcpy(dst, kk + (size_t)r0 * kv_row, b1); dst += b1;
            if (b2 > 0) { memcpy(dst, kk, b2); dst += b2; }
            memcpy(dst, kv + (size_t)r0 * kv_row, b1); dst += b1;
            if (b2 > 0) { memcpy(dst, kv, b2); dst += b2; }
        }
    }
    if (hdr.has_logits) {
        size_t lb = (size_t)hp->n_vocab * sizeof(float);
        memcpy(dst, lg_src, lb); dst += lb;
    }
    if (save_ds) {
        size_t tb = (size_t)4 * 2304 * sizeof(float);
        memcpy(dst, pm_contents(s->dspark.last_taps), tb); dst += tb;
        int ds_p0 = n_tokens - ds_win;
        for (int li = 0; li < 3; li++) {
            const DsparkLayer *L = &s->dspark.layers[li];
            const uint16_t *kk = (const uint16_t *)pm_contents(L->kv_k);
            const uint16_t *kv = (const uint16_t *)pm_contents(L->kv_v);
            if (!L->is_swa) {
                size_t nb_k = (size_t)n_tokens * 1024 * sizeof(uint16_t);
                memcpy(dst, kk, nb_k); dst += nb_k;
                memcpy(dst, kv, nb_k); dst += nb_k;
            } else if (ds_win > 0) {
                int cap = L->kv_cap;
                int r0 = ds_p0 % cap;
                int first = (r0 + ds_win <= cap) ? ds_win : (cap - r0);
                int second = ds_win - first;
                size_t b1 = (size_t)first * 1024 * sizeof(uint16_t);
                size_t b2 = (size_t)second * 1024 * sizeof(uint16_t);
                memcpy(dst, kk + (size_t)r0 * 1024, b1); dst += b1;
                if (b2 > 0) { memcpy(dst, kk, b2); dst += b2; }
                memcpy(dst, kv + (size_t)r0 * 1024, b1); dst += b1;
                if (b2 > 0) { memcpy(dst, kv, b2); dst += b2; }
            }
        }
    }

    if (!s->disk_thr_started) {
        pthread_mutex_init(&s->disk_mu, NULL);
        pthread_cond_init(&s->disk_cv, NULL);
        s->disk_stop = 0;
        s->disk_q_head = s->disk_q_tail = s->disk_q_len = 0;
        if (pthread_create(&s->disk_thr, NULL, ink_disk_worker, s) != 0) {
            pthread_mutex_destroy(&s->disk_mu);
            pthread_cond_destroy(&s->disk_cv);
            free(payload);
            return;
        }
        s->disk_thr_started = 1;
    }
    pthread_mutex_lock(&s->disk_mu);
    /* Skip if identical path is already queued. */
    for (int qi = 0; qi < s->disk_q_len; qi++) {
        int idx = (s->disk_q_head + qi) & 3;
        if (strcmp(s->disk_q[idx].final_path, final_path) == 0) {
            pthread_mutex_unlock(&s->disk_mu);
            free(payload);
            return;
        }
    }
    if (s->disk_q_len == 4) {
        /* Drop oldest queued job so memory stays strictly bounded. */
        free(s->disk_q[s->disk_q_head].payload);
        s->disk_q[s->disk_q_head].payload = NULL;
        s->disk_q_head = (s->disk_q_head + 1) & 3;
        s->disk_q_len--;
    }
    int slot = s->disk_q_tail;
    memcpy(s->disk_q[slot].dir, dir, sizeof dir);
    memcpy(s->disk_q[slot].final_path, final_path, sizeof final_path);
    memcpy(s->disk_q[slot].tmp_path, tmp_path, sizeof tmp_path);
    s->disk_q[slot].payload = payload;
    s->disk_q[slot].payload_bytes = (size_t)(dst - payload);
    s->disk_q_tail = (s->disk_q_tail + 1) & 3;
    s->disk_q_len++;
    pthread_cond_signal(&s->disk_cv);
    pthread_mutex_unlock(&s->disk_mu);
}

static int ink_disk_cache_restore(InkSession *s, const int32_t *tokens, int n_tokens) {
    const char *noc = getenv("INK_NO_DISK_CACHE");
    if ((noc && noc[0] != '0' && noc[0] != '\0') || n_tokens < 1024) return 0;
    char dir[512];
    if (ink_disk_cache_dir(dir, sizeof dir) != 0) return 0;
    DIR *dp = opendir(dir);
    if (!dp) return 0;

    char prefix[32];
    snprintf(prefix, sizeof prefix, "%016llx_", (unsigned long long)s->model_fp);
    size_t plen = strlen(prefix);
    const InkHParams *hp = &s->gguf.hp;

    char best_path[640] = "";
    InkDiskSnapHdr best_hdr;
    memset(&best_hdr, 0, sizeof best_hdr);
    int best_sp = 0;

    struct dirent *de;
    while ((de = readdir(dp)) != NULL) {
        size_t nl = strlen(de->d_name);
        if (nl < plen + 6 || strncmp(de->d_name, prefix, plen) != 0 ||
            strcmp(de->d_name + nl - 5, ".inkp") != 0) continue;
        char full[640];
        if (snprintf(full, sizeof full, "%s/%s", dir, de->d_name) >= (int)sizeof full) continue;
        int fd = open(full, O_RDONLY);
        if (fd < 0) continue;
        InkDiskSnapHdr hdr;
        if (ink_read_all(fd, &hdr, sizeof hdr) == 0 &&
            memcmp(hdr.magic, "INKP", 4) == 0 && hdr.version == 2 &&
            hdr.model_fp == s->model_fp && hdr.kv_q8 == s->kv_q8 &&
            hdr.kv_row_u16 == (int32_t)s->kv_row_u16 && hdr.n_slots == s->n_slots &&
            hdr.conv_all_elems == (int32_t)s->conv_all_elems && hdr.n_vocab == (int32_t)hp->n_vocab &&
            hdr.n_tokens > best_sp && hdr.n_tokens <= n_tokens && hdr.n_tokens <= s->max_ctx &&
            hdr.swa_win >= 0 && hdr.swa_win <= hdr.n_tokens &&
            (!hdr.has_dspark || (hdr.dspark_swa_win >= 0 && hdr.dspark_swa_win <= 1024 && hdr.dspark_swa_win <= hdr.n_tokens)) &&
            (hdr.n_tokens < n_tokens || hdr.has_logits)) {
            uint64_t want_h = ink_fnv1a_tokens(tokens, hdr.n_tokens);
            if (hdr.tok_hash == want_h) {
                int32_t *ftoks = (int32_t *)malloc((size_t)hdr.n_tokens * sizeof(int32_t));
                if (ftoks) {
                    if (ink_read_all(fd, ftoks, (size_t)hdr.n_tokens * sizeof(int32_t)) == 0 &&
                        memcmp(ftoks, tokens, (size_t)hdr.n_tokens * sizeof(int32_t)) == 0) {
                        best_sp = hdr.n_tokens;
                        best_hdr = hdr;
                        memcpy(best_path, full, sizeof full);
                    }
                    free(ftoks);
                }
            }
        }
        close(fd);
    }
    closedir(dp);
    if (best_sp <= 0) return 0;

    int fd = open(best_path, O_RDONLY);
    if (fd < 0) return 0;
    off_t skip = (off_t)(sizeof(InkDiskSnapHdr) + (size_t)best_sp * sizeof(int32_t));
    if (lseek(fd, skip, SEEK_SET) != skip) { close(fd); return 0; }
    int fail = ink_read_all(fd, pm_contents(s->conv_all), (size_t)s->conv_all_elems * sizeof(float));
    size_t kv_row = (size_t)s->kv_row_u16;
    int win = best_hdr.swa_win;
    int p0 = best_sp - win;
    for (int il = 0; !fail && il < s->n_slots; il++) {
        int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
        uint16_t *kk = (uint16_t *)pm_contents(s->kv_k[il]);
        uint16_t *kv = (uint16_t *)pm_contents(s->kv_v[il]);
        if (!hp->is_swa[ip]) {
            fail = ink_read_all(fd, kk, (size_t)best_sp * kv_row * sizeof(uint16_t)) ||
                   ink_read_all(fd, kv, (size_t)best_sp * kv_row * sizeof(uint16_t));
        } else if (win > 0) {
            int cap = s->kv_cap[il];
            int r0 = p0 % cap;
            int first = (r0 + win <= cap) ? win : (cap - r0);
            int second = win - first;
            fail = ink_read_all(fd, kk + (size_t)r0 * kv_row, (size_t)first * kv_row * sizeof(uint16_t)) ||
                   (second > 0 && ink_read_all(fd, kk, (size_t)second * kv_row * sizeof(uint16_t))) ||
                   ink_read_all(fd, kv + (size_t)r0 * kv_row, (size_t)first * kv_row * sizeof(uint16_t)) ||
                   (second > 0 && ink_read_all(fd, kv, (size_t)second * kv_row * sizeof(uint16_t)));
        }
    }
    if (!fail && best_hdr.has_logits) {
        size_t lb = (size_t)hp->n_vocab * sizeof(float);
        if (best_sp == n_tokens) {
            fail = ink_read_all(fd, pm_contents(s->logits_buf[0]), lb);
        } else if (best_hdr.has_dspark && s->dspark.ready) {
            if (lseek(fd, (off_t)lb, SEEK_CUR) < 0) fail = 1;
        }
    }
    int ds_restored = 0;
    int ds_p0 = 0;
    if (!fail && best_hdr.has_dspark && s->dspark.ready) {
        int ds_win = best_hdr.dspark_swa_win;
        ds_p0 = best_sp - ds_win;
        fail = ink_read_all(fd, pm_contents(s->dspark.last_taps), (size_t)4 * 2304 * sizeof(float));
        for (int li = 0; !fail && li < 3; li++) {
            DsparkLayer *L = &s->dspark.layers[li];
            uint16_t *kk = (uint16_t *)pm_contents(L->kv_k);
            uint16_t *kv = (uint16_t *)pm_contents(L->kv_v);
            if (!L->is_swa) {
                fail = ink_read_all(fd, kk, (size_t)best_sp * 1024 * sizeof(uint16_t)) ||
                       ink_read_all(fd, kv, (size_t)best_sp * 1024 * sizeof(uint16_t));
            } else if (ds_win > 0) {
                int cap = L->kv_cap;
                int r0 = ds_p0 % cap;
                int first = (r0 + ds_win <= cap) ? ds_win : (cap - r0);
                int second = ds_win - first;
                fail = ink_read_all(fd, kk + (size_t)r0 * 1024, (size_t)first * 1024 * sizeof(uint16_t)) ||
                       (second > 0 && ink_read_all(fd, kk, (size_t)second * 1024 * sizeof(uint16_t))) ||
                       ink_read_all(fd, kv + (size_t)r0 * 1024, (size_t)first * 1024 * sizeof(uint16_t)) ||
                       (second > 0 && ink_read_all(fd, kv, (size_t)second * 1024 * sizeof(uint16_t)));
            }
        }
        if (!fail) ds_restored = 1;
    }
    close(fd);
    if (fail) { ink_reset(s); return 0; }

    if (s->live_tokens && best_sp <= s->max_ctx) {
        memcpy(s->live_tokens, tokens, (size_t)best_sp * sizeof(int32_t));
        s->live_len = best_sp;
    }
    if (s->tok_hist && best_sp <= s->max_ctx) {
        memcpy(s->tok_hist, tokens, (size_t)best_sp * sizeof(int32_t));
        s->tok_hist_len = best_sp;
    }
    s->swa_valid_from = p0;
    s->max_pos_written = best_sp;
    s->active_out_slot = 0;
    s->argmax_valid = 0;
    s->has_prev_argmax = 0;
    s->greedy_streak = 0;
    s->pos = best_sp;
    s->dspark.pos = ds_restored ? best_sp : 0;
    s->dspark_live_len = ds_restored ? best_sp : 0;
    s->dspark_swa_valid_from = ds_restored ? ds_p0 : 0;
    for (int i = 0; i < INK_MAX_SNAPS; i++) s->snaps[i].valid = 0;
    s->dspark.pending_got = 0;
    s->dspark.pending_nb = 0;
    s->dspark.skip_countdown = 0;
    s->dspark.req_disabled = 0;
    s->dspark.req_rounds = 0;
    s->dspark.skip_mult = 1;
    s->dspark.consec_unprof = 0;
    s->dspark.util_ema = 1.15f;
    s->dspark.ema_net_speedup = 1.15f;
    s->sampled_mode = 0;
    s->spec_round_open = 0;
    return best_sp;
}

void ink_prefix_save(InkSession *s, const int32_t *tokens, int n_tokens, int save_logits) {
    if (!s || !tokens || n_tokens <= 0 || n_tokens != s->pos || n_tokens > s->max_ctx) return;
    ink_cancel_inflight(s);
    int saved_spec_idx = s->spec_active_idx;
    if (s->conv_pending_n > 0 || (s->dspark.ready && s->dspark.pending_got > 0)) {
        PmBatch batch = NULL;
        if (pm_begin_concurrent(&batch) == PM_OK) {
            int ok = 1;
            if (s->conv_pending_n > 0 && ink_enqueue_conv_commit(s, batch) != 0) ok = 0;
            if (ok && s->dspark.ready && s->dspark.pending_got > 0 && ink_dspark_flush_pending(s, batch) != 0) ok = 0;
            if (ok) pm_wait(batch, NULL);
            else pm_abort(batch);
        }
        s->conv_pending_n = 0;
        s->dspark.pending_got = 0;
        s->dspark.pending_nb = 0;
    }
    const InkHParams *hp = &s->gguf.hp;
    size_t conv_elems = (size_t)s->conv_all_elems;
    int is_pinned = (save_logits == 2);
    int has_lg = (save_logits == 1);

    /* Choose slot: reuse matching pos, else free slot, else LRU non-pinned (or LRU pinned if is_pinned). */
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
        s->snaps[slot].conv_state = (float *)malloc(conv_elems * sizeof(float));
        s->snaps[slot].logits = (float *)malloc((size_t)hp->n_vocab * sizeof(float));
        if (!s->snaps[slot].tokens || !s->snaps[slot].conv_state || !s->snaps[slot].logits) {
            s->snaps[slot].valid = 0;
            return;
        }
    }
    if (s->dspark.ready) {
        if (!s->snaps[slot].dspark_last_taps) {
            s->snaps[slot].dspark_last_taps = (float *)malloc(4 * 2304 * sizeof(float));
        }
        if (s->snaps[slot].dspark_last_taps) {
            memcpy(s->snaps[slot].dspark_last_taps, pm_contents(s->dspark.last_taps), 4 * 2304 * sizeof(float));
            s->snaps[slot].dspark_pos = s->dspark.pos;
        }
    } else {
        s->snaps[slot].dspark_pos = 0;
    }
    memcpy(s->snaps[slot].tokens, tokens, (size_t)n_tokens * sizeof(int32_t));
    memcpy(s->snaps[slot].conv_state, pm_contents(s->conv_all), conv_elems * sizeof(float));
    if (has_lg) {
        int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
        if (saved_spec_idx >= 0) {
            const float *lsrc = (const float *)pm_contents(s->spec_logits) + (size_t)saved_spec_idx * (size_t)nv;
            memcpy(s->snaps[slot].logits, lsrc, (size_t)nv * sizeof(float));
        } else {
            memcpy(s->snaps[slot].logits, pm_contents(s->logits_buf[s->active_out_slot]), (size_t)hp->n_vocab * sizeof(float));
        }
    }
    /* Optional SWA KV window backup so pinned system/early checkpoints survive arbitrary SWA ring wraps. */
    s->snaps[slot].swa_win = 0;
    int n_swa_alloc = 0;
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        if (s->snaps[i].swa_kv) n_swa_alloc++;
    }
    int max_swa_snaps = (s->budget.headroom_mb >= 16384) ? 6 : INK_MAX_SWA_SNAPS;
    int want_swa = (s->budget.swa_slots > 0 && hp->swa > 0 && s->budget.headroom_mb >= 4096 &&
                    (is_pinned || (n_tokens <= s->max_ctx && (s->snaps[slot].swa_kv != NULL || n_swa_alloc < max_swa_snaps))));
    if (want_swa) {
        size_t kv_row = (size_t)s->kv_row_u16;
        size_t swa_elems = (size_t)s->budget.swa_slots * (size_t)hp->swa * kv_row * 2;
        if (!s->snaps[slot].swa_kv && n_swa_alloc < max_swa_snaps) {
            s->snaps[slot].swa_kv = (uint16_t *)malloc(swa_elems * sizeof(uint16_t));
        }
        if (s->snaps[slot].swa_kv) {
            int win = (n_tokens < (int)hp->swa) ? n_tokens : (int)hp->swa;
            int p0 = n_tokens - win;
            uint16_t *sd = s->snaps[slot].swa_kv;
            for (int il = 0; il < s->n_slots; il++) {
                int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
                if (!hp->is_swa[ip]) continue;
                int cap = s->kv_cap[il];
                const uint16_t *kk = (const uint16_t *)pm_contents(s->kv_k[il]);
                const uint16_t *kv = (const uint16_t *)pm_contents(s->kv_v[il]);
                int r0 = p0 % cap;
                int first = (r0 + win <= cap) ? win : (cap - r0);
                int second = win - first;
                memcpy(sd, kk + (size_t)r0 * kv_row, (size_t)first * kv_row * sizeof(uint16_t));
                if (second > 0) memcpy(sd + (size_t)first * kv_row, kk, (size_t)second * kv_row * sizeof(uint16_t));
                sd += (size_t)win * kv_row;
                memcpy(sd, kv + (size_t)r0 * kv_row, (size_t)first * kv_row * sizeof(uint16_t));
                if (second > 0) memcpy(sd + (size_t)first * kv_row, kv, (size_t)second * kv_row * sizeof(uint16_t));
                sd += (size_t)win * kv_row;
            }
            s->snaps[slot].swa_win = win;
        }
    }
    /* Back up DSpark's 2 SWA layers (0 & 2, <=8 MB) alongside main SWA backup so pinned/compaction
       restores keep DSpark speculative decoding warm across arbitrary SWA ring wraps. */
    s->snaps[slot].dspark_swa_win = 0;
    if (s->snaps[slot].swa_kv && s->dspark.ready && s->dspark.pos == n_tokens) {
        if (!s->snaps[slot].dspark_swa_kv) {
            s->snaps[slot].dspark_swa_kv = (uint16_t *)malloc((size_t)2 * 2 * 1024 * 1024 * sizeof(uint16_t));
        }
        if (s->snaps[slot].dspark_swa_kv) {
            int ds_win = (n_tokens < 1024) ? n_tokens : 1024;
            int ds_p0 = n_tokens - ds_win;
            uint16_t *sd = s->snaps[slot].dspark_swa_kv;
            for (int li = 0; li < 3; li++) {
                const DsparkLayer *L = &s->dspark.layers[li];
                if (!L->is_swa) continue;
                int cap = L->kv_cap;
                const uint16_t *kk = (const uint16_t *)pm_contents(L->kv_k);
                const uint16_t *kv = (const uint16_t *)pm_contents(L->kv_v);
                int r0 = ds_p0 % cap;
                int first = (r0 + ds_win <= cap) ? ds_win : (cap - r0);
                int second = ds_win - first;
                memcpy(sd, kk + (size_t)r0 * 1024, (size_t)first * 1024 * sizeof(uint16_t));
                if (second > 0) memcpy(sd + (size_t)first * 1024, kk, (size_t)second * 1024 * sizeof(uint16_t));
                sd += (size_t)ds_win * 1024;
                memcpy(sd, kv + (size_t)r0 * 1024, (size_t)first * 1024 * sizeof(uint16_t));
                if (second > 0) memcpy(sd + (size_t)first * 1024, kv, (size_t)second * 1024 * sizeof(uint16_t));
                sd += (size_t)ds_win * 1024;
            }
            s->snaps[slot].dspark_swa_win = ds_win;
        }
    }
    s->snaps[slot].pos = n_tokens;
    s->snaps[slot].has_logits = has_lg ? 1 : 0;
    s->snaps[slot].pinned = is_pinned ? 1 : 0;
    s->snaps[slot].last_used = ++s->snap_clock;
    s->snaps[slot].valid = 1;
    if (save_logits != 0 && (is_pinned || n_tokens >= 1024)) {
        ink_disk_cache_save(s, tokens, n_tokens, has_lg, has_lg ? s->snaps[slot].logits : NULL);
    }
}

int ink_prefix_restore(InkSession *s, const int32_t *tokens, int n_tokens) {
    if (!s || !tokens || n_tokens <= 0) { ink_reset(s); return 0; }
    ink_cancel_inflight(s);
    s->conv_pending_n = 0;
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;
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
        int oldest_needed = (sp >= hp->swa) ? (sp - (int)hp->swa + 1) : 0;
        int swa_still_live = (oldest_needed >= s->swa_valid_from);
        int need_win = (sp < (int)hp->swa) ? sp : (int)hp->swa;
        int has_swa_backup = (s->snaps[i].swa_kv != NULL && s->snaps[i].swa_win >= need_win);
        if (!swa_still_live && !has_swa_backup) continue;
        if (best < 0 || sp > s->snaps[best].pos) {
            best = i;
            best_need_swa = !swa_still_live;
        }
    }
    if (best < 0) {
        int disk_sp = ink_disk_cache_restore(s, tokens, n_tokens);
        if (disk_sp > 0) return disk_sp;
        ink_reset(s);
        return 0;
    }
    memcpy(pm_contents(s->conv_all), s->snaps[best].conv_state, (size_t)s->conv_all_elems * sizeof(float));
    int sp = s->snaps[best].pos;
    if (best_need_swa && s->snaps[best].swa_kv) {
        size_t kv_row = (size_t)s->kv_row_u16;
        int win = s->snaps[best].swa_win;
        int p0 = sp - win;
        const uint16_t *ss = s->snaps[best].swa_kv;
        for (int il = 0; il < s->n_slots; il++) {
            int ip = (il < hp->n_phys) ? il : hp->loop_start + (il - hp->n_phys) % (hp->loop_end - hp->loop_start);
            if (!hp->is_swa[ip]) continue;
            int cap = s->kv_cap[il];
            uint16_t *kk = (uint16_t *)pm_contents(s->kv_k[il]);
            uint16_t *kv = (uint16_t *)pm_contents(s->kv_v[il]);
            int r0 = p0 % cap;
            int first = (r0 + win <= cap) ? win : (cap - r0);
            int second = win - first;
            memcpy(kk + (size_t)r0 * kv_row, ss, (size_t)first * kv_row * sizeof(uint16_t));
            if (second > 0) memcpy(kk, ss + (size_t)first * kv_row, (size_t)second * kv_row * sizeof(uint16_t));
            ss += (size_t)win * kv_row;
            memcpy(kv + (size_t)r0 * kv_row, ss, (size_t)first * kv_row * sizeof(uint16_t));
            if (second > 0) memcpy(kv, ss + (size_t)first * kv_row, (size_t)second * kv_row * sizeof(uint16_t));
            ss += (size_t)win * kv_row;
        }
        s->swa_valid_from = p0;
        s->max_pos_written = sp;
    }
    if (sp == n_tokens && s->snaps[best].has_logits) {
        memcpy(pm_contents(s->logits_buf[0]), s->snaps[best].logits, (size_t)hp->n_vocab * sizeof(float));
    }
    if (s->tok_hist && sp <= s->max_ctx) {
        memcpy(s->tok_hist, tokens, (size_t)sp * sizeof(int32_t));
        s->tok_hist_len = sp;
    }
    s->active_out_slot = 0;
    s->argmax_valid = 0;
    s->has_prev_argmax = 0;
    s->greedy_streak = 0;
    s->pos = sp;
    s->live_len = sp;
    s->snaps[best].last_used = ++s->snap_clock;
    s->dspark.pending_got = 0;
    s->dspark.pending_nb = 0;
    s->dspark.skip_countdown = 0;
    s->dspark.req_disabled = 0;
    s->dspark.req_rounds = 0;
    s->dspark.skip_mult = 1;
    s->dspark.util_ema = 1.15f;
    s->dspark.consec_unprof = 0;
    s->dspark.ema_net_speedup = 1.15f;
    s->sampled_mode = 0;
    s->spec_round_open = 0;
    if (s->dspark.ready) {
        int ds_ok = (s->snaps[best].dspark_last_taps && s->snaps[best].dspark_pos == sp && sp <= s->dspark_live_len);
        int ds_oldest = (sp > 1024) ? (sp - 1024) : 0;
        int ds_need_win = (sp < 1024) ? sp : 1024;
        int ds_swa_live = (ds_oldest >= s->dspark_swa_valid_from);
        int ds_has_backup = (s->snaps[best].dspark_swa_kv != NULL && s->snaps[best].dspark_swa_win >= ds_need_win);
        if (ds_ok && (ds_swa_live || ds_has_backup)) {
            if (!ds_swa_live) {
                int ds_win = s->snaps[best].dspark_swa_win;
                int ds_p0 = sp - ds_win;
                const uint16_t *ss = s->snaps[best].dspark_swa_kv;
                for (int li = 0; li < 3; li++) {
                    DsparkLayer *L = &s->dspark.layers[li];
                    if (!L->is_swa) continue;
                    int cap = L->kv_cap;
                    uint16_t *kk = (uint16_t *)pm_contents(L->kv_k);
                    uint16_t *kv = (uint16_t *)pm_contents(L->kv_v);
                    int r0 = ds_p0 % cap;
                    int first = (r0 + ds_win <= cap) ? ds_win : (cap - r0);
                    int second = ds_win - first;
                    memcpy(kk + (size_t)r0 * 1024, ss, (size_t)first * 1024 * sizeof(uint16_t));
                    if (second > 0) memcpy(kk, ss + (size_t)first * 1024, (size_t)second * 1024 * sizeof(uint16_t));
                    ss += (size_t)ds_win * 1024;
                    memcpy(kv + (size_t)r0 * 1024, ss, (size_t)first * 1024 * sizeof(uint16_t));
                    if (second > 0) memcpy(kv, ss + (size_t)first * 1024, (size_t)second * 1024 * sizeof(uint16_t));
                    ss += (size_t)ds_win * 1024;
                }
                s->dspark_swa_valid_from = ds_p0;
            }
            memcpy(pm_contents(s->dspark.last_taps), s->snaps[best].dspark_last_taps, 4 * 2304 * sizeof(float));
            s->dspark.pos = sp;
            s->dspark_live_len = sp;
        } else {
            s->dspark.pos = 0;
            s->dspark_live_len = 0;
        }
    }
    for (int i = 0; i < INK_MAX_SNAPS; i++) {
        if (i != best && s->snaps[i].valid && s->snaps[i].pos > sp) s->snaps[i].valid = 0;
    }
    return sp;
}

static int enqueue_step(InkSession *s, PmBatch *bp, int32_t token_id, PmBuffer indirect_tok_buf,
                        int save_conv_backup, int out_slot, int compute_logits, int advance_state) {
    PmBatch batch = *bp;
    const InkHParams *hp = &s->gguf.hp;
    int do_prof = (advance_state && getenv("INK_PROFILE") != NULL && s->total_steps == 35);
    if (do_prof) { prof_active = 1; memset(prof_ms, 0, sizeof prof_ms); }
    if (s->pos >= s->max_ctx) {
        snprintf(s->error, sizeof s->error, "context full (%d/%d)", s->pos, s->max_ctx);
        return 1;
    }
    if (!indirect_tok_buf && (token_id < 0 || token_id >= hp->n_vocab)) {
        snprintf(s->error, sizeof s->error, "token id %d out of range [0,%lld)", token_id, (long long)hp->n_vocab);
        return 1;
    }

    /* tau = 1 + alpha * log(max(1, (pos + 1) / floor)) */
    float ratio = (float)(s->pos + 1) / (float)hp->log_scale_floor;
    if (ratio < 1.0f) ratio = 1.0f;
    float tau = 1.0f + hp->log_scale_alpha * logf(ratio);

    /* 0. Lazy commit of any pending verified speculative conv raw log + DSpark taps (0 extra submits!) */
    if (advance_state && s->conv_pending_n > 0) {
        if (ink_enqueue_conv_commit(s, batch) != 0) return 1;
    }
    int cap_taps = (advance_state && s->dspark.ready && !s->dspark.req_disabled && getenv("INK_NO_SPEC") == NULL &&
                    s->dspark.pos + s->dspark.pending_got == s->pos);
    const int64_t tap_stride = 64;
    if (cap_taps && s->dspark.pending_got > 0 &&
        (s->dspark.pending_nb != (int)tap_stride || s->dspark.pending_got >= (int)tap_stride)) {
        if (ink_dspark_flush_pending(s, batch) != 0) return 1;
    }
    int64_t tap_slot = (cap_taps && s->dspark.pending_got > 0) ? (int64_t)s->dspark.pending_got : 0;

    int64_t d = hp->n_embd;
    /* 1. Embedding lookup + token_embd_norm (+ concurrent conv_all backup when GPU-chained speculatively) */
    if (indirect_tok_buf) {
        if (save_conv_backup && s->conv_all_elems > 0) {
            CALL(s, batch, K_COPY, (s->conv_all_elems + INK_NT - 1) / INK_NT, a_int(s->conv_all_elems),
                 a_f32v(s->conv_all, 0, s->conv_all_elems),
                 a_f32v(s->conv_backup, 0, s->conv_all_elems));
        }
        if (s->gguf.tok_embd.type == INK_GGML_Q8_0 && !s->ablate_has_emb) {
            CALL(s, batch, K_EMBED_Q8_INDIRECT, 1, a_int(d), a_int(hp->n_vocab),
                 a_u32v(indirect_tok_buf, 0, 1),
                 a_tensor_u16(s, &s->gguf.tok_embd),
                 a_tensor_f32(s, &s->gguf.tok_norm),
                 a_f32(hp->rms_eps),
                 a_f32v(s->cur, 0, d));
        } else {
            CALL(s, batch, K_EMBED_GGML_INDIRECT, 1, a_int(d), a_int(hp->n_vocab), a_int((int64_t)s->gguf.tok_embd.type),
                 a_u32v(indirect_tok_buf, 0, 1),
                 a_tensor_ggml_u16(s, &s->gguf.tok_embd), a_ggml_grids(s),
                 a_tensor_f32(s, &s->gguf.tok_norm), a_f32(hp->rms_eps),
                 a_int(s->ablate_has_emb ? 1 : 0), a_ablate_dirs(s),
                 a_f32v(s->cur, 0, d));
        }
    } else {
        if (s->gguf.tok_embd.type == INK_GGML_Q8_0 && !s->ablate_has_emb) {
            CALL(s, batch, K_EMBED_Q8, 1, a_int(token_id), a_int(d),
                 a_tensor_u16(s, &s->gguf.tok_embd),
                 a_tensor_f32(s, &s->gguf.tok_norm),
                 a_f32(hp->rms_eps),
                 a_f32v(s->cur, 0, d));
        } else {
            CALL(s, batch, K_EMBED_GGML, 1, a_int(token_id), a_int(d), a_int((int64_t)s->gguf.tok_embd.type),
                 a_tensor_ggml_u16(s, &s->gguf.tok_embd), a_ggml_grids(s),
                 a_tensor_f32(s, &s->gguf.tok_norm), a_f32(hp->rms_eps),
                 a_int(s->ablate_has_emb ? 1 : 0), a_ablate_dirs(s),
                 a_f32v(s->cur, 0, d));
        }
    }
    pm_barrier(batch);

    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;

    /* 2. Prelude layers [0 .. start) */
    for (int il = 0; il < start; il++) {
        if (cap_taps && il == 5) {
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d),
                 a_f32v(s->cur, 0, d), a_f32v(s->dspark.taps, (0 * tap_stride + tap_slot) * d, d));
            pm_barrier(batch);
        }
        if (dispatch_layer(s, bp, il, il, &n_blocks, 0, -1, tau) != 0) return 1;
        batch = *bp;
    }

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
            if (cap_taps && pass == 1 && ip == 11) {
                CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d),
                     a_f32v(s->cur, 0, d), a_f32v(s->dspark.taps, (1 * tap_stride + tap_slot) * d, d));
                pm_barrier(batch);
            } else if (cap_taps && pass == 2 && ip == 17) {
                CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d),
                     a_f32v(s->cur, 0, d), a_f32v(s->dspark.taps, (2 * tap_stride + tap_slot) * d, d));
                pm_barrier(batch);
            }
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
        int os = out_slot & 1;
        PmBuffer h_dst = s->hidden_out[os];
        PmBuffer l_dst = s->logits_buf[os];
        PmBuffer a_dst = s->argmax_out_buf[os];
        prof_mark(bp, 9); batch = *bp;
        CALL(s, batch, K_MIX_RMSNORM, 1, a_int(d), a_int(n_blocks), a_f32(hp->rms_eps),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
             a_f32v(s->cur, 0, d),
             a_tensor_f32(s, &s->gguf.out_res_query),
             a_tensor_f32(s, &s->gguf.out_norm),
             a_f32v(h_dst, 0, d));
        pm_barrier(batch);
        if (cap_taps) {
            CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d),
                 a_f32v(h_dst, 0, d), a_f32v(s->dspark.taps, (3 * tap_stride + tap_slot) * d, d));
        }
        int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
        if (s->can_verify_nb && getenv("INK_NO_MMA32") == NULL && s->fast_buf && s->fast_output_s.qs.off >= 0 && s->fast_output_is_q4s) {
            CALL(s, batch, K_DV_Q4S_MMA32_NB, (nv + 31) / 32, a_int(nv), a_int(d), a_int(1),
                 a_int(0), a_int(0), a_int(0), a_f32(hp->logit_softcap),
                 a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
                 a_f32v(h_dst, 0, d), a_f32v(l_dst, 0, nv));
        } else if (s->can_verify_nb && getenv("INK_NO_MMA32") == NULL && s->fast_output_s.qs.off < 0 && s->gguf.output.type == INK_GGML_Q8_0) {
            CALL(s, batch, K_DV_Q8_MMA32_NB, (nv + 31) / 32, a_int(nv), a_int(d), a_int(1),
                 a_int(0), a_int(0), a_int(0), a_f32(hp->logit_softcap),
                 a_tensor_u16(s, &s->gguf.output),
                 a_f32v(h_dst, 0, d), a_f32v(l_dst, 0, nv));
        } else if (s->fast_buf && s->fast_output_s.qs.off >= 0) {
            CALL(s, batch, s->fast_output_is_q4s ? K_DV_Q4S_RMS : K_DV_Q8S_RMS, (nv + 7) / 8, a_int(nv), a_int(d),
                 a_int(0), a_int(0), a_f32(hp->rms_eps), a_f32(hp->logit_softcap),
                 a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
                 a_f32v(h_dst, 0, d), a_f32v(s->rms_parts, 0, 3 * 64), a_f32v(l_dst, 0, nv));
        } else if (s->fast_buf && s->fast_output.off >= 0 && hp->logit_softcap > 0.0f) {
            CALL(s, batch, K_DV_Q4_RMS, (nv + 7) / 8, a_int(nv), a_int(d),
                 a_int(0), a_int(0), a_f32(hp->rms_eps), a_f32(hp->logit_softcap),
                 a_fast_u16(s, &s->fast_output), a_f32v(h_dst, 0, d),
                 a_f32v(s->rms_parts, 0, 3 * 64), a_f32v(l_dst, 0, nv));
        } else {
            if (dispatch_mv_q4(s, batch, &s->gguf.output, &s->fast_output, nv, d, 0, h_dst, 0, l_dst, 0) != 0) return 1;
            if (hp->logit_softcap > 0.0f) {
                pm_barrier(batch);
                CALL(s, batch, K_SOFTCAP, (nv + INK_NT - 1) / INK_NT, a_int(nv),
                     a_f32(hp->logit_softcap), a_f32v(l_dst, 0, nv));
            }
        }
        pm_barrier(batch);
        CALL(s, batch, K_TOPK64_S1, 32, a_int(32), a_int(nv),
             a_f32v(l_dst, 0, nv), a_f32v(s->topk_s1_vals, 0, 32 * 64), a_u32v(s->topk_s1_ids, 0, 32 * 64));
        pm_barrier(batch);
        CALL(s, batch, K_TOPK64_S2, 1, a_int(32 * 64),
             a_f32v(s->topk_s1_vals, 0, 32 * 64), a_u32v(s->topk_s1_ids, 0, 32 * 64),
             a_f32v(s->topk_vals[os], 0, 64), a_u32v(s->topk_ids[os], 0, 64), a_u32v(a_dst, 0, 1));
        if (advance_state) {
            s->active_out_slot = os;
            s->hidden_is_prefill = 0;
            s->argmax_valid = 1;
            s->last_hidden_off = 0;
        }
        prof_mark(bp, 10); batch = *bp;
    } else {
        if (cap_taps) {
            CALL(s, batch, K_MIX_RMSNORM, 1, a_int(d), a_int(n_blocks), a_f32(hp->rms_eps),
                 a_f32v(s->blocks, 0, INK_MAX_BLOCKS * d),
                 a_f32v(s->cur, 0, d),
                 a_tensor_f32(s, &s->gguf.out_res_query),
                 a_tensor_f32(s, &s->gguf.out_norm),
                 a_f32v(s->dspark.taps, (3 * tap_stride + tap_slot) * d, d));
            pm_barrier(batch);
        }
        if (advance_state) s->argmax_valid = 0;
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

    if (advance_state) {
        if (cap_taps && s->dspark.pos + s->dspark.pending_got == s->pos) {
            if (s->dspark.pending_got == 0) {
                s->dspark.pending_pos0 = s->pos;
                s->dspark.pending_nb = (int)tap_stride;
                s->dspark.pending_got = 1;
            } else {
                s->dspark.pending_got++;
            }
        }
        if (s->live_tokens && s->pos < s->max_ctx && !indirect_tok_buf) {
            s->live_tokens[s->pos] = token_id;
            s->live_len = s->pos + 1;
        }
        s->pos++;
        ink_mark_written(s, s->pos);
        s->total_steps++;
    }
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
    } else if (ink_ggml_type_supported(w->type)) {
        CALL(s, batch, K_MM_GGML, nb * ((rows + 3) / 4), a_int(rows), a_int(cols), a_int(nb), a_int((int64_t)w->type),
             a_tensor_ggml_u16(s, w), a_ggml_grids(s),
             a_f32v(x_buf, x_off, nb * cols), a_f32v(out_buf, out_off, nb * rows));
    } else {
        snprintf(s->error, sizeof s->error, "unsupported weight type %u (%s) for batched matmul", w->type, ink_ggml_type_name(w->type));
        return 1;
    }
    return 0;
}

static int dispatch_dequant_q8_bf16(InkSession *s, PmBatch batch, const InkTensor *w, int64_t rows, int64_t cols, int64_t bf16_off) {
    if (w->type == INK_GGML_Q8_0) {
        CALL(s, batch, K_DEQUANT_Q8_BF16, (rows + 3) / 4, a_int(rows), a_int(cols),
             a_tensor_u16(s, w), a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)));
        return 0;
    }
    if (ink_ggml_type_supported(w->type)) {
        CALL(s, batch, K_DEQUANT_GGML_BF16, (rows + 3) / 4, a_int(rows), a_int(cols), a_int((int64_t)w->type),
             a_tensor_ggml_u16(s, w), a_ggml_grids(s), a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)));
        return 0;
    }
    snprintf(s->error, sizeof s->error, "unsupported weight type %u (%s) for dequant_bf16", w->type, ink_ggml_type_name(w->type));
    return 1;
}

static int dispatch_dequant_q4_or_q8_bf16(InkSession *s, PmBatch batch, const InkTensor *w, const FastSpan *fs, const FastQ4S *q4s, int64_t rows, int64_t cols, int64_t bf16_off) {
    if (!s->fast_is_q8s && s->fast_buf && q4s && q4s->qs.off >= 0) {
        CALL(s, batch, K_DEQUANT_Q4S_BF16, (rows + 7) / 8, a_int(rows), a_int(cols),
             a_fast_u32(s, &q4s->qs), a_fast_u16(s, &q4s->ds), a_u32v(s->w_bf16, bf16_off, rows * (cols / 2)));
        return 0;
    }
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

static int dispatch_mm_q8s_bf16(InkSession *s, PmBatch batch, int64_t rows, int64_t cols, int64_t nb,
                                const FastQ4S *q8s, PmBuffer x_bf16, int64_t x_off, PmBuffer out_buf, int64_t out_off) {
    int64_t r_tiles = (rows + 63) / 64;
    int k = (nb >= 128) ? K_MM_Q8S_BF16_128 : (nb >= 33) ? K_MM_Q8S_BF16_64 : K_MM_Q8S_BF16_16;
    int64_t tm = (nb >= 128) ? 128 : (nb >= 33) ? 64 : 16;
    int64_t t_tiles = (nb + tm - 1) / tm;
    CALL(s, batch, k, t_tiles * r_tiles, a_int(rows), a_int(cols), a_int(nb),
         a_fast_u32(s, &q8s->qs), a_fast_u16(s, &q8s->ds),
         a_u16v(x_bf16, x_off, nb * cols),
         a_f32v(out_buf, out_off, nb * rows));
    return 0;
}

static int dispatch_layer_batch(InkSession *s, PmBatch *bp, int il, int ip, int *n_blocks, int has_messages, int64_t nb,
                                int save_spec_hist, int force_fp32) {
    PmBatch batch = *bp;
    const InkHParams *hp = &s->gguf.hp;
    const InkLayerTensors *L = &s->gguf.layers[ip];
    const FastLayer *FL = &s->fast_layers[ip];
    int64_t d = hp->n_embd;
    int64_t n_head = hp->n_head;
    int64_t n_head_kv = hp->n_head_kv;
    int64_t hd = hp->head_dim;
    int64_t n_q = n_head * hd;
    int64_t n_kv = n_head_kv * hd;
    int64_t n_rel = hp->rel_len;
    int is_swa = hp->is_swa[ip];
    int64_t bd = nb * d;
    int64_t spec_slot_base = (int64_t)il * (int64_t)INK_SPEC_MAX_NB * (2 * n_kv + 2 * d);
    int64_t m4_min = force_fp32 ? 1000000 : 8;
    {
        const char *env_m4 = getenv("INK_M4_BATCH_MIN");
        if (!force_fp32 && env_m4 && env_m4[0]) m4_min = strtoll(env_m4, NULL, 10);
    }

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
        if (s->fast_buf && FL->ctrl_gf_qkv.off >= 0 && rank <= 64) {
            int64_t citems = ((nb + 15) / 16) * ((d + INK_T - 1) / INK_T);
            CALL(s, batch, K_CTRL_UP_APPLY_GF_BATCH, citems, a_int(d), a_int(rank), a_int(nb), a_int(0 * rank * d),
                 a_fast_u32(s, &FL->ctrl_gf_qkv),
                 a_f32v(s->ctrl_gate, 0 * nb * rank, nb * rank), a_f32v(q_src, q_off, bd), a_f32v(s->messages, 0 * bd, bd), a_f32v(s->inp_q, 0, bd));
            CALL(s, batch, K_CTRL_UP_APPLY_GF_BATCH, citems, a_int(d), a_int(rank), a_int(nb), a_int(1 * rank * d),
                 a_fast_u32(s, &FL->ctrl_gf_qkv),
                 a_f32v(s->ctrl_gate, 1 * nb * rank, nb * rank), a_f32v(k_src, k_off, bd), a_f32v(s->messages, 1 * bd, bd), a_f32v(s->inp_k, 0, bd));
            CALL(s, batch, K_CTRL_UP_APPLY_GF_BATCH, citems, a_int(d), a_int(rank), a_int(nb), a_int(2 * rank * d),
                 a_fast_u32(s, &FL->ctrl_gf_qkv),
                 a_f32v(s->ctrl_gate, 2 * nb * rank, nb * rank), a_f32v(v_src, v_off, bd), a_f32v(s->messages, 2 * bd, bd), a_f32v(s->inp_v, 0, bd));
        } else {
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
        }
        pm_barrier(batch);
        q_src = s->inp_q; q_off = 0;
        k_src = s->inp_k; k_off = 0;
        v_src = s->inp_v; v_off = 0;
    }

    /* Wave 2: Input RMSNorms (ink_rmsnorm already supports nb rows!). */
    /* Wave 2: Input RMSNorms + overlapped Q8/F32 -> BF16 weight dequantization (-48 barriers/prefill!). */
    int has_k_dyn = (!is_swa && L->attn_k_conv_proj.present);
    int has_v_dyn = (!is_swa && L->attn_v_conv_proj.present);
    int qkv_q8s = (!force_fp32 && s->fast_buf && s->fast_is_q8s &&
                   FL->attn_q_s.qs.off >= 0 && FL->attn_k_s.qs.off >= 0 && FL->attn_v_s.qs.off >= 0 &&
                   FL->attn_rel_s.qs.off >= 0 && FL->attn_gate_s.qs.off >= 0);
    int qkv_bf16 = (!qkv_q8s && nb >= m4_min &&
                    ink_ggml_type_supported(L->attn_q.type) && ink_ggml_type_supported(L->attn_k.type) &&
                    ink_ggml_type_supported(L->attn_v.type) && ink_ggml_type_supported(L->attn_rel.type) &&
                    ink_ggml_type_supported(L->attn_gate.type));
    int64_t off_q = 0;
    int64_t off_k = off_q + n_q * (d / 2);
    int64_t off_v = off_k + n_kv * (d / 2);
    int64_t off_r = off_v + n_kv * (d / 2);
    int64_t off_g = off_r + (n_head * n_rel) * (d / 2);
    int64_t off_s = qkv_q8s ? 0 : (off_g + n_q * (d / 2));
    int64_t off_kd = off_s + n_head * (d / 2);
    int sink_bf16 = ((qkv_q8s || qkv_bf16) && nb >= 64 && L->attn_sink_proj.type == INK_GGML_F32);
    int kdyn_bf16 = ((qkv_q8s || qkv_bf16) && has_k_dyn && nb >= 64 && L->attn_k_conv_proj.type == INK_GGML_F32);

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
    if (qkv_bf16) {
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_q, &FL->attn_q, &FL->attn_q_s, n_q, d, off_q)) return 1;
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_k, &FL->attn_k, &FL->attn_k_s, n_kv, d, off_k)) return 1;
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_v, &FL->attn_v, &FL->attn_v_s, n_kv, d, off_v)) return 1;
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_rel, &FL->attn_rel, &FL->attn_rel_s, n_head * n_rel, d, off_r)) return 1;
        if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_gate, &FL->attn_gate, &FL->attn_gate_s, n_q, d, off_g)) return 1;
    }
    if (sink_bf16) {
        if (dispatch_dequant_f32_bf16(s, batch, &L->attn_sink_proj, n_head, d, off_s)) return 1;
    }
    if (kdyn_bf16) {
        if (dispatch_dequant_f32_bf16(s, batch, &L->attn_k_conv_proj, n_head_kv * hp->conv_len, d, off_kd)) return 1;
    }
    pm_barrier(batch);
    prof_mark(bp, 0); batch = *bp; /* 0: mix+ctrl+norm+qkv_deq */

    /* Wave 3: ALL independent attention projections concurrently — Metal 4 Neural Accelerators (in-tile Q8S or BF16)! */
    if (qkv_q8s || qkv_bf16) {
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
        if (qkv_q8s) {
            if (dispatch_mm_q8s_bf16(s, batch, n_q, d, nb, &FL->attn_q_s, s->moe_slot_out, qx_off, s->q_buf, 0)) return 1;
            if (dispatch_mm_q8s_bf16(s, batch, n_kv, d, nb, &FL->attn_k_s, s->moe_slot_out, kx_off, s->k_raw, 0)) return 1;
            if (dispatch_mm_q8s_bf16(s, batch, n_kv, d, nb, &FL->attn_v_s, s->moe_slot_out, vx_off, s->v_raw, 0)) return 1;
            if (dispatch_mm_q8s_bf16(s, batch, n_head * n_rel, d, nb, &FL->attn_rel_s, s->moe_slot_out, qx_off, s->r_buf, 0)) return 1;
            if (dispatch_mm_q8s_bf16(s, batch, n_q, d, nb, &FL->attn_gate_s, s->moe_slot_out, qx_off, s->gate_buf, 0)) return 1;
        } else {
            if (dispatch_mm_bf16(s, batch, n_q, d, nb, off_q, s->moe_slot_out, qx_off, s->q_buf, 0)) return 1;
            if (dispatch_mm_bf16(s, batch, n_kv, d, nb, off_k, s->moe_slot_out, kx_off, s->k_raw, 0)) return 1;
            if (dispatch_mm_bf16(s, batch, n_kv, d, nb, off_v, s->moe_slot_out, vx_off, s->v_raw, 0)) return 1;
            if (dispatch_mm_bf16(s, batch, n_head * n_rel, d, nb, off_r, s->moe_slot_out, qx_off, s->r_buf, 0)) return 1;
            if (dispatch_mm_bf16(s, batch, n_q, d, nb, off_g, s->moe_slot_out, qx_off, s->gate_buf, 0)) return 1;
        }
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
    if (save_spec_hist) {
        int64_t bkv = nb * n_kv;
        CALL(s, batch, K_COPY, (bkv + INK_NT - 1) / INK_NT, a_int(bkv),
             a_f32v(s->k_raw, 0, bkv), a_f32v(s->spec_hist, spec_slot_base, bkv));
        CALL(s, batch, K_COPY, (bkv + INK_NT - 1) / INK_NT, a_int(bkv),
             a_f32v(s->v_raw, 0, bkv), a_f32v(s->spec_hist, spec_slot_base + (int64_t)INK_SPEC_MAX_NB * n_kv, bkv));
    }
    CALL(s, batch, K_CONVOLVE_BATCH, conv_kv_items, a_int(n_kv), a_int(n_head_kv), a_int(has_k_dyn ? 1 : 0), a_int(nb), a_int(s->pos),
         a_f32v(s->k_raw, 0, nb * n_kv), a_tensor_f32(s, &L->attn_k_conv),
         has_k_dyn ? a_f32v(s->dyn_buf, 0, nb * n_head_kv * hp->conv_len) : a_f32v(s->dummy_f32, 0, 64),
         a_conv_k(s, il), a_f32v(s->k_buf, 0, nb * n_kv));
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
        CALL(s, batch, K_CONVOLVE_BATCH, conv_kv_items, a_int(n_kv), a_int(n_head_kv), a_int(0), a_int(nb), a_int(s->pos),
             a_f32v(s->v_raw, 0, nb * n_kv), a_tensor_f32(s, &L->attn_v_conv),
             a_f32v(s->dummy_f32, 0, 64),
             a_conv_v(s, il), a_f32v(s->v_buf, 0, nb * n_kv));
        pm_barrier(batch);
    } else {
        pm_barrier(batch);
        if (dispatch_mm(s, batch, &L->attn_v_conv_proj, n_head_kv * hp->conv_len, d, nb,
                        v_in, 0, s->dyn_buf, 0)) return 1;
        pm_barrier(batch);
        CALL(s, batch, K_CONVOLVE_BATCH, conv_kv_items, a_int(n_kv), a_int(n_head_kv), a_int(1), a_int(nb), a_int(s->pos),
             a_f32v(s->v_raw, 0, nb * n_kv), a_tensor_f32(s, &L->attn_v_conv),
             a_f32v(s->dyn_buf, 0, nb * n_head_kv * hp->conv_len),
             a_conv_v(s, il), a_f32v(s->v_buf, 0, nb * n_kv));
        pm_barrier(batch);
    }

    /* Wave 5: K clip + V head RMSNorm concurrently. */
    CALL(s, batch, K_CLIP_BATCH, nb * n_head_kv, a_int(hd), a_int(n_head_kv), a_int(0),
         a_f32(hp->qk_clip), a_int(0), a_f32(1.0f),
         a_f32v(s->k_buf, 0, nb * n_kv), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->dummy_f32, 0, 64));
    CALL(s, batch, K_HEAD_RMSNORM, nb * n_head_kv, a_int(hd), a_f32(hp->rms_eps),
         a_f32v(s->v_buf, 0, nb * n_kv));
    pm_barrier(batch);

    /* Store K, V for all nb tokens into KV cache (Q8_0 block-32 or FP16) */
    int64_t kv_cap = s->kv_cap[il];
    if (s->kv_q8) {
        CALL(s, batch, K_KV_STORE_BATCH_Q8, nb * n_head_kv, a_int(n_head_kv), a_int(s->pos), a_int(kv_cap), a_int(is_swa),
             a_f32v(s->k_buf, 0, nb * n_kv), a_f32v(s->v_buf, 0, nb * n_kv),
             a_u16v(s->kv_k[il], 0, kv_cap * s->kv_row_u16), a_u16v(s->kv_v[il], 0, kv_cap * s->kv_row_u16));
    } else {
        CALL(s, batch, K_KV_STORE_BATCH, nb, a_int(n_kv), a_int(s->pos), a_int(kv_cap), a_int(is_swa),
             a_f32v(s->k_buf, 0, nb * n_kv), a_f32v(s->v_buf, 0, nb * n_kv),
             a_u16v(s->kv_k[il], 0, kv_cap * s->kv_row_u16), a_u16v(s->kv_v[il], 0, kv_cap * s->kv_row_u16));
    }
    pm_barrier(batch);

    /* 1-Pass Metal 4 FlashAttention (zero score DRAM buffer, 96 fewer barriers/chunk!) */
    prof_mark(bp, 2); batch = *bp; /* 2: conv+clip+hnorm+kv */
    uint32_t dims0 = (uint32_t)n_head | ((uint32_t)n_head_kv << 8) | ((uint32_t)hd << 16) | ((uint32_t)n_rel << 24);
    uint32_t dims1 = (uint32_t)hp->rel_extent | ((uint32_t)(is_swa ? 1 : 0) << 16) | ((uint32_t)hp->swa << 17) | ((uint32_t)(s->kv_q8 ? 1 : 0) << 31);
    int64_t k_start = (is_swa && s->pos >= hp->swa) ? (s->pos - hp->swa + 1) : 0;
    int64_t span = (s->pos + nb) - k_start;
    int64_t sc_stride = span;
    uint32_t kv_info = (uint32_t)kv_cap | ((uint32_t)sc_stride << 16);
    int64_t q_tiles = (nb + 63) / 64;
    int o_q8s = (!force_fp32 && s->fast_buf && s->fast_is_q8s && FL->attn_output_s.qs.off >= 0);
    int o_bf16 = (!o_q8s && nb >= m4_min && ink_ggml_type_supported(L->attn_output.type));
    if (hd == 128 && n_rel == 16 && !getenv("INK_OLD_ATTN")) {
        int64_t fq_tiles = (nb + 31) / 32;
        CALL(s, batch, K_FLASH_ATTN_M4, n_head * fq_tiles,
             a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos), a_int(nb), a_int(k_start),
             a_f32v(s->q_buf, 0, nb * n_q),
             a_f32v(s->r_buf, 0, nb * n_head * n_rel),
             a_tensor_f32(s, &L->attn_rel_bias),
             a_f32v(s->sinks_buf, 0, nb * n_head),
             a_u16v(s->kv_k[il], 0, kv_cap * s->kv_row_u16),
             a_u16v(s->kv_v[il], 0, kv_cap * s->kv_row_u16),
             a_f32v(s->gate_buf, 0, nb * n_q),
             a_f32v(s->attn_out, 0, nb * n_q));
        if (o_bf16) {
            if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_output, &FL->attn_output, &FL->attn_output_s, d, n_q, 0)) return 1;
        }
        pm_barrier(batch);
        prof_mark(bp, 3); batch = *bp; /* 3: flash_attn_m4 */
    } else {
        CALL(s, batch, (hd == 128 && n_rel == 16) ? K_ATTN_SCORES_M4 : K_ATTN_SCORES_MMA, n_head * q_tiles * 4,
             a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos), a_int(nb), a_int(k_start),
             a_f32v(s->q_buf, 0, nb * n_q),
             a_f32v(s->r_buf, 0, nb * n_head * n_rel),
             a_tensor_f32(s, &L->attn_rel_bias),
             a_f32v(s->sinks_buf, 0, nb * n_head),
             a_u16v(s->kv_k[il], 0, kv_cap * s->kv_row_u16),
             a_u16v(s->attn_scores, 0, nb * n_head * sc_stride));
        pm_barrier(batch);
        prof_mark(bp, 3); batch = *bp; /* 3: attn_scores_m4 */
        CALL(s, batch, K_ATTN_SOFTMAX_MMA, nb * n_head,
             a_u32(dims0), a_u32(dims1), a_int(sc_stride), a_int(s->pos), a_int(nb), a_int(k_start),
             a_u16v(s->attn_scores, 0, nb * n_head * sc_stride),
             a_f32v(s->sinks_buf, 0, nb * n_head));
        pm_barrier(batch);
        prof_mark(bp, 4); batch = *bp; /* 4: attn_softmax_mma */
        if (hd == 128) {
            CALL(s, batch, K_ATTN_VALUES_M4, n_head * q_tiles * 2,
                 a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos), a_int(nb), a_int(k_start),
                 a_u16v(s->attn_scores, 0, nb * n_head * sc_stride),
                 a_f32v(s->sinks_buf, 0, nb * n_head),
                 a_u16v(s->kv_v[il], 0, kv_cap * s->kv_row_u16),
                 a_f32v(s->gate_buf, 0, nb * n_q),
                 a_f32v(s->attn_out, 0, nb * n_q));
        } else {
            CALL(s, batch, K_ATTN_VALUES_MMA, n_head * q_tiles * 4,
                 a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos), a_int(nb), a_int(k_start),
                 a_u16v(s->attn_scores, 0, nb * n_head * sc_stride),
                 a_f32v(s->sinks_buf, 0, nb * n_head),
                 a_u16v(s->kv_v[il], 0, kv_cap * s->kv_row_u16),
                 a_f32v(s->gate_buf, 0, nb * n_q),
                 a_f32v(s->attn_out, 0, nb * n_q));
        }
        if (o_bf16) {
            /* Overlapped with K_ATTN_VALUES_MMA (-48 barriers/prefill!). */
            if (dispatch_dequant_q4_or_q8_bf16(s, batch, &L->attn_output, &FL->attn_output, &FL->attn_output_s, d, n_q, 0)) return 1;
        }
        pm_barrier(batch);
        prof_mark(bp, 5); batch = *bp; /* 5: attn_values_mma+o_deq */
    }

    /* Output projection + post shortconv + residual (fused when non-block-boundary!) */
    if (o_q8s || o_bf16) {
        if (dispatch_f32_to_bf16(s, batch, nb * n_q, s->attn_out, 0, 0)) return 1;
        pm_barrier(batch);
        if (o_q8s) {
            if (dispatch_mm_q8s_bf16(s, batch, d, n_q, nb, &FL->attn_output_s, s->moe_slot_out, 0, s->o_raw, 0)) return 1;
        } else {
            if (dispatch_mm_bf16(s, batch, d, n_q, nb, 0, s->moe_slot_out, 0, s->o_raw, 0)) return 1;
        }
    } else {
        if (dispatch_mm(s, batch, &L->attn_output, d, n_q, nb, s->attn_out, 0, s->o_raw, 0)) return 1;
    }
    pm_barrier(batch);
    if (s->has_ablate && ip < 32 && s->ablate_has_layer[ip]) {
        CALL(s, batch, K_ABLATE_DIR, nb, a_int(d), a_int((int64_t)(1 + ip) * d),
             a_ablate_dirs(s), a_f32v(s->o_raw, 0, bd));
        pm_barrier(batch);
    }
    int64_t conv_d_items = ((nb + 31) / 32) * ((d + 63) / 64);
    if (save_spec_hist) {
        CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
             a_f32v(s->o_raw, 0, bd), a_f32v(s->spec_hist, spec_slot_base + (int64_t)INK_SPEC_MAX_NB * (2 * n_kv), bd));
    }
    if (ip % hp->block_size == 0) {
        if (*n_blocks >= INK_MAX_BLOCKS) {
            snprintf(s->error, sizeof s->error, "block count overflow");
            return 1;
        }
        CALL(s, batch, K_CONVOLVE_BATCH, conv_d_items, a_int(d), a_int(1), a_int(0), a_int(nb), a_int(-1),
             a_f32v(s->o_raw, 0, bd), a_tensor_f32(s, &L->attn_post_conv),
             a_f32v(s->dummy_f32, 0, 64),
             a_conv_ao(s, il), a_f32v(s->cur, 0, bd));
        CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->inp_sa, 0, bd),
             a_f32v(s->blocks, (int64_t)(*n_blocks) * bd, bd));
        (*n_blocks)++;
    } else {
        CALL(s, batch, K_CONVOLVE_ADD_BATCH, conv_d_items, a_int(d), a_int(nb),
             a_f32v(s->o_raw, 0, bd), a_tensor_f32(s, &L->attn_post_conv),
             a_conv_ao(s, il), a_f32v(s->inp_sa, 0, bd), a_f32v(s->cur, 0, bd));
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
        if (s->fast_buf && FL->ctrl_gf3.off >= 0 && rank <= 64) {
            int64_t citems = ((nb + 15) / 16) * ((d + INK_T - 1) / INK_T);
            CALL(s, batch, K_CTRL_UP_APPLY_GF_BATCH, citems, a_int(d), a_int(rank), a_int(nb), a_int(0),
                 a_fast_u32(s, &FL->ctrl_gf3),
                 a_f32v(s->ctrl_gate, 0, nb * rank), a_f32v(s->norm_buf, 0, bd), a_f32v(s->messages, 3 * bd, bd), a_f32v(s->norm_buf, 0, bd));
        } else {
            int64_t citems = ((nb + 7) / 8) * ((d + INK_T - 1) / INK_T);
            CALL(s, batch, K_CTRL_UP_APPLY_BATCH, citems, a_int(d), a_int(rank), a_int(nb),
                 a_tensor_f32(s, &L->loop_ctrl_gain[3]), a_tensor_f32(s, &L->loop_ctrl_forget[3]),
                 a_f32v(s->ctrl_gate, 0, nb * rank), a_f32v(s->norm_buf, 0, bd), a_f32v(s->messages, 3 * bd, bd), a_f32v(s->norm_buf, 0, bd));
        }
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
    int64_t direct_thresh = force_fp32 ? 16 : (m4_min - 1);
    {
        const char *env_t = getenv("INK_DIRECT_THRESH");
        if (!force_fp32 && env_t && env_t[0]) direct_thresh = strtoll(env_t, NULL, 10);
    }
    int used_sum_gather = 0;
    int use_iq4s_moe = (!force_fp32 && s->iq4s_buf && FL->moe_gu_iq4s.qs.off >= 0 && FL->moe_dn_iq4s.qs.off >= 0);
    int use_q8s_moe = (!force_fp32 && !use_iq4s_moe && s->fast_buf && s->fast_is_q8s && FL->moe_gu_q4s.qs.off >= 0 && FL->moe_dn_q4s.qs.off >= 0);
    if (use_iq4s_moe || use_q8s_moe) {
        /* Sprint F: Aligned in-tile IQ4S/Q8S Metal 4 MoE + vectorized gather/SITU + zero-scatter direct sum-gather! */
        int64_t tot_rows = nb * hp->n_expert_used;
        int use_256 = 0; /* TM=128 (64 regs/thread) beats TM=256 (128 regs, spills) by 13% on M5 Max! */
        int use_128 = (nb >= 768 && !use_256);
        int use_64 = (nb >= 384 && !use_128 && !use_256);
        int64_t tm = use_256 ? 256 : (use_128 ? 128 : (use_64 ? 64 : 16));
        int64_t tn = 64;
        int moe_mm_k = use_iq4s_moe
            ? (use_256 ? K_MOE_MM_IQ4S_256 : (use_128 ? K_MOE_MM_IQ4S_128 : (use_64 ? K_MOE_MM_IQ4S_64 : K_MOE_MM_IQ4S_16)))
            : (use_128 ? K_MOE_MM_Q8S_128 : (use_64 ? K_MOE_MM_Q8S_64 : K_MOE_MM_Q8S_16));
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
        CALL(s, batch, K_MOE_GATHER_BF16_V2, hp->n_expert * 8, a_int(d), a_int(hp->n_expert_used), a_int(INK_PREFILL_B),
             a_f32v(s->norm_buf, 0, bd),
             a_u32v(s->exp_toks, 0, hp->n_expert * INK_PREFILL_B),
             a_u32v(s->exp_slots, 0, hp->n_expert * INK_PREFILL_B),
             a_u32v(s->exp_cnt, 0, hp->n_expert),
             a_u32v(s->exp_off, 0, hp->n_expert),
             a_u16v(s->moe_slot_out, 0, tot_rows * d),
             a_u32v(s->row_dst, 0, tot_rows));
        pm_barrier(batch);
        prof_mark(bp, 7); batch = *bp; /* 7: moe_gather_bf16_v2+tiles */
        PmArg gu_qs = use_iq4s_moe ? a_iq4s_u32(s, &FL->moe_gu_iq4s.qs) : a_fast_u32(s, &FL->moe_gu_q4s.qs);
        PmArg gu_ds = use_iq4s_moe ? a_iq4s_u16(s, &FL->moe_gu_iq4s.ds) : a_fast_u16(s, &FL->moe_gu_q4s.ds);
        CALL(s, batch, moe_mm_k, max_tiles * gu_r_tiles, a_int(gu_rows), a_int(d),
             gu_qs, gu_ds,
             a_u16v(s->moe_slot_out, 0, tot_rows * d),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16),
             a_f32v(s->moe_gu, 0, tot_rows * gu_rows));
        pm_barrier(batch);
        prof_mark(bp, 8); batch = *bp; /* 8: moe_gu_iq4s */
        CALL(s, batch, K_SITU_BATCH_BF16_V2, tot_rows,
             a_int(n_ff), a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_f32v(s->moe_gu, 0, tot_rows * gu_rows),
             a_u16v(s->moe_hid, 0, tot_rows * n_ff));
        pm_barrier(batch);
        prof_mark(bp, 9); batch = *bp; /* 9: situ_batch_bf16_v2 */
        PmArg dn_qs = use_iq4s_moe ? a_iq4s_u32(s, &FL->moe_dn_iq4s.qs) : a_fast_u32(s, &FL->moe_dn_q4s.qs);
        PmArg dn_ds = use_iq4s_moe ? a_iq4s_u16(s, &FL->moe_dn_iq4s.ds) : a_fast_u16(s, &FL->moe_dn_q4s.ds);
        CALL(s, batch, moe_mm_k, max_tiles * dn_r_tiles, a_int(d), a_int(n_ff),
             dn_qs, dn_ds,
             a_u16v(s->moe_hid, 0, tot_rows * n_ff),
             a_u32v(s->exp_cnt, 0, hp->n_expert), a_u32v(s->exp_off, 0, hp->n_expert),
             a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16),
             a_f32v(s->moe_gu, 0, tot_rows * d));
        pm_barrier(batch);
        prof_mark(bp, 10); batch = *bp; /* 10: moe_dn_iq4s (0 scatter!) */
        used_sum_gather = 1;
    } else if (nb <= direct_thresh &&
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
    } else if (nb >= m4_min &&
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
    } else if (((nb >= 8 &&
                 (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL || L->ffn_gate_up_exps.type == INK_GGML_Q8_0) &&
                 (L->ffn_down_exps.type == INK_GGML_IQ4_NL || L->ffn_down_exps.type == INK_GGML_Q8_0)) ||
                (!(L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL || L->ffn_gate_up_exps.type == INK_GGML_Q8_0) ||
                 !(L->ffn_down_exps.type == INK_GGML_IQ4_NL || L->ffn_down_exps.type == INK_GGML_Q8_0))) &&
               ink_ggml_type_supported(L->ffn_gate_up_exps.type) &&
               ink_ggml_type_supported(L->ffn_down_exps.type)) {
        /* Sprint 9B + Universal GGML: Metal 4 Neural Accelerator (am_matmul_bf16) Expert-Grouped MoE! */
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
        int gu_is_iq4 = (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL);
        int gu_is_q8 = (L->ffn_gate_up_exps.type == INK_GGML_Q8_0);
        int64_t gu_deq_rgrps = gu_is_iq4 ? ((gu_rows + 7) / 8) : ((gu_rows + 3) / 4);
        for (int64_t e_lo = 0; e_lo < hp->n_expert; e_lo += e_chunk_gu) {
            int64_t e_hi = e_lo + e_chunk_gu;
            if (e_hi > hp->n_expert) e_hi = hp->n_expert;
            int64_t ec = e_hi - e_lo;
            int64_t max_ct = ec + tot_rows / tm;
            if (max_ct > tile_cap) max_ct = tile_cap;
            CALL(s, batch, K_MOE_CHUNK_TILES, 1, a_int(e_lo), a_int(e_hi), a_int(tm), a_int(gu_r_tiles),
                 a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16));
            if (gu_is_iq4 || gu_is_q8) {
                CALL(s, batch, gu_is_iq4 ? K_DEQUANT_IQ4_EXP_BF16 : K_DEQUANT_Q8_EXP_BF16,
                     ec * gu_deq_rgrps, a_int(e_lo), a_int(gu_rows), a_int(d),
                     a_tensor_u16(s, &L->ffn_gate_up_exps), a_u32v(s->exp_cnt, 0, hp->n_expert),
                     a_u32v(s->w_bf16, 0, ec * gu_w_u32));
            } else {
                CALL(s, batch, K_DEQUANT_GGML_EXP_BF16,
                     ec * gu_deq_rgrps, a_int(e_lo), a_int(gu_rows), a_int(d), a_int((int64_t)L->ffn_gate_up_exps.type),
                     a_tensor_ggml_u16(s, &L->ffn_gate_up_exps), a_ggml_grids(s), a_u32v(s->exp_cnt, 0, hp->n_expert),
                     a_u32v(s->w_bf16, 0, ec * gu_w_u32));
            }
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

        /* Down projection (32 experts/chunk fits in 24M+ u32 w_bf16!) */
        int64_t e_chunk_dn = 32;
        int64_t dn_w_u32 = d * (n_ff / 2);
        int64_t dn_r_tiles = (d + tn - 1) / tn;
        int dn_is_iq4 = (L->ffn_down_exps.type == INK_GGML_IQ4_NL);
        int dn_is_q8 = (L->ffn_down_exps.type == INK_GGML_Q8_0);
        int64_t dn_deq_rgrps = dn_is_iq4 ? ((d + 7) / 8) : ((d + 3) / 4);
        for (int64_t e_lo = 0; e_lo < hp->n_expert; e_lo += e_chunk_dn) {
            int64_t e_hi = e_lo + e_chunk_dn;
            if (e_hi > hp->n_expert) e_hi = hp->n_expert;
            int64_t ec = e_hi - e_lo;
            int64_t max_ct = ec + tot_rows / tm;
            if (max_ct > tile_cap) max_ct = tile_cap;
            CALL(s, batch, K_MOE_CHUNK_TILES, 1, a_int(e_lo), a_int(e_hi), a_int(tm), a_int(dn_r_tiles),
                 a_u32v(s->exp_cnt, 0, hp->n_expert),
                 a_u32v(s->tile_exp, 0, tile_cap), a_u32v(s->tile_m0, 0, tile_cap), a_u32v(s->tile_cnt, 0, 16));
            if (dn_is_iq4 || dn_is_q8) {
                CALL(s, batch, dn_is_iq4 ? K_DEQUANT_IQ4_EXP_BF16 : K_DEQUANT_Q8_EXP_BF16,
                     ec * dn_deq_rgrps, a_int(e_lo), a_int(d), a_int(n_ff),
                     a_tensor_u16(s, &L->ffn_down_exps), a_u32v(s->exp_cnt, 0, hp->n_expert),
                     a_u32v(s->w_bf16, 0, ec * dn_w_u32));
            } else {
                CALL(s, batch, K_DEQUANT_GGML_EXP_BF16,
                     ec * dn_deq_rgrps, a_int(e_lo), a_int(d), a_int(n_ff), a_int((int64_t)L->ffn_down_exps.type),
                     a_tensor_ggml_u16(s, &L->ffn_down_exps), a_ggml_grids(s), a_u32v(s->exp_cnt, 0, hp->n_expert),
                     a_u32v(s->w_bf16, 0, ec * dn_w_u32));
            }
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
    if (used_sum_gather) {
        CALL(s, batch, K_MOE_SUM_GATHER_BATCH, nb * ((d + 511) / 512), a_int(d), a_int(hp->n_expert_used),
             a_f32v(s->moe_gu, 0, nb * hp->n_expert_used * d),
             a_u32v(s->row_dst, 0, nb * hp->n_expert_used),
             a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
             a_f32v(s->moe_out, 0, bd));
    } else {
        /* Sprint 30: Parallel coalesced MoE slot sum + coalesced post shortconv & residual add (446ms -> ~30ms on 2.4K prefill!) */
        CALL(s, batch, K_MOE_SUM_BATCH, nb * ((d + 127) / 128), a_int(d), a_int(hp->n_expert_used),
             a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d),
             a_f32v(s->moe_out, 0, bd));
    }
    pm_barrier(batch);
    if (s->has_ablate && ip < 32 && s->ablate_has_layer[ip]) {
        CALL(s, batch, K_ABLATE_DIR, nb, a_int(d), a_int((int64_t)(1 + ip) * d),
             a_ablate_dirs(s), a_f32v(s->moe_out, 0, bd));
        pm_barrier(batch);
    }
    if (save_spec_hist) {
        CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
             a_f32v(s->moe_out, 0, bd), a_f32v(s->spec_hist, spec_slot_base + (int64_t)INK_SPEC_MAX_NB * (2 * n_kv + d), bd));
    }
    CALL(s, batch, K_CONVOLVE_ADD_BATCH, conv_d_items, a_int(d), a_int(nb),
         a_f32v(s->moe_out, 0, bd),
         a_tensor_f32(s, &L->ffn_post_conv),
         a_conv_fo(s, il),
         a_f32v(s->ffn_inp, 0, bd),
         a_f32v(s->cur, 0, bd));
    pm_barrier(batch);
    prof_mark(bp, 11); batch = *bp; /* 11: moe_reduce_batch */
    return 0;
}

static int run_chunk_ex(InkSession *s, const int32_t *tokens, int nb, int compute_logits_mode, int save_spec_hist, int force_fp32) {
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
    if (s->gguf.tok_embd.type == INK_GGML_Q8_0 && !s->ablate_has_emb) {
        CALL(s, batch, K_EMBED_Q8_BATCH, nb, a_int(d),
             a_u32v(s->tok_ids_buf, 0, nb),
             a_tensor_u16(s, &s->gguf.tok_embd),
             a_tensor_f32(s, &s->gguf.tok_norm),
             a_f32(hp->rms_eps),
             a_f32v(s->cur, 0, bd));
    } else {
        CALL(s, batch, K_EMBED_GGML_BATCH, nb, a_int(d), a_int((int64_t)s->gguf.tok_embd.type),
             a_u32v(s->tok_ids_buf, 0, nb),
             a_tensor_ggml_u16(s, &s->gguf.tok_embd), a_ggml_grids(s),
             a_tensor_f32(s, &s->gguf.tok_norm), a_f32(hp->rms_eps),
             a_int(s->ablate_has_emb ? 1 : 0), a_ablate_dirs(s),
             a_f32v(s->cur, 0, bd));
    }
    pm_barrier(batch);

    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;

    int do_pprof = (!save_spec_hist && getenv("INK_PREFILL_PROF") != NULL);
    if (do_pprof) { prof_active = 1; memset(prof_ms, 0, sizeof prof_ms); }
    int cap_taps = (!save_spec_hist && s->dspark.ready && s->dspark.pos == s->pos);

    /* 2. Prelude layers [0 .. start) */
    for (int il = 0; il < start; il++) {
        if (cap_taps && il == 5) {
            CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
                 a_f32v(s->cur, 0, bd), a_f32v(s->dspark.taps, 0, bd));
            pm_barrier(batch);
        }
        if (dispatch_layer_batch(s, &batch, il, il, &n_blocks, 0, nb, save_spec_hist, force_fp32) != 0) { pm_wait(batch, NULL); return 1; }
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
            if (cap_taps && pass == 1 && ip == 11) {
                CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
                     a_f32v(s->cur, 0, bd), a_f32v(s->dspark.taps, 1 * bd, bd));
                pm_barrier(batch);
            } else if (cap_taps && pass == 2 && ip == 17) {
                CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
                     a_f32v(s->cur, 0, bd), a_f32v(s->dspark.taps, 2 * bd, bd));
                pm_barrier(batch);
            }
            int il = (pass == 0) ? ip : (int)hp->n_phys + (pass - 1) * (end - start) + (ip - start);
            if (pass == 0) {
                if (dispatch_layer_batch(s, &batch, il, ip, &n_blocks, 0, nb, save_spec_hist, force_fp32) != 0) { pm_wait(batch, NULL); return 1; }
            } else {
                const InkLayerTensors *L = &s->gguf.layers[ip];
                CALL(s, batch, K_MIX_BATCH, 4 * nb, a_int(d), a_int(bank_n_blocks), a_int(nb),
                     a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * bd),
                     a_f32v(s->bank_out, 0, bd),
                     a_tensor_f32(s, &L->loop_msg_query),
                     a_f32v(s->messages, 0, 4 * bd));
                pm_barrier(batch);
                if (dispatch_layer_batch(s, &batch, il, ip, &n_blocks, 1, nb, save_spec_hist, force_fp32) != 0) { pm_wait(batch, NULL); return 1; }
            }
        }
    }

    /* 4. Coda layers [end .. n_phys) */
    for (int il = end; il < hp->n_phys; il++) {
        if (dispatch_layer_batch(s, &batch, il, il, &n_blocks, 0, nb, save_spec_hist, force_fp32) != 0) { pm_wait(batch, NULL); return 1; }
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

    /* 5. Output mix -> output_norm (+ DSpark full-chunk absorb + lm_head!) */
    int64_t last_off = (int64_t)(nb - 1) * d;
    if (compute_logits_mode == 1 || (compute_logits_mode == 0 && cap_taps)) {
        CALL(s, batch, K_MIX_BATCH, nb, a_int(d), a_int(n_blocks), a_int(nb),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd),
             a_f32v(s->cur, 0, bd),
             a_tensor_f32(s, &s->gguf.out_res_query),
             a_f32v(s->norm_buf, 0, bd));
        pm_barrier(batch);
        if (cap_taps) {
            CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
                 a_f32v(s->norm_buf, 0, bd), a_tensor_f32(s, &s->gguf.out_norm),
                 a_f32v(s->dspark.taps, 3 * bd, bd));
            pm_barrier(batch);
            if (compute_logits_mode == 1) {
                CALL(s, batch, K_COPY, (d + INK_NT - 1) / INK_NT, a_int(d),
                     a_f32v(s->dspark.taps, 3 * bd + last_off, d), a_f32v(s->norm_buf, last_off, d));
                pm_barrier(batch);
            }
        } else {
            CALL(s, batch, K_RMSNORM, 1, a_int(d), a_int(1), a_f32(hp->rms_eps),
                 a_f32v(s->norm_buf, last_off, d), a_tensor_f32(s, &s->gguf.out_norm), a_f32v(s->norm_buf, last_off, d));
            pm_barrier(batch);
        }
        if (compute_logits_mode == 1) {
            int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
            if (s->fast_buf && s->fast_output_s.qs.off >= 0 && !force_fp32) {
                CALL(s, batch, s->fast_output_is_q4s ? K_DV_Q4S_RMS : K_DV_Q8S_RMS, (nv + 7) / 8, a_int(nv), a_int(d),
                     a_int(0), a_int(0), a_f32(hp->rms_eps), a_f32(hp->logit_softcap),
                     a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
                     a_f32v(s->norm_buf, last_off, d), a_f32v(s->rms_parts, 0, 3 * 64), a_f32v(s->logits_buf[0], 0, nv));
            } else {
                if (dispatch_mv(s, batch, &s->gguf.output, nv, d, 0, s->norm_buf, last_off, s->logits_buf[0], 0) != 0) { pm_wait(batch, NULL); return 1; }
                if (hp->logit_softcap > 0.0f) {
                    pm_barrier(batch);
                    CALL(s, batch, K_SOFTCAP, (nv + INK_NT - 1) / INK_NT, a_int(nv),
                         a_f32(hp->logit_softcap), a_f32v(s->logits_buf[0], 0, nv));
                }
            }
            pm_barrier(batch);
            CALL(s, batch, K_TOPK64_S1, 32, a_int(32), a_int(nv),
                 a_f32v(s->logits_buf[0], 0, nv), a_f32v(s->topk_s1_vals, 0, 32 * 64), a_u32v(s->topk_s1_ids, 0, 32 * 64));
            pm_barrier(batch);
            CALL(s, batch, K_TOPK64_S2, 1, a_int(32 * 64),
                 a_f32v(s->topk_s1_vals, 0, 32 * 64), a_u32v(s->topk_s1_ids, 0, 32 * 64),
                 a_f32v(s->topk_vals[0], 0, 64), a_u32v(s->topk_ids[0], 0, 64), a_u32v(s->argmax_out_buf[0], 0, 1));
            s->active_out_slot = 0;
            s->hidden_is_prefill = 1;
            s->last_hidden_off = last_off;
            s->argmax_valid = 1;
        }
    } else if (compute_logits_mode == 2) {
        /* All nb tokens into s->verify_logits[0..nb*nv) for ink_verify_draft. */
        CALL(s, batch, K_MIX_BATCH, nb, a_int(d), a_int(n_blocks), a_int(nb),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd),
             a_f32v(s->cur, 0, bd),
             a_tensor_f32(s, &s->gguf.out_res_query),
             a_f32v(s->norm_buf, 0, bd));
        pm_barrier(batch);
        CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
             a_f32v(s->norm_buf, 0, bd), a_tensor_f32(s, &s->gguf.out_norm), a_f32v(s->norm_buf, 0, bd));
        pm_barrier(batch);
        int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
        if (s->fast_buf && s->fast_output_s.qs.off >= 0 && !force_fp32) {
            for (int t = 0; t < nb; t++) {
                CALL(s, batch, s->fast_output_is_q4s ? K_DV_Q4S_RMS : K_DV_Q8S_RMS, (nv + 7) / 8, a_int(nv), a_int(d),
                     a_int(0), a_int(0), a_f32(hp->rms_eps), a_f32(hp->logit_softcap),
                     a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
                     a_f32v(s->norm_buf, (int64_t)t * d, d), a_f32v(s->rms_parts, 0, 3 * 64),
                     a_f32v(s->verify_logits, (int64_t)t * nv, nv));
            }
        } else {
            for (int t = 0; t < nb; t++) {
                if (dispatch_mv_q4(s, batch, &s->gguf.output, &s->fast_output, nv, d, 0,
                                   s->norm_buf, (int64_t)t * d, s->verify_logits, (int64_t)t * nv) != 0) { pm_wait(batch, NULL); return 1; }
            }
            if (hp->logit_softcap > 0.0f) {
                pm_barrier(batch);
                int64_t tot_l = (int64_t)nb * nv;
                CALL(s, batch, K_SOFTCAP, (tot_l + INK_NT - 1) / INK_NT, a_int(tot_l),
                     a_f32(hp->logit_softcap), a_f32v(s->verify_logits, 0, tot_l));
            }
        }
        s->active_out_slot = 0;
        s->hidden_is_prefill = 1;
        s->last_hidden_off = last_off;
        s->argmax_valid = 0;
    }
    if (cap_taps) {
        if (compute_logits_mode == 1) pm_barrier(batch);
        if (ink_dspark_enqueue_absorb(s, batch, nb, nb, s->pos) != 0) { pm_wait(batch, NULL); return 1; }
    }

    double gpu_s = 0.0;
    if (pm_wait(batch, &gpu_s) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_wait: %s", pm_log());
        return 1;
    }
    if (compute_logits_mode == 1) {
        s->prev_argmax = (int32_t)((const uint32_t *)pm_contents(s->argmax_out_buf[0]))[0];
        s->has_prev_argmax = 1;
    } else {
        s->has_prev_argmax = 0;
    }
    s->greedy_streak = 0;
    if (!save_spec_hist) {
        if (s->live_tokens && s->pos + nb <= s->max_ctx) {
            memcpy(s->live_tokens + s->pos, tokens, (size_t)nb * sizeof(int32_t));
            s->live_len = s->pos + nb;
        }
        s->pos += nb;
        ink_mark_written(s, s->pos);
        s->total_steps += nb;
    }
    s->total_gpu_seconds += gpu_s;
    return 0;
}

static int run_chunk(InkSession *s, const int32_t *tokens, int nb, int compute_last_logits) {
    return run_chunk_ex(s, tokens, nb, compute_last_logits ? 1 : 0, 0, 0);
}

/* ---- Sprint 71B: 100% lossless bit-exact tiny-batch (B=2..8) grouped-MoE verify + adaptive PLD ---- */

static int dispatch_layer_nb(InkSession *s, PmBatch batch, int il, int ip, int *n_blocks,
                             int has_messages, int64_t nb) {
    const InkHParams *hp = &s->gguf.hp;
    const InkLayerTensors *L = &s->gguf.layers[ip];
    const FastLayer *FL = &s->fast_layers[ip];
    int64_t d = hp->n_embd;
    int64_t n_head = hp->n_head;
    int64_t n_head_kv = hp->n_head_kv;
    int64_t hd = hp->head_dim;
    int64_t n_q = n_head * hd;
    int64_t n_kv = n_head_kv * hd;
    int64_t n_rel = hp->rel_len;
    int is_swa = hp->is_swa[ip];
    int is_blk = (ip % hp->block_size == 0);
    int64_t bd = nb * d;
    int64_t msg_base = has_messages ? ((int64_t)(ip - hp->loop_start) * 4 * bd) : 0;

    /* Wave 0: optional block save (concurrent with mix!) + fused depth-attention mix (+ RMSNorm or ctrl_prep) */
    if (is_blk) {
        if (*n_blocks >= INK_MAX_BLOCKS) { snprintf(s->error, sizeof s->error, "block count overflow"); return 1; }
        CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->cur, 0, bd),
             a_f32v(s->blocks, (int64_t)(*n_blocks) * bd, bd));
    }
    int use_mma = (getenv("INK_NO_MMA32") == NULL);
    int use_q8s_attn = (s->fast_buf && s->fast_is_q8s && FL->attn_qrgkv_s.qs.off >= 0);
    int use_q4s_attn = (s->fast_buf && !s->fast_is_q8s && FL->attn_qrgkv_s.qs.off >= 0 && FL->attn_q_s.qs.off >= 0 && FL->attn_sink_s.qs.off >= 0);
    PmBuffer q_in = s->inp_q, k_in = s->inp_k, v_in = s->inp_v;
    int64_t qkv_n_parts = 0;
    if (!has_messages) {
        if (!L->attn_res_query.present) {
            if (is_swa) {
                CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
                     a_f32v(s->cur, 0, bd), a_tensor_f32(s, &L->attn_norm), a_f32v(s->inp_q, 0, bd));
                k_in = s->inp_q; v_in = s->inp_q;
            } else {
                CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
                     a_f32v(s->cur, 0, bd), a_tensor_f32(s, &L->attn_q_inp_norm), a_f32v(s->inp_q, 0, bd));
                CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
                     a_f32v(s->cur, 0, bd), a_tensor_f32(s, &L->attn_k_inp_norm), a_f32v(s->inp_k, 0, bd));
                CALL(s, batch, K_RMSNORM, nb, a_int(d), a_int(1), a_f32(hp->rms_eps),
                     a_f32v(s->cur, 0, bd), a_tensor_f32(s, &L->attn_v_inp_norm), a_f32v(s->inp_v, 0, bd));
            }
        } else if (is_swa) {
            CALL(s, batch, K_MIX_RMSNORM_NB, nb, a_int(d), a_int(*n_blocks), a_int(nb), a_f32(hp->rms_eps),
                 a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd), a_f32v(s->cur, 0, bd),
                 a_tensor_f32(s, &L->attn_res_query),
                 a_tensor_f32(s, &L->attn_norm), a_tensor_f32(s, &L->attn_norm), a_tensor_f32(s, &L->attn_norm),
                 a_f32v(s->inp_q, 0, bd), a_f32v(s->inp_q, 0, bd), a_f32v(s->inp_q, 0, bd));
            k_in = s->inp_q; v_in = s->inp_q;
        } else {
            CALL(s, batch, K_MIX_RMSNORM_NB, 3 * nb, a_int(d), a_int(*n_blocks), a_int(nb), a_f32(hp->rms_eps),
                 a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd), a_f32v(s->cur, 0, bd),
                 a_tensor_f32(s, &L->attn_res_query),
                 a_tensor_f32(s, &L->attn_q_inp_norm), a_tensor_f32(s, &L->attn_k_inp_norm), a_tensor_f32(s, &L->attn_v_inp_norm),
                 a_f32v(s->inp_q, 0, bd), a_f32v(s->inp_k, 0, bd), a_f32v(s->inp_v, 0, bd));
        }
        pm_barrier(batch);
    } else {
        int64_t rank = hp->gate_rank;
        int64_t citems = (d + 31) / 32;
        qkv_n_parts = citems;
        CALL(s, batch, K_MIX_CTRL_PREP_NB, 3 * nb, a_int(d), a_int(*n_blocks), a_int(nb), a_int(is_swa),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd), a_f32v(s->cur, 0, bd),
             a_tensor_f32(s, &L->attn_res_query),
             a_f32v(s->messages, msg_base, 3 * bd),
             a_f32v(s->mix_out, 0, 3 * bd), a_f32v(s->ctrl_cat, 0, 6 * bd));
        pm_barrier(batch);
        if (FL->ctrl_down_qkv_s.qs.off >= 0) {
            CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_SMALL_NB : K_DV_Q4S_SMALL_NB, 3 * rank, a_int(3 * rank), a_int(rank), a_int(2 * d), a_int(nb),
                 a_int(0), a_f32(hp->rms_eps),
                 a_fast_u32(s, &FL->ctrl_down_qkv_s.qs), a_fast_u16(s, &FL->ctrl_down_qkv_s.ds),
                 a_f32v(s->ctrl_cat, 0, 6 * bd), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->ctrl_gate, 0, 3 * nb * rank));
        } else {
            CALL(s, batch, K_DV_F16_SMALL_NB, 3 * rank, a_int(3 * rank), a_int(rank), a_int(2 * d), a_int(nb),
                 a_fast_u16(s, &FL->ctrl_down_qkv_q4),
                 a_f32v(s->ctrl_cat, 0, 6 * bd), a_f32v(s->ctrl_gate, 0, 3 * nb * rank));
        }
        pm_barrier(batch);
        CALL(s, batch, K_CTRL_UP_APPLY_GF_RM_3_NB, 3 * citems, a_int(d), a_int(rank), a_int(nb),
             a_fast_u32(s, &FL->ctrl_gf_rm_qkv), a_f32v(s->ctrl_gate, 0, 3 * nb * rank),
             a_f32v(s->mix_out, 0, 3 * bd), a_f32v(s->messages, msg_base, 3 * bd),
             is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_q_inp_norm),
             is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_k_inp_norm),
             is_swa ? a_tensor_f32(s, &L->attn_norm) : a_tensor_f32(s, &L->attn_v_inp_norm),
             a_f32v(s->inp_q, 0, bd), a_f32v(s->inp_k, 0, bd), a_f32v(s->inp_v, 0, bd),
             a_f32v(s->rms_parts, 0, 8 * 3 * 128));
        pm_barrier(batch);
        if (!use_q8s_attn && (use_mma || !use_q4s_attn)) {
            CALL(s, batch, K_SCALE_RMS_PARTS_NB, 3 * nb, a_int(d), a_int(nb), a_int(citems), a_f32(hp->rms_eps),
                 a_f32v(s->rms_parts, 0, 8 * 3 * 128),
                 a_f32v(s->inp_q, 0, bd), a_f32v(s->inp_k, 0, bd), a_f32v(s->inp_v, 0, bd));
            pm_barrier(batch);
            qkv_n_parts = 0;
        }
    }

    /* Wave 3: Lossless Q8S fused QRGKV, MMA32 Q4S, or Lossless Q8_0 MMA32! */
    int has_k_dyn = (!is_swa && L->attn_k_conv_proj.present);
    int64_t dyn_sz = n_head_kv * hp->conv_len;
    int64_t n_dyn_use = has_k_dyn ? dyn_sz : 0;
    if (use_q8s_attn || (use_q4s_attn && !use_mma)) {
        int64_t tot_rows = n_q + n_head * n_rel + n_q + 2 * n_kv + n_head + 2 * n_dyn_use;
        uint32_t sd0 = (uint32_t)n_q | ((uint32_t)(n_head * n_rel) << 16);
        uint32_t sd1 = (uint32_t)n_kv | ((uint32_t)n_head << 16);
        uint32_t sd2 = (uint32_t)n_dyn_use | ((uint32_t)qkv_n_parts << 16);
        CALL(s, batch, use_q8s_attn ? K_DV_Q8S_QRGKV_SDYN_NB : K_DV_Q4S_QRGKV_SDYN_NB, (tot_rows + 3) / 4,
             a_u32(sd0), a_u32(sd1), a_u32(sd2), a_int(d), a_int(nb), a_f32(hp->rms_eps),
             a_fast_u32(s, &FL->attn_qrgkv_s.qs), a_fast_u16(s, &FL->attn_qrgkv_s.ds),
             a_f32v(q_in, 0, bd), a_f32v(k_in, 0, bd), a_f32v(v_in, 0, bd),
             a_f32v(s->rms_parts, 0, 8 * 3 * 128),
             a_f32v(s->q_buf, 0, nb * n_q), a_f32v(s->r_buf, 0, nb * n_head * n_rel), a_f32v(s->gate_buf, 0, nb * n_q),
             a_f32v(s->k_raw, 0, nb * n_kv), a_f32v(s->v_raw, 0, nb * n_kv),
             a_f32v(s->sinks_buf, 0, nb * n_head),
             has_k_dyn ? a_f32v(s->dyn_buf, 0, nb * 2 * dyn_sz) : a_f32v(s->dummy_f32, 0, 64));
    } else if (use_q4s_attn) {
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_q + 31) / 32, a_int(n_q), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_q_s.qs), a_fast_u16(s, &FL->attn_q_s.ds),
             a_f32v(q_in, 0, bd), a_f32v(s->q_buf, 0, nb * n_q));
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_head * n_rel + 31) / 32, a_int(n_head * n_rel), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_rel_s.qs), a_fast_u16(s, &FL->attn_rel_s.ds),
             a_f32v(q_in, 0, bd), a_f32v(s->r_buf, 0, nb * n_head * n_rel));
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_q + 31) / 32, a_int(n_q), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_gate_s.qs), a_fast_u16(s, &FL->attn_gate_s.ds),
             a_f32v(q_in, 0, bd), a_f32v(s->gate_buf, 0, nb * n_q));
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_kv + 31) / 32, a_int(n_kv), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_k_s.qs), a_fast_u16(s, &FL->attn_k_s.ds),
             a_f32v(k_in, 0, bd), a_f32v(s->k_raw, 0, nb * n_kv));
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (n_kv + 31) / 32, a_int(n_kv), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_fast_u32(s, &FL->attn_v_s.qs), a_fast_u16(s, &FL->attn_v_s.ds),
             a_f32v(v_in, 0, bd), a_f32v(s->v_raw, 0, nb * n_kv));
        CALL(s, batch, K_DV_Q4S_SMALL_NB, n_head, a_int(n_head), a_int(n_head), a_int(d), a_int(nb),
             a_int(0), a_f32(hp->rms_eps),
             a_fast_u32(s, &FL->attn_sink_s.qs), a_fast_u16(s, &FL->attn_sink_s.ds),
             a_f32v(q_in, 0, bd), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->sinks_buf, 0, nb * n_head));
        if (has_k_dyn) {
            CALL(s, batch, K_DV_Q4S_SMALL_NB, dyn_sz, a_int(dyn_sz), a_int(2 * dyn_sz), a_int(d), a_int(nb),
                 a_int(0), a_f32(hp->rms_eps),
                 a_fast_u32(s, &FL->attn_k_dyn_s.qs), a_fast_u16(s, &FL->attn_k_dyn_s.ds),
                 a_f32v(k_in, 0, bd), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->dyn_buf, 0, nb * 2 * dyn_sz));
            CALL(s, batch, K_DV_Q4S_SMALL_NB, dyn_sz, a_int(dyn_sz), a_int(2 * dyn_sz), a_int(d), a_int(nb),
                 a_int(0), a_f32(hp->rms_eps),
                 a_fast_u32(s, &FL->attn_v_dyn_s.qs), a_fast_u16(s, &FL->attn_v_dyn_s.ds),
                 a_f32v(v_in, 0, bd), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->dyn_buf, dyn_sz, nb * 2 * dyn_sz));
        }
    } else {
        /* Lossless Q8_0 MMA32 (285-297 GB/s, 0.00e+00 nb=1 vs nb=8, zero 4-bit attention degradation!) */
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_q + 31) / 32, a_int(n_q), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_q), a_f32v(q_in, 0, bd), a_f32v(s->q_buf, 0, nb * n_q));
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_head * n_rel + 31) / 32, a_int(n_head * n_rel), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_rel), a_f32v(q_in, 0, bd), a_f32v(s->r_buf, 0, nb * n_head * n_rel));
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_q + 31) / 32, a_int(n_q), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_gate), a_f32v(q_in, 0, bd), a_f32v(s->gate_buf, 0, nb * n_q));
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_kv + 31) / 32, a_int(n_kv), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_k), a_f32v(k_in, 0, bd), a_f32v(s->k_raw, 0, nb * n_kv));
        CALL(s, batch, K_DV_Q8_MMA32_NB, (n_kv + 31) / 32, a_int(n_kv), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(0.0f),
             a_tensor_u16(s, &L->attn_v), a_f32v(v_in, 0, bd), a_f32v(s->v_raw, 0, nb * n_kv));
        CALL(s, batch, K_DV_F16_SMALL_NB, n_head, a_int(n_head), a_int(n_head), a_int(d), a_int(nb),
             a_fast_u16(s, &FL->attn_sink_proj), a_f32v(q_in, 0, bd), a_f32v(s->sinks_buf, 0, nb * n_head));
        if (has_k_dyn) {
            CALL(s, batch, K_DV_F16_SMALL_NB, dyn_sz, a_int(dyn_sz), a_int(2 * dyn_sz), a_int(d), a_int(nb),
                 a_fast_u16(s, &FL->attn_k_conv_proj), a_f32v(k_in, 0, bd), a_f32v(s->dyn_buf, 0, nb * 2 * dyn_sz));
            CALL(s, batch, K_DV_F16_SMALL_NB, dyn_sz, a_int(dyn_sz), a_int(2 * dyn_sz), a_int(d), a_int(nb),
                 a_fast_u16(s, &FL->attn_v_conv_proj), a_f32v(v_in, 0, bd), a_f32v(s->dyn_buf, dyn_sz, nb * 2 * dyn_sz));
        }
    }
    pm_barrier(batch);

    /* Wave 4+5 (Sprint C): Fused 3-head GQA Split-K + Q8_0 KV cache (5.65x fewer KV DRAM bytes + 40-core saturation for B=2..8!) */
    int64_t kv_cap = s->kv_cap[il];
    int64_t n_keys_nb = (is_swa && s->pos >= hp->swa) ? hp->swa : (s->pos + nb);
    int64_t n_splits = ink_choose_attn_splits(s, n_keys_nb, nb);
    int64_t split_elems = nb * n_head * n_splits * 132;
    uint32_t dims0 = (uint32_t)n_head | ((uint32_t)n_head_kv << 8) | ((uint32_t)hd << 16) | ((uint32_t)n_rel << 24);
    uint32_t dims1 = (uint32_t)hp->rel_extent | ((uint32_t)(is_swa ? 1 : 0) << 16) | ((uint32_t)hp->swa << 17);
    uint32_t kv_info = (uint32_t)kv_cap | ((uint32_t)(L->attn_q_clip.present ? 1 : 0) << 16)
                     | ((uint32_t)(L->attn_rel_clip.present ? 1 : 0) << 17)
                     | ((uint32_t)(has_k_dyn ? 1 : 0) << 18)
                     | ((uint32_t)(s->kv_q8 ? 1 : 0) << 19)
                     | ((uint32_t)nb << 20)
                     | ((uint32_t)n_splits << 24);
    CALL(s, batch, K_KV_ATTN_GQA3_SPLIT_NB, n_head_kv * n_splits,
         a_u32(dims0), a_u32(dims1), a_u32(kv_info), a_int(s->pos),
         a_f32(hp->qk_clip), a_f32(hp->rel_clip), a_f32(hp->rms_eps),
         is_swa ? a_f32v(s->dummy_ones, 0, 8) : a_f32v(s->tau_buf, 0, nb),
         a_f32v(s->k_raw, 0, nb * n_kv), a_f32v(s->v_raw, 0, nb * n_kv),
         a_tensor_f32(s, &L->attn_k_conv), a_tensor_f32(s, &L->attn_v_conv),
         has_k_dyn ? a_f32v(s->dyn_buf, 0, nb * 2 * dyn_sz) : a_f32v(s->dummy_f32, 0, 64),
         a_conv_k(s, il), a_conv_v(s, il),
         a_f32v(s->q_buf, 0, nb * n_q),
         L->attn_q_clip.present ? a_tensor_f32(s, &L->attn_q_clip) : a_f32v(s->dummy_f32, 0, 64),
         a_f32v(s->r_buf, 0, nb * n_head * n_rel),
         L->attn_rel_clip.present ? a_tensor_f32(s, &L->attn_rel_clip) : a_f32v(s->dummy_f32, 0, 64),
         a_tensor_f32(s, &L->attn_rel_bias),
         a_f32v(s->sinks_buf, 0, nb * n_head),
         a_f32v(s->gate_buf, 0, nb * n_q),
         a_u16v(s->kv_k[il], 0, kv_cap * s->kv_row_u16), a_u16v(s->kv_v[il], 0, kv_cap * s->kv_row_u16),
         a_raw_k(s, il), a_raw_v(s, il),
         a_f32v(s->attn_scores, 0, split_elems),
         a_f32v(s->attn_out, 0, nb * n_q));
    if (n_splits > 1) {
        pm_barrier(batch);
        CALL(s, batch, K_ATTN_GQA3_REDUCE_NB, nb * n_head,
             a_int(n_splits),
             a_f32v(s->attn_scores, 0, split_elems),
             a_f32v(s->gate_buf, 0, nb * n_q),
             a_f32v(s->attn_out, 0, nb * n_q));
    }
    pm_barrier(batch);

    /* Wave 6: Attn output proj (Lossless Q8S, MMA32 Q4S, or Lossless Q8_0 MMA32) + post-conv + raw_log + residual! */
    int ablate_ip = (s->has_ablate && ip < 32 && s->ablate_has_layer[ip]);
    if (ablate_ip) {
        if (s->fast_buf && s->fast_is_q8s && FL->attn_output_s.qs.off >= 0) {
            CALL(s, batch, K_DV_Q8S_NB, (d + 3) / 4, a_int(d), a_int(n_q), a_int(nb),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
                 a_f32v(s->attn_out, 0, nb * n_q), a_f32v(s->o_raw, 0, bd));
        } else if (FL->attn_output_s.qs.off >= 0) {
            CALL(s, batch, K_DV_Q4S_MMA32_NB, (d + 31) / 32, a_int(d), a_int(n_q), a_int(nb),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
                 a_f32v(s->attn_out, 0, nb * n_q), a_f32v(s->o_raw, 0, bd));
        } else {
            CALL(s, batch, K_DV_Q8_MMA32_NB, (d + 31) / 32, a_int(d), a_int(n_q), a_int(nb),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_tensor_u16(s, &L->attn_output),
                 a_f32v(s->attn_out, 0, nb * n_q), a_f32v(s->o_raw, 0, bd));
        }
        pm_barrier(batch);
        CALL(s, batch, K_POST_CONV_ABLATE_RES_NB, 1, a_int(d), a_int(nb), a_int(is_blk ? 0 : 1), a_int(0),
             a_f32v(s->o_raw, 0, bd), a_int((int64_t)(1 + ip) * d), a_ablate_dirs(s),
             a_tensor_f32(s, &L->attn_post_conv), a_conv_ao(s, il),
             a_f32v(s->cur, 0, bd), a_raw_ao(s, il), a_f32v(s->cur, 0, bd));
    } else if (s->fast_buf && s->fast_is_q8s && FL->attn_output_s.qs.off >= 0) {
        CALL(s, batch, K_DV_Q8S_CONV_NB, (d + 3) / 4, a_int(d), a_int(n_q), a_int(nb), a_int(is_blk ? 0 : 1),
             a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
             a_f32v(s->attn_out, 0, nb * n_q), a_tensor_f32(s, &L->attn_post_conv),
             a_conv_ao(s, il), a_f32v(s->cur, 0, bd), a_raw_ao(s, il), a_f32v(s->cur, 0, bd));
    } else if (FL->attn_output_s.qs.off >= 0 && !use_mma) {
        CALL(s, batch, K_DV_Q4S_CONV_NB, (d + 7) / 8, a_int(d), a_int(n_q), a_int(nb), a_int(is_blk ? 0 : 1),
             a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
             a_f32v(s->attn_out, 0, nb * n_q), a_tensor_f32(s, &L->attn_post_conv),
             a_conv_ao(s, il), a_f32v(s->cur, 0, bd), a_raw_ao(s, il), a_f32v(s->cur, 0, bd));
    } else {
        if (FL->attn_output_s.qs.off >= 0) {
            CALL(s, batch, K_DV_Q4S_MMA32_NB, (d + 31) / 32, a_int(d), a_int(n_q), a_int(nb),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_fast_u32(s, &FL->attn_output_s.qs), a_fast_u16(s, &FL->attn_output_s.ds),
                 a_f32v(s->attn_out, 0, nb * n_q), a_f32v(s->o_raw, 0, bd));
        } else {
            CALL(s, batch, K_DV_Q8_MMA32_NB, (d + 31) / 32, a_int(d), a_int(n_q), a_int(nb),
                 a_int(0), a_int(0), a_int(0), a_f32(0.0f),
                 a_tensor_u16(s, &L->attn_output),
                 a_f32v(s->attn_out, 0, nb * n_q), a_f32v(s->o_raw, 0, bd));
        }
        pm_barrier(batch);
        CALL(s, batch, K_POST_CONV_RES_NB, (d + 63) / 64, a_int(d), a_int(nb), a_int(is_blk ? 0 : 1), a_int(0),
             a_f32v(s->o_raw, 0, bd), a_tensor_f32(s, &L->attn_post_conv),
             a_conv_ao(s, il), a_f32v(s->cur, 0, bd), a_raw_ao(s, il), a_f32v(s->cur, 0, bd));
    }
    if (is_blk) (*n_blocks)++;
    pm_barrier(batch);

    /* Wave 7: FFN fused depth-attention mix (+ RMSNorm or ctrl_prep + deferred rms_parts!) */
    int64_t ffn_n_parts = 0;
    if (!has_messages) {
        CALL(s, batch, K_MIX_RMSNORM_NB, nb, a_int(d), a_int(*n_blocks), a_int(nb), a_f32(hp->rms_eps),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd), a_f32v(s->cur, 0, bd),
             a_tensor_f32(s, &L->ffn_res_query),
             a_tensor_f32(s, &L->ffn_norm), a_tensor_f32(s, &L->ffn_norm), a_tensor_f32(s, &L->ffn_norm),
             a_f32v(s->o_raw, 0, bd), a_f32v(s->o_raw, 0, bd), a_f32v(s->o_raw, 0, bd));
        pm_barrier(batch);
    } else {
        int64_t rank = hp->gate_rank;
        int64_t citems = (d + 31) / 32;
        ffn_n_parts = citems;
        CALL(s, batch, K_MIX_CTRL_PREP_NB, nb, a_int(d), a_int(*n_blocks), a_int(nb), a_int(0),
             a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd), a_f32v(s->cur, 0, bd),
             a_tensor_f32(s, &L->ffn_res_query),
             a_f32v(s->messages, msg_base + 3 * bd, bd),
             a_f32v(s->mix_out, 0, bd), a_f32v(s->ctrl_cat, 0, 2 * bd));
        pm_barrier(batch);
        if (FL->ctrl_down3_s.qs.off >= 0) {
            CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_SMALL_NB : K_DV_Q4S_SMALL_NB, rank, a_int(rank), a_int(rank), a_int(2 * d), a_int(nb),
                 a_int(0), a_f32(hp->rms_eps),
                 a_fast_u32(s, &FL->ctrl_down3_s.qs), a_fast_u16(s, &FL->ctrl_down3_s.ds),
                 a_f32v(s->ctrl_cat, 0, 2 * bd), a_f32v(s->dummy_f32, 0, 64), a_f32v(s->ctrl_gate, 0, nb * rank));
        } else {
            CALL(s, batch, K_DV_F16_SMALL_NB, rank, a_int(rank), a_int(rank), a_int(2 * d), a_int(nb),
                 a_fast_u16(s, &FL->ctrl_down[3]),
                 a_f32v(s->ctrl_cat, 0, 2 * bd), a_f32v(s->ctrl_gate, 0, nb * rank));
        }
        pm_barrier(batch);
        CALL(s, batch, K_CTRL_UP_APPLY_GF_RM_NB, citems, a_int(d), a_int(rank), a_int(nb),
             a_fast_u32(s, &FL->ctrl_gf_rm3), a_f32v(s->ctrl_gate, 0, nb * rank),
             a_f32v(s->mix_out, 0, bd), a_f32v(s->messages, msg_base + 3 * bd, bd),
             a_tensor_f32(s, &L->ffn_norm), a_f32v(s->o_raw, 0, bd),
             a_f32v(s->rms_parts, 0, 8 * 3 * 128));
        pm_barrier(batch);
        if (FL->ffn_gate_inp_s.qs.off < 0 ||
            (FL->moe_gu_q4s.qs.off < 0 && !(s->iq4s_buf && FL->moe_gu_iq4s.qs.off >= 0) && L->ffn_gate_up_exps.type != INK_GGML_IQ4_NL)) {
            CALL(s, batch, K_SCALE_RMS_PARTS_NB, nb, a_int(d), a_int(nb), a_int(citems), a_f32(hp->rms_eps),
                 a_f32v(s->rms_parts, 0, 8 * 3 * 128),
                 a_f32v(s->o_raw, 0, bd), a_f32v(s->o_raw, 0, bd), a_f32v(s->o_raw, 0, bd));
            pm_barrier(batch);
            ffn_n_parts = 0;
        }
    }

    /* Wave 8: Bit-exact 1-row/TG single-pass Router + O(1) ascending-eid unique-expert plan */
    int64_t n_ff = hp->n_ff_exp;
    int64_t gpe = (n_ff + 7) / 8;
    if (FL->ffn_gate_inp_s.qs.off >= 0) {
        CALL(s, batch, s->fast_is_q8s ? K_DV_Q8S_SMALL_NB : K_DV_Q4S_SMALL_NB, hp->n_expert, a_int(hp->n_expert), a_int(hp->n_expert), a_int(d), a_int(nb),
             a_int(ffn_n_parts), a_f32(hp->rms_eps),
             a_fast_u32(s, &FL->ffn_gate_inp_s.qs), a_fast_u16(s, &FL->ffn_gate_inp_s.ds),
             a_f32v(s->o_raw, 0, bd), a_f32v(s->rms_parts, 0, 8 * 3 * 128), a_f32v(s->router_logits, 0, nb * hp->n_expert));
    } else {
        CALL(s, batch, K_DV_F16_SMALL_NB, hp->n_expert, a_int(hp->n_expert), a_int(hp->n_expert), a_int(d), a_int(nb),
             a_fast_u16(s, &FL->ffn_gate_inp),
             a_f32v(s->o_raw, 0, bd), a_f32v(s->router_logits, 0, nb * hp->n_expert));
    }
    pm_barrier(batch);
    CALL(s, batch, K_MOE_PLAN_NB, 1, a_int(hp->n_expert), a_int(hp->n_expert_used), a_int(nb), a_int(gpe),
         a_int(L->ffn_exp_probs_b.present ? 1 : 0), a_f32(hp->expert_weights_scale),
         a_f32v(s->router_logits, 0, nb * hp->n_expert),
         L->ffn_exp_probs_b.present ? a_tensor_f32(s, &L->ffn_exp_probs_b) : a_f32v(s->dummy_f32, 0, 64),
         a_u32v(s->moe_ids, 0, nb * hp->n_expert_used), a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
         a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
         a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4));
    pm_barrier(batch);

    /* Wave 9: Indirect grouped unique-expert MoE GU (dequant-ONCE + aligned Q8S/Q4S/IQ4S + deferred rms_parts!) */
    int64_t max_u = nb * hp->n_expert_used;
    int64_t gu_max = max_u * gpe;
    if (FL->moe_gu_q4s.qs.off >= 0) {
        int k_gu = s->fast_is_q8s ? K_MOE_GU_Q8S_NB : K_MOE_GU_Q4S_NB;
        PmArg ga[] = {
            a_int(n_ff), a_int(d), a_int(nb), a_int(hp->n_expert_used),
            a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
            a_int(ffn_n_parts), a_f32(hp->rms_eps),
            a_fast_u32(s, &FL->moe_gu_q4s.qs), a_fast_u16(s, &FL->moe_gu_q4s.ds),
            a_f32v(s->o_raw, 0, bd), a_f32v(s->rms_parts, 0, 8 * 3 * 128),
            a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
            a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
            a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff)
        };
        if (pm_launch_indirect(batch, s->kernels[k_gu], gu_max, s->spec_u_cnt, 4, 128, ga, 17) != PM_OK) {
            snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[k_gu], pm_log());
            return 1;
        }
    } else if (s->iq4s_buf && FL->moe_gu_iq4s.qs.off >= 0) {
        PmArg ga[] = {
            a_int(n_ff), a_int(d), a_int(nb), a_int(hp->n_expert_used),
            a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
            a_int(ffn_n_parts), a_f32(hp->rms_eps),
            a_iq4s_u32(s, &FL->moe_gu_iq4s.qs), a_iq4s_u16(s, &FL->moe_gu_iq4s.ds),
            a_f32v(s->o_raw, 0, bd), a_f32v(s->rms_parts, 0, 8 * 3 * 128),
            a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
            a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
            a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff)
        };
        if (pm_launch_indirect(batch, s->kernels[K_MOE_GU_IQ4S_NB], gu_max, s->spec_u_cnt, 4, 128, ga, 17) != PM_OK) {
            snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[K_MOE_GU_IQ4S_NB], pm_log());
            return 1;
        }
    } else if (L->ffn_gate_up_exps.type == INK_GGML_Q8_0) {
        PmArg ga[] = {
            a_int(n_ff), a_int(d), a_int(nb), a_int(hp->n_expert_used),
            a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
            a_tensor_u16(s, &L->ffn_gate_up_exps),
            a_f32v(s->o_raw, 0, bd),
            a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
            a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
            a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff)
        };
        if (pm_launch_indirect(batch, s->kernels[K_MOE_GU_Q8_NB], gu_max, s->spec_u_cnt, 4, 128, ga, 13) != PM_OK) {
            snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[K_MOE_GU_Q8_NB], pm_log());
            return 1;
        }
    } else if (L->ffn_gate_up_exps.type == INK_GGML_IQ4_NL) {
        PmArg ga[] = {
            a_int(n_ff), a_int(d), a_int(nb), a_int(hp->n_expert_used),
            a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
            a_int(ffn_n_parts), a_f32(hp->rms_eps),
            a_tensor_u16(s, &L->ffn_gate_up_exps),
            a_f32v(s->o_raw, 0, bd), a_f32v(s->rms_parts, 0, 8 * 3 * 128),
            a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
            a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
            a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff)
        };
        if (pm_launch_indirect(batch, s->kernels[K_MOE_GU_IQ4_NB], gu_max, s->spec_u_cnt, 4, 128, ga, 16) != PM_OK) {
            snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[K_MOE_GU_IQ4_NB], pm_log());
            return 1;
        }
    } else {
        int64_t gu_rgrps = (n_ff + 3) / 4;
        CALL(s, batch, K_MOE_GU_GGML_NB, max_u * gu_rgrps, a_int(n_ff), a_int(d), a_int(nb), a_int(hp->n_expert_used),
             a_int((int64_t)L->ffn_gate_up_exps.type), a_f32(hp->situ_beta), a_f32(hp->situ_linear_beta),
             a_tensor_ggml_u16(s, &L->ffn_gate_up_exps), a_ggml_grids(s),
             a_f32v(s->o_raw, 0, bd),
             a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
             a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff));
    }
    pm_barrier(batch);

    /* Wave 10: 2-pass parallel-expert MoE Down (4.05x faster, 354+ GB/s, 100% bit-exact!) + L2 reduce/conv/residual! */
    int64_t dn_gpe = (d + 15) / 16;
    if (ablate_ip) {
        if (FL->moe_dn_q4s.qs.off >= 0 && !s->fast_is_q8s) {
            PmArg da[] = {
                a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
                a_fast_u32(s, &FL->moe_dn_q4s.qs), a_fast_u16(s, &FL->moe_dn_q4s.ds),
                a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
                a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
                a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d)
            };
            if (call_k(s, batch, K_MOE_DOWN_P1_Q4S_NB, max_u * dn_gpe, 12, da) != 0) return 1;
        } else if (FL->moe_dn_q4s.qs.off >= 0 && s->fast_is_q8s) {
            CALL(s, batch, K_MOE_DOWN_P1_Q8S_NB, max_u * ((d + 3) / 4), a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
                 a_fast_u32(s, &FL->moe_dn_q4s.qs), a_fast_u16(s, &FL->moe_dn_q4s.ds),
                 a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
                 a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                 a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
                 a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d));
        } else if (s->iq4s_buf && FL->moe_dn_iq4s.qs.off >= 0) {
            PmArg da[] = {
                a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
                a_iq4s_u32(s, &FL->moe_dn_iq4s.qs), a_iq4s_u16(s, &FL->moe_dn_iq4s.ds),
                a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
                a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
                a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d)
            };
            if (call_k(s, batch, K_MOE_DOWN_P1_IQ4S_NB, max_u * dn_gpe, 12, da) != 0) return 1;
        } else if (L->ffn_down_exps.type == INK_GGML_Q8_0) {
            PmArg da[] = {
                a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
                a_tensor_u16(s, &L->ffn_down_exps),
                a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
                a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
                a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d)
            };
            if (call_k(s, batch, K_MOE_DOWN_P1_Q8_NB, max_u * dn_gpe, 11, da) != 0) return 1;
        } else {
            CALL(s, batch, K_MOE_DOWN_P1_GGML_NB, max_u * ((d + 3) / 4), a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
                 a_int((int64_t)L->ffn_down_exps.type), a_tensor_ggml_u16(s, &L->ffn_down_exps), a_ggml_grids(s),
                 a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
                 a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
                 a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
                 a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d));
        }
        pm_barrier(batch);
        CALL(s, batch, K_MOE_SUM_ABLATE_CONV_NB, 1, a_int(d), a_int(nb), a_int(hp->n_expert_used), a_int(0),
             a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d),
             a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
             a_int((int64_t)(1 + ip) * d), a_ablate_dirs(s),
             a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
             a_f32v(s->cur, 0, bd), a_raw_fo(s, il), a_f32v(s->cur, 0, bd));
    } else if (FL->moe_dn_q4s.qs.off >= 0 && !s->fast_is_q8s) {
        PmArg da[] = {
            a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
            a_fast_u32(s, &FL->moe_dn_q4s.qs), a_fast_u16(s, &FL->moe_dn_q4s.ds),
            a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
            a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
            a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
            a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d)
        };
        if (dn_gpe == gpe) {
            if (pm_launch_indirect(batch, s->kernels[K_MOE_DOWN_P1_Q4S_NB], max_u * dn_gpe, s->spec_u_cnt, 4, 128, da, 12) != PM_OK) {
                snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[K_MOE_DOWN_P1_Q4S_NB], pm_log());
                return 1;
            }
        } else {
            if (call_k(s, batch, K_MOE_DOWN_P1_Q4S_NB, max_u * dn_gpe, 12, da) != 0) return 1;
        }
        pm_barrier(batch);
        CALL(s, batch, K_MOE_DOWN_P2_NB, (d + 63) / 64, a_int(d), a_int(nb), a_int(hp->n_expert_used), a_int(0),
             a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d),
             a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
             a_u32v(s->spec_u_mask, 0, 64), a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
             a_f32v(s->cur, 0, bd), a_raw_fo(s, il), a_f32v(s->cur, 0, bd));
    } else if (FL->moe_dn_q4s.qs.off >= 0 && s->fast_is_q8s) {
        CALL(s, batch, K_MOE_DOWN_Q8S_NB, (d + 3) / 4, a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
             a_fast_u32(s, &FL->moe_dn_q4s.qs), a_fast_u16(s, &FL->moe_dn_q4s.ds),
             a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
             a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
             a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
             a_f32v(s->cur, 0, bd), a_raw_fo(s, il), a_f32v(s->cur, 0, bd));
    } else if (s->iq4s_buf && FL->moe_dn_iq4s.qs.off >= 0) {
        PmArg da[] = {
            a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
            a_iq4s_u32(s, &FL->moe_dn_iq4s.qs), a_iq4s_u16(s, &FL->moe_dn_iq4s.ds),
            a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
            a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
            a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
            a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d)
        };
        if (dn_gpe == gpe) {
            if (pm_launch_indirect(batch, s->kernels[K_MOE_DOWN_P1_IQ4S_NB], max_u * dn_gpe, s->spec_u_cnt, 4, 128, da, 12) != PM_OK) {
                snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[K_MOE_DOWN_P1_IQ4S_NB], pm_log());
                return 1;
            }
        } else {
            if (call_k(s, batch, K_MOE_DOWN_P1_IQ4S_NB, max_u * dn_gpe, 12, da) != 0) return 1;
        }
        pm_barrier(batch);
        CALL(s, batch, K_MOE_DOWN_P2_NB, (d + 63) / 64, a_int(d), a_int(nb), a_int(hp->n_expert_used), a_int(0),
             a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d),
             a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
             a_u32v(s->spec_u_mask, 0, 64), a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
             a_f32v(s->cur, 0, bd), a_raw_fo(s, il), a_f32v(s->cur, 0, bd));
    } else if (L->ffn_down_exps.type == INK_GGML_Q8_0) {
        PmArg da[] = {
            a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
            a_tensor_u16(s, &L->ffn_down_exps),
            a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
            a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
            a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
            a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d)
        };
        if (dn_gpe == gpe) {
            if (pm_launch_indirect(batch, s->kernels[K_MOE_DOWN_P1_Q8_NB], max_u * dn_gpe, s->spec_u_cnt, 4, 128, da, 11) != PM_OK) {
                snprintf(s->error, sizeof s->error, "kernel %s: %s", k_names[K_MOE_DOWN_P1_Q8_NB], pm_log());
                return 1;
            }
        } else {
            if (call_k(s, batch, K_MOE_DOWN_P1_Q8_NB, max_u * dn_gpe, 11, da) != 0) return 1;
        }
        pm_barrier(batch);
        CALL(s, batch, K_MOE_DOWN_P2_NB, (d + 63) / 64, a_int(d), a_int(nb), a_int(hp->n_expert_used), a_int(0),
             a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d),
             a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
             a_u32v(s->spec_u_mask, 0, 64), a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
             a_f32v(s->cur, 0, bd), a_raw_fo(s, il), a_f32v(s->cur, 0, bd));
    } else if (L->ffn_down_exps.type == INK_GGML_IQ4_NL) {
        CALL(s, batch, K_MOE_DOWN_IQ4_NB, (d + 3) / 4, a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
             a_tensor_u16(s, &L->ffn_down_exps),
             a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
             a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
             a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
             a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
             a_f32v(s->cur, 0, bd), a_raw_fo(s, il), a_f32v(s->cur, 0, bd));
    } else {
        CALL(s, batch, K_MOE_DOWN_P1_GGML_NB, max_u * ((d + 3) / 4), a_int(d), a_int(n_ff), a_int(nb), a_int(hp->n_expert_used),
             a_int((int64_t)L->ffn_down_exps.type), a_tensor_ggml_u16(s, &L->ffn_down_exps), a_ggml_grids(s),
             a_f32v(s->moe_hid, 0, nb * hp->n_expert_used * n_ff),
             a_u32v(s->spec_u_eids, 0, 64), a_u32v(s->spec_u_mask, 0, 64),
             a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d));
        pm_barrier(batch);
        CALL(s, batch, K_MOE_DOWN_P2_NB, (d + 63) / 64, a_int(d), a_int(nb), a_int(hp->n_expert_used), a_int(0),
             a_f32v(s->moe_slot_out, 0, nb * hp->n_expert_used * d),
             a_f32v(s->moe_wts, 0, nb * hp->n_expert_used),
             a_u32v(s->spec_u_mask, 0, 64), a_u32v(s->spec_u_slot, 0, 64 * 8), a_u32v(s->spec_u_cnt, 0, 4),
             a_tensor_f32(s, &L->ffn_post_conv), a_conv_fo(s, il),
             a_f32v(s->cur, 0, bd), a_raw_fo(s, il), a_f32v(s->cur, 0, bd));
    }
    pm_barrier(batch);
    return 0;
}

static int ink_verify_batch(InkSession *s, const int32_t *tokens, int nb) {
    const InkHParams *hp = &s->gguf.hp;
    int64_t d = hp->n_embd;
    int64_t bd = (int64_t)nb * d;
    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;

    uint32_t *tids = (uint32_t *)pm_contents(s->tok_ids_buf);
    float *taus = (float *)pm_contents(s->tau_buf);
    for (int t = 0; t < nb; t++) {
        tids[t] = (uint32_t)tokens[t];
        float ratio = (float)(s->pos + t + 1) / (float)hp->log_scale_floor;
        if (ratio < 1.0f) ratio = 1.0f;
        taus[t] = 1.0f + hp->log_scale_alpha * logf(ratio);
    }

    PmBatch batch = NULL;
    if (pm_begin_concurrent(&batch) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_begin_concurrent: %s", pm_log());
        return 1;
    }
    /* Lazy commit of any previous pending verify raw log + DSpark taps in the same command buffer! */
    if (s->conv_pending_n > 0) {
        if (ink_enqueue_conv_commit(s, batch) != 0) { pm_abort(batch); return 1; }
    }
    int cap_taps = (s->dspark.ready && s->dspark.pos + s->dspark.pending_got == s->pos);
    if (cap_taps && s->dspark.pending_got > 0) {
        if (ink_dspark_flush_pending(s, batch) != 0) { pm_abort(batch); return 1; }
    }

    /* 1. Batched embedding lookup */
    if (s->gguf.tok_embd.type == INK_GGML_Q8_0 && !s->ablate_has_emb) {
        CALL(s, batch, K_EMBED_Q8_BATCH, nb, a_int(d),
             a_u32v(s->tok_ids_buf, 0, nb),
             a_tensor_u16(s, &s->gguf.tok_embd),
             a_tensor_f32(s, &s->gguf.tok_norm),
             a_f32(hp->rms_eps),
             a_f32v(s->cur, 0, bd));
    } else {
        CALL(s, batch, K_EMBED_GGML_BATCH, nb, a_int(d), a_int((int64_t)s->gguf.tok_embd.type),
             a_u32v(s->tok_ids_buf, 0, nb),
             a_tensor_ggml_u16(s, &s->gguf.tok_embd), a_ggml_grids(s),
             a_tensor_f32(s, &s->gguf.tok_norm), a_f32(hp->rms_eps),
             a_int(s->ablate_has_emb ? 1 : 0), a_ablate_dirs(s),
             a_f32v(s->cur, 0, bd));
    }
    pm_barrier(batch);

    int n_blocks = 0;
    int start = (int)hp->loop_start;
    int end = (int)hp->loop_end;

    /* 2. Prelude layers [0 .. start) */
    for (int il = 0; il < start; il++) {
        if (cap_taps && il == 5) {
            CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
                 a_f32v(s->cur, 0, bd), a_f32v(s->dspark.taps, 0, bd));
            pm_barrier(batch);
        }
        if (dispatch_layer_nb(s, batch, il, il, &n_blocks, 0, nb) != 0) { pm_abort(batch); return 1; }
    }
    /* Save prelude output & blocks */
    CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->cur, 0, bd), a_f32v(s->prelude, 0, bd));
    int prelude_n_blocks = n_blocks;
    if (prelude_n_blocks > 0) {
        int64_t pn = (int64_t)prelude_n_blocks * bd;
        CALL(s, batch, K_COPY, (pn + INK_NT - 1) / INK_NT, a_int(pn),
             a_f32v(s->blocks, 0, pn), a_f32v(s->prelude_blk, 0, pn));
    }
    pm_barrier(batch);

    /* 3. Looped core passes [0 .. loop_count) */
    for (int pass = 0; pass < hp->loop_count; pass++) {
        if (pass > 0) {
            int src_off = prelude_n_blocks + 1;
            int bank_n_blocks = n_blocks - src_off;
            if (bank_n_blocks < 0) bank_n_blocks = 0;
            if (bank_n_blocks > 0) {
                int64_t bn = (int64_t)bank_n_blocks * bd;
                CALL(s, batch, K_COPY, (bn + INK_NT - 1) / INK_NT, a_int(bn),
                     a_f32v(s->blocks, (int64_t)src_off * bd, bn), a_f32v(s->bank_blk, 0, bn));
            }
            CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->cur, 0, bd), a_f32v(s->bank_out, 0, bd));
            pm_barrier(batch);
            CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd), a_f32v(s->prelude, 0, bd), a_f32v(s->cur, 0, bd));
            n_blocks = prelude_n_blocks;
            if (prelude_n_blocks > 0) {
                int64_t pn = (int64_t)prelude_n_blocks * bd;
                CALL(s, batch, K_COPY, (pn + INK_NT - 1) / INK_NT, a_int(pn),
                     a_f32v(s->prelude_blk, 0, pn), a_f32v(s->blocks, 0, pn));
            }
            /* Precompute ALL 12 looped layers' messages concurrently via bit-exact am_mix_fused_nb! */
            for (int ip = start; ip < end; ip++) {
                const InkLayerTensors *ML = &s->gguf.layers[ip];
                CALL(s, batch, K_MIX_CTRL_PREP_NB, 4 * nb, a_int(d), a_int(bank_n_blocks), a_int(nb), a_int(0),
                     a_f32v(s->bank_blk, 0, INK_MAX_BLOCKS * bd),
                     a_f32v(s->bank_out, 0, bd),
                     a_tensor_f32(s, &ML->loop_msg_query),
                     a_f32v(s->mix_out, 0, 4 * bd),
                     a_f32v(s->messages, (int64_t)(ip - start) * 4 * bd, 4 * bd),
                     a_f32v(s->ctrl_cat, (int64_t)(ip - start) * 8 * bd, 8 * bd));
            }
            pm_barrier(batch);
        }
        for (int ip = start; ip < end; ip++) {
            if (cap_taps && pass == 1 && ip == 11) {
                CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
                     a_f32v(s->cur, 0, bd), a_f32v(s->dspark.taps, 1 * bd, bd));
                pm_barrier(batch);
            } else if (cap_taps && pass == 2 && ip == 17) {
                CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
                     a_f32v(s->cur, 0, bd), a_f32v(s->dspark.taps, 2 * bd, bd));
                pm_barrier(batch);
            }
            int il = (pass == 0) ? ip : (int)hp->n_phys + (pass - 1) * (end - start) + (ip - start);
            if (dispatch_layer_nb(s, batch, il, ip, &n_blocks, pass > 0 ? 1 : 0, nb) != 0) { pm_abort(batch); return 1; }
        }
    }

    /* 4. Coda layers [end .. n_phys) */
    for (int il = end; il < hp->n_phys; il++) {
        if (dispatch_layer_nb(s, batch, il, il, &n_blocks, 0, nb) != 0) { pm_abort(batch); return 1; }
    }

    /* 5. Fused Output mix+norm -> lm_head_nb (259 MB read ONCE for all nb tokens!) -> concurrent GPU argmax */
    CALL(s, batch, K_MIX_RMSNORM_NB, nb, a_int(d), a_int(n_blocks), a_int(nb), a_f32(hp->rms_eps),
         a_f32v(s->blocks, 0, INK_MAX_BLOCKS * bd), a_f32v(s->cur, 0, bd),
         a_tensor_f32(s, &s->gguf.out_res_query),
         a_tensor_f32(s, &s->gguf.out_norm), a_tensor_f32(s, &s->gguf.out_norm), a_tensor_f32(s, &s->gguf.out_norm),
         a_f32v(s->spec_hidden, 0, bd), a_f32v(s->spec_hidden, 0, bd), a_f32v(s->spec_hidden, 0, bd));
    pm_barrier(batch);
    if (cap_taps) {
        CALL(s, batch, K_COPY, (bd + INK_NT - 1) / INK_NT, a_int(bd),
             a_f32v(s->spec_hidden, 0, bd), a_f32v(s->dspark.taps, 3 * bd, bd));
    }
    if (s->fast_output_s.qs.off >= 0 && s->fast_output_is_q4s && getenv("INK_NO_MMA32") == NULL) {
        CALL(s, batch, K_DV_Q4S_MMA32_NB, (nv + 31) / 32, a_int(nv), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(hp->logit_softcap),
             a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
             a_f32v(s->spec_hidden, 0, bd), a_f32v(s->spec_logits, 0, nb * nv));
    } else if (s->fast_output_s.qs.off >= 0) {
        CALL(s, batch, s->fast_output_is_q4s ? K_DV_Q4S_NB : K_DV_Q8S_NB, (nv + 3) / 4, a_int(nv), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(hp->logit_softcap),
             a_fast_u32(s, &s->fast_output_s.qs), a_fast_u16(s, &s->fast_output_s.ds),
             a_f32v(s->spec_hidden, 0, bd), a_f32v(s->spec_logits, 0, nb * nv));
    } else {
        CALL(s, batch, K_DV_Q8_MMA32_NB, (nv + 31) / 32, a_int(nv), a_int(d), a_int(nb),
             a_int(0), a_int(0), a_int(0), a_f32(hp->logit_softcap),
             a_tensor_u16(s, &s->gguf.output),
             a_f32v(s->spec_hidden, 0, bd), a_f32v(s->spec_logits, 0, nb * nv));
    }
    pm_barrier(batch);
    for (int t = 0; t < nb; t++) {
        CALL(s, batch, K_TOPK64_S1, 32, a_int(32), a_int(nv),
             a_f32v(s->spec_logits, (int64_t)t * nv, nv),
             a_f32v(s->topk_s1_vals, (int64_t)t * 32 * 64, 32 * 64),
             a_u32v(s->topk_s1_ids, (int64_t)t * 32 * 64, 32 * 64));
    }
    pm_barrier(batch);
    for (int t = 0; t < nb; t++) {
        CALL(s, batch, K_TOPK64_S2, 1, a_int(32 * 64),
             a_f32v(s->topk_s1_vals, (int64_t)t * 32 * 64, 32 * 64),
             a_u32v(s->topk_s1_ids, (int64_t)t * 32 * 64, 32 * 64),
             a_f32v(s->spec_topk_vals, (int64_t)t * 64, 64),
             a_u32v(s->spec_topk_ids, (int64_t)t * 64, 64),
             a_u32v(s->spec_argmax_out, t, 1));
    }

    double gpu_s = 0.0;
    if (pm_wait(batch, &gpu_s) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_wait verify_batch: %s", pm_log());
        return 1;
    }
    s->total_gpu_seconds += gpu_s;
    s->dspark.last_verify_s = gpu_s;
    s->last_verify_ms = (float)(gpu_s * 1000.0);
    if (nb >= 2 && nb <= 8 && s->last_verify_ms > 0.5f && s->last_verify_ms < 500.0f && s->ema_base_ms > 1.0f) {
        float vr = s->last_verify_ms / s->ema_base_ms;
        if (vr < 1.0f) vr = 1.0f;
        if (vr > 6.0f) vr = 6.0f;
        s->ema_vratio[nb] = 0.70f * s->ema_vratio[nb] + 0.30f * vr;
    }
    ink_mark_written(s, s->pos + nb);
    return 0;
}

void ink_set_prediction(InkSession *s, const int32_t *tokens, int n_tokens) {
    if (!s) return;
    if (!tokens || n_tokens <= 0) {
        s->pred_len = 0;
        return;
    }
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

static int pld_search_buf(const int32_t *ctx, int ctx_len, int32_t cur_tok,
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

/* Check whether the last 2 tokens [h[len-2], h[len-1]] occurred earlier (needed before skipping GPU chain). */
static int ink_pld_has_bigram(const int32_t *h, int len) {
    if (!h || len < 3) return 0;
    int32_t a = h[len - 2], b = h[len - 1];
    int lo = len - 4096; if (lo < 0) lo = 0;
    for (int j = len - 3; j >= lo; j--) {
        if (h[j] == a && h[j + 1] == b) return 1;
    }
    return 0;
}

/* Zero-cost adaptive longest-suffix n-gram drafter over prompt + generated history (returns M in [0..7]). */
static int ink_pld_draft(const InkSession *s, int32_t cur_tok, int32_t *out_draft, int *out_match_n) {
    *out_match_n = 0;
    if (!s->tok_hist || s->pos < 2 || s->pos + 2 >= s->max_ctx) return 0;
    const int32_t *h = s->tok_hist;
    int len = s->pos + 1; /* h[0..s->pos-1] already stored, cur_tok is at index s->pos */
    int min_n = (s->pld_ema2 >= 0.25f) ? 2 : 3;
    const int32_t *src = NULL;
    int src_len = 0, found = -1, match_n = 0;

    /* 1. Explicit target-file prediction buffer (Cursor Instant Apply / OpenAI Predicted Outputs) */
    if (s->pred_toks && s->pred_len >= 4) {
        int pn = pld_search_buf(h, len, cur_tok, s->pred_toks, s->pred_len, 3, 10, &found);
        if (pn >= 3) { src = s->pred_toks; src_len = s->pred_len; match_n = pn; }
    }
    /* 2. Current prompt + output history (zero-cost PLD) */
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
    /* 3. Cross-turn session output history (SuffixDecoding-lite: recurring tool JSON & Coax idioms) */
    if (!src && s->sess_hist && s->sess_hist_len >= 5) {
        int sn = pld_search_buf(h, len, cur_tok, s->sess_hist, s->sess_hist_len, 4, 10, &found);
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

int ink_prefill(InkSession *s, const int32_t *tokens, int n_tokens, int compute_last_logits) {
    if (n_tokens <= 0) return 0;
    if (n_tokens == 1) return ink_step(s, tokens[0], compute_last_logits);
    ink_cancel_inflight(s);
    ink_flush_spec(s);
    int hist_start = s->pos;
    if (s->tok_hist && hist_start + n_tokens <= s->max_ctx) {
        memcpy(s->tok_hist + hist_start, tokens, (size_t)n_tokens * sizeof(int32_t));
        s->tok_hist_len = hist_start + n_tokens;
    }
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
        if (enqueue_step(s, &batch, tokens[i], NULL, 0, 0, need_logits, 1) != 0) {
            pm_wait(batch, NULL);
            return 1;
        }
    }
    double gpu_s = 0.0;
    if (pm_wait(batch, &gpu_s) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_wait: %s", pm_log());
        return 1;
    }
    if (compute_last_logits) {
        s->prev_argmax = (int32_t)((const uint32_t *)pm_contents(s->argmax_out_buf[0]))[0];
        s->has_prev_argmax = 1;
    } else {
        s->has_prev_argmax = 0;
    }
    s->greedy_streak = 0;
    s->total_gpu_seconds += gpu_s;
    return 0;
}

static void ink_record_spec_outcome(InkSession *s, int used_dspark, int m, int match_n, int n_acc, float pred_e, float cost_ms, int pos0) {
    float t_base = s->ema_base_ms > 1.0f ? s->ema_base_ms : 10.5f;
    if (used_dspark) {
        s->dspark.rounds++;
        s->dspark.drafted += m;
        s->dspark.accepted += n_acc;
        s->dspark.ema_acc = 0.75f * s->dspark.ema_acc + 0.25f * (float)n_acc;
        if (pred_e > 0.20f) {
            float obs_cal = (float)n_acc / pred_e;
            if (obs_cal < 0.30f) obs_cal = 0.30f;
            if (obs_cal > 1.05f) obs_cal = 1.05f;
            s->dspark.ema_cal = 0.75f * s->dspark.ema_cal + 0.25f * obs_cal;
            if (s->dspark.ema_cal < 0.35f) s->dspark.ema_cal = 0.35f;
            if (s->dspark.ema_cal > 1.00f) s->dspark.ema_cal = 1.00f;
        }
        float net = (cost_ms > 0.5f) ? ((1.0f + (float)n_acc) * t_base / cost_ms) : 1.0f;
        s->dspark.ema_net_speedup = 0.65f * s->dspark.ema_net_speedup + 0.35f * net;
        if (net >= 1.03f) {
            s->dspark.consec_unprof = 0;
            s->dspark.skip_countdown = 0;
        } else {
            s->dspark.consec_unprof++;
            if (s->dspark.consec_unprof >= 2 || (s->dspark.rounds >= 2 && s->dspark.ema_net_speedup < 1.00f)) {
                int sh = s->dspark.consec_unprof - 1;
                if (sh > 2) sh = 2;
                s->dspark.skip_countdown = 12 << sh;
            }
        }
        if (getenv("INK_DSPARK_TRACE") != NULL) {
            fprintf(stderr, "[DSPARK pos=%d m=%d acc=%d pred=%.2f cal=%.2f cost=%.1fms base=%.1fms net=%.2fx skip=%d dr=%.2fms]\n",
                    pos0, m, n_acc, (double)pred_e, (double)s->dspark.ema_cal, (double)cost_ms, (double)t_base,
                    (double)net, s->dspark.skip_countdown,
                    s->dspark.draft_gpu_s * 1000.0 / (double)s->dspark.rounds);
        }
    } else if (m > 0) {
        float rate = (float)n_acc / (float)m;
        if (match_n == 2) s->pld_ema2 = 0.80f * s->pld_ema2 + 0.20f * rate;
        else if (match_n == 3) s->pld_ema3 = 0.80f * s->pld_ema3 + 0.20f * rate;
    }
    s->spec_rounds++;
    s->spec_drafted += m;
    s->spec_accepted += n_acc;
}

static void ink_finish_spec_round(InkSession *s) {
    if (!s || !s->spec_round_open) return;
    s->spec_round_open = 0;
    ink_record_spec_outcome(s, s->spec_round_dspark, s->spec_round_m, s->spec_round_match_n,
                            s->spec_round_hits, s->spec_round_pred, s->spec_round_cost_ms, s->pos);
}

int ink_step(InkSession *s, int32_t token_id, int compute_logits) {
    const InkHParams *hp = &s->gguf.hp;
    if (s->pos >= s->max_ctx) {
        snprintf(s->error, sizeof s->error, "context full (%d/%d)", s->pos, s->max_ctx);
        return 1;
    }
    if (token_id < 0 || token_id >= hp->n_vocab) {
        snprintf(s->error, sizeof s->error, "token id %d out of range [0,%lld)", token_id, (long long)hp->n_vocab);
        return 1;
    }
    /* Sprint 71 + Sprint D/E: 100% exact speculative queue HIT (0.000 ms, greedy AND sampled!) */
    if (s->spec_head < s->spec_tail && compute_logits && token_id == s->spec_q[s->spec_head]) {
        int idx = s->spec_head++;
        ink_sess_hist_push(s, token_id);
        if (s->spec_round_open) s->spec_round_hits++;
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
        s->hidden_is_prefill = 0;
        s->argmax_valid = 1;
        s->prev_argmax = s->spec_argmax[idx];
        s->has_prev_argmax = 1;
        if (!s->sampled_mode) s->greedy_streak++;
        if (s->spec_head == s->spec_tail && s->spec_round_open) ink_finish_spec_round(s);
        return 0;
    }
    /* Spec queue empty or divergence: settle sampled telemetry & drop unconsumed slots. */
    if (s->spec_round_open) ink_finish_spec_round(s);
    s->spec_head = 0;
    s->spec_tail = 0;
    s->spec_active_idx = -1;

    int use_pipe = (pm_is_m4() && getenv("INK_PROFILE") == NULL && getenv("PM_PASS_PROF") == NULL);
    int allow_chain = (use_pipe && getenv("INK_NO_CHAIN") == NULL);
    int allow_spec = (use_pipe && s->can_verify_nb && compute_logits && getenv("INK_NO_SPEC") == NULL);

    /* Sprint E (Ch 102): Online-timed E(k)/c(k) Hybrid PLD + DSpark + Bit-Exact B=2..8 Verifier! */
    if (allow_spec && !s->inflight_active) {
        int32_t draft[7];
        float ds_probs[7] = {0};
        int match_n = 0;
        int m = ink_pld_draft(s, token_id, draft, &match_n);
        int used_dspark = 0;
        float pred_e = 0.0f;
        float t_base = s->ema_base_ms > 1.0f ? s->ema_base_ms : 10.5f;

        /* Filter low-confidence PLD matches via Ch 102 E(k)/vratio(k+1) > 1.05 */
        if (m >= 1 && match_n < 4) {
            float p_pld = (match_n == 2) ? s->pld_ema2 : s->pld_ema3;
            if (s->sampled_mode) p_pld *= 0.80f;
            int best_k = 0;
            float best_gain = 1.05f, cum = 1.0f, ek = 0.0f;
            for (int k = 1; k <= m; k++) {
                cum *= p_pld;
                ek += cum;
                float g = (1.0f + ek) / s->ema_vratio[k + 1];
                if (g > best_gain) { best_gain = g; best_k = k; pred_e = ek; }
            }
            m = best_k;
        } else if (m >= 1) {
            float p_hi = s->sampled_mode ? 0.72f : 0.85f;
            float cum = 1.0f, ek = 0.0f;
            int best_k = 0;
            float best_gain = 1.03f;
            for (int k = 1; k <= m; k++) {
                cum *= p_hi;
                ek += cum;
                float g = (1.0f + ek) / s->ema_vratio[k + 1];
                if (g > best_gain) { best_gain = g; best_k = k; pred_e = ek; }
            }
            m = best_k;
        }

        /* Prefer PLD only on high-confidence >=4-gram verbatim matches (0.0 ms draft); otherwise run DSpark! */
        if (!(m >= 1 && match_n >= 4) &&
            s->dspark.ready && !s->dspark.req_disabled &&
            s->dspark.pos + s->dspark.pending_got == s->pos &&
            s->pos >= 1 && s->pos + 1 < s->max_ctx && s->dspark.skip_countdown == 0) {
            int max_c = s->max_ctx - s->pos - 1;
            const char *k_env = getenv("INK_DSPARK_K");
            int k_cap = (k_env && atoi(k_env) >= 1 && atoi(k_env) <= 7) ? atoi(k_env)
                      : (s->dspark.ema_acc >= 3.0f ? 7 : 5);
            if (max_c > k_cap) max_c = k_cap;
            int32_t ds_toks[7];
            if (ink_dspark_draft(s, token_id, max_c, ds_toks, ds_probs) == 0) {
                /* Ch 102 exact optimal k* = argmax_{k in 0..max_c} (1 + E(k)) / t_verify_est(k + 1) */
                float cal = s->dspark.ema_cal * (s->sampled_mode ? 0.82f : 1.0f);
                if (cal < 0.30f) cal = 0.30f;
                if (cal > 1.00f) cal = 1.00f;
                int best_k = 0;
                float best_rate = (1.0f / t_base) * 1.03f;
                float best_e = 0.0f;
                float cum = 1.0f, ek = 0.0f;
                for (int k = 1; k <= max_c; k++) {
                    float pk = cal * ds_probs[k - 1];
                    if (pk > 0.98f) pk = 0.98f;
                    cum *= pk;
                    ek += cum;
                    float t_ver = t_base * s->ema_vratio[k + 1];
                    float rate = (1.0f + ek) / t_ver;
                    if (rate > best_rate) {
                        best_rate = rate;
                        best_k = k;
                        best_e = ek;
                    }
                }
                if (best_k >= 1) {
                    for (int k = 0; k < best_k; k++) draft[k] = ds_toks[k];
                    m = best_k;
                    pred_e = best_e;
                    used_dspark = 1;
                } else {
                    /* Unprofitable draft: skip verify completely (save ~25-45 ms!) and update Cascade + Ch 102 gates */
                    s->dspark.req_rounds++;
                    double b1 = s->dspark.b1_ema_s > 1e-4 ? s->dspark.b1_ema_s : 0.015;
                    double denom = s->dspark.last_draft_s + b1;
                    float util = (denom > 1e-6) ? (float)(b1 / denom) : 0.8f;
                    s->dspark.util_ema = 0.65f * s->dspark.util_ema + 0.35f * util;
                    if (s->dspark.req_rounds >= 3 && s->dspark.util_ema < 0.96f) {
                        s->dspark.req_disabled = 1;
                    }
                    ink_record_spec_outcome(s, 1, 0, 0, 0, 0.0f, s->dspark.last_draft_ms + t_base, s->pos);
                    m = 0;
                }
            }
        }

        if (m >= 1) {
            ink_discard_prep(s);
            int32_t batch_toks[8];
            batch_toks[0] = token_id;
            for (int k = 0; k < m; k++) batch_toks[k + 1] = draft[k];
            int nb = m + 1;
            int pos0 = s->pos;
            if (ink_verify_batch(s, batch_toks, nb) != 0) return 1;
            const uint32_t *am = (const uint32_t *)pm_contents(s->spec_argmax_out);
            float round_cost_ms = (used_dspark ? s->dspark.last_draft_ms : 0.0f) + s->last_verify_ms;

            int q_end = m;
            if (!s->sampled_mode) {
                int n_acc = 0;
                for (int k = 0; k < m; k++) {
                    if ((int32_t)am[k] == draft[k]) n_acc++;
                    else break;
                }
                q_end = n_acc;
                if (used_dspark) {
                    s->dspark.req_rounds++;
                    double b1 = s->dspark.b1_ema_s > 1e-4 ? s->dspark.b1_ema_s : 0.015;
                    double round_s = s->dspark.last_draft_s + s->dspark.last_verify_s;
                    float util = (round_s > 1e-6) ? (float)((double)(1 + n_acc) * b1 / round_s) : 1.0f;
                    s->dspark.util_ema = 0.65f * s->dspark.util_ema + 0.35f * util;
                    if (util >= 1.05f) {
                        s->dspark.skip_mult = 1;
                    } else if (s->dspark.req_rounds >= 3 && s->dspark.util_ema < 0.96f) {
                        s->dspark.req_disabled = 1;
                    }
                }
                ink_record_spec_outcome(s, used_dspark, m, match_n, n_acc, pred_e, round_cost_ms, pos0);
            } else {
                /* Sampled mode: keep all nb verified target Top-64 slots; settle telemetry on miss/drain! */
                s->spec_round_open = 1;
                s->spec_round_dspark = used_dspark;
                s->spec_round_m = m;
                s->spec_round_match_n = match_n;
                s->spec_round_hits = 0;
                s->spec_round_pred = pred_e;
                s->spec_round_cost_ms = round_cost_ms;
            }

            /* Record token 0 in history & advance pos by 1; queue verified slots 1..q_end for instant 0.000ms hits! */
            ink_sess_hist_push(s, token_id);
            if (s->tok_hist && pos0 < s->max_ctx) {
                s->tok_hist[pos0] = token_id;
                if (pos0 + 1 > s->tok_hist_len) s->tok_hist_len = pos0 + 1;
            }
            if (s->live_tokens && pos0 < s->max_ctx) {
                s->live_tokens[pos0] = token_id;
                s->live_len = pos0 + 1;
            }
            for (int t = 0; t <= q_end; t++) {
                s->spec_q[t] = batch_toks[t];
                s->spec_argmax[t] = (int32_t)am[t];
            }
            s->spec_head = 1;
            s->spec_tail = 1 + q_end;
            s->spec_active_idx = 0;
            s->conv_pending_pos0 = pos0;
            s->conv_pending_n = 1;
            if (s->dspark.ready && s->dspark.pos == pos0) {
                s->dspark.pending_pos0 = pos0;
                s->dspark.pending_nb = nb;
                s->dspark.pending_got = 1;
            }
            s->pos = pos0 + 1;
            s->total_steps++;
            s->hidden_is_prefill = 0;
            s->argmax_valid = 1;
            s->prev_argmax = (int32_t)am[0];
            s->has_prev_argmax = 1;
            if (!s->sampled_mode) s->greedy_streak++;
            return 0;
        }
    }

    /* Record token_id in history for future PLD matches */
    ink_sess_hist_push(s, token_id);
    if (s->dspark.ready && s->dspark.skip_countdown > 0) s->dspark.skip_countdown--;
    if (s->tok_hist && s->pos < s->max_ctx) {
        s->tok_hist[s->pos] = token_id;
        if (s->pos + 1 > s->tok_hist_len) s->tok_hist_len = s->pos + 1;
    }

    /* Sprint 69 + Sprint D: 2-in-flight GPU-to-GPU chained greedy execution (or zero-jitter prep_batch in sampled_mode!) */
    if (use_pipe) {
        int is_greedy_tok = (!s->sampled_mode && s->has_prev_argmax && token_id == s->prev_argmax);
        uint64_t cur_ticket = 0;
        int cur_slot = 0;
        if (s->inflight_active && s->inflight_pos == s->pos &&
            s->inflight_logits == compute_logits && token_id == s->inflight_expected_tok) {
            /* HIT: step t is already executing on GPU in s->inflight_slot! */
            cur_ticket = s->inflight_ticket;
            cur_slot = s->inflight_slot;
            s->inflight_active = 0;
            if (!s->sampled_mode) s->greedy_streak++;
            if (s->live_tokens && s->pos < s->max_ctx) {
                s->live_tokens[s->pos] = token_id;
                s->live_len = s->pos + 1;
            }
            s->pos++;
            ink_mark_written(s, s->pos);
            s->total_steps++;
        } else {
            if (s->inflight_active) ink_cancel_inflight(s);
            if (is_greedy_tok) s->greedy_streak++;
            else s->greedy_streak = 0;

            PmBatch cur_batch = NULL;
            if (s->conv_pending_n == 0 && !(s->dspark.ready && allow_spec) &&
                s->prep_batch && s->prep_pos == s->pos && s->prep_logits == compute_logits) {
                cur_batch = s->prep_batch;
                cur_slot = s->prep_slot;
                s->prep_batch = NULL;
                pm_patch_first_arg_int(cur_batch, (int64_t)token_id);
                if (s->live_tokens && s->pos < s->max_ctx) {
                    s->live_tokens[s->pos] = token_id;
                    s->live_len = s->pos + 1;
                }
                s->pos++;
                ink_mark_written(s, s->pos);
                s->total_steps++;
            } else {
                ink_discard_prep(s);
                cur_slot = s->next_m4_slot;
                s->next_m4_slot ^= 1;
                if (pm_begin_concurrent_slot(&cur_batch, cur_slot) != PM_OK) {
                    snprintf(s->error, sizeof s->error, "pm_begin_concurrent_slot: %s", pm_log());
                    return 1;
                }
                if (enqueue_step(s, &cur_batch, token_id, NULL, 0, cur_slot, compute_logits, 1) != 0) {
                    pm_abort(cur_batch);
                    return 1;
                }
            }
            if (pm_submit_async(cur_batch, &cur_ticket, &cur_slot) != PM_OK) {
                snprintf(s->error, sizeof s->error, "pm_submit_async: %s", pm_log());
                return 1;
            }
        }

        /* While GPU executes cur_slot, prepare or GPU-chain next_slot = cur_slot ^ 1!
         * Sprint D: NEVER speculatively chain argmax when sampled_mode != 0 (eliminates 2x ITL P95/P99 spikes!). */
        uint64_t next_ticket = 0;
        int next_slot = cur_slot ^ 1;
        int chained_next = 0;
        int dspark_want = (allow_spec && s->dspark.ready && !s->dspark.req_disabled &&
                           s->dspark.pos + s->dspark.pending_got == s->pos);
        int hold_for_spec = (allow_spec && (ink_pld_has_bigram(s->tok_hist, s->pos) || dspark_want));
        if (compute_logits && s->pos < s->max_ctx && !dspark_want) {
            s->next_m4_slot = cur_slot;
            PmBatch next_batch = NULL;
            if (pm_begin_concurrent_slot(&next_batch, next_slot) == PM_OK) {
                if (allow_chain && !s->sampled_mode && s->greedy_streak >= 1 && !hold_for_spec) {
                    if (enqueue_step(s, &next_batch, 0, s->argmax_out_buf[cur_slot], 1, next_slot, compute_logits, 0) != 0) {
                        pm_abort(next_batch);
                    } else if (pm_submit_async_after(next_batch, cur_ticket, &next_ticket, &next_slot) == PM_OK) {
                        chained_next = 1;
                        ink_mark_written(s, s->pos + 1);
                    }
                } else {
                    if (enqueue_step(s, &next_batch, 0, NULL, 0, next_slot, compute_logits, 0) == 0) {
                        s->prep_batch = next_batch;
                        s->prep_pos = s->pos;
                        s->prep_logits = compute_logits;
                        s->prep_slot = next_slot;
                    } else {
                        pm_abort(next_batch);
                    }
                }
            }
        }

        double gpu_s = 0.0;
        if (pm_wait_ticket(cur_ticket, cur_slot, &gpu_s) != PM_OK) {
            if (chained_next) pm_wait_ticket(next_ticket, next_slot, NULL);
            ink_discard_prep(s);
            snprintf(s->error, sizeof s->error, "pm_wait_ticket: %s", pm_log());
            return 1;
        }
        s->total_gpu_seconds += gpu_s;
        if (gpu_s > 0.001 && gpu_s < 0.25) {
            s->dspark.b1_ema_s = (s->dspark.b1_ema_s > 1e-4) ? (0.8 * s->dspark.b1_ema_s + 0.2 * gpu_s) : gpu_s;
            s->ema_base_ms = 0.80f * s->ema_base_ms + 0.20f * (float)(gpu_s * 1000.0);
        }
        if (compute_logits) {
            int32_t next_tok = (int32_t)((const uint32_t *)pm_contents(s->argmax_out_buf[cur_slot]))[0];
            s->active_out_slot = cur_slot;
            s->hidden_is_prefill = 0;
            s->argmax_valid = 1;
            s->last_hidden_off = 0;
            s->prev_argmax = next_tok;
            s->has_prev_argmax = 1;
            if (chained_next) {
                s->inflight_active = 1;
                s->inflight_ticket = next_ticket;
                s->inflight_slot = next_slot;
                s->inflight_pos = s->pos;
                s->inflight_logits = compute_logits;
                s->inflight_expected_tok = next_tok;
            }
        } else {
            s->argmax_valid = 0;
            s->has_prev_argmax = 0;
        }
        return 0;
    }
    ink_cancel_inflight(s);
    PmBatch batch = NULL;
    if (pm_begin_concurrent(&batch) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_begin_concurrent: %s", pm_log());
        return 1;
    }
    if (enqueue_step(s, &batch, token_id, NULL, 0, 0, compute_logits, 1) != 0) {
        pm_wait(batch, NULL);
        return 1;
    }
    double gpu_s = 0.0;
    if (pm_wait(batch, &gpu_s) != PM_OK) {
        snprintf(s->error, sizeof s->error, "pm_wait: %s", pm_log());
        return 1;
    }
    if (compute_logits) {
        s->prev_argmax = (int32_t)((const uint32_t *)pm_contents(s->argmax_out_buf[0]))[0];
        s->has_prev_argmax = 1;
    } else {
        s->has_prev_argmax = 0;
    }
    s->greedy_streak = 0;
    s->total_gpu_seconds += gpu_s;
    return 0;
}

const float *ink_logits(const InkSession *s) {
    if (!s) return NULL;
    if (s->spec_active_idx >= 0) {
        int64_t nv = s->gguf.hp.n_vocab_unpadded > 0 ? s->gguf.hp.n_vocab_unpadded : s->gguf.hp.n_vocab;
        return (const float *)pm_contents(s->spec_logits) + (size_t)s->spec_active_idx * (size_t)nv;
    }
    return (const float *)pm_contents(s->logits_buf[s->active_out_slot]);
}
const float *ink_hidden(const InkSession *s) {
    if (!s) return NULL;
    if (s->spec_active_idx >= 0) {
        return (const float *)pm_contents(s->spec_hidden) + (size_t)s->spec_active_idx * (size_t)s->gguf.hp.n_embd;
    }
    if (s->hidden_is_prefill) return (const float *)pm_contents(s->norm_buf) + s->last_hidden_off;
    return (const float *)pm_contents(s->hidden_out[s->active_out_slot]);
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

static float next_u01(uint64_t *rng_state) {
    uint64_t x = rng_state ? *rng_state : 0x9e3779b97f4a7c15ULL;
    x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
    if (rng_state) *rng_state = x;
    return (float)((x * 2685821657736338717ULL) >> 40) / (float)(1ULL << 24);
}

static int32_t sample_logits_ex(const float *logits, int n, const InkSampleParams *p, uint64_t *rng_state,
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
    if (!idx || !val) {
        if (top_k > 64) { free(idx); free(val); }
        return 0;
    }
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
        float u = next_u01(rng_state);
        if (u < p_draft) {
            if (out_accepted) *out_accepted = 1;
            chosen = verify_draft;
        } else {
            if (out_accepted) *out_accepted = 0;
            float rem = 1.0f - p_draft;
            if (rem <= 1e-7f) {
                chosen = idx[0];
            } else {
                float r = next_u01(rng_state);
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
        float r = next_u01(rng_state);
        float acc = 0.0f;
        for (int i = 0; i < cut; i++) {
            acc += val[i];
            if (r <= acc) { chosen = idx[i]; break; }
        }
    }
    if (top_k > 64) { free(idx); free(val); }
    return (int32_t)chosen;
}

static int32_t sample_from_topk64(const float *tv, const uint32_t *ti, int n_avail,
                                  const InkSampleParams *p, uint64_t *rng_state,
                                  int32_t verify_draft, int *out_accepted) {
    if (!tv || !ti || n_avail <= 0) return -1;
    float temp = p ? p->temperature : 0.0f;
    if (temp <= 1e-6f) {
        int32_t best = (int32_t)ti[0];
        if (out_accepted) *out_accepted = (verify_draft >= 0 && best == verify_draft) ? 1 : 0;
        return best;
    }
    int top_k = (p && p->top_k > 0) ? p->top_k : 40;
    if (top_k > n_avail) top_k = n_avail;
    if (top_k > 64) top_k = 64;
    float top_p = (p && p->top_p > 0.0f && p->top_p < 1.0f) ? p->top_p : 0.95f;
    float val[64];
    int idx[64];
    int k = 0;
    float inv_t = 1.0f / temp;
    for (int i = 0; i < top_k; i++) {
        if (ti[i] == 0xffffffffu) break;
        val[k] = tv[i] * inv_t;
        idx[k] = (int)ti[i];
        k++;
    }
    if (k == 0) return 0;
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
        float u = next_u01(rng_state);
        if (u < p_draft) {
            if (out_accepted) *out_accepted = 1;
            chosen = verify_draft;
        } else {
            if (out_accepted) *out_accepted = 0;
            float rem = 1.0f - p_draft;
            if (rem <= 1e-7f) {
                chosen = idx[0];
            } else {
                float r = next_u01(rng_state);
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
        float r = next_u01(rng_state);
        float acc = 0.0f;
        for (int i = 0; i < cut; i++) {
            acc += val[i];
            if (r <= acc) { chosen = idx[i]; break; }
        }
    }
    return (int32_t)chosen;
}

int32_t ink_sample(const InkSession *s, const InkSampleParams *p, uint64_t *rng_state) {
    if (!s) return -1;
    int n = ink_vocab_unpadded(s);
    if (n <= 0) return -1;

    float temp = p ? p->temperature : 0.0f;
    ((InkSession *)s)->sampled_mode = (temp > 1e-6f) ? 1 : 0;
    if (temp <= 1e-6f) {
        if (s->spec_active_idx >= 0) {
            return s->spec_argmax[s->spec_active_idx];
        }
        if (s->argmax_valid) {
            return (int32_t)((const uint32_t *)pm_contents(s->argmax_out_buf[s->active_out_slot]))[0];
        }
    } else {
        int top_k = (p && p->top_k > 0 && p->top_k < n) ? p->top_k : 40;
        if (top_k <= 64 && s->argmax_valid && getenv("INK_CPU_SAMPLE") == NULL) {
            if (s->spec_active_idx >= 0) {
                const float *tv = (const float *)pm_contents(s->spec_topk_vals) + (size_t)s->spec_active_idx * 64;
                const uint32_t *ti = (const uint32_t *)pm_contents(s->spec_topk_ids) + (size_t)s->spec_active_idx * 64;
                return sample_from_topk64(tv, ti, 64, p, rng_state, -1, NULL);
            } else {
                const float *tv = (const float *)pm_contents(s->topk_vals[s->active_out_slot]);
                const uint32_t *ti = (const uint32_t *)pm_contents(s->topk_ids[s->active_out_slot]);
                return sample_from_topk64(tv, ti, 64, p, rng_state, -1, NULL);
            }
        }
    }
    const float *logits = ink_logits(s);
    if (!logits) return -1;
    return sample_logits_ex(logits, n, p, rng_state, -1, NULL);
}

static void rewind_conv_shift3(float *dst, const float *old_s, const float *raw, int64_t n_ch, int a) {
    for (int64_t ch = 0; ch < n_ch; ch++) {
        float *o = dst + ch * 3;
        const float *os = old_s + ch * 3;
        if (a >= 3) {
            o[0] = raw[(int64_t)(a - 3) * n_ch + ch];
            o[1] = raw[(int64_t)(a - 2) * n_ch + ch];
            o[2] = raw[(int64_t)(a - 1) * n_ch + ch];
        } else if (a == 2) {
            o[0] = os[2];
            o[1] = raw[ch];
            o[2] = raw[n_ch + ch];
        } else {
            o[0] = os[1];
            o[1] = os[2];
            o[2] = raw[ch];
        }
    }
}

static void rewind_conv_ring4(float *dst, const float *old_s, const float *raw, int64_t n_ch, int pos0, int a) {
    memcpy(dst, old_s, (size_t)n_ch * 4 * sizeof(float));
    int t0 = (a >= 3) ? (a - 3) : 0;
    for (int t = t0; t < a; t++) {
        int slot = (pos0 + t) & 3;
        const float *rt = raw + (int64_t)t * n_ch;
        for (int64_t ch = 0; ch < n_ch; ch++) {
            dst[ch * 4 + slot] = rt[ch];
        }
    }
}

int ink_verify_draft(InkSession *s, const int32_t *tokens, int nb,
                     const InkSampleParams *p, uint64_t *rng_state, int32_t *out_next_token) {
    if (!s || !tokens || !out_next_token || nb < 1 || nb > INK_SPEC_MAX_NB) return -1;
    const InkHParams *hp = &s->gguf.hp;
    if (s->pos + nb > s->max_ctx) {
        snprintf(s->error, sizeof s->error, "context full (%d+%d/%d)", s->pos, nb, s->max_ctx);
        return -1;
    }
    if (nb == 1) {
        if (ink_step(s, tokens[0], 1) != 0) return -1;
        *out_next_token = ink_sample(s, p, rng_state);
        return 0;
    }

    ink_cancel_inflight(s);
    ink_flush_spec(s);
    int64_t d = hp->n_embd;
    int64_t n_kv = hp->n_head_kv * hp->head_dim;
    int64_t nv = hp->n_vocab_unpadded > 0 ? hp->n_vocab_unpadded : hp->n_vocab;
    int pos0 = s->pos;

    /* Sprint 71B/C fast path: use bit-exact grouped-MoE ink_verify_batch when Q4S is active! */
    if (s->can_verify_nb && pm_is_m4()) {
        if (ink_verify_batch(s, tokens, nb) != 0) return -1;
        const float *vlog = (const float *)pm_contents(s->spec_logits);
        const float *stv = (const float *)pm_contents(s->spec_topk_vals);
        const uint32_t *sti = (const uint32_t *)pm_contents(s->spec_topk_ids);
        int top_k_v = (p && p->top_k > 0 && p->top_k < (int)nv) ? p->top_k : 40;
        int use_gpu_topk = (top_k_v <= 64 && getenv("INK_CPU_SAMPLE") == NULL);
        int accepted = 0;
        int32_t next_tok = -1;
        for (int j = 0; j < nb - 1; j++) {
            int ok = 0;
            int32_t sampled = use_gpu_topk
                ? sample_from_topk64(stv + (int64_t)j * 64, sti + (int64_t)j * 64, 64, p, rng_state, tokens[j + 1], &ok)
                : sample_logits_ex(vlog + (int64_t)j * nv, (int)nv, p, rng_state, tokens[j + 1], &ok);
            if (ok) accepted++;
            else { next_tok = sampled; break; }
        }
        if (accepted == nb - 1) {
            next_tok = use_gpu_topk
                ? sample_from_topk64(stv + (int64_t)(nb - 1) * 64, sti + (int64_t)(nb - 1) * 64, 64, p, rng_state, -1, NULL)
                : sample_logits_ex(vlog + (int64_t)(nb - 1) * nv, (int)nv, p, rng_state, -1, NULL);
        }
        int a = accepted + 1;
        memcpy(pm_contents(s->logits_buf[0]), vlog + (int64_t)(a - 1) * nv, (size_t)nv * sizeof(float));
        memcpy(pm_contents(s->hidden_out[0]), (const float *)pm_contents(s->spec_hidden) + (int64_t)(a - 1) * d, (size_t)d * sizeof(float));
        memcpy(pm_contents(s->topk_vals[0]), stv + (int64_t)(a - 1) * 64, 64 * sizeof(float));
        memcpy(pm_contents(s->topk_ids[0]), sti + (int64_t)(a - 1) * 64, 64 * sizeof(uint32_t));
        ((uint32_t *)pm_contents(s->argmax_out_buf[0]))[0] = sti[(a - 1) * 64];
        s->active_out_slot = 0;
        s->hidden_is_prefill = 0;
        s->last_hidden_off = 0;
        s->argmax_valid = 1;
        s->conv_pending_pos0 = pos0;
        s->conv_pending_n = a;
        if (s->dspark.ready && s->dspark.pos == pos0) {
            s->dspark.pending_pos0 = pos0;
            s->dspark.pending_nb = nb;
            s->dspark.pending_got = a;
        }
        if (s->tok_hist && pos0 + a <= s->max_ctx) {
            memcpy(s->tok_hist + pos0, tokens, (size_t)a * sizeof(int32_t));
            if (pos0 + a > s->tok_hist_len) s->tok_hist_len = pos0 + a;
        }
        if (s->live_tokens && pos0 + a <= s->max_ctx) {
            memcpy(s->live_tokens + pos0, tokens, (size_t)a * sizeof(int32_t));
            s->live_len = pos0 + a;
        }
        s->pos = pos0 + a;
        ink_mark_written(s, pos0 + nb);
        s->total_steps += a;
        *out_next_token = next_tok;
        return accepted;
    }

    if (!s->verify_logits) {
        if (alloc_buf_unset(s, &s->verify_logits, (size_t)INK_SPEC_MAX_NB * (size_t)hp->n_vocab * sizeof(float)) != 0) return -1;
    }
    if (!s->spec_hist) {
        size_t sh_elems = (size_t)s->n_slots * (size_t)INK_SPEC_MAX_NB * (size_t)(2 * n_kv + 2 * d);
        if (alloc_buf_unset(s, &s->spec_hist, sh_elems * sizeof(float)) != 0) return -1;
    }
    if (!s->spec_old_conv) {
        s->spec_old_conv = (float *)malloc((size_t)s->conv_all_elems * sizeof(float));
        if (!s->spec_old_conv) { snprintf(s->error, sizeof s->error, "out of memory allocating spec_old_conv"); return -1; }
    }

    memcpy(s->spec_old_conv, pm_contents(s->conv_all), (size_t)s->conv_all_elems * sizeof(float));

    const char *fp32_env = getenv("INK_SPEC_FP32");
    int force_fp32 = (fp32_env != NULL && fp32_env[0] != '0');
    if (run_chunk_ex(s, tokens, nb, 2, 1, force_fp32) != 0) {
        memcpy(pm_contents(s->conv_all), s->spec_old_conv, (size_t)s->conv_all_elems * sizeof(float));
        return -1;
    }

    const float *vlog = (const float *)pm_contents(s->verify_logits);
    int accepted = 0;
    int32_t next_tok = -1;
    for (int j = 0; j < nb - 1; j++) {
        int ok = 0;
        int32_t sampled = sample_logits_ex(vlog + (int64_t)j * nv, (int)nv, p, rng_state, tokens[j + 1], &ok);
        if (ok) accepted++;
        else { next_tok = sampled; break; }
    }
    if (accepted == nb - 1) {
        next_tok = sample_logits_ex(vlog + (int64_t)(nb - 1) * nv, (int)nv, p, rng_state, -1, NULL);
    }

    int a = accepted + 1;
    memcpy(pm_contents(s->logits_buf[0]), vlog + (int64_t)(a - 1) * nv, (size_t)nv * sizeof(float));
    s->active_out_slot = 0;
    s->hidden_is_prefill = 1;
    s->last_hidden_off = (int64_t)(a - 1) * d;
    s->argmax_valid = 0;

    if (a < nb) {
        float *conv_dst = (float *)pm_contents(s->conv_all);
        const float *sh_base = (const float *)pm_contents(s->spec_hist);
        int64_t slot_stride = (int64_t)INK_SPEC_MAX_NB * (2 * n_kv + 2 * d);
        for (int il = 0; il < s->n_slots; il++) {
            float *cd = conv_dst + (int64_t)il * s->conv_slot_stride;
            const float *co = s->spec_old_conv + (int64_t)il * s->conv_slot_stride;
            const float *sh = sh_base + (int64_t)il * slot_stride;
            rewind_conv_ring4(cd, co, sh, n_kv, pos0, a);
            rewind_conv_ring4(cd + n_kv * 4, co + n_kv * 4, sh + (int64_t)INK_SPEC_MAX_NB * n_kv, n_kv, pos0, a);
            rewind_conv_shift3(cd + 2 * n_kv * 4, co + 2 * n_kv * 4, sh + (int64_t)INK_SPEC_MAX_NB * (2 * n_kv), d, a);
            rewind_conv_shift3(cd + 2 * n_kv * 4 + d * 3, co + 2 * n_kv * 4 + d * 3, sh + (int64_t)INK_SPEC_MAX_NB * (2 * n_kv + d), d, a);
        }
    }

    if (s->tok_hist && pos0 + a <= s->max_ctx) {
        memcpy(s->tok_hist + pos0, tokens, (size_t)a * sizeof(int32_t));
        if (pos0 + a > s->tok_hist_len) s->tok_hist_len = pos0 + a;
    }
    if (s->live_tokens && pos0 + a <= s->max_ctx) {
        memcpy(s->live_tokens + pos0, tokens, (size_t)a * sizeof(int32_t));
        s->live_len = pos0 + a;
    }
    s->pos = pos0 + a;
    ink_mark_written(s, pos0 + nb);
    s->total_steps += a;
    *out_next_token = next_tok;
    return accepted;
}

const char *ink_error(const InkSession *s) { return s ? s->error : "null session"; }
const char *ink_backend_name(const InkSession *s) { (void)s; return "psionic-metal"; }
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

    PmKernel k_nv = NULL, k_mx = NULL, k_iq = NULL, k_ggml = NULL, k_ablate = NULL;
    if (pm_kernel(mod, "ink_dv_nvfp4", &k_nv) != PM_OK ||
        pm_kernel(mod, "ink_dv_mxfp8", &k_mx) != PM_OK ||
        pm_kernel(mod, "ink_dv_iq4",   &k_iq) != PM_OK ||
        pm_kernel(mod, "ink_dv_ggml",  &k_ggml) != PM_OK ||
        pm_kernel(mod, "ink_ablate_dir", &k_ablate) != PM_OK) {
        snprintf(err, err_bytes, "pm_kernel: %s", pm_log());
        if (k_nv) pm_kernel_free(k_nv);
        if (k_mx) pm_kernel_free(k_mx);
        if (k_iq) pm_kernel_free(k_iq);
        if (k_ggml) pm_kernel_free(k_ggml);
        if (k_ablate) pm_kernel_free(k_ablate);
        pm_unload(mod); return 1;
    }

    int rc = 1;
    const int rows = 8, cols = 64; /* 4 NVFP4 blocks/row, 2 MXFP8 blocks/row, 2 IQ4 blocks/row */
    PmBuffer b_x = NULL, b_out = NULL, b_nv_p = NULL, b_nv_s = NULL, b_mx_w = NULL, b_mx_s = NULL, b_iq_w = NULL;
    PmBuffer b_g_grids = NULL, b_g_w = NULL, b_g_x = NULL, b_g_out = NULL, b_ab_d = NULL, b_ab_x = NULL;
    if (pm_alloc(cols * 4, &b_x) != PM_OK ||
        pm_alloc(rows * 4, &b_out) != PM_OK ||
        pm_alloc(rows * (cols / 16) * 8, &b_nv_p) != PM_OK ||
        pm_alloc(rows * (cols / 16), &b_nv_s) != PM_OK ||
        pm_alloc(rows * cols, &b_mx_w) != PM_OK ||
        pm_alloc(rows * (cols / 32), &b_mx_s) != PM_OK ||
        pm_alloc(rows * (cols / 32) * 18, &b_iq_w) != PM_OK ||
        pm_alloc((size_t)INK_GRID_TOTAL_U32 * 4, &b_g_grids) != PM_OK ||
        pm_alloc(4 * 256 * 4, &b_g_w) != PM_OK ||
        pm_alloc(256 * 4, &b_g_x) != PM_OK ||
        pm_alloc(4 * 4, &b_g_out) != PM_OK ||
        pm_alloc(256 * 4, &b_ab_d) != PM_OK ||
        pm_alloc(2 * 256 * 4, &b_ab_x) != PM_OK) {
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

    /* 4. Verify Metal ink_dv_ggml across ALL 21 GGML quant types against CPU ink_ggml_dequant_row */
    memcpy(pm_contents(b_g_grids), ink_ggml_grid_table(NULL), (size_t)INK_GRID_TOTAL_U32 * sizeof(uint32_t));
    float *gx = (float *)pm_contents(b_g_x);
    for (int c = 0; c < 256; c++) gx[c] = (float)(((c * 7) % 19) - 9) * 0.0625f;
    static const uint32_t all_ggml_types[21] = {
        INK_GGML_F32, INK_GGML_F16, INK_GGML_BF16,
        INK_GGML_Q4_0, INK_GGML_Q4_1, INK_GGML_Q5_0, INK_GGML_Q5_1, INK_GGML_Q8_0,
        INK_GGML_Q2_K, INK_GGML_Q3_K, INK_GGML_Q4_K, INK_GGML_Q5_K, INK_GGML_Q6_K,
        INK_GGML_IQ2_XXS, INK_GGML_IQ2_XS, INK_GGML_IQ3_XXS, INK_GGML_IQ4_NL,
        INK_GGML_IQ3_S, INK_GGML_IQ2_S, INK_GGML_IQ4_XS, INK_GGML_Q2_0
    };
    static const uint16_t test_f16[4] = {0x3800, 0x3C00, 0x3400, 0xBC00}; /* 0.5, 1.0, 0.25, -1.0 */
    float max_ggml_all = 0.0f;
    for (int ti = 0; ti < 21; ti++) {
        uint32_t wt = all_ggml_types[ti];
        int64_t be = ink_ggml_block_elems(wt);
        int64_t bb = ink_ggml_block_bytes(wt);
        int64_t nblk = 256 / be;
        int64_t rb = nblk * bb;
        uint8_t *wb = (uint8_t *)pm_contents(b_g_w);
        memset(wb, 0, 4 * 256 * 4);
        for (int r = 0; r < 4; r++) {
            uint8_t *rowp = wb + (size_t)r * (size_t)rb;
            if (wt == INK_GGML_F32) {
                float *fp = (float *)rowp;
                for (int c = 0; c < 256; c++) fp[c] = (float)(((r * 13 + c * 5) % 23) - 11) * 0.125f;
            } else if (wt == INK_GGML_F16) {
                uint16_t *hp = (uint16_t *)rowp;
                for (int c = 0; c < 256; c++) hp[c] = f32_to_f16_host((float)(((r * 13 + c * 5) % 23) - 11) * 0.125f);
            } else if (wt == INK_GGML_BF16) {
                uint16_t *bp = (uint16_t *)rowp;
                for (int c = 0; c < 256; c++) {
                    float fv = (float)(((r * 13 + c * 5) % 23) - 11) * 0.125f;
                    uint32_t u; memcpy(&u, &fv, 4);
                    bp[c] = (uint16_t)(u >> 16);
                }
            } else {
                for (int64_t b = 0; b < nblk; b++) {
                    uint8_t *blk = rowp + (size_t)b * (size_t)bb;
                    for (int64_t j = 0; j < bb; j++) {
                        blk[j] = (uint8_t)((r * 37 + b * 53 + j * 17 + ti * 11 + 23) & 0xFF);
                    }
                    uint16_t d0 = test_f16[(r + b) & 3];
                    uint16_t d1 = test_f16[(r + b + 1) & 3];
                    if (wt == INK_GGML_Q2_K) {
                        memcpy(blk + 80, &d0, 2); memcpy(blk + 82, &d1, 2);
                    } else if (wt == INK_GGML_Q3_K) {
                        memcpy(blk + 108, &d0, 2);
                    } else if (wt == INK_GGML_Q4_K || wt == INK_GGML_Q5_K || wt == INK_GGML_Q4_1 || wt == INK_GGML_Q5_1) {
                        memcpy(blk + 0, &d0, 2); memcpy(blk + 2, &d1, 2);
                    } else if (wt == INK_GGML_Q6_K) {
                        memcpy(blk + 208, &d0, 2);
                    } else {
                        memcpy(blk + 0, &d0, 2);
                    }
                }
            }
        }
        if (pm_begin(&batch) != PM_OK) { snprintf(err, err_bytes, "pm_begin(%s): %s", ink_ggml_type_name(wt), pm_log()); goto cleanup; }
        PmArg g_args[8] = {
            a_int(4), a_int(256), a_int(0), a_int((int64_t)wt),
            a_u16v(b_g_w, 0, (4 * rb + 1) / 2),
            a_u32v(b_g_grids, 0, INK_GRID_TOTAL_U32),
            a_f32v(b_g_x, 0, 256), a_f32v(b_g_out, 0, 4)
        };
        if (pm_launch(batch, k_ggml, 1, 128, 0, g_args, 8) != PM_OK || pm_wait(batch, NULL) != PM_OK) {
            snprintf(err, err_bytes, "ink_dv_ggml(%s) launch: %s", ink_ggml_type_name(wt), pm_log()); goto cleanup;
        }
        const float *ggot = (const float *)pm_contents(b_g_out);
        float max_t = 0.0f;
        float dec[256];
        for (int r = 0; r < 4; r++) {
            if (ink_ggml_dequant_row(wt, wb + (size_t)r * (size_t)rb, dec, 256) != 0) {
                snprintf(err, err_bytes, "cpu dequant failed for %s", ink_ggml_type_name(wt)); goto cleanup;
            }
            float ref = 0.0f;
            for (int c = 0; c < 256; c++) ref += dec[c] * gx[c];
            float diff = fabsf(ggot[r] - ref);
            if (diff > max_t) max_t = diff;
        }
        if (max_t > max_ggml_all) max_ggml_all = max_t;
        if (max_t > 1e-3f || isnan(max_t)) {
            snprintf(err, err_bytes, "ink_dv_ggml(%s) mismatch: max_abs=%g", ink_ggml_type_name(wt), max_t);
            goto cleanup;
        }
    }

    /* 5. Verify Metal ink_ablate_dir against CPU x - d * dot(d, x) */
    float *abd = (float *)pm_contents(b_ab_d);
    float *abx = (float *)pm_contents(b_ab_x);
    float norm2 = 0.0f;
    for (int i = 0; i < 256; i++) { abd[i] = (float)((i % 7) - 3) * 0.1f; norm2 += abd[i] * abd[i]; }
    float invn = 1.0f / sqrtf(norm2);
    for (int i = 0; i < 256; i++) abd[i] *= invn;
    float ab_ref[2 * 256];
    for (int t = 0; t < 2; t++) {
        float dot = 0.0f;
        for (int i = 0; i < 256; i++) {
            float v = (float)(((t * 17 + i * 3) % 15) - 7) * 0.25f;
            abx[t * 256 + i] = v;
            dot += v * abd[i];
        }
        for (int i = 0; i < 256; i++) ab_ref[t * 256 + i] = abx[t * 256 + i] - dot * abd[i];
    }
    if (pm_begin(&batch) != PM_OK) { snprintf(err, err_bytes, "pm_begin(ablate): %s", pm_log()); goto cleanup; }
    PmArg ab_args[4] = { a_int(256), a_int(0), a_f32v(b_ab_d, 0, 256), a_f32v(b_ab_x, 0, 2 * 256) };
    if (pm_launch(batch, k_ablate, 2, INK_NT, 0, ab_args, 4) != PM_OK || pm_wait(batch, NULL) != PM_OK) {
        snprintf(err, err_bytes, "ink_ablate_dir launch: %s", pm_log()); goto cleanup;
    }
    float max_ab = 0.0f;
    for (int i = 0; i < 2 * 256; i++) {
        float diff = fabsf(abx[i] - ab_ref[i]);
        if (diff > max_ab) max_ab = diff;
    }
    if (max_ab > 1e-5f) { snprintf(err, err_bytes, "ink_ablate_dir mismatch: max_abs=%g", max_ab); goto cleanup; }

    fprintf(stderr,
            "metal-quant-kernels device='%s' compile=%.2fs nvfp4_max_abs=%.2e mxfp8_max_abs=%.2e iq4_max_abs=%.2e ggml21_max_abs=%.2e ablate_max_abs=%.2e ok=true\n",
            info.name, compile_s, max_nv, max_mx, max_iq, max_ggml_all, max_ab);
    rc = 0;

cleanup:
    if (b_x) pm_free(b_x);
    if (b_out) pm_free(b_out);
    if (b_nv_p) pm_free(b_nv_p);
    if (b_nv_s) pm_free(b_nv_s);
    if (b_mx_w) pm_free(b_mx_w);
    if (b_mx_s) pm_free(b_mx_s);
    if (b_iq_w) pm_free(b_iq_w);
    if (b_g_grids) pm_free(b_g_grids);
    if (b_g_w) pm_free(b_g_w);
    if (b_g_x) pm_free(b_g_x);
    if (b_g_out) pm_free(b_g_out);
    if (b_ab_d) pm_free(b_ab_d);
    if (b_ab_x) pm_free(b_ab_x);
    pm_kernel_free(k_nv); pm_kernel_free(k_mx); pm_kernel_free(k_iq);
    pm_kernel_free(k_ggml); pm_kernel_free(k_ablate);
    pm_unload(mod);
    return rc;
}
