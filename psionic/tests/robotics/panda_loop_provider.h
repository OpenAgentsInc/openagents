/* Test-only inference effect seam. Scheduling/controller/SDK order stays in
   the shared loop and its existing Coax session. Effect booleans are1/0. */
#ifndef PSIONIC_PANDA_LOOP_PROVIDER_H
#define PSIONIC_PANDA_LOOP_PROVIDER_H
#include "panda-policy.h"
typedef struct {
 void *context;
 int (*forward)(void *,const uint8_t *,const uint8_t *,const float *,const float *,float *,PandaPolicyTiming *);
 const char *(*error)(const void *);
 const char *profile;
 const char *schema;
 int gpu_timing_measured;
} PandaLoopProvider;
#endif
