#ifndef PSIONIC_QWEN_ATTENTION_MEMORY_H
#define PSIONIC_QWEN_ATTENTION_MEMORY_H
#include "qwen-attention.h"
/* Address observations only; caller owns truthful extents and lifetimes. */
coax_t_QfResult psionic_qwen_prepare(coax_t_QfShape,coax_v_f32,coax_v_f32);
coax_t_QfResult psionic_qwen_attend(coax_t_QfShape,int64_t,coax_v_f32,coax_v_u8,coax_v_f32,coax_v_f32);
#endif
