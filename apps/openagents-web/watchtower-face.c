// Watchtower's C face for the browser (coax/web/host.js): scalar Wasm
// exports over watchtower.coax, which holds the game and its drawing. It
// decides nothing; after each call it hands the analytics batch and the edit
// layer's one request to the host's HTTP import. No assets, no scene:
// gpu/watchtower.coax's bg_fs draws the grid and the panels' glass, and fx_fs
// the strokes watch_frame writes.
#include "watchtower.h"
#include "analytics-face.h"
#include "edit-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
static bool golden;

static coax_t_Watch *game_at(uint32_t state) { return (coax_t_Watch *)(uintptr_t)state; }
static coax_v_t_Watch at(uint32_t state) { return (coax_v_t_Watch){game_at(state), 1}; }
static void drain(uint32_t state) { track_drain(&game_at(state)->track.e[0]); edit_face_drain((coax_v_t_RealmEdit){game_at(state)->ed.e, 1}); }
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return watch_wants_text(at(state)) ? 1u : 0u; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Watch) * 1000003u + offsetof(coax_t_Watch, outbox) * 13u + 0x57415443u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Watch); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Watch((coax_t_Watch *)(uintptr_t)state); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_watch_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_watch_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_watch_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  watch_golden(at(state), golden);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_watch_max_vertices() * coaxial_watch_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_watch_uniform_floats()};
  int64_t n = watch_frame(at(state), out, u, w, h, scale, dt, still != 0);
  drain(state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  bool used = watch_key(at(state), key, mods);
  drain(state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return watch_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { watch_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  bool used = watch_pointer(at(state), x, y, phase, kind, id);
  drain(state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)watch_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  watch_message(at(state), kind, (coax_string){(const char *)inbox, length});
  drain(state);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)watch_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)game_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)game_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { watch_action_done(at(state)); }
