// The Grid's C face for the browser (coax/web/host.js): thegrid.coax is the
// page, verse/player.coax walks you (its own library, as the realm links
// it) and verse/grid.coax reads a phone's thumbs. Once a frame it passes
// values between them in order: the page's controls, the player's step
// among the stations' walls, then the page's frame with where you ended up.
// grid.data (asset 0) goes to the page; the other four assets are the GPU's
// and the page is only told they came. Where you are is kept so a reload
// puts you back (place-face.h). It decides nothing. Every pointer is
// an offset into the host's WebAssembly.Memory; the page's state is the
// host's (coaxial_state_size).
#include "thegrid.h"
#include "Player.h"
#include "Grid.h"
#include "analytics-face.h"
#include "edit-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Player player;
static coax_t_Sticks sticks;
static bool placed, golden;
static double now;
static float width, height, dpr;
static uint8_t inbox[4096];
static uint8_t data[65536];
static coax_t_Track track_state;
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
  edit_face_drain(ed(state));
  place_frame(state);
  track_frame(tr(), dt, w, h, scale);
  if (n > 0 && thegrid_draws(at(state)) > 0) track_loaded(tr(), track_cstr("load-the-grid"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
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
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 1; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { (void)k; return (uint32_t)sizeof(((coax_t_TheGrid *)0)->robots); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)k; return (uint32_t)(uintptr_t)grid(state)->robots.e; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { (void)k; return (uint32_t)thegrid_store_bytes(at(state)); }

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
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)thegrid_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  start(state);
  if (place_message(state, kind, inbox, length)) return;
  if (edit_face_message(ed(state), kind, (coax_string){(const char *)inbox, length}) && kind != 600) { edit_face_drain(ed(state)); return; }
  if (post_id >= 0 && kind == 512 + post_id) {
    post_id = -1;
    thegrid_post_answer(at(state), (coax_string){(const char *)inbox, length});
    return;
  }
  thegrid_message(at(state), kind, (coax_string){(const char *)inbox, length});
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, length})) track_drain(&track_state);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { int32_t a = (int32_t)thegrid_action(at(state)); return a ? a : place_action(); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return place_out_now ? place_outbox() : (uint32_t)(uintptr_t)grid(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return place_out_now ? place_outbox_len() : (uint32_t)grid(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { if (place_out_now) place_done(); else thegrid_action_done(at(state)); }
EXPORT("coaxial_wants_text") int32_t coaxial_wants_text(uint32_t state) { return thegrid_wants_text(at(state)) ? 1 : 0; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)grid(state)->field.e; }

EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof data ? (uint32_t)(uintptr_t)data : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  start(state);
  if (i == 0) {
    if (bytes > 0 && bytes <= sizeof data) thegrid_data(at(state), (coax_string){(const char *)data, bytes});
    else thegrid_arrived(at(state), false);
  } else if (i >= 1 && i <= 4) thegrid_arrived(at(state), bytes > 0);
}
