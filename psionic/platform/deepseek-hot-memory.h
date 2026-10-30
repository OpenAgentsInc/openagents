#ifndef PSIONIC_DEEPSEEK_HOT_MEMORY_H
#define PSIONIC_DEEPSEEK_HOT_MEMORY_H
#include "deepseek-hot.h"
/* File/packet identity and immutable lifetimes are caller obligations. A packet
 * is lossless original FP4 bytes, never a different quantization profile. */
coax_t_DeResult psionic_dc_expert(int64_t width,int64_t hidden,coax_v_u8 packet,coax_v_f32 input,float route,float limit,coax_v_f32 output,coax_t_DeWork work);
coax_t_DcResult psionic_dc_seed(coax_t_DcConfig,coax_v_s64,coax_v_s64,coax_v_s64,coax_v_s64,coax_v_s64);
coax_t_DcResult psionic_dc_dispatch(coax_t_DcConfig,coax_t_DcState,int64_t,coax_v_s64,coax_v_t_DcLane,coax_v_t_DcLane);
coax_t_DcResult psionic_dc_prefetch(coax_t_DcConfig,coax_t_DcState,int64_t,coax_v_s64,coax_v_s64,coax_v_s64);
coax_t_DcResult psionic_dc_finish(coax_t_DcConfig,coax_t_DcState,coax_v_s64,coax_v_s64,coax_v_s64);
coax_t_DcPlan psionic_dc_plan(coax_t_DcConfig,coax_t_DcState,coax_v_s64,coax_v_t_DcMove,coax_v_s64,coax_v_t_DcMove);
coax_t_DcResult psionic_dc_install(coax_t_DcConfig,coax_t_DcState,coax_t_DcTicket,coax_v_s64,coax_v_t_DcMove,coax_v_u8,coax_v_u8,coax_v_u8);
coax_t_DcResult psionic_dc_fetch(int fd,_Bool mapped,int64_t keys,int64_t stride,int64_t base,int64_t file_bytes,coax_v_s64 ids,coax_v_u8 output,coax_v_u8 packet_work,coax_v_s64 offsets_work);
#endif
