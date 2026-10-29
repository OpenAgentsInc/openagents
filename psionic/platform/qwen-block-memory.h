#ifndef PSIONIC_QWEN_BLOCK_MEMORY_H
#define PSIONIC_QWEN_BLOCK_MEMORY_H
#include "qwen-block.h"
coax_t_QmResult psionic_qwen_begin(int64_t,float,coax_v_f32,coax_v_f32,coax_v_f32,coax_v_f32);
coax_t_QmResult psionic_qwen_finish(coax_t_QmDtype,int64_t,int64_t,float,coax_v_f32,coax_v_f32,coax_v_f32,coax_v_u8,coax_v_u8,coax_v_u8,coax_v_f32,coax_v_f32);
#endif
