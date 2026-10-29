#ifndef PSIONIC_TENSOR_VIEW_MEMORY_H
#define PSIONIC_TENSOR_VIEW_MEMORY_H
#include "tensor-view.h"
/* One authoritative storage record; truthful aligned spans, immutable inputs
   and exclusive synchronous access. All writes to its data use tv_scatter.
   Retire before freeing; a reused allocation needs a fresh identity/epoch.
   Coax owns admission, offsets, data publication and version changes. */
coax_t_TvResult tv_gather(coax_v_t_TvStorage owner,coax_t_TvView view,
    coax_v_f32 data,coax_v_f32 out,coax_v_f32 scratch);
coax_t_TvResult tv_scatter(coax_v_t_TvStorage owner,coax_t_TvView view,
    coax_v_f32 source,coax_v_f32 data,coax_v_f32 scratch);
#endif
