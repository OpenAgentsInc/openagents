#ifndef PSIONIC_TENSOR_OPS_MEMORY_H
#define PSIONIC_TENSOR_OPS_MEMORY_H
#include "tensor-ops.h"
/* Read-only two-owner/two-view records, truthful aligned backing spans and
   exclusive synchronous access. Unused B is a live empty view. Results are
   fresh/unpublished spans; replace registered tensors through tv_scatter.
   Coax owns shapes, arithmetic, gradients, work and publication. */
coax_t_ToResult to_forward(coax_t_ToRequest request,coax_v_t_TvStorage owners,coax_v_t_TvView views,
    coax_v_f32 a,coax_v_f32 b,coax_v_s64 indices,coax_v_f32 out,coax_v_f32 scratch,coax_v_s64 aux,coax_v_s64 aux_stage);
coax_t_ToGradient to_vjp(coax_t_ToRequest request,coax_v_t_TvStorage owners,coax_v_t_TvView views,
    coax_v_f32 a,coax_v_f32 b,coax_v_s64 indices,coax_v_f32 dy,coax_v_f32 ga,coax_v_f32 gb,coax_v_f32 sa,coax_v_f32 sb);
#endif
