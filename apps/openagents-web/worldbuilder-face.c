// WorldBuilder's C face for the browser (coax/web/host.js): scalar Wasm exports over
// worldbuilder.coax. The host keeps one WorldBuilder; this file decides nothing: the frame, keys,
// the pointer and wheel, the inbox (typed text, the slot's save, the pack's progress, HTTP
// answers), the page's one request handed to the host's HTTP import, the save (action 4), the
// pack's catalog (asset 0) and the scene drawn from it.
#include "worldbuilder.h"
#include "face-address.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#ifndef COAXIAL_IMPORT
#define COAXIAL_IMPORT(name) __attribute__((import_module("env"), import_name(name)))
#endif
COAXIAL_IMPORT("coaxial_http") int32_t coaxial_http(cx_face_addr url, uint32_t url_len, cx_face_addr body, uint32_t body_len, int32_t flags);

static coax_t_WorldBuilder *state_at(uint32_t state) { return (coax_t_WorldBuilder *)(uintptr_t)state; }
static coax_v_t_WorldBuilder at(uint32_t state) { return (coax_v_t_WorldBuilder){state_at(state), 1}; }
static uint8_t inbox[262144];
static uint8_t catalog[262144];

// After every call: the page's request, if it has one.
static void drain(uint32_t state) {
  coax_t_WorldBuilder *s = state_at(state);
  if (!s->req_ready) return;
  int32_t id = coaxial_http((cx_face_addr)(uintptr_t)s->req_url.e, (uint32_t)s->req_url_n,
                            (cx_face_addr)(uintptr_t)s->req_body.e, (uint32_t)s->req_body_n, (int32_t)s->req_flags);
  worldbuilder_requested(at(state), id);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_WorldBuilder) * 1000003u + offsetof(coax_t_WorldBuilder, plates) * 7u + offsetof(coax_t_WorldBuilder, outbox) * 13u + 0x57424c44u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_WorldBuilder); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_WorldBuilder(state_at(state)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_worldbuilder_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_worldbuilder_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_worldbuilder_uniform_floats(); }

static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  worldbuilder_golden(at(state), golden);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_worldbuilder_max_vertices() * coaxial_worldbuilder_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_worldbuilder_uniform_floats()};
  int64_t n = worldbuilder_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  drain(state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  (void)mods; bool used = worldbuilder_key(at(state), key, true); drain(state); return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)mods; return worldbuilder_key(at(state), key, false) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { worldbuilder_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  bool used = worldbuilder_pointer(at(state), x, y, phase, kind, id); drain(state); return used ? 1u : 0u;
}
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(uint32_t state, float dy) { return worldbuilder_wheel(at(state), dy) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)worldbuilder_cursor(at(state)); }
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return worldbuilder_wants_text(at(state)) ? 1u : 0u; }
// The typed line, in device pixels: x0, y0, x1, y1.
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->field.e; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  worldbuilder_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
  drain(state);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)worldbuilder_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { worldbuilder_action_done(at(state)); }

EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof catalog ? (uint32_t)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) {
  (void)state; return i == 0 ? (uint32_t)sizeof catalog : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) worldbuilder_catalog(at(state), (coax_string){(const char *)catalog, (int64_t)bytes});
  if (i >= 0 && i <= 3 && bytes == 0) worldbuilder_pack_failed(at(state));
}
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)worldbuilder_draw_count(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 1u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { (void)k; return (uint32_t)sizeof(((coax_t_WorldBuilder *)0)->objects); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)k; return (uint32_t)(uintptr_t)state_at(state)->objects.e; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { (void)k; return (uint32_t)worldbuilder_store_bytes(at(state)); }
// The store is copied only when the world changed (its version), not every frame.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { (void)k; return (uint32_t)state_at(state)->drawn_version; }
