// Platform ABI transport only. Layout, event interpretation and admission are Coax.
#include "chat.h"
#include "face.h"
#include <string.h>
static coax_t_ForgeChatEvent event;
static coax_t_ForgeChatModels models;
static coax_t_ForgeVoiceCoder voice;
static coax_t_HudFeed feed;
void fgc_reset(void) { coax_default_t_ForgeChatEvent(&event); coax_default_t_ForgeVoiceCoder(&voice); coax_default_t_HudFeed(&feed); }
int64_t fgv_arm(void) {return fv_arm((coax_v_t_ForgeVoiceCoder){&voice,1});}
void fgv_cancel(void) {fv_cancel((coax_v_t_ForgeVoiceCoder){&voice,1},0);}
int64_t fgv_message(const uint8_t *p,int64_t n,bool available,bool busy) {
  return fv_message((coax_v_t_ForgeVoiceCoder){&voice,1},(coax_string){(const char *)p,n},available,busy);
}
int64_t fgv_copy(uint8_t *p,int64_t cap) {
  if(voice.n>cap) return -1;memcpy(p,voice.text.e,(size_t)voice.n);return voice.n;
}
int64_t fgc_read(const uint8_t *p, int64_t n) {
  return fc_read((coax_v_t_ForgeChatEvent){&event,1}, (coax_string){(const char *)p,n});
}
int64_t fgc_copy(int64_t field, uint8_t *p, int64_t cap) {
  int64_t n = field == 0 ? event.sn : event.tn;
  if(n < 0 || n > cap) return -1;
  memcpy(p, field == 0 ? event.session.e : event.text.e, (size_t)n); return n;
}
bool fgc_send(bool busy, const uint8_t *p, int64_t n) { return fc_can_send(busy,(coax_string){(const char *)p,n}); }
int64_t fgc_router_default(uint8_t *out, int64_t cap) {
  coax_string s=fc_router_default(); if(s.count>cap) return -1;
  memcpy(out,s.data,(size_t)s.count);return s.count;
}
int64_t fgc_chat_mode(bool opened, bool busy,int64_t now) { return fc_chat_mode(opened,busy,(coax_v_t_HudFeed){&feed,1},now); }
void fgc_feed_event(int64_t kind,const uint8_t *p,int64_t n,int64_t now) {
  fc_feed_event((coax_v_t_HudFeed){&feed,1},kind,(coax_string){(const char *)p,n},now);
}
int64_t fgc_feed_copy(uint8_t *p,int64_t cap) {
  coax_string s=fc_feed_text((coax_v_t_HudFeed){&feed,1});
  if(s.count>cap)return -1;memcpy(p,s.data,(size_t)s.count);return s.count;
}
int64_t fgc_launch_option(int64_t i,uint8_t *out,int64_t cap) {
  coax_string s=fc_launch_option(i);if(s.count>cap)return -1;
  memcpy(out,s.data,(size_t)s.count);return s.count;
}
int64_t fgc_feed_draw(float *out,int64_t cap,float w,float h,float d) {
  return fc_feed_draw((coax_v_f32){out,cap},(coax_v_t_HudFeed){&feed,1},w,h,d);
}
int64_t fgc_enter_route(bool ready, bool opened, int64_t key, bool down, bool repeated, bool modified, bool held) {
  return fc_enter_route(ready,opened,key,down,repeated,modified,held);
}
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
