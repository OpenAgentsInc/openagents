// Fairy Forest's C face for the browser (coax/web/host.js): fairy.coax is
// the page, verse/player.coax walks you (its own library), verse/grid.coax
// reads a phone's thumbs and verse/fairy-forest/stream.coax is the stream
// and the pond (Stream.h). Once a frame it passes values between them in
// order: the page's controls, the player's step among the forest's walls
// with a fairy's flight (every Space press, or a tap on a phone's right
// half, is one wing beat up, with no limit; held, she hovers; let go, she
// glides down; Shift in the air dives), the stream's say on what water you are in,
// then the page's frame with where you ended up, and the stream's uniform
// block after it. The stream is built once from the page's carved ground; its
// baked nodes are store 2. It decides nothing. Every pointer is an offset into
// the host's WebAssembly.Memory; the page's state is the host's.
#include "fairy.h"
#include "Player.h"
#include "Grid.h"
#include "Stream.h"
#include "analytics-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define IMPORT(name) __attribute__((import_module("env"), import_name(name)))
IMPORT("coaxial_http") int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags);

static coax_t_Player player;
static coax_t_Sticks sticks;
static bool placed;
static double now;
static float width, height, dpr;
static uint8_t inbox[16384];
static coax_t_Track track_state;
static coax_t_Stream stream;
static bool stream_built;
static float course[48];
static float pads[7 * 8];
static coax_t_Swim swim;
static bool jump_was;
static float flap_phase;
static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static coax_v_t_Stream sv(void) { return (coax_v_t_Stream){&stream, 1}; }

#define FLAP_UP 5.2f      /* m/s up from each beat in the air: about 0.55 m a beat */
#define GLIDE -1.5f       /* m/s: the gentle fall between beats */
#define HOVER -0.2f       /* m/s: the fall while Space is held */
#define CEILING 40.0f     /* m over the ground: beats stop lifting here */
#define DIVE -7.0f        /* m/s: the fall while Shift is held in the air, wings folded */

static coax_t_Fairy *fairy(uint32_t state) { return (coax_t_Fairy *)(uintptr_t)state; }
static coax_v_t_Fairy at(uint32_t state) { return (coax_v_t_Fairy){fairy(state), 1}; }
#include "place-face.h"
static void start(uint32_t state) {
  if (!track_state.page_n) track_init(tr(), track_cstr("fairy-forest"));
  if (placed) return;
  coax_t_Point spawn = fairy_spawn(at(state));
  player = verse_player_place(spawn.x, spawn.y, spawn.z, fairy_spawn_face());
  sticks = verse_sticks_reset();
  placed = true;
  verse_place_golden(place_k(), golden);
  place_seen(state);
  if (!stream_built) {
    int64_t n = fairy_course((coax_v_f32){course, 48});
    coax_t_Fairy *f = fairy(state);
    fairy_stream_start(sv(), (coax_v_f32){f->heights.e, fairy_heights(at(state))}, (coax_v_f32){course, n}, fairy_level(),
                       fairy_pond_x(), fairy_pond_z(), 80, 2, -80, -80);
    stream_built = true;
    swim.pace = 1;
  }
}

// Where you are, kept so a reload puts you back (place-face.h).
static coax_t_Place place_here(uint32_t state) {
  if (!placed) return (coax_t_Place){0};
  return fairy_place_of(at(state), player.pos.x, player.pos.y, player.pos.z, verse_player_face(player));
}
static void place_put(uint32_t state, coax_t_Place p) {
  coax_t_Place q = fairy_place_fix(at(state), p);
  player = verse_player_place(q.x, 0, q.z, q.face);
  fairy_place_camera(at(state), q);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Fairy) * 1000003u + offsetof(coax_t_Fairy, objects) * 7u + offsetof(coax_t_Fairy, draws) * 13u + 0x46414952u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Fairy); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_fairy_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_fairy_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_fairy_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  start(state);
  coax_t_Fairy *g = fairy(state);
  width = w; height = h; dpr = scale;
  dt = dt < 0 ? 0 : dt > 0.1f ? 0.1f : dt;
  now += dt;
  fairy_golden(at(state), golden);
  fairy_controls(at(state), dt, sticks.dyaw, sticks.dpitch);
  bool jump = g->jump || sticks.jump;
  int64_t taps = fairy_taps(at(state)) + (sticks.jump ? 1 : 0);
  bool afloat = swim.medium >= 2;
  bool grounded = player.on_ground;
  player = verse_player_input(player, g->key_x, g->key_y, sticks.pad_x, sticks.pad_y, verse_runs(g->run, sticks.run, sticks.pad_x, sticks.pad_y), (jump || taps > 0) && !afloat, 0, 0);
  player.pace = swim.pace;
  player.theta = fairy_player_theta(at(state));
  // flight: every press is one wing beat up, as many as you like (on the
  // ground the player's jump lifts you); between beats you glide down, and
  // held you hover
  bool beat = taps > 0 || (jump && !jump_was);
  if (beat) flap_phase = 1.5708f + 6.2832f * (float)(int)(flap_phase / 6.2832f + 1.0f);   /* wings to the top, then the downstroke */
  if (beat && !grounded && !afloat && player.pos.y < CEILING) player.vel.y = player.vel.y > 0 ? player.vel.y * 0.5f + FLAP_UP : FLAP_UP;
  if (!player.on_ground && player.vel.y < (jump ? HOVER : GLIDE)) player.vel.y = jump ? HOVER : GLIDE;
  if (player.pos.y > CEILING && player.vel.y > 0) player.vel.y = 0;
  // Shift in the air dives (on the ground it still walks): a fast fall, wings folded down
  bool diving = g->run && !player.on_ground && !afloat && !beat;
  if (diving && player.vel.y > DIVE) player.vel.y = DIVE;
  jump_was = jump;
  sticks = verse_sticks_used(sticks);
  int64_t walls = fairy_walls(at(state));
  player = verse_player_advance_among(player, dt, (coax_v_f32){0, 0}, (coax_v_f32){g->walls.e, walls * 6});
  coax_t_Vec3 feet = verse_player_feet(player);
  coax_v_f32 heights = {g->heights.e, fairy_heights(at(state))};
  swim = fairy_stream_swim(sv(), heights, player.pos.x, player.pos.z, feet.y, false, dt, now);
  // the wings: a slow beat standing, a full stroke after each press, a flutter while you hover or glide
  flap_phase += dt * (player.on_ground ? 3.0f : jump ? 22.0f : 11.0f);
  if (diving) flap_phase = 4.7124f;   /* sin = -1: the wings held down */
  if (flap_phase > 6283.0f) flap_phase -= 6283.1853f;
  int64_t floats = fairy_stream_floats(sv(), dt, (coax_v_f32){pads, 7 * 8});
  bool small = fairy_phone(at(state));
  fairy_water(at(state), swim.medium, fairy_stream_vertices(small), fairy_stream_patches(sv()), (coax_v_f32){pads, 7 * 8}, floats);
  // in the water you are drawn at its float line; else at the ground plus how high you fly
  float air = player.on_ground ? 0.0f : feet.y;   /* standing (on the ground or a crown) is not flying */
  float lift = feet.y;
  if (swim.medium >= 2) lift = swim.feet - fairy_ground(at(state), feet.x, feet.z);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_fairy_max_vertices() * coaxial_fairy_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_fairy_uniform_floats()};
  int64_t n = fairy_frame(at(state), out, u, w, h, scale, dt, still != 0, feet.x, lift, feet.z, verse_player_face(player),
                          verse_player_act(player), player.vel.x, player.vel.z, air, flap_phase,
                          sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  if (n > 0) {
    int64_t at_water = fairy_water_at();
    float *eye = u.data + fairy_eye_at();
    float aspect = w / (h > 1 ? h : 1);
    float fy = aspect * 1.25f; fy = fy < 0.95f ? 0.95f : fy > 1.9f ? 1.9f : fy;
    fairy_stream_frame(sv(), (coax_v_f32){u.data + at_water, u.count - at_water}, golden ? 2.0 : now, small, eye[0], eye[1], eye[2], 2.0f / (fy * (h > 1 ? h : 1)));
  }
  place_frame(state);
  track_frame(tr(), dt, w, h, scale);
  if (n > 0) track_loaded(tr(), track_cstr("load-fairy-forest"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)fairy(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return fairy(state)->started ? (uint32_t)fairy_draws(at(state)) : 0u; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 3; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)(k == 2 ? fairy_stream_node_bytes() : fairy_store_cap(k)); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  if (k == 2) return (uint32_t)(uintptr_t)stream.nodes.e;
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)fairy(state)->heights.e : (void *)fairy(state)->objects.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { (void)state; return coaxial_store_cap(k); }
// The last draw is the stream's: the host draws it after the rest, in two halves.
EXPORT("coaxial_water_draws") uint32_t coaxial_water_draws(uint32_t state) { (void)state; return 1; }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  start(state);
  track_key(tr(), key, false);
  bool used = fairy_key(at(state), key, mods);
  track_drain(&track_state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  track_key_up(tr(), key);
  return fairy_key_up(at(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { fairy_blur(at(state)); sticks = verse_sticks_reset(); track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start(state);
  track_pointer(tr(), x, y, phase, kind, (coax_string){"", 0});
  track_drain(&track_state);
  if (fairy_pointer(at(state), x, y, phase, kind, id)) return 1u;
  if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { fairy_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (place_message(state, kind, inbox, length)) return;
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) track_drain(&track_state);
}
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { (void)state; return place_action(); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { (void)state; return place_outbox(); }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { (void)state; return place_outbox_len(); }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { (void)state; place_done(); }
