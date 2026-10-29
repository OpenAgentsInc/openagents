#ifndef PSIONIC_ATTENTION_MEMORY_H
#define PSIONIC_ATTENTION_MEMORY_H
#include "attention.h"
/* Valid allocations, extents, alignment, lifetimes and exclusive access belong
   to the caller. Only address overlap is observed here; Coax owns admission. */
coax_t_PaResult psionic_attention_step(coax_t_PaShape, coax_v_f32, coax_v_f32,
    coax_v_f32, int64_t, coax_v_f32, coax_v_f32);
coax_t_PaResult psionic_attention_forward(coax_t_PaShape, coax_v_f32, coax_v_f32,
    coax_v_f32, int64_t, coax_v_f32);
#endif
