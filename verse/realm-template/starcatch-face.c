// Star Catcher's C face for the browser (coax/web/host.js): scalar Wasm
// exports over starcatch.coax, which holds the game and its drawing. It
// decides nothing. No assets, no scene: starcatch.wgsl's bg_fs draws the sky
// and the hill, and fx_fs the strokes catch_frame writes.
#include "starcatch.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
static bool golden;

static coax_t_Catch *game_at(uint32_t state) { return (coax_t_Catch *)(uintptr_t)state; }
static coax_v_t_Catch at(uint32_t state) { return (coax_v_t_Catch){game_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Catch) * 1000003u + offsetof(coax_t_Catch, outbox) * 13u + 0x43415443u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Catch); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_catch_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_catch_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_catch_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  catch_golden(at(state), golden);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_catch_max_vertices() * coaxial_catch_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_catch_uniform_floats()};
  int64_t n = catch_frame(at(state), out, u, w, h, scale, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { return catch_key(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return catch_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { catch_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return catch_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)catch_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  catch_message(at(state), kind, (coax_string){(const char *)inbox, length});
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)catch_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)game_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)game_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { catch_action_done(at(state)); }
