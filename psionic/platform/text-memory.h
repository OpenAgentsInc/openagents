#ifndef PSIONIC_TEXT_MEMORY_H
#define PSIONIC_TEXT_MEMORY_H
#include "text.h"
/* qwen35-text-v1, synchronous borrowed storage. Admit the exact immutable table
   with pt_admit once, keep its bytes alive and unchanged for all subsequent
   calls. Read-only spans may alias. Every declared writable span (including
   tails) is disjoint from all other spans. Pointers/extents/alignment/lifetime
   and exclusive access remain caller obligations. Workspaces may be dirty on
   refusal; published output and zero-result metadata are atomic. */
coax_t_PtResult psionic_text_encode(coax_v_u8 table,coax_string input,
    coax_v_s64 output,coax_v_s64 stage,coax_v_u8 normal,coax_v_s64 scalars,coax_v_s64 symbols);
coax_t_PtResult psionic_text_normalize(coax_v_u8 table,coax_string input,
    coax_v_u8 output,coax_v_s64 scalars);
coax_t_PtPrompt psionic_text_render(coax_t_PtEntry state,coax_t_PtQuestion question,
    coax_v_t_PtOption options,coax_v_u8 output,coax_v_u8 scratch);
coax_t_PtSplit psionic_text_split(coax_v_s64 candidate,coax_v_s64 full,
    coax_v_s64 ends,coax_v_t_PtBranch output);
#endif
