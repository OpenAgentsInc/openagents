#ifndef PSIONIC_WEIGHTS_MEMORY_H
#define PSIONIC_WEIGHTS_MEMORY_H
#include "weights.h"
/* original-weights-v1, synchronous and borrowed: every view must describe a real
   live allocation; strings/raw files remain immutable until descriptors die.
   Read-only spans may alias each other. Every declared writable span, including
   spare capacity, is disjoint from all other spans. Native callers use these
   wrappers; Coax callers provide the same truthful overlap observation.
   Inventory publication is atomic; metadata workspaces may change on refusal.
   Decode preserves BOTH float spans on refusal. */
coax_t_PwResult psionic_weights_inspect(coax_v_u8 file, coax_v_t_PwTensor output, coax_v_t_PwTensor draft);
coax_t_PwResult psionic_weights_bundle(coax_t_PwIdentity id, coax_v_u8 config,
    coax_v_u8 tokenizer, coax_v_u8 index, coax_v_t_PwShard shards,
    coax_v_t_PwSpec specs, coax_v_t_PwTensor output, coax_v_t_PwTensor draft);
coax_t_PwResult psionic_weights_decode(coax_v_u8 file, coax_string name,
    int64_t first, int64_t count, coax_v_f32 output, coax_v_f32 scratch,
    coax_v_t_PwTensor inventory, coax_v_t_PwTensor draft);
#endif
