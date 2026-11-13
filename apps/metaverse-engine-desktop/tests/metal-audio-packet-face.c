// Firm reason: 6 (test-only native face storage/length fault observations).
// Reuse the real recording realm ABI; only these sound effects are synthetic.
#include <string.h>
#define HF_EXTERNAL_TRANSPORT 1
#define coaxial_message packet_base_message
#include "../../../metaverse-engine/host/tests/fixture-face.c"
#undef coaxial_message
static uint8_t packet_bytes[4]={'o','k','\n','!'};
static uintptr_t packet_address;
static uint32_t packet_count;
void coaxial_message(HfAddress state,int32_t kind,uint32_t n) {
  if(kind>=930&&kind<=932&&n==4){
    memcpy(&packet_count,game(state)->inbox.e,4);
    packet_address=kind==931?0:kind==932?UINTPTR_MAX-1:(uintptr_t)packet_bytes;
  }else packet_base_message(state,kind,n);
}
HfAddress coaxial_sounds(HfAddress state){(void)state;return packet_address;}
uint32_t coaxial_sounds_len(HfAddress state){(void)state;uint32_t n=packet_count;packet_count=0;return n;}
uint32_t coaxial_sounds_capacity(HfAddress state){(void)state;return sizeof packet_bytes;}
