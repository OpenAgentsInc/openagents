/* Coax decides protocol behavior; C observes addresses only. */
#ifndef PSIONIC_DEEPSEEK_FRONT_MEMORY_H
#define PSIONIC_DEEPSEEK_FRONT_MEMORY_H
#include "deepseek-front.h"
coax_t_DfResult psionic_df_feed(coax_t_DfState, coax_v_s64,
    coax_v_t_DfEvent, coax_v_t_DfEvent);
coax_t_DfRecipe psionic_df_recipe(coax_v_t_DfMessage, _Bool,
    coax_v_t_DfSegment, coax_v_t_DfSegment);
coax_t_PtResult psionic_df_encode_r1(coax_v_u8, coax_v_t_DfSegment,
    coax_v_s64, coax_v_s64, coax_v_s64, coax_v_u8, coax_v_u8, coax_v_s64, coax_v_s64);
coax_t_PtResult psionic_df_r1_units(coax_v_u8, coax_v_s64, coax_v_s64, coax_v_s64);
#endif
