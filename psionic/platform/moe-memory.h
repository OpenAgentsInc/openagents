#ifndef PSIONIC_MOE_MEMORY_H
#define PSIONIC_MOE_MEMORY_H
#include "moe.h"
coax_t_MoResult psionic_moe_forward(coax_v_f32 logits, int64_t k,
    coax_v_f32 x, coax_v_t_MoExpert experts, int64_t budget, coax_v_f32 out);
#endif
