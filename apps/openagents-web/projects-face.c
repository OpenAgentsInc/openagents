// The projects list's C face for the browser: scalar Wasm exports over the
// Coax procedures in projects.coax. The host keeps one ProjectsState.
#include "projects.h"
#include "analytics-face.h"
#include <stddef.h>
#include <string.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_v_t_ProjectsState at(uint32_t state) { coax_v_t_ProjectsState s = {(coax_t_ProjectsState *)(uintptr_t)state, 1}; return s; }

// What people do here (analytics.coax): its Track, and an inbox big enough for
// the host's page facts; the list's own messages are copied into its inbox.
static coax_t_Track track_state;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static uint8_t inbox[65536];

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_ProjectsState) * 1000003u + _Alignof(coax_t_ProjectsState) * 31u
                    + offsetof(coax_t_ProjectsState, viewer) * 7u + offsetof(coax_t_ProjectsState, outbox) * 13u + 0x50524cu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_ProjectsState); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_projects_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_projects_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_projects_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms,
                       float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_projects_max_vertices() * coaxial_projects_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_projects_uniform_floats()};
  int64_t n = coaxial_projects_frame(at(state), out, u, width, height, dpr, dt, still != 0);
  if (!track_state.page_n) track_init(tr(), track_cstr("projects"));
  track_frame(tr(), dt, width, height, dpr);
  if (n > 0) track_loaded(tr(), track_cstr("load-landing"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  track_key(tr(), key, 0);
  track_drain(&track_state);
  return coaxial_projects_key(at(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) { (void)state; (void)mods; track_key_up(tr(), key); return 0u; }
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { (void)state; track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  (void)id;
  uint32_t used = coaxial_projects_pointer(at(state), x, y, phase, kind) ? 1u : 0u;
  track_pointer(tr(), x, y, phase, kind, track_cstr(coaxial_projects_cursor(at(state)) == 1 ? "link" : ""));
  track_drain(&track_state);
  return used;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_projects_wheel(at(state), dy); }
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) {
  if (length < 0 || (uint32_t)length > sizeof inbox) return;
  if (!track_state.page_n) track_init(tr(), track_cstr("projects"));   // the facts come before the first frame
  _Bool mine = track_message(tr(), kind, (coax_string){(const char *)inbox, length});
  track_drain(&track_state);
  if (mine || length > coaxial_projects_inbox_cap()) return;
  memcpy(at(state).data->inbox.e, inbox, (size_t)length);
  coaxial_projects_message(at(state), kind, length);
}
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  if (length > sizeof inbox || (int64_t)length > coaxial_projects_inbox_cap()) return;
  memcpy(at(state).data->inbox.e, inbox, (size_t)length);
  coaxial_projects_hash(at(state), (int64_t)length);
}
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return coaxial_projects_wants_text(at(state)) ? 1u : 0u; }
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->field.e; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_projects_cursor(at(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_projects_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)at(state).data->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)at(state).data->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_projects_action_done(at(state)); }
