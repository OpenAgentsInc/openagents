#ifndef PSIONIC_TENSOR_MEMORY_H
#define PSIONIC_TENSOR_MEMORY_H
#include "tensor.h"

/* Source is exactly the logical contiguous f32 tensor. Destination/scratch
   may have spare capacity. All views live throughout the synchronous call.
   Coax validates shape, extents, overlap observation and finite data. Native
   callers still own pointer validity, actual allocation extent and lifetime. */
coax_t_PsionicCopy psionic_tensor_copy(coax_t_PsionicShape shape,
    coax_v_f32 source, coax_v_f32 destination, coax_v_f32 scratch);
#endif
