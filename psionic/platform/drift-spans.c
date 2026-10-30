/* Every exported drift procedure that writes caller storage writes one log
   and reads only scalars and structures, so there is no second span to
   overlap: the adapter only names a native array as a log. */
#include "drift-spans.h"

coax_v_t_DrRecord psionic_dr_log(coax_t_DrRecord *records, int64_t count) {
    return (coax_v_t_DrRecord){records,count};
}
