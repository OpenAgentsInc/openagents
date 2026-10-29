#ifndef PSIONIC_GPTOSS_MEMORY_H
#define PSIONIC_GPTOSS_MEMORY_H
#include "gptoss.h"
coax_t_GStep psionic_gptoss_forward(coax_t_GModel model,int64_t token,int64_t position,int64_t budget,coax_v_f32 cache,coax_t_GOutput out,coax_t_GWork work);
coax_t_GResult psionic_gptoss_bind(uint8_t target,int64_t capacity,coax_v_t_GFile files,coax_v_t_GBlock blocks,coax_v_t_GBlock draft,coax_v_t_GModel out);
coax_t_GEmission psionic_gptoss_emit(coax_v_f32 logits,coax_v_u8 mask,coax_t_GSampling options,coax_t_GGeneration state,_Bool cancel,coax_v_f64 probabilities,coax_t_GSampleWork work);
coax_t_GResult psionic_gptoss_mask(coax_v_string alternatives,coax_string prefix,coax_v_string pieces,coax_v_u8 out,coax_v_u8 stage);
coax_t_GResult psionic_gptoss_display(coax_t_HmRecord record,coax_string bytes,coax_v_u8 out);
coax_t_GSpecBlock psionic_gptoss_block(int64_t vocab,coax_v_f64 target,coax_v_f64 draft,coax_v_s64 proposals,coax_v_f64 uniforms,_Bool greedy,coax_v_s64 out,coax_v_s64 stage,coax_v_f64 residual,coax_v_f64 scratch);
coax_t_GResult psionic_gptoss_snapshot(coax_v_f32 input,coax_v_f32 out);
#endif
