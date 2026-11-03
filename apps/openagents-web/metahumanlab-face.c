// MetaHuman Lab's C face for the browser (coax/web/host.js): scalar Wasm exports over
// metahumanlab.coax, which plays the CC0 people rig's clips on one figure and writes its capsules
// and the HUD. It decides nothing. gpu/metahumanlab.coax's bg_fs traces the floor and the figure
// from the uniforms, and fx_fs draws the strokes lab_frame writes. A private human pack
// (realms.json "private:human", coax/web/host.js), when one loads on a local address, is assets
// 0 to 3: its catalog comes here, its vertices, indices and layers go to the GPU, and the scene
// draws its mesh, skinned by the pose's palette (store 0). A pack with a face rig carries its
// block after the catalog's text; lab_face runs it each frame into the face vertices' rows, which
// follow the palette in store 0.
#include "metahumanlab.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];
// The catalog and the face rig's block: Wasm memory is 64 MiB in all (coax/web/wasm.sh).
#define CATALOG_BYTES (28u << 20)
#define PALETTE_FLOATS (65 * 12)
#define FACE_ROWS (4 * 65536)
static uint8_t catalog[CATALOG_BYTES];
static uint32_t catalog_bytes;
static float face_work[1024 + 9 * 1024 + 3 * 12 * 1024 + 9 * 40000 + 19 * 40000];
static bool face_still[1024];
static float pose_store[PALETTE_FLOATS + FACE_ROWS];
static uint32_t pose_version, pal_seen = 0xffffffffu;
static bool golden;

static coax_t_Lab *lab_at(uint32_t state) { return (coax_t_Lab *)(uintptr_t)state; }
static coax_v_t_Lab at(uint32_t state) { return (coax_v_t_Lab){lab_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Lab) * 1000003u + offsetof(coax_t_Lab, outbox) * 13u + 0x4d484c42u);
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
  int64_t n = lab_frame(at(state), out, u, w, h, scale, dt, still != 0);
  // the pose store: the palette, then the face rig's rows
  coax_t_Lab *l = lab_at(state);
  if ((uint32_t)l->pal_version != pal_seen) {
    pal_seen = (uint32_t)l->pal_version;
    for (int i = 0; i < PALETTE_FLOATS; i++) pose_store[i] = l->palette.e[i];
    pose_version++;
  }
  if (catalog_bytes) {
    coax_string cat = {(const char *)catalog, (int64_t)catalog_bytes};
    coax_v_f32 work = {face_work, (int64_t)(sizeof face_work / sizeof face_work[0])};
    coax_v_bool st = {face_still, 1024};
    coax_v_f32 rows = {pose_store + PALETTE_FLOATS, FACE_ROWS};
    if (lab_face(at(state), cat, work, st, rows)) pose_version++;
  }
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

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)lab_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)lab_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)lab_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { lab_action_done(at(state)); }

EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof catalog ? (uint32_t)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(uint32_t state, int32_t i) {
  (void)state; return i == 0 ? (uint32_t)sizeof catalog : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) { catalog_bytes = bytes; lab_catalog(at(state), (coax_string){(const char *)catalog, (int64_t)bytes}); }
  lab_pack_part(at(state), i, i == 0 && bytes > sizeof catalog ? 0 : (int64_t)bytes);
}
// the level of detail drawn: its run of draws
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)(lab_at(state)->draws.e + 4 * lab_draw_first(at(state))); }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)lab_draw_count(at(state)); }
EXPORT("coaxial_water_draws") uint32_t coaxial_water_draws(uint32_t state) { return (uint32_t)lab_water_draws(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 1u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { (void)k; return (uint32_t)sizeof pose_store; }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)state; (void)k; return (uint32_t)(uintptr_t)pose_store; }
// the palette always, the face rows only while the rig runs
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) {
  (void)k;
  coax_t_Lab *l = lab_at(state);
  return (uint32_t)(PALETTE_FLOATS + (l->face_state == 2 ? 4 * l->face_count : 0)) * 4u;
}
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { (void)state; (void)k; return pose_version; }
