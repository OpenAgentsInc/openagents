// The characters realm's C face for the browser: scalar Wasm exports that
// wrap characters.coax, which takes views. Every pointer is an offset into
// the host's WebAssembly.Memory. Logic stays in characters.coax and
// verse/figure.coax.
#include "characters.h"
#include "analytics-face.h"

#define EXPORT(name) __attribute__((export_name(name)))

static coax_t_Chars *state_at(uint32_t state) { return (coax_t_Chars *)(uintptr_t)state; }
static coax_v_t_Chars chars_of(uint32_t state) { return (coax_v_t_Chars){state_at(state), 1}; }
static coax_t_Track track_state;
static coax_v_t_Track tr(void) { return (coax_v_t_Track){&track_state, 1}; }
static void track_page(void) { if (!track_state.page_n) track_init(tr(), track_cstr("realm-characters")); }

EXPORT("coaxial_layout") uint32_t coaxial_layout(void) {
  return (uint32_t)(sizeof(coax_t_Chars) * 1000003u + _Alignof(coax_t_Chars) * 31u
                    + offsetof(coax_t_Chars, figs) + offsetof(coax_t_Chars, inbox) * 7u);
}
EXPORT("coaxial_state_size") uint32_t coaxial_state_size(void) { return (uint32_t)sizeof(coax_t_Chars); }
EXPORT("coaxial_max_vertices") uint32_t coaxial_max_vertices(void) { return (uint32_t)coaxial_chars_max_vertices(); }
EXPORT("coaxial_vertex_floats") uint32_t coaxial_vertex_floats(void) { return (uint32_t)coaxial_chars_vertex_floats(); }
EXPORT("coaxial_uniform_floats") uint32_t coaxial_uniform_floats(void) { return (uint32_t)coaxial_chars_uniform_floats(); }

EXPORT("coaxial_frame")
uint32_t coaxial_frame(uint32_t state, uint32_t vertices, uint32_t uniforms,
                       float width, float height, float dpr, float dt, uint32_t still) {
  coax_v_f32 out = {(float *)(uintptr_t)vertices, coaxial_chars_max_vertices() * coaxial_chars_vertex_floats()};
  coax_v_f32 u = {(float *)(uintptr_t)uniforms, coaxial_chars_uniform_floats()};
  int64_t n = coaxial_chars_frame(chars_of(state), out, u, width, height, dpr, dt, still != 0);
  track_page();
  track_frame(tr(), dt, width, height, dpr);
  if (n > 0) track_loaded(tr(), track_cstr("load-realm-characters"));
  track_drain(&track_state);
  return n < 0 ? 0u : (uint32_t)n;
}

EXPORT("coaxial_key") uint32_t coaxial_key(uint32_t state, int32_t key, int32_t mods) {
  track_page();
  track_key(tr(), key, 0);
  track_drain(&track_state);
  return coaxial_chars_key(chars_of(state), key, mods) ? 1u : 0u;
}
EXPORT("coaxial_key_up") uint32_t coaxial_key_up(uint32_t state, int32_t key, int32_t mods) {
  (void)state; (void)mods; track_key_up(tr(), key); return 0u;
}
EXPORT("coaxial_blur") void coaxial_blur(uint32_t state) { (void)state; track_blur(tr()); }
EXPORT("coaxial_pointer") uint32_t coaxial_pointer(uint32_t state, float x, float y, int32_t phase, int32_t kind, int32_t id) {
  track_page();
  track_pointer(tr(), x, y, phase, kind, track_cstr(coaxial_chars_cursor(chars_of(state)) == 1 ? "button" : ""));
  uint32_t used = coaxial_chars_pointer(chars_of(state), x, y, phase, kind, id) ? 1u : 0u;
  track_drain(&track_state);
  return used;
}
EXPORT("coaxial_wheel") void coaxial_wheel(uint32_t state, float dy) { coaxial_chars_wheel(chars_of(state), dy); }
EXPORT("coaxial_cursor") int32_t coaxial_cursor(uint32_t state) { return (int32_t)coaxial_chars_cursor(chars_of(state)); }
EXPORT("coaxial_inbox") uint32_t coaxial_inbox(uint32_t state) { return (uint32_t)(uintptr_t)&state_at(state)->inbox; }
EXPORT("coaxial_inbox_cap") uint32_t coaxial_inbox_cap(void) { return (uint32_t)coaxial_chars_inbox_cap(); }
EXPORT("coaxial_message") void coaxial_message(uint32_t state, int32_t kind, uint32_t length) {
  uint32_t cap = (uint32_t)coaxial_chars_inbox_cap();
  coax_string bytes = {(const char *)&state_at(state)->inbox, (int64_t)(length < cap ? length : cap)};
  track_page();
  track_message(tr(), kind, bytes);
  track_drain(&track_state);
}
EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) { coaxial_chars_hash(chars_of(state), (int64_t)length); }
