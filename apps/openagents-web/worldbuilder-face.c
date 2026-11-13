// WorldBuilder's C face for the browser (coax/web/host.js): scalar Wasm exports over
// worldbuilder.coax, plus the atlas calls coax/web/gpui.js makes (coaxial_gpui_*). The host keeps
// one WorldBuilder; this file decides nothing: the frame, keys, the pointer and wheel, the inbox
// (typed text, the slot's save, the pack's progress, HTTP answers), the page's one request handed
// to the host's HTTP import, the save (action 4), the pack's catalog (asset 0) and the scene drawn
// from it.
// The page is also an Everglade zone (worldbuilder-walk.coax, everglade/zone.coax): your character
// walks the world on Everglade's controller, stepped here each frame in lab-walk-face.h's
// order, its figure's instances and the baked clips two more stores, its parts' draws after
// WorldBuilder's own on the people's index buffer (the page's second pack).
#include "worldbuilder-walk.h"
#include "Everglade-step.h"
#include "Sticks.h"
#include "face-address.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#ifndef COAXIAL_IMPORT
#define COAXIAL_IMPORT(name) __attribute__((import_module("env"), import_name(name)))
#endif
COAXIAL_IMPORT("coaxial_http") int32_t coaxial_http(cx_face_addr url, uint32_t url_len, cx_face_addr body, uint32_t body_len, int32_t flags);

static coax_t_WalkBuilder *walk_at(uint32_t state) { return (coax_t_WalkBuilder *)(uintptr_t)state; }
static coax_v_t_WalkBuilder walk(uint32_t state) { return (coax_v_t_WalkBuilder){walk_at(state), 1}; }
static coax_t_WorldBuilder *state_at(uint32_t state) { return &walk_at(state)->b.e[0]; }
static coax_v_t_WorldBuilder at(uint32_t state) { return (coax_v_t_WorldBuilder){state_at(state), 1}; }
static coax_t_EgZone *zone_at(uint32_t state) { return &walk_at(state)->z.e[0]; }
static coax_v_t_EgZone zone(uint32_t state) { return (coax_v_t_EgZone){zone_at(state), 1}; }
// The people's catalog; the rig the clips are baked from, and the bake (everglade/scene/bodies.coax BD_BAKE_FLOATS).
static uint8_t people_catalog[65536];
static coax_t_People rig;
static uint8_t rig_file[2097152]; static uint32_t rig_bytes;   /* the pack's rig entry (VRG1, #13306), when the pack carries one: the bake reads its sets first */
static float bake[6453720];
static float zone_strokes[250000 * 16];   // the zone's own strokes (everglade/zone.coax MAX_VERTICES, VERTEX_FLOATS): made, never drawn
static uint32_t baked, frames;
static int64_t figure_draws;
static coax_t_Player player;   // your body (metaverse-engine/move/walk.coax)
static coax_t_Sticks sticks;   // a phone's thumb (metaverse-engine/core/sticks.coax)
static double now;
static float width, height, dpr;
static coax_t_Drive drive;     // the agent driver's (metaverse-engine/agent/drive.coax): the zone holds a view of it
#define PEOPLE_CATALOG 10      /* the page's assets, in order: WorldBuilder's pack (0..3), the HUD's fonts, icons and licence (4..9), the people's pack (10..13) */
#define PEOPLE_INDEX 12
/* The page asks for 4096 pages of memory (metaverse-engine/render/adoption.coax render_memory_pages): the zone, the bake and the ground. */
static uint8_t inbox[1310720];   // a kept world comes back through it (worldbuilder.coax SAVE_CAP)
static uint8_t catalog[262144];

// After every call: the page's request, if it has one.
static void drain(uint32_t state) {
  coax_t_WorldBuilder *s = state_at(state);
  if (!s->req_ready) return;
  int32_t id = coaxial_http((cx_face_addr)(uintptr_t)s->req_url.e, (uint32_t)s->req_url_n,
                            (cx_face_addr)(uintptr_t)s->req_body.e, (uint32_t)s->req_body_n, (int32_t)s->req_flags);
  worldbuilder_requested(at(state), id);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_WalkBuilder) * 1000003u + offsetof(coax_t_WorldBuilder, ed) * 7u + offsetof(coax_t_WorldBuilder, outbox) * 13u + offsetof(coax_t_EgZone, outbox) * 17u + 0x57424c47u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_WalkBuilder); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_WalkBuilder(walk_at(state)); zone_drive_attach(zone(state), (coax_v_t_Drive){&drive, 1}); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)wbw_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)wbw_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)wbw_uniform_floats(); }

static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  worldbuilder_golden(at(state), golden);
  now += dt;
  width = w; height = h; dpr = scale;
  zone_quality_frame(zone(state), dt, w, h, scale, golden);
  // your body's step on Everglade's controller, in lab-walk-face.h's order; in the overview it stands
  coax_t_ZoneWalk zw = zone_walk(zone(state));
  if (zw.put) player = metaverse_player_place(zw.x, zw.y, zw.z, zw.face);
  if (zw.on && wbw_walks(walk(state))) {
    float frame = dt;
    float step = metaverse_player_counts(player, frame);
    coax_t_EgMoveIn input = zone_move_in(zone(state));
    input.pad_x = sticks.pad_x; input.pad_y = sticks.pad_y;
    input.run = metaverse_runs(input.run, sticks.run, sticks.pad_x, sticks.pad_y);
    input.jump = input.jump || sticks.jump;
    coax_t_EgMoveWorld mw = zone_move_world(zone(state), player.body.pos.x, player.body.pos.z);
    player.body = eg_step_begin(player.body, input, mw, step); player.cam.theta = input.theta;
    coax_v_f32 near = zone_move_bodies(zone(state));
    coax_v_f32 walls = zone_move_walls(zone(state));
    coax_v_f32 floors = zone_move_floors(zone(state));
    coax_t_EgBody origin = eg_step_body(player.body);
    coax_t_EgMotionIntent intent = eg_step_motion_intent(input, mw);
    coax_t_EgJump jump = zone_jump_plan(zone(state), origin, intent, player.body.jumps.waiting > 0);
    player.body = eg_step_advance_jump(player.body, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){walls.data, mw.walls * 6}, (coax_v_f32){floors.data, mw.floors * 10}, frame, zone_move_scale(zone(state)), jump);
    coax_t_EgBody kept = zone_keep_body(zone(state), origin, eg_step_body(player.body));
    if (kept.x != player.body.pos.x || kept.z != player.body.pos.z) player = metaverse_player_constrain(player, kept.x, kept.z);
    player.body = eg_step_put(player.body, kept);
    zone_rest_motion(zone(state), origin, kept, intent.walk, intent.jump);
    coax_t_Vec3 feet = metaverse_player_feet(player);
    zone_you(zone(state), feet.x, feet.y, feet.z, metaverse_player_face(player), !player.body.on_ground, false, sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  } else {
    sticks = metaverse_sticks_reset();
  }
  sticks = metaverse_sticks_used(sticks);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, wbw_max_vertices() * wbw_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, wbw_uniform_floats()};
  int64_t n = wbw_frame(walk(state), (coax_v_f32){zone_strokes, sizeof zone_strokes / sizeof zone_strokes[0]}, out, u, w, h, scale, dt, still != 0);
  figure_draws = 0;
  if (zone_at(state)->bodies.e[0].ready) {
    if (!baked) { zone_bake_from(zone(state), (coax_v_t_People){&rig, 1}, (coax_v_f32){bake, sizeof bake / sizeof bake[0]}, (coax_v_u8){rig_file, rig_bytes}); baked = 1; }
    figure_draws = zone_scene(zone(state), (coax_v_f32){zone_at(state)->records.e, sizeof zone_at(state)->records.e / sizeof(float)});
  }
  wbw_scene(walk(state), figure_draws);
  frames += 1;
  drain(state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  bool used = wbw_key(walk(state), key, mods); drain(state); return used ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return wbw_key_up(walk(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { wbw_blur(walk(state)); sticks = metaverse_sticks_reset(); }
EXPORT("coaxial_focus") void coaxial_focus(uint32_t state) { (void)state; }
static uint32_t pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) {
  bool used = wbw_pointer(walk(state), x, y, phase, kind, id, button);
  // the finger the zone says is the thumb's moves the stick (metaverse-engine/core/sticks.coax)
  if (zone_thumb(zone(state))) sticks = metaverse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  drain(state);
  return used ? 1u : 0u;
}
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) { return pointer(state, x, y, phase, kind, id, phase == 1 ? -1 : 0); }
EXPORT("coaxial_pointer_buttons_v1") uint32_t coaxial_pointer_buttons_v1(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) { return pointer(state, x, y, phase, kind, id, button); }
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(uint32_t state, float dy) { return wbw_wheel(walk(state), dy) ? 1u : 0u; }
/* The mouselook (metaverse-engine/cam/mouselook.coax): host.js holds the pointer when this says 2 and forwards the mouse's travel. */
EXPORT("coaxial_mouselook") int32_t coaxial_mouselook(uint32_t state) { return (int32_t)wbw_mouselook(walk(state)); }
EXPORT("coaxial_look_delta") void coaxial_look_delta(uint32_t state, float dx, float dy) { zone_look_delta(zone(state), dx, dy); }
EXPORT("coaxial_pointer_lock") void coaxial_pointer_lock(uint32_t state, int32_t status) { zone_pointer_lock(zone(state), status); }
/* Your Character Create character, when you have one (host.js message 16; everglade/zone.coax zone_message). */
EXPORT("coaxial_player_appearance_v1") uint32_t coaxial_player_appearance_v1(void) { return 1u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)worldbuilder_cursor(at(state)); }
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return worldbuilder_wants_text(at(state)) ? 1u : 0u; }
// The typed line, in device pixels: x0, y0, x1, y1.
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->field.e; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  wbw_message(walk(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
  drain(state);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)worldbuilder_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { worldbuilder_action_done(at(state)); }

/* the pack's rig entry (kind 7), handed over before the catalog (coax/web/host.js deliverRig): the bake reads it */
EXPORT("coaxial_pack_rig") uint32_t coaxial_pack_rig(uint32_t state, uint32_t bytes) { (void)state; return bytes <= sizeof rig_file ? (uint32_t)(uintptr_t)rig_file : 0u; }
EXPORT("coaxial_pack_rig_capacity") uint32_t coaxial_pack_rig_capacity(uint32_t state) { (void)state; return (uint32_t)sizeof rig_file; }
EXPORT("coaxial_pack_rig_ready") void coaxial_pack_rig_ready(uint32_t state, uint32_t bytes) { (void)state; rig_bytes = bytes <= sizeof rig_file ? bytes : 0u; }
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  if (i == PEOPLE_CATALOG) return bytes <= sizeof people_catalog ? (uint32_t)(uintptr_t)people_catalog : 0u;
  return i == 0 && bytes <= sizeof catalog ? (uint32_t)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) {
  (void)state; return i == 0 ? (uint32_t)sizeof catalog : i == PEOPLE_CATALOG ? (uint32_t)sizeof people_catalog : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) worldbuilder_catalog(at(state), (coax_string){(const char *)catalog, (int64_t)bytes});
  if (i >= 0 && i <= 3 && bytes == 0) worldbuilder_pack_failed(at(state));
  if (i == PEOPLE_CATALOG && bytes > 0 && bytes <= sizeof people_catalog) zone_catalog(zone(state), (coax_string){(const char *)people_catalog, (int64_t)bytes});
}
/* The people's index buffer is an index asset a draw may name (host.js coaxial_draw_index_v1). */
EXPORT("coaxial_asset_index_v1") uint32_t coaxial_asset_index_v1(int32_t i) { return i == PEOPLE_INDEX; }
EXPORT("coaxial_draw_index_v1") int32_t coaxial_draw_index_v1(uint32_t state, uint32_t i, uint32_t shadow, uint32_t field) { (void)shadow; return (int32_t)wbw_draw_index(walk(state), (int64_t)i, (int64_t)field); }
/* The figure's draws blend over the scene as a ghost (host.js coaxial_draw_blend_v1: 0 opaque, 2 source over). */
EXPORT("coaxial_draw_blend_v1") uint32_t coaxial_draw_blend_v1(uint32_t state, int32_t i) { return (uint32_t)wbw_draw_blend(walk(state), (int64_t)i); }
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)walk_at(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)walk_at(state)->draw_n; }
// Two stores: the instances' rows, and the ground's samples (worldbuilder-land.coax wb_land_gpu). Each
// is sent again only when its version changes.
// Four stores: the instances' rows, the ground's samples (worldbuilder-land.coax wb_land_gpu), the
// figure's instances (the zone's records) and the baked clips. Each is sent again only when its version changes.
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 4u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) {
  return k == 0 ? (uint32_t)sizeof(((coax_t_WorldBuilder *)0)->objects) : k == 1 ? (uint32_t)sizeof(((coax_t_WorldBuilder *)0)->ground)
       : k == 2 ? (uint32_t)(4096u * 24u * 4u) : (uint32_t)sizeof bake;
}
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  return k == 0 ? (uint32_t)(uintptr_t)state_at(state)->objects.e : k == 1 ? (uint32_t)(uintptr_t)state_at(state)->ground.e
       : k == 2 ? (uint32_t)(uintptr_t)zone_at(state)->records.e : (uint32_t)(uintptr_t)bake;
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  if (k == 0) return (uint32_t)worldbuilder_store_bytes(at(state));
  if (k == 1) return (uint32_t)worldbuilder_ground_bytes(at(state));
  if (k == 2) { uint32_t n = 0; coax_t_Bodies *b = &zone_at(state)->bodies.e[0]; for (int64_t d = 0; d < b->draw_n && d < figure_draws; d++) n += b->draws.e[d * 4 + 3]; if (n > 4096u) n = 4096u; return n * 24u * 4u; }
  return (uint32_t)sizeof bake;
}
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) {
  return k == 0 ? (uint32_t)worldbuilder_scene_version(at(state)) : k == 1 ? (uint32_t)worldbuilder_ground_version(at(state)) : k == 2 ? frames : baked;
}
// The store is copied only when the world changed (its version), not every frame.

// The atlas (coax/web/gpui.js): what to rasterize, then where it went.
EXPORT("coaxial_gpui_glyphs") int32_t gpui_glyphs(void) { return (int32_t)worldbuilder_glyph_count(); }
EXPORT("coaxial_gpui_glyph_code") int32_t gpui_glyph_code(int32_t g) { return (int32_t)worldbuilder_glyph_code(g); }
EXPORT("coaxial_gpui_fonts") int32_t gpui_fonts(void) { return (int32_t)worldbuilder_font_count(); }
EXPORT("coaxial_gpui_font_size") float gpui_font_size(int32_t k) { return worldbuilder_font_size(k); }
EXPORT("coaxial_gpui_font_weight") int32_t gpui_font_weight(int32_t k) { return (int32_t)worldbuilder_font_weight(k); }
EXPORT("coaxial_gpui_icons") int32_t gpui_icons(void) { return (int32_t)worldbuilder_icon_count(); }
EXPORT("coaxial_gpui_icon_size") float gpui_icon_size(int32_t i) { return worldbuilder_icon_size(i); }
EXPORT("coaxial_gpui_icon_name") int32_t gpui_icon_name(uint32_t state, int32_t i) { return (int32_t)worldbuilder_icon_name(at(state), i); }
EXPORT("coaxial_gpui_scratch") uint32_t gpui_scratch(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->scratch.e; }
EXPORT("coaxial_gpui_bytes") uint32_t gpui_bytes(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->sbytes.e; }
EXPORT("coaxial_gpui_set_font") void gpui_set_font(uint32_t state, int32_t slot) { worldbuilder_set_font(at(state), slot); }
EXPORT("coaxial_gpui_set_icon") void gpui_set_icon(uint32_t state, int32_t i, float x, float y, float w, float h) { worldbuilder_set_icon(at(state), i, x, y, w, h); }
EXPORT("coaxial_gpui_set_atlas") void gpui_set_atlas(uint32_t state, float w, float h, int32_t levels) { worldbuilder_set_atlas(at(state), w, h, levels); }
EXPORT("coaxial_gpui_set_dynamic") void gpui_set_dynamic(uint32_t state, float size, float pen_x, float pen_y, float y0, int32_t cols, int32_t color_cols, float color_w, float color_h) {
  worldbuilder_set_dynamic(at(state), size, pen_x, pen_y, y0, cols, color_cols, color_w, color_h);
}
EXPORT("coaxial_gpui_next_cluster") int32_t gpui_next_cluster(uint32_t state) { return (int32_t)worldbuilder_next_cluster(at(state)); }
EXPORT("coaxial_gpui_cluster") int32_t gpui_cluster(uint32_t state, int32_t slot) { return (int32_t)worldbuilder_cluster(at(state), slot); }
EXPORT("coaxial_gpui_measured") void gpui_measured(uint32_t state, int32_t slot, float advance, int32_t color) { worldbuilder_measured(at(state), slot, advance, color != 0); }
EXPORT("coaxial_gpui_next_cell") int32_t gpui_next_cell(uint32_t state) { return (int32_t)worldbuilder_next_cell(at(state)); }
EXPORT("coaxial_gpui_cell") int32_t gpui_cell(uint32_t state, int32_t cell) { return worldbuilder_cell(at(state), cell) ? 1 : 0; }
EXPORT("coaxial_gpui_cell_ready") void gpui_cell_ready(uint32_t state, int32_t cell) { worldbuilder_cell_ready(at(state), cell); }
