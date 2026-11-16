// Firm reason: 6 (native GPU carrier for the compiled chat page; state, layout and paint are Coax).
// The chat page's surface (host.h, GitHub #13645). It is included by the component carrier
// (components/gallery/linux/main.c, COMPONENTS_CHAT), so it shares that file's App, its atlas
// code and the one generated library. It holds no behaviour.
#include "host.h"
#include "core-host.h"
#include "../shared/host/apple/phone-drive.h"
#include "../shared/host/focus-ring.h"

typedef struct {
  App app;
  coax_t_ChatPage *s;
  float safe_top, safe_bottom, keyboard, editor_height;
  int focused, reduced;
  float zoom;     // the text size the page draws at (oc_text_zoom), 1 the default
  int settle;     // frames still owed after a change (glyphs asked for in one frame are drawn in the next)
  coax_t_OaDrive *drive;   // coax drive's session (debug builds, -OADrive), or NULL
  void (*drove)(void *context); void *drove_context;   // the host's own turn after each operation
  void (*changed)(void *context); void *context;
#ifndef __APPLE__
  coax_t_ChEditor *editor;
#endif
} Chat;

static coax_v_t_ChatPage oc_state(Chat *c) { return (coax_v_t_ChatPage){c->s, 1}; }
static coax_string oc_string(const char *text) { return (coax_string){text ? text : "", text ? (int64_t)strlen(text) : 0}; }
int oa_view_write(const char *email,const char *rejoin,const char *environment,unsigned char *out,int cap) {
 return cap<1 ? -1:(int)avk_write(oc_string(email),oc_string(rejoin),oc_string(environment),(coax_v_u8){out,cap});
}
int oa_view_capacity(void) {return (int)avk_capacity();}
int oa_view_value_capacity(void) {return (int)avk_value_capacity();}
int oa_view_read(const unsigned char *raw,int n,int which,char *out,int cap) {
 if(n<0 || cap<1)return -1;int got=(int)avk_read((coax_string){(const char *)raw,n},which,(coax_v_u8){(uint8_t *)out,cap-1});if(got>=0)out[got]=0;return got;
}
static void oc_changed(Chat *c);
static void oc_woke(Chat *c);
int oi_change(int mode,const char *command,const char *expected,const char *account,const unsigned char *state,int state_n,const char *input,char *out,int cap) {
 if (cap<1) return -1;int n=(int)ids_change(mode,oc_string(command),oc_string(expected),oc_string(account),(coax_string){(const char *)state,state_n},oc_string(input),(coax_v_u8){(uint8_t *)out,cap-1});if(n>=0) out[n]=0;return n;
}
int oi_backup(const unsigned char *raw,int n,const char *expected,char *out,int cap) {
 if(cap<1 || n<0) return -1;int count=(int)ids_backup((coax_string){(const char *)raw,n},oc_string(expected),(coax_v_u8){(uint8_t *)out,cap-1});if(count>=0) out[count]=0;return count;
}
int oi_public(const unsigned char *raw,int n,unsigned char *out,int cap) {return n<0 ? -1:(int)ids_public((coax_string){(const char *)raw,n},(coax_v_u8){out,cap});}
int oi_identity(const char *account,const char *pubkey,unsigned char *out,int cap) {return (int)ids_identity(oc_string(account),oc_string(pubkey),(coax_v_u8){out,cap});}
int oi_http(int method,const char *path,int bytes) {return ids_http(method,oc_string(path),bytes);}
void oc_secret_ready(void *chat,int ready) {Chat *c=chat;if(c) {chat_secret_ready(oc_state(c),ready!=0);oc_changed(c);}}
int oc_private_mode(void *chat) {Chat *c=chat;return c ? (int)chat_private_mode(oc_state(c)):0;}
void oc_scroll_to(void *chat,float points) {Chat *c=chat;if(c) {chat_scroll_to(oc_state(c),points/chat_ui_scale(oc_state(c)));oc_woke(c);}}
int oc_scroll_info(void *chat,float out[3]) {Chat *c=chat;return c && chat_scroll_info(oc_state(c),(coax_v_f32){out,3});}
void oc_native_scroll(void *chat,int on) {Chat *c=chat;if(c) chat_native_scroll(oc_state(c),on!=0);}
int oc_row_visible(void *chat,int row) {Chat *c=chat;return c && chat_row_visible(oc_state(c),row);}
static void oc_changed(Chat *c) { c->settle = 3; }
static void oc_woke(Chat *c) { oc_changed(c); if (c->changed) c->changed(c->context); }

static int oc_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  Chat *c = user;
  if (c->drive) dt = oa_drive_dt((coax_v_t_OaDrive){c->drive, 1}, dt);   // the driver holds the clock
#ifndef __APPLE__
  c->focused=che_sync((coax_v_t_ChEditor){c->editor,1},oc_state(c));
#endif
  chat_env(oc_state(c), c->safe_top, c->safe_bottom, c->keyboard, c->focused != 0, c->editor_height, c->reduced != 0);
  int n = (int)chat_frame(oc_state(c), (coax_v_f32){vertices, (int64_t)c->app.max_vertices * coaxial_components_vertex_floats()}, w, h, dpr, dt);
#ifndef __APPLE__
  n=(int)che_paint((coax_v_t_ChEditor){c->editor,1},oc_state(c),(coax_v_f32){vertices,(int64_t)c->app.max_vertices*coaxial_components_vertex_floats()},n,dt);
  float ring[4],style[14];
  if(che_ring((coax_v_t_ChEditor){c->editor,1},oc_state(c),(coax_v_f32){ring,4}) && chat_style((coax_v_f32){style,14}))n=(int)ui_focus_ring(vertices,(uint32_t)n,c->app.max_vertices,dpr,ring[0],ring[1],ring[2],ring[3],style+10);
#endif
  if (atlas_serve(&c->app)) oc_changed(c);
  if (c->settle > 0) c->settle--;
  return n < 0 ? 0 : n;
}
static void oc_on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  (void)clicks; (void)mods;
  Chat *c = user;
#ifndef __APPLE__
  if(che_pointer((coax_v_t_ChEditor){c->editor,1},oc_state(c),x,y,phase,clicks,mods)){oc_woke(c);return;}
#endif
  chat_pointer(oc_state(c), x, y, phase);
  oc_woke(c);
}
static int oc_on_drive(void *user, const char *op, char *out, int cap) {
  Chat *c = user;
  oc_woke(c);
  return (int)chat_drive(oc_state(c), oc_string(op), (coax_v_u8){(uint8_t *)out, cap});
}

static int oc_access_count(void *user) {return (int)chat_rows(oc_state(user));}
static int oc_access_read(void *user,int row,UiElement *out) {
  Chat *c=user;float facts[8];memset(out,0,sizeof *out);
  if (!chat_row(oc_state(c),row,(coax_v_f32){facts,8})) return 0;
  if (!(facts[2]>0 && facts[3]>0)) return 0;
  memcpy(out->box,facts,sizeof out->box);out->role=(int)facts[4];out->state=(int)facts[5];out->parent=(int)facts[6];out->focus=(int)facts[7];
  int n=(int)chat_row_id(oc_state(c),row,(coax_v_u8){(uint8_t *)out->id,sizeof out->id-1});if(n<1)return 0;out->id[n]=0;
  n=(int)chat_row_label(oc_state(c),row,(coax_v_u8){(uint8_t *)out->label,sizeof out->label-1});if(n<0)return 0;out->label[n]=0;
  n=(int)chat_row_value(oc_state(c),row,(coax_v_u8){(uint8_t *)out->value,sizeof out->value-1});if(n<0)return 0;out->value[n]=0;
  return 1;
}
static int oc_access_press(void *user,const char *id) {Chat *c=user;int pressed=chat_activate(oc_state(c),oc_string(id));if(pressed)oc_woke(c);return pressed;}

static void oc_window_changed(void *user) {oc_woke(user);}
#ifndef __APPLE__
// Exact SDK callbacks, also callable by the hidden Vulkan qualification.
int oc_input_key(void *chat,int key,int mods,int down) {
 Chat*c=chat;if(!c||!down)return 0;coax_v_t_ChEditor editor={c->editor,1};char copy[16384];
 int n=(int)che_copy(editor,oc_state(c),key,mods,(coax_v_u8){(uint8_t*)copy,sizeof copy});
 if(n>0){if(ui_copy(c->app.host,copy,(size_t)n))(void)che_copied(editor,oc_state(c),key,mods);oc_woke(c);return 1;}
 int took=che_key(editor,oc_state(c),key,mods);if(took)oc_woke(c);return took;
}
int oc_input_text(void *chat,const char *text,size_t n) {Chat*c=chat;if(!c||!text)return 0;int took=che_insert((coax_v_t_ChEditor){c->editor,1},oc_state(c),(coax_string){text,(int64_t)n});if(took)oc_woke(c);return took;}
int oc_input_paste(void *chat,const char *text,size_t n) {Chat*c=chat;if(!c||!text)return 0;int took=che_paste((coax_v_t_ChEditor){c->editor,1},oc_state(c),(coax_string){text,(int64_t)n});if(took)oc_woke(c);return took;}
int oc_input_edit(void *chat,const char *commit,size_t n,const char *preedit,size_t m,int begin,int end) {Chat*c=chat;if(!c)return 0;int took=che_edit((coax_v_t_ChEditor){c->editor,1},oc_state(c),(coax_string){commit,(int64_t)n},(coax_string){preedit,(int64_t)m},begin,end);if(took)oc_woke(c);return took;}
static int oc_input_wants(void *chat) {Chat*c=chat;return che_sync((coax_v_t_ChEditor){c->editor,1},oc_state(c));}
static int oc_input_rect(void *chat,float out[4]) {Chat*c=chat;return che_rect((coax_v_t_ChEditor){c->editor,1},oc_state(c),(coax_v_f32){out,4});}
static uint64_t oc_input_owner(void *chat) {Chat*c=chat;(void)oc_input_wants(c);return che_owner((coax_v_t_ChEditor){c->editor,1});}
static void oc_input_cancel(void *chat) {Chat*c=chat;che_cancel((coax_v_t_ChEditor){c->editor,1});oc_woke(c);}
static void oc_input_focus(void *chat,int on) {Chat*c=chat;che_focus((coax_v_t_ChEditor){c->editor,1},on!=0);oc_woke(c);}
static int oc_sdk_key(void*u,int k,int m,int d) {return oc_input_key(u,k,m,d);}
static void oc_sdk_text(void*u,const char*t,size_t n) {(void)oc_input_text(u,t,n);}
static void oc_sdk_paste(void*u,const char*t,size_t n) {(void)oc_input_paste(u,t,n);}
static void oc_sdk_edit(void*u,const char*c,size_t n,const char*p,size_t m,int a,int b) {(void)oc_input_edit(u,c,n,p,m,a,b);}
#endif
static void oc_wheel(void *user,float dy) {Chat *c=user;float f[3];if(oc_scroll_info(c,f) && f[0])oc_scroll_to(c,f[1]+dy/ui_dpr(c->app.host));}
void *oc_open_window(const char *resources,int width,int height,int hidden) {
  Chat *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  App *a = &c->app;
  snprintf(a->resources, sizeof a->resources, "%s", resources);
  char shader[PATH_MAX + 32]; snprintf(shader, sizeof shader,
#ifdef __APPLE__
  "%s/components.metal",
#else
  "%s/components.spv",
#endif
  resources);
  c->s = calloc(1, sizeof *c->s);
  if (!c->s) { free(c); return NULL; }
#ifndef __APPLE__
  c->editor=calloc(1,sizeof *c->editor);if(!c->editor){free(c->s);free(c);return NULL;}
  che_open((coax_v_t_ChEditor){c->editor,1});
#endif
  a->atlas = c->s->atlas.e; a->max_vertices = (uint32_t)chat_max_vertices();
  UiApp app = {.title = "OpenAgents", .app_id="com.openagents.desktop", .width = width, .height = height, .hidden=hidden, .min_width=360,.min_height=480,.external_clock=1,.changed=oc_window_changed,.max_vertices = a->max_vertices, .shader = shader, .user = c,
               .frame = oc_on_frame, .pointer = oc_on_pointer, .drive = oc_on_drive,.wheel=oc_wheel,.busy=oc_moving,
               .access_capacity=(uint32_t)chat_access_capacity(),.elements=oc_access_count,.element=oc_access_read,.activate=oc_access_press};
#ifndef __APPLE__
  app.key=oc_sdk_key;app.text=oc_sdk_text;app.wants_text=oc_input_wants;app.text_rect=oc_input_rect;app.text_owner=oc_input_owner;app.text_edit=oc_sdk_edit;app.text_cancel=oc_input_cancel;app.text_paste=oc_sdk_paste;app.focus=oc_input_focus;
#endif
  a->host = ui_open(&app,
#ifdef __APPLE__
  0
#else
  hidden!=0
#endif
  );
  if (!a->host) {
#ifndef __APPLE__
    free(c->editor);
#endif
    free(c->s); free(c); return NULL;
  }
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  a->dpr *= chat_scale();
  if (!atlas_setup(a)) {
    gallery_free(a);
#ifndef __APPLE__
    free(c->editor);
#endif
    free(c->s); free(c); return NULL;
  }
  oc_changed(c);
  return c;
}
void *oc_open(const char *resources) {return oc_open_window(resources,402,874,0);}
#ifdef __APPLE__
void *oc_window(void *chat) {return chat ? ui_window(((Chat *)chat)->app.host):NULL;}
#else
void *oc_window(void *chat) {(void)chat;return NULL;}
#endif
int oc_drive(void *chat,const char *op,char *out,int cap) {
#ifdef OPENAGENTS_PHONE_DRIVE
 (void)chat;(void)op;(void)out;(void)cap;return -1;
#else
 return chat ? ui_drive(((Chat *)chat)->app.host,op,out,cap):-1;
#endif
}
#ifdef __APPLE__
void oc_access_editor(void *chat,const char *id,void *view) {if(chat)ui_access_editor(((Chat *)chat)->app.host,id,view);}
void *oc_view(void *chat) { return chat ? ui_view(((Chat *)chat)->app.host) : NULL; }
#else
void oc_access_editor(void *chat,const char *id,void *view) {(void)chat;(void)id;(void)view;}
void *oc_view(void *chat) {(void)chat;return NULL;}
#endif
void oc_frame(void *chat) { if (chat) ui_frame(((Chat *)chat)->app.host); }
int oc_shot_frame(void *chat,int width,int height,float dt) {return chat && width>0 && height>0 && ui_shot_frame(((Chat *)chat)->app.host,(uint32_t)width,(uint32_t)height,dt);}
int oc_shot_write(void *chat,const char *path) {return chat && path && ui_shot_write(((Chat *)chat)->app.host,path);}
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
int oc_row(void *chat, int i, float row[8], char *id, int id_cap, char *label, int label_cap) {
  Chat *c = chat;
  if (!c || id_cap < 1 || label_cap < 1 || !chat_row(oc_state(c), i, (coax_v_f32){row, 8})) return 0;
  id[chat_row_id(oc_state(c), i, (coax_v_u8){(uint8_t *)id, id_cap - 1})] = 0;
  label[chat_row_label(oc_state(c), i, (coax_v_u8){(uint8_t *)label, label_cap - 1})] = 0;
  return 1;
}
int oc_activate(void *chat, const char *id) { Chat *c = chat; if (!c) return 0; oc_changed(c); return chat_activate(oc_state(c), oc_string(id)); }
int oc_layer(void *chat) { Chat *c = chat; return c ? (int)chat_layer(oc_state(c)) : 0; }
int oc_scroll_page(void *chat, int dir) { Chat *c = chat; if (!c || !chat_scroll_page(oc_state(c), dir)) return 0; oc_changed(c); return 1; }
// The atlas again at the page's new scale: the fonts and icons are read and rasterized afresh.
static int oc_atlas_again(Chat *c, float zoom) {
  App *a = &c->app;
  for (int i = 0; i < 4; i++) { free(a->font_bytes[i]); a->font_bytes[i] = NULL; }
  for (int i = 0; i < 64; i++) { free(a->svg[i]); a->svg[i] = NULL; a->svg_n[i] = 0; }
  free(a->mono); free(a->color); free(a->work); a->mono = a->color = NULL; a->work = NULL;
  memset(&a->plan, 0, sizeof a->plan); memset(&a->fonts, 0, sizeof a->fonts); memset(&c->s->atlas, 0, sizeof c->s->atlas);
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  a->dpr *= chat_scale() * zoom;
  return atlas_setup(a);
}
float oc_text_zoom(void *chat, float zoom) {
  Chat *c = chat;
  if (!c) return 1;
  float was = c->zoom > 0 ? c->zoom : 1, now = chat_text_zoom(oc_state(c), zoom);
  if (now == was) return now;
  // the glyph atlas holds every face at every size at the device's scale; past the largest scale it
  // fits in, the page draws at the largest text size that fits
  while (!oc_atlas_again(c, now)) {
    if (now <= 1) { c->zoom = 1; oc_changed(c); return 1; }
    now = chat_text_zoom(oc_state(c), now - 0.05f > 1 ? now - 0.05f : 1);
  }
  c->zoom = now; oc_changed(c);
  return now;
}
int oc_refused(void *chat) { Chat *c = chat; return c ? (int)chat_refused(oc_state(c)) : 0; }
unsigned long long oc_presented(void *chat) {
#ifdef __APPLE__
 return chat ? ui_presented(((Chat *)chat)->app.host) : 0;
#else
 (void)chat;return 0;
#endif
}
void oc_close(void *chat) {
  Chat *c = chat;
  if (!c) return;
  free(c->drive);
  oa_core_on_guest(NULL, NULL);
  gallery_free(&c->app);
#ifndef __APPLE__
  ue_close((coax_v_t_UiEditor){c->editor->edit.e,1});free(c->editor);
#endif
  free(c->s); free(c);
}
static int oc_bytes(int64_t n, char *out, int cap) { if (cap < 1) return 0; if (n < 0) n = 0; if (n > cap - 1) n = cap - 1; out[n] = 0; return (int)n; }
static void oc_guest(void *chat, const uint8_t *body, int64_t n) {
  Chat *c = chat;
  if (c) { chat_guest(oc_state(c), (coax_string){body ? (const char *)body : "", body ? n : 0}); oc_changed(c); }
}
void oc_attach(void *chat, void *core, const char *kept) {
  Chat *c = chat;
  if (!c || !core) return;
  chat_attach(oc_state(c), (coax_v_t_AppCore){core, 1}, oc_string(kept));
  oa_core_on_guest(oc_guest, c);
}
// coax drive (debug builds): phone-drive.c answers on the main thread, so these run there.
static Chat *oc_driven;
static int oc_drive_answer(const char *op, char *out, int cap) {
  Chat *c = oc_driven;
  if (!c || !c->drive || cap < 1) return -1;
  int n = (int)oa_drive_op((coax_v_t_OaDrive){c->drive, 1}, oc_state(c), oc_string(op), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0; oc_woke(c);
  if (c->drove) c->drove(c->drove_context);
  return n;
}
static void oc_drive_step(float dt) {
  Chat *c = oc_driven;
  if (c && c->drive) { oa_drive_frame((coax_v_t_OaDrive){c->drive, 1}, oc_state(c), dt); oc_woke(c); }
}
void oc_on_drive_op(void *chat, void (*after)(void *context), void *context) { Chat *c = chat; if (c) { c->drove = after; c->drove_context = context; } }
int oc_drive_start(void *chat, const char *where, const char *token, float w, float h, float dpr) {
  Chat *c = chat;
  if (!c || c->drive || !where || !*where) return 0;
  c->drive = calloc(1, sizeof *c->drive);
  if (!c->drive) return 0;
  oa_drive_open((coax_v_t_OaDrive){c->drive, 1}, oc_state(c), w, h, dpr);
  oc_driven = c;
#ifdef OPENAGENTS_PHONE_DRIVE
  if (!phone_drive_start(where, token, oc_drive_answer, oc_drive_step)) { oc_driven = NULL; free(c->drive); c->drive = NULL; return 0; }
#else
  // Desktop surfaces use the shared host's driver; they do not link the iPhone socket adapter.
  oc_driven=NULL;free(c->drive);c->drive=NULL;return 0;
#endif
  return 1;
}
int oc_env_save(void *chat, char *out, int cap) { Chat *c = chat; return c && cap > 0 ? oc_bytes(chat_env_save(oc_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
void oc_env_restore(void *chat, const char *text) { Chat *c = chat; if (c) { chat_env_restore(oc_state(c), oc_string(text)); oc_changed(c); } }
void oc_step(void *chat) { Chat *c = chat; if (c) { chat_step(oc_state(c)); oc_changed(c); } }
int oc_draft_rev(void *chat) { Chat *c = chat; return c ? (int)chat_draft_rev(oc_state(c)) : 0; }
int oc_draft(void *chat, char *out, int cap) { Chat *c = chat; return c && cap > 0 ? oc_bytes(chat_draft(oc_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int oc_focus(void *chat, char *out, int cap) { Chat *c = chat; return c && cap > 0 ? oc_bytes(chat_focus(oc_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int oc_field_box(void *chat, float box[4]) { Chat *c = chat; return c && chat_field_box(oc_state(c), (coax_v_f32){box, 4}); }
int oc_field_kind(void *chat) {Chat*c=chat;return c?(int)chat_field_kind(oc_state(c)):-1;}
int oc_field_text_box(void *chat,float box[4]) {Chat*c=chat;return c && chat_field_text_box(oc_state(c),(coax_v_f32){box,4});}
int oc_field_text(void *chat, char *out, int cap) { Chat *c = chat; return c && cap > 0 ? oc_bytes(chat_field_text(oc_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int oc_set_field(void *chat, const char *text) { Chat *c = chat; if (!c) return 0; oc_changed(c); return chat_set_field(oc_state(c), oc_string(text)); }
int oc_submit(void *chat) { Chat *c = chat; if (!c) return 0; oc_changed(c); return chat_submit(oc_state(c)); }
void oc_blur(void *chat) { Chat *c = chat; if (c) { chat_blur(oc_state(c)); oc_changed(c); } }
void oc_native(void *chat, int on) { Chat *c = chat; if (c) { chat_native(oc_state(c), on != 0); oc_changed(c); } }
int oc_menu(void *chat, float out[6]) { Chat *c = chat; return c && chat_menu(oc_state(c), (coax_v_f32){out, 6}); }
int oc_sheet(void *chat) { Chat *c = chat; return c ? (int)chat_sheet(oc_state(c)) : 0; }
int oc_find(void *chat, const char *id, float box[4]) { Chat *c = chat; return c && chat_find(oc_state(c), oc_string(id), (coax_v_f32){box, 4}); }
int oc_row_value(void *chat, int i, char *out, int cap) { Chat *c = chat; return c && cap > 0 ? oc_bytes(chat_row_value(oc_state(c), i, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
#include "core-host.c"
#include "../../components/app/gallery/host.c"

int oi_cancel(const unsigned char *raw,int n,unsigned char *out,int cap) {return (int)ids_cancel((coax_string){(const char *)raw,n},(coax_v_u8){out,cap});}
