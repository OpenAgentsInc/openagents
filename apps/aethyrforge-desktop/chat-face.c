// Platform ABI transport only. Layout, event interpretation and admission are Coax.
#include "chat.h"
#include "face.h"
#include <string.h>
static coax_t_ForgeChatEvent event;
static coax_t_ForgeChatModels models;
void fgc_reset(void) { coax_default_t_ForgeChatEvent(&event); }
int64_t fgc_read(const uint8_t *p, int64_t n) {
  return fc_read((coax_v_t_ForgeChatEvent){&event,1}, (coax_string){(const char *)p,n});
}
int64_t fgc_copy(int64_t field, uint8_t *p, int64_t cap) {
  int64_t n = field == 0 ? event.sn : event.tn;
  if(n < 0 || n > cap) return -1;
  memcpy(p, field == 0 ? event.session.e : event.text.e, (size_t)n); return n;
}
bool fgc_send(bool busy, const uint8_t *p, int64_t n) { return fc_can_send(busy,(coax_string){(const char *)p,n}); }
bool fgc_answer(bool busy, bool pending, const uint8_t *p, int64_t n) { return fc_can_answer(busy,pending,(coax_string){(const char *)p,n}); }
int64_t fgc_catalog(int64_t i, bool label, uint8_t *out, int64_t cap) {
  coax_string s = label ? fc_label(i) : fc_engine(i);
  if(s.count > cap) return -1;
  memcpy(out,s.data,(size_t)s.count); return s.count;
}
void fgc_models_reset(void) { coax_default_t_ForgeChatModels(&models); }
bool fgc_models(const uint8_t *p, int64_t n) {return fc_models((coax_v_t_ForgeChatModels){&models,1},(coax_string){(const char *)p,n});}
int64_t fgc_model(int64_t i, int64_t effort, uint8_t *out, int64_t cap) {
  coax_string s = fc_model((coax_v_t_ForgeChatModels){&models,1},i,effort);
  if(s.count>cap) return -1; memcpy(out,s.data,(size_t)s.count);return s.count;
}
float fgc_box(int64_t kind, float w, float h, int64_t component) {
  coax_t_HudBox b = fc_box(kind,w,h);
  const float v[4] = {b.x,b.y,b.width,b.height}; return v[component];
}
int64_t fgc_draw(float *out, int64_t cap, float w, float h, float d, bool busy) {
  return fc_draw((coax_v_f32){out,cap},w,h,d,busy);
}
