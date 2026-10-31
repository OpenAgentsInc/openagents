/* Elephant and Mammoth decisions on a CUDA device, for any host: open once,
   then decide prepared requests one at a time. The Coax device plan
   (psionic/lib/device/plan.coax) decides residency, streaming and every
   launch; this module performs the effects. The CPU session (chapter 14)
   authenticates and binds the original files before eg_place. */
#ifndef PSIONIC_ELEPHANT_GPU_H
#define PSIONIC_ELEPHANT_GPU_H
#include "cuda-host.h"
#include "decision-answer.h"
#include "plan.h"

#define EG_KERNELS 21   /* the first 21 of DP_KERNELS; the last two are Metal's */
#define EG_MAX_TOKENS 512 /* DP_MAX_TOKENS */
#define EG_WEIGHTS 1026 /* the most dp_weights of any model (Mammoth's) */
#define EG_STREAMABLE 527 /* the most dp_streamable of any model (DP_MAX_STREAMABLE) */
#define EG_VOCAB 248320 /* both models' vocabulary */
#define EG_BATCH 8 /* DP_BATCH: the most questions in one batched pass */

enum { EG_OK = 0, EG_SKIP = 1, EG_FAILED = 2, EG_STOPPED = 3, EG_REFUSED = 4 };

/* The three generated CUDA modules: coax cuda of project, layer and warp.coax. */
typedef struct { const char *text[3]; size_t length[3]; } EgSources;

typedef struct {
    coax_t_QxModel model;
    coax_t_DpModel shape;       /* dp_model(kind): the plan's shape */
    int64_t kind, weights, streamable, width, fork_buffer; /* from the plan for this kind */
    int64_t max_tokens, budget, op_cap;
    PcDeviceInfo device;
    PcModule modules[3];
    PcKernel kernels[EG_KERNELS];
    coax_t_DpPlan placed, plan;   /* the opening placement; the current decision */
    _Bool stream[EG_STREAMABLE];
    coax_t_DpOp *ops;
    PcBuffer workspace, slots[2], arena, fork; /* fork: the kept prefix state (dp_fork_bytes) */
    PcBuffer address[EG_WEIGHTS];
    void *pinned[EG_WEIGHTS];
    int64_t bytes[EG_WEIGHTS];
    PcStream compute, copy;
    PcEvent filled[2], released[2];
    float *x, *rope, *rope_work, *logits; /* pinned inputs and the logits */
    int64_t tokens, prefix;
    int64_t fork_tokens[EG_MAX_TOKENS], fork_length; /* the prefix the fork buffer holds; 0: none */
    int64_t forked, prefix_passes;                     /* the last decision forked; prefix passes run */
    int64_t batched;                                   /* questions in the last batched pass; 0: one at a time */
    double compile_seconds, upload_seconds;
    int64_t resident_bytes, pinned_bytes, free_before;
} EgEngine;

/* Open the device and compile the kernels; run before the long CPU
   authentication so a missing driver or NVRTC is found first. Returns
   EG_SKIP without a driver or NVRTC, EG_FAILED with a message in pc_log(). */
int eg_compile(EgEngine *e, const EgSources *src);
/* Place the authenticated session's weights for decisions of up to
   max_tokens tokens within budget bytes (0: the free device memory less
   256 MiB; a larger budget is lowered to that). */
int eg_place(EgEngine *e, coax_t_QxModel model, int64_t max_tokens, int64_t budget);
/* The same for plan kind 0 (Elephant, as eg_place) or 1 (Mammoth); refused
   when the session's model does not have that kind's shape. */
int eg_place_kind(EgEngine *e, int64_t kind, coax_t_QxModel model, int64_t max_tokens, int64_t budget);
/* Why the last call refused or failed. */
const char *eg_log(void);
/* The session's bytes for plan weight w. */
const void *eg_weight(EgEngine *e, int64_t w);
/* Plan one decision and stage its inputs: embedding rows and rotary
   coefficients, computed by Coax. EG_REFUSED if the plan refuses its shape. */
int eg_prepare(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix);
/* Issue operation i of the current plan on the engine's streams. */
int eg_issue(EgEngine *e, int64_t i, PcStream compute);
/* Prepare, upload, run every operation and bring the logits back to
   e->logits. stop (may be null) is asked before each layer; when it returns
   true the decision is abandoned, the streams are drained and EG_STOPPED is
   returned with nothing published. */
int eg_decide(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix, int (*stop)(void *), void *context);
/* The same decision through a fork (chapter 12): the prefix block (tokens
   0..prefix) runs once and is kept; this and every later decision with the
   same prefix tokens runs only its suffix from the kept state. Without a
   prefix (0) it is eg_decide. e->forked says which way it ran. */
int eg_decide_fork(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix, int (*stop)(void *), void *context);
/* Many questions at once (chapter 36): questions[i] has counts[i] tokens,
   and all share their first `prefix` tokens. The prefix pass runs unless the
   fork buffer holds it; then every question's suffix runs in one batched
   pass (dp_plan_batch) and question i's logits land at logits + i*EG_VOCAB.
   A batch the plan does not admit (or questions with different prefixes)
   runs one eg_decide_fork at a time; e->batched says which way it ran. */
int eg_decide_batch(EgEngine *e, const int64_t *const *tokens, const int64_t *counts, int64_t questions, int64_t prefix, float *logits, int (*stop)(void *), void *context);
/* Whether operation i begins a layer (or the final norm). */
int eg_layer_start(const EgEngine *e, int64_t i);

#endif
