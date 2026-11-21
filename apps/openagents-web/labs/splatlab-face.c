// Splat Lab's C face for the browser (coax/web/host.js): scalar Wasm exports over splatlab.coax,
// which decides the scene, the camera, the emitters and the static splats, and writes the HUD. It
// decides nothing. The static splats are the particles' memory (group 1 binding 0 of
// gpu/splatlab-fx.coax, three vec4s each), the meshes' vertices are store 0 (gpu/splatlab.coax
// scene_vs), and coaxial_sort hands host.js the bitonic sort's plan. Asset 1 is realms.json's
// local:/private/splats/skull.splat, a CC BY 4.0 capture a local address loads from the cache
// outside the repo; anywhere else it arrives empty and the shrine stands without it.
#include "splatlab.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[1024];
// Wasm memory is 64 MiB in all (coax/web/wasm.sh): splats 18 MiB, the capture 8 MiB.
#define SPLAT_FLOATS_ALL (393216 * 12)
#define MESH_FLOATS_ALL (24000 * 8)
#define CAPTURE_BYTES (8u << 20)
static float splats[SPLAT_FLOATS_ALL];
static float mesh[MESH_FLOATS_ALL];
static uint8_t capture[CAPTURE_BYTES];
static uint32_t capture_bytes;
static bool golden;

static coax_t_Lab *lab_at(uint32_t state) { return (coax_t_Lab *)(uintptr_t)state; }
static coax_v_t_Lab at(uint32_t state) { return (coax_v_t_Lab){lab_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Lab) * 1000003u + offsetof(coax_t_Lab, outbox) * 13u + 0x53504c42u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Lab); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Lab((coax_t_Lab *)(uintptr_t)state); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_lab_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_lab_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_lab_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  lab_golden(at(state), golden);
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_lab_max_vertices() * coaxial_lab_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_lab_uniform_floats()};
  coax_v_f32 s = {splats, SPLAT_FLOATS_ALL};
  coax_v_f32 m = {mesh, MESH_FLOATS_ALL};
  coax_string cap = {(const char *)capture, (int64_t)capture_bytes};
  int64_t n = lab_frame(at(state), out, u, s, m, cap, w, h, scale, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { return lab_key(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return lab_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { lab_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return lab_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
}
EXPORT("coaxial_wheel") uint32_t coaxial_wheel(uint32_t state, float dy) { return lab_wheel(at(state), dy) ? 1u : 0u; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)lab_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  lab_message(at(state), kind, (coax_string){(const char *)inbox, length});
}
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) return;
  lab_hash(at(state), (coax_string){(const char *)inbox, length});
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)lab_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)lab_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { lab_action_done(at(state)); }

// asset 1: the capture (asset 0 is the particles' shader, which host.js keeps)
EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 1 && bytes <= sizeof capture ? (uint32_t)(uintptr_t)capture : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) {
  (void)state; return i == 1 ? (uint32_t)sizeof capture : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i != 1) return;
  capture_bytes = bytes <= sizeof capture ? bytes : 0u;
  lab_capture(at(state), (coax_string){(const char *)capture, (int64_t)capture_bytes});
}

// the splats: static ones from memory, animated ones made on the GPU (instances: all of them)
EXPORT("coaxial_particle_cap") uint32_t coaxial_particle_cap(void) { return (uint32_t)sizeof splats; }
EXPORT("coaxial_particles") uint32_t coaxial_particles(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)splats; }
EXPORT("coaxial_particle_bytes") uint32_t coaxial_particle_bytes(uint32_t state) {
  int64_t n = lab_static(at(state));
  return (uint32_t)((n > 0 ? n : 1) * 12 * 4);
}
EXPORT("coaxial_particle_instances") uint32_t coaxial_particle_instances(uint32_t state) { return (uint32_t)lab_total(at(state)); }
EXPORT("coaxial_particle_version") uint32_t coaxial_particle_version(uint32_t state) { return (uint32_t)lab_splat_version(at(state)); }
EXPORT("coaxial_sort") uint32_t coaxial_sort(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->sort.e; }

// the meshes: one draw of their vertices, in order (store 0)
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return lab_mesh_n(at(state)) > 0 ? 1u : 0u; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 1u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { (void)k; return (uint32_t)sizeof mesh; }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)state; (void)k; return (uint32_t)(uintptr_t)mesh; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  (void)k;
  int64_t n = lab_mesh_n(at(state));
  return (uint32_t)((n > 0 ? n : 1) * 8 * 4);
}
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { (void)k; return (uint32_t)lab_mesh_version(at(state)); }
