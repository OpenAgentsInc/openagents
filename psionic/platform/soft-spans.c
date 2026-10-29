/* Observe native span overlap; the limit scan's arithmetic stays in Coax. */
#include "soft-spans.h"
#include "spans.h"

coax_t_SmLimit psionic_sm_limit(int64_t k, int64_t v, double beta, coax_t_SmFormat format,
        coax_v_s64 keys, coax_v_s64 values) {
    bool alias=psionic_spans_overlap(keys.data,keys.count,sizeof(int64_t),values.data,values.count,sizeof(int64_t));
    return sm_limit(k,v,beta,format,keys,values,alias);
}
