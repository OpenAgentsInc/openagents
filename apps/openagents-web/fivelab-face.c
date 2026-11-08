// 5E Lab's C face for the browser (coax/web/host.js): scalar Wasm exports
// over fivelab.coax, which holds the battle on the SRD engine
// (aethyr/rules/srd/) and its drawing. It decides nothing.
// gpu/fivelab.coax's bg_fs draws the grid, the glade and the log's glass, writing the glade's
// depth; scene_vs/scene_fs the figures (Everglade's pack: its catalog here, its vertices, indices
// and layers on the GPU), from two stores: the frame's instances (fivelab-bodies.coax bd_build)
// and the clips baked once (bd_bake); fx_fs the strokes lab_frame writes, and fivelab-fx.wgsl the
// glade's effects (puffs), drawn after the figures, depth-tested.
// Player mode walks your hero on Everglade's controller, as everglade-face.c does: everglade-step.coax
// (aethyr/walk.coax, with player.coax's verse_player_*) steps your body on the keys and a phone's
// thumbs (aethyr/sticks.coax) among the figures round you (lab_move_world), and the Lab says where
// it may stand (lab_keep) and draws it (lab_you).
#include "fivelab.h"
#include "Everglade-step.h"
#include "Sticks.h"
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
static float bake[6430320];        // fivelab-bodies.coax BD_BAKE_FLOATS
static uint32_t baked;            // the bake's generation: 1 once baked
static uint32_t frames;
static int64_t draw_n;
static coax_t_Player player;   // your body in Player mode (aethyr/walk.coax)
static coax_t_Sticks sticks;   // a phone's thumbs
static float width, height, dpr;
static double now;
static bool touched;

static coax_t_Lab *lab_at(uint32_t state) { return (coax_t_Lab *)(uintptr_t)state; }
static coax_v_t_Lab at(uint32_t state) { return (coax_v_t_Lab){lab_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Lab) * 1000003u + offsetof(coax_t_Lab, outbox) * 13u + 0x35454c42u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Lab); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Lab((coax_t_Lab *)(uintptr_t)state); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_lab_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_lab_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_lab_uniform_floats(); }
// bg_fs traces the glade's ground, trees and sky per pixel (the figures are drawn, not traced):
// drawn at most 6.4 million pixels (3200 by 2000; the host scales the canvas up past that).
EXPORT("coaxial_pixel_budget") uint32_t coaxial_pixel_budget(void) { return 6400000u; }
// The experimental meadow and castle share one large geometry storage buffer.
EXPORT("coaxial_character_storage_v1") uint32_t coaxial_character_storage_v1(void) { return 256u * 1024u * 1024u; }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  lab_golden(at(state), golden);
  width = w; height = h; dpr = scale; now += dt;
  // Player mode: your body's step on Everglade's controller, in everglade-face.c's order
  coax_t_LabWalk walk = lab_walk(at(state));
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
    player = eg_step_advance(player, mw, (coax_v_f32){near.data, mw.bodies * 7}, (coax_v_f32){walls.data, mw.walls * 6}, (coax_v_f32){floors.data, mw.floors * 10}, frame);
    coax_t_EgBody kept = lab_keep_body(at(state), origin, eg_step_body(player));
    if (kept.x != player.pos.x || kept.z != player.pos.z) player = verse_player_constrain(player, kept.x, kept.z);
    player = eg_step_put(player, kept);
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
  if (lab_at(state)->bodies.e[0].ready) {
    if (!baked) { lab_bake(at(state), (coax_v_t_People){&rig, 1}, (coax_v_f32){bake, sizeof bake / sizeof bake[0]}); baked = 1; }
    draw_n = lab_scene(at(state), (coax_v_f32){lab_at(state)->records.e, sizeof lab_at(state)->records.e / sizeof(float)});
  }
  frames += 1;
  return n < 0 ? 0u : (uint32_t)n;
}

// ---- the figures: the pack's catalog, and the scene's draws over its vertices and the two stores
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof catalog ? (uint32_t)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) {
  (void)state;return i == 0 ? (uint32_t)sizeof catalog : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) lab_catalog(at(state), (coax_string){(const char *)catalog, bytes});
}
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->bodies.e[0].draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { (void)state; return draw_n > 0 ? (uint32_t)draw_n : 0u; }
EXPORT("coaxial_shadow_draws_v1") uint32_t coaxial_shadow_draws_v1(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->shadow_draws.e; }
EXPORT("coaxial_shadow_draw_count_v1") uint32_t coaxial_shadow_draw_count_v1(uint32_t state) { return (uint32_t)lab_at(state)->shadow_n; }
/* Character Create's key-light shadow tiles (charactercreator.coax shadow_frame; coax/web/host.js shadowTexture): none elsewhere. */
EXPORT("coaxial_shadow_atlas_v1") uint32_t coaxial_shadow_atlas_v1(uint32_t state, int32_t coarse, int32_t which) { return (uint32_t)lab_shadow_atlas(at(state), coarse, which); }
EXPORT("coaxial_shadow_v1") int32_t coaxial_shadow_v1(uint32_t state, int32_t field) { return (int32_t)lab_shadow_field(at(state), field); }
EXPORT("coaxial_shadow_event_v1") void coaxial_shadow_event_v1(uint32_t state, int32_t kind, int32_t arg) { lab_shadow_event(at(state), kind, arg); }
EXPORT("coaxial_layers_side_v1") uint32_t coaxial_layers_side_v1(uint32_t state) { return (uint32_t)lab_layers_side(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
// The frame store (host.js, group 0 binding 1): the tokens and squares bg_fs reads, every frame.
EXPORT("coaxial_frame_store_cap") uint32_t coaxial_frame_store_cap(void) { return (uint32_t)sizeof(((coax_t_Lab *)0)->frame.e); }
EXPORT("coaxial_frame_store") uint32_t coaxial_frame_store(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->frame.e; }
EXPORT("coaxial_frame_store_bytes") uint32_t coaxial_frame_store_bytes(uint32_t state) { return (uint32_t)coaxial_lab_frame_floats(at(state)) * 4u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 0 ? (uint32_t)sizeof(((coax_t_Lab *)0)->records.e) : (uint32_t)sizeof bake; }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { return k == 0 ? (uint32_t)(uintptr_t)lab_at(state)->records.e : (uint32_t)(uintptr_t)bake; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  (void)state;
  if (k == 0) { uint32_t n = 0; coax_t_Bodies *b = &lab_at(state)->bodies.e[0]; for (int64_t d = 0; d < b->draw_n; d++) n += b->draws.e[d * 4 + 3]; return n * 24u * 4u; }
  return (uint32_t)sizeof bake;
}
// The instances every frame; the bake once, when it is made.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { (void)state; return k == 0 ? frames : baked; }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { return lab_key(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return lab_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { lab_blur(at(state)); sticks = verse_sticks_reset(); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  if (lab_pointer(at(state), x, y, phase, kind, id)) return 1u;
  // a finger the Lab left (Player mode, off its buttons) is a thumb's: the stick, the look, a tap's jump
  if (kind != 0 && lab_at(state)->player) { touched = true; sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now); return 1u; }
  return 0u;
}
// Additive DOM input hook; the original pointer export remains available to native hosts.
EXPORT("coaxial_pointer_buttons_v1") uint32_t coaxial_pointer_buttons_v1(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id, int32_t button) {
  if (lab_pointer_buttons(at(state), x, y, phase, kind, id, button)) return 1u;
  if (kind != 0 && lab_at(state)->player) { touched = true; sticks = verse_sticks_world_touch(sticks, id, phase, x, y, width, height, dpr, now); return 1u; }
  return 0u;
}
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(uint32_t state, float dy) { return lab_wheel(at(state), dy) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)lab_cursor(at(state)); }

// ---- the glade's effects, drawn after bg_fs (fivelab-fx.wgsl, host.js particles): puffs of 12 floats
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_Lab *)0)->puffs.e[0].out); }
EXPORT("coaxial_particles") uint32_t coaxial_particles(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->puffs.e[0].out.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(uint32_t state) { return (uint32_t)(lab_at(state)->puffs.e[0].out_n * 12 * sizeof(float)); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(uint32_t state) { return lab_at(state)->started ? (uint32_t)lab_at(state)->puffs.e[0].out_n : 0u; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  lab_message(at(state), kind, (coax_string){(const char *)inbox, length});
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)lab_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)lab_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { lab_action_done(at(state)); }

// Experimental's bar: its pictures, cards and cooldowns for the host to draw (coax/web/host.js coaxial_media; experimental-combat.coax xc_frame).
EXPORT("coaxial_media") uint32_t coaxial_media(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->xc.e[0].media.e; }
EXPORT("coaxial_media_len") uint32_t coaxial_media_len(uint32_t state) { coax_t_Lab *l = lab_at(state); return l->creator.e[0].experimental && !l->creator.e[0].walk.on ? (uint32_t)l->xc.e[0].media_n : 0u; }

// Read-only observations for headless wardrobe checks.
EXPORT("coaxial_field_probe") int32_t coaxial_field_probe(uint32_t state,int32_t field) {return (int32_t)lab_creator_probe(at(state),field);}

EXPORT("coaxial_player_appearance_v1") uint32_t coaxial_player_appearance_v1(void) { return 1u; }

// Experimental's per-frame audio commands, consumed once by the shared host.
EXPORT("coaxial_sounds") uint32_t coaxial_sounds(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->experimental_audio.e[0].lines.e; }
EXPORT("coaxial_sounds_capacity") uint32_t coaxial_sounds_capacity(uint32_t state) { return sizeof lab_at(state)->experimental_audio.e[0].lines.e; }
EXPORT("coaxial_sounds_len") uint32_t coaxial_sounds_len(uint32_t state) { coax_t_ExperimentalSound *audio = &lab_at(state)->experimental_audio.e[0]; uint32_t n = (uint32_t)audio->n; audio->n = 0; return n; }

#include "experimental-sound-bank.h"
