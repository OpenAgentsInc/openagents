// /nip/01's C face: scalar Wasm exports over nip01.coax, and the one call
// into nip01-verify.coax (its own unit, so only its entry is declared here):
// each selection runs the real verifier and its observations go back.
#include "nip01.h"
#include <stddef.h>
extern void nv_run(int64_t mode, coax_v_s64 observation);
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_Nip *state_at(uint32_t p) { return (coax_t_Nip *)(uintptr_t)p; }
static coax_v_t_Nip at(uint32_t p) { return (coax_v_t_Nip){state_at(p), 1}; }
EXPORT("coaxial_layout") uint32_t layout(void) { return sizeof(coax_t_Nip) * 1000003u + 0x4e495031u; }
EXPORT("coaxial_state_size") uint32_t state_size(void) { return sizeof(coax_t_Nip); }
EXPORT("coaxial_max_vertices") uint32_t max_vertices(void) { return 50400; }
EXPORT("coaxial_vertex_floats") uint32_t vertex_floats(void) { return 16; }
EXPORT("coaxial_uniform_floats") uint32_t uniform_floats(void) { return 16; }
EXPORT("coaxial_frame") uint32_t frame(uint32_t p, uint32_t vertices, uint32_t uniforms, float w, float h, float d, float dt, uint32_t still) {
  int64_t request = nip_requested(at(p));
  if (request >= 0) {
    int64_t result[68] = {0};
    nv_run(request, (coax_v_s64){result, 68});
    nip_observed(at(p), (coax_v_s64){result, 68});
  }
  return nip_frame(at(p), (coax_v_f32){(float *)(uintptr_t)vertices, 50400 * 16}, (coax_v_f32){(float *)(uintptr_t)uniforms, 16}, w, h, d, dt, still != 0);
}
EXPORT("coaxial_key") uint32_t key(uint32_t p, int32_t k, int32_t mods) { (void)mods; return nip_key(at(p), k); }
EXPORT("coaxial_pointer") uint32_t pointer(uint32_t p, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)kind; (void)id; return nip_pointer(at(p), x, y, phase);
}
EXPORT("coaxial_cursor") int32_t cursor(uint32_t p) { (void)p; return 1; }
EXPORT("coaxial_nip_mode") int64_t mode(uint32_t p) { return state_at(p)->mode; }
EXPORT("coaxial_nip_error") int64_t error(uint32_t p) { return state_at(p)->error; }
EXPORT("coaxial_nip_verified") int32_t verified(uint32_t p) { return state_at(p)->verified; }
EXPORT("coaxial_nip_view") int64_t view(uint32_t p) { return state_at(p)->view; }
EXPORT("coaxial_action") int32_t action(uint32_t p) {return (int32_t)state_at(p)->action;}
EXPORT("coaxial_outbox") uint32_t outbox(uint32_t p) {(void)p;static const char route[]="/nip";return (uint32_t)(uintptr_t)route;}
EXPORT("coaxial_outbox_len") uint32_t outbox_len(uint32_t p) {(void)p;return 4;}
EXPORT("coaxial_action_done") void action_done(uint32_t p) {state_at(p)->action=0;}
