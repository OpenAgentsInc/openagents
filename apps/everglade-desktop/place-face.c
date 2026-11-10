// Everglade for desktop: face.h's place calls over the C that everglade/workshop/temple.coax compiles to.
#include "temple.h"
#include "face.h"
#include <stdlib.h>

static coax_v_t_Temple pat(void *place) { return (coax_v_t_Temple){(coax_t_Temple *)place, 1}; }

void *plc_new(void) {
  void *place = calloc(1, (size_t)tp_size());
  if (place && !tp_make(pat(place))) { free(place); return 0; }
  return place;
}
int32_t plc_vertex_floats(void) { return (int32_t)tp_vertex_floats(); }
int32_t plc_record_floats(void) { return (int32_t)tp_record_floats(); }
int32_t plc_vertices(void *place) { return (int32_t)tp_vertices(pat(place)); }
int32_t plc_records(void *place) { return (int32_t)tp_records(pat(place)); }
int32_t plc_draws(void *place) { return (int32_t)tp_draws(pat(place)); }
int32_t plc_group_first(void *place, int32_t group) { return (int32_t)tp_group_first(pat(place), group); }
int32_t plc_group_count(void *place, int32_t group) { return (int32_t)tp_group_count(pat(place), group); }
int32_t plc_copy_mesh(void *place, float *out, int32_t floats) { return floats > 0 && tp_copy_mesh(pat(place), (coax_v_f32){out, floats}) ? 1 : 0; }
int32_t plc_copy_records(void *place, float *out, int32_t floats) { return floats > 0 && tp_copy_records(pat(place), (coax_v_f32){out, floats}) ? 1 : 0; }
int32_t plc_copy_draws(void *place, uint32_t *out, int32_t words) { return words > 0 && tp_copy_draws(pat(place), (coax_v_u32){out, words}) ? 1 : 0; }
