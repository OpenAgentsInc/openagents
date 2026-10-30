// Ice Cream Man's realm's C face for the browser (coax/web/host.js): scalar
// Wasm exports over icecream.coax (the HUD) and verse/icecream/scene.coax
// (the run and the house, its own library). The host keeps one Realm, the
// two side by side. Input goes to the HUD first (its buttons, the stick's
// ring), then to the run; each frame the run's facts go to the HUD. Store 0
// is the house and its models, store 1 the objects placed this frame
// (docs/verse/realm-host.md). Action 4 keeps the save in the realm's slot,
// message 4 brings it back; action 30 hands the page a round's trace.
#include "icecream.h"
// The edit layer's RealmEdit (in icecream.h) and the scene both have an [8] bool: the scene's keeps its own name here.
#define coax_a8_bool coax_a8_bool_scene
#include "Scene.h"
#undef coax_a8_bool
#include "analytics-face.h"
#include "edit-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))
#define ACT_TRACE 30
#define ACT_KEEP 4
#define MSG_KEPT 4

typedef struct { coax_t_IceRealm hud; coax_t_IceScene scene; } Realm;
static Realm *state_at(uint32_t state) { return (Realm *)(uintptr_t)state; }
static coax_v_t_IceRealm at(uint32_t state) { return (coax_v_t_IceRealm){&state_at(state)->hud, 1}; }
static coax_v_t_IceScene scene_at(uint32_t state) { return (coax_v_t_IceScene){&state_at(state)->scene, 1}; }
static coax_v_t_RealmEdit ed(uint32_t state) { return (coax_v_t_RealmEdit){state_at(state)->hud.ed.e, 1}; }
// The board's rows are the run's options: minions and weapons change the house being played at
// once, backrooms the next house (verse/icecream/play.coax board_now).
static void edit_options(uint32_t state) {
  coax_t_Options *o = &state_at(state)->scene.plays.e[0].options;
  o->no_minions = !ice_row_on(at(state), (coax_string){"minions", 7});
  o->no_weapons = !ice_row_on(at(state), (coax_string){"weapons", 7});
  o->no_backrooms = !ice_row_on(at(state), (coax_string){"backrooms", 9});
  icecream_board_now(scene_at(state));
}
static uint8_t inbox[4096];
static float facts[64];
static int tracing[1024];
static coax_t_Track track_state;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static bool entered;
static float seen[64];
#define WORLD "ice-cream-man"

static void track_start(void) { if (!track_state.page_n) track_init(tr(), track_cstr(WORLD)); }
static void track_num_of(const char *k, double v) { track_num(tr(), track_cstr(k), v); }
static const char *DIFFICULTIES[] = {"easy", "normal", "hard"};
static const char *ENDS_[] = {"playing", "escaped", "melted", "scooped"};
// The run's moments, from its facts: a round begun, a key found, a round's end.
static void track_run(uint32_t seed) {
  static bool begun;
  if (facts[52] < seen[52]) begun = false;
  bool fresh = !begun && facts[52] > 0;
  if (fresh) begun = true;
  if (fresh && track_begin(tr(), track_cstr("round-start"))) {
    track_num_of("seed", seed);
    track_str(tr(), track_cstr("difficulty"), track_cstr(DIFFICULTIES[(int)facts[42] % 3]));
    track_bool(tr(), track_cstr("touch"), facts[34] > 0);
    track_end(tr());
  }
  if (!fresh && facts[38] > seen[38] && track_begin(tr(), track_cstr("objective"))) {
    track_str(tr(), track_cstr("objective"), track_cstr("key"));
    track_num_of("keys", facts[38]);
    track_num_of("s", facts[35]);
    track_end(tr());
  }
  if (facts[1] != 0 && seen[1] == 0 && track_begin(tr(), track_cstr("round-end"))) {
    track_str(tr(), track_cstr("end"), track_cstr(ENDS_[(int)facts[1] & 3]));
    track_num_of("seed", seed);
    track_num_of("s", facts[35]);
    track_num_of("keys", facts[38]);
    track_num_of("hits", facts[39]);
    track_str(tr(), track_cstr("difficulty"), track_cstr(DIFFICULTIES[(int)facts[42] % 3]));
    track_bool(tr(), track_cstr("best"), facts[37] > 0);
    track_end(tr());
  }
  for (int i = 0; i < 64; i++) seen[i] = facts[i];
}

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
  track_start();
  icecream_facts(scene, (coax_v_f32){facts, 64});
  track_run((uint32_t)icecream_seed(scene));
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_ice_max_vertices() * coaxial_ice_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, floats};
  int64_t n = ice_frame(at(state), out, u, width, height, dpr, dt, still != 0, (coax_v_f32){facts, 64}, icecream_seed(scene));
  acts(state);
  edit_options(state);
  edit_face_drain(ed(state));
  icecream_scene_uniforms(scene, u, width, height, still != 0);
  track_frame(tr(), dt, width, height, dpr);
  if (n > 0 && !entered) { entered = true; track_world(tr(), track_cstr(WORLD)); track_loaded(tr(), track_cstr("load-engine")); }
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  track_start();
  track_key(tr(), key, ice_wants_text(at(state)));
  track_drain(&track_state);
  bool used = ice_key(at(state), key, true);
  edit_face_drain(ed(state));
  if (ice_wants_text(at(state))) return used ? 1u : 0u;
  acts(state);
  if (used) return 1u;
  return icecream_key(scene_at(state), key, true, mods) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  track_key_up(tr(), key);
  if (ice_key(at(state), key, false)) return 1u;
  if (ice_wants_text(at(state))) return 0u;
  return icecream_key(scene_at(state), key, false, mods) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { track_blur(tr()); ice_blur(at(state)); acts(state); icecream_blur(scene_at(state)); }
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { icecream_wheel(scene_at(state), dy); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  track_start();
  track_pointer(tr(), x, y, phase, kind, (coax_string){"", 0});
  track_drain(&track_state);
  int64_t zone = ice_pointer(at(state), x, y, phase, kind, id);
  acts(state);
  edit_face_drain(ed(state));
  if (zone >= 0) return 1u;
  coax_t_IceRealm *h = &state_at(state)->hud;
  icecream_pointer(scene_at(state), x, y, phase, kind, id, zone == -2 ? 1 : 0, h->stick_cx, h->stick_cy);
  return 1u;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)ice_cursor(at(state)); }
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return ice_wants_text(at(state)) ? 1u : 0u; }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  if (length > sizeof inbox) length = sizeof inbox;
  track_start();
  if (kind != 600 && ice_edit_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length})) { edit_face_drain(ed(state)); return; }
  if (kind == 600) ice_edit_message(at(state), kind, (coax_string){(const char *)inbox, (int64_t)length});
  if (track_message(tr(), kind, (coax_string){(const char *)inbox, (int64_t)length})) track_drain(&track_state);
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
  if (a) { tracing[0] = 0; return (int32_t)a; }
  if (icecream_trace_ready(scene_at(state))) { icecream_trace_sent(scene_at(state)); tracing[0] = 1; return ACT_TRACE; }
  if (icecream_save_ready(scene_at(state))) { icecream_save(scene_at(state)); tracing[0] = 2; return ACT_KEEP; }
  return 0;
}
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) {
  if (tracing[0] == 2) return (uint32_t)(uintptr_t)state_at(state)->scene.plays.e[0].save.e;
  if (tracing[0]) return (uint32_t)(uintptr_t)state_at(state)->scene.plays.e[0].trace.e;
  return (uint32_t)(uintptr_t)state_at(state)->hud.outbox.e;
}
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) {
  if (tracing[0] == 2) return (uint32_t)state_at(state)->scene.plays.e[0].save_n;
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
