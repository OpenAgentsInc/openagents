/* Elephant decisions on a CUDA device, for any host: open once, then decide
   prepared requests one at a time. The Coax device plan
   (psionic/lib/device/plan.coax) decides residency, streaming and every
   launch; this module performs the effects. The CPU session (chapter 10)
   authenticates and binds the original files before eg_place. */
#ifndef PSIONIC_ELEPHANT_GPU_H
#define PSIONIC_ELEPHANT_GPU_H
#include "cuda-host.h"
#include "decision-answer.h"
#include "plan.h"

#define EG_KERNELS 19   /* DP_KERNELS */
#define EG_WEIGHTS 514  /* DP_WEIGHTS */
#define EG_WIDTH 4096   /* DP_WIDTH */
#define EG_VOCAB 248320 /* DP_VOCAB */

enum { EG_OK = 0, EG_SKIP = 1, EG_FAILED = 2, EG_STOPPED = 3, EG_REFUSED = 4 };

/* The three generated CUDA modules: coax cuda of project, layer and warp.coax. */
typedef struct { const char *text[3]; size_t length[3]; } EgSources;

typedef struct {
    coax_t_QxModel model;
    int64_t max_tokens, budget, op_cap;
    PcDeviceInfo device;
    PcModule modules[3];
    PcKernel kernels[EG_KERNELS];
    coax_t_DpPlan placed, plan;   /* the opening placement; the current decision */
    _Bool stream[96];
    coax_t_DpOp *ops;
    PcBuffer workspace, slots[2], arena;
    PcBuffer address[EG_WEIGHTS];
    void *pinned[EG_WEIGHTS];
    int64_t bytes[EG_WEIGHTS];
    PcStream compute, copy;
    PcEvent filled[2], released[2];
    float *x, *rope, *rope_work, *logits; /* pinned inputs and the logits */
    int64_t tokens, prefix;
    double compile_seconds, upload_seconds;
    int64_t resident_bytes, pinned_bytes, free_before;
} EgEngine;

/* Open the device and compile the kernels; run before the long CPU
   authentication so a missing driver or NVRTC is found first. Returns
   EG_SKIP without a driver or NVRTC, EG_FAILED with a message in pc_log(). */
int eg_compile(EgEngine *e, const EgSources *src);
/* Place the authenticated session's weights for decisions of up to
   max_tokens tokens within budget bytes (0: the free device memory less
   512 MiB; a larger budget is lowered to that). */
int eg_place(EgEngine *e, coax_t_QxModel model, int64_t max_tokens, int64_t budget);
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
/* Whether operation i begins a layer (or the final norm). */
int eg_layer_start(const EgEngine *e, int64_t i);

#endif
