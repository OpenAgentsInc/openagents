// Everglade v2's C face for the browser (coax/web/host.js): scalar Wasm exports over everglade2.coax,
// which says what is round you, where you may stand and what is drawn. It decides nothing.
// The controller steps your body as fivelab-face.c's Player mode does: everglade-step.coax
// (aethyr/walk.coax, with player.coax's verse_player_*) on the keys and a phone's thumbs
// (aethyr/sticks.coax). The scene is three stores: the records (the glade, its grass, your
// character's parts), the clips baked once (into the state, sized by fivelab-bodies.coax), and the
// kit pieces in view this frame.
#include "everglade2.h"
#include "Everglade-step.h"
#include "Sticks.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
static uint8_t catalog[65536];
static coax_t_People rig;
static uint32_t baked;
static uint32_t frames;
static int64_t draw_n;
static coax_t_Player player;
static coax_t_Sticks sticks;
static float width, height, dpr;
static double now;

static coax_t_E2 *e2_at(uint32_t state) { return (coax_t_E2 *)(uintptr_t)state; }
static coax_v_t_E2 at(uint32_t state) { return (coax_v_t_E2){e2_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_E2) * 1000003u + offsetof(coax_t_E2, inst) * 13u + 0x45324532u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_E2); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_E2((coax_t_E2 *)(uintptr_t)state); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_e2_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_e2_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_e2_uniform_floats(); }
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) { return 6400000u; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  width = w; height = h; dpr = scale; now += dt;
  if (e2_start(at(state))) player = verse_player_place(e2_at(state)->you_x, 0, e2_at(state)->you_z, e2_at(state)->you_face);
  float frame = dt;
  float step = verse_player_counts(player, frame);
  e2_look(at(state), sticks.dyaw, sticks.dpitch, sticks.zoom);
  coax_t_EgMoveIn input = e2_move_in(at(state));
  input.pad_x = sticks.pad_x; input.pad_y = sticks.pad_y;
  input.run = verse_runs(input.run, sticks.run, sticks.pad_x, sticks.pad_y);
  input.jump = input.jump || sticks.jump;
  coax_t_EgMoveWorld mw = e2_move_world(at(state), player.pos.x, player.pos.z);
  mw.pace = e2_pace(input.run);
  player = eg_step_begin(player, input, mw, step);
  coax_v_f32 near = e2_move_bodies(at(state));
  player = eg_step_advance(player, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){0, 0}, (coax_v_f32){0, 0}, frame);
  coax_t_Point kept = e2_keep(at(state), player.pos.x, player.pos.z);
  if (kept.x != player.pos.x || kept.z != player.pos.z) player = verse_player_constrain(player, kept.x, kept.z);
  coax_t_Vec3 feet = verse_player_feet(player);
  e2_you(at(state), feet.x, feet.y, feet.z, verse_player_face(player));
  sticks = verse_sticks_used(sticks);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_e2_max_vertices() * coaxial_e2_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_e2_uniform_floats()};
  int64_t n = e2_frame(at(state), out, u, w, h, scale, dt, still != 0);
  draw_n = 0;
  if (e2_at(state)->bodies.e[0].ready) {
    if (!baked) { e2_bake(at(state), (coax_v_t_People){&rig, 1}); baked = 1; }
    draw_n = e2_scene(at(state));
  }
  frames += 1;
  return n < 0 ? 0u : (uint32_t)n;
}

// ---- the pack's catalog; the draws and the separate shadow list over the three stores
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state; return i == 0 && bytes <= sizeof catalog ? (uint32_t)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) { (void)state; return i == 0 ? (uint32_t)sizeof catalog : 0u; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) e2_catalog(at(state), (coax_string){(const char *)catalog, bytes});
}
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)e2_at(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { (void)state; return draw_n > 0 ? (uint32_t)draw_n : 0u; }
EXPORT("coaxial_shadow_draws_v1") uint32_t coaxial_shadow_draws_v1(uint32_t state) { return (uint32_t)(uintptr_t)e2_at(state)->shadow.e; }
EXPORT("coaxial_shadow_draw_count_v1") uint32_t coaxial_shadow_draw_count_v1(uint32_t state) { return draw_n > 0 ? (uint32_t)e2_at(state)->shadow_n : 0u; }
EXPORT("coaxial_layers_side_v1") uint32_t coaxial_layers_side_v1(uint32_t state) { return (uint32_t)e2_layers_side(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 3; }
EXPORT("coaxial_frame_store_cap") uint32_t coaxial_frame_store_cap(void) { return (uint32_t)sizeof(((coax_t_E2 *)0)->frame.e); }
EXPORT("coaxial_frame_store") uint32_t coaxial_frame_store(uint32_t state) { return (uint32_t)(uintptr_t)e2_at(state)->frame.e; }
EXPORT("coaxial_frame_store_bytes") uint32_t coaxial_frame_store_bytes(uint32_t state) { (void)state; return (uint32_t)sizeof(((coax_t_E2 *)0)->frame.e); }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) {
  return k == 0 ? (uint32_t)sizeof(((coax_t_E2 *)0)->records.e) : k == 1 ? (uint32_t)sizeof(((coax_t_E2 *)0)->bake.e) : (uint32_t)sizeof(((coax_t_E2 *)0)->inst.e);
}
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  return k == 0 ? (uint32_t)(uintptr_t)e2_at(state)->records.e : k == 1 ? (uint32_t)(uintptr_t)e2_at(state)->bake.e : (uint32_t)(uintptr_t)e2_at(state)->inst.e;
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  if (k == 0) return (uint32_t)e2_record_bytes(at(state));
  if (k == 1) return (uint32_t)sizeof(((coax_t_E2 *)0)->bake.e);
  uint32_t n = (uint32_t)e2_inst_bytes(at(state)); return n ? n : 32u;
}
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { (void)state; return k == 1 ? baked : frames; }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { return e2_key(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return e2_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { e2_blur(at(state)); sticks = verse_sticks_reset(); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  if (e2_pointer(at(state), x, y, phase, kind, id)) return 1u;
  // a finger is a thumb's: the stick, the look, a tap's jump
  if (kind != 0) { sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now); return 1u; }
  return 0u;
}
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(uint32_t state, float dy) { return e2_wheel(at(state), dy) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { (void)state; return 0; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  e2_message(at(state), kind, (coax_string){(const char *)inbox, length});
}
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { (void)state; return 0; }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { (void)state; return 0u; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { (void)state; }

// Your saved character arrives as message 16 (the browser's own copy, from Character Create).
EXPORT("coaxial_player_appearance_v1") uint32_t coaxial_player_appearance_v1(void) { return 1u; }
EXPORT("coaxial_field_probe") int32_t coaxial_field_probe(uint32_t state, int32_t field) { return (int32_t)e2_probe(at(state), field); }
