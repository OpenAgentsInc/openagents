// Everglade for desktop: face.h's Next issue calls over the C that next.coax compiles to (in workshop.coax's unit).
#include "workshop.h"
#include "face.h"
#include <stdlib.h>

static coax_v_t_NextPanel nat(void *panel) { return (coax_v_t_NextPanel){(coax_t_NextPanel *)panel, 1}; }

void *nxp_new(void) {
  void *panel = calloc(1, (size_t)np_size());
  if (panel) np_init(nat(panel));
  return panel;
}
int32_t nxp_vertex_floats(void) { return (int32_t)np_vertex_floats(); }
int32_t nxp_max_vertices(void) { return (int32_t)np_max_vertices(); }
int32_t nxp_frame(void *panel, float *vertices, float *panel4, float width, float height, float scale, float dt, int64_t now) {
  int64_t n = np_frame(nat(panel), (coax_v_f32){vertices, np_max_vertices() * np_vertex_floats()}, (coax_v_f32){panel4, 4}, width, height, scale, dt, now);
  return n < 0 ? 0 : (int32_t)n;
}
int32_t nxp_pointer(void *panel, float x, float y, int32_t phase, float width, float height) { return np_pointer(nat(panel), x, y, phase, width, height) ? 1 : 0; }
int32_t nxp_wheel(void *panel, float x, float y, float dy, float width, float height) { return np_wheel(nat(panel), x, y, dy, width, height) ? 1 : 0; }
int32_t nxp_wants_keys(void *panel) { return np_wants_keys(nat(panel)) ? 1 : 0; }
void nxp_key(void *panel, int32_t key) { np_key(nat(panel), key); }
int32_t nxp_take_request(void *panel) { return (int32_t)np_take_request(nat(panel)); }
void nxp_context(void *panel, const uint8_t *text, int32_t length, const uint8_t *report, int32_t report_length, int32_t ok) {
  if (length < 0 || report_length < 0) return;
  np_context(nat(panel), (coax_string){(const char *)text, length}, (coax_string){(const char *)report, report_length}, ok != 0);
}
int32_t nxp_chosen(void *panel) { return (int32_t)np_chosen(nat(panel)); }
void nxp_work(void *panel, const uint8_t *text, int32_t length, const uint8_t *report, int32_t report_length, int32_t ok) {
  if (length < 0 || report_length < 0) return;
  np_work(nat(panel), (coax_string){(const char *)text, length}, (coax_string){(const char *)report, report_length}, ok != 0);
}
int32_t nxp_take_usage(void *panel, uint8_t *out, int32_t cap) { return cap <= 0 ? 0 : (int32_t)np_take_usage(nat(panel), (coax_v_u8){out, cap}); }
int32_t nxp_take_decision(void *panel, uint8_t *out, int32_t cap) { return cap <= 0 ? 0 : (int32_t)np_take_decision(nat(panel), (coax_v_u8){out, cap}); }
int32_t nxp_state(void *panel) { return (int32_t)np_state(nat(panel)); }
void nxp_close(void *panel) { np_close(nat(panel)); }
