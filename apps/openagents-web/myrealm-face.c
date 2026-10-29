// A realm someone built: its C face for the browser (coax/web/host.js), scalar Wasm exports over
// myrealm.coax. The host keeps one MyRealm; the face keeps the page's Track (analytics.coax), hands
// the realm's one request and the Track's batches to the host's HTTP import, and gives the host the
// pack's catalog (asset 0) and the scene drawn from it. It decides nothing.
#include "myrealm.h"
#include "analytics-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_MyRealm *state_at(uint32_t state) { return (coax_t_MyRealm *)(uintptr_t)state; }
static coax_v_t_MyRealm at(uint32_t state) { return (coax_v_t_MyRealm){state_at(state), 1}; }
static coax_t_Track track_state;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static uint8_t inbox[16384];
static uint8_t catalog[32768];

static void start_track(void) { if (!track_state.page_n) track_init(tr(), track_cstr("realm-made")); }
static void drain(uint32_t state) {
  coax_t_MyRealm *s = state_at(state);
  if (s->req_ready) {
    int32_t id = coaxial_http((uint32_t)(uintptr_t)s->req_url.e, (uint32_t)s->req_url_n,
                              (uint32_t)(uintptr_t)s->req_body.e, (uint32_t)s->req_body_n, (int32_t)s->req_flags);
    realm_requested(at(state), id);
  }
  track_drain(&track_state);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_MyRealm) * 1000003u + _Alignof(coax_t_MyRealm) * 31u
                    + offsetof(coax_t_MyRealm, plates) * 7u + offsetof(coax_t_MyRealm, outbox) * 13u + 0x4d59u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_MyRealm); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_myrealm_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_myrealm_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_myrealm_uniform_floats(); }

static uint32_t golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0u; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_myrealm_max_vertices() * coaxial_myrealm_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_myrealm_uniform_floats()};
  start_track();
  int64_t n = realm_frame(at(state), tr(), out, u, width, height, dpr, dt, still != 0 || golden != 0u);
  track_frame(tr(), dt, width, height, dpr);
  if (n > 0) track_loaded(tr(), track_cstr("load-realm"));
  drain(state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  (void)mods; start_track();
  track_key(tr(), key, false);
  uint32_t used = realm_key(at(state), tr(), key, true) ? 1u : 0u;
  drain(state);
  return used;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  (void)mods; track_key_up(tr(), key);
  return realm_key(at(state), tr(), key, false) ? 1u : 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { track_blur(tr()); realm_blur(at(state)); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start_track();
  track_pointer(tr(), x, y, phase, kind, realm_label(at(state), x, y));
  uint32_t used = realm_pointer(at(state), tr(), x, y, phase, kind, id) ? 1u : 0u;
  drain(state);
  return used;
}
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)realm_cursor(at(state)); }

EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) {
  if (length < 0 || (uint32_t)length > sizeof inbox) return;
  start_track();
  coax_string bytes = {(const char *)inbox, length};
  bool taken = track_message(tr(), kind, bytes);
  if (kind == 600 || !taken) realm_message(at(state), tr(), kind, bytes);
  drain(state);
}

EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)realm_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { realm_action_done(at(state)); }

EXPORT("coaxial_asset") uint32_t coaxial_asset(uint32_t state, int32_t i, uint32_t bytes) {
  (void)state;
  return i == 0 && bytes <= sizeof catalog ? (uint32_t)(uintptr_t)catalog : 0u;
}
EXPORT("coaxial_asset_ready") void coaxial_asset_ready(uint32_t state, int32_t i, uint32_t bytes) {
  if (i == 0 && bytes > 0 && bytes <= sizeof catalog) realm_catalog(at(state), (coax_string){(const char *)catalog, (int64_t)bytes});
  if (i >= 0 && i <= 3 && bytes == 0) realm_pack_failed(at(state));
}
EXPORT("coaxial_draws") uint32_t coaxial_draws(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->draws.e; }
EXPORT("coaxial_draw_count") uint32_t coaxial_draw_count(uint32_t state) { return (uint32_t)realm_draw_count(at(state)); }
EXPORT("coaxial_stores") uint32_t coaxial_stores(void) { return 1u; }
EXPORT("coaxial_store_cap") uint32_t coaxial_store_cap(int32_t k) { (void)k; return (uint32_t)sizeof(((coax_t_MyRealm *)0)->objects); }
EXPORT("coaxial_store") uint32_t coaxial_store(uint32_t state, int32_t k) { (void)k; return (uint32_t)(uintptr_t)state_at(state)->objects.e; }
EXPORT("coaxial_store_bytes") uint32_t coaxial_store_bytes(uint32_t state, int32_t k) { (void)k; return (uint32_t)(realm_object_count(at(state)) * 16 * sizeof(float)); }
