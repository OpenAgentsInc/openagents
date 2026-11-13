// Everglade for desktop: face.h's model calls over the C that model.coax compiles to.
#include "model.h"
#include "face.h"
#include <stdlib.h>

static coax_v_t_WsModel mat(void *model) { return (coax_v_t_WsModel){(coax_t_WsModel *)model, 1}; }

void *egm_new(void) {
  void *model = calloc(1, (size_t)ws_model_size());
  if (model) ws_model_init(mat(model));
  return model;
}
int32_t egm_catalog(void *model, const uint8_t *text, int32_t length) {
  if (length < 0) return 0;
  return ws_model_catalog(mat(model), (coax_string){(const char *)text, length}) ? 1 : 0;
}
int32_t egm_bake_floats(void) { return (int32_t)ws_model_bake_floats(); }
int32_t egm_bake(void *model, const uint8_t *rig, int32_t rig_length, float *out) {
  if (!rig || rig_length <= 0) return 0;
  return ws_model_bake(mat(model), (coax_v_u8){(uint8_t *)rig, rig_length}, (coax_v_f32){out, ws_model_bake_floats()}) ? 1 : 0;
}
int32_t egm_records(void) { return (int32_t)ws_model_records(); }
int32_t egm_record_floats(void) { return (int32_t)ws_model_record_floats(); }
int32_t egm_frame(void *model, const float *you5, float dt, float *records, uint32_t *draws) {
  int64_t n = ws_model_frame(mat(model), (coax_v_f32){(float *)you5, 5}, dt,
    (coax_v_f32){records, ws_model_records() * ws_model_record_floats()}, (coax_v_u32){draws, ws_model_records() * 4});
  return n < 0 ? 0 : (int32_t)n;
}
