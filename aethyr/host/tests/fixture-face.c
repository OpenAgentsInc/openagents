// Platform ABI glue for the Coax host-conformance realm. This is linked in
// place of a game's face, through the same host and GPU/platform adapters.
// The recorder, defaults, outputs and interpretation are fixture.coax's.
#include "fixture.h"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#define HOST_TRANSPORT_HEADER "fixture.h"
#include "../../../coax/web/host-transport-face.c"

#if defined(__wasm__)
#define HF_EXPORT(name) __attribute__((export_name(name)))
#define HF_IMPORT(name) __attribute__((import_module("env"),import_name(name)))
#else
#define HF_EXPORT(name)
#define HF_IMPORT(name)
#endif
#if defined(COAXIAL_NATIVE_ADDRESS)
typedef uintptr_t HfAddress;
#else
typedef uint32_t HfAddress;
#endif
HF_IMPORT("coaxial_socket_open") int32_t coaxial_socket_open(HfAddress url,uint32_t n);
HF_IMPORT("coaxial_socket_send") void coaxial_socket_send(int32_t id,HfAddress text,uint32_t n);
HF_IMPORT("coaxial_socket_close") void coaxial_socket_close(int32_t id);
HF_IMPORT("coaxial_http_bounded") int32_t coaxial_http_bounded(HfAddress url,uint32_t n,HfAddress body,uint32_t body_n,int32_t flags,int64_t timeout);
HF_IMPORT("coaxial_http_cancel") int32_t coaxial_http_cancel(int32_t id);

static coax_t_HostFixture *game(HfAddress p) { return (coax_t_HostFixture *)(uintptr_t)p; }
static coax_v_t_HostFixture view(HfAddress p) { return (coax_v_t_HostFixture){game(p),1}; }
static void drain(HfAddress p) {
  coax_t_HostFixtureEffect e=hf_take_effect(view(p));HfAddress bytes=(HfAddress)(uintptr_t)e.data.data;
  switch(e.op){
    case 1:hf_result(view(p),e.op,coaxial_socket_open(bytes,(uint32_t)e.data.count));break;
    case 2:coaxial_socket_send((int32_t)e.target,bytes,(uint32_t)e.data.count);break;
    case 3:coaxial_socket_close((int32_t)e.target);break;
    case 4:hf_result(view(p),e.op,coaxial_http_bounded(bytes,(uint32_t)e.data.count,0,0,4,1000));break;
    case 5:hf_result(view(p),e.op,coaxial_http_cancel((int32_t)e.target));break;
  }
}
HF_EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return 0x48464331u; }
HF_EXPORT("coaxial_host_contract_version") uint32_t coaxial_host_contract_version(void) { return 1; }
HF_EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return sizeof(coax_t_HostFixture); }
HF_EXPORT("coaxial_state_init") void coaxial_state_init(HfAddress p) { coax_default_t_HostFixture(game(p)); }
HF_EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return 3; }
HF_EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return 12; }
HF_EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return 16; }
HF_EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { (void)on; }
HF_EXPORT("coaxial_frame") uint32_t coaxial_frame(HfAddress p,HfAddress v,HfAddress u,float w,float h,float dpr,float dt,uint32_t still) {
  return (uint32_t)hf_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,36},(coax_v_f32){(float *)(uintptr_t)u,16},w,h,dpr,dt,still!=0);
}
HF_EXPORT("coaxial_key") uint32_t coaxial_key(HfAddress p,int32_t k,int32_t mods) { return hf_key(view(p),k,mods,true); }
HF_EXPORT("coaxial_key_up") uint32_t coaxial_key_up(HfAddress p,int32_t k,int32_t mods) { return hf_key(view(p),k,mods,false); }
HF_EXPORT("coaxial_pointer") uint32_t coaxial_pointer(HfAddress p,float x,float y,int32_t phase,int32_t kind,int32_t id) { return hf_pointer(view(p),x,y,phase,kind,id); }
HF_EXPORT("coaxial_blur") void coaxial_blur(HfAddress p) { hf_blur(view(p)); }
HF_EXPORT("coaxial_wheel") void coaxial_wheel(HfAddress p,float dy) { hf_wheel(view(p),dy); }
HF_EXPORT("coaxial_inbox") HfAddress coaxial_inbox(HfAddress p) { return (HfAddress)(uintptr_t)game(p)->inbox.e; }
HF_EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return 128; }
HF_EXPORT("coaxial_message") void coaxial_message(HfAddress p,int32_t kind,uint32_t n) {
  hf_message(view(p),kind,(coax_string){(const char *)game(p)->inbox.e,n});
  drain(p);
}
HF_EXPORT("coaxial_asset") HfAddress coaxial_asset(HfAddress p,int32_t i,uint32_t bytes) { (void)i;(void)bytes;return (HfAddress)(uintptr_t)game(p)->asset.e; }
HF_EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(HfAddress p,int32_t i) { (void)p;(void)i;return 64; }
HF_EXPORT("coaxial_asset_ready") void coaxial_asset_ready(HfAddress p,int32_t i,uint32_t bytes) { hf_asset_ready(view(p),i,bytes); }
HF_EXPORT("coaxial_action") int32_t coaxial_action(HfAddress p) { return (int32_t)game(p)->action; }
HF_EXPORT("coaxial_outbox") HfAddress coaxial_outbox(HfAddress p) { return (HfAddress)(uintptr_t)game(p)->outbox.e; }
HF_EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(HfAddress p) { return (uint32_t)game(p)->out_n; }
HF_EXPORT("coaxial_action_done") void coaxial_action_done(HfAddress p) { hf_action_done(view(p)); }
HF_EXPORT("coaxial_host_probe") int64_t coaxial_host_probe(HfAddress p,int32_t event,int32_t field) { return hf_probe(view(p),event,field); }
HF_EXPORT("coaxial_host_reset") void coaxial_host_reset(HfAddress p) { hf_reset(view(p)); }
