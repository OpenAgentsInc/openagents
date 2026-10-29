#include "projection.h"
#include <stdint.h>
#include <string.h>
static uint8_t input[1048576],output[1048576],scratch[262144],owner[32];
static int effects;
uint8_t *npa_test_input(void){return input;}
uint8_t *npa_test_output(void){return output;}
uint8_t *npa_test_owner_key(void){return owner;}
int64_t npa_test_run(int64_t count){return npa_test_roundtrip((coax_v_u8){input,count},(coax_v_u8){output,sizeof output});}
int npa_test_signed(int64_t count){return npa_test_verify((coax_v_u8){input,count},(coax_v_u8){scratch,sizeof scratch});}
void npa_test_reset_effect(void){effects=0;}
int npa_test_effect_count(void){return effects;}
int npa_test_admit(int64_t count){int ok=npa_test_owner((coax_v_u8){input,count},(coax_v_u8){owner,sizeof owner},(coax_v_u8){scratch,sizeof scratch});if(ok)effects++;return ok;}
