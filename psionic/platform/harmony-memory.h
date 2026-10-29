#ifndef PSIONIC_HARMONY_MEMORY_H
#define PSIONIC_HARMONY_MEMORY_H
#include <stdbool.h>
#include "harmony.h"
coax_t_PtResult psionic_harmony_encode(coax_v_u8 table,coax_string text,coax_v_s64 out,coax_v_s64 stage,coax_v_s64 symbols);
coax_t_PtResult psionic_harmony_decode(coax_v_u8 table,coax_v_s64 tokens,coax_v_u8 out,coax_v_u8 stage);
coax_t_PtResult psionic_harmony_system(int64_t effort,coax_v_u8 out,coax_v_u8 work);
coax_t_HmResult psionic_harmony_render(coax_v_u8 table,coax_v_t_HmMessage messages,bool completion,coax_v_s64 out,coax_v_s64 stage,coax_v_s64 symbols);
coax_t_HmResult psionic_harmony_parse(coax_v_u8 table,coax_v_s64 tokens,bool completion,coax_v_t_HmRecord out,coax_v_t_HmRecord draft,coax_v_u8 bytes,coax_v_u8 work);
#endif
