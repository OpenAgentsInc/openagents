// Everglade's C face for the browser (coax/web/host.js): everglade.coax is
// the page, verse/player.coax walks you (its own library, as the realm
// links it) and verse/grid.coax reads a phone's thumbs. Once a frame it
// passes values between them in order: the page's controls, the player's
// step among the city's walls, then the page's frame with where you
// ended up, after the city keeps you out of the woods (everglade_hold).
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
#include "Player.h"
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
#define NPC_URL "/api/playtest/npc/ask"
#define PLAYER_URL "/api/playtest/player"
static int32_t ask_id = -1, mint_id = -1;
#define WORLD_URL "/api/playtest/world/everglade"
#define EDIT_URL "/api/playtest/realm/everglade/edit"
#define RESET_URL "/api/playtest/world/everglade/reset"
static int32_t world_id = -1, edit_id = -1, post_id = -1;
static bool world_asked;
static void edit_out(uint32_t state);
static bool facts_seen, has_player, mint_tried;
static void ask_out(uint32_t state);
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static coax_t_River river;
static bool river_built;
static uint32_t river_gen, frames;   // the river's bake (store 3) and frames drawn (stores 1 and 2 change every frame)
static float river_levels[105 + 16];   // the river's levels, then the ponds' rows (everglade_levels)
static float river_floats[28 * 8];
static coax_t_Swim swim;
static coax_v_t_River rv(void) { return (coax_v_t_River){&river, 1}; }
// The walls' round bodies, then each rowboat's hull as a capsule along its keel (river.coax W6): so
// you swim and wade round a hull, not through it. 7 numbers each, as the page's bodies.
#define BOAT_BODIES 3
static float hull_bodies[(16 + BOAT_BODIES) * 7];
static int64_t with_hulls(coax_t_Everglade *g, int64_t bodies) {
  int64_t n = bodies < 16 ? bodies : 16;
  memcpy(hull_bodies, g->bodies.e, (size_t)n * 7 * sizeof(float));
  if (river.aboard >= 0) return n;
  for (int64_t i = 0; i < g->float_n && n < 16 + BOAT_BODIES; i++) {
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
static coax_v_t_Everglade at(uint32_t state) {
  coax_v_t_Everglade v = {city(state), 1};
  if (!demo_attached) { demo_attached = true; everglade_attach_demolish(v, (coax_v_t_DemoWorld){&demo_world, 1}); }
  return v;
}
static void start(uint32_t state) {
  if (!track_state.page_n) track_init(tr(), track_cstr("everglade"));
  if (placed) return;
  coax_t_Point spawn = everglade_spawn(at(state));
  player = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(state)));
  sticks = verse_sticks_reset();
  placed = true;
  if (!river_built) {
    everglade_levels(at(state), (coax_v_f32){river_levels, 105 + 16});
    everglade_river_start(rv(), (coax_v_f32){city(state)->heights.e, 105 * 105}, (coax_v_f32){river_levels, 105 + 16}, 104, 2, -104, -104);
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
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_everglade_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_everglade_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_everglade_uniform_floats(); }

// golden mode (goldens.ts): the frame runs still, so the clock holds at noon and the river and leaves hold one moment
static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  start(state);
  frames += 1;
  coax_t_Everglade *g = city(state);
  width = w; height = h; dpr = scale;
  dt = dt < 0 ? 0 : dt > 0.1f ? 0.1f : dt;
  if (!golden) now += dt;
  everglade_controls(at(state), dt, sticks.dyaw, sticks.dpitch);
  if (everglade_warp(at(state))) {
    coax_t_Point spawn = everglade_spawn(at(state));
    player = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(state)));
    // you stand there on the ground: out of the water and any rowboat, breathing, still
    if (river_built) everglade_river_reset(rv());
    swim = (coax_t_Swim){0}; swim.pace = 1; swim.breath = 1; swim.surface = -1000;
  }
  {
    // a blast's throw on you (everglade-watch.coax watch_spell_you): once, into your velocity
    coax_t_Point k = everglade_knock(at(state));
    if (k.x != 0 || k.y != 0 || k.z != 0) {
      player.vel.x += k.x; player.vel.z += k.z;
      if (k.y > 0) { player.vel.y = k.y > player.vel.y ? k.y : player.vel.y; player.on_ground = false; }
    }
  }
  // Hold Person holds you where you stand (everglade-watch.coax)
  bool busy = everglade_busy(at(state)) || everglade_held(at(state));
  if (busy) sticks = verse_sticks_reset();
  if (busy) player = verse_player_input(player, 0, 0, 0, 0, false, false, 0, 0);
  else player = verse_player_input(player, g->key_x, g->key_y, sticks.pad_x, sticks.pad_y, verse_runs(g->run, sticks.run, sticks.pad_x, sticks.pad_y), (g->jump || sticks.jump) && !everglade_afloat(at(state)), 0, 0);
  {
    // the ice under you (everglade-frost.coax): Slippery Ice takes your grip, Difficult Terrain and
    // slush your pace; Prone or caught in the ice you can't move
    coax_t_FrFooting foot = everglade_footing(at(state), player.pos.x, player.pos.z, player.pos.y);
    if (foot.still) player = verse_player_input(player, 0, 0, 0, 0, false, false, 0, 0);
    player.pace = swim.pace * foot.pace * everglade_slow(at(state));   // and Ray of Frost slows you
    player.grip = foot.grip;
  }
  player.theta = everglade_player_theta(at(state));
  sticks = verse_sticks_used(sticks);
  // on the bridge's deck its rails are in your way; off it (in the river under it) its piles are
  bool deck = everglade_on_bridge(at(state), player.pos.x, player.pos.z) && swim.medium < 2;
  int64_t walls = everglade_walls_on(at(state), deck);
  int64_t bodies = with_hulls(g, everglade_bodies_on(at(state), deck));
  coax_t_Vec3 was = player.pos;
  if (river.aboard >= 0) {
    // in a rowboat: W and S row, A and D turn (a thumb's stick too); you sit where it takes you
    float ahead = g->key_y + sticks.pad_y, turn = g->key_x + sticks.pad_x;
    everglade_river_row(rv(), busy ? 0 : ahead, busy ? 0 : turn);
    coax_t_Seat seat = everglade_river_seat(rv());
    player.pos.x = seat.x; player.pos.z = seat.z; player.prev_pos = player.pos;
    player.vel.x = 0; player.vel.z = 0;
    player.face = seat.yaw; player.prev_face = seat.yaw;
  } else {
    float vy0 = player.vel.y;
    player = verse_player_advance_among(player, dt, (coax_v_f32){hull_bodies, bodies * 7}, (coax_v_f32){g->walls.e, walls * 6});
    // Reverse Gravity's cylinder and the Wind Walls move you up (everglade-abilities.coax)
    coax_t_SpLift lift = everglade_lift(at(state), player.pos.x, player.pos.z, was.y, vy0, player.pos.y, player.vel.y, dt);
    if (lift.on) {
      player.prev_pos.y += lift.y - player.pos.y;
      player.pos.y = lift.y; player.vel.y = lift.vy;
      if (lift.y > 0.01f || lift.vy > 0) { player.on_ground = false; player.act = 3; }
    }
  }
  // afloat, the current carries you
  if (swim.medium == 2 || swim.medium == 3) { player.pos.x += swim.flow_x * dt; player.pos.z += swim.flow_z * dt; }
  coax_t_Point kept = everglade_hold(at(state), was.x, was.z, player.pos.x, player.pos.z);
  if (kept.x != player.pos.x || kept.z != player.pos.z) {
    if (kept.x != player.pos.x) player.vel.x = 0;
    if (kept.z != player.pos.z) player.vel.z = 0;
    player.pos.x = kept.x; player.pos.z = kept.z;
    player.prev_pos = player.pos;
  }
  // the river: what water you are in, on the bridge's deck none
  // the rain's rise on it and its current's gain (verse/water-weather.coax, W9)
  everglade_river_weather(rv(), everglade_rise(at(state)), everglade_flow_gain(at(state)));
  coax_v_f32 heights = {g->heights.e, 105 * 105};
  if (everglade_on_bridge(at(state), player.pos.x, player.pos.z) && swim.medium < 2) {
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
  int64_t n = everglade_frame(at(state), out, u, w, h, scale, dt, still != 0 || golden, feet.x, feet.y, feet.z, verse_player_face(player),
                           verse_player_act(player), player.vel.x, player.vel.z,
                           sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
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
  ask_out(state);
  edit_out(state);
  place_frame(state);
  track_frame(tr(), dt, w, h, scale);
  if (n > 0) track_loaded(tr(), track_cstr("load-everglade"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

// The published edits once, then an admin's heard words, ORIGINAL's reset and new list, one request each at a time.
static void edit_out(uint32_t state) {
  coax_t_Everglade *g = city(state);
  edit_face_drain((coax_v_t_RealmEdit){g->ev.ed.e, 1});   /* the shared edit layer's one request */
  if (facts_seen && !world_asked) {
    world_asked = true;
    world_id = coaxial_http((uint32_t)(uintptr_t)WORLD_URL, sizeof WORLD_URL - 1, 0, 0, 4);
  }
  /* heard sentences go one at a time, in the order said: the next is taken only when none is out */
  int64_t n = edit_id < 0 ? everglade_edit_ask(at(state)) : 0;
  if (n > 0) {
    edit_id = coaxial_http((uint32_t)(uintptr_t)EDIT_URL, sizeof EDIT_URL - 1, (uint32_t)(uintptr_t)g->ev.ask.e, (uint32_t)n, 1 | 4 | 8);
    everglade_edit_ask_sent(at(state));
    if (edit_id < 0) { edit_id = -1; everglade_edit_answer(at(state), (coax_string){"0\n", 2}); }
  }
  // ORIGINAL: the designed city back, before any list published after it.
  if (post_id < 0 && everglade_edit_reset(at(state))) {
    post_id = coaxial_http((uint32_t)(uintptr_t)RESET_URL, sizeof RESET_URL - 1, (uint32_t)(uintptr_t)"{}", 2, 1 | 4 | 8);
    if (post_id < 0) { post_id = -1; everglade_edit_saved(at(state), 0); }
  }
  n = everglade_edit_post(at(state));
  if (n > 0 && post_id < 0) {
    post_id = coaxial_http((uint32_t)(uintptr_t)WORLD_URL, sizeof WORLD_URL - 1, (uint32_t)(uintptr_t)g->ev.post.e, (uint32_t)n, 1 | 4 | 8);   /* the player key: a realm admin account publishes too */
    everglade_edit_post_sent(at(state));
    if (post_id < 0) { post_id = -1; everglade_edit_saved(at(state), 0); }
  }
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
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)city(state)->heights.e : k == 1 ? (void *)city(state)->objects.e : (void *)city(state)->palette.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { (void)state; return coaxial_store_cap(k); }
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
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { everglade_blur(at(state)); sticks = verse_sticks_reset(); track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start(state);
  track_pointer(tr(), x, y, phase, kind, (coax_string){"", 0});
  track_drain(&track_state);
  if (everglade_pointer(at(state), x, y, phase, kind, id)) return 1u;
  if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
// The spell bar's icons and card for the host to draw (coax/web/host.js coaxial_media): everglade-spells.coax sp_media.
EXPORT("coaxial_media") uint32_t coaxial_media(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->sp.media.e; }
EXPORT("coaxial_media_len") uint32_t coaxial_media_len(uint32_t state) { return (uint32_t)city(state)->sp.media_n; }
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { everglade_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (place_message(state, kind, inbox, length)) return;
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
    // a golden shot is the designed city: what admins published (test edits too) never holds a release
    // back; the world is still asked for, so the page settles as it always does, and read as nobody's edits
    static const char empty[] = "200\n{\"doc\": null, \"version\": 0}";
    if (golden) everglade_world(at(state), (coax_string){empty, sizeof empty - 1});
    else everglade_world(at(state), (coax_string){(const char *)inbox, length});
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
  if (ask_id >= 0 && kind == 512 + ask_id) {
    ask_id = -1;
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
  return a ? a : place_action();
}
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return place_out_now ? place_outbox() : (uint32_t)(uintptr_t)city(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return place_out_now ? place_outbox_len() : (uint32_t)city(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { if (place_out_now) place_done(); else everglade_action_done(at(state)); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return everglade_wants_text(at(state)) ? 1 : 0; }
// ---- the spells' fire and smoke, drawn after the scene (everglade-fx.wgsl, host.js particles)
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_Everglade *)0)->sp.sprites); }
EXPORT("coaxial_particles") uint32_t coaxial_particles(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->sp.sprites.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(uint32_t state) { return (uint32_t)(city(state)->sp.sprite_n * 12 * sizeof(float)); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(uint32_t state) { return city(state)->started ? (uint32_t)city(state)->sp.sprite_n : 0u; }
EXPORT("coaxial_grass_instances") uint32_t coaxial_grass_instances(uint32_t state) { return city(state)->started ? (uint32_t)everglade_grass_instances(at(state)) : 0u; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->field.e; }
