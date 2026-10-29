// Build your own realm's C face for the browser (coax/web/host.js): scalar Wasm exports over
// builder.coax. The host keeps one Builder; the face keeps the page's Track (analytics.coax) and
// hands the page's one request and the Track's batches to the host's HTTP import. It decides nothing.
#include "builder.h"
#include "analytics-face.h"
#include <stdbool.h>
#include <stddef.h>

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Builder *state_at(uint32_t state) { return (coax_t_Builder *)(uintptr_t)state; }
static coax_v_t_Builder at(uint32_t state) { return (coax_v_t_Builder){state_at(state), 1}; }
static coax_t_Track track_state;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static uint8_t inbox[16384];

static void start_track(void) { if (!track_state.page_n) track_init(tr(), track_cstr("realm-builder")); }
// After every call: the page's request (the plan, the make, a player key), then the Track's batch.
static void drain(uint32_t state) {
  coax_t_Builder *s = state_at(state);
  if (s->req_ready) {
    int32_t id = coaxial_http((uint32_t)(uintptr_t)s->req_url.e, (uint32_t)s->req_url_n,
                              (uint32_t)(uintptr_t)s->req_body.e, (uint32_t)s->req_body_n, (int32_t)s->req_flags);
    builder_requested(at(state), id);
  }
  track_drain(&track_state);
}

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Builder) * 1000003u + _Alignof(coax_t_Builder) * 31u
                    + offsetof(coax_t_Builder, plates) * 7u + offsetof(coax_t_Builder, outbox) * 13u + 0x42554cu);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Builder); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_builder_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_builder_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_builder_uniform_floats(); }

// Golden mode (coax/web/goldens.ts): drawn as if still.
static uint32_t golden;
EXPORT("coaxial_golden") void coaxial_golden(uint32_t on) { golden = on != 0u; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms, float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_builder_max_vertices() * coaxial_builder_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_builder_uniform_floats()};
  start_track();
  int64_t n = builder_frame(at(state), tr(), out, u, width, height, dpr, dt, still != 0 || golden != 0u);
  track_frame(tr(), dt, width, height, dpr);
  if (n > 0) track_loaded(tr(), track_cstr("load-builder"));
  drain(state);
  return n < 0 ? 0u : (uint32_t)n;
}

// Typed keys are never recorded.
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  start_track();
  track_key(tr(), key, builder_wants_text(at(state)));
  uint32_t used = builder_key(at(state), tr(), key, mods, true) ? 1u : 0u;
  drain(state);
  return used;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  track_key_up(tr(), key);
  uint32_t used = builder_key(at(state), tr(), key, mods, false) ? 1u : 0u;
  drain(state);
  return used;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { track_blur(tr()); builder_blur(at(state)); drain(state); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  start_track();
  track_pointer(tr(), x, y, phase, kind, builder_label(at(state), x, y));
  uint32_t used = builder_pointer(at(state), tr(), x, y, phase, kind, id) ? 1u : 0u;
  drain(state);
  return used;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { builder_wheel(at(state), dy); }
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { (void)state; return (uint32_t)(uintptr_t)inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return sizeof inbox; }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, int32_t length) {
  if (length < 0 || (uint32_t)length > sizeof inbox) return;
  start_track();
  coax_string bytes = {(const char *)inbox, length};
  bool taken = track_message(tr(), kind, bytes);
  if (kind == 600 || !taken) builder_message(at(state), tr(), kind, bytes);
  drain(state);
}
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return builder_wants_text(at(state)) ? 1u : 0u; }
// The typed line, in device pixels: x0, y0, x1, y1 (all zero: none).
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->field.e; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)builder_cursor(at(state)); }
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)builder_action(at(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)state_at(state)->outbox.e; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { builder_action_done(at(state)); }
