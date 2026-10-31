// Star Catcher's native face (verse/kms/realm.h) over starcatch-kms.coax and
// starcatch-light.coax. It decides nothing.
#include "realm.h"
#include "starcatch.h"
#include "starcatch-light.h"

static coax_v_t_Catch at(void *state) { return (coax_v_t_Catch){(coax_t_Catch *)state, 1}; }

const char *realm_name(void) { return "star-catcher"; }
size_t realm_state_size(void) { return sizeof(coax_t_Catch); }
int realm_max_vertices(void) { return (int)coaxial_catch_max_vertices(); }
int realm_vertex_floats(void) { return (int)coaxial_catch_vertex_floats(); }
int realm_uniform_floats(void) { return (int)coaxial_catch_uniform_floats(); }

int realm_frame(void *state, float *vertices, float *uniforms, float w, float h, float scale, float dt) {
  coax_v_f32 out = {vertices, coaxial_catch_max_vertices() * coaxial_catch_vertex_floats()};
  coax_v_f32 u = {uniforms, coaxial_catch_uniform_floats()};
  int64_t n = catch_frame(at(state), out, u, w, h, scale, dt, false);
  return n < 0 ? 0 : (int)n;
}
bool realm_key(void *state, int key) { return catch_key(at(state), key, 0); }
bool realm_key_up(void *state, int key) { return catch_key_up(at(state), key, 0); }
int realm_demo_key(void *state) { return (int)catch_demo_key(at(state)); }
int realm_action(void *state) {
  int a = (int)catch_action(at(state));
  if (a) catch_action_done(at(state));
  return a;
}

// The uniforms: size (0, 1), time (2), fade (3), the ground's top (4), the flash (5).
void realm_bg(const float *u, float x, float y, float rgb[3]) {
  coax_t_Rgb c = starcatch_bg(x, y, u[0], u[1], u[2], u[4], u[5]);
  rgb[0] = c.r; rgb[1] = c.g; rgb[2] = c.b;
}
// (px, py): the fragment in the stroke's frame; shape: length, core, light, glow.
float realm_fx(const float *u, float px, float py, const float *shape) {
  return starcatch_fx(px, py, shape[0], shape[1], shape[2], shape[3], u[3]);
}
