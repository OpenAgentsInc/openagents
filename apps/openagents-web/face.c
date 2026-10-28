// The splash's C face for the browser: scalar Wasm exports that wrap the
// Coax procedures, which take views. Every pointer is an offset into the
// host's WebAssembly.Memory, where the host keeps the state, vertex and
// uniform regions. Logic stays in splash.coax and ui.coax.
#include "splash.h"
#include "analytics-face.h"

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_SplashState *state_at(uint32_t state) { return (coax_t_SplashState *)(uintptr_t)state; }
static coax_v_t_Ui ui_of(uint32_t state) { return (coax_v_t_Ui){&state_at(state)->ui, 1}; }
// What people do here (analytics.coax): the page is "home", or "404".
static coax_t_Track track_state;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }

// The state's layout fingerprint: the host keeps state across a code swap
// only when this matches, and starts the state over when it doesn't.
EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_SplashState) * 1000003u + _Alignof(coax_t_SplashState) * 31u
                    + offsetof(coax_t_SplashState, frames) + offsetof(coax_t_SplashState, ui) * 7u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_SplashState); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_splash_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_splash_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_splash_uniform_floats(); }

// Which page to draw: 0 the splash, 1 the not-found page.
EXPORT("coaxial_page") void coaxial_page(uint32_t state, int32_t page) { state_at(state)->page = page; }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms,
                       float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_t_SplashState s = {state_at(state), 1};
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_splash_max_vertices() * coaxial_splash_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_splash_uniform_floats()};
  int64_t n = coaxial_splash_frame(s, ui_of(state), out, u, width, height, dpr, dt, still != 0);
  if (!track_state.page_n) track_init(tr(), track_cstr(state_at(state)->page == 1 ? "404" : "home"));
  track_frame(tr(), dt, width, height, dpr);
  if (n > 0) track_loaded(tr(), track_cstr("load-landing"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

// Input: keys (a code point, or a named key below zero), the pointer in
// device pixels, and messages whose text the host writes to the inbox.
// Typed keys (a code or a key going into the terminal) are never recorded.
EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  track_key(tr(), key, coaxial_splash_wants_text(ui_of(state)));
  track_drain(&track_state);
  return coaxial_splash_key(ui_of(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  (void)state; (void)mods; track_key_up(tr(), key); return 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { (void)state; track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind) {
  track_pointer(tr(), x, y, phase, kind, track_cstr(coaxial_splash_cursor(ui_of(state)) == 1 ? "link" : ""));
  uint32_t used = coaxial_splash_pointer(ui_of(state), x, y, phase, kind) ? 1u : 0u;
  track_drain(&track_state);
  return used;
}
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)&state_at(state)->ui.inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)coaxial_splash_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  uint32_t cap = (uint32_t)coaxial_splash_inbox_cap();
  coax_string bytes = {(const char *)&state_at(state)->ui.inbox, (int64_t)(length < cap ? length : cap)};
  _Bool mine = track_message(tr(), kind, bytes);
  track_drain(&track_state);
  if (mine) return;
  coaxial_splash_message(ui_of(state), kind, length);
}
EXPORT("coaxial_wants_text") uint32_t coaxial_wants_text(uint32_t state) { return coaxial_splash_wants_text(ui_of(state)) ? 1u : 0u; }
// The text field taking typing, in device pixels: x0, y0, x1, y1 (all zero: none).
EXPORT("coaxial_field") uint32_t coaxial_field(uint32_t state) { return (uint32_t)(uintptr_t)&state_at(state)->ui.field; }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_splash_cursor(ui_of(state)); }

// Output: an action code and its text in the outbox; done empties it.
EXPORT("coaxial_action") int32_t coaxial_action(uint32_t state) { return (int32_t)coaxial_splash_action(ui_of(state)); }
EXPORT("coaxial_outbox") uint32_t coaxial_outbox(uint32_t state) { return (uint32_t)(uintptr_t)&state_at(state)->ui.outbox; }
EXPORT("coaxial_outbox_len") uint32_t coaxial_outbox_len(uint32_t state) { return (uint32_t)state_at(state)->ui.out_n; }
EXPORT("coaxial_action_done") void coaxial_action_done(uint32_t state) { coaxial_splash_action_done(ui_of(state)); }
