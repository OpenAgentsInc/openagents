/* Native Psionic Metal runtime for Inkling 2 Mini (inkling2_loop GGUF).
   Kernels are Coax (psionic/lib/inkling/kernels.coax) compiled by
   `coax metal-compute` and executed through psionic/platform/metal-host.m. */
#ifndef PSIONIC_INKLING_METAL_H
#define PSIONIC_INKLING_METAL_H
#include <stddef.h>
#include <stdint.h>

typedef struct InkSession InkSession;

typedef struct {
    int64_t n_vocab;
    int64_t n_vocab_unpadded;
    int64_t n_embd;
    int64_t n_head;
    int64_t n_head_kv;
    int64_t head_dim;
    int64_t n_ff_exp;
    int64_t n_expert;
    int64_t n_expert_used;
    int64_t n_phys;
    int64_t n_slot;
    int64_t loop_start;
    int64_t loop_end;
    int64_t loop_count;
    int64_t gate_rank;
    int64_t swa;
    int64_t rel_len;
    int64_t rel_extent;
    int64_t conv_len;
    int64_t block_size;
    int64_t log_scale_floor;
    int64_t eos_token;
    float   rms_eps;
    float   expert_weights_scale;
    float   situ_beta;
    float   situ_linear_beta;
    float   logit_softcap;
    float   qk_clip;
    float   rel_clip;
    float   log_scale_alpha;
    uint8_t is_swa[64];
} InkHParams;

typedef struct {
    float temperature;
    float top_p;
    int   top_k;
} InkSampleParams;

/* Open an Inkling 2 Mini GGUF checkpoint on Metal.
   metal_path is a file containing MSL produced by `coax metal-compute psionic/lib/inkling/kernels.coax`.
   max_ctx is the maximum context length for full-attention layers (e.g. 4096 or 8192). */
int  ink_open(const char *gguf_path, const char *metal_path, int max_ctx,
              InkSession **out, char *err, size_t err_bytes);
void ink_close(InkSession *s);
void ink_reset(InkSession *s);
int  ink_step(InkSession *s, int32_t token_id, int compute_logits);
int  ink_prefill(InkSession *s, const int32_t *tokens, int n_tokens, int compute_last_logits);

const float      *ink_logits(const InkSession *s);
const InkHParams *ink_hparams(const InkSession *s);
int               ink_vocab_size(const InkSession *s);
int               ink_vocab_unpadded(const InkSession *s);
int               ink_pos(const InkSession *s);
int               ink_max_ctx(const InkSession *s);
int32_t           ink_bos_token(const InkSession *s);
int32_t           ink_eos_token(const InkSession *s);
int               ink_is_eog(const InkSession *s, int32_t id);
int32_t           ink_find_token(const InkSession *s, const char *name);

int         ink_encode(const InkSession *s, const char *text, size_t len,
                       int32_t *out, int max_out, int parse_special);
int         ink_decode_piece(const InkSession *s, int32_t id, char *out, int cap);
const char *ink_token_str(const InkSession *s, int32_t id);
int32_t     ink_sample(const InkSession *s, const InkSampleParams *p, uint64_t *rng_state);

const char *ink_error(const InkSession *s);
double      ink_last_gpu_seconds(const InkSession *s);

#endif
