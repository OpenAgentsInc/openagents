// The fixture realm's C face for the browser (coax/web/host.js): scalar
// Wasm exports over fixture.coax. The host keeps one Fixture; this file
// decides nothing. It is the smallest realm module the realm host takes
// (docs/verse/realm-host.md): layout and sizes, the frame, keys, the pointer,
// the inbox (the slot's save and the page facts) and actions (keep, go).
#include "fixture.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Fixture *state_at(uint32_t state) { return (coax_t_Fixture *)(uintptr_t)state; }
static coax_v_t_Fixture at(uint32_t state) { return (coax_v_t_Fixture){state_at(state), 1}; }
static uint8_t inbox[16384];

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Fixture) * 1000003u + offsetof(coax_t_Fixture, plates) * 7u + offsetof(coax_t_Fixture, outbox) * 13u + 0x46495855u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Fixture); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_fixture_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_fixture_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_fixture_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_fixture_max_vertices() * coaxial_fixture_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_fixture_uniform_floats()};
  int64_t n = fixture_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { (void)mods; return fixture_key(at(state), key, true) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)mods; return fixture_key(at(state), key, false) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { fixture_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return fixture_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)fixture_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  fixture_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)fixture_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { fixture_action_done(at(state)); }
