// Firm reason: 5 (thin face ABI projection: actual pointers and scalars from the Coax recording realm).
#define coaxial_state_init audio_base_state_init
#define coaxial_message audio_base_message
#include "fixture-face.c"
#undef coaxial_state_init
#undef coaxial_message
#include <stdio.h>
static coax_t_HostAudioFixture audio_fixture;
static uint8_t wave[60],listing[256];
static coax_v_t_HostAudioFixture audio_view(void){return(coax_v_t_HostAudioFixture){&audio_fixture,1};}
HF_EXPORT("coaxial_state_init") void coaxial_state_init(HfAddress p){audio_base_state_init(p);audio_fixture=hfa_open();(void)hfa_wave((coax_v_u8){wave,sizeof wave});}
HF_EXPORT("coaxial_message") void coaxial_message(HfAddress p,int32_t kind,uint32_t n){audio_base_message(p,kind,n);if(kind==930)(void)hfa_offer(audio_view(),(coax_string){(const char*)game(p)->inbox.e,n});}
static coax_t_HostAudioFixturePacket packet_plan(void){HfAddress maximum=(HfAddress)~(HfAddress)0;return hfa_packet_extent(audio_view(),(uint64_t)(uintptr_t)audio_fixture.line.e,(uint64_t)maximum);}
HF_EXPORT("coaxial_sounds") HfAddress coaxial_sounds(HfAddress p){(void)p;return(HfAddress)packet_plan().address;}
HF_EXPORT("coaxial_sounds_capacity") uint32_t coaxial_sounds_capacity(HfAddress p){(void)p;return(uint32_t)packet_plan().capacity;}
HF_EXPORT("coaxial_sounds_len") uint32_t coaxial_sounds_len(HfAddress p){(void)p;int64_t n=hfa_take(audio_view());return(uint32_t)(n?n+packet_plan().extra:0);}
HF_EXPORT("coaxial_sound_bank_len") uint32_t coaxial_sound_bank_len(HfAddress p){(void)p;coax_t_HostAudioFixtureBank plan=hfa_bank(audio_view());int n=snprintf((char*)listing,sizeof listing,"pulse %llu %lld\npulse2 %llu %zu",(unsigned long long)(uintptr_t)(wave+plan.offset),(long long)plan.bytes,(unsigned long long)(uintptr_t)wave,sizeof wave);return(uint32_t)(n+plan.extra);}
HF_EXPORT("coaxial_sound_bank") HfAddress coaxial_sound_bank(HfAddress p){(void)p;return(HfAddress)(uintptr_t)listing;}
HF_EXPORT("coaxial_sound_bank_capacity") uint32_t coaxial_sound_bank_capacity(HfAddress p){(void)p;return sizeof listing;}
HF_EXPORT("coaxial_sound_bank_storage") HfAddress coaxial_sound_bank_storage(HfAddress p){(void)p;return(HfAddress)(uintptr_t)wave;}
HF_EXPORT("coaxial_sound_bank_storage_bytes") uint32_t coaxial_sound_bank_storage_bytes(HfAddress p){(void)p;return sizeof wave;}
void coaxial_audio_fixture_mode(int mode){audio_fixture.mode=mode;}
int64_t coaxial_audio_fixture_probe(int field){switch(field){case 0:return audio_fixture.reads;case 1:return audio_fixture.drains;case 2:return audio_fixture.count;}return -1;}
