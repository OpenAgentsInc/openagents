#ifndef PSIONIC_DEEPSEEK_MEMORY_H
#define PSIONIC_DEEPSEEK_MEMORY_H
#include "deepseek.h"
coax_t_DsResult psionic_ds_step(coax_t_DsModel,int64_t,int64_t,coax_v_f32,coax_t_DsOutput,coax_t_DsWork);
coax_t_DsResult psionic_ds_bind(int64_t,coax_v_t_DsFile,coax_v_t_DsBlock,coax_v_t_DsBlock,coax_v_t_DsModel);
coax_t_PtResult psionic_ds_chat(coax_v_u8,coax_v_t_DsMessage,coax_v_s64,coax_v_s64,coax_v_s64,coax_v_u8,coax_v_s64,coax_v_s64);
coax_t_PtResult psionic_ds_encode(coax_v_u8,coax_string,coax_v_s64,coax_v_s64,coax_v_u8,coax_v_s64,coax_v_s64);
coax_t_PtResult psionic_ds_decode(coax_v_u8,coax_v_s64,coax_v_u8,coax_v_u8);
#endif
