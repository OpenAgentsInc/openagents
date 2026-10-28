/* Flat test-only Wasm ABI. Arithmetic and metadata admission remain Coax. */
#include "scalar.h"
#include <stdint.h>
__attribute__((export_name("evaluate"))) int evaluate(int32_t op,uint32_t a,uint32_t b,uint32_t out){
return sc_evaluate(op,(coax_v_u8){(uint8_t *)(uintptr_t)a,32},(coax_v_u8){(uint8_t *)(uintptr_t)b,32},(coax_v_u8){(uint8_t *)(uintptr_t)out,32});}
