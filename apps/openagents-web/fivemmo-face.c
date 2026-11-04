// 5E MMO's C face for the browser (coax/web/host.js): fivemmo.coax is the page (Everglade's
// state, with the battle's view beside it), fivemmo-battle.coax runs the SRD 5e fight (its own C
// unit, Fivemmo-battle.h), aethyr/player.coax walks you and verse/grid.coax reads a phone's
// thumbs, verse/everglade/river.coax is Everglade's river. Once a frame, in order: the camera's
// keys, the battle's step, its numbers into the view, your pace from your Speed, the walk among
// the walls and the others' bodies, kept on the grid (and pulled back to your square when the
// engine couldn't follow), the square under you to the battle, the bodies posed, Everglade's frame
// with your feet, the river, then the view's shapes and HUD. It decides nothing. Every pointer is an
// offset into the host's WebAssembly.Memory; the page's state is the host's (coaxial_state_size).
#include "fivemmo.h"
#include "Fivemmo-battle.h"
#include "Player.h"
#include "Grid.h"
#include "River.h"
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Player player;
static coax_t_Sticks sticks;
static bool placed, golden, aimed;
static double now;
static float width, height, dpr;
static uint8_t inbox[16384];
static uint8_t catalog[16384];
static coax_t_Mb battle;
static coax_t_Mmo view;
static float actors[10 * 12];
static float events[64 * 8];
static float bodies[48 * 7];
static uint8_t name_buf[32];
static int32_t pick = 6;
static uint32_t frames, river_gen;
static coax_t_River river;
static bool river_built;
static float river_levels[177 + 16];
static float river_floats[28 * 8];
static coax_v_t_River rv(void) { return (coax_v_t_River){&river, 1}; }
static coax_v_t_Mb mb(void) { return (coax_v_t_Mb){&battle, 1}; }
static coax_v_t_Mmo mm(void) { return (coax_v_t_Mmo){&view, 1}; }

static coax_t_Everglade *city(uint32_t state) { return (coax_t_Everglade *)(uintptr_t)state; }
// The town's breakable pieces, the spell bar's pictures and a fork's state, handed to the city on
// its first use, as Everglade's face does (the city's frame reads them).
static coax_t_DemoWorld demo_world;
static coax_t_EgFork fork_state;
static bool attached;
static uint8_t media_big[10240];
static coax_v_t_Everglade at(uint32_t state) {
  coax_v_t_Everglade v = {city(state), 1};
  if (!attached) {
    attached = true;
    everglade_attach_demolish(v, (coax_v_t_DemoWorld){&demo_world, 1});
    everglade_attach_media(v, (coax_v_u8){media_big, sizeof media_big});
    everglade_attach_fork(v, (coax_v_t_EgFork){&fork_state, 1});
  }
  return v;
}

// The globals' declared defaults (coax_default_T, from the compiler), before any call:
// _initialize runs this in the browser, the loader natively.
__attribute__((constructor)) static void coaxial_face_defaults(void) {
  coax_default_t_Player(&player);
  coax_default_t_Sticks(&sticks);
  coax_default_t_Mb(&battle);
  coax_default_t_Mmo(&view);
  coax_default_t_River(&river);
  coax_default_t_DemoWorld(&demo_world);
}

static void new_battle(uint32_t state) {
  mb_new(mb(), pick, 7 + (int64_t)frames);
  mmo_set_pick(mm(), pick);
  coax_t_Point p = mmo_start_point(mm());
  player = verse_player_place(p.x, 0, p.z, 3.14159265f);
  (void)state;
}
static void start(uint32_t state) {
  if (placed) return;
  placed = true;
  new_battle(state);
  sticks = verse_sticks_reset();
  // the camera behind you, looking up the grid at the Skeletons
  city(state)->orbit.yaw = 3.14159265f;
  if (!river_built) {
    everglade_levels(at(state), (coax_v_f32){river_levels, 177 + 16});
    everglade_river_start(rv(), (coax_v_f32){city(state)->heights.e, 177 * 177}, (coax_v_f32){river_levels, 177 + 16}, 176, 2, -176, -176);
    river_built = true; river_gen += 1;
  }
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Everglade) * 1000003u + offsetof(coax_t_Everglade, objects) * 7u + offsetof(coax_t_Everglade, draws) * 13u + 0x354d4d4fu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Everglade); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Everglade((coax_t_Everglade *)(uintptr_t)state); coax_default_t_Mmo(&view); coax_default_t_Mb(&battle); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_everglade_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_everglade_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_everglade_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }
// The graphics quality (everglade-quality.coax): the pixels the host draws at most (host.js drawn),
// and what a frame costs, for a headless check (globalThis.coaxial.quality(field)).
static uint32_t quality_state;
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) {
  if (!quality_state || golden) return 0;
  int64_t px = everglade_pixel_budget(at(quality_state), width, height, dpr);
  return px > 0 ? (uint32_t)px : 0u;
}
EXPORT("coaxial_quality_probe") int32_t coaxial_quality_probe(uint32_t state, int32_t field) { return (int32_t)everglade_quality_probe(at(state), field); }
EXPORT("coaxial_probe") int32_t coaxial_probe(uint32_t state) {
  (void)state;
  return (int32_t)(player.pos.x * 10 + 5000.5f) * 10000 + (int32_t)(player.pos.z * 10 + 5000.5f);
}

// What the battle says this frame, into the view.
static void take_battle(uint32_t state) {
  int64_t n = mb_actors(mb(), (coax_v_f32){actors, sizeof actors / sizeof actors[0]});
  for (int64_t i = 0; i < n; i++) {
    int64_t k = mb_name(mb(), i, (coax_v_u8){name_buf, sizeof name_buf});
    mmo_set_name(mm(), i, (coax_string){(const char *)name_buf, k});
  }
  mmo_take(at(state), mm(), (coax_v_f32){actors, sizeof actors / sizeof actors[0]}, n, mb_clock(mb()), mb_beat(mb()),
           mb_wave(mb()), mb_kills(mb()), mb_over(mb()), mb_me(mb()), mb_target(mb()), mb_queued(mb()));
  int64_t e = mb_events(mb(), (coax_v_f32){events, sizeof events / sizeof events[0]});
  mmo_events(at(state), mm(), (coax_v_f32){events, sizeof events / sizeof events[0]}, e);
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  start(state);
  frames += 1;
  coax_t_Everglade *g = city(state);
  width = w; height = h; dpr = scale;
  float frame = dt;
  quality_state = state;
  everglade_quality_frame(at(state), frame, golden);   /* the graphics quality's governor (everglade-quality.coax) */
  dt = verse_player_counts(player, frame);
  if (!golden) now += dt;
  everglade_controls(at(state), dt, sticks.dyaw, sticks.dpitch);
  // the battle on its clock (a golden frame holds it, as it holds the river)
  mb_step(mb(), golden ? 0.0f : dt);
  take_battle(state);
  float pace = mmo_pace(mm());
  bool stuck = pace <= 0;
  player = verse_player_input(player, stuck ? 0 : g->key_x, stuck ? 0 : g->key_y, stuck ? 0 : sticks.pad_x, stuck ? 0 : sticks.pad_y, false, false, 0, 0);
  player.pace = pace;
  player.theta = everglade_player_theta(at(state));
  if (sticks.zoom != 0) everglade_wheel(at(state), sticks.zoom);
  sticks = verse_sticks_used(sticks);
  int64_t walls = everglade_walls_on(at(state), false);
  int64_t nb = mmo_bodies(at(state), mm(), (coax_v_f32){bodies, sizeof bodies / sizeof bodies[0]});
  int64_t floors = everglade_floors_near(at(state), player.pos.x, player.pos.z);
  player = verse_player_advance_in_realm(player, frame, (coax_v_f32){bodies, nb * 7}, (coax_v_f32){g->walls.e, walls * 6},
                                         (coax_v_f32){g->floors.e, floors * 10}, (coax_string){"everglade", sizeof "everglade" - 1});
  // on the grid; and where the engine couldn't take you (a Skeleton in the way, Restrained), back
  coax_t_Point kept = mmo_hold(mm(), player.pos.x, player.pos.z);
  if (mmo_drift(mm(), kept.x, kept.z) > 2.5f) {
    coax_t_Point home = mmo_home(mm());
    float k = dt * 6 > 1 ? 1 : dt * 6;
    kept.x += (home.x - kept.x) * k; kept.z += (home.z - kept.z) * k;
  }
  if (kept.x != player.pos.x || kept.z != player.pos.z) {
    player.pos.x = kept.x; player.pos.z = kept.z; player.prev_pos = player.pos;
  }
  /* the controller's floor is flat at 0; stand on the green's terrain, which rises above it */
  { float gy = mmo_ground(at(state), player.pos.x, player.pos.z);
    if (player.pos.y < gy) { player.pos.y = gy; player.prev_pos.y = gy; if (player.vel.y < 0) player.vel.y = 0; } }
  coax_t_Vec3 feet = verse_player_feet(player);
  int64_t sq = mmo_square(feet.x, feet.z);
  mb_want(mb(), sq / 100, sq % 100);
  float speed = __builtin_sqrtf(player.vel.x * player.vel.x + player.vel.z * player.vel.z);
  mmo_pose(at(state), mm(), feet.x, feet.y, feet.z, verse_player_face(player), speed, dt);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_everglade_max_vertices() * coaxial_everglade_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_everglade_uniform_floats()};
  everglade_guide_begin(at(state), golden);
  int64_t n = everglade_frame(at(state), out, u, w, h, scale, dt, still != 0 || golden, feet.x, feet.y, feet.z, verse_player_face(player),
                              verse_player_act(player), player.vel.x, player.vel.z,
                              sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  (void)n;
  // Everglade's first frame starts its orbit at the town's spawn; turn it once to look up the grid
  if (!aimed && city(state)->started) { city(state)->orbit.yaw = 3.14159265f; aimed = true; }
  n = mmo_finish(at(state), mm(), out, u);
  if (n > 0) {
    bool small = everglade_phone(at(state));
    int64_t at_water = everglade_water_at();
    float *eye = u.data + everglade_eye_at();
    float aspect = w / (h > 1 ? h : 1);
    float fy = aspect * 1.25f; fy = fy < 0.95f ? 0.95f : fy > 1.9f ? 1.9f : fy;
    everglade_river_frame(rv(), (coax_v_f32){u.data + at_water, u.count - at_water}, now, small, eye[0], eye[1], eye[2], 2.0f / (fy * (h > 1 ? h : 1)));
    int64_t at_fx = everglade_fx_at();
    everglade_river_fx(rv(), (coax_v_f32){u.data + at_fx, u.count - at_fx}, golden ? 0 : dt, now, player.pos.x, player.pos.z, 0, eye[0], eye[1], eye[2]);
  }
  return n < 0 ? 0u : (uint32_t)n;
}

// What the view says a key or a press means: a hotbar slot, the next target, a class, a creature.
static void act(uint32_t state, int64_t what) {
  if (what >= 100 && what < 200) { bool used = mb_queue(mb(), what - 100); (void)used; }
  else if (what == 200) mb_next_target(mb());
  else if (what >= 300 && what < 312) { pick = (int32_t)(what - 300); new_battle(state); }
  else if (what >= 400 && what < 500) mb_set_target(mb(), what - 400);
}

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return city(state)->started ? (uint32_t)everglade_draws(at(state)) : 0u; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 4; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)(k == 3 ? everglade_river_node_bytes() : everglade_store_cap(k)); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  if (k == 3) return (uint32_t)(uintptr_t)river.nodes.e;
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)city(state)->ground_store.e : k == 1 ? (void *)city(state)->objects.e : (void *)city(state)->palette.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return k == 1 || k == 2 ? (uint32_t)everglade_store_used(at(state), k) : coaxial_store_cap(k); }   /* the raid's people only while drawn (everglade.coax) */
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) {
  return k == 0 ? (uint32_t)city(state)->ground_gen : k == 3 ? river_gen : frames;
}
EXPORT("coaxial_water_draws") uint32_t coaxial_water_draws(uint32_t state) { (void)state; return 1; }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  start(state);
  int64_t what = mmo_key(at(state), mm(), key, mods);
  if (what >= 100) act(state, what);
  return what != 0 ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  (void)mods;
  return mmo_key_up(at(state), key) ? 1u : 0u;
}
EXPORT("coaxial_focus") void coaxial_focus(uint32_t state) { (void)state; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { city(state)->held = 0; city(state)->drag_on = false; sticks = verse_sticks_reset(); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start(state);
  int64_t what = mmo_pointer(at(state), mm(), x, y, phase, kind, id);
  if (what >= 100) { act(state, what); return 1u; }
  if (what != 0) return 1u;
  if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_media") uint32_t coaxial_media(uint32_t state) { coax_t_Everglade *g = city(state); return g->sp.media_big.count ? (uint32_t)(uintptr_t)g->sp.media_big.data : (uint32_t)(uintptr_t)g->sp.media.e; }
EXPORT("coaxial_media_len") uint32_t coaxial_media_len(uint32_t state) { return (uint32_t)city(state)->sp.media_n; }
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { everglade_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
// The page's facts (600: the ruleset line turns the spells' effects on) and the pack's progress go to
// the city; the rest of Everglade's messages (its guards, its guide, its room) have no place here.
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (kind >= 512) return;
  everglade_message(at(state), kind, (coax_string){(const char *)inbox, length});
}

EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof catalog ? (uint32_t)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) { (void)state; return i == 0 ? (uint32_t)sizeof catalog : 0u; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  start(state);
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) everglade_catalog(at(state), (coax_string){(const char *)catalog, bytes});
  if (i >= 0 && i <= 3 && bytes == 0) everglade_pack_failed(at(state));
}

// #class=NAME: play that class (the screenshot tool can't press keys)
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  int64_t p = mmo_hash_pick((coax_string){(const char *)inbox, length});
  if (p >= 0 && p != pick) { pick = (int32_t)p; new_battle(state); }
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { (void)state; return 0; }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { (void)state; return 0; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { (void)state; }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { (void)state; return 0; }
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_Everglade *)0)->sp.sprites); }
EXPORT("coaxial_particles") uint32_t coaxial_particles(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->sp.sprites.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(uint32_t state) { return (uint32_t)(city(state)->sp.sprite_n * 12 * sizeof(float)); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(uint32_t state) { return city(state)->started ? (uint32_t)city(state)->sp.sprite_n : 0u; }
EXPORT("coaxial_grass_instances") uint32_t coaxial_grass_instances(uint32_t state) { return city(state)->started ? (uint32_t)everglade_grass_instances(at(state)) : 0u; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->field.e; }
