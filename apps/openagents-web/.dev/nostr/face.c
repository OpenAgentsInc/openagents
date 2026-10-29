// Scalar transport for the local-only Coax display probe.
#include "probe.h"
#include <stddef.h>
// Both modules use Coax's generated pointer/count int-view ABI. Including
// their full headers together would duplicate unrelated anonymous arrays.
extern void nv_run(int64_t mode,coax_v_s64 observation);
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_NostrProbe *state_at(uint32_t p) { return (coax_t_NostrProbe *)(uintptr_t)p; }
static coax_v_t_NostrProbe at(uint32_t p) { return (coax_v_t_NostrProbe){state_at(p),1}; }
EXPORT("coaxial_layout") uint32_t layout(void) { return sizeof(coax_t_NostrProbe)*1000003u+0x4e505231u; }
EXPORT("coaxial_state_size") uint32_t state_size(void) { return sizeof(coax_t_NostrProbe); }
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void) { return 50400; }
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void) { return 16; }
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void) { return 16; }
EXPORT("coaxial_frame") uint32_t frame(uint32_t p,uint32_t vertices,uint32_t uniforms,float w,float h,float d,float dt,uint32_t still) {
  (void)dt;(void)still;
  int64_t request=np_requested(at(p));
  if(request>=0) {
    int64_t result[4]={0};
    nv_run(request,(coax_v_s64){result,4});
    np_observed(at(p),(coax_v_s64){result,4});
  }
  return np_frame(at(p),(coax_v_f32){(float *)(uintptr_t)vertices,50400*16},(coax_v_f32){(float *)(uintptr_t)uniforms,16},w,h,d);
}
EXPORT("coaxial_key") uint32_t key(uint32_t p,int32_t k,int32_t mods) { (void)mods;return np_key(at(p),k); }
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p,float x,float y,int32_t phase,int32_t kind,int32_t id) {
  (void)kind;(void)id;return np_pointer(at(p),x,y,phase);
}
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p) { (void)p;return 1; }
EXPORT("coaxial_probe_mode") int64_t mode(uint32_t p) { return state_at(p)->mode; }
EXPORT("coaxial_probe_error") int64_t error(uint32_t p) { return state_at(p)->error; }
EXPORT("coaxial_probe_verified") int32_t verified(uint32_t p) { return state_at(p)->verified; }
EXPORT("coaxial_probe_written") int64_t written(uint32_t p) { return state_at(p)->written; }
