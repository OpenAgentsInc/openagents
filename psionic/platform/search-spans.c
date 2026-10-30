/* sr_search reads policies, tasks and tiers and writes rounds; sr_measure
   reads tasks and writes cells; sr_expand and sr_tasks write one array each.
   Inputs and outputs have different types, so nothing can overlap and the
   adapter only names native arrays as each kind of span. */
#include "search-spans.h"

coax_v_t_SrEntry psionic_sr_entries(coax_t_SrEntry *entries, int64_t count) {
    return (coax_v_t_SrEntry){entries,count};
}
coax_v_t_SrTask psionic_sr_tasks(coax_t_SrTask *tasks, int64_t count) {
    return (coax_v_t_SrTask){tasks,count};
}
coax_v_t_SrCell psionic_sr_cells(coax_t_SrCell *cells, int64_t count) {
    return (coax_v_t_SrCell){cells,count};
}
coax_v_t_SrTier psionic_sr_tiers(coax_t_SrTier *tiers, int64_t count) {
    return (coax_v_t_SrTier){tiers,count};
}
coax_v_t_SrRound psionic_sr_rounds(coax_t_SrRound *rounds, int64_t count) {
    return (coax_v_t_SrRound){rounds,count};
}
