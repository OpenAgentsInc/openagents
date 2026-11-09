/* Scalar ABI only. The page's data, layout, drawing and input are Coax (status.coax). */
#include "status.h"
#include <stdint.h>
#include "analytics-face.h"
static unsigned char inbox[262144];
static coax_t_Track tracking;
static uint32_t golden_mode;
static coax_v_t_Track tr(void){return (coax_v_t_Track){&tracking,1};}
static void track_ready(void){if(!tracking.page_n)track_init(tr(),track_cstr("status"));}
#define EXPORT(name) __attribute__((export_name(name)))
EXPORT("coaxial_golden") void golden(uint32_t on){golden_mode=on!=0;}
static coax_t_St *at(uint32_t p){return (coax_t_St *)(uintptr_t)p;}
static coax_v_t_St view(uint32_t p){return (coax_v_t_St){at(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_St)*1000003u+1u;}
EXPORT("coaxial_state_size") uint32_t state_size(void){return sizeof(coax_t_St);}
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_St((coax_t_St *)(uintptr_t)state); }
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void){return (uint32_t)st_max_vertices();}
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void){return (uint32_t)coaxial_land_vertex_floats();}
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void){return (uint32_t)coaxial_land_uniform_floats();}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t vertices,uint32_t uniforms,float w,float h,float d,float dt,uint32_t still){int64_t n=st_frame(view(p),(coax_v_t_LandState){0,0},(coax_v_f32){(float *)(uintptr_t)vertices,st_max_vertices()*coaxial_land_vertex_floats()},(coax_v_f32){(float *)(uintptr_t)uniforms,coaxial_land_uniform_floats()},w,h,d,dt,still!=0||golden_mode!=0);track_ready();track_frame(tr(),dt,w,h,d);track_drain(&tracking);return (uint32_t)n;}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return st_pointer(view(p),x,y,phase,kind,id);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){st_wheel(view(p),dy);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){(void)mods;return st_key(view(p),k);}
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p){return st_hit(view(p),at(p)->px,at(p)->py)>=0;}
EXPORT("coaxial_action") int32_t action(uint32_t p){return (int32_t)at(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p){return (uint32_t)at(p)->out_n;}
EXPORT("coaxial_action_done") void action_done(uint32_t p){st_done(view(p));}
/* Read-only probes for browser checks. */
EXPORT("coaxial_status_commits") uint32_t commits(uint32_t p){return (uint32_t)at(p)->n;}
EXPORT("coaxial_status_live") uint32_t live(uint32_t p){return at(p)->live;}
EXPORT("coaxial_status_limit") float limit(uint32_t p){return at(p)->limit;}

EXPORT("coaxial_inbox") uint32_t incoming(uint32_t p){(void)p;return (uint32_t)(uintptr_t)inbox;}
EXPORT("coaxial_inbox_cap") uint32_t incoming_cap(void){return sizeof inbox;}
EXPORT("coaxial_message") void message(uint32_t p,int32_t kind,int32_t n){if(n<0||n>(int32_t)sizeof inbox)return;coax_string text={(const char*)inbox,n};st_message(view(p),kind,text);track_ready();_Bool used=track_message(tr(),kind,text);(void)used;track_drain(&tracking);}
