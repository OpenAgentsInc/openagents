#ifndef PSIONIC_GPTOSS_ATTENTION_MEMORY_H
#define PSIONIC_GPTOSS_ATTENTION_MEMORY_H
#include "gptoss-attention.h"
coax_t_GaResult psionic_gptoss_attention(coax_t_GaShape s, int64_t position,
    coax_v_f32 q, coax_v_f32 k, coax_v_f32 v, coax_v_f32 sinks,
    coax_v_f32 cache, coax_v_f32 out, coax_v_f32 work);
coax_t_GoResult psionic_gptoss_rotate(int64_t heads, coax_v_f32 x,
    coax_v_f32 coefficients, coax_v_f32 out, coax_v_f32 work);
coax_t_GoResult psionic_gptoss_swiglu(coax_v_f32 pairs, coax_v_f32 out);
coax_t_GoResult psionic_gptoss_project(int64_t rows, int64_t columns,
    coax_v_f32 matrix, coax_v_f32 x, coax_v_f32 bias, coax_v_f32 out, coax_v_f32 work);
#endif
