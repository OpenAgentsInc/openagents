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
// both are an admin's. It lends the page a grid to walk the city again.
// verse/everglade/river.coax is the river (River.h): built once from the
// page's carved ground, each frame it says what water you are in, whose
// pace, jump, current and breath the face passes to the controller and the
// page, steps the ducks and lily pads, and writes the river's uniform block
// after the page's frame; its baked nodes are store 3. It decides nothing. Every pointer is an offset into the host's
// WebAssembly.Memory; the page's state is the host's (coaxial_state_size).
#include "everglade.h"
#include "Player.h"
#include "Grid.h"
#include "River.h"
#include "analytics-face.h"
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
static uint8_t reach_grid[400 * 400];
static uint32_t reach_queue[400 * 400];
static void edit_out(uint32_t state);
static bool facts_seen, has_player, mint_tried;
static void ask_out(uint32_t state);
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static coax_t_River river;
static bool river_built;
static float river_levels[105];
static float river_floats[9 * 8];
static coax_t_Swim swim;
static coax_v_t_River rv(void) { return (coax_v_t_River){&river, 1}; }

static coax_t_Everglade *city(uint32_t state) { return (coax_t_Everglade *)(uintptr_t)state; }
static coax_v_t_Everglade at(uint32_t state) { return (coax_v_t_Everglade){city(state), 1}; }
static void start(uint32_t state) {
  if (!track_state.page_n) track_init(tr(), track_cstr("everglade"));
  if (placed) return;
  coax_t_Point spawn = everglade_spawn(at(state));
  player = verse_player_place(spawn.x, spawn.y, spawn.z, everglade_spawn_face(at(state)));
  sticks = verse_sticks_reset();
  placed = true;
  if (!river_built) {
    everglade_levels(at(state), (coax_v_f32){river_levels, 105});
    everglade_river_start(rv(), (coax_v_f32){city(state)->heights.e, 105 * 105}, (coax_v_f32){river_levels, 105}, 104, 2, -104, -104);
    river_built = true;
    swim.pace = 1; swim.breath = 1;
  }
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
  else player = verse_player_input(player, g->key_x, g->key_y, sticks.pad_x, sticks.pad_y, g->run || sticks.run, (g->jump || sticks.jump) && !everglade_afloat(at(state)), 0, 0);
  player.pace = swim.pace;
  player.theta = everglade_player_theta(at(state));
  sticks = verse_sticks_used(sticks);
  int64_t walls = everglade_walls(at(state));
  coax_t_Vec3 was = player.pos;
  player = verse_player_advance_among(player, dt, (coax_v_f32){0, 0}, (coax_v_f32){g->walls.e, walls * 6});
  // afloat, the current carries you
  if (swim.medium >= 2) { player.pos.x += swim.flow_x * dt; player.pos.z += swim.flow_z * dt; }
  coax_t_Point kept = everglade_hold(at(state), was.x, was.z, player.pos.x, player.pos.z);
  if (kept.x != player.pos.x || kept.z != player.pos.z) {
    if (kept.x != player.pos.x) player.vel.x = 0;
    if (kept.z != player.pos.z) player.vel.z = 0;
    player.pos.x = kept.x; player.pos.z = kept.z;
    player.prev_pos = player.pos;
  }
  // the river: what water you are in, on the bridge's deck none
  coax_v_f32 heights = {g->heights.e, 105 * 105};
  if (everglade_on_bridge(at(state), player.pos.x, player.pos.z) && swim.medium < 2) {
    swim = (coax_t_Swim){0}; swim.pace = 1; swim.breath = 1; swim.surface = -1000;
  } else {
    swim = everglade_river_swim(rv(), heights, player.pos.x, player.pos.z, g->dive && !busy, dt, now);
  }
  if (swim.defeated) {
    // the sixth level of Exhaustion: you come to on the nearer bank
    player = verse_player_place(swim.bank_x, 0, swim.bank_z, verse_player_face(player));
    swim.medium = 0; swim.pace = 1;
  }
  int64_t floats = everglade_river_floats(rv(), dt, (coax_v_f32){river_floats, 9 * 8});
  bool small = everglade_phone(at(state));
  everglade_water(at(state), swim.medium, swim.feet, swim.breath, swim.levels,
                  everglade_river_vertices(small), everglade_river_patches(), (coax_v_f32){river_floats, 9 * 8}, floats);
  coax_t_Vec3 feet = verse_player_feet(player);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_everglade_max_vertices() * coaxial_everglade_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_everglade_uniform_floats()};
  int64_t n = everglade_frame(at(state), out, u, w, h, scale, dt, still != 0, feet.x, feet.y, feet.z, verse_player_face(player),
                           verse_player_act(player), player.vel.x, player.vel.z,
                           sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  if (n > 0) {
    // the river's uniform block, after the page's frame has written the rest
    int64_t at_water = everglade_water_at();
    float *eye = u.data + everglade_eye_at();
    float aspect = w / (h > 1 ? h : 1);
    float fy = aspect * 1.25f; fy = fy < 0.95f ? 0.95f : fy > 1.9f ? 1.9f : fy;
    everglade_river_frame(rv(), (coax_v_f32){u.data + at_water, u.count - at_water}, now, small, eye[0], eye[1], eye[2], 2.0f / (fy * (h > 1 ? h : 1)));
  }
  ask_out(state);
  edit_out(state);
  track_frame(tr(), dt, w, h, scale);
  if (n > 0) track_loaded(tr(), track_cstr("load-everglade"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

// The published edits once, then an admin's heard words, ORIGINAL's reset and new list, one request each at a time.
static void edit_out(uint32_t state) {
  coax_t_Everglade *g = city(state);
  if (facts_seen && !world_asked) {
    world_asked = true;
    world_id = coaxial_http((uint32_t)(uintptr_t)WORLD_URL, sizeof WORLD_URL - 1, 0, 0, 4);
  }
  int64_t n = everglade_edit_ask(at(state));
  if (n > 0 && edit_id < 0) {
    edit_id = coaxial_http((uint32_t)(uintptr_t)EDIT_URL, sizeof EDIT_URL - 1, (uint32_t)(uintptr_t)g->ev.ask.e, (uint32_t)n, 1 | 4);
    everglade_edit_ask_sent(at(state));
    if (edit_id < 0) { edit_id = -1; everglade_edit_answer(at(state), (coax_string){"0\n", 2}); }
  }
  // ORIGINAL: the designed city back, before any list published after it.
  if (post_id < 0 && everglade_edit_reset(at(state))) {
    post_id = coaxial_http((uint32_t)(uintptr_t)RESET_URL, sizeof RESET_URL - 1, (uint32_t)(uintptr_t)"{}", 2, 1 | 4);
    if (post_id < 0) { post_id = -1; everglade_edit_saved(at(state), 0); }
  }
  n = everglade_edit_post(at(state));
  if (n > 0 && post_id < 0) {
    post_id = coaxial_http((uint32_t)(uintptr_t)WORLD_URL, sizeof WORLD_URL - 1, (uint32_t)(uintptr_t)g->ev.post.e, (uint32_t)n, 1 | 4);
    everglade_edit_post_sent(at(state));
    if (post_id < 0) { post_id = -1; everglade_edit_saved(at(state), 0); }
  }
  if (everglade_edit_wants_check(at(state)))
    everglade_edit_check(at(state), (coax_v_u8){reach_grid, sizeof reach_grid}, (coax_v_u32){reach_queue, sizeof reach_queue / sizeof reach_queue[0]});
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
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { everglade_wheel(at(state), dy); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (kind == 600) { facts_seen = true; if (has_line(length, "player=")) has_player = true; }
  if (mint_id >= 0 && kind == 512 + mint_id) {
    mint_id = -1;
    if (length > 4 && !memcmp(inbox, "200\n", 4)) {
      everglade_keep_player(at(state), (coax_string){(const char *)inbox + 4, length - 4});
      has_player = true;
    }
    return;
  }
  if (world_id >= 0 && kind == 512 + world_id) { world_id = -1; everglade_world(at(state), (coax_string){(const char *)inbox, length}); return; }
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
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)everglade_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)city(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { everglade_action_done(at(state)); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return everglade_wants_text(at(state)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)city(state)->field.e; }
