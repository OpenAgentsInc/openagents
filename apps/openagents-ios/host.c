// Firm reason: 6 (native GPU carrier for the compiled chat page; state, layout and paint are Coax).
// The chat page's surface (host.h, GitHub #13645). It is included by the component carrier
// (components/gallery/linux/main.c, COMPONENTS_CHAT), so it shares that file's App, its atlas
// code and the one generated library. It holds no behaviour.
#include "host.h"

typedef struct {
  App app;
  coax_t_ChatPage *s;
  float safe_top, safe_bottom, keyboard, editor_height;
  int focused, reduced;
  int settle;     // frames still owed after a change (glyphs asked for in one frame are drawn in the next)
  void (*changed)(void *context); void *context;
} Chat;

static coax_v_t_ChatPage oc_state(Chat *c) { return (coax_v_t_ChatPage){c->s, 1}; }
static coax_string oc_string(const char *text) { return (coax_string){text ? text : "", text ? (int64_t)strlen(text) : 0}; }
static void oc_changed(Chat *c) { c->settle = 3; }
static void oc_woke(Chat *c) { oc_changed(c); if (c->changed) c->changed(c->context); }

static int oc_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  Chat *c = user;
  chat_env(oc_state(c), c->safe_top, c->safe_bottom, c->keyboard, c->focused != 0, c->editor_height, c->reduced != 0);
  int n = (int)chat_frame(oc_state(c), (coax_v_f32){vertices, (int64_t)c->app.max_vertices * coaxial_components_vertex_floats()}, w, h, dpr, dt);
  if (atlas_serve(&c->app)) oc_changed(c);
  if (c->settle > 0) c->settle--;
  return n < 0 ? 0 : n;
}
static void oc_on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  (void)clicks; (void)mods;
  Chat *c = user;
  chat_pointer(oc_state(c), x, y, phase);
  oc_woke(c);
}
static int oc_on_drive(void *user, const char *op, char *out, int cap) {
  Chat *c = user;
  oc_woke(c);
  return (int)chat_drive(oc_state(c), oc_string(op), (coax_v_u8){(uint8_t *)out, cap});
}

void *oc_open(const char *resources) {
  Chat *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  App *a = &c->app;
  snprintf(a->resources, sizeof a->resources, "%s", resources);
  char shader[PATH_MAX + 32]; snprintf(shader, sizeof shader, "%s/components.metal", resources);
  c->s = calloc(1, sizeof *c->s);
  if (!c->s) { free(c); return NULL; }
  a->atlas = c->s->atlas.e; a->max_vertices = (uint32_t)chat_max_vertices();
  UiApp app = {.title = "OpenAgents", .width = 402, .height = 874, .max_vertices = a->max_vertices, .shader = shader, .user = c,
               .frame = oc_on_frame, .pointer = oc_on_pointer, .drive = oc_on_drive};
  a->host = ui_open(&app, 0);
  if (!a->host) { free(c->s); free(c); return NULL; }
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  a->dpr *= chat_scale();
  if (!atlas_setup(a)) { gallery_free(a); free(c->s); free(c); return NULL; }
  oc_changed(c);
  return c;
}
void *oc_view(void *chat) { return chat ? ui_view(((Chat *)chat)->app.host) : NULL; }
void oc_frame(void *chat) { if (chat) ui_frame(((Chat *)chat)->app.host); }
int oc_moving(void *chat) { Chat *c = chat; return c && (c->settle > 0 || chat_moving(oc_state(c))); }
void oc_on_change(void *chat, void (*changed)(void *context), void *context) { Chat *c = chat; if (c) { c->changed = changed; c->context = context; } }
void oc_env(void *chat, float safe_top, float safe_bottom, float keyboard, int focused, float editor_height, int reduced) {
  Chat *c = chat;
  if (!c) return;
  if (c->safe_top != safe_top || c->safe_bottom != safe_bottom || c->keyboard != keyboard || c->focused != (focused != 0) || c->editor_height != editor_height ||
      c->reduced != (reduced != 0)) oc_changed(c);
  c->safe_top = safe_top; c->safe_bottom = safe_bottom; c->keyboard = keyboard; c->focused = focused != 0; c->editor_height = editor_height;
  c->reduced = reduced != 0;
}
int oc_action(void *chat) { Chat *c = chat; return c ? (int)chat_action(oc_state(c)) : 0; }
int oc_set_text(void *chat, const char *text) { Chat *c = chat; if (!c) return 0; oc_changed(c); return chat_set_text(oc_state(c), oc_string(text)); }
int oc_text(void *chat, char *out, int cap) {
  Chat *c = chat;
  if (!c || cap < 1) return 0;
  int n = (int)chat_text(oc_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
int oc_editor(void *chat, float box[6]) { Chat *c = chat; return c && chat_editor(oc_state(c), (coax_v_f32){box, 6}); }
int oc_style(float style[14]) { return chat_style((coax_v_f32){style, 14}); }
int oc_rows(void *chat) { Chat *c = chat; return c ? (int)chat_rows(oc_state(c)) : 0; }
int oc_rows_stamp(void *chat) { Chat *c = chat; return c ? (int)chat_rows_stamp(oc_state(c)) : 0; }
int oc_row(void *chat, int i, float row[7], char *id, int id_cap, char *label, int label_cap) {
  Chat *c = chat;
  if (!c || id_cap < 1 || label_cap < 1 || !chat_row(oc_state(c), i, (coax_v_f32){row, 7})) return 0;
  id[chat_row_id(oc_state(c), i, (coax_v_u8){(uint8_t *)id, id_cap - 1})] = 0;
  label[chat_row_label(oc_state(c), i, (coax_v_u8){(uint8_t *)label, label_cap - 1})] = 0;
  return 1;
}
int oc_activate(void *chat, const char *id) { Chat *c = chat; if (!c) return 0; oc_changed(c); return chat_activate(oc_state(c), oc_string(id)); }
int oc_refused(void *chat) { Chat *c = chat; return c ? (int)chat_refused(oc_state(c)) : 0; }
unsigned long long oc_presented(void *chat) { return chat ? ui_presented(((Chat *)chat)->app.host) : 0; }
void oc_close(void *chat) {
  Chat *c = chat;
  if (!c) return;
  gallery_free(&c->app);
  free(c->s); free(c);
}
static int oc_bytes(int64_t n, char *out, int cap) { if (cap < 1) return 0; if (n < 0) n = 0; if (n > cap - 1) n = cap - 1; out[n] = 0; return (int)n; }
void oc_attach(void *chat, void *core, const char *kept) {
  Chat *c = chat;
  if (c && core) chat_attach(oc_state(c), (coax_v_t_AppCore){core, 1}, oc_string(kept));
}
void oc_step(void *chat) { Chat *c = chat; if (c) { chat_step(oc_state(c)); oc_changed(c); } }
int oc_draft_rev(void *chat) { Chat *c = chat; return c ? (int)chat_draft_rev(oc_state(c)) : 0; }
int oc_draft(void *chat, char *out, int cap) { Chat *c = chat; return c && cap > 0 ? oc_bytes(chat_draft(oc_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int oc_focus(void *chat, char *out, int cap) { Chat *c = chat; return c && cap > 0 ? oc_bytes(chat_focus(oc_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int oc_field_box(void *chat, float box[4]) { Chat *c = chat; return c && chat_field_box(oc_state(c), (coax_v_f32){box, 4}); }
int oc_field_text(void *chat, char *out, int cap) { Chat *c = chat; return c && cap > 0 ? oc_bytes(chat_field_text(oc_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int oc_set_field(void *chat, const char *text) { Chat *c = chat; if (!c) return 0; oc_changed(c); return chat_set_field(oc_state(c), oc_string(text)); }
int oc_submit(void *chat) { Chat *c = chat; if (!c) return 0; oc_changed(c); return chat_submit(oc_state(c)); }
void oc_blur(void *chat) { Chat *c = chat; if (c) { chat_blur(oc_state(c)); oc_changed(c); } }
#include "core-host.c"
#include "../../components/app/gallery/host.c"
