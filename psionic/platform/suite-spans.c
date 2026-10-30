/* su_append writes one store and reads only structures, so there is no second
   span to overlap: the adapter only names a native array as a store. */
#include "suite-spans.h"

coax_v_t_SuReceipt psionic_su_store(coax_t_SuReceipt *receipts, int64_t count) {
    return (coax_v_t_SuReceipt){receipts,count};
}
