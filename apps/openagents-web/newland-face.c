// The /new-land mock-up's C face for the browser (as face.c is the
// splash's): scalar Wasm exports over the Coax procedures in newland.coax.
// The state the host keeps is the page's clock, its tables and its input.
#include "newland.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_LandState *state_at(uint32_t state) { return (coax_t_LandState *)(uintptr_t)state; }
static coax_v_t_LandState view(uint32_t state) { return (coax_v_t_LandState){state_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_LandState) * 1000003u + _Alignof(coax_t_LandState) * 31u
                    + offsetof(coax_t_LandState, plot) * 7u + offsetof(coax_t_LandState, login) * 13u + 0x4c414eu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_LandState); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_land_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_land_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_land_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms,
                       float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_land_max_vertices() * coaxial_land_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_land_uniform_floats()};
  int64_t n = coaxial_land_frame(view(state), out, u, width, height, dpr, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

// Input: the pointer opens LOG IN, MAKE YOUR OWN and a place's page (action 1 and its URL).
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  return coaxial_land_pointer(view(state), x, y, phase, kind) ? 1u : 0u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_land_cursor(view(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_land_action(view(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)&state_at(state)->outbox; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_land_action_done(view(state)); }
