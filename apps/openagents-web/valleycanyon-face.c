/* Firm reason: 1 (scalar browser ABI; source, camera, input, admission and drawing are Coax). */
#include "valleycanyon.h"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_ValleyCanyon *state(uint32_t p){return (coax_t_ValleyCanyon *)(uintptr_t)p;}
static coax_v_t_ValleyCanyon view(uint32_t p){return (coax_v_t_ValleyCanyon){state(p),1};}
static coax_t_EchoCatalogScratch catalogScratch;
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_ValleyCanyon)*1000003u+0x56435031u;}
EXPORT("coaxial_state_size") uint32_t size(void){return sizeof(coax_t_ValleyCanyon);}
EXPORT("coaxial_state_init") void init(uint32_t p){coax_default_t_ValleyCanyon(state(p));}
EXPORT("coaxial_max_vertices") uint32_t vertices(void){return 32768;}
EXPORT("coaxial_vertex_floats") uint32_t stride(void){return 12;}
EXPORT("coaxial_uniform_floats") uint32_t uniforms(void){return 128;}
EXPORT("coaxial_sampling_v1") uint32_t sampling(uint32_t p){return (uint32_t)canyon_sampling(view(p));}
EXPORT("coaxial_pixel_budget") uint32_t pixel_budget(void){return (uint32_t)canyon_pixel_budget();}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){canyon_step(view(p),dt,still!=0);return (uint32_t)canyon_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,32768*12},(coax_v_f32){(float *)(uintptr_t)u,128},w,h,d);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){(void)mods;return canyon_hold(view(p),k,true);}
EXPORT("coaxial_key_up") uint32_t key_up(uint32_t p,int32_t k,int32_t mods){(void)mods;return canyon_hold(view(p),k,false);}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return canyon_pointer_event(view(p),x,y,phase,kind,id);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){canyon_wheel(view(p),dy);}
EXPORT("coaxial_blur") void blur(uint32_t p){canyon_blur(view(p));}
EXPORT("coaxial_scene_ready") uint32_t ready(uint32_t p){return state(p)->ready;}
EXPORT("coaxial_asset") uint32_t asset(uint32_t p,int32_t i,uint32_t n){(void)p;return i==0&&n<=sizeof(catalogScratch.bytes.e)?(uint32_t)(uintptr_t)catalogScratch.bytes.e:0;}
EXPORT("coaxial_asset_capacity") uint32_t capacity(uint32_t p,int32_t i){(void)p;return i==0?sizeof(catalogScratch.bytes.e):0;}
EXPORT("coaxial_asset_ready") void loaded(uint32_t p,int32_t i,uint32_t n){if(i==0&&n<=sizeof(catalogScratch.bytes.e))canyon_catalog(view(p),(coax_v_u8){catalogScratch.bytes.e,n});else canyon_asset(view(p),i,n);}
EXPORT("coaxial_draws") uint32_t draws(uint32_t p){return (uint32_t)(uintptr_t)state(p)->draws.e;}
EXPORT("coaxial_draw_count") uint32_t draw_count(uint32_t p){return canyon_ready(view(p))?4u:0u;}
EXPORT("coaxial_stores") uint32_t stores(void){return 1;}
EXPORT("coaxial_store") uint32_t store(uint32_t p,uint32_t i){return i==0?(uint32_t)(uintptr_t)state(p)->echo.e[0].palette.e:0;}
EXPORT("coaxial_store_bytes") uint32_t store_bytes(uint32_t p,uint32_t i){(void)p;return i==0?sizeof(state(0)->echo.e[0].palette.e):0;}
EXPORT("coaxial_store_cap") uint32_t store_cap(uint32_t i){return i==0?sizeof(state(0)->echo.e[0].palette.e):0;}
EXPORT("coaxial_store_version") uint32_t store_version(uint32_t p,uint32_t i){return i==0?(uint32_t)state(p)->echo.e[0].revision:0;}
EXPORT("coaxial_guard_probe") int32_t probe(uint32_t p,int32_t f){return (int32_t)canyon_probe(view(p),f);}
EXPORT("coaxial_action") int32_t action(uint32_t p){return state(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)state(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t out_len(uint32_t p){return state(p)->out_n;}
EXPORT("coaxial_action_done") void done(uint32_t p){state(p)->action=0;state(p)->out_n=0;}

/* Optional pack gate ABI v1: platform computes SHA256; Coax checks the complete private source. */
EXPORT("coaxial_pack_gate_version") uint32_t pack_version(void){return 1;}
EXPORT("coaxial_pack_digest") uint32_t pack_digest(uint32_t p){return (uint32_t)(uintptr_t)state(p)->pack_digest.e;}
EXPORT("coaxial_pack_digest_capacity") uint32_t pack_capacity(void){return 32;}
EXPORT("coaxial_pack_admit") uint32_t pack_admit(uint32_t p,uint32_t bytes){return canyon_pack_gate(view(p),bytes);}
