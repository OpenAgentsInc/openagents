// 5E Lab's C face for the browser (coax/web/host.js): scalar Wasm exports
// over fivelab.coax, which holds the battle on the SRD engine
// (aethyr/rules/srd/) and its drawing. It decides nothing.
// gpu/fivelab.coax's bg_fs draws the grid, the glade and the log's glass, writing the glade's
// depth; scene_vs/scene_fs the figures (Everglade's pack: its catalog here, its vertices, indices
// and layers on the GPU), from two stores: the frame's instances (fivelab-bodies.coax bd_build)
// and the clips baked once (bd_bake); fx_fs the strokes lab_frame writes, and fivelab-fx.wgsl the
// glade's effects (puffs), drawn after the figures, depth-tested.
#include "fivelab.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
static bool golden;
// The pack's catalog; the rig the clips are baked from, and the bake; the frame's instances.
static uint8_t catalog[16384];
static coax_t_People rig;
static float bake[544440];        // fivelab-bodies.coax BD_BAKE_FLOATS
static uint32_t baked;            // the bake's generation: 1 once baked
static float records[1320 * 24];  // BD_RECORDS instances of BD_REC_FLOATS
static uint32_t frames;
static int64_t draw_n;

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
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  lab_golden(at(state), golden);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_lab_max_vertices() * coaxial_lab_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_lab_uniform_floats()};
  int64_t n = lab_frame(at(state), out, u, w, h, scale, dt, still != 0);
  draw_n = 0;
  if (lab_at(state)->bodies.e[0].ready) {
    if (!baked) { lab_bake((coax_v_t_People){&rig, 1}, (coax_v_f32){bake, sizeof bake / sizeof bake[0]}); baked = 1; }
    draw_n = lab_scene(at(state), (coax_v_f32){records, sizeof records / sizeof records[0]});
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
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return k == 0 ? (uint32_t)sizeof records : (uint32_t)sizeof bake; }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)state; return k == 0 ? (uint32_t)(uintptr_t)records : (uint32_t)(uintptr_t)bake; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  (void)state;
  if (k == 0) { uint32_t n = 0; coax_t_Bodies *b = &lab_at(state)->bodies.e[0]; for (int64_t d = 0; d < b->draw_n; d++) n += b->draws.e[d * 4 + 3]; return n * 24u * 4u; }
  return (uint32_t)sizeof bake;
}
// The instances every frame; the bake once, when it is made.
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { (void)state; return k == 0 ? frames : baked; }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { return lab_key(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return lab_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { lab_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return lab_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
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
