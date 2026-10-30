#ifndef PSIONIC_DEEPSEEK_PREFILL_MEMORY_H
#define PSIONIC_DEEPSEEK_PREFILL_MEMORY_H
#include "deepseek-prefill.h"
coax_t_DpResult psionic_dp_project(coax_t_DeMatrix,int64_t,coax_v_f32,coax_v_f32,coax_v_f64,coax_v_f32,coax_v_u8);
coax_t_DpResult psionic_dp_expert(coax_t_DeExpert,int64_t,coax_v_f32,coax_v_f32,float,coax_v_f32,coax_t_DpWork);
coax_t_DpResult psionic_dp_groups(int64_t,int64_t,int64_t,coax_v_s64,coax_v_f32,coax_v_s64,coax_v_s64,coax_v_s64,coax_v_s64,coax_v_s64,coax_v_s64);
coax_t_DpResult psionic_dp_gather(int64_t,int64_t,int64_t,int64_t,int64_t,coax_v_s64,coax_v_f32,coax_v_s64,coax_v_s64,coax_v_f32,coax_v_f32,coax_v_f32);
coax_t_DpResult psionic_dp_scatter(int64_t,int64_t,int64_t,int64_t,int64_t,coax_v_s64,coax_v_s64,coax_v_s64,coax_v_f32,coax_v_f32);
coax_t_DpResult psionic_dp_combine(int64_t,int64_t,int64_t,int64_t,coax_v_s64,coax_v_f32,coax_v_u8,_Bool,coax_v_f32,coax_v_f32,coax_v_f32,coax_v_f32);
#endif
