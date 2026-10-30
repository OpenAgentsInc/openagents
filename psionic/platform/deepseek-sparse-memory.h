#ifndef PSIONIC_DEEPSEEK_SPARSE_MEMORY_H
#define PSIONIC_DEEPSEEK_SPARSE_MEMORY_H
#include "deepseek-sparse.h"
coax_t_DaSelection psionic_da_select(coax_v_f32 scores,int64_t visible,coax_v_u8 mask,int64_t keep,coax_v_s64 out,coax_v_s64 heap);
coax_t_DaSelection psionic_da_candidates(coax_v_f32 scores,int64_t visible,int64_t block,int64_t keep,coax_v_u8 out,coax_v_u8 stage,coax_v_f32 blocks,coax_v_s64 heap);
coax_t_DkResult psionic_da_norm(coax_v_u16 input,coax_v_u16 weight,coax_v_u16 out,coax_v_u16 stage);
coax_t_DaResult psionic_da_pool(int64_t ratio,int64_t position,coax_v_f32 kv,coax_v_f32 gate,coax_v_f32 old_kv,coax_v_f32 old_gate,coax_v_u16 norm,coax_v_u16 latent,coax_v_f32 next_kv,coax_v_f32 next_gate,coax_t_DaPoolWork work);
coax_t_DaResult psionic_da_pool_chunk(int64_t ratio,int64_t position,int64_t chunk,int64_t width,coax_v_f32 kv,coax_v_f32 gate,coax_v_f32 old_kv,coax_v_f32 old_gate,coax_v_u16 norm,coax_v_u16 latent,coax_v_f32 next_kv,coax_v_f32 next_gate,coax_t_DaPoolWork work);
coax_t_DkResult psionic_da_rotate(_Bool compressed,int64_t position,int64_t heads,int64_t width,int64_t rope,_Bool inverse,coax_v_u16 input,coax_v_u16 out,coax_v_u16 stage);
coax_t_DkResult psionic_da_store(uint8_t kind,int64_t rows,int64_t width,int64_t row,coax_v_u16 input,coax_t_DkPacked out,coax_v_u8 stage);
coax_t_DkResult psionic_da_index(int64_t heads,int64_t width,int64_t rows,int64_t visible,coax_t_DkPacked query,coax_t_DkPacked keys,coax_v_u16 weights,coax_v_f32 out,coax_v_f32 stage);
coax_t_DkResult psionic_da_attend(coax_t_DaAttention shape,int64_t position,coax_v_u16 query,coax_t_DkPacked window,int64_t rows,coax_t_DkPacked compressed,coax_v_s64 ids,coax_v_f32 sink,coax_v_u16 out,coax_t_DaAttentionWork work);
#endif
