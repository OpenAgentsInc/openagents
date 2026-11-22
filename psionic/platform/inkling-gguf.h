/* Internal GGUF loader & BPE tokenizer structures for Psionic Inkling 2 Mini. */
#ifndef PSIONIC_INKLING_GGUF_H
#define PSIONIC_INKLING_GGUF_H
#include "inkling-metal.h"
#include <stddef.h>
#include <stdint.h>

#define INK_GGML_F32    0
#define INK_GGML_Q8_0   8
#define INK_GGML_IQ4_NL 20
#define INK_MAX_SLOTS   128

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

int  ink_gguf_open(const char *path, InkGGUF *g, char *err, size_t err_bytes);
void ink_gguf_close(InkGGUF *g);

int64_t ink_gguf_tokenize(const InkGGUF *g, const char *text, size_t len,
                          int32_t *out_ids, int64_t max_ids);
size_t  ink_gguf_decode_token(const InkGGUF *g, int32_t id, char *out, size_t max_out);
int32_t ink_gguf_find_token(const InkGGUF *g, const char *name);

#endif
