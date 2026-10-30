/* Scalar ABI only. Document content, layout, rendering and input are Coax. */
#include "connect-pylon.h"
#include <stdint.h>
#include "analytics-face.h"
static unsigned char inbox[16384];
static coax_t_Track tracking;
static uint32_t golden_mode;
static coax_v_t_Track tr(void){return (coax_v_t_Track){&tracking,1};}
static void track_ready(void){if(!tracking.page_n)track_init(tr(),track_cstr("connect-pylon"));}
#define EXPORT(name) __attribute__((export_name(name)))
EXPORT("coaxial_golden") void golden(uint32_t on){golden_mode=on!=0;}
static coax_t_Cp *at(uint32_t p){return (coax_t_Cp *)(uintptr_t)p;}
static coax_v_t_Cp view(uint32_t p){return (coax_v_t_Cp){at(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_Cp)*1000003u+(uint32_t)cp_revision();}
EXPORT("coaxial_state_size") uint32_t state_size(void){return sizeof(coax_t_Cp);}
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void){return (uint32_t)cp_max_vertices();}
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void){return (uint32_t)coaxial_land_vertex_floats();}
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void){return (uint32_t)coaxial_land_uniform_floats();}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t vertices,uint32_t uniforms,float w,float h,float d,float dt,uint32_t still){int64_t n=cp_frame(view(p),(coax_v_t_LandState){0,0},(coax_v_f32){(float *)(uintptr_t)vertices,cp_max_vertices()*coaxial_land_vertex_floats()},(coax_v_f32){(float *)(uintptr_t)uniforms,coaxial_land_uniform_floats()},w,h,d,dt,still!=0||golden_mode!=0);track_ready();track_frame(tr(),dt,w,h,d);track_drain(&tracking);return (uint32_t)n;}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return cp_pointer(view(p),x,y,phase,kind,id);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){cp_wheel(view(p),dy);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){(void)mods;return cp_key(view(p),k);}
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p){return cp_hit(view(p),at(p)->px,at(p)->py)>=0;}
EXPORT("coaxial_action") int32_t action(uint32_t p){return (int32_t)at(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p){return (uint32_t)at(p)->out_n;}
EXPORT("coaxial_action_done") void action_done(uint32_t p){cp_done(view(p));}
/* Read-only probes for browser/ABI checks of the real product controls. */
EXPORT("coaxial_guide_scroll") float scroll(uint32_t p){return at(p)->scroll;}
EXPORT("coaxial_guide_limit") float limit(uint32_t p){return at(p)->limit;}
EXPORT("coaxial_guide_control") float control(uint32_t p,uint32_t which,uint32_t axis){coax_t_Cp*s=at(p);coax_t_Box b={0};if(which==0)b=s->markdown;else if(which==1)b=s->command;else if(which==2)b=s->home;else if(which==3)b=s->body;return axis==0?b.x0:axis==1?b.y0:axis==2?b.x1:b.y1;}

EXPORT("coaxial_inbox") uint32_t incoming(uint32_t p){(void)p;return (uint32_t)(uintptr_t)inbox;}
EXPORT("coaxial_inbox_cap") uint32_t incoming_cap(void){return sizeof inbox;}
EXPORT("coaxial_message") void message(uint32_t p,int32_t kind,int32_t n){(void)p;if(n<0||n>(int32_t)sizeof inbox)return;track_ready();_Bool used=track_message(tr(),kind,(coax_string){(const char*)inbox,n});(void)used;track_drain(&tracking);}
