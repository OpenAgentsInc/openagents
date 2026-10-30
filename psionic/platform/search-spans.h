#ifndef PSIONIC_SEARCH_SPANS_H
#define PSIONIC_SEARCH_SPANS_H
#include "search.h"
coax_v_t_SrEntry psionic_sr_entries(coax_t_SrEntry *entries, int64_t count);
coax_v_t_SrTask psionic_sr_tasks(coax_t_SrTask *tasks, int64_t count);
coax_v_t_SrCell psionic_sr_cells(coax_t_SrCell *cells, int64_t count);
coax_v_t_SrTier psionic_sr_tiers(coax_t_SrTier *tiers, int64_t count);
coax_v_t_SrRound psionic_sr_rounds(coax_t_SrRound *rounds, int64_t count);
#endif
