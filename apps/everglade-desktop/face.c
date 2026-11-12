// Everglade for desktop: face.h over the C that scenes.coax compiles to (the worlds: the list, the
// Workshop).
#include "workshop.h"
#include "face.h"
#include <stdlib.h>
#include <string.h>

static coax_v_t_Scenes at(void *state) { return (coax_v_t_Scenes){(coax_t_Scenes *)state, 1}; }

void *eg_new(void) {
  void *state = calloc(1, (size_t)sc_state_size());
  if (state) sc_state_init(at(state));
  return state;
}
int32_t eg_vertex_floats(void) { return (int32_t)sc_vertex_floats(); }
int32_t eg_max_vertices(void) { return (int32_t)sc_max_vertices(); }
int32_t eg_frame(void *state, float *vertices, float width, float height, float scale, float dt) {
  coax_v_f32 out = {vertices, sc_max_vertices() * sc_vertex_floats()};
  int64_t n = sc_draw(at(state), out, width, height, scale, dt);
  return n < 0 ? 0 : (int32_t)n;
}
void eg_pointer(void *state, float x, float y, int32_t phase) { sc_pointer(at(state), x, y, phase); }
void eg_wheel(void *state, float dy) { sc_wheel(at(state), dy); }
int32_t eg_key(void *state, int32_t key, int32_t down) { return sc_key(at(state), key, down != 0) ? 1 : 0; }
int32_t eg_you(void *state, float *out5) { return sc_you(at(state), (coax_v_f32){out5, 5}) ? 1 : 0; }
int32_t eg_camera(void *state, float width, float height, float scale, float *out16) { return sc_camera(at(state), width, height, scale, (coax_v_f32){out16, 16}) ? 1 : 0; }
void eg_pad(void *state, float x, float y) { sc_pad(at(state), x, y); }
void eg_safe(void *state, float left, float top, float bottom) { sc_safe(at(state), left, top, bottom); }
void eg_turn_offer(void *state) { sc_turn_offer(at(state)); }
int32_t eg_turns(void *state) { return sc_turns(at(state)) ? 1 : 0; }
void eg_turn(void *state, float yaw, float pitch) { sc_turn(at(state), yaw, pitch); }
int32_t eg_tap(void *state, float x, float y) { return sc_tap(at(state), x, y) ? 1 : 0; }
int32_t eg_scene(void *state) { return (int32_t)sc_scene(at(state)); }
int32_t eg_place(void *state) { return (int32_t)sc_place(at(state)); }
void eg_go(void *state, int32_t world) { sc_go(at(state), world); }
int32_t eg_claims(void *state, float x, float y) { return sc_claims(at(state), x, y) ? 1 : 0; }
void eg_runs(void *state, int32_t world, int32_t runs) { sc_host_runs(at(state), world, runs != 0); }
int32_t eg_on_worlds(void *state, float x, float y) { return sc_on_worlds(at(state), x, y) ? 1 : 0; }
void eg_opening(void *state, int32_t opening) { sc_host_opening(at(state), opening != 0); }
void eg_login_offer(void *state) { sc_login_offer(at(state)); }
int32_t eg_login_wants_keys(void *state) { return sc_login_wants_keys(at(state)) ? 1 : 0; }
int32_t eg_login_open(void *state) { return sc_login_open(at(state)) ? 1 : 0; }
int32_t eg_login_char(void *state, int32_t ch) { return sc_login_char(at(state), ch) ? 1 : 0; }
int32_t eg_login_take(void *state, char *out, int32_t cap, int32_t *n) {
  int64_t m = sc_login_text(at(state), (coax_v_u8){(uint8_t *)out, cap < 0 ? 0 : cap});
  *n = (int32_t)m;
  return (int32_t)sc_login_take(at(state));
}
void eg_login_show(void *state, int32_t login_state, int32_t busy, const char *words, const char *who) {
  sc_login_show(at(state), login_state, busy != 0, (coax_string){words, (int64_t)strlen(words)}, (coax_string){who, (int64_t)strlen(who)});
}
