/* The Granite dense decoder on Metal (granite-metal-f32-v1), and the same
   launches on the CPU as its reference. The kernels are Coax
   (psionic/lib/granite/kernels.coax): `coax metal-compute` for the GPU and
   `coax c --library` for the CPU, which runs each work item of the same
   launch list in turn. This file only allocates, records launches and waits;
   it does no arithmetic on activations. Include the generated host.h first. */
#ifndef PSIONIC_GRANITE_METAL_H
#define PSIONIC_GRANITE_METAL_H
#include <stddef.h>
#include <stdint.h>
#include "granite-file.h"

typedef enum { GR_METAL = 0, GR_CPU = 1 } GrBackend;
typedef struct GrSession GrSession;
/* Called after each layer with the residual stream of the call's tokens. */
typedef void (*GrLayerDump)(void *context, int layer, const float *x, int64_t tokens, int64_t width);

/* 0 on success; otherwise a message in error. capacity is the most positions
   a conversation may hold, a multiple of 256. metal_source is coax
   metal-compute output (unused on the CPU). */
int gr_session_open(const char *path, int64_t capacity, GrBackend backend, const char *metal_source, size_t metal_bytes,
                    GrSession **out, char *error, size_t error_bytes);
void gr_session_close(GrSession *s);
const GrFile *gr_session_file(const GrSession *s);
int64_t gr_session_position(const GrSession *s);
void gr_session_reset(GrSession *s);
/* Appends tokens at the current position and leaves the last token's
   full-vocabulary logits in gr_session_logits and its greedy choice in
   *next (-2 when a logit is not finite). seconds is the wall time of the
   call. 0 on success. */
int gr_session_eval(GrSession *s, const int64_t *ids, int64_t count, int64_t *next, GrLayerDump dump, void *context, double *seconds);
const float *gr_session_logits(const GrSession *s);
const char *gr_session_error(const GrSession *s);
/* GPU time of every launch so far. */
double gr_session_gpu_seconds(const GrSession *s);
double gr_session_bench_kernel(GrSession *s, const char *name, int per, int index, int reps, int trials);
double gr_session_bench_product(GrSession *s, int index, int64_t tokens, int reps);
#endif
