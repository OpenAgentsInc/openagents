// A lab walk's C face (metaverse/lab-walk.coax) for the browser (coax/web/host.js) and the Mac and iPhone
// apps' page host (metaverse-engine/host/apple): scalar exports over the world's module. It decides nothing. The
// module is an Everglade zone (everglade/zone.coax), as the Archery Range's is (archery-range-face.c,
// whose shape this follows): nearly every export hands the host's call to the zone, and the frame, the
// pointer and the page's message go through the world (LAB_FRAME, LAB_POINTER, LAB_MESSAGE). The scene
// draws your figure with Character Create's passes (everglade/scene/gpu/charactercreator.coax); the Grid, its sky and
// the stations are the walk's quads, drawn after it as the page's particles by bitcoin-lab-fx.wgsl
// (metaverse-engine/fx/gpu-quads.coax), 16 floats a quad.
//
// A world's face (bitcoin-lab-walk-face.c, nostr-lab-walk-face.c, coax-lab-walk-face.c) includes its module's header, names
// these and includes this file once:
//   LAB_STATE            the world's state type, holding `w: [1] LabWalk`
//   LAB_VIEW             a view of one
//   LAB_DEFAULT          its default constructor
//   LAB_FRAME, LAB_POINTER, LAB_POINTER_BUTTONS, LAB_MESSAGE   the world's exports
//   LAB_TAG              four letters for its layout's hash
//   LAB_PROBE            (optional) the world's probe, for checks: coaxial.guardProbe(field)
#include "Everglade-step.h"
#include "Sticks.h"
#include "face-address.h"
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
// The pack's catalog; the rig the clips are baked from, and the bake. The frame's instances are the
// zone's records, in its state: the module's own memory stays under host.js's 8 MiB start.
static uint8_t catalog[65536];
static coax_t_People rig;
static float bake[6453720];        // everglade/scene/bodies.coax BD_BAKE_FLOATS
static uint32_t baked;            // the bake's generation: 1 once baked
static uint8_t rig_file[2097152]; static uint32_t rig_bytes;   /* the pack's rig entry (VRG1, #13508): the people's and the two-handed clips' sets */
static uint32_t frames;
static int64_t draw_n;
static coax_t_Player player;   // your body (metaverse-engine/move/walk.coax)
static coax_t_Sticks sticks;   // a phone's thumb: the stick of the finger the zone says is its (everglade/zone-touch.coax)
static double now;
static coax_t_Drive drive;   // the agent driver's (metaverse-engine/agent/drive.coax): the zone holds a view of it

static LAB_STATE *world_at(cx_face_addr state) { return (LAB_STATE *)(uintptr_t)state; }
static coax_t_LabWalk *walk_at(cx_face_addr state) { return &world_at(state)->w.e[0]; }
static LAB_VIEW at(cx_face_addr state) { return (LAB_VIEW){world_at(state), 1}; }
static coax_t_EgZone *zone_at(cx_face_addr state) { return &walk_at(state)->z.e[0]; }
static coax_v_t_EgZone zone(cx_face_addr state) { return (coax_v_t_EgZone){zone_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(LAB_STATE) * 1000003u + offsetof(coax_t_EgZone, outbox) * 13u + LAB_TAG);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(LAB_STATE); }
EXPORT("coaxial_state_init") void coaxial_state_init(cx_face_addr state) { LAB_DEFAULT(world_at(state)); zone_drive_attach(zone(state), (coax_v_t_Drive){&drive, 1}); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_lab_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_lab_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_lab_uniform_floats(); }
// The glade's ground, trees and sky are traced per pixel: drawn at most 6.4 million pixels (3200 by
// 2000; the host scales the canvas up past that), as Experimental's are.
// The graphics governor's (everglade/zone.coax zone_pixel_budget, metaverse-engine/core/quality-page.coax): fewer while frames run long.
static cx_face_addr quality_state;
static float width, height, dpr;
static bool golden;
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) {
  if (!quality_state) return 6400000u;
  int64_t px = zone_pixel_budget(zone(quality_state), width, height, dpr);
  return px > 0 ? (uint32_t)px : 6400000u;
}
// The meadow shares one large geometry storage buffer.
EXPORT("coaxial_character_storage_v1") uint32_t coaxial_character_storage_v1(void) { return 256u * 1024u * 1024u; }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(cx_face_addr state, cx_face_addr vertices, cx_face_addr uniforms, float w, float h, float scale, float dt, uint32_t still) {
  now += dt;
  width = w; height = h; dpr = scale; quality_state = state; zone_quality_frame(zone(state), dt, w, h, scale, golden);
  // your body's step on Everglade's controller, in everglade-face.c's order
  coax_t_ZoneWalk walk = zone_walk(zone(state));
  if (walk.put) player = metaverse_player_place(walk.x, walk.y, walk.z, walk.face);
  if (walk.on) {
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
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_lab_max_vertices() * coaxial_lab_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_lab_uniform_floats()};
  int64_t n = LAB_FRAME(at(state), out, u, w, h, scale, dt, still != 0);
  draw_n = 0;
  if (zone_at(state)->bodies.e[0].ready) {
    if (!baked && zone_bake_from(zone(state), (coax_v_t_People){&rig, 1}, (coax_v_f32){bake, sizeof bake / sizeof bake[0]}, (coax_v_u8){rig_file, rig_bytes})) baked = 1;   /* once the pack's rig is in (#13508) */
    draw_n = zone_scene(zone(state), (coax_v_f32){zone_at(state)->records.e, sizeof zone_at(state)->records.e / sizeof(float)});
  }
  frames += 1;
  return n < 0 ? 0u : (uint32_t)n;
}

// ---- the figures: the pack's catalog, and the scene's draws over its vertices and the two stores
/* the pack's rig entry (kind 7), handed over before the catalog (coax/web/host.js deliverRig): the bake reads it */
EXPORT("coaxial_pack_rig") cx_face_addr coaxial_pack_rig(cx_face_addr state, uint32_t bytes) { (void)state; return bytes <= sizeof rig_file ? (cx_face_addr)(uintptr_t)rig_file : 0u; }
EXPORT("coaxial_pack_rig_capacity") uint32_t coaxial_pack_rig_capacity(cx_face_addr state) { (void)state; return (uint32_t)sizeof rig_file; }
EXPORT("coaxial_pack_rig_ready") void coaxial_pack_rig_ready(cx_face_addr state, uint32_t bytes) { (void)state; rig_bytes = bytes <= sizeof rig_file ? bytes : 0u; }
/* the same for a native host (metaverse-engine/host/README.md: coaxial_rig_v1, #13508) */
EXPORT("coaxial_rig_v1") cx_face_addr coaxial_rig_v1(cx_face_addr state, uint32_t bytes) { return coaxial_pack_rig(state, bytes); }
EXPORT("coaxial_rig_ready_v1") void coaxial_rig_ready_v1(cx_face_addr state, uint32_t bytes) { coaxial_pack_rig_ready(state, bytes); }
EXPORT("coaxial_asset") cx_face_addr coaxial_asset(cx_face_addr state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof catalog ? (cx_face_addr)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(cx_face_addr state, int32_t i) {
  (void)state;return i == 0 ? (uint32_t)sizeof catalog : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(cx_face_addr state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) zone_catalog(zone(state), (coax_string){(const char *)catalog, bytes});
}
EXPORT("coaxial_draws") cx_face_addr coaxial_draws(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->bodies.e[0].draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(cx_face_addr state) { (void)state; return draw_n > 0 ? (uint32_t)draw_n : 0u; }
EXPORT("coaxial_shadow_draws_v1") cx_face_addr coaxial_shadow_draws_v1(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->shadow_draws.e; }
EXPORT("coaxial_shadow_draw_count_v1") uint32_t coaxial_shadow_draw_count_v1(cx_face_addr state) { return (uint32_t)zone_at(state)->shadow_n; }
/* The scene's key-light shadow tiles (charactercreator.coax shadow_frame; coax/web/host.js shadowTexture). */
EXPORT("coaxial_shadow_atlas_v1") uint32_t coaxial_shadow_atlas_v1(cx_face_addr state, int32_t coarse, int32_t which) { return (uint32_t)zone_shadow_atlas(zone(state), coarse, which); }
EXPORT("coaxial_shadow_v1") int32_t coaxial_shadow_v1(cx_face_addr state, int32_t field) { return (int32_t)zone_shadow_field(zone(state), field); }
EXPORT("coaxial_shadow_event_v1") void coaxial_shadow_event_v1(cx_face_addr state, int32_t kind, int32_t arg) { zone_shadow_event(zone(state), kind, arg); }
EXPORT("coaxial_layers_side_v1") uint32_t coaxial_layers_side_v1(cx_face_addr state) { return (uint32_t)zone_layers_side(zone(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 0 ? (uint32_t)sizeof(((coax_t_EgZone *)0)->records.e) : (uint32_t)sizeof bake; }
EXPORT("coaxial_store") cx_face_addr coaxial_store(cx_face_addr state, int32_t k) { return k == 0 ? (cx_face_addr)(uintptr_t)zone_at(state)->records.e : (cx_face_addr)(uintptr_t)bake; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(cx_face_addr state, int32_t k) {
  if (k == 0) { uint32_t n = 0; coax_t_Bodies *b = &zone_at(state)->bodies.e[0]; for (int64_t d = 0; d < b->draw_n; d++) n += b->draws.e[d * 4 + 3]; return n * 24u * 4u; }
  return (uint32_t)sizeof bake;
}
// The instances every frame; the bake once, when it is made.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(cx_face_addr state, int32_t k) { (void)state; return k == 0 ? frames : baked; }

static coax_v_t_LabWalk walk(cx_face_addr state) { return (coax_v_t_LabWalk){walk_at(state), 1}; }
// A hub's keys first (lab-walk.coax lab_key: the index, the next station and the one before), then the zone's.
EXPORT("coaxial_key") uint32_t coaxial_key(cx_face_addr state, int32_t key, int32_t mods) { (void)mods; return lab_key(walk(state), key) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(cx_face_addr state, int32_t key, int32_t mods) { return zone_key_up(zone(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(cx_face_addr state) { zone_blur(zone(state)); sticks = metaverse_sticks_reset(); }
/* Metaverse Engine host contract v1 (metaverse-engine/host/README.md), as fivelab-face.c has it: a native build says so;
   the browser's module does not yet. */
#if defined(COAXIAL_NATIVE_ADDRESS)
EXPORT("coaxial_host_contract_version") uint32_t coaxial_host_contract_version(void) { return 1; }
#endif
/* Nothing is held across a blur, so focus has nothing to restore. */
EXPORT("coaxial_focus") void coaxial_focus(cx_face_addr state) { (void)state; }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(cx_face_addr state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  bool took = LAB_POINTER(at(state), x, y, phase, kind, id);
  // the finger the zone says is the thumb's moves the stick (metaverse-engine/core/sticks.coax)
  if (zone_thumb(zone(state))) sticks = metaverse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return took ? 1u : 0u;
}
// Additive DOM input hook: the mouse's buttons, for the bow.
EXPORT("coaxial_pointer_buttons_v1") uint32_t coaxial_pointer_buttons_v1(cx_face_addr state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) {
  bool took = LAB_POINTER_BUTTONS(at(state), x, y, phase, kind, id, button);
  if (zone_thumb(zone(state))) sticks = metaverse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now);
  return took ? 1u : 0u;
}
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(cx_face_addr state, float dy) { return zone_wheel(zone(state), dy) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(cx_face_addr state) { return (int32_t)zone_cursor(zone(state)); }
/* The mouselook (metaverse-engine/cam/mouselook.coax): host.js holds the pointer when this says 2 and forwards the mouse's travel. */
EXPORT("coaxial_mouselook") int32_t coaxial_mouselook(cx_face_addr state) { return (int32_t)zone_mouselook(zone(state)); }
EXPORT("coaxial_look_delta") void coaxial_look_delta(cx_face_addr state, float dx, float dy) { zone_look_delta(zone(state), dx, dy); }
EXPORT("coaxial_pointer_lock") void coaxial_pointer_lock(cx_face_addr state, int32_t status) { zone_pointer_lock(zone(state), status); }
/* A phone's turn as look and aim (metaverse-engine/cam/gyro.coax, #13189): host.js from devicemotion, the page host from CoreMotion. */
EXPORT("coaxial_tilt") void coaxial_tilt(cx_face_addr state, float dyaw, float dpitch) { zone_tilt(zone(state), dyaw, dpitch); }
EXPORT("coaxial_gyro") int32_t coaxial_gyro(cx_face_addr state) { return (int32_t)zone_gyro(zone(state)); }
EXPORT("coaxial_gyro_status") void coaxial_gyro_status(cx_face_addr state, int32_t status) { zone_gyro_status(zone(state), status); }

// ---- the Grid, its sky and the stations, drawn after the scene (bitcoin-lab-fx.wgsl, host.js particles): quads of 16 floats
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_LabWalk *)0)->quads.e); }
EXPORT("coaxial_particles") cx_face_addr coaxial_particles(cx_face_addr state) { return (cx_face_addr)(uintptr_t)walk_at(state)->quads.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(cx_face_addr state) { return (uint32_t)(walk_at(state)->quad_n * 16 * sizeof(float)); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(cx_face_addr state) { return zone_at(state)->creator.e[0].on ? (uint32_t)walk_at(state)->quad_n : 0u; }

EXPORT("coaxial_inbox") cx_face_addr coaxial_inbox(cx_face_addr state) { (void)state; return (cx_face_addr)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
// The HUD is the FANTASY style (metaverse-engine/ui/hud-style.coax): its strokes blend straight alpha, sealed with their coverage.
static cx_face_addr page_state;
EXPORT("coaxial_fx_alpha_v1") uint32_t coaxial_fx_alpha_v1(void) { return page_state && zone_straight_alpha(zone(page_state)) ? 1u : 0u; }
EXPORT("coaxial_message") void coaxial_message(cx_face_addr state, int32_t kind, uint32_t length) {
  page_state = state;
  if (length > sizeof inbox) return;
  LAB_MESSAGE(at(state), kind, (coax_string){(const char *)inbox, length});
}

EXPORT("coaxial_action") int32_t coaxial_action(cx_face_addr state) { return (int32_t)lab_action(walk(state)); }   /* a hub's address as you move, then the zone's */
EXPORT("coaxial_outbox") cx_face_addr coaxial_outbox(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(cx_face_addr state) { return (uint32_t)zone_at(state)->out_n; }
/* the allocations those bytes and the media packet's are in, whole (host contract v1) */
EXPORT("coaxial_outbox_capacity") uint32_t coaxial_outbox_capacity(cx_face_addr state) { return (uint32_t)sizeof zone_at(state)->outbox.e; }
EXPORT("coaxial_action_done") void coaxial_action_done(cx_face_addr state) { zone_action_done(zone(state)); }

// The bar: its pictures, cards and cooldowns for the host to draw (coax/web/host.js coaxial_media; everglade/zone-combat.coax xc_frame).
EXPORT("coaxial_media") cx_face_addr coaxial_media(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->xc.e[0].media.e; }
EXPORT("coaxial_media_len") uint32_t coaxial_media_len(cx_face_addr state) { coax_t_EgZone *l = zone_at(state); return l->creator.e[0].experimental && !l->creator.e[0].walk.on ? (uint32_t)l->xc.e[0].media_n : 0u; }
EXPORT("coaxial_media_capacity") uint32_t coaxial_media_capacity(cx_face_addr state) { return (uint32_t)sizeof zone_at(state)->xc.e[0].media.e; }

/* The agent driver (metaverse-engine/agent/drive.coax, coax/web/host.js coaxial.drive): the answer to the last command (message 700). */
EXPORT("coaxial_drive_out") cx_face_addr coaxial_drive_out(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_drive_out(zone(state)).data; }
EXPORT("coaxial_drive_out_len") uint32_t coaxial_drive_out_len(cx_face_addr state) { return (uint32_t)zone_drive_out(zone(state)).count; }
// Read-only observations for headless checks.
#ifdef LAB_PROBE
// What a check reads of the walk and the world (lab-walk.coax lab_probe, and the world's own fields).
EXPORT("coaxial_guard_probe") int32_t coaxial_guard_probe(cx_face_addr state,int32_t field) {return (int32_t)LAB_PROBE(at(state),field);}
#endif
EXPORT("coaxial_field_probe") int32_t coaxial_field_probe(cx_face_addr state,int32_t field) {return (int32_t)zone_probe(zone(state),field);}

EXPORT("coaxial_player_appearance_v1") uint32_t coaxial_player_appearance_v1(void) { return 1u; }

// The zone's per-frame audio commands, consumed once by the shared host.
EXPORT("coaxial_sounds") cx_face_addr coaxial_sounds(cx_face_addr state) { return (cx_face_addr)(uintptr_t)zone_at(state)->experimental_audio.e[0].lines.e; }
EXPORT("coaxial_sounds_capacity") uint32_t coaxial_sounds_capacity(cx_face_addr state) { return sizeof zone_at(state)->experimental_audio.e[0].lines.e; }
EXPORT("coaxial_sounds_len") uint32_t coaxial_sounds_len(cx_face_addr state) { coax_t_ExperimentalSound *audio = &zone_at(state)->experimental_audio.e[0]; uint32_t n = (uint32_t)audio->n; audio->n = 0; return n; }

#include "experimental-sound-bank.h"
