/* Firm reason: 1 (scalar browser ABI and analytics transport; UI is Coax). */
#include "changelog.h"
#include <stdint.h>
#include "analytics-face.h"
#define EXPORT(name) __attribute__((export_name(name)))
static unsigned char inbox[16384];
static coax_t_Track tracking;
static coax_v_t_Track tr(void){return (coax_v_t_Track){&tracking,1};}
static void ready(void){if(!tracking.page_n)track_init(tr(),track_cstr("coax-changelog"));}
static coax_t_ChangelogState *at(uint32_t p){return (coax_t_ChangelogState *)(uintptr_t)p;}
static coax_v_t_ChangelogState view(uint32_t p){return (coax_v_t_ChangelogState){at(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void){return sizeof(coax_t_ChangelogState)*1000003u+(uint32_t)cc_revision();}
EXPORT("coaxial_state_size") uint32_t state_size(void){return sizeof(coax_t_ChangelogState);}
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void){return 100000;}
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void){return 12;}
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void){return 88;}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still){
  int64_t n=cc_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,100000*12},(coax_v_f32){(float *)(uintptr_t)u,88},w,h,d,dt,still!=0);
  ready();track_frame(tr(),dt,w,h,d);track_drain(&tracking);return (uint32_t)n;
}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id){return cc_pointer(view(p),x,y,phase,kind,id);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy){cc_wheel(view(p),dy);}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods){return cc_key(view(p),k,mods);}
EXPORT("coaxial_blur") void blur(uint32_t p){cc_blur(view(p));}
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p){return cc_hit(view(p),at(p)->px,at(p)->py)>=0;}
EXPORT("coaxial_action") int32_t action(uint32_t p){return (int32_t)at(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p){return (uint32_t)(uintptr_t)at(p)->outbox.e;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p){return (uint32_t)at(p)->out_n;}
EXPORT("coaxial_action_done") void done(uint32_t p){cc_done(view(p));}
/* Read-only probes for the real product's input/layout checks. CSS pixels. */
EXPORT("coaxial_changelog_scroll") float scroll(uint32_t p){return at(p)->scroll;}
EXPORT("coaxial_changelog_limit") float limit(uint32_t p){return at(p)->limit;}
EXPORT("coaxial_changelog_first_visible") int32_t first(uint32_t p){return (int32_t)at(p)->first;}
EXPORT("coaxial_changelog_last_visible") int32_t last(uint32_t p){return (int32_t)at(p)->last;}
EXPORT("coaxial_changelog_control") float control(uint32_t p,int32_t id,int32_t axis){coax_t_CcBox b=cc_control(view(p),id);return axis==0?b.x0:axis==1?b.y0:axis==2?b.x1:b.y1;}
EXPORT("coaxial_probe") uint32_t probe(uint32_t p){return (uint32_t)(at(p)->last+1);}
EXPORT("coaxial_guard_probe") int32_t guard_probe(uint32_t p,int32_t field){
  if(field==0)return (int32_t)(sizeof(at(p)->tops.e)/sizeof(at(p)->tops.e[0]));
  if(field==1)return (int32_t)at(p)->scroll;if(field==2)return (int32_t)at(p)->limit;
  if(field>=10&&field<18){coax_t_CcBox b=cc_control(view(p),(field-10)/2);return (int32_t)(field%2?(b.y0+b.y1)*.5f:(b.x0+b.x1)*.5f);}
  return 0;
}
EXPORT("coaxial_inbox") uint32_t incoming(uint32_t p){(void)p;return (uint32_t)(uintptr_t)inbox;}
EXPORT("coaxial_inbox_cap") uint32_t capacity(void){return sizeof inbox;}
EXPORT("coaxial_message") void message(uint32_t p,int32_t kind,int32_t n){(void)p;if(n<0||n>(int32_t)sizeof inbox)return;ready();(void)track_message(tr(),kind,(coax_string){(const char*)inbox,n});track_drain(&tracking);}
