#include "generated.c"
#include <stdint.h>
__attribute__((export_name("decode")))
int decode(uint32_t input,uint32_t n,uint32_t output,uint32_t cap) {
 coax_t_NC_Result r=coax_p_fi_b64url_decode((coax_string){(const char *)(uintptr_t)input,n},(coax_v_u8){(uint8_t *)(uintptr_t)output,cap});
 return r.error ? -1 : (int)r.written;
}
