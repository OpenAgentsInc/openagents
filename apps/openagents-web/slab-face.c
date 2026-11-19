// SLAB Simulation's C face for the browser (coax/web/host.js): scalar Wasm
// exports over slab.coax. The host keeps one Slab; this file decides nothing.
// No assets: slab.wgsl's bg_fs traces the sky and the platform, and surface_vs
// draws the clipmap sea from the uniforms slab_frame writes (docs/research/ocean/slab-world-spec.md).
#include "slab.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Slab *state_at(uint32_t state) { return (coax_t_Slab *)(uintptr_t)state; }
static coax_v_t_Slab at(uint32_t state) { return (coax_v_t_Slab){state_at(state), 1}; }
static uint8_t inbox[16384];

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_Slab) * 1000003u + 0x534c4142u); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Slab); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Slab((coax_t_Slab *)(uintptr_t)state); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_slab_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_slab_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_slab_uniform_floats(); }

// Golden mode (host.js coaxial_golden): the sea holds one moment.
static bool golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_slab_max_vertices() * coaxial_slab_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_slab_uniform_floats()};
  slab_golden(at(state), golden);
  int64_t n = slab_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

// W10: the clipmap sea's vertices this frame (host.js draws surface_vs that many times after bg_fs).
EXPORT("coaxial_surface_vertices") uint32_t coaxial_surface_vertices(uint32_t state) {
  int64_t n = slab_surface_vertices(at(state));
  return n < 0 ? 0u : (uint32_t)n;
}

// The photoreal style's scene (slab-photoreal.coax): the platform's parts as meshes, drawn by
// host.js scene_vs/scene_fs between bg_fs and the sea. Four stores: the vertices, the placed
// parts, the materials and the ground's sky field. No draws when the realm is not photoreal.
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->photo.e[0].draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)slab_photo_draws(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 4; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)slab_photo_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  coax_t_SlabPhoto *p = &state_at(state)->photo.e[0];
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)p->mesh.e : k == 1 ? (void *)p->records.e : k == 2 ? (void *)p->materials.e : (void *)p->field.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return (uint32_t)slab_photo_store_bytes(at(state), k); }
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { return (uint32_t)slab_photo_store_version(at(state), k); }
EXPORT("coaxial_store_span_v1") uint32_t coaxial_store_span_v1(uint32_t state, int32_t k, int32_t field) { return (uint32_t)slab_photo_store_span(at(state), k, field); }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { (void)mods; return slab_key(at(state), key, true) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)mods; return slab_key(at(state), key, false) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { slab_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return slab_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { slab_wheel(at(state), dy); }
// The pointer's cursor over the HUD's buttons and what can be picked (0 default, 1 link).
EXPORT("coaxial_cursor") uint32_t coaxial_cursor(uint32_t state) { int64_t c = slab_cursor(at(state)); return c < 0 ? 0u : (uint32_t)c; }
// The spray thrown at the walls (slab-water.coax): its bursts' rows are the page's particles
// (host.js "Particles"), drawn by slab-fx.wgsl, uploaded when a new burst is thrown.
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof(((coax_t_Slab *)0)->water.e[0].spray.e[0].rows); }
EXPORT("coaxial_particles") uint32_t coaxial_particles(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->water.e[0].spray.e[0].rows.e; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(uint32_t state) { (void)state; return (uint32_t)sizeof(((coax_t_Slab *)0)->water.e[0].spray.e[0].rows); }
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(uint32_t state) { int64_t n = slab_particle_instances(at(state)); return n < 0 ? 0u : (uint32_t)n; }
EXPORT("coaxial_particle_version") uint32_t coaxial_particle_version(uint32_t state) { return (uint32_t)slab_particle_version(at(state)); }
// The open sea's foam field (slab-foam.coax): the frame store at group 0 binding 1 (host.js
// "frame"), copied when the field has stepped (slab_foam_store_bytes answers 0 otherwise).
EXPORT("coaxial_frame_store_cap") uint32_t coaxial_frame_store_cap(void) { return (uint32_t)sizeof(((coax_t_Slab *)0)->sea_foam.e[0].packed); }
EXPORT("coaxial_frame_store") uint32_t coaxial_frame_store(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->sea_foam.e[0].packed.e; }
EXPORT("coaxial_frame_store_bytes") uint32_t coaxial_frame_store_bytes(uint32_t state) { int64_t n = slab_foam_store_bytes(at(state)); return n < 0 ? 0u : (uint32_t)n; }
// The HUD's panels and strokes blend over the frame by their coverage.
EXPORT("coaxial_fx_alpha_v1") uint32_t coaxial_fx_alpha_v1(void) { return 1; }
// The post pass (gpu/slab-photoreal.coax slab_post_fs) draws contact; none of its five bloom passes.
EXPORT("coaxial_bloom_passes_v1") uint32_t coaxial_bloom_passes_v1(uint32_t state) { (void)state; return 0; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  slab_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
}

// The HUD's pane (coax/web/host.js, "A page's own pane"): the module the browser draws it with
// (scene-pane.coax), what the HUD holds now as text (slab-hud.coax), and how many times that changed.
EXPORT("coaxial_pane") uint32_t coaxial_pane(uint32_t state) { return (uint32_t)(uintptr_t)slab_pane(at(state)).data; }
EXPORT("coaxial_pane_len") uint32_t coaxial_pane_len(uint32_t state) { return (uint32_t)slab_pane(at(state)).count; }
EXPORT("coaxial_pane_rev") int32_t coaxial_pane_rev(uint32_t state) { return (int32_t)slab_pane_rev(at(state)); }
EXPORT("coaxial_pane_model") uint32_t coaxial_pane_model(uint32_t state) { return (uint32_t)(uintptr_t)slab_pane_model(at(state)).data; }
EXPORT("coaxial_pane_model_len") uint32_t coaxial_pane_model_len(uint32_t state) { return (uint32_t)slab_pane_model(at(state)).count; }

// The page's address, said to the browser when a setting of the "What would it take?" panel moves
// (slab-panel.coax; coax/web/host.js, action 19): every state has an address.
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)slab_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)slab_outbox(at(state)).data; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)slab_outbox(at(state)).count; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { slab_action_done(at(state)); }
