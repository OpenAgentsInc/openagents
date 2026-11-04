// Everglade's C face for the browser (coax/web/host.js): everglade.coax is
// the page, aethyr/player.coax walks you (its own library, as the realm
// links it) and verse/grid.coax reads a phone's thumbs. Once a frame it
// passes values between them in order: the page's controls, the player's
// step among the city's walls, then the page's frame with where you
// ended up, after the city keeps you inside the world's edge (everglade_hold).
// The pack's catalog (asset 0) goes to the page. A question the page writes
// for a character goes to the character chat (/api/playtest/npc/ask) with the
// browser's player key, so the character remembers that player; a browser
// with no key yet is given one first (POST /api/playtest/player), which the
// page keeps (action 5). The answer goes back to the page. An admin's voice
// edits (everglade-voice.coax): the published list is asked for once
// (GET /api/playtest/world/everglade, everyone), heard words go to the
// server's selector (POST /api/playtest/realm/everglade/edit) and the new
// list is published (POST /api/playtest/world/everglade); the server checks
// both are an admin's.
// verse/everglade/river.coax is the river (River.h): built once from the
// page's carved ground, each frame it says what water you are in, whose
// pace, jump, current and breath the face passes to the controller and the
// page, steps the ducks and lily pads, and writes the river's uniform block
// after the page's frame; its baked nodes are store 3. Where you are is
// kept so a reload puts you back (place-face.h, everglade-place.coax): a
// place the hash names wins, else the newer of this browser's and the
// server's copies, moved to open ground if it no longer is. It decides nothing. Every pointer is an offset into the host's
// WebAssembly.Memory; the page's state is the host's (coaxial_state_size).
#include "everglade.h"
#include "Guard-guide-policy.h"
#include "Everglade-step.h"
#include "Grid.h"
#include "River.h"
#include "analytics-face.h"
#include "edit-face.h"
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Player player;
static coax_t_Sticks sticks;
static bool placed;
static double now;
static float width, height, dpr;
static uint8_t inbox[16384];
static uint8_t catalog[16384];
static coax_t_Track track_state;
static coax_t_EgSeen track_seen;   // what everglade_track saw last frame (everglade-track.coax)
#define NPC_URL "/api/playtest/npc/ask"
#define PLAYER_URL "/api/playtest/player"
static int32_t ask_id = -1, mint_id = -1;
/* Everglade's world, or a fork's own (made-<slug>: everglade-fork.coax everglade_world_path) */
static uint8_t world_url_buf[4][128];
static uint32_t world_path(uint32_t state, int what);
static coax_t_EgFork fork_state;   /* FORK's state (everglade-fork.coax everglade_attach_fork) */
#define WORLD_URL_OF(st) (uint32_t)(uintptr_t)world_url_buf[0], world_path(st, 0)
#define RESET_URL_OF(st) (uint32_t)(uintptr_t)world_url_buf[1], world_path(st, 1)
#define EDIT_URL_OF(st) (uint32_t)(uintptr_t)world_url_buf[2], world_path(st, 2)
#define LOOK_URL_OF(st) (uint32_t)(uintptr_t)world_url_buf[3], world_path(st, 3)
static int32_t world_id = -1, edit_id = -1, post_id = -1;
static bool world_asked;
static void edit_out(uint32_t state);
static void bar_frame(uint32_t state);
static bool facts_seen, has_player, mint_tried;
static void ask_out(uint32_t state);
static void guard_out(uint32_t state);
static void guide_out(uint32_t state);
static int32_t guide_id = -1;
static int64_t guide_sequence, guide_intent;
static bool guide_authenticated, guide_world_outer;
static coax_t_GgsSession guide_state;
static coax_v_t_GgsSession guides(void) { return (coax_v_t_GgsSession){&guide_state,1}; }
static int32_t guard_id = -1;
static int64_t guard_sequence;
static coax_t_GaSession guard_state;
static coax_v_t_GaSession gs(void) { return (coax_v_t_GaSession){&guard_state,1}; }
__attribute__((import_module("env"), import_name("coaxial_now_ms"))) extern int64_t guard_now_ms(void);
__attribute__((import_module("env"), import_name("coaxial_http_bounded"))) extern int32_t guard_http(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,int64_t);
__attribute__((import_module("env"), import_name("coaxial_http_cancel"))) extern int32_t guard_cancel(int32_t);
static void room_out(uint32_t state, float dt);
// the guard captain's plan (verse/everglade/captain.coax): one request at most, Coax decides when
static void captain_out(uint32_t state);
static int32_t captain_id = -1;
static int64_t captain_sequence;
static uint8_t captain_body[4096];
static coax_t_Captain captain_state;
static coax_v_t_Captain cs(void) { return (coax_v_t_Captain){&captain_state, 1}; }
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static coax_t_River river;
static bool river_built;
static uint32_t river_gen, frames;   // the river's bake (store 3) and frames drawn (stores 1 and 2 change every frame)
static float river_levels[177 + 16];   // the river's levels, then the ponds' rows (everglade_levels)
static float river_floats[28 * 8];
static coax_t_Swim swim;
static coax_v_t_River rv(void) { return (coax_v_t_River){&river, 1}; }
// The walls' round bodies, then each rowboat's hull as a capsule along its keel (river.coax W6): so
// you swim and wade round a hull, not through it. 7 numbers each, as the page's bodies.
#define BOAT_BODIES 3
static float hull_bodies[BOAT_BODIES * 7];
/* The rowboats' hulls round you (7 floats each), for the movement step; none while you row one. */
static int64_t boat_hulls(coax_t_Everglade *g) {
  int64_t n = 0;
  if (river.aboard >= 0) return 0;
  for (int64_t i = 0; i < g->float_n && n < BOAT_BODIES; i++) {
    const float *f = g->floats.e + i * 8;
    if (f[7] < 1.5f) continue;
    float ax = __builtin_sinf(f[3]), az = __builtin_cosf(f[3]), half = 1.6f - 0.55f;
    float *b = hull_bodies + n * 7;
    b[0] = f[0] - ax * half; b[1] = 0.3f; b[2] = f[2] - az * half;
    b[3] = f[0] + ax * half; b[4] = 0.3f; b[5] = f[2] + az * half;
    b[6] = 0.55f;
    n++;
  }
  return n;
}
#include "place-face.h"
static bool place_back;   // a kept place was put back: checked again when the published pieces come

static coax_t_Everglade *city(uint32_t state) { return (coax_t_Everglade *)(uintptr_t)state; }
// The town's breakable pieces and their debris (everglade-spells.coax), handed to the city on its first use.
static coax_t_DemoWorld demo_world;
static bool demo_attached;
// The spell bar's pictures for the host, room for every slot moved (everglade-arrange.coax).
static uint8_t media_big[10240];
static coax_v_t_Everglade at(uint32_t state) {
  coax_v_t_Everglade v = {city(state), 1};
  if (!demo_attached) {
    demo_attached = true;
    everglade_attach_demolish(v, (coax_v_t_DemoWorld){&demo_world, 1});
    everglade_attach_media(v, (coax_v_u8){media_big, sizeof media_big});
    everglade_attach_fork(v, (coax_v_t_EgFork){&fork_state, 1});
  }
  return v;
}
static uint32_t world_path(uint32_t state, int what) {
  return (uint32_t)everglade_world_path(at(state), (coax_v_u8){world_url_buf[what], sizeof world_url_buf[what]}, what);
}
static void start(uint32_t state) {
  if (!track_state.page_n) track_init(tr(), track_cstr("everglade"));
  if (placed) return;
  coax_t_Point spawn = everglade_spawn(at(state));
  player = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(state)));
  sticks = verse_sticks_reset();
  placed = true;
  if (!river_built) {
    everglade_levels(at(state), (coax_v_f32){river_levels, 177 + 16});
    everglade_river_start(rv(), (coax_v_f32){city(state)->heights.e, 177 * 177}, (coax_v_f32){river_levels, 177 + 16}, 176, 2, -176, -176);
    river_built = true; river_gen += 1;
    swim.pace = 1; swim.breath = 1;
  }
  place_seen(state);
}

// Where you are: your feet, your facing, the camera, the room you're in.
static coax_t_Place place_here(uint32_t state) {
  coax_t_Everglade *g = city(state);
  if (!placed) return (coax_t_Place){0};
  return everglade_place_here(at(state), verse_place_of(player.pos.x, player.pos.y, player.pos.z, verse_player_face(player), g->orbit,
                        everglade_room_at(at(state), player.pos.x, player.pos.z)));
}
// Put you at a kept place, on open ground near it if it no longer is.
static void place_put(uint32_t state, coax_t_Place p) {
  coax_t_Place q = everglade_place_fix(at(state), p);
  player = verse_player_place(q.x, 0, q.z, q.face);
  everglade_place_camera(at(state), q);
  swim = (coax_t_Swim){0}; swim.pace = 1; swim.breath = 1; swim.surface = -1000;
  place_back = true;
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Everglade) * 1000003u + offsetof(coax_t_Everglade, objects) * 7u + offsetof(coax_t_Everglade, draws) * 13u + 0x45474c44u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Everglade); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Everglade((coax_t_Everglade *)(uintptr_t)state); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_everglade_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_everglade_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_everglade_uniform_floats(); }

// golden mode (goldens.ts): the frame runs still, so the clock holds at noon and the river and leaves hold one moment
static bool golden;
// The graphics quality (everglade-quality.coax): the pixels the host draws at most (host.js drawn),
// and what a frame costs, for a headless check (globalThis.coaxial.quality(field)).
static uint32_t quality_state;
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) {
  if (!quality_state || golden) return 0;
  int64_t px = everglade_pixel_budget(at(quality_state), width, height, dpr);
  return px > 0 ? (uint32_t)px : 0u;
}
EXPORT("coaxial_quality_probe") int32_t coaxial_quality_probe(uint32_t state, int32_t field) { return (int32_t)everglade_quality_probe(at(state), field); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }
// Where you stand, for a headless walk (globalThis.coaxial.probe()): x and z to a tenth of a metre,
// as (x + 500) * 10 * 10000 + (z + 500) * 10.
EXPORT("coaxial_probe") int32_t coaxial_probe(uint32_t state) {
  (void)state;
  return (int32_t)(player.pos.x * 10 + 5000.5f) * 10000 + (int32_t)(player.pos.z * 10 + 5000.5f);
}

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  start(state);
  frames += 1;
  coax_t_Everglade *g = city(state);
  width = w; height = h; dpr = scale;
  float frame = dt;   /* the player's clock (aethyr/loop.coax) steps it; the rest moves by what it counts */
  quality_state = state;
  everglade_quality_frame(at(state), frame, golden);   /* the graphics quality's governor (everglade-quality.coax) */
  dt = verse_player_counts(player, frame);
  if (!golden) now += dt;
  everglade_controls(at(state), dt, sticks.dyaw, sticks.dpitch);
  if (everglade_warp(at(state))) {
    coax_t_Point spawn = everglade_spawn(at(state));
    player = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(state)));
    // you stand there on the ground: out of the water and any rowboat, breathing, still
    if (river_built) everglade_river_reset(rv());
    swim = (coax_t_Swim){0}; swim.pace = 1; swim.breath = 1; swim.surface = -1000;
  }
  // Your movement step (everglade-move.coax, the world's half; everglade-step.coax, the controller's):
  // the same Coax step the room server runs for your seat, in the order written there.
  bool busy = everglade_busy(at(state)) || everglade_held(at(state));
  if (busy) sticks = verse_sticks_reset();
  coax_t_EgMoveIn input = {g->key_x, g->key_y, sticks.pad_x, sticks.pad_y, verse_runs(g->run, sticks.run, sticks.pad_x, sticks.pad_y),
                           g->jump || sticks.jump, everglade_player_theta(at(state))};
  if (sticks.zoom != 0) everglade_wheel(at(state), sticks.zoom);   /* a pinch zooms as the wheel does (verse/grid.coax) */
  sticks = verse_sticks_used(sticks);
  coax_t_Seat seat = {0};
  if (river.aboard >= 0) {
    // in a rowboat: W and S row, A and D turn (a thumb's stick too); you sit where it takes you
    float ahead = g->key_y + sticks.pad_y, turn = g->key_x + sticks.pad_x;
    everglade_river_row(rv(), busy ? 0 : ahead, busy ? 0 : turn);
    seat = everglade_river_seat(rv());
  }
  int64_t hulls = boat_hulls(g);
  coax_t_EgMoveWorld mw = everglade_move_world(at(state), player.pos.x, player.pos.y, player.pos.z, swim.medium, swim.pace, swim.flow_x, swim.flow_z,
                                               river.aboard >= 0, seat.x, seat.z, seat.yaw, (coax_v_f32){hull_bodies, hulls * 7});
  player = eg_step_begin(player, input, mw, dt);
  coax_t_EgBody before = eg_step_body(player);
  coax_v_f32 near = everglade_move_bodies(at(state));
  player = eg_step_advance(player, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){g->walls.e, mw.walls * 6}, (coax_v_f32){g->floors.e, mw.floors * 10}, frame);
  coax_t_EgBody moved = everglade_move_after(at(state), mw, before, eg_step_body(player), dt);
  player = eg_step_put(player, moved);
  float show_vx = moved.show_vx, show_vz = moved.show_vz;
  // the river: what water you are in, on the bridge's deck none
  // the rain's rise on it and its current's gain (verse/water-weather.coax, W9)
  everglade_river_weather(rv(), everglade_rise(at(state)), everglade_flow_gain(at(state)));
  coax_v_f32 heights = {g->heights.e, 177 * 177};
  // Water Breathing (everglade-tide.coax): your breath never runs short under water
  everglade_river_gills(rv(), everglade_water_breathing(at(state)));
  // on the bridge's deck, or in Part Water's trench (everglade-tide.coax), you are out of the water
  if ((everglade_on_bridge(at(state), player.pos.x, player.pos.z) && swim.medium < 2) || everglade_parted(at(state), player.pos.x, player.pos.z)) {
    swim = (coax_t_Swim){0}; swim.pace = 1; swim.breath = 1; swim.surface = -1000;
  } else {
    // your feet's height over the bed: over the water's surface (levitating, falling) you are not in it
    // on the ice over the water you stand on it (everglade-frost.coax)
    swim = everglade_river_swim_over(rv(), heights, player.pos.x, player.pos.z, player.pos.y + everglade_ice_lift(at(state), player.pos.x, player.pos.z), g->dive && !busy, dt, now);
  }
  if (swim.defeated) {
    // the sixth level of Exhaustion: you come to on the nearer bank
    player = verse_player_place(swim.bank_x, 0, swim.bank_z, verse_player_face(player));
    swim.medium = 0; swim.pace = 1;
  }
  // the spells' fields over the floats (Reverse Gravity, the Wind Walls: everglade-abilities.coax), as last frame left them
  static float spell_fields[16 * 10];
  everglade_river_fields(rv(), (coax_v_f32){spell_fields, 16 * 10}, everglade_spell_fields(at(state), (coax_v_f32){spell_fields, 16 * 10}));
  int64_t floats = everglade_river_floats(rv(), dt, (coax_v_f32){river_floats, 28 * 8});
  // a rowboat near you to board, or E (the prompt) to board it or step ashore
  everglade_boat_near(at(state), swim.medium == 3 || swim.aloft || busy ? -1 : everglade_river_boat_near(rv(), player.pos.x, player.pos.z));
  if (everglade_boat_start(at(state)) && river.aboard < 0) everglade_river_board(rv(), everglade_river_first_boat(rv()));
  if (everglade_boat_take(at(state))) {
    if (river.aboard >= 0) {
      coax_t_Seat off = everglade_river_leave(rv(), heights, now);
      if (off.on) player = verse_player_place(off.x, 0, off.z, off.yaw);
      swim = everglade_river_swim(rv(), heights, player.pos.x, player.pos.z, false, 0, now);
    } else if (everglade_river_board(rv(), everglade_river_boat_near(rv(), player.pos.x, player.pos.z))) {
      swim = everglade_river_swim(rv(), heights, player.pos.x, player.pos.z, false, 0, now);
    }
  }
  bool small = everglade_phone(at(state));
  everglade_water(at(state), swim.medium, swim.feet, swim.breath, swim.levels,
                  everglade_river_vertices(small), everglade_river_patches(), (coax_v_f32){river_floats, 28 * 8}, floats);
  coax_t_Vec3 feet = verse_player_feet(player);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_everglade_max_vertices() * coaxial_everglade_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_everglade_uniform_floats()};
  everglade_guide_begin(at(state), golden);
  int64_t n = everglade_frame(at(state), out, u, w, h, scale, dt, still != 0 || golden, feet.x, feet.y, feet.z, verse_player_face(player),
                           verse_player_act(player), player.vel.x, player.vel.z,
                           sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  guide_out(state);
  n = everglade_finish_frame(at(state), out, u);
  if (n > 0) {
    // the river's uniform block, after the page's frame has written the rest
    int64_t at_water = everglade_water_at();
    float *eye = u.data + everglade_eye_at();
    float aspect = w / (h > 1 ? h : 1);
    float fy = aspect * 1.25f; fy = fy < 0.95f ? 0.95f : fy > 1.9f ? 1.9f : fy;
    everglade_river_frame(rv(), (coax_v_f32){u.data + at_water, u.count - at_water}, now, small, eye[0], eye[1], eye[2], 2.0f / (fy * (h > 1 ? h : 1)));
    // the wakes, the particles and the weir's sheet (W6)
    int64_t at_fx = everglade_fx_at();
    everglade_river_fx(rv(), (coax_v_f32){u.data + at_fx, u.count - at_fx}, golden ? 0 : dt, now, player.pos.x, player.pos.z, swim.medium, eye[0], eye[1], eye[2]);
    // what struck the water this frame (spells, falling debris: W8) splashes, rings and shoves the floats
    static float hits[32];
    int64_t hn = everglade_water_hits(at(state), (coax_v_f32){hits, 32});
    for (int64_t k = 0; k < hn; k++) {
      // the Water Orb's water running out over the floats (kind 4, everglade-spells.coax SP_SHOVE): its own impulse
      if ((int64_t)hits[k * 4 + 2] == 4) everglade_river_shove(rv(), hits[k * 4], hits[k * 4 + 1], hits[k * 4 + 3], everglade_orb_impulse(), everglade_orb_sheet());
      else everglade_river_splash(rv(), hits[k * 4], hits[k * 4 + 1], (int64_t)hits[k * 4 + 2], hits[k * 4 + 3], now);
    }
  }
  guard_out(state);
  captain_out(state);
  ask_out(state);
  room_out(state, dt);
  edit_out(state);
  place_frame(state);
  if (placed) everglade_track(at(state), (coax_v_t_EgSeen){&track_seen, 1}, tr(), player.pos.x, player.pos.z);

  bar_frame(state);
  track_frame(tr(), dt, w, h, scale);
  // world-enter and load-engine once it draws, like every realm (the analytics contract, analytics-contract.json)
  static bool entered;
  if (n > 0 && !entered) { entered = true; track_world(tr(), track_cstr("everglade")); track_loaded(tr(), track_cstr("load-engine")); }
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

// The spell bar as its player arranged it (everglade-arrange.coax): kept in this browser by the
// host's action 12 (message 12 gives it back at load), and for an account on the server at
// /api/playtest/progress/<slot>/hotbar, asked for once and sent at each change. Not in golden mode.
#define BAR_ACTION 12
static uint8_t bar_text[400], bar_url[128], bar_body[1024];
static uint32_t bar_text_n;
static bool bar_out_now;
static int32_t bar_ask_id = -1;
static int32_t bar_post_id = -1;   // the send in flight, whose answer everglade-arrange.coax reads (#12719)
static void bar_frame(uint32_t state) {
  if (golden || !facts_seen) return;
  if (everglade_bar_ask(at(state))) {
    int64_t un = everglade_bar_url(at(state), (coax_v_u8){bar_url, sizeof bar_url});
    if (un > 0) bar_ask_id = coaxial_http((uint32_t)(uintptr_t)bar_url, (uint32_t)un, 0, 0, 4 | 8);
    if (bar_ask_id < 0) everglade_bar_server(at(state), (coax_string){"0\n", 2});   /* no copy coming: send without one */
  }
  if (!bar_text_n) { int64_t n = everglade_bar_keep(at(state), (coax_v_u8){bar_text, sizeof bar_text}); if (n > 0) bar_text_n = (uint32_t)n; }
  int64_t bn = everglade_bar_send(at(state), (coax_v_u8){bar_body, sizeof bar_body});
  if (bn > 0) {
    int64_t un = everglade_bar_url(at(state), (coax_v_u8){bar_url, sizeof bar_url});
    bar_post_id = un > 0 ? coaxial_http((uint32_t)(uintptr_t)bar_url, (uint32_t)un, (uint32_t)(uintptr_t)bar_body, (uint32_t)bn, 1 | 4 | 8) : -1;
    if (bar_post_id < 0) everglade_bar_unsent(at(state));
  }
}

// The published edits once, then an admin's heard words, ORIGINAL's reset and new list, one request each at a time.
static void edit_out(uint32_t state) {
  coax_t_Everglade *g = city(state);
  edit_face_drain((coax_v_t_RealmEdit){g->ev.ed.e, 1});   /* the shared edit layer's one request */
  if (everglade_fork_ready(at(state))) {   /* FORK's one request (everglade-fork.coax) */
    int32_t id = coaxial_http((uint32_t)(uintptr_t)fork_state.req_url.e, (uint32_t)fork_state.req_url_n,
                              (uint32_t)(uintptr_t)fork_state.req_body.e, (uint32_t)fork_state.req_body_n, (int32_t)fork_state.req_flags);
    everglade_fork_requested(at(state), id);
  }
  if (facts_seen && !world_asked) {
    world_asked = true;
    world_id = coaxial_http(WORLD_URL_OF(state), 0, 0, 4);
  }
  /* heard sentences go one at a time, in the order said: the next is taken only when none is out */
  int64_t n = edit_id < 0 ? everglade_edit_ask(at(state)) : 0;
  if (n > 0) {
    /* a look goes to .../look with the next frame drawn (host flag 32) */
    if (everglade_edit_look(at(state))) edit_id = coaxial_http(LOOK_URL_OF(state), (uint32_t)(uintptr_t)g->ev.ask.e, (uint32_t)n, 1 | 4 | 8 | 32);
    else edit_id = coaxial_http(EDIT_URL_OF(state), (uint32_t)(uintptr_t)g->ev.ask.e, (uint32_t)n, 1 | 4 | 8);
    everglade_edit_ask_sent(at(state));
    if (edit_id < 0) { edit_id = -1; everglade_edit_answer(at(state), (coax_string){"0\n", 2}); }
  }
  // ORIGINAL: the designed city back, before any list published after it.
  if (post_id < 0 && everglade_edit_reset(at(state))) {
    post_id = coaxial_http(RESET_URL_OF(state), (uint32_t)(uintptr_t)"{}", 2, 1 | 4 | 8);
    if (post_id < 0) { post_id = -1; everglade_edit_saved(at(state), 0); }
  }
  n = everglade_edit_post(at(state));
  if (n > 0 && post_id < 0) {
    post_id = coaxial_http(WORLD_URL_OF(state), (uint32_t)(uintptr_t)g->ev.post.e, (uint32_t)n, 1 | 4 | 8);   /* the player key: a realm admin account publishes too */
    everglade_edit_post_sent(at(state));
    if (post_id < 0) { post_id = -1; everglade_edit_saved(at(state), 0); }
  }
}

// The realm's room (everglade-room.coax; coax/web/api/room.c): where you are about every second,
// answered with the others and the chat's lines since; a line said goes on its own.
#define ROOM_EVERY_S 1.0
#define ROOM_WAIT_S 10.0
static int32_t room_id = -1, chat_id = -1;
static double room_at = -10, room_asked_at;
static char room_url_buf[120], chat_url_buf[120];
static uint8_t room_body[256], chat_body[1200];
static coax_v_t_RoomState rooms(uint32_t state) { return (coax_v_t_RoomState){&city(state)->room.e[0], 1}; }
static void room_out(uint32_t state, float dt) {
  if (golden || !facts_seen) return;
  if (!has_player && !mint_tried) {
    mint_tried = true;
    mint_id = coaxial_http((uint32_t)(uintptr_t)PLAYER_URL, sizeof PLAYER_URL - 1, (uint32_t)(uintptr_t)"{}", 2, 1 | 4);
    if (mint_id < 0) mint_id = -1;
    return;
  }
  if (mint_id >= 0 || !has_player) return;
  (void)dt;
  coax_string said = chat_id < 0 ? everglade_chat_take(at(state)) : (coax_string){"", 0};
  if (said.count > 0) {
    track_chat(tr(), track_cstr("room"), said.count);   // its length only
    int64_t un = room_url(everglade_room_name(), true, (coax_v_u8){(uint8_t *)chat_url_buf, sizeof chat_url_buf});
    coax_string sid = {(const char *)track_state.sid.e, track_state.sid_n};
    int64_t bn = room_chat_body(said, sid, (coax_v_u8){chat_body, sizeof chat_body});
    if (un > 0 && bn > 0) {
      chat_id = coaxial_http((uint32_t)(uintptr_t)chat_url_buf, (uint32_t)un, (uint32_t)(uintptr_t)chat_body, (uint32_t)bn, 1 | 4 | 8);
      if (chat_id < 0) { chat_id = -1; everglade_chat_answer(at(state), (coax_string){"0\n", 2}); }
    } else everglade_chat_answer(at(state), (coax_string){"400\n", 4});
  }
  if (room_id >= 0 && now - room_asked_at < ROOM_WAIT_S) return;
  if (now - room_at < ROOM_EVERY_S) return;
  room_at = now;
  int64_t un = room_url(everglade_room_name(), false, (coax_v_u8){(uint8_t *)room_url_buf, sizeof room_url_buf});
  coax_t_Vec3 feet = verse_player_feet(player);
  float air = player.on_ground ? 0.0f : feet.y;
  int64_t bn = room_presence_body(feet.x, air, feet.z, verse_player_face(player), city(state)->room.e[0].last, (coax_v_u8){room_body, sizeof room_body});
  if (un <= 0 || bn <= 0) return;
  room_id = coaxial_http((uint32_t)(uintptr_t)room_url_buf, (uint32_t)un, (uint32_t)(uintptr_t)room_body, (uint32_t)bn, 1 | 4 | 8);
  if (room_id < 0) room_id = -1;
  room_asked_at = now;
}

// The page's question, once the browser has a player key (or minting one failed).
static void ask_out(uint32_t state) {
  int64_t n = everglade_ask(at(state));
  if (n <= 0 || ask_id >= 0 || !facts_seen) return;
  if (!has_player && !mint_tried) {
    mint_tried = true;
    mint_id = coaxial_http((uint32_t)(uintptr_t)PLAYER_URL, sizeof PLAYER_URL - 1, (uint32_t)(uintptr_t)"{}", 2, 1 | 4);
    if (mint_id < 0) mint_id = -1;
    return;
  }
  if (mint_id >= 0) return;
  ask_id = coaxial_http((uint32_t)(uintptr_t)NPC_URL, sizeof NPC_URL - 1, (uint32_t)(uintptr_t)city(state)->ask.e, (uint32_t)n, 1 | 4 | 8);
  everglade_ask_sent(at(state));
  track_chat(tr(), track_cstr("npc"), n);   // a question to a character: its length only
  if (ask_id < 0) { ask_id = -1; everglade_answer(at(state), (coax_string){"0\n", 2}); }
}
static bool has_line(uint32_t length, const char *prefix) {
  size_t k = strlen(prefix);
  const char *p = (const char *)inbox, *end = p + length;
  for (const char *line = p; line < end;) {
    const char *nl = memchr(line, '\n', (size_t)(end - line));
    if (!nl) nl = end;
    if ((size_t)(nl - line) > k && !memcmp(line, prefix, k)) return true;
    line = nl + 1;
  }
  return false;
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
// What the host uploads: the ground and the river only when built, the objects and joints every frame.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) {
  return k == 0 ? (uint32_t)city(state)->ground_gen : k == 3 ? river_gen : frames;
}
// The last draw is the river's (everglade.coax WATER_DRAW): the host draws it after the rest, in two halves.
EXPORT("coaxial_water_draws") uint32_t coaxial_water_draws(uint32_t state) { (void)state; return 1; }

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
EXPORT("coaxial_focus") void coaxial_focus(uint32_t state) { everglade_guard_focus(at(state), true); guard_out(state); guide_out(state); }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { everglade_blur(at(state)); guard_out(state); guide_out(state); sticks = verse_sticks_reset(); track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start(state);
  track_pointer(tr(), x, y, phase, kind, (coax_string){"", 0});
  track_drain(&track_state);
  if (everglade_pointer(at(state), x, y, phase, kind, id)) return 1u;
  if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
// The spell bar's icons and card for the host to draw (coax/web/host.js coaxial_media): everglade-spells.coax sp_media.
EXPORT("coaxial_media") uint32_t coaxial_media(uint32_t state) { coax_t_Everglade *g = city(state); return g->sp.media_big.count ? (uint32_t)(uintptr_t)g->sp.media_big.data : (uint32_t)(uintptr_t)g->sp.media.e; }
EXPORT("coaxial_media_len") uint32_t coaxial_media_len(uint32_t state) { return (uint32_t)city(state)->sp.media_n; }
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { everglade_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (kind == 601 || kind == 603) { everglade_guard_message(at(state), kind, (coax_string){(const char *)inbox,length}); guard_out(state); }
  if (kind == 606) { guide_authenticated = ggp_viewer((coax_string){(const char *)inbox,length}); guide_out(state); }
  if (kind == 601 || kind == 603) guide_out(state);
  if (guide_id >= 0 && kind == 512 + guide_id) {
    guide_id = -1;
    ggs_receive(guides(), guide_sequence, guard_now_ms(), (coax_string){(const char *)inbox,length});
    return;
  }
  if (guard_id >= 0 && kind == 512 + guard_id) {
    guard_id = -1;
    ga_receive(gs(), guard_sequence, guard_now_ms(), (coax_string){(const char *)inbox, length});
    return;
  }
  if (captain_id >= 0 && kind == 512 + captain_id) {
    captain_id = -1;
    everglade_captain_receive(at(state), cs(), captain_sequence, guard_now_ms(), (coax_string){(const char *)inbox, length});
    return;
  }
  if (place_message(state, kind, inbox, length)) return;
  if (kind == BAR_ACTION) { if (!golden) everglade_bar_kept(at(state), (coax_string){(const char *)inbox, length}); return; }
  if (bar_ask_id >= 0 && kind == 512 + bar_ask_id) { bar_ask_id = -1; everglade_bar_server(at(state), (coax_string){(const char *)inbox, length}); return; }
  if (bar_post_id >= 0 && kind == 512 + bar_post_id) { bar_post_id = -1; everglade_bar_heard(at(state), (coax_string){(const char *)inbox, length}); return; }
  if (kind == 600) { facts_seen = true; if (has_line(length, "player=")) has_player = true; }
  if (mint_id >= 0 && kind == 512 + mint_id) {
    mint_id = -1;
    if (length > 4 && !memcmp(inbox, "200\n", 4)) {
      everglade_keep_player(at(state), (coax_string){(const char *)inbox + 4, length - 4});
      has_player = true;
    }
    return;
  }
  if (world_id >= 0 && kind == 512 + world_id) {
    world_id = -1;
    guide_world_outer = ggp_world_answer((coax_string){(const char *)inbox,length});
    // a golden shot is the designed city: what admins published (test edits too) never holds a release
    // back; the world is still asked for, so the page settles as it always does, and read as nobody's edits
    static const char empty[] = "200\n{\"doc\": null, \"version\": 0}";
    if (golden) everglade_world(at(state), (coax_string){empty, sizeof empty - 1});
    else { everglade_world(at(state), (coax_string){(const char *)inbox, length}); }
    // a piece published since may stand where you were put back
    if (place_back && !place_keeper.moved) place_put(state, place_here(state));
    return;
  }
  if (edit_id >= 0 && kind == 512 + edit_id) { edit_id = -1; everglade_edit_answer(at(state), (coax_string){(const char *)inbox, length}); return; }
  if (post_id >= 0 && kind == 512 + post_id) {
    post_id = -1;
    int status = 0;
    for (uint32_t i = 0; i < length && inbox[i] >= '0' && inbox[i] <= '9'; i++) status = status * 10 + (inbox[i] - '0');
    everglade_edit_saved(at(state), status);
    return;
  }
  if (room_id >= 0 && kind == 512 + room_id) {
    room_id = -1;
    int64_t st = room_take(rooms(state), (coax_string){(const char *)inbox, length});
    if (st == 403 && has_player) { has_player = false; mint_tried = false; }
    return;
  }
  if (chat_id >= 0 && kind == 512 + chat_id) {   // a line said: the room asked now, so it shows; a refusal waits and tries again (everglade-room.coax)
    chat_id = -1; room_at = -10;
    everglade_chat_answer(at(state), (coax_string){(const char *)inbox, length});
    return;
  }
  if (ask_id >= 0 && kind == 512 + ask_id) {
    ask_id = -1;
    if (length < 1 || inbox[0] != '2') track_fail(tr(), track_cstr("npc"), length > 2 ? (inbox[0] - '0') * 100 + (inbox[1] - '0') * 10 + (inbox[2] - '0') : 0);
    everglade_answer(at(state), (coax_string){(const char *)inbox, length});
    return;
  }
  everglade_message(at(state), kind, (coax_string){(const char *)inbox, length});
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) track_drain(&track_state);
}

EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof catalog ? (uint32_t)(uintptr_t)catalog : 0u;
}
// Physical destination allocation observation; the typed host admits its
// requested byte span before copying into this existing asset buffer.
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) {
  (void)state;return i == 0 ? (uint32_t)sizeof catalog : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  start(state);
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) everglade_catalog(at(state), (coax_string){(const char *)catalog, bytes});
  if (i >= 0 && i <= 3 && bytes == 0) everglade_pack_failed(at(state));
}

EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  everglade_hash(at(state), (coax_string){(const char *)inbox, length});
  // a hash that named a place (or the map) wins over the kept one
  coax_t_Everglade *g = city(state);
  verse_place_named(place_k(), g->warp || g->above);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) {
  int32_t a = (int32_t)everglade_action(at(state));
  if (!a) a = place_action();
  if (!a && bar_text_n) { bar_out_now = true; a = BAR_ACTION; }
  return a;
}
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return bar_out_now ? (uint32_t)(uintptr_t)bar_text : place_out_now ? place_outbox() : (uint32_t)(uintptr_t)city(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return bar_out_now ? bar_text_n : place_out_now ? place_outbox_len() : (uint32_t)city(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { if (bar_out_now) { bar_out_now = false; bar_text_n = 0; } else if (place_out_now) place_done(); else everglade_action_done(at(state)); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return everglade_wants_text(at(state)) ? 1 : 0; }
// ---- the spells' fire and smoke, drawn after the scene (everglade-fx.wgsl, host.js particles)
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_Everglade *)0)->sp.sprites); }
EXPORT("coaxial_particles") uint32_t coaxial_particles(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->sp.sprites.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(uint32_t state) { return (uint32_t)(city(state)->sp.sprite_n * 12 * sizeof(float)); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(uint32_t state) { return city(state)->started ? (uint32_t)city(state)->sp.sprite_n : 0u; }
EXPORT("coaxial_grass_instances") uint32_t coaxial_grass_instances(uint32_t state) { return city(state)->started ? (uint32_t)everglade_grass_instances(at(state)) : 0u; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->field.e; }

// Coax owns the URL/body, consent, cadence, deadline, decoding and lifecycle.
// This binding only moves bounded byte views and aborts the revoked fetch.
static void guard_out(uint32_t state) {
  int64_t monotonic_ms = guard_now_ms();
  coax_t_GaInput page_input = everglade_guard_take(at(state));
  coax_t_GaInput policy_input;
  _Static_assert(sizeof policy_input == sizeof page_input, "shared guard input ABI");
  memcpy(&policy_input, &page_input, sizeof policy_input);
  ga_page(gs(), policy_input, monotonic_ms);
  if (guard_id >= 0 && !ga_pending(gs(), guard_sequence)) { guard_cancel(guard_id); guard_id = -1; }
  coax_t_GaEffect effect = ga_effect(gs(), monotonic_ms);
  if (!effect.body.count) return;
  guard_sequence = ga_sequence(gs());
  guard_id = guard_http((uint32_t)(uintptr_t)effect.url.data, (uint32_t)effect.url.count,
                       (uint32_t)(uintptr_t)effect.body.data, (uint32_t)effect.body.count,
                       (uint32_t)effect.flags, effect.timeout_ms);
  ga_sent(gs(), guard_sequence, guard_id >= 0);
}
// The captain's request: Coax owns the body, cadence and deadline; a dropped one is aborted.
static void captain_out(uint32_t state) {
  int64_t monotonic_ms = guard_now_ms();
  if (captain_id >= 0 && !everglade_captain_pending(cs(), captain_sequence)) { guard_cancel(captain_id); captain_id = -1; }
  coax_t_CapEffect effect = everglade_captain_effect(at(state), cs(), monotonic_ms, (coax_v_u8){captain_body, sizeof captain_body});
  if (!effect.body.count || captain_id >= 0) return;
  captain_sequence = effect.sequence;
  captain_id = guard_http((uint32_t)(uintptr_t)effect.url.data, (uint32_t)effect.url.count,
                          (uint32_t)(uintptr_t)effect.body.data, (uint32_t)effect.body.count, 5u, effect.timeout_ms);
  everglade_captain_sent(cs(), captain_sequence, captain_id >= 0);
}
EXPORT("coaxial_captain_probe") int32_t coaxial_captain_probe(uint32_t state, int32_t field) { (void)state; return (int32_t)everglade_captain_probe(cs(), field); }
// The whole game update precedes this call; only the local view is copied
// back before HUD draw. C performs transport/clock/span effects only.
static void guide_out(uint32_t state) {
  int64_t ms = guard_now_ms();
  coax_t_EgGuideInput page = everglade_guide_take(at(state));
  guide_intent = ggp_intent(guide_intent, page.asked, page.boundary, page.asked_epoch, page.epoch);
  coax_t_GgpFacts facts = page.facts;
  facts.world_known = ggp_world_ready(facts.world_known, guide_world_outer);
  coax_t_GgObservation observation = ggp_project(facts);
  coax_t_GgsInput input = {0};
  input.observation = observation; input.epoch = page.epoch; input.actor = facts.guard;
  input.generation = page.generation; input.world_revision = page.world_revision;
  input.goal_revision = ggp_goal_key(facts.followed, facts.stage); input.intent = guide_intent;
  input.configured = page.configured; input.authenticated = guide_authenticated;
  input.suspended = page.suspended; input.blocked = page.blocked; input.boundary = page.boundary;
  input.url = page.url; input.hint_intent = page.hint_intent;
  ggs_step(guides(), input, ms);
  if (guide_id >= 0 && !ggs_pending(guides(), guide_sequence)) { guard_cancel(guide_id); guide_id = -1; }
  coax_t_GgsEffect effect = ggs_effect(guides(), ms);
  if (effect.body.count) {
    guide_sequence = effect.sequence;
    guide_id = guard_http((uint32_t)(uintptr_t)effect.url.data, (uint32_t)effect.url.count,
        (uint32_t)(uintptr_t)effect.body.data, (uint32_t)effect.body.count,
        (uint32_t)effect.flags, effect.timeout_ms);
    ggs_sent(guides(), guide_sequence, guide_id >= 0);
  }
  everglade_guide_view(at(state), ggs_view(guides(), ms), observation, ggp_offer(facts), ms);
}
EXPORT("coaxial_guard_probe") int32_t coaxial_guard_probe(uint32_t state, int32_t field) {
  if (field == 248) return guide_authenticated ? 1 : 0;
  if (field >= 240 && field <= 252) return (int32_t)everglade_guide_probe(at(state), field);
  if (field >= 200) return (int32_t)ggs_probe(guides(), field);
  return (int32_t)ga_probe(gs(), field);
}

// The globals' declared defaults (coax_default_T, from the compiler), before any call:
// _initialize runs this in the browser, the loader natively.
__attribute__((constructor)) static void coaxial_face_defaults(void) {
  coax_default_t_Player(&player);
  coax_default_t_Sticks(&sticks);
  coax_default_t_GaSession(&guard_state);
  coax_default_t_River(&river);
  coax_default_t_DemoWorld(&demo_world);
}
