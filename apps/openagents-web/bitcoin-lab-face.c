/* Firm reason: 1 (scalar browser ABI; the realm's behavior and state are Coax). */
#include "bitcoin-lab.h"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_BitcoinLab *at(uint32_t p){return (coax_t_BitcoinLab *)(uintptr_t)p;}
static coax_v_t_BitcoinLab view(uint32_t p){return (coax_v_t_BitcoinLab){at(p),1};}
static uint8_t accessible[16384]; /* Derived transport bytes, rebuilt from Coax state. */
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_BitcoinLab)*1000003u+offsetof(coax_t_BitcoinLab,session)*7u+0x424c3031u;}
EXPORT("coaxial_state_size") uint32_t state_size(void){return sizeof(coax_t_BitcoinLab);}
EXPORT("coaxial_state_init") void state_init(uint32_t p){coax_default_t_BitcoinLab(at(p));}
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void){return 72000;}
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void){return 12;}
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void){return 88;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){return (uint32_t)lab_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,72000*12},(coax_v_f32){(float *)(uintptr_t)u,88},w,h,d,dt,still!=0);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){return lab_key(view(p),k,true,mods);}
EXPORT("coaxial_key_up") uint32_t key_up(uint32_t p,int32_t k,int32_t mods){return lab_key(view(p),k,false,mods);}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return lab_pointer(view(p),x,y,phase,kind,id);}
EXPORT("coaxial_blur") void blur(uint32_t p){lab_blur(view(p));}
EXPORT("coaxial_inbox") uint32_t inbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->inbox.e;}
EXPORT("coaxial_inbox_cap") uint32_t inbox_cap(uint32_t p){(void)p;return 256;}
EXPORT("coaxial_hash") void hash(uint32_t p,int32_t n){lab_hash(view(p),n);}
EXPORT("coaxial_message") void message(uint32_t p,int32_t k,int32_t n){lab_message(view(p),k,n);}
EXPORT("coaxial_wants_text") uint32_t wants_text(uint32_t p){return at(p)->explorer.e[0].search;}
EXPORT("coaxial_field") uint32_t field(uint32_t p){return (uint32_t)(uintptr_t)at(p)->field_box.e;}
EXPORT("coaxial_action") int32_t action(uint32_t p){return (int32_t)at(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p){return (uint32_t)at(p)->out_n;}
EXPORT("coaxial_action_done") void done(uint32_t p){at(p)->action=0;at(p)->out_n=0;}
/* Read-only observations of the real session for muted browser checks. */
EXPORT("coaxial_probe") uint32_t probe(uint32_t p){return (uint32_t)(at(p)->chapter+1);}
EXPORT("coaxial_guard_probe") int32_t guard_probe(uint32_t p,int32_t field){return (int32_t)lab_guard(view(p),field);}
EXPORT("coaxial_scene_ready") uint32_t scene_ready(uint32_t p){return lab_scene_ready(view(p));}
EXPORT("coaxial_draws") uint32_t draws(uint32_t p){return (uint32_t)(uintptr_t)at(p)->draws.e;}
EXPORT("coaxial_draw_count") uint32_t draw_count(uint32_t p){return at(p)->mesh_n>0?1u:0u;}
EXPORT("coaxial_stores") uint32_t stores(void){return 1;}
EXPORT("coaxial_store_cap") uint32_t store_cap(int32_t k){(void)k;return 32768*4;}
EXPORT("coaxial_store") uint32_t store(uint32_t p,int32_t k){(void)k;return (uint32_t)(uintptr_t)at(p)->mesh.e;}
EXPORT("coaxial_store_bytes") uint32_t store_bytes(uint32_t p,int32_t k){(void)k;return at(p)->mesh_n*8*4;}
EXPORT("coaxial_lab_station") int32_t station(uint32_t p){return (int32_t)at(p)->selected;}
EXPORT("coaxial_lab_pose") float pose(uint32_t p,int32_t field){return gridwalk_pose((coax_v_t_GridWalk){at(p)->session.e,1},field);}
EXPORT("coaxial_a11y") uint32_t a11y(uint32_t p){return (uint32_t)lab_a11y(view(p),(coax_v_u8){accessible,sizeof accessible});}
EXPORT("coaxial_a11y_buf") uint32_t a11y_buf(void){return (uint32_t)(uintptr_t)accessible;}
EXPORT("coaxial_a11y_do") void a11y_do(uint32_t p,int32_t id){lab_access(view(p),id,true);}
EXPORT("coaxial_a11y_focus") void a11y_focus(uint32_t p,int32_t id){lab_access(view(p),id,false);}
