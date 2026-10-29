// Everglade's C face for the browser (coax/web/host.js): everglade.coax is
// the page, verse/player.coax walks you (its own library, as the realm
// links it) and verse/grid.coax reads a phone's thumbs. Once a frame it
// passes values between them in order: the page's controls, the player's
// step among the city's walls, then the page's frame with where you
// ended up. It decides nothing. Every pointer is an offset into the host's
// WebAssembly.Memory; the page's state is the host's (coaxial_state_size).
#include "everglade.h"
#include "Player.h"
#include "Grid.h"
#include "analytics-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Player player;
static coax_t_Sticks sticks;
static bool placed;
static double now;
static float width, height, dpr;
static uint8_t inbox[16384];
static coax_t_Track track_state;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }

static coax_t_Everglade *city(uint32_t state) { return (coax_t_Everglade *)(uintptr_t)state; }
static coax_v_t_Everglade at(uint32_t state) { return (coax_v_t_Everglade){city(state), 1}; }
static void start(uint32_t state) {
  if (!track_state.page_n) track_init(tr(), track_cstr("everglade"));
  if (placed) return;
  coax_t_Point spawn = everglade_spawn(at(state));
  player = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(state)));
  sticks = verse_sticks_reset();
  placed = true;
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Everglade) * 1000003u + offsetof(coax_t_Everglade, objects) * 7u + offsetof(coax_t_Everglade, draws) * 13u + 0x45474c44u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Everglade); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_everglade_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_everglade_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_everglade_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  start(state);
  coax_t_Everglade *g = city(state);
  width = w; height = h; dpr = scale;
  dt = dt < 0 ? 0 : dt > 0.1f ? 0.1f : dt;
  now += dt;
  everglade_controls(at(state), dt, sticks.dyaw, sticks.dpitch);
  if (everglade_warp(at(state))) {
    coax_t_Point spawn = everglade_spawn(at(state));
    player = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(state)));
  }
  bool busy = everglade_busy(at(state));
  if (busy) sticks = verse_sticks_reset();
  if (busy) player = verse_player_input(player, 0, 0, 0, 0, false, false, 0, 0);
  else player = verse_player_input(player, g->key_x, g->key_y, sticks.pad_x, sticks.pad_y, g->run || sticks.run, g->jump || sticks.jump, 0, 0);
  player.theta = everglade_player_theta(at(state));
  sticks = verse_sticks_used(sticks);
  int64_t walls = everglade_walls(at(state));
  player = verse_player_advance_among(player, dt, (coax_v_f32){0, 0}, (coax_v_f32){g->walls.e, walls * 6});
  coax_t_Vec3 feet = verse_player_feet(player);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_everglade_max_vertices() * coaxial_everglade_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_everglade_uniform_floats()};
  int64_t n = everglade_frame(at(state), out, u, w, h, scale, dt, still != 0, feet.x, feet.y, feet.z, verse_player_face(player),
                           verse_player_act(player), player.vel.x, player.vel.z,
                           sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  track_frame(tr(), dt, w, h, scale);
  if (n > 0) track_loaded(tr(), track_cstr("load-everglade"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return city(state)->started ? (uint32_t)everglade_draws(at(state)) : 0u; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)everglade_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)city(state)->heights.e : (void *)city(state)->objects.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { (void)state; return (uint32_t)everglade_store_cap(k); }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  start(state);
  track_key(tr(), key, false);
  bool used = everglade_key(at(state), key, mods);
  track_drain(&track_state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  track_key_up(tr(), key);
  return everglade_key_up(at(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { everglade_blur(at(state)); sticks = verse_sticks_reset(); track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start(state);
  track_pointer(tr(), x, y, phase, kind, (coax_string){"", 0});
  track_drain(&track_state);
  if (everglade_pointer(at(state), x, y, phase, kind, id)) return 1u;
  if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { everglade_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) track_drain(&track_state);
}

EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  everglade_hash(at(state), (coax_string){(const char *)inbox, length});
}
