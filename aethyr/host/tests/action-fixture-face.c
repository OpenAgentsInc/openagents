// Firm reason: 5 (thin face projection of Coax recording plans and actual scalar-read observations).
#define HF_FIXTURE_HEADER "action-fixture.h"
#define coaxial_state_init action_base_state_init
#define coaxial_outbox action_base_outbox
#define coaxial_outbox_len action_base_outbox_len
#define HF_INIT_EXPORT(name)
#define HF_OUTBOX_EXPORT(name)
#define HF_OMIT_OUTBOX_CAPACITY
#include "fixture-face.c"
#undef coaxial_state_init
#undef coaxial_outbox
#undef coaxial_outbox_len
static coax_t_HostActionFixture action_fixture;
static coax_v_t_HostActionFixture action_view(void){return(coax_v_t_HostActionFixture){&action_fixture,1};}
static coax_t_HostActionFixtureSpan action_plan(HfAddress p){
#if defined(__wasm__)
 uint64_t limit=(uint64_t)__builtin_wasm_memory_size(0)*65536;
#else
 uint64_t limit=UINTPTR_MAX;
#endif
 return hpa_plan(action_view(),(uint64_t)(uintptr_t)game(p)->outbox.e,sizeof game(p)->outbox.e,(uint64_t)game(p)->out_n,limit);
}
HF_EXPORT("coaxial_state_init") void coaxial_state_init(HfAddress p){action_base_state_init(p);action_fixture=hpa_open();}
HF_EXPORT("coaxial_outbox") HfAddress coaxial_outbox(HfAddress p){hpa_read(action_view(),0);return(HfAddress)action_plan(p).address;}
HF_EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(HfAddress p){hpa_read(action_view(),2);return(uint32_t)action_plan(p).bytes;}
#ifndef HAF_NO_CAPACITY
HF_EXPORT("coaxial_outbox_capacity") uint32_t coaxial_outbox_capacity(HfAddress p){hpa_read(action_view(),1);return(uint32_t)action_plan(p).capacity;}
#endif
HF_EXPORT("coaxial_action_fixture_mode") void coaxial_action_fixture_mode(int32_t mode){hpa_mode(action_view(),mode);}
HF_EXPORT("coaxial_action_fixture_probe") int64_t coaxial_action_fixture_probe(int32_t field){return hpa_probe(action_view(),field);}
