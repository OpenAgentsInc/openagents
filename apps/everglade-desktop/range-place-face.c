// Everglade's apps: face.h's range-place calls over the C that everglade/range/place.coax compiles to.
#include "range-place.h"
#include "face.h"
#include <stdlib.h>

static coax_v_t_RangePlace rat(void *place) { return (coax_v_t_RangePlace){(coax_t_RangePlace *)place, 1}; }

void *rpl_new(void) {
  void *place = calloc(1, (size_t)rp_size());
  if (place && !rp_make(rat(place))) { free(place); return 0; }
  return place;
}
int32_t rpl_vertex_floats(void) { return (int32_t)rp_vertex_floats(); }
int32_t rpl_record_floats(void) { return (int32_t)rp_record_floats(); }
int32_t rpl_vertices(void *place) { return (int32_t)rp_vertices(rat(place)); }
int32_t rpl_records(void *place) { return (int32_t)rp_records(rat(place)); }
int32_t rpl_draws(void *place) { return (int32_t)rp_draws(rat(place)); }
int32_t rpl_group_first(void *place, int32_t group) { return (int32_t)rp_group_first(rat(place), group); }
int32_t rpl_group_count(void *place, int32_t group) { return (int32_t)rp_group_count(rat(place), group); }
int32_t rpl_copy_mesh(void *place, float *out, int32_t floats) { return floats > 0 && rp_copy_mesh(rat(place), (coax_v_f32){out, floats}) ? 1 : 0; }
int32_t rpl_copy_records(void *place, float *out, int32_t floats) { return floats > 0 && rp_copy_records(rat(place), (coax_v_f32){out, floats}) ? 1 : 0; }
int32_t rpl_copy_draws(void *place, uint32_t *out, int32_t words) { return words > 0 && rp_copy_draws(rat(place), (coax_v_u32){out, words}) ? 1 : 0; }
