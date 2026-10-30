#ifndef PSIONIC_TENSOR_DEVICE_RUNTIME_H
#define PSIONIC_TENSOR_DEVICE_RUNTIME_H
#include "tensor-device.h"
#include <stddef.h>
/* Opaque owned handles. One host thread/exclusive runtime access. Pointer
   copies do not retain; use ptd_view for an alias or ptd_to for an owned copy.
   GPU source must be the matching trusted Coax-generated module. */
typedef struct PtdSession PtdSession;
typedef struct PtdTensor PtdTensor;
typedef struct {coax_t_TdState state;coax_t_TvView view;} PtdInfo;
coax_t_TdStatus ptd_open(coax_t_TdDevice device,int64_t budget,const char *source,size_t bytes,PtdSession **out);
coax_t_TdStatus ptd_close(PtdSession **session);
coax_t_TdSessionState ptd_stats(const PtdSession *session);
PtdInfo ptd_info(const PtdTensor *tensor);
coax_t_TdStatus ptd_from_f32(PtdSession *session,coax_t_TvShape shape,coax_t_TdDtype dtype,const float *values,int64_t count,PtdTensor **out);
coax_t_TdStatus ptd_from_bytes(PtdSession *session,coax_t_TvShape shape,coax_t_TdDtype source_dtype,const uint8_t *values,int64_t bytes,coax_t_TdDtype dtype,PtdTensor **out);
coax_t_TdStatus ptd_to(PtdTensor *source,PtdSession *target,coax_t_TdDtype dtype,PtdTensor **out);
coax_t_TdStatus ptd_view(PtdTensor *source,coax_t_TvView view,PtdTensor **out);
coax_t_TdStatus ptd_random(PtdSession *session,coax_t_TvShape shape,coax_t_TdDtype dtype,coax_t_TdRng counter,PtdTensor **out);
/* Returns a pending handle. Export/placement/release require an explicit wait. */
coax_t_TdStatus ptd_program(PtdSession *session,PtdTensor *a,PtdTensor *b,coax_t_TdDropout request,PtdTensor **out);
coax_t_TdStatus ptd_wait(PtdTensor *tensor);
coax_t_TdStatus ptd_cancel(PtdTensor *tensor);
coax_t_TdStatus ptd_export(PtdTensor *tensor,uint8_t *out,int64_t capacity);
coax_t_TdStatus ptd_release(PtdTensor **tensor);
#endif
