// Everglade for desktop: face.h's Triage calls over the C that triage.coax compiles to (in workshop.coax's unit).
#include "workshop.h"
#include "face.h"
#include <stdlib.h>

static coax_v_t_Triage tat(void *triage) { return (coax_v_t_Triage){(coax_t_Triage *)triage, 1}; }

void *trg_new(void) {
  void *triage = calloc(1, (size_t)tr_size());
  if (triage) tr_init(tat(triage));
  return triage;
}
int32_t trg_vertex_floats(void) { return (int32_t)tr_vertex_floats(); }
int32_t trg_max_vertices(void) { return (int32_t)tr_max_vertices(); }
int32_t trg_frame(void *triage, float *vertices, float *panel4, float width, float height, float scale, float dt) {
  int64_t n = tr_frame(tat(triage), (coax_v_f32){vertices, tr_max_vertices() * tr_vertex_floats()}, (coax_v_f32){panel4, 4}, width, height, scale, dt);
  return n < 0 ? 0 : (int32_t)n;
}
int32_t trg_pointer(void *triage, float x, float y, int32_t phase, float width, float height) { return tr_pointer(tat(triage), x, y, phase, width, height) ? 1 : 0; }
int32_t trg_wheel(void *triage, float x, float y, float dy, float width, float height) { return tr_wheel(tat(triage), x, y, dy, width, height) ? 1 : 0; }
int32_t trg_state(void *triage) { return (int32_t)tr_state(tat(triage)); }
void trg_close(void *triage) { tr_close(tat(triage)); }
