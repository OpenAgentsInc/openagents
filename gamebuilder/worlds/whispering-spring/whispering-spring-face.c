/* Firm reason: 1 (scalar browser ABI; The Whispering Spring is Coax). */
// The Whispering Spring's browser face: scalar Wasm exports over world-spring.coax for coax/web/host.js.
#include "world-spring.h"
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_Spring at(uint32_t state) { return (coax_v_t_Spring){(coax_t_Spring *)(uintptr_t)state, 1}; }
static coax_t_Spring *raw(uint32_t state) { return (coax_t_Spring *)(uintptr_t)state; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_Spring) * 7u + 0x57535052u); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Spring); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Spring(raw(state)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)ws_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)ws_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)ws_uniform_floats(); }
EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  (void)still;
  coax_v_f32 out = {(float *)(uintptr_t)vertices, ws_max_vertices() * ws_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, ws_uniform_floats()};
  int64_t n = ws_frame(at(state), out, u, width, height, dpr, dt);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->inbox.e; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)ws_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) { ws_message(at(state), kind, length); }
EXPORT("coaxial_pointer_buttons_v1") uint32_t coaxial_pointer_buttons_v1(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) {
  ws_pointer(at(state), x, y, phase, kind, id, button);
  return 0;
}
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  return i == 0 && bytes <= (uint32_t)ws_asset_cap() ? (uint32_t)(uintptr_t)raw(state)->models.e : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) { (void)state; return i == 0 ? (uint32_t)ws_asset_cap() : 0u; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) { ws_asset_ready(at(state), i, (int64_t)bytes); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)ws_cursor(at(state)); }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { ws_blur(at(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)raw(state)->action; }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)raw(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { ws_action_done(at(state)); }
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)raw(state)->host_draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)ws_draw_count(at(state)); }
EXPORT("coaxial_draw_blend_v1") int32_t coaxial_draw_blend_v1(uint32_t state, int32_t i) { (void)state; (void)i; return 0; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 4; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)ws_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  coax_t_Spring *h = raw(state);
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)h->mesh.e : k == 1 ? (void *)h->records.e : k == 2 ? (void *)h->skin.e : (void *)h->tris.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return (uint32_t)ws_store_bytes(at(state), k); }
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { return (uint32_t)ws_store_version(at(state), k); }
