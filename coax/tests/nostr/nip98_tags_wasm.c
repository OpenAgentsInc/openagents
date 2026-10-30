/* Test-only WASM transport; parsed tags carry no trust. */
#include "tags.h"
#include <stdint.h>
__attribute__((export_name("tags"))) int tags(uint32_t text,int length,uint32_t up,int un,uint32_t mp,int mn,uint32_t pp,int pn,int overlap,uint32_t rp){
  coax_t_N98SourceTags t=n98_source_tags((coax_string){(const char *)(uintptr_t)text,length},
    (coax_v_u8){(uint8_t *)(uintptr_t)up,un},(coax_v_u8){(uint8_t *)(uintptr_t)mp,mn},
    (coax_v_u8){(uint8_t *)(uintptr_t)pp,pn},overlap!=0);
  int32_t *r=(int32_t *)(uintptr_t)rp;r[0]=t.valid;r[1]=(int32_t)t.uc;r[2]=(int32_t)t.mc;r[3]=(int32_t)t.pc;
  r[4]=(int32_t)t.un;r[5]=(int32_t)t.mn;r[6]=(int32_t)t.pn;return t.valid;
}
