// Orbit League's C face for the browser (coax/web/host.js): scalar Wasm
// exports over orbitleague.coax, which holds the match, the controls, the
// drawing and the analytics. It decides nothing; after each call it hands
// the analytics batch to the host's HTTP import. The arena is traced by
// orbitleague.wgsl's bg_fs from the uniforms, so there is no scene to bind.
// Online (?room=NAME) its socket commands go out through the host's
// socket imports after each call, as project-face.c's do.
#include "orbitleague.h"
#include "analytics-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_socket_open") int32_t coaxial_socket_open(uint32_t url, uint32_t len);
IMPORT("coaxial_socket_send") void coaxial_socket_send(int32_t id, uint32_t ptr, uint32_t len);
IMPORT("coaxial_socket_close") void coaxial_socket_close(int32_t id);

static uint8_t inbox[4096];
static bool golden;

static coax_t_OrbitLeague *game_at(uint32_t state) { return (coax_t_OrbitLeague *)(uintptr_t)state; }
static coax_v_t_OrbitLeague at(uint32_t state) { return (coax_v_t_OrbitLeague){game_at(state), 1}; }
// The analytics batch, then the pipe's socket commands in order; then the pipe is empty.
static void drain(uint32_t state) {
  track_drain(&game_at(state)->track.e[0]);
  coax_t_Pipe *p = &game_at(state)->pipe.e[0];
  int64_t n = p->n;
  for (int64_t k = 0; k < n; k++) {
    uint32_t from = (uint32_t)(uintptr_t)&p->buf.e[p->from.e[k]], len = (uint32_t)p->len.e[k];
    int32_t id = (int32_t)p->id.e[k];
    if (p->op.e[k] == 1) game_socket_opened(at(state), coaxial_socket_open(from, len));
    else if (p->op.e[k] == 2) coaxial_socket_send(id, from, len);
    else if (p->op.e[k] == 3) coaxial_socket_close(id);
  }
  p->n = 0; p->used = 0;
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_OrbitLeague) * 1000003u + offsetof(coax_t_OrbitLeague, bodies) * 7u + offsetof(coax_t_OrbitLeague, outbox) * 13u + 0x4f524249u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_OrbitLeague); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_orbitleague_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_orbitleague_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_orbitleague_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }
// The arena is traced per pixel: at most this many a frame, so phones and big screens keep 60 FPS.
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) { return 1600000u; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  if (golden) game_golden(at(state), true);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_orbitleague_max_vertices() * coaxial_orbitleague_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_orbitleague_uniform_floats()};
  int64_t n = game_frame(at(state), out, u, w, h, scale, dt, still != 0);
  drain(state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  bool used = game_key(at(state), key, mods);
  drain(state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  return game_key_up(at(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { game_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  bool used = game_pointer(at(state), x, y, phase, kind, id);
  drain(state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)game_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  game_message(at(state), kind, (coax_string){(const char *)inbox, length});
  drain(state);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)game_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)game_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)game_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { game_action_done(at(state)); }
