/* Firm reason: 1 (scalar browser ABI only; all source, gameplay, admission and drawing logic is Coax). */
#include "ocarinalab.h"
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#define EXPORT(n) __attribute__((export_name(n)))
static uint8_t incoming[100663296];
static coax_t_OcarinaLab *state(uint32_t p){return(coax_t_OcarinaLab*)(uintptr_t)p;}
static coax_v_t_OcarinaLab view(uint32_t p){return(coax_v_t_OcarinaLab){state(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_OcarinaLab)*1000003u+0x4f434c31u;}
EXPORT("coaxial_state_size") uint32_t size(void){return sizeof(coax_t_OcarinaLab);}
EXPORT("coaxial_state_init") void init(uint32_t p){coax_default_t_OcarinaLab(state(p));}
EXPORT("coaxial_max_vertices") uint32_t vertices(void){return 16384;}
EXPORT("coaxial_vertex_floats") uint32_t stride(void){return 12;}
EXPORT("coaxial_uniform_floats") uint32_t uniforms(void){return 64;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){ocl_step(view(p),dt,still!=0);return(uint32_t)ocl_frame(view(p),(coax_v_f32){(float*)(uintptr_t)v,16384*12},(coax_v_f32){(float*)(uintptr_t)u,64},w,h,d);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){(void)mods;return ocl_hold(view(p),k,true);}
EXPORT("coaxial_key_up") uint32_t key_up(uint32_t p,int32_t k,int32_t mods){(void)mods;return ocl_hold(view(p),k,false);}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return ocl_pointer(view(p),x,y,phase,kind,id);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){ocl_wheel(view(p),dy);}
EXPORT("coaxial_blur") void blur(uint32_t p){ocl_blur(view(p));}
EXPORT("coaxial_scene_ready") uint32_t ready(uint32_t p){return ocl_ready(view(p));}
EXPORT("coaxial_asset") uint32_t asset(uint32_t p,int32_t i,uint32_t n){(void)p;return i==0&&n>0&&n<=(uint32_t)ocl_catalog_capacity(view(p))?(uint32_t)(uintptr_t)incoming:0;}
EXPORT("coaxial_asset_capacity") uint32_t capacity(uint32_t p,int32_t i){(void)p;return i==0?(uint32_t)ocl_catalog_capacity(view(p)):0;}
EXPORT("coaxial_asset_ready") void loaded(uint32_t p,int32_t i,uint32_t n){if(i==0&&n<=sizeof(incoming)){if(ocl_catalog_admit(view(p),(coax_v_u8){incoming,n}))ocl_asset(view(p),i,n);}else ocl_asset(view(p),i,n);}
EXPORT("coaxial_draws") uint32_t draws(uint32_t p){return(uint32_t)(uintptr_t)state(p)->draws.e;}
EXPORT("coaxial_draw_count") uint32_t draw_count(uint32_t p){return ocl_ready(view(p))?1:0;}
EXPORT("coaxial_stores") uint32_t stores(void){return 1;}
EXPORT("coaxial_store") uint32_t store(uint32_t p,uint32_t i){return i==0?(uint32_t)(uintptr_t)state(p)->palette.e:0;}
EXPORT("coaxial_store_bytes") uint32_t store_bytes(uint32_t p,uint32_t i){(void)p;return i==0?sizeof(state(0)->palette.e):0;}
EXPORT("coaxial_store_cap") uint32_t store_cap(uint32_t i){return i==0?sizeof(state(0)->palette.e):0;}
EXPORT("coaxial_store_version") uint32_t store_version(uint32_t p,uint32_t i){return i==0?state(p)->revision:0;}
EXPORT("coaxial_guard_probe") double probe(uint32_t p,int32_t k){return ocl_probe(view(p),k);}
EXPORT("coaxial_action") int32_t action(uint32_t p){return state(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return(uint32_t)(uintptr_t)state(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t out_len(uint32_t p){return state(p)->out_n;}
EXPORT("coaxial_action_done") void done(uint32_t p){state(p)->action=0;state(p)->out_n=0;}
EXPORT("coaxial_pack_gate_version") uint32_t pack_version(void){return 1;}
EXPORT("coaxial_pack_digest") uint32_t pack_digest(uint32_t p){return(uint32_t)(uintptr_t)state(p)->pack_digest.e;}
EXPORT("coaxial_pack_digest_capacity") uint32_t pack_capacity(void){return 32;}
EXPORT("coaxial_pack_admit") uint32_t pack_admit(uint32_t p,uint32_t bytes){return ocl_pack_gate(view(p),bytes);}
EXPORT("coaxial_storage_limit_v1") uint32_t storage_limit(void){return 268435456u;}
