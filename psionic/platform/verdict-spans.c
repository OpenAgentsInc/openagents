/* The gates read spans and write none, so there is nothing to overlap: the
   adapter only names a native array as scores. */
#include "verdict-spans.h"

coax_v_f64 psionic_vd_scores(double *scores, int64_t count) {
    return (coax_v_f64){scores,count};
}
