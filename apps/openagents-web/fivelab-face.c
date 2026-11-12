// 5E Lab's C face for the browser (coax/web/host.js): scalar Wasm exports
// over fivelab.coax, which holds the battle on the SRD engine
// (aethyr/rules/srd/) and its drawing. It decides nothing.
// gpu/fivelab.coax's bg_fs draws the grid, the glade and the log's glass, writing the glade's
// depth; scene_vs/scene_fs the figures (Everglade's pack: its catalog here, its vertices, indices
// and layers on the GPU), from two stores: the frame's instances (everglade/scene/bodies.coax bd_build)
// and the clips baked once (bd_bake); fx_fs the strokes lab_frame writes, and fivelab-fx.wgsl the
// glade's effects (puffs), drawn after the figures, depth-tested.
// Player mode walks your hero on Everglade's controller, as everglade-face.c does: everglade-step.coax
// (aethyr/walk.coax, with player.coax's verse_player_*) steps your body on the keys and a phone's
// thumbs (aethyr/sticks.coax) among the figures round you (lab_move_world), and the Lab says where
// it may stand (lab_keep) and draws it (lab_you).
#include "fivelab.h"
#include "Everglade-step.h"
#include "Sticks.h"
#include "face-address.h"
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
static bool golden;
// The pack's catalog; the rig the clips are baked from, and the bake. The frame's instances (5 MB at
// the most figures) are the Lab's records, in its state: the module's own memory stays under host.js's 8 MiB start.
static uint8_t catalog[65536];
static coax_t_People rig;
static float bake[6453720];        // everglade/scene/bodies.coax BD_BAKE_FLOATS
static uint32_t baked;            // the bake's generation: 1 once baked
static uint32_t frames;
static int64_t draw_n;
static coax_t_Player player;   // your body in Player mode (aethyr/walk.coax)
static coax_t_Sticks sticks;   // a phone's thumbs
static float width, height, dpr;
static double now;
static bool touched;
static coax_t_Drive drive;   // the agent driver's (aethyr/agent/drive.coax): the Lab holds a view of it

static coax_t_Lab *lab_at(cx_face_addr state) { return (coax_t_Lab *)(uintptr_t)state; }
static coax_v_t_Lab at(cx_face_addr state) { return (coax_v_t_Lab){lab_at(state), 1}; }
// The zone (everglade/zone.coax): Experimental's world, your body, the keys held and the figures' stores.
static coax_t_EgZone *zone_at(cx_face_addr state) { return &lab_at(state)->w.e[0]; }
static coax_v_t_EgZone zone(cx_face_addr state) { return (coax_v_t_EgZone){zone_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Lab) * 1000003u + offsetof(coax_t_Lab, w) * 13u + 0x35454c42u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Lab); }
EXPORT("coaxial_state_init") void coaxial_state_init(cx_face_addr state) { coax_default_t_Lab((coax_t_Lab *)(uintptr_t)state); zone_drive_attach(zone(state), (coax_v_t_Drive){&drive, 1}); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_lab_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_lab_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_lab_uniform_floats(); }
// bg_fs traces the glade's ground, trees and sky per pixel (the figures are drawn, not traced):
// drawn at most 6.4 million pixels (3200 by 2000; the host scales the canvas up past that).
// The graphics governor's (everglade/zone.coax zone_pixel_budget, aethyr/quality-page.coax): fewer while frames run long.
static cx_face_addr quality_state;
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) {
  if (!quality_state) return 6400000u;
  int64_t px = lab_pixel_budget((coax_v_t_Lab){(coax_t_Lab *)(uintptr_t)quality_state, 1}, width, height, dpr);
  return px > 0 ? (uint32_t)px : 6400000u;
}
// The experimental meadow and castle share one large geometry storage buffer.
EXPORT("coaxial_character_storage_v1") uint32_t coaxial_character_storage_v1(void) { return 256u * 1024u * 1024u; }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(cx_face_addr state, cx_face_addr vertices, cx_face_addr uniforms, float w, float h, float scale, float dt, uint32_t still) {
  lab_golden(at(state), golden);
  width = w; height = h; dpr = scale; now += dt;
  quality_state = state; lab_quality_frame(at(state), dt, w, h, scale, golden);
  // Player mode: your body's step on Everglade's controller, in everglade-face.c's order
  coax_t_ZoneWalk walk = lab_walk(at(state));
  if (walk.put) player = verse_player_place(walk.x, walk.y, walk.z, walk.face);
  if (walk.on) {
    float frame = dt;
    float step = verse_player_counts(player, frame);
    lab_look(at(state), sticks.dyaw, sticks.dpitch, sticks.zoom);
    coax_t_EgMoveIn input = lab_move_in(at(state));
    input.pad_x = sticks.pad_x; input.pad_y = sticks.pad_y;
    input.run = verse_runs(input.run, sticks.run, sticks.pad_x, sticks.pad_y);
    input.jump = input.jump || sticks.jump;
    coax_t_EgMoveWorld mw = lab_move_world(at(state), player.pos.x, player.pos.z);
    player = eg_step_begin(player, input, mw, step);
    coax_v_f32 near = lab_move_bodies(at(state));
    coax_v_f32 walls = lab_move_walls(at(state));
    coax_v_f32 floors = lab_move_floors(at(state));
    coax_t_EgBody origin = eg_step_body(player);
    coax_t_EgMotionIntent intent = eg_step_motion_intent(input, mw);
    coax_t_EgJump jump = lab_jump_plan(at(state), origin, intent, player.jumps.waiting > 0);
    player = eg_step_advance_jump(player, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){walls.data, mw.walls * 6}, (coax_v_f32){floors.data, mw.floors * 10}, frame, lab_move_scale(at(state)), jump);
    coax_t_EgBody kept = lab_keep_body(at(state), origin, eg_step_body(player));
    if (kept.x != player.pos.x || kept.z != player.pos.z) player = verse_player_constrain(player, kept.x, kept.z);
    player = eg_step_put(player, kept);
    lab_rest_motion(at(state), origin, kept, intent.walk, intent.jump);
    coax_t_Vec3 feet = verse_player_feet(player);
    lab_you(at(state), feet.x, feet.y, feet.z, verse_player_face(player), !player.on_ground, touched, sticks.move_on, sticks.ox, sticks.oy, sticks.pad_x, sticks.pad_y);
  } else {
    sticks = verse_sticks_reset();
  }
  sticks = verse_sticks_used(sticks);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_lab_max_vertices() * coaxial_lab_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_lab_uniform_floats()};
  int64_t n = lab_frame(at(state), out, u, w, h, scale, dt, still != 0);
  draw_n = 0;
  if (zone_at(state)->bodies.e[0].ready) {
    if (!baked) { zone_bake(zone(state), (coax_v_t_People){&rig, 1}, (coax_v_f32){bake, sizeof bake / sizeof bake[0]}); baked = 1; }
    draw_n = lab_scene(at(state), (coax_v_f32){zone_at(state)->records.e, sizeof zone_at(state)->records.e / sizeof(float)});
  }
  frames += 1;
  return n < 0 ? 0u : (uint32_t)n;
}

// ---- the figures: the pack's catalog, and the scene's draws over its vertices and the two stores
EXPORT("coaxial_asset") cx_face_addr coaxial_asset(cx_face_addr state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof catalog ? (cx_face_addr)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(cx_face_addr state, int32_t i) {
  (void)state;return i == 0 ? (uint32_t)sizeof catalog : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(cx_face_addr state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) lab_catalog(at(state), (coax_string){(const char *)catalog, bytes});
}
// The glade's environment mesh's ask for its runs' boxes (aethyr/render/mesh-cull.coax, #13168): the host writes them once the pack is in.
EXPORT("coaxial_mesh_bounds_v1") cx_face_addr coaxial_mesh_bounds_v1(cx_face_addr state) { return zone_at(state)->glade_cull.e[0].ask.e[0] ? (cx_face_addr)(uintptr_t)&zone_at(state)->glade_cull.e[0] : 0u; }
EXPORT("coaxial_draws") cx_face_addr coaxial_draws(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->bodies.e[0].draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(cx_face_addr state) { (void)state; return draw_n > 0 ? (uint32_t)draw_n : 0u; }
EXPORT("coaxial_shadow_draws_v1") cx_face_addr coaxial_shadow_draws_v1(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->shadow_draws.e; }
EXPORT("coaxial_shadow_draw_count_v1") uint32_t coaxial_shadow_draw_count_v1(cx_face_addr state) { return (uint32_t)zone_at(state)->shadow_n; }
/* Character Create's key-light shadow tiles (charactercreator.coax shadow_frame; coax/web/host.js shadowTexture): none elsewhere. */
EXPORT("coaxial_shadow_atlas_v1") uint32_t coaxial_shadow_atlas_v1(cx_face_addr state, int32_t coarse, int32_t which) { return (uint32_t)zone_shadow_atlas(zone(state), coarse, which); }
EXPORT("coaxial_shadow_v1") int32_t coaxial_shadow_v1(cx_face_addr state, int32_t field) { return (int32_t)zone_shadow_field(zone(state), field); }
EXPORT("coaxial_shadow_event_v1") void coaxial_shadow_event_v1(cx_face_addr state, int32_t kind, int32_t arg) { zone_shadow_event(zone(state), kind, arg); }
EXPORT("coaxial_layers_side_v1") uint32_t coaxial_layers_side_v1(cx_face_addr state) { return (uint32_t)zone_layers_side(zone(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
// The frame store (host.js, group 0 binding 1): the tokens and squares bg_fs reads, every frame.
EXPORT("coaxial_frame_store_cap") uint32_t coaxial_frame_store_cap(void) { return (uint32_t)sizeof(((coax_t_Lab *)0)->frame.e); }
EXPORT("coaxial_frame_store") cx_face_addr coaxial_frame_store(cx_face_addr state) { return (cx_face_addr)(uintptr_t)lab_at(state)->frame.e; }
EXPORT("coaxial_frame_store_bytes") uint32_t coaxial_frame_store_bytes(cx_face_addr state) { return (uint32_t)coaxial_lab_frame_floats(at(state)) * 4u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 0 ? (uint32_t)sizeof(((coax_t_EgZone *)0)->records.e) : (uint32_t)sizeof bake; }
EXPORT("coaxial_store") cx_face_addr coaxial_store(cx_face_addr state, int32_t k) { return k == 0 ? (cx_face_addr)(uintptr_t)zone_at(state)->records.e : (cx_face_addr)(uintptr_t)bake; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(cx_face_addr state, int32_t k) {
  (void)state;
  if (k == 0) { uint32_t n = 0; coax_t_Bodies *b = &zone_at(state)->bodies.e[0]; for (int64_t d = 0; d < b->draw_n; d++) n += b->draws.e[d * 4 + 3]; return n * 24u * 4u; }
  return (uint32_t)sizeof bake;
}
// The instances every frame; the bake once, when it is made.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(cx_face_addr state, int32_t k) { (void)state; return k == 0 ? frames : baked; }

EXPORT("coaxial_key") uint32_t coaxial_key(cx_face_addr state, int32_t key, int32_t mods) { return lab_key(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(cx_face_addr state, int32_t key, int32_t mods) { return lab_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(cx_face_addr state) { lab_blur(at(state)); sticks = verse_sticks_reset(); }
/* Aethyr host contract v1 (aethyr/host/README.md), as everglade2-face.c has it: addresses of the
   host's width (face-address.h), focus as a call of its own, and the whole allocations the action,
   media and sound-bank bytes are in. A native build says so; the browser's module does not yet
   (there v1 also means the shared transport adapter a page gets with its render carrier). */
#if defined(COAXIAL_NATIVE_ADDRESS)
EXPORT("coaxial_host_contract_version") uint32_t coaxial_host_contract_version(void) { return 1; }
#endif
/* Nothing is held across a blur, so focus has nothing to restore. */
EXPORT("coaxial_focus") void coaxial_focus(cx_face_addr state) { (void)state; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(cx_face_addr state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  if (lab_pointer(at(state), x, y, phase, kind, id)) {
    // in a zone's world, the finger the zone says is the thumb's moves the stick (everglade/zone-touch.coax)
    if (zone_thumb(zone(state))) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
    return 1u;
  }
  // a finger the Lab left (Player mode, off its buttons) is a thumb's: the stick, the look, a tap's jump
  if (kind != 0 && lab_at(state)->player) { touched = true; sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now); return 1u; }
  return 0u;
}
// Additive DOM input hook; the original pointer export remains available to native hosts.
EXPORT("coaxial_pointer_buttons_v1") uint32_t coaxial_pointer_buttons_v1(cx_face_addr state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) {
  if (lab_pointer_buttons(at(state), x, y, phase, kind, id, button)) {
    if (zone_thumb(zone(state))) sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
    return 1u;
  }
  if (kind != 0 && lab_at(state)->player) { touched = true; sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now); return 1u; }
  return 0u;
}
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(cx_face_addr state, float dy) { return lab_wheel(at(state), dy) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(cx_face_addr state) { return (int32_t)zone_cursor(zone(state)); }
/* Experimental's mouselook (aethyr/cam/mouselook.coax): host.js holds the pointer when this says 2 and forwards the mouse's travel. */
EXPORT("coaxial_mouselook") int32_t coaxial_mouselook(cx_face_addr state) { return (int32_t)zone_mouselook(zone(state)); }
EXPORT("coaxial_look_delta") void coaxial_look_delta(cx_face_addr state, float dx, float dy) { zone_look_delta(zone(state), dx, dy); }
EXPORT("coaxial_pointer_lock") void coaxial_pointer_lock(cx_face_addr state, int32_t status) { zone_pointer_lock(zone(state), status); }
/* A phone's turn as look and aim (aethyr/cam/gyro.coax, #13189): host.js from devicemotion, the page host from CoreMotion. */
EXPORT("coaxial_tilt") void coaxial_tilt(cx_face_addr state, float dyaw, float dpitch) { zone_tilt(zone(state), dyaw, dpitch); }
EXPORT("coaxial_gyro") int32_t coaxial_gyro(cx_face_addr state) { return (int32_t)zone_gyro(zone(state)); }
EXPORT("coaxial_gyro_status") void coaxial_gyro_status(cx_face_addr state, int32_t status) { zone_gyro_status(zone(state), status); }

// ---- the glade's effects, drawn after bg_fs (fivelab-fx.wgsl, host.js particles): puffs of 12 floats
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_EgZone *)0)->puffs.e[0].out); }
EXPORT("coaxial_particles") cx_face_addr coaxial_particles(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->puffs.e[0].out.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(cx_face_addr state) { return (uint32_t)(zone_at(state)->puffs.e[0].out_n * 12 * sizeof(float)); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(cx_face_addr state) { return lab_at(state)->started ? (uint32_t)zone_at(state)->puffs.e[0].out_n : 0u; }

EXPORT("coaxial_inbox") cx_face_addr coaxial_inbox(cx_face_addr state) { (void)state; return (cx_face_addr)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
// Everglade Experimental draws its HUD in the FANTASY style (aethyr/ui/hud-style.coax): its strokes then
// blend straight alpha, sealed with their coverage (lab_straight_alpha). The 5E Lab keeps max blending.
static cx_face_addr page_state;
EXPORT("coaxial_fx_alpha_v1") uint32_t coaxial_fx_alpha_v1(void) { return page_state && zone_straight_alpha(zone(page_state)) ? 1u : 0u; }
EXPORT("coaxial_message") void coaxial_message(cx_face_addr state, int32_t kind, uint32_t length) {
  page_state = state;
  if (length > sizeof inbox) return;
  lab_message(at(state), kind, (coax_string){(const char *)inbox, length});
}

EXPORT("coaxial_action") int32_t coaxial_action(cx_face_addr state) { return (int32_t)zone_action(zone(state)); }
EXPORT("coaxial_outbox") cx_face_addr coaxial_outbox(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(cx_face_addr state) { return (uint32_t)zone_at(state)->out_n; }
/* the allocations those bytes and the media packet's are in, whole (host contract v1) */
EXPORT("coaxial_outbox_capacity") uint32_t coaxial_outbox_capacity(cx_face_addr state) { return (uint32_t)sizeof zone_at(state)->outbox.e; }
EXPORT("coaxial_action_done") void coaxial_action_done(cx_face_addr state) { zone_action_done(zone(state)); }

// Experimental's bar: its pictures, cards and cooldowns for the host to draw (coax/web/host.js coaxial_media; experimental-combat.coax xc_frame).
EXPORT("coaxial_media") cx_face_addr coaxial_media(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->xc.e[0].media.e; }
EXPORT("coaxial_media_len") uint32_t coaxial_media_len(cx_face_addr state) { coax_t_EgZone *l = zone_at(state); return l->creator.e[0].experimental && !l->creator.e[0].walk.on ? (uint32_t)l->xc.e[0].media_n : 0u; }
EXPORT("coaxial_media_capacity") uint32_t coaxial_media_capacity(cx_face_addr state) { return (uint32_t)sizeof zone_at(state)->xc.e[0].media.e; }

// Read-only observations for headless wardrobe checks.
/* The agent driver (aethyr/agent/drive.coax, coax/web/host.js coaxial.drive): the answer to the last command (message 700). */
EXPORT("coaxial_drive_out") cx_face_addr coaxial_drive_out(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_drive_out(zone(state)).data; }
EXPORT("coaxial_drive_out_len") uint32_t coaxial_drive_out_len(cx_face_addr state) { return (uint32_t)zone_drive_out(zone(state)).count; }
EXPORT("coaxial_field_probe") int32_t coaxial_field_probe(cx_face_addr state,int32_t field) {return (int32_t)zone_probe(zone(state),field);}

EXPORT("coaxial_player_appearance_v1") uint32_t coaxial_player_appearance_v1(void) { return 1u; }

// Experimental's per-frame audio commands, consumed once by the shared host.
EXPORT("coaxial_sounds") cx_face_addr coaxial_sounds(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->experimental_audio.e[0].lines.e; }
EXPORT("coaxial_sounds_capacity") uint32_t coaxial_sounds_capacity(cx_face_addr state) { return sizeof zone_at(state)->experimental_audio.e[0].lines.e; }
EXPORT("coaxial_sounds_len") uint32_t coaxial_sounds_len(cx_face_addr state) { coax_t_ExperimentalSound *audio = &zone_at(state)->experimental_audio.e[0]; uint32_t n = (uint32_t)audio->n; audio->n = 0; return n; }

#include "experimental-sound-bank.h"
