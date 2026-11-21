/* Firm reason: 1 (scalar browser ABI; all world behavior and drawing are Coax). */
#include "valleylab.h"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_ValleyLab *at(uint32_t p){return (coax_t_ValleyLab *)(uintptr_t)p;}
__attribute__((import_module("env"),import_name("coaxial_http_bounded"))) extern int32_t valley_fetch(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,int64_t);
static coax_v_t_ValleyLab view(uint32_t p){return (coax_v_t_ValleyLab){at(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_ValleyLab)*1000003u+offsetof(coax_t_ValleyLab,you)*7u+0x564c3032u;}
EXPORT("coaxial_state_size") uint32_t state_size(void){return sizeof(coax_t_ValleyLab);}
EXPORT("coaxial_state_init") void state_init(uint32_t p){coax_default_t_ValleyLab(at(p));}
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void){return 81920;}
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void){return 12;}
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void){return 88;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){uint32_t n=(uint32_t)valley_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,81920*12},(coax_v_f32){(float *)(uintptr_t)u,88},w,h,d,dt,still!=0);if(at(p)->scan_need){int32_t id=valley_fetch((uint32_t)(uintptr_t)at(p)->page_url.e,(uint32_t)at(p)->page_url_n,0,0,4,10000);valley_scan_started(view(p),id);}else if(at(p)->need_page>=0){int32_t id=valley_fetch((uint32_t)(uintptr_t)at(p)->page_url.e,(uint32_t)at(p)->page_url_n,0,0,4,5000);valley_http_started(view(p),id);}return n;}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){(void)mods;return valley_key(view(p),k,true);}
EXPORT("coaxial_key_up") uint32_t key_up(uint32_t p,int32_t k,int32_t mods){(void)mods;return valley_key(view(p),k,false);}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return valley_pointer(view(p),x,y,phase,kind,id);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){valley_wheel(view(p),dy);}
EXPORT("coaxial_blur") void blur(uint32_t p){valley_blur(view(p));}
EXPORT("coaxial_scene_ready") uint32_t scene_ready(uint32_t p){return at(p)->ready;}
EXPORT("coaxial_draws") uint32_t draws(uint32_t p){return (uint32_t)(uintptr_t)at(p)->draws.e;}
EXPORT("coaxial_draw_count") uint32_t draw_count(uint32_t p){return at(p)->mesh_n>0?1u:0u;}
EXPORT("coaxial_stores") uint32_t stores(void){return 2;}
EXPORT("coaxial_store_cap") uint32_t store_cap(int32_t k){return k==0?16384*8*4:65536*4;}
EXPORT("coaxial_store") uint32_t store(uint32_t p,int32_t k){return (uint32_t)(uintptr_t)(k==0?(void*)at(p)->mesh.e:(void*)at(p)->index.e);}
EXPORT("coaxial_store_bytes") uint32_t store_bytes(uint32_t p,int32_t k){return k==0?at(p)->mesh_n*8*4:at(p)->index_n*4;}
EXPORT("coaxial_guard_probe") int32_t probe(uint32_t p,int32_t f){return (int32_t)valley_probe(view(p),f);}

EXPORT("coaxial_inbox") uint32_t inbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->inbox.e;}
EXPORT("coaxial_inbox_cap") uint32_t inbox_cap(void){return sizeof(at(0)->inbox.e);}
EXPORT("coaxial_hash") void hash(uint32_t p,int32_t n){valley_hash(view(p),n);}
EXPORT("coaxial_action") int32_t action(uint32_t p){return (int32_t)at(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p){return (uint32_t)at(p)->out_n;}
EXPORT("coaxial_action_done") void done(uint32_t p){at(p)->action=0;at(p)->out_n=0;}

EXPORT("coaxial_message") void message(uint32_t p,int32_t kind,int32_t n){valley_message(view(p),kind,n);}
