/* The Psionic engine contract for Inkling 2 Mini (inkling2_loop GGUF).
   One server (inkling-serve.c) and one command-line runner (inkling-cli.c)
   are written against this header; each engine implements all of it:
     inkling-metal.c  Apple GPUs.  Kernels: psionic/lib/inkling/kernels.coax
                      through `coax metal-compute` and metal-host.m.
     inkling-cuda.c   NVIDIA GPUs on Linux and Windows.  Kernels:
                      psionic/lib/inkling/kernels-cuda.coax through
                      `coax cuda`, NVRTC and cuda-host.c.
   The `kernel_path` arguments name that engine's generated kernel source
   (inkling-kernels.metal or inkling-kernels-cuda.cu). */
#ifndef PSIONIC_INKLING_ENGINE_H
#define PSIONIC_INKLING_ENGINE_H
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

/* device_ram_mb is the memory the weights, KV cache and scratch must fit in:
   unified memory on Apple GPUs, VRAM on a discrete GPU. */
typedef struct {
    int64_t device_ram_mb;
    int64_t os_reserve_mb;
    int64_t weights_mb;
    int64_t kv_cache_mb;
    int64_t scratchpad_mb;
    int64_t total_mb;
    int64_t headroom_mb;
    int64_t max_context_len;
    int     full_slots;
    int     swa_slots;
} InkMemoryBudget;

/* The engine linked into this binary: "psionic-metal", "psionic-cuda" or
   "psionic-vulkan". A null session names the linked engine too. */
const char *ink_backend_name(const InkSession *s);

/* Open an Inkling 2 Mini GGUF checkpoint on this engine's device.
   kernel_path is the engine's generated kernel source (see the top of this file).
   max_ctx is the maximum context length for full-attention layers (e.g. 4096, 8192, 32768).
   loop_count_override > 0 overrides the checkpoint's middle-block loop count K (e.g. 1, 3, 5). */
int  ink_open(const char *gguf_path, const char *kernel_path, int max_ctx,
              InkSession **out, char *err, size_t err_bytes);
int  ink_open_ex(const char *gguf_path, const char *kernel_path, int max_ctx,
                 int loop_count_override, InkSession **out, char *err, size_t err_bytes);
const InkMemoryBudget *ink_memory_budget(const InkSession *s);
/* Compile kernel_path on the device and verify Coax-emitted ink_dv_nvfp4, ink_dv_mxfp8, and ink_dv_iq4
   against exact CPU reference dequantization (returns 0 on pass). */
int  ink_test_quant_kernels(const char *kernel_path, char *err, size_t err_bytes);
void ink_close(InkSession *s);
void ink_reset(InkSession *s);
void ink_prefix_save(InkSession *s, const int32_t *tokens, int n_tokens, int save_logits);
int  ink_prefix_restore(InkSession *s, const int32_t *tokens, int n_tokens);
/* Optional predicted-output token buffer (Cursor Instant Apply / OpenAI Predicted Outputs) for
   zero-cost exact speculative drafting during file rewrites. Pass tokens=NULL or n_tokens=0 to clear. */
void ink_set_prediction(InkSession *s, const int32_t *tokens, int n_tokens);
int  ink_step(InkSession *s, int32_t token_id, int compute_logits);
int  ink_prefill(InkSession *s, const int32_t *tokens, int n_tokens, int compute_last_logits);
/* Verify tokens[0..n_tokens-1] in one batched forward pass starting at current s->pos:
   tokens[0] is the already-sampled current token; tokens[1..n_tokens-1] are speculative drafts.
   Accepts drafts matching the target distribution (exact argmax when temperature <= 1e-6, or
   exact speculative rejection sampling when temperature > 0), rewinds recurrent state to
   pos0 + 1 + accepted, leaves s->logits at the last accepted token, and writes the verified
   next token into *out_next_token. Returns accepted draft count [0..n_tokens-1], or -1 on error. */
int  ink_verify_draft(InkSession *s, const int32_t *tokens, int n_tokens,
                      const InkSampleParams *p, uint64_t *rng_state,
                      int32_t *out_next_token);

const float      *ink_logits(const InkSession *s);
const float      *ink_hidden(const InkSession *s);
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
