// Everglade for desktop: face.h over the C that workshop.coax compiles to.
#include "workshop.h"
#include "face.h"
#include <stdlib.h>

static coax_v_t_Workshop at(void *state) { return (coax_v_t_Workshop){(coax_t_Workshop *)state, 1}; }

void *eg_new(void) {
  void *state = calloc(1, (size_t)ws_state_size());
  if (state) ws_state_init(at(state));
  return state;
}
int32_t eg_vertex_floats(void) { return (int32_t)ws_vertex_floats(); }
int32_t eg_max_vertices(void) { return (int32_t)ws_max_vertices(); }
int32_t eg_frame(void *state, float *vertices, float width, float height, float scale, float dt) {
  coax_v_f32 out = {vertices, ws_max_vertices() * ws_vertex_floats()};
  int64_t n = ws_draw(at(state), out, width, height, scale, dt);
  return n < 0 ? 0 : (int32_t)n;
}
void eg_pointer(void *state, float x, float y, int32_t phase) { ws_pointer(at(state), x, y, phase); }
void eg_wheel(void *state, float dy) { ws_wheel(at(state), dy); }
int32_t eg_key(void *state, int32_t key, int32_t down) { return ws_key(at(state), key, down != 0) ? 1 : 0; }
int32_t eg_you(void *state, float *out5) { return ws_you(at(state), (coax_v_f32){out5, 5}) ? 1 : 0; }
int32_t eg_camera(void *state, float width, float height, float scale, float *out16) { return ws_camera(at(state), width, height, scale, (coax_v_f32){out16, 16}) ? 1 : 0; }
void eg_pad(void *state, float x, float y) { ws_pad(at(state), x, y); }
void eg_safe(void *state, float left, float top, float bottom) { ws_safe(at(state), left, top, bottom); }
void eg_turn_offer(void *state) { ws_turn_offer(at(state)); }
int32_t eg_turns(void *state) { return ws_turns(at(state)) ? 1 : 0; }
void eg_turn(void *state, float yaw, float pitch) { ws_turn(at(state), yaw, pitch); }
int32_t eg_tap(void *state, float x, float y) { return ws_tap(at(state), x, y) ? 1 : 0; }
