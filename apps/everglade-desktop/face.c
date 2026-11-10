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
