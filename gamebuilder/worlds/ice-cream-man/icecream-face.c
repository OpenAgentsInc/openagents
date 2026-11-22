/* Firm reason: 1 (scalar browser ABI; Ice Cream Man is Coax). */
// Ice Cream Man's standalone gamebuilder.org C face for coax/web/host.js:
// scalar Wasm exports over apps/openagents-web/icecream.coax (the HUD) and
// metaverse/icecream/scene.coax (the seeded house, rules, AI, and meshes).
// Omits openagents.com-only analytics/realm-edit HTTP calls so gamebuilder.org
// runs cleanly with zero external API errors, and maps the menu's "/worlds"
// navigation back to gamebuilder.org's "/" landing page.
#include "icecream.h"
#include "Scene.h"
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define ACT_NAVIGATE 1
#define ACT_KEEP 4
#define ACT_TRACE 30
#define MSG_KEPT 4

#define SCENE_STORE_WORDS (600000 * 6)   // metaverse/icecream/scene.coax MAX_SCENE * SCENE_WORDS
#define TRACE_BYTES 3000000              // metaverse/icecream/play.coax TRACE_CAP
typedef struct {
  coax_t_IceRealm hud;
  coax_t_IceScene scene;
  uint32_t scene_store[SCENE_STORE_WORDS];
  uint8_t trace_store[TRACE_BYTES];
} Realm;

static Realm *state_at(uint32_t state) { return (Realm *)(uintptr_t)state; }
static coax_v_t_IceRealm at(uint32_t state) { return (coax_v_t_IceRealm){&state_at(state)->hud, 1}; }
static coax_v_t_IceScene scene_at(uint32_t state) {
  Realm *r = state_at(state);
  coax_v_t_IceScene s = {&r->scene, 1};
  if (!r->scene.verts.data) {
    icecream_attach(s, (coax_v_u32){r->scene_store, SCENE_STORE_WORDS}, (coax_v_u8){r->trace_store, TRACE_BYTES});
  }
  return s;
}

static uint8_t inbox[4096];
static float facts[64];
static int out_mode;   // 0 hud outbox, 1 trace, 2 save, 3 "/" home override
static const uint8_t gb_home[1] = {'/'};

static void acts(uint32_t state) {
  for (int k = 0; k < 16; k++) {
    int64_t a = ice_next_act(at(state));
    if (!a) return;
    icecream_act(scene_at(state), a >> 8, a & 255);
  }
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(Realm) * 1000003u + offsetof(Realm, scene) * 7u + offsetof(coax_t_IceRealm, outbox) * 13u + 0x49434742u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(Realm); }
EXPORT("coaxial_state_init") void coaxial_state_init(uint32_t state) {
  coax_default_t_IceScene(&state_at(state)->scene);
  state_at(state)->scene.plays.e[0].start_on_title = true;
}
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
  if (ice_wants_text(at(state))) return used ? 1u : 0u;
  acts(state);
  if (used) return 1u;
  return icecream_key(scene_at(state), key, true, mods) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  if (ice_key(at(state), key, false)) return 1u;
  if (ice_wants_text(at(state))) return 0u;
  return icecream_key(scene_at(state), key, false, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { ice_blur(at(state)); acts(state); icecream_blur(scene_at(state)); }
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { icecream_wheel(scene_at(state), dy); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  int32_t pid = id & 0x3fffffff;
  int64_t zone = ice_pointer(at(state), x, y, phase, kind, pid);
  acts(state);
  if (zone >= 0) return 1u;
  coax_t_IceRealm *h = &state_at(state)->hud;
  icecream_pointer(scene_at(state), x, y, phase, kind, pid, zone == -2 ? 1 : 0, h->stick_cx, h->stick_cy);
  return 1u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)ice_cursor(at(state)); }
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return ice_wants_text(at(state)) ? 1u : 0u; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  if (kind == 600) ice_page(at(state), (coax_string){(const char *)inbox, (int64_t)length});
  if (kind == MSG_KEPT) icecream_load(scene_at(state), (coax_string){(const char *)inbox, (int64_t)length});
  if (kind == 601) icecream_suspend(scene_at(state), true);
  if (kind == 602) icecream_suspend(scene_at(state), false);
}
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  icecream_hash(scene_at(state), (coax_string){(const char *)inbox, (int64_t)length});
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) {
  int64_t a = ice_action(at(state));
  if (a) {
    coax_t_IceRealm *h = &state_at(state)->hud;
    if (a == ACT_NAVIGATE && h->out_n == 7 && memcmp(h->outbox.e, "/worlds", 7) == 0) {
      out_mode = 3;
    } else {
      out_mode = 0;
    }
    return (int32_t)a;
  }
  if (icecream_trace_ready(scene_at(state))) { icecream_trace_sent(scene_at(state)); out_mode = 1; return ACT_TRACE; }
  if (icecream_save_ready(scene_at(state))) { icecream_save(scene_at(state)); out_mode = 2; return ACT_KEEP; }
  return 0;
}
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) {
  if (out_mode == 3) return (uint32_t)(uintptr_t)gb_home;
  if (out_mode == 2) return (uint32_t)(uintptr_t)state_at(state)->scene.plays.e[0].save.e;
  if (out_mode == 1) return (uint32_t)(uintptr_t)scene_at(state).data[0].plays.e[0].trace.data;
  return (uint32_t)(uintptr_t)state_at(state)->hud.outbox.e;
}
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) {
  if (out_mode == 3) return 1u;
  if (out_mode == 2) return (uint32_t)state_at(state)->scene.plays.e[0].save_n;
  if (out_mode == 1) return (uint32_t)icecream_trace_len(scene_at(state));
  return (uint32_t)state_at(state)->hud.out_n;
}
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) {
  if (out_mode == 0 || out_mode == 3) ice_action_done(at(state));
  out_mode = 0;
}

EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->scene.draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)icecream_draw_count(scene_at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 2u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { return (uint32_t)icecream_scene_store_cap(k); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) {
  coax_t_IceScene *s = scene_at(state).data;
  return k == 1 ? (uint32_t)(uintptr_t)s->objs.e : (uint32_t)(uintptr_t)s->verts.data;
}
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { return (uint32_t)icecream_scene_store_bytes(scene_at(state), k); }
