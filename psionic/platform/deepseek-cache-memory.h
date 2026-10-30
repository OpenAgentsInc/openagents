#ifndef PSIONIC_DEEPSEEK_CACHE_MEMORY_H
#define PSIONIC_DEEPSEEK_CACHE_MEMORY_H
#include "deepseek-cache.h"
coax_t_DkResult psionic_dk_pack(uint8_t kind,int64_t rows,int64_t width,coax_v_u16 input,coax_t_DkPacked output,coax_v_u8 stage);
coax_t_DkResult psionic_dk_unpack(uint8_t kind,int64_t rows,int64_t width,coax_t_DkPacked input,coax_v_u16 output,coax_v_u16 stage);
#endif
