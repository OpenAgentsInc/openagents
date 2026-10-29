#ifndef PSIONIC_DEEPSEEK_HYPER_MEMORY_H
#define PSIONIC_DEEPSEEK_HYPER_MEMORY_H
#include "deepseek-hyper.h"
coax_t_DhMixResult psionic_dh_prepare(int64_t,coax_v_f32,coax_t_DhCarry,coax_t_DhWeights,coax_v_f32,coax_v_f32);
coax_t_DhFinish psionic_dh_finish(int64_t,coax_v_f32,coax_v_f32,coax_t_DhMix,coax_v_f32,coax_v_f32);
coax_t_DsResult psionic_dh_collapse(int64_t,coax_v_f32,coax_t_DhCarry,coax_v_f32,coax_v_f32);
#endif
