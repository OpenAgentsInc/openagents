/* Test-only ABI transport. All parsing, hashing and verification remain Coax. */
#include "event.h"
#include <stdint.h>
#include <string.h>
__attribute__((export_name("event"))) int event(uint32_t p,int length,uint32_t out,int capacity,int overlap,int verify,uint32_t result){
  coax_string text={(const char*)(uintptr_t)p,length};coax_v_u8 bytes={(uint8_t*)(uintptr_t)out,capacity};
  coax_t_NostrEvent e=verify?nostr_event_verify_vartime(text,bytes,overlap):nostr_event_read(text,bytes,overlap);
  int32_t *r=(int32_t*)(uintptr_t)result;r[0]=(int)e.error;r[1]=(int)e.written;r[2]=e.verified;r[3]=(int)e.tags;
  r[4]=(int)e.kind;r[5]=(int)e.content_from;r[6]=(int)e.content_to;
  memcpy(r+8,e.computed_id.e,32);return (int)e.error;
}
