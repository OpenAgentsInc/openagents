/* Internal GGUF loader & BPE tokenizer structures for Psionic Inkling 2 Mini. */
#ifndef PSIONIC_INKLING_GGUF_H
#define PSIONIC_INKLING_GGUF_H
#include "inkling-engine.h"
#include <stddef.h>
#include <stdint.h>

#define INK_GGML_F32     0
#define INK_GGML_F16     1
#define INK_GGML_Q4_0    2
#define INK_GGML_Q4_1    3
#define INK_GGML_Q5_0    6
#define INK_GGML_Q5_1    7
#define INK_GGML_Q8_0    8
#define INK_GGML_Q2_K    10
#define INK_GGML_Q3_K    11
#define INK_GGML_Q4_K    12
#define INK_GGML_Q5_K    13
#define INK_GGML_Q6_K    14
#define INK_GGML_IQ2_XXS 16
#define INK_GGML_IQ2_XS  17
#define INK_GGML_IQ3_XXS 18
#define INK_GGML_IQ4_NL  20
#define INK_GGML_IQ3_S   21
#define INK_GGML_IQ2_S   22
#define INK_GGML_IQ4_XS  23
#define INK_GGML_BF16    30
#define INK_GGML_Q2_0    42
#define INK_MAX_SLOTS    128

/* Packed 32-bit offsets inside ink_ggml_grid_table() (total 4,384 uint32_t = 17,536 bytes). */
#define INK_GRID_OFF_IQ2XXS  0     /* 256 x u64 = 512 u32 */
#define INK_GRID_OFF_IQ2XS   512   /* 512 x u64 = 1024 u32 */
#define INK_GRID_OFF_IQ2S    1536  /* 1024 x u64 = 2048 u32 */
#define INK_GRID_OFF_IQ3XXS  3584  /* 256 x u32 = 256 u32 */
#define INK_GRID_OFF_IQ3S    3840  /* 512 x u32 = 512 u32 */
#define INK_GRID_OFF_KSIGNS  4352  /* 128 x u8  = 32 u32 */
#define INK_GRID_TOTAL_U32   4384

typedef struct {
    uint64_t offset;   /* byte offset from start of GGUF file (mmap base) */
    uint64_t nbytes;
    uint32_t type;
    uint32_t ndims;
    uint64_t dims[4];
    int      present;
} InkTensor;

typedef struct {
    InkTensor attn_norm;
    InkTensor attn_q_inp_norm;
    InkTensor attn_k_inp_norm;
    InkTensor attn_v_inp_norm;
    InkTensor attn_q;
    InkTensor attn_k;
    InkTensor attn_v;
    InkTensor attn_output;
    InkTensor attn_gate;
    InkTensor attn_rel;
    InkTensor attn_rel_bias;
    InkTensor attn_q_clip;
    InkTensor attn_rel_clip;
    InkTensor attn_sink_proj;
    InkTensor attn_k_conv;
    InkTensor attn_k_conv_proj;
    InkTensor attn_v_conv;
    InkTensor attn_v_conv_proj;
    InkTensor attn_post_conv;
    InkTensor attn_res_query;
    InkTensor ffn_res_query;
    InkTensor ffn_norm;
    InkTensor ffn_gate_inp;
    InkTensor ffn_exp_probs_b;
    InkTensor ffn_gate_up_exps;
    InkTensor ffn_down_exps;
    InkTensor ffn_post_conv;
    /* Loop core tensors (only present for layers in [loop_start, loop_end)) */
    InkTensor loop_msg_query;
    InkTensor loop_ctrl_down[4];   /* 0=q, 1=k, 2=v, 3=ffn */
    InkTensor loop_ctrl_gain[4];
    InkTensor loop_ctrl_forget[4];
} InkLayerTensors;

typedef struct {
    char    *text;
    uint32_t len;
    int32_t  type; /* 1=normal, 3=control/special, 6=byte */
} InkVocabTok;

typedef struct {
    uint64_t key;
    int32_t  rank;
    int32_t  merged_id;
} InkMergeEntry;

typedef struct {
    uint64_t hash;
    int32_t  id;
} InkTokHashEntry;

typedef struct {
    InkVocabTok    *tokens;
    int64_t         n_tokens;
    InkTokHashEntry *tok_map;
    size_t          tok_cap;
    InkMergeEntry  *merge_map;
    size_t          merge_cap;
    int32_t        *special_ids;
    int64_t         n_special;
    uint8_t         byte_decoder[512]; /* UTF-8 codepoint (0..323) -> byte */
    int32_t         byte_encoder[256]; /* byte -> unicode codepoint */
} InkTokenizer;

typedef struct {
    int             fd;
    uint8_t        *map;
    size_t          file_size;
    size_t          map_size;
    uint64_t        data_offset;
    InkHParams      hp;
    InkTensor       tok_embd;
    InkTensor       tok_norm;
    InkTensor       out_res_query;
    InkTensor       out_norm;
    InkTensor       output;
    InkLayerTensors layers[32];
    InkTokenizer    tok;
} InkGGUF;

typedef struct {
    InkTensor attn_output_a;
    InkTensor attn_output_b;
    InkTensor ffn_down_exps_a;
    InkTensor ffn_down_exps_b;
} InkLoraLayer;

typedef struct {
    uint8_t      *map;
    size_t        file_size;
    size_t        map_size;
    uint64_t      data_offset;
    uint64_t      fp;
    float         alpha;
    InkTensor     tok_embd_a;
    InkTensor     tok_embd_b;
    InkLoraLayer  layers[32];
} InkLora;

int  ink_gguf_open(const char *path, InkGGUF *g, char *err, size_t err_bytes);
int  ink_gguf_open_ex(const char *path, int copy_on_write, InkGGUF *g, char *err, size_t err_bytes);
void ink_gguf_close(InkGGUF *g);
int  ink_file_is_safetensors(const char *path);
int  ink_gguf_apply_safetensors_attn_overlay(InkGGUF *g, const char *path, uint64_t *out_fp,
                                             char *err, size_t err_bytes);

int  ink_ggml_block_elems(uint32_t type);
int  ink_ggml_block_bytes(uint32_t type);
int  ink_ggml_type_supported(uint32_t type);
const char *ink_ggml_type_name(uint32_t type);
int  ink_ggml_dequant_row(uint32_t type, const uint8_t *src, float *dst, int64_t n);
const uint32_t *ink_ggml_grid_table(size_t *out_u32_count);

int  ink_lora_open(const char *path, InkLora *l, char *err, size_t err_bytes);
void ink_lora_close(InkLora *l);
/* Extract normalized direction-ablation vectors into out_dirs[(1 + n_phys) * n_embd]:
   slot 0 = token_embd, slot 1 + ip = layer ip (shared by attn_output & ffn_down_exps). */
int  ink_lora_extract_ablate(const InkLora *l, int n_phys, int n_embd, int n_expert,
                             float *out_dirs, int *out_has_emb, uint8_t *out_has_layer,
                             char *err, size_t err_bytes);

int64_t ink_gguf_tokenize(const InkGGUF *g, const char *text, size_t len,
                          int32_t *out_ids, int64_t max_ids);
size_t  ink_gguf_decode_token(const InkGGUF *g, int32_t id, char *out, size_t max_out);
int32_t ink_gguf_find_token(const InkGGUF *g, const char *name);

#endif
