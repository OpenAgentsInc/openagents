/* Firm reason: 1. Scalar browser ABI only; Coax owns all world policy. */
#include "nights.h"
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#define EXPORT(n) __attribute__((export_name(n)))
static uint8_t catalog[262144],inbox[4096];
static float dummy_bake[4];
static coax_t_Nights *state(uint32_t p){return(coax_t_Nights*)(uintptr_t)p;}
static coax_v_t_Nights view(uint32_t p){return(coax_v_t_Nights){state(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_Nights)*1000003u+0x4e574e31u;}
EXPORT("coaxial_state_size") uint32_t size(void){return sizeof(coax_t_Nights);}
EXPORT("coaxial_state_init") void init(uint32_t p){coax_default_t_Nights(state(p));}
EXPORT("coaxial_max_vertices") uint32_t vertices(void){return 32768;}
EXPORT("coaxial_vertex_floats") uint32_t stride(void){return 16;}
EXPORT("coaxial_uniform_floats") uint32_t uniforms(void){return 104;}
EXPORT("coaxial_pixel_budget") uint32_t pixels(void){return 6400000;}
EXPORT("coaxial_character_storage_v1") uint32_t storage(void){return 268435456;}
EXPORT("coaxial_layers_side_v1") uint32_t layers(uint32_t p){(void)p;return 1024;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){night_step(view(p),dt,still!=0);uint32_t n=(uint32_t)night_frame(view(p),(coax_v_f32){(float*)(uintptr_t)v,32768*16},(coax_v_f32){(float*)(uintptr_t)u,104},w,h,d);night_scene(view(p));return n;}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t m){(void)m;return night_key(view(p),k,true);}
EXPORT("coaxial_key_up") uint32_t key_up(uint32_t p,int32_t k,int32_t m){(void)m;return night_key(view(p),k,false);}
EXPORT("coaxial_blur") void blur(uint32_t p){night_blur(view(p));}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return night_pointer(view(p),x,y,phase,kind,id);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){night_wheel(view(p),dy);}
EXPORT("coaxial_asset") uint32_t asset(uint32_t p,int32_t i,uint32_t n){(void)p;return i==0&&n<=sizeof catalog?(uint32_t)(uintptr_t)catalog:0;}
EXPORT("coaxial_asset_capacity") uint32_t capacity(uint32_t p,int32_t i){(void)p;return i==0?sizeof catalog:0;}
EXPORT("coaxial_asset_ready") void loaded(uint32_t p,int32_t i,uint32_t n){if(i<0||i>3)return;state(p)->loaded.e[i]=n;if(i==0&&n>0&&n<=sizeof catalog)night_catalog(view(p),(coax_string){(const char*)catalog,n});}
EXPORT("coaxial_scene_ready") uint32_t ready(uint32_t p){return state(p)->ready;}
EXPORT("coaxial_draws") uint32_t draws(uint32_t p){return(uint32_t)(uintptr_t)state(p)->draws.e;}
EXPORT("coaxial_draw_count") uint32_t draw_count(uint32_t p){return state(p)->draw_n;}
EXPORT("coaxial_shadow_draws_v1") uint32_t shadow_draws(uint32_t p){return(uint32_t)(uintptr_t)state(p)->draws.e;}
EXPORT("coaxial_shadow_draw_count_v1") uint32_t shadow_count(uint32_t p){return state(p)->draw_n;}
EXPORT("coaxial_stores") uint32_t stores(void){return 2;}
EXPORT("coaxial_store") uint32_t store(uint32_t p,int32_t i){return i==0?(uint32_t)(uintptr_t)state(p)->records.e:(uint32_t)(uintptr_t)dummy_bake;}
EXPORT("coaxial_store_cap") uint32_t store_cap(int32_t i){return i==0?sizeof(state(0)->records.e):sizeof dummy_bake;}
EXPORT("coaxial_store_bytes") uint32_t store_bytes(uint32_t p,int32_t i){(void)p;return store_cap(i);}
EXPORT("coaxial_store_version") uint32_t store_version(uint32_t p,int32_t i){return i==0?state(p)->revision:1;}
EXPORT("coaxial_inbox") uint32_t incoming(uint32_t p){(void)p;return(uint32_t)(uintptr_t)inbox;}
EXPORT("coaxial_inbox_cap") uint32_t incoming_cap(void){return sizeof inbox;}
EXPORT("coaxial_message") void message(uint32_t p,int32_t kind,uint32_t n){if(n<=sizeof inbox)night_message(view(p),kind,(coax_string){(const char*)inbox,n});}
EXPORT("coaxial_action") int32_t action(uint32_t p){return night_action(view(p));}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return(uint32_t)(uintptr_t)state(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t out_len(uint32_t p){return state(p)->out_n;}
EXPORT("coaxial_action_done") void done(uint32_t p){state(p)->out_n=0;}
EXPORT("coaxial_field_probe") double probe(uint32_t p,int32_t k){return k==0?state(p)->ready:k==1?state(p)->hero:k==2?state(p)->palette:k==3?state(p)->zone:k==4?state(p)->you.pos.x:k==5?state(p)->you.pos.z:k==6?state(p)->clip:k==7?state(p)->draw_n:k==8?state(p)->you.pos.y:k==9?state(p)->choosing:k==10?state(p)->you.dist:0;}
