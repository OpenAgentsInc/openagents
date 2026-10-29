// Dead Gears' C face for the browser (coax/web/host.js): scalar Wasm exports
// over deadgears.coax, which holds the fight (verse/deadgears/rules.coax),
// the page's controls, its drawing and its analytics. It decides nothing;
// after each call it hands the analytics batch to the host's HTTP import.
// Every pointer is an offset into the host's WebAssembly.Memory; the page's
// state is the host's (coaxial_state_size).
#include "deadgears.h"
#include "analytics-face.h"
#include "edit-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
static bool golden;

static coax_t_DeadGears *game_at(uint32_t state) { return (coax_t_DeadGears *)(uintptr_t)state; }
static coax_v_t_DeadGears at(uint32_t state) { return (coax_v_t_DeadGears){game_at(state), 1}; }
static void drain(uint32_t state) { track_drain(&game_at(state)->track.e[0]); edit_face_drain((coax_v_t_RealmEdit){game_at(state)->ed.e, 1}); }
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return deadgears_wants_text(at(state)) ? 1u : 0u; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_DeadGears) * 1000003u + offsetof(coax_t_DeadGears, objects) * 7u + offsetof(coax_t_DeadGears, outbox) * 13u + 0x44474552u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_DeadGears); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_deadgears_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_deadgears_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_deadgears_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  if (golden) deadgears_golden(at(state), true);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_deadgears_max_vertices() * coaxial_deadgears_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_deadgears_uniform_floats()};
  int64_t n = deadgears_frame(at(state), out, u, w, h, scale, dt, still != 0);
  drain(state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)game_at(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return game_at(state)->started ? 1u : 0u; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 1u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { (void)k; return (uint32_t)deadgears_store_cap(); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)k; return (uint32_t)(uintptr_t)game_at(state)->objects.e; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  (void)k;
  int64_t n = deadgears_objects(at(state));
  return (uint32_t)((n < 1 ? 1 : n) * 16 * 4);
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  bool used = deadgears_key(at(state), key, mods);
  drain(state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  return deadgears_key_up(at(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { deadgears_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  bool used = deadgears_pointer(at(state), x, y, phase, kind, id);
  drain(state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)deadgears_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  deadgears_message(at(state), kind, (coax_string){(const char *)inbox, length});
  drain(state);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)deadgears_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)game_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)game_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { deadgears_action_done(at(state)); }
