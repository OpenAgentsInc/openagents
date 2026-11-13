// Firm reason: 6 (test-only native physical pointer and storage observations).
#include <string.h>
#define HF_EXTERNAL_TRANSPORT 1
#define coaxial_message media_base_message
#include "../../../metaverse-engine/host/tests/fixture-face.c"
#undef coaxial_message
static uint8_t media_packet_bytes[4]={65,66,67,63};
static uintptr_t media_packet_address;
static uint32_t media_packet_count,media_packet_capacity;
void coaxial_message(HfAddress state,int32_t kind,uint32_t n){
 if(kind>=930&&kind<=934&&n==4){
  memcpy(&media_packet_count,game(state)->inbox.e,4);media_packet_capacity=kind==934?1024:sizeof media_packet_bytes;
  media_packet_address=kind==931?0:kind==933?UINTPTR_MAX-1:kind==934?UINTPTR_MAX-15:(uintptr_t)media_packet_bytes;
 }else media_base_message(state,kind,n);
}
HfAddress coaxial_media(HfAddress state){(void)state;return media_packet_address;}
uint32_t coaxial_media_len(HfAddress state){(void)state;return media_packet_count;}
#ifndef COAXIAL_MEDIA_NO_CAPACITY
uint32_t coaxial_media_capacity(HfAddress state){(void)state;return media_packet_capacity;}
#endif
