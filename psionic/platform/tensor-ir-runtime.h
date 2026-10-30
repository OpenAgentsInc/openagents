#ifndef PSIONIC_TENSOR_IR_RUNTIME_H
#define PSIONIC_TENSOR_IR_RUNTIME_H
#include "tensor-ir.h"
#include <stddef.h>
#include <stdbool.h>
/* Exclusive host access. The compiled object owns its graph, plan, module and
   persistent buffers. Calls complete before returning; no implicit placement.
   The compiler digest must identify the trusted producer of the generated code.
   Module bytes and observed backend identity are hashed by Coax at open. */
typedef struct Pti Pti;
/* Tracks this object's five owned payload buffers; driver scratch is separate. */
typedef struct {uint64_t allocations,runs,module_compiles;int64_t buffer_bytes,workspace_bytes;} PtiStats;
typedef struct {
    void *context;uint64_t digest[4];
    coax_t_TiStatus (*admit)(void*,coax_t_TiPlan);
    coax_t_TiStatus (*acquire)(void*);
    void (*release)(void*);
    void (*cpu)(void*,const coax_t_TiLaunch*,bool,coax_v_f32,coax_v_f32,coax_v_f32,coax_v_u8);
    void *(*kernel)(void*,int64_t);
} PtiExtensions;
coax_t_TiStatus pti_attach(Pti *program,const PtiExtensions *extensions);
coax_t_TiStatus pti_open(coax_t_TiProgram program,coax_t_TiTarget target,bool reference,const char *module,size_t bytes,Pti **out);
/* Specializations share immutable code; closing its owner is refused until
   every derived handle has closed. Each derived plan owns independent buffers. */
coax_t_TiStatus pti_derive(Pti *owner,coax_t_TiProgram program,Pti **out);
coax_t_TiStatus pti_run(Pti *program,const float *input,int64_t count,float *out,int64_t capacity);
coax_t_TiPlan pti_info(const Pti *program);
PtiStats pti_stats(const Pti *program);
bool pti_matches(const Pti *cached,coax_t_TiProgram program,coax_t_TiTarget target);
coax_t_TiStatus pti_close(Pti **program);
#endif
