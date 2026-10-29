#ifndef PSIONIC_GPTOSS_DEVICE_H
#define PSIONIC_GPTOSS_DEVICE_H
#include "cuda-host.h"
#include "device-host.h"
typedef struct PgSession PgSession;
/* All admission/shape/dispatch decisions come from Coax. A nonzero PcStatus
   is a platform failure. An unsuccessful why is a Coax refusal. No fallback. */
PcStatus pg_open(coax_t_GProfile profile,coax_v_t_GFile files,const char *cuda,size_t cuda_bytes,size_t budget,PgSession **out,coax_t_GResult *why);
PcStatus pg_reset(PgSession *session);
PcStatus pg_step(PgSession *session,int64_t token,int64_t position,int choose,int64_t limit,int wait,int64_t control[8]);
/* IDs validated before upload. Each bounded chunk commits atomically;
   control carries logical failures, separate from a platform failure. */
typedef int (*PgPoll)(void *context);
/* Poll observes a Coax cancellation/deadline decision between committed chunks. */
PcStatus pg_prefill_poll(PgSession *session,const int64_t *tokens,int64_t count,int64_t position,int choose,int64_t limit,int64_t control[8],PgPoll poll,void *context,int *interrupted);
PcStatus pg_prefill(PgSession *session,const int64_t *tokens,int64_t count,int64_t position,int choose,int64_t limit,int64_t control[8]);
PcStatus pg_read(PgSession *session,int slot,void *out,size_t bytes);

PcStatus pg_wait(PgSession *session);
void pg_close(PgSession *session);
coax_t_GDPlan pg_plan(const PgSession *session);
coax_t_GShape pg_shape(const PgSession *session);
const char *pg_error(const PgSession *session);
#ifdef PSIONIC_DEVICE_TEST
void pg_profile(PgSession *session,int enable);
PcStatus pg_patch(PgSession *session,size_t offset,const void *bytes,size_t count);
PcStatus pg_open_cpu(coax_t_GProfile profile,coax_v_t_GFile files,PgSession **out,coax_t_GResult *why);
#endif
#endif
