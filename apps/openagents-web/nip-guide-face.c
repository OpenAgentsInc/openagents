/* Scalar browser ABI only: UI and observations remain Coax. */
#include "nip-guide.h"
#include <stdint.h>
extern int64_t ng_demo(int64_t nip, int64_t mode);
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_Ng *at(uint32_t p) {return (coax_t_Ng *)(uintptr_t)p;}
static coax_v_t_Ng view(uint32_t p) {return (coax_v_t_Ng){at(p),1};}
EXPORT("coaxial_layout") uint32_t layout(void) {return sizeof(coax_t_Ng)*1000003u+0x4e495047u;}
EXPORT("coaxial_state_size") uint32_t state_size(void) {return sizeof(coax_t_Ng);}
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void) {return 80000;}
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void) {return 16;}
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void) {return 16;}
EXPORT("coaxial_page") void page(uint32_t p,int32_t n) {ng_page(view(p),n);}
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t vertices,uint32_t uniforms,float w,float h,float d,float dt,uint32_t still) {
  if(ng_requested(view(p))>=0) ng_observed(view(p),ng_demo(ng_number(at(p)->page),at(p)->mode));
  return (uint32_t)ng_frame(view(p),(coax_v_f32){(float *)(uintptr_t)vertices,80000*16},(coax_v_f32){(float *)(uintptr_t)uniforms,16},w,h,d,dt,still!=0);
}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods) {(void)mods;return ng_key(view(p),k);}
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id) {(void)kind;(void)id;return ng_pointer(view(p),x,y,phase);}
EXPORT("coaxial_wheel") void wheel(uint32_t p,float dy) {ng_wheel(view(p),dy);}
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p) {(void)p;return 1;}
EXPORT("coaxial_action") int32_t action(uint32_t p) {return (int32_t)at(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p) {return (uint32_t)(uintptr_t)at(p)->route.e;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p) {return (uint32_t)at(p)->route_n;}
EXPORT("coaxial_action_done") void action_done(uint32_t p) {at(p)->action=0;}
EXPORT("coaxial_nip_code") int64_t code(uint32_t p) {return at(p)->code;}
EXPORT("coaxial_nip_mode") int64_t mode(uint32_t p) {return at(p)->mode;}
EXPORT("coaxial_nip_view") int64_t guide_view(uint32_t p) {return at(p)->view;}
EXPORT("coaxial_nip_page") int64_t guide_page(uint32_t p) {return at(p)->page;}
