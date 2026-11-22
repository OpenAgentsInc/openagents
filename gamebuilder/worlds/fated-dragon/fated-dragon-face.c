/* Firm reason: 1 (scalar browser ABI; Fated Dragon is Coax). */
// Fated Dragon's browser face: scalar Wasm exports over fated-dragon.coax for coax/web/host.js.
#include "fated-dragon.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_FatedDragon at(uint32_t state) { return (coax_v_t_FatedDragon){(coax_t_FatedDragon *)(uintptr_t)state, 1}; }
static coax_t_FatedDragon *raw(uint32_t state) { return (coax_t_FatedDragon *)(uintptr_t)state; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_FatedDragon) * 7u + 0x46445247u); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_FatedDragon); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_FatedDragon(raw(state)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)fd_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)fd_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)fd_uniform_floats(); }
EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  (void)still;
  coax_v_f32 out = {(float *)(uintptr_t)vertices, fd_max_vertices() * fd_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, fd_uniform_floats()};
  int64_t n = fd_frame(at(state), out, u, width, height, dpr, dt);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)fd_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) { fd_message(at(state), kind, length); }
EXPORT("coaxial_pointer_buttons_v1") uint32_t coaxial_pointer_buttons_v1(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) {
  fd_pointer(at(state), x, y, phase, kind, id, button);
  return 0;
}
EXPORT("coaxial_mouselook") int32_t coaxial_mouselook(uint32_t state) { return (int32_t)fd_mouselook(at(state)); }
EXPORT("coaxial_look_delta") void coaxial_look_delta(uint32_t state, float dx, float dy) { fd_look_delta(at(state), dx, dy); }
EXPORT("coaxial_pointer_lock") void coaxial_pointer_lock(uint32_t state, int32_t status) { fd_pointer_lock(at(state), status); }
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  return i == 0 && bytes <= (uint32_t)fd_asset_cap() ? (uint32_t)(uintptr_t)raw(state)->dragon.e[0].data.e : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) { (void)state; return i == 0 ? (uint32_t)fd_asset_cap() : 0u; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) { fd_asset_ready(at(state), i, (int64_t)bytes); }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { fd_blur(at(state)); }
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->host_draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)fd_draw_count(at(state)); }
EXPORT("coaxial_draw_blend_v1") int32_t coaxial_draw_blend_v1(uint32_t state, int32_t i) { return (int32_t)fd_draw_blend(at(state), i); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 4; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)fd_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  coax_t_FatedDragon *h = raw(state);
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)h->mesh.e : k == 1 ? (void *)h->records.e : k == 2 ? (void *)h->skin.e : (void *)h->tris.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return (uint32_t)fd_store_bytes(at(state), k); }
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { return (uint32_t)fd_store_version(at(state), k); }
