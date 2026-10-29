#ifndef PSIONIC_DEEPSEEK_V2_MATH_MEMORY_H
#define PSIONIC_DEEPSEEK_V2_MATH_MEMORY_H
#include "deepseek-v2-math.h"
coax_t_MlResult psionic_ml_attention(coax_t_MlShape,coax_t_MlMode,coax_v_u8,
    coax_v_f32,coax_v_f32,coax_v_f32,coax_v_f32,int64_t,double,
    coax_v_f32,coax_v_f32,coax_v_f32,coax_v_f64);
coax_t_DvRotateResult psionic_dv_rotary(int64_t,int64_t,int64_t,coax_v_f32,coax_v_f32,int64_t,coax_v_f32,coax_v_f32,coax_v_f32);
coax_t_DsResult psionic_dv_yarn(int64_t,int64_t,coax_v_f32,coax_v_f32);
#endif
