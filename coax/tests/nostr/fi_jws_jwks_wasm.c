#include "generated.c"
#include <stdint.h>
__attribute__((export_name("verify")))
int verify(uint32_t t, uint32_t tn, uint32_t j, uint32_t jn) {
 return fi_jws_document_signature((coax_v_u8){(uint8_t *)(uintptr_t)t,tn},(coax_string){(const char *)(uintptr_t)j,jn});
}
