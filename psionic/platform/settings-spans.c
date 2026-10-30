/* st_fit reads recorded rows and writes its trail of steps, which have
   different types, and st_simulate writes rows only: there is nothing to
   overlap, so the adapter only names native arrays as rows and as a trail. */
#include "settings-spans.h"

coax_v_t_StRow psionic_st_rows(coax_t_StRow *rows, int64_t count) {
    return (coax_v_t_StRow){rows,count};
}
coax_v_t_StStep psionic_st_trail(coax_t_StStep *steps, int64_t count) {
    return (coax_v_t_StStep){steps,count};
}
