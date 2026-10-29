#include "fi-projection.h"
#include <stdint.h>
static uint8_t input[1048576],output[1048576],modulus[256];
uint8_t *fpa_test_input(void){return input;}
uint8_t *fpa_test_output(void){return output;}
uint8_t *fpa_test_modulus(void){return modulus;}
int64_t fpa_test_run(int64_t n){return fpa_test_roundtrip((coax_v_u8){input,n},(coax_v_u8){output,sizeof output});}
int fpa_test_verified(int64_t n){return fpa_test_signature((coax_v_u8){input,n},(coax_v_u8){modulus,sizeof modulus});}
