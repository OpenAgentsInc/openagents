// Ice Cream Man's realm's C face for the browser (coax/web/host.js): scalar
// Wasm exports over icecream.coax (the HUD) and verse/icecream/scene.coax
// (the house, its own library). The host keeps one Realm, the two side by
// side; the scene is built again whenever the HUD's seed changes, and its
// vertices go up as the one store (docs/verse/realm-host.md).
#include "icecream.h"
#include "Scene.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

typedef struct { coax_t_IceRealm hud; coax_t_IceScene scene; } Realm;
static Realm *state_at(uint32_t state) { return (Realm *)(uintptr_t)state; }
static coax_v_t_IceRealm at(uint32_t state) { return (coax_v_t_IceRealm){&state_at(state)->hud, 1}; }
static coax_v_t_IceScene scene_at(uint32_t state) { return (coax_v_t_IceScene){&state_at(state)->scene, 1}; }
static uint8_t inbox[1024];

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(Realm) * 1000003u + offsetof(Realm, scene) * 7u + offsetof(coax_t_IceRealm, outbox) * 13u + 0x49434531u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(Realm); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_ice_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_ice_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_ice_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  int64_t floats = coaxial_ice_uniform_floats();
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_ice_max_vertices() * coaxial_ice_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, floats};
  int64_t n = ice_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  coax_v_t_IceScene scene = scene_at(state);
  uint64_t seed = ice_seed(at(state));
  if (!icecream_scene_built(scene) || icecream_scene_seed(scene) != seed) icecream_scene_build(scene, seed);
  icecream_scene_uniforms(scene, (coax_v_f32){(float *)(uintptr_t)uniforms, floats}, width, height, ice_time(at(state)));
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) { (void)mods; return ice_key(at(state), key, true) ? 1u : 0u; }
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)mods; return ice_key(at(state), key, false) ? 1u : 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { ice_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  return ice_pointer(at(state), x, y, phase, kind, id) ? 1u : 0u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)ice_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) { (void)state, (void)kind, (void)length; }
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  ice_hash(at(state), (coax_string){(const char *)inbox, (int64_t)length});
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)ice_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->hud.outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->hud.out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { ice_action_done(at(state)); }

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->scene.draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return icecream_scene_built(scene_at(state)) ? 3u : 0u; }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 1u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { (void)k; return (uint32_t)icecream_scene_store_cap(); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)k; return (uint32_t)(uintptr_t)state_at(state)->scene.verts.e; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { (void)k; return (uint32_t)icecream_scene_store_bytes(scene_at(state)); }
