// A project page's C face for the browser (as face.c is the splash's):
// scalar Wasm exports over the Coax procedures in project.coax. The state
// the host keeps is the page's clock, its tree and its work (read once).
#include "project.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

typedef struct { coax_t_ProjectState page; coax_t_Tree tree; coax_t_Work work; } State;
static State *state_at(uint32_t state) { return (State *)(uintptr_t)state; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(State) * 1000003u + _Alignof(State) * 31u + offsetof(State, tree) * 7u + offsetof(State, work) * 13u + 0x50524fu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(State); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_project_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_project_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_project_uniform_floats(); }
EXPORT("coaxial_page") void coaxial_page(uint32_t state, int32_t page) { state_at(state)->page.page = page; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms,
                       float width, float height, float dpr, float dt, uint32_t still) {
  State *st = state_at(state);
  coax_v_t_ProjectState s = {&st->page, 1};
  coax_v_t_Tree t = {&st->tree, 1};
  coax_v_t_Work w = {&st->work, 1};
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_project_max_vertices() * coaxial_project_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_project_uniform_floats()};
  int64_t n = coaxial_project_frame(s, t, w, out, u, width, height, dpr, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}
