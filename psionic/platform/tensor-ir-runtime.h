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
typedef struct {uint64_t allocations,runs;int64_t buffer_bytes,workspace_bytes;} PtiStats;
coax_t_TiStatus pti_open(coax_t_TiProgram program,coax_t_TiTarget target,bool reference,const char *module,size_t bytes,Pti **out);
coax_t_TiStatus pti_run(Pti *program,const float *input,int64_t count,float *out,int64_t capacity);
coax_t_TiPlan pti_info(const Pti *program);
PtiStats pti_stats(const Pti *program);
bool pti_matches(const Pti *cached,coax_t_TiProgram program,coax_t_TiTarget target);
coax_t_TiStatus pti_close(Pti **program);
#endif
