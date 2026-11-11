// Painted Village's C face for the browser (coax/web/host.js): scalar Wasm exports over
// painted.coax, which holds the camera, the HUD and the scene's buffers. It decides nothing.
// The scene is drawn by Aethyr's stylized shading (gpu/painted.coax): no assets, three stores
// (the kit's vertices, the records, the ground table) copied to the GPU when their generation changes.
#include "painted.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static uint8_t inbox[4096];

static coax_t_Painted *page_at(uint32_t state) { return (coax_t_Painted *)(uintptr_t)state; }
static coax_v_t_Painted at(uint32_t state) { return (coax_v_t_Painted){page_at(state), 1}; }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Painted) * 1000003u + offsetof(coax_t_Painted, scene) * 13u + offsetof(coax_t_Painted, host_draws) * 7u + 0x50414e54u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Painted); }
// A new state's declared defaults (coax_default_T, from the compiler). The host calls this first.
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) { coax_default_t_Painted(page_at(state)); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_painted_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_painted_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_painted_uniform_floats(); }
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { (void)on; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float w, float h, float scale, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_painted_max_vertices() * coaxial_painted_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_painted_uniform_floats()};
  int64_t n = painted_frame(at(state), out, u, w, h, scale, dt, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

// The scene: its draws, the grass, and the stores.
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)page_at(state)->host_draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)painted_draws(at(state)); }
EXPORT("coaxial_water_draws") uint32_t coaxial_water_draws(uint32_t state) { return (uint32_t)painted_water(at(state)); }
EXPORT("coaxial_grass_instances") uint32_t coaxial_grass_instances(uint32_t state) { return (uint32_t)painted_grass(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 3; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)painted_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  coax_t_Painted *p = page_at(state);
  return (uint32_t)(uintptr_t)(k == 0 ? (void *)p->scene.e[0].mesh.e : k == 1 ? (void *)p->scene.e[0].records.e : (void *)p->scene.e[0].table.e);
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return (uint32_t)painted_store_bytes(at(state), k); }
EXPORT("coaxial_store_version") uint32_t coaxial_store_version(uint32_t state, int32_t k) { (void)k; return (uint32_t)painted_generation(at(state)); }

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { return painted_key(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { return painted_key_up(at(state), key, mods) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { painted_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return painted_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)painted_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) return;
  painted_message(at(state), kind, (coax_string){(const char *)inbox, length});
}
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) return;
  painted_hash(at(state), (coax_string){(const char *)inbox, length});
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)painted_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)page_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)page_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { painted_action_done(at(state)); }
