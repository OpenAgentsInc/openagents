#ifndef PSIONIC_MXFP4_MEMORY_H
#define PSIONIC_MXFP4_MEMORY_H
#include "mxfp4.h"
coax_t_MxResult psionic_mxfp4_decode(int64_t rows, int64_t columns,
    coax_v_u8 packed, coax_v_u8 scales, coax_v_f32 out);
#endif
