/* Firm reason: 1 (scalar browser ABI; playback, controls and drawing are Coax). */
#include "animationlab.h"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#define EXPORT(name) __attribute__((export_name(name)))
static uint8_t bundle[2][67108864],inbox[262144];
static uint32_t active_bundle; /* Publish the incoming bank only after Coax admits it. */
static uint32_t geometry[16777216];
static coax_t_AlState *state(uint32_t p){return (coax_t_AlState *)(uintptr_t)p;}
static coax_v_t_AlState view(uint32_t p){return (coax_v_t_AlState){state(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_AlState)*1000003u+0x414c4231u;}
EXPORT("coaxial_state_size") uint32_t size(void){return sizeof(coax_t_AlState);}
EXPORT("coaxial_state_init") void init(uint32_t p){coax_default_t_AlState(state(p));}
EXPORT("coaxial_max_vertices") uint32_t vertices(void){return 65536;}
EXPORT("coaxial_vertex_floats") uint32_t stride(void){return 12;}
EXPORT("coaxial_uniform_floats") uint32_t uniforms(void){return 48;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){return (uint32_t)al_draw(view(p),(coax_v_f32){(float *)(uintptr_t)v,65536*12},(coax_v_f32){(float *)(uintptr_t)u,48},(coax_v_u32){geometry,16777216},w,h,d,dt,still!=0);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){(void)mods;return al_key(view(p),k);}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){(void)kind;(void)id;return al_pointer(view(p),x,y,phase);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float delta){al_wheel(view(p),delta);}
EXPORT("coaxial_blur") void blur(uint32_t p){state(p)->drag=false;state(p)->seek=false;state(p)->slider=false;}
EXPORT("coaxial_scene_ready") uint32_t ready(uint32_t p){return state(p)->ready;}
EXPORT("coaxial_asset") uint32_t asset(uint32_t p,int32_t i,uint32_t n){(void)p;return i==0&&n<=sizeof bundle[0]?(uint32_t)(uintptr_t)bundle[active_bundle^1u]:0;}
EXPORT("coaxial_asset_capacity") uint32_t capacity(uint32_t p,int32_t i){(void)p;return i==0?sizeof bundle[0]:0;}
EXPORT("coaxial_asset_ready") void loaded(uint32_t p,int32_t i,uint32_t n){if(i==0&&n<=sizeof bundle[0]&&al_loaded(view(p),(coax_v_u8){bundle[active_bundle^1u],n}))active_bundle^=1u;}
EXPORT("coaxial_draws") uint32_t draws(uint32_t p){return (uint32_t)(uintptr_t)state(p)->draws.e;}
EXPORT("coaxial_draw_count") uint32_t count(uint32_t p){return state(p)->mesh_words>0?(uint32_t)state(p)->draw_count:0;}
EXPORT("coaxial_stores") uint32_t stores(void){return 2;}
EXPORT("coaxial_store") uint32_t store(uint32_t p,uint32_t i){return i==0?(uint32_t)(uintptr_t)geometry:(uint32_t)(uintptr_t)state(p)->gpu.e;}
EXPORT("coaxial_store_bytes") uint32_t bytes(uint32_t p,uint32_t i){return i==0?(state(p)->mesh_words>0?(uint32_t)state(p)->mesh_words*4:16):sizeof state(p)->gpu.e;}
EXPORT("coaxial_store_cap") uint32_t cap(uint32_t i){return i==0?sizeof geometry:12800*4;}
EXPORT("coaxial_store_version") uint32_t version(uint32_t p,uint32_t i){return i==0?(uint32_t)state(p)->mesh_version:(uint32_t)state(p)->revision;}
EXPORT("coaxial_inbox") uint32_t input(uint32_t p){(void)p;return (uint32_t)(uintptr_t)inbox;}
EXPORT("coaxial_inbox_cap") uint32_t input_cap(void){return sizeof inbox;}
EXPORT("coaxial_hash") void hash(uint32_t p,uint32_t n){if(n<=sizeof inbox)al_hash(view(p),(coax_string){(const char *)inbox,n});}
EXPORT("coaxial_action") uint32_t action(uint32_t p){return (uint32_t)state(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)state(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t out_len(uint32_t p){return (uint32_t)state(p)->out_n;}
EXPORT("coaxial_action_done") void done(uint32_t p){state(p)->action=0;state(p)->out_n=0;}
EXPORT("coaxial_guard_probe") int32_t probe(uint32_t p,int32_t f){return (int32_t)al_probe(view(p),f);}

EXPORT("coaxial_message") void message(uint32_t p,int32_t kind,uint32_t n){if(n<=sizeof inbox)al_message(view(p),kind,(coax_string){(const char *)inbox,n});}
