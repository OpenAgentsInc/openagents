/* Elephant decisions on an Apple GPU through Metal, for any host: open once,
   then decide prepared requests one at a time. The same interface as the
   CUDA engine (elephant-gpu.h), so psionic-serve links one or the other. The
   Coax device plan (psionic/lib/device/plan.coax) decides every launch; this
   module performs the effects. On unified memory every weight is resident:
   a plan that would stream is refused. The CPU session (chapter 10)
   authenticates and binds the original files before eg_place. */
#ifndef PSIONIC_ELEPHANT_METAL_H
#define PSIONIC_ELEPHANT_METAL_H
#include "metal-host.h"
#include "decision-answer.h"
#include "plan.h"

#define EG_KERNELS 21   /* DP_KERNELS: the CUDA engine's 19 and the two matrix-unit tiles */
#define EG_WEIGHTS 514  /* DP_WEIGHTS */
#define EG_STREAMABLE 277 /* DP_STREAMABLE */
#define EG_WIDTH 4096   /* DP_WIDTH */
#define EG_VOCAB 248320 /* DP_VOCAB */

enum { EG_OK = 0, EG_SKIP = 1, EG_FAILED = 2, EG_STOPPED = 3, EG_REFUSED = 4 };

/* The four generated Metal modules: coax metal-compute of project, layer, warp and tile.coax. */
typedef struct { const char *text[4]; size_t length[4]; } EgSources;

typedef struct {
    coax_t_QxModel model;
    int64_t max_tokens, budget, op_cap;
    PmDeviceInfo device;
    PmModule modules[4];
    PmKernel kernels[EG_KERNELS];
    coax_t_DpPlan placed, plan;
    _Bool stream[EG_STREAMABLE];
    coax_t_DpOp *ops;
    PmBuffer workspace, arena;
    int64_t offset[EG_WEIGHTS]; /* each resident weight's byte offset in the arena */
    int64_t bytes[EG_WEIGHTS];
    float logits[EG_VOCAB];
    float *rope_work;
    int64_t tokens, prefix;
    double compile_seconds, upload_seconds, gpu_seconds; /* gpu_seconds: the last decision's command buffers */
    int64_t resident_bytes, pinned_bytes; /* pinned_bytes stays 0: nothing streams */
} EgEngine;

/* Open the device and compile the kernels; EG_SKIP without a Metal device. */
int eg_compile(EgEngine *e, const EgSources *src);
/* Place the authenticated session's weights for decisions of up to
   max_tokens tokens within budget bytes (0: the device's recommended working
   set less 4 GiB). EG_REFUSED if the plan would stream any weight. */
int eg_place(EgEngine *e, coax_t_QxModel model, int64_t max_tokens, int64_t budget);
const char *eg_log(void);
const void *eg_weight(EgEngine *e, int64_t w);
/* Plan one decision and write its inputs (embedding rows and the host-built
   rotary table, both computed by Coax) into the shared workspace. */
int eg_prepare(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix);
/* Encode operations [from, to) of the current plan into batch. */
int eg_encode(EgEngine *e, PmBatch batch, int64_t from, int64_t to);
/* Prepare, run every operation and copy the logits to e->logits. stop (may
   be null) is asked before each layer; when it returns true the decision is
   abandoned with nothing published and EG_STOPPED is returned. */
int eg_decide(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix, int (*stop)(void *), void *context);
int eg_layer_start(const EgEngine *e, int64_t i);

#endif
