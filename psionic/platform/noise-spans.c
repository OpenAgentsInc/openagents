/* Observe native span overlap; measuring stays in Coax. Only ns_means writes
   caller storage while reading a second span; ns_run reads only scalars. */
#include "noise-spans.h"
#include "spans.h"
#define OV(a,b) psionic_spans_overlap((a).data,(a).count,sizeof(*(a).data), \
    (b).data,(b).count,sizeof(*(b).data))

coax_t_NsStatus psionic_ns_means(coax_v_f64 scores, int64_t items, int64_t first, int64_t runs, coax_v_f64 out) {
    return ns_means(scores,items,first,runs,out,OV(scores,out));
}
