#include "nfkc.h"
#include <stdint.h>
__attribute__((export_name("normalize"))) uint32_t normalize(uint32_t input,int32_t count,uint32_t scratch,int32_t points,uint32_t output,int32_t capacity,int32_t overlap,uint32_t result){
 coax_t_UnResult r=un_nfkc((coax_string){(const char*)(uintptr_t)input,count},(coax_v_u32){(uint32_t*)(uintptr_t)scratch,points},(coax_v_u8){(uint8_t*)(uintptr_t)output,capacity},overlap!=0);
 int32_t *out=(int32_t*)(uintptr_t)result;out[0]=(int32_t)r.error;out[1]=(int32_t)r.written;return(uint32_t)r.error;
}
