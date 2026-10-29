// Ice Cream Man's realm's C face for the browser (coax/web/host.js): scalar
// Wasm exports over icecream.coax (the HUD) and verse/icecream/scene.coax
// (the run and the house, its own library). The host keeps one Realm, the
// two side by side. Input goes to the HUD first (its buttons, the stick's
// ring), then to the run; each frame the run's facts go to the HUD. Store 0
// is the house and its models, store 1 the objects placed this frame
// (docs/verse/realm-host.md). Action 30 hands the page a round's trace.
#include "icecream.h"
#include "Scene.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define ACT_TRACE 30

typedef struct { coax_t_IceRealm hud; coax_t_IceScene scene; } Realm;
static Realm *state_at(uint32_t state) { return (Realm *)(uintptr_t)state; }
static coax_v_t_IceRealm at(uint32_t state) { return (coax_v_t_IceRealm){&state_at(state)->hud, 1}; }
static coax_v_t_IceScene scene_at(uint32_t state) { return (coax_v_t_IceScene){&state_at(state)->scene, 1}; }
static uint8_t inbox[1024];
static float facts[64];
static int tracing[1024];

static void acts(uint32_t state) {
  for (int k = 0; k < 16; k++) {
    int64_t a = ice_next_act(at(state));
    if (!a) return;
    icecream_act(scene_at(state), a >> 8, a & 255);
  }
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(Realm) * 1000003u + offsetof(Realm, scene) * 7u + offsetof(coax_t_IceRealm, outbox) * 13u + 0x49434532u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(Realm); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_ice_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_ice_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_ice_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  int64_t floats = coaxial_ice_uniform_floats();
  coax_v_t_IceScene scene = scene_at(state);
  icecream_frame(scene, width, height, dpr, still ? 0.0f : dt);
  icecream_facts(scene, (coax_v_f32){facts, 64});
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_ice_max_vertices() * coaxial_ice_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, floats};
  int64_t n = ice_frame(at(state), out, u, width, height, dpr, dt, still != 0, (coax_v_f32){facts, 64}, icecream_seed(scene));
  acts(state);
  icecream_scene_uniforms(scene, u, width, height, still != 0);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  bool used = ice_key(at(state), key, true);
  acts(state);
  if (used) return 1u;
  return icecream_key(scene_at(state), key, true, mods) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  return icecream_key(scene_at(state), key, false, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { ice_blur(at(state)); acts(state); icecream_blur(scene_at(state)); }
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { icecream_wheel(scene_at(state), dy); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  int64_t zone = ice_pointer(at(state), x, y, phase, kind, id);
  acts(state);
  if (zone >= 0) return 1u;
  coax_t_IceRealm *h = &state_at(state)->hud;
  icecream_pointer(scene_at(state), x, y, phase, kind, id, zone == -2 ? 1 : 0, h->stick_cx, h->stick_cy);
  return 1u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)ice_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  if (kind == 600) ice_page(at(state), (coax_string){(const char *)inbox, (int64_t)length});
  if (kind == 601) icecream_suspend(scene_at(state), true);
  if (kind == 602) icecream_suspend(scene_at(state), false);
}
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  icecream_hash(scene_at(state), (coax_string){(const char *)inbox, (int64_t)length});
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) {
  int64_t a = ice_action(at(state));
  if (a) { tracing[0] = 0; return (int32_t)a; }
  if (icecream_trace_ready(scene_at(state))) { icecream_trace_sent(scene_at(state)); tracing[0] = 1; return ACT_TRACE; }
  return 0;
}
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) {
  if (tracing[0]) return (uint32_t)(uintptr_t)state_at(state)->scene.plays.e[0].trace.e;
  return (uint32_t)(uintptr_t)state_at(state)->hud.outbox.e;
}
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) {
  if (tracing[0]) return (uint32_t)icecream_trace_len(scene_at(state));
  return (uint32_t)state_at(state)->hud.out_n;
}
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { if (!tracing[0]) ice_action_done(at(state)); tracing[0] = 0; }

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->scene.draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)icecream_draw_count(scene_at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)icecream_scene_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  coax_t_IceScene *s = &state_at(state)->scene;
  return k == 1 ? (uint32_t)(uintptr_t)s->objs.e : (uint32_t)(uintptr_t)s->verts.e;
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return (uint32_t)icecream_scene_store_bytes(scene_at(state), k); }
