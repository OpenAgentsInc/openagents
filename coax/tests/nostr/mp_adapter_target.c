#include "mp_adapter.h"
#include <stdint.h>
static uint8_t input[1048576], output[1048576];
uint8_t *mpa_input(void) { return input; }
uint8_t *mpa_output(void) { return output; }
int64_t mpa_test_run(int64_t n) { return mpa_test_copy((coax_v_u8){input,n},(coax_v_u8){output,sizeof output}); }
