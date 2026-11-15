/* Firm reason: 1 (scalar browser ABI only; camera, input and drawing are Coax). */
#include "microduck.h"
#include <stdint.h>
#include <stdbool.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_Microduck *state(uint32_t p) { return (coax_t_Microduck *)(uintptr_t)p; }
static coax_v_t_Microduck view(uint32_t p) { return (coax_v_t_Microduck){state(p),1}; }
EXPORT("coaxial_layout") uint32_t layout(void) { return sizeof(coax_t_Microduck)*1000003u+0x4d443031u; }
EXPORT("coaxial_state_size") uint32_t size(void) { return sizeof(coax_t_Microduck); }
EXPORT("coaxial_state_init") void init(uint32_t p) { coax_default_t_Microduck(state(p)); }
EXPORT("coaxial_max_vertices") uint32_t vertices(void) { return 32768; }
EXPORT("coaxial_vertex_floats") uint32_t stride(void) { return 12; }
EXPORT("coaxial_uniform_floats") uint32_t uniforms(void) { return 64; }
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t v,uint32_t u,float w,float h,float d,float dt,uint32_t still) {
  (void)dt; (void)still;
  return md_frame(view(p),(coax_v_f32){(float *)(uintptr_t)v,32768*12},(coax_v_f32){(float *)(uintptr_t)u,64},w,h,d);
}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods) { (void)mods; return md_key(view(p),k); }
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id) { return md_pointer(view(p),x,y,phase,kind,id); }
EXPORT("coaxial_wheel") void wheel(uint32_t p,float delta) { md_wheel(view(p),delta); }
EXPORT("coaxial_blur") void blur(uint32_t p) { state(p)->drag=false; }
EXPORT("coaxial_scene_ready") uint32_t ready(uint32_t p) { return state(p)->ready; }
EXPORT("coaxial_asset") uint32_t asset(uint32_t p,int32_t i,uint32_t n) { (void)p; (void)i; (void)n; return 0; }
EXPORT("coaxial_asset_ready") void asset_ready(uint32_t p,int32_t i,uint32_t n) { if(i==0) md_asset(view(p),n); }
EXPORT("coaxial_draws") uint32_t draws(uint32_t p) { return (uint32_t)(uintptr_t)state(p)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t draw_count(uint32_t p) { return state(p)->ready?1u:0u; }
EXPORT("coaxial_pack_gate_version") uint32_t pack_version(void) { return 1; }
EXPORT("coaxial_pack_digest") uint32_t pack_digest(uint32_t p) { return (uint32_t)(uintptr_t)state(p)->digest.e; }
EXPORT("coaxial_pack_digest_capacity") uint32_t pack_capacity(void) { return 32; }
EXPORT("coaxial_pack_admit") uint32_t pack_admit(uint32_t p,uint32_t bytes) { return md_gate(view(p),bytes); }
EXPORT("coaxial_guard_probe") int32_t probe(uint32_t p,int32_t field) { return field==0?state(p)->ready:field==1?state(p)->mode:field==2?state(p)->draws.e[1]:0; }
