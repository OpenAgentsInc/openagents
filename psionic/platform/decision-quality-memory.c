#include "decision-quality-memory.h"
/* pq_measure only reads; the caller keeps both views valid for the call. */
coax_t_PqReport psionic_quality(coax_v_t_PqItem items,coax_v_f64 probabilities){return pq_measure(items,probabilities);}
