// The Grid's C face for the browser (coax/web/host.js): thegrid.coax is the
// page, verse/player.coax walks you (its own library, as the realm links
// it) and verse/grid.coax reads a phone's thumbs. Once a frame it passes
// values between them in order: the page's controls, the player's step
// among the stations' walls, then the page's frame with where you ended up.
// grid.data (asset 0) goes to the page; the other four assets are the GPU's
// and the page is only told they came. Cast.bin (asset 5, and 6 as the GPU's
// copy; 7 its texture sheets) is opened here with verse/cast.coax, which
// plays each person's clips and writes their joint palettes, store 1, for
// the people the page lists (thegrid-people.coax). The page's room, the-grid,
// is posted where you are about five times a second, and its answer handed
// back; a browser with no player key gets one first. Where you are is kept so a reload
// puts you back (place-face.h). It decides nothing. Every pointer is
// an offset into the host's WebAssembly.Memory; the page's state is the
// host's (coaxial_state_size).
#include "thegrid.h"
#include "Player.h"
#include "Grid.h"
#include "Cast.h"
#include "analytics-face.h"
#include "edit-face.h"
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Player player;
static coax_t_Sticks sticks;
static bool placed, golden;
static double now;
static float width, height, dpr;
static uint8_t inbox[4096];
static uint8_t data[65536];
// Cast.bin: the character (and the robots iOS draws from it), read by verse/cast.coax
#define CAST_CAP (6u << 20)
static uint8_t cast_bytes[CAST_CAP];
static uint32_t cast_n;
static coax_t_Cast cast_set;
static bool cast_open;
#define PEOPLE_MAX 16
#define JOINTS_MAX 65
static coax_t_Anim anims[PEOPLE_MAX];
static float anim_whose[PEOPLE_MAX];
static coax_t_Joint palette[PEOPLE_MAX * JOINTS_MAX];
static uint32_t palette_bytes;
// the room the-grid (thegrid-people.coax; coax/web/api/room.c)
#define PLAYER_URL "/api/playtest/player"
#define ROOM_WAIT_S 5.0
static int32_t room_id = -1, mint_id = -1;
static bool facts_seen, has_player, mint_tried;
static double room_at = -10, room_asked_at;
static char room_url_buf[120];
static uint8_t room_body[256];
static void room_out(uint32_t state);
static void people_out(uint32_t state);
static coax_t_Track track_state;
static coax_t_GridSeen track_seen;   // what thegrid_track saw last frame (thegrid-track.coax)
#define SUGGEST_URL "/api/playtest/survey/answer"
static int32_t post_id = -1;
static void post_out(uint32_t state);
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
#include "place-face.h"

static coax_t_TheGrid *grid(uint32_t state) { return (coax_t_TheGrid *)(uintptr_t)state; }
static coax_v_t_TheGrid at(uint32_t state) { return (coax_v_t_TheGrid){grid(state), 1}; }
static coax_v_t_RealmEdit ed(uint32_t state) { return (coax_v_t_RealmEdit){grid(state)->ed.e, 1}; }
static void start(uint32_t state) {
  if (!track_state.page_n) track_init(tr(), track_cstr("the-grid"));
  if (placed) return;
  thegrid_golden(at(state), golden);
  player = verse_player_place(0, 0, 0, 0);
  sticks = verse_sticks_reset();
  placed = true;
  verse_place_golden(place_k(), golden);
  thegrid_walls(at(state));   // the page started: its orbit is where you start
  place_seen(state);
}

static coax_t_Place place_here(uint32_t state) {
  if (!placed) return (coax_t_Place){0};
  return verse_place_of(player.pos.x, player.pos.y, player.pos.z, verse_player_face(player), grid(state)->orbit, -1);
}
static void place_put(uint32_t state, coax_t_Place p) {
  if (thegrid_diving(at(state))) return;   // WALK AROUND from the homepage lands at the start spot
  coax_t_Place q = thegrid_place_fix(at(state), p);
  player = verse_player_place(q.x, 0, q.z, q.face);
  thegrid_place_camera(at(state), q);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_TheGrid) * 1000003u + offsetof(coax_t_TheGrid, robots) * 7u + offsetof(coax_t_TheGrid, draws) * 13u + 0x47524944u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_TheGrid); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_thegrid_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_thegrid_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_thegrid_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  start(state);
  coax_t_TheGrid *g = grid(state);
  thegrid_golden(at(state), golden);
  width = w; height = h; dpr = scale;
  dt = dt < 0 ? 0 : dt > 0.1f ? 0.1f : dt;
  now += dt;
  thegrid_controls(at(state), dt, sticks.dyaw, sticks.dpitch);
  bool busy = thegrid_busy(at(state));
  if (busy) sticks = verse_sticks_reset();
  if (busy) player = verse_player_input(player, 0, 0, 0, 0, false, false, 0, 0);
  else player = verse_player_input(player, g->key_x, g->key_y, sticks.pad_x, sticks.pad_y, verse_runs(g->run, sticks.run, sticks.pad_x, sticks.pad_y), g->jump || sticks.jump, 0, 0);
  player.theta = thegrid_player_theta(at(state));
  if (sticks.zoom != 0) thegrid_wheel(at(state), sticks.zoom);   /* a pinch zooms as the wheel does (verse/grid.coax) */
  sticks = verse_sticks_used(sticks);
  int64_t walls = thegrid_walls(at(state));
  player = verse_player_advance_among(player, dt, (coax_v_f32){0, 0}, (coax_v_f32){g->walls.e, walls * 6});
  coax_t_Vec3 feet = verse_player_feet(player);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_thegrid_max_vertices() * coaxial_thegrid_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_thegrid_uniform_floats()};
  int64_t n = thegrid_frame(at(state), out, u, w, h, scale, dt, still != 0, feet.x, feet.y, feet.z, verse_player_face(player),
                            verse_player_act(player), player.vel.x, player.vel.z,
                            sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  post_out(state);
  room_out(state);
  people_out(state);
  edit_face_drain(ed(state));
  place_frame(state);
  thegrid_track(at(state), (coax_v_t_GridSeen){&track_seen, 1}, tr(), feet.x, feet.z);

  track_frame(tr(), dt, w, h, scale);
  // world-enter and load-engine once it draws, like every realm (the analytics contract, analytics-contract.json)
  static bool entered;
  if (n > 0 && thegrid_draws(at(state)) > 0 && !entered) { entered = true; track_world(tr(), track_cstr("the-grid")); track_loaded(tr(), track_cstr("load-engine")); }
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

// Each person the page lists, posed: their clips moved on for how they move (verse_cast_update) and
// their joints placed where they stand (verse_cast_pose), into store 1. A slot that holds someone new
// starts their clips afresh. In golden mode everyone holds one moment.
static void people_out(uint32_t state) {
  coax_t_TheGrid *g = grid(state);
  palette_bytes = 0;
  if (!cast_open) return;
  coax_v_u8 b = {cast_bytes, cast_n};
  int64_t n = g->people_n, joints = cast_set.joints;
  if (n > PEOPLE_MAX) n = PEOPLE_MAX;
  if (joints <= 0 || joints > JOINTS_MAX) return;
  double t = golden ? 1.0 : now;
  for (int64_t i = 0; i < n; i++) {
    const float *p = &g->people.e[i * 6];
    if (anim_whose[i] != p[5]) { anims[i] = (coax_t_Anim){0}; anims[i].fade_start = -1; anim_whose[i] = p[5]; }
    int64_t act = (int64_t)p[4];
    anims[i] = verse_cast_update(b, cast_set, anims[i], act, act == 3 && p[1] > 0.05f, t);
    if (!verse_cast_pose(b, cast_set, anims[i], t, p[0], p[1], p[2], p[3], (coax_v_t_Joint){&palette[i * joints], joints})) return;
  }
  palette_bytes = (uint32_t)(n * joints * sizeof(coax_t_Joint));
}

// Where you are, to the room about five times a second, once the browser has a player key (minting
// one when it has none); the answer is the others, for the page.
static void room_out(uint32_t state) {
  if (golden || !facts_seen) return;
  if (!has_player && !mint_tried) {
    mint_tried = true;
    mint_id = coaxial_http((uint32_t)(uintptr_t)PLAYER_URL, sizeof PLAYER_URL - 1, (uint32_t)(uintptr_t)"{}", 2, 1 | 4);
    if (mint_id < 0) mint_id = -1;
    return;
  }
  if (mint_id >= 0 || !has_player) return;
  if (room_id >= 0 && now - room_asked_at < ROOM_WAIT_S) return;
  if (now - room_at < thegrid_room_every()) return;
  room_at = now;
  int64_t un = room_url(thegrid_room_name(at(state)), false, (coax_v_u8){(uint8_t *)room_url_buf, sizeof room_url_buf});
  coax_t_Vec3 feet = verse_player_feet(player);
  int64_t bn = room_presence_body(feet.x, feet.y > 0 ? feet.y : 0, feet.z, verse_player_face(player), 0, (coax_v_u8){room_body, sizeof room_body});
  if (un <= 0 || bn <= 0) return;
  room_id = coaxial_http((uint32_t)(uintptr_t)room_url_buf, (uint32_t)un, (uint32_t)(uintptr_t)room_body, (uint32_t)bn, 1 | 4 | 8);
  if (room_id < 0) room_id = -1;
  room_asked_at = now;
}

// A suggestion the page wrote goes to the site's feedback; its answer comes back to the page.
static void post_out(uint32_t state) {
  int64_t n = thegrid_post(at(state));
  if (n <= 0 || post_id >= 0) return;
  post_id = coaxial_http((uint32_t)(uintptr_t)SUGGEST_URL, sizeof SUGGEST_URL - 1, (uint32_t)(uintptr_t)grid(state)->post.e, (uint32_t)n, 1 | 4 | 8);
  thegrid_post_sent(at(state));
  if (post_id < 0) { post_id = -1; thegrid_post_answer(at(state), (coax_string){"0\n", 2}); }
}

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)grid(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return grid(state)->started ? (uint32_t)thegrid_draws(at(state)) : 0u; }
// Store 0 the robots, store 1 the people's joint palettes.
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 1 ? (uint32_t)sizeof palette : (uint32_t)sizeof(((coax_t_TheGrid *)0)->robots); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { return k == 1 ? (uint32_t)(uintptr_t)palette : (uint32_t)(uintptr_t)grid(state)->robots.e; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return k == 1 ? palette_bytes : (uint32_t)thegrid_store_bytes(at(state)); }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  start(state);
  track_key(tr(), key, false);
  bool used = thegrid_key(at(state), key, mods);
  edit_face_drain(ed(state));
  track_drain(&track_state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  track_key_up(tr(), key);
  return thegrid_key_up(at(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { thegrid_blur(at(state)); sticks = verse_sticks_reset(); track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start(state);
  track_pointer(tr(), x, y, phase, kind, (coax_string){"", 0});
  track_drain(&track_state);
  if (thegrid_pointer(at(state), x, y, phase, kind, id)) { edit_face_drain(ed(state)); return 1u; }
  if (kind != 0) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return kind != 0 ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { thegrid_wheel(at(state), dy); }
EXPORT("coaxial_tilt") void coaxial_tilt(uint32_t state, float dyaw, float dpitch) { thegrid_tilt(at(state), dyaw, dpitch); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)thegrid_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (place_message(state, kind, inbox, length)) return;
  if (kind == 600) {
    facts_seen = true;
    for (uint32_t i = 0; i + 7 <= length; i++)
      if ((i == 0 || inbox[i - 1] == '\n') && !memcmp(inbox + i, "player=", 7) && i + 7 < length && inbox[i + 7] != '\n') has_player = true;
  }
  if (mint_id >= 0 && kind == 512 + mint_id) {
    mint_id = -1;
    if (length > 4 && !memcmp(inbox, "200\n", 4)) { thegrid_keep_player(at(state), (coax_string){(const char *)inbox + 4, length - 4}); has_player = true; }
    return;
  }
  if (room_id >= 0 && kind == 512 + room_id) {
    room_id = -1;
    int64_t st = thegrid_room_answer(at(state), (coax_string){(const char *)inbox, length});
    if (st == 403 && has_player) { has_player = false; mint_tried = false; }   // the key was refused: a new one
    return;
  }
  if (edit_face_message(ed(state), kind, (coax_string){(const char *)inbox, length}) && kind != 600) { edit_face_drain(ed(state)); return; }
  if (post_id >= 0 && kind == 512 + post_id) {
    post_id = -1;
    thegrid_post_answer(at(state), (coax_string){(const char *)inbox, length});
    return;
  }
  bool diving = thegrid_diving(at(state));
  thegrid_message(at(state), kind, (coax_string){(const char *)inbox, length});
  if (!diving && thegrid_diving(at(state))) {   // from the homepage: the start spot, whatever was kept
    player = verse_player_place(0, 0, 0, 0);
    thegrid_spawn(at(state));
  }
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) track_drain(&track_state);
}

// A new player key goes first (action 5: host.js keeps the outbox as the browser's player).
static bool keeping;
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) {
  if (grid(state)->keep_player) { grid(state)->keep_player = false; keeping = true; return 5; }
  int32_t a = (int32_t)thegrid_action(at(state)); return a ? a : place_action();
}
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return keeping ? (uint32_t)(uintptr_t)grid(state)->player_out.e : place_out_now ? place_outbox() : (uint32_t)(uintptr_t)grid(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return keeping ? (uint32_t)grid(state)->player_out_n : place_out_now ? place_outbox_len() : (uint32_t)grid(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { if (keeping) keeping = false; else if (place_out_now) place_done(); else thegrid_action_done(at(state)); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return thegrid_wants_text(at(state)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)grid(state)->field.e; }

EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  if (i == 5) return bytes <= CAST_CAP ? (uint32_t)(uintptr_t)cast_bytes : 0u;
  return i == 0 && bytes <= sizeof data ? (uint32_t)(uintptr_t)data : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  start(state);
  if (i == 0) {
    if (bytes > 0 && bytes <= sizeof data) thegrid_data(at(state), (coax_string){(const char *)data, bytes});
    else thegrid_arrived(at(state), false);
  } else if (i == 5) {
    // Cast.bin: admitted whole (verse_cast_open checks every buffer, index and joint), or nobody draws
    cast_n = bytes <= CAST_CAP ? bytes : 0;
    cast_set = verse_cast_open((coax_v_u8){cast_bytes, cast_n});
    cast_open = cast_n > 0 && cast_set.ok && cast_set.joints <= JOINTS_MAX;
    thegrid_cast(at(state), cast_open, cast_set.body_vertices.offset / 4, cast_set.body_indices.offset / 4, cast_set.body_indices.count, cast_set.joints);
  } else if (i >= 1 && i <= 7) thegrid_arrived(at(state), bytes > 0);
}
