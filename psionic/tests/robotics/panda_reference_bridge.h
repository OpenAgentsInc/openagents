/* Opt-in development oracle, never a shipped or serving NN runtime. */
#ifndef PSIONIC_PANDA_REFERENCE_BRIDGE_H
#define PSIONIC_PANDA_REFERENCE_BRIDGE_H
#include "panda_loop_provider.h"
typedef struct PandaReference PandaReference;
/* Coax-prepared pixels512x1536, source IDs152, normalized state132 and explicit
   noise5280. Callback writes ALL5280 normalized actions. Effects return1/0.
   Caller completes GPU work before returning; borrowed views then expire. */
typedef int (*PandaReferenceInfer)(void *,const float *,const int64_t *,int64_t,const float *,const float *,float *);
PandaReference *panda_reference_open(const char *,const char *,PandaReferenceInfer,void *,char *,size_t);
int panda_reference_forward(PandaReference *,const uint8_t *,const uint8_t *,const float *,const float *,float *,PandaPolicyTiming *);
const char *panda_reference_error(const PandaReference *);
int panda_reference_copy(PandaReference *,const char *,void *,size_t);
void panda_reference_close(PandaReference *);
/* Same CLI positions/case effects as native runner, with pre-opened provider.
   This runner returns a CLI exit code:0 means success, unlike effect booleans. */
int panda_reference_run(PandaReference *,int,char **,double);
#endif
