/* Firm reason: 1 (scalar browser ABI only; the world, walking, camera and HUD are Coax: importworld.coax). */
/* An imported world's C face for coax/web/host.js (docs/metaverse/realm-host.md): the state, the frame,
   input, the pack's catalog (asset 0) and its scene, drawn as indexed draws with one object store. */
#include "importworld.h"
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <string.h>
#include "face-address.h"
#define EXPORT(name) __attribute__((export_name(name)))
static uint8_t catalog[4194304];
static uint8_t inbox[4096];
static coax_t_ImportWorld *state(cx_face_addr p) { return (coax_t_ImportWorld *)(uintptr_t)p; }
static coax_v_t_ImportWorld view(cx_face_addr p) { return (coax_v_t_ImportWorld){state(p), 1}; }
EXPORT("coaxial_layout") uint32_t coaxial_layout(void) { return (uint32_t)(sizeof(coax_t_ImportWorld) * 1000003u + offsetof(coax_t_ImportWorld, draws) * 7u + 0x49575231u); }
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_ImportWorld); }
EXPORT("coaxial_state_init") void coaxial_state_init(cx_face_addr p) { memset(state(p), 0, sizeof(coax_t_ImportWorld)); iwr_init(view(p)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return 32768; }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return 12; }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return 256; }
EXPORT("coaxial_frame") uint32_t coaxial_frame(cx_face_addr p, cx_face_addr v, cx_face_addr u, float w, float h, float d, float dt, uint32_t still) {
  int64_t n = iwr_frame(view(p), (coax_v_f32){(float *)(uintptr_t)v, 32768 * 12}, (coax_v_f32){(float *)(uintptr_t)u, 256}, w, h, d, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}
EXPORT("coaxial_key") uint32_t coaxial_key(cx_face_addr p, int32_t k, int32_t mods) { (void)mods; return iwr_key(view(p), k, true) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(cx_face_addr p, int32_t k, int32_t mods) { (void)mods; return iwr_key(view(p), k, false) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(cx_face_addr p) { iwr_blur(view(p)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(cx_face_addr p, float x, float y, int32_t phase, int32_t kind, int32_t id) { return iwr_pointer(view(p), x, y, phase, kind, id) ? 1u : 0u; }
EXPORT("coaxial_wheel") void coaxial_wheel(cx_face_addr p, float delta) { iwr_wheel(view(p), delta); }
EXPORT("coaxial_inbox") cx_face_addr coaxial_inbox(cx_face_addr p) { (void)p; return (cx_face_addr)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(cx_face_addr p, int32_t kind, uint32_t n) { (void)p; (void)kind; (void)n; }
EXPORT("coaxial_asset") cx_face_addr coaxial_asset(cx_face_addr p, int32_t i, uint32_t n) { (void)p; return i == 0 && n <= sizeof catalog ? (cx_face_addr)(uintptr_t)catalog : 0; }
EXPORT("coaxial_asset_capacity") uint32_t coaxial_asset_capacity(cx_face_addr p, int32_t i) { (void)p; return i == 0 ? (uint32_t)sizeof catalog : 0u; }
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(cx_face_addr p, int32_t i, uint32_t n) {
  if (i == 0 && n > 0 && n <= sizeof catalog) iwr_catalog(view(p), (coax_string){(const char *)catalog, (int64_t)n});
  iwr_asset(view(p), i, (int64_t)n);
}
EXPORT("coaxial_draws") cx_face_addr coaxial_draws(cx_face_addr p) { return (cx_face_addr)(uintptr_t)state(p)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(cx_face_addr p) { return (uint32_t)iwr_draw_count(view(p)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 1u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { (void)k; return (uint32_t)sizeof(((coax_t_ImportWorld *)0)->objects); }
EXPORT("coaxial_store") cx_face_addr coaxial_store(cx_face_addr p, int32_t k) { (void)k; return (cx_face_addr)(uintptr_t)state(p)->objects.e; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(cx_face_addr p, int32_t k) { (void)k; uint32_t n = (uint32_t)state(p)->draw_n; return (n ? n : 1u) * 32u * (uint32_t)sizeof(float); }
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(cx_face_addr p, int32_t k) { (void)k; return (uint32_t)state(p)->store_version; }
EXPORT("coaxial_scene_ready") uint32_t coaxial_scene_ready(cx_face_addr p) { return state(p)->ready ? 1u : 0u; }
/* For headless checks: 0 ready, 1 the catalog admitted, 2 failed, 3 to 6 each asset's bytes, 7 draws, 8 the map. */
EXPORT("coaxial_guard_probe") int32_t coaxial_guard_probe(cx_face_addr p, int32_t field) {
  coax_t_ImportWorld *s = state(p);
  switch (field) { case 0: return s->ready; case 1: return s->admitted; case 2: return s->failed; case 3: case 4: case 5: case 6: return (int32_t)s->assets.e[field - 3]; case 7: return (int32_t)s->draw_n; case 8: return (int32_t)s->map; default: return 0; }
}
