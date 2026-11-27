// Firm reason: 6 (thin native GPU carrier for the compiled Coax iOS page; included by gpu-carrier.c).
#include "gpu-host.h"
#include "../shared/host/apple/ui-host-apple.h"

typedef struct {
  App app;
  coax_t_IosPhonePage *s;
  float safe_top, safe_bottom, keyboard, editor_height;
  int focused, reduced;
  float zoom;
  int settle;
  void (*changed)(void *context);
  void *context;
} IosGpuHost;

static coax_v_t_IosPhonePage oa_state(IosGpuHost *c) { return (coax_v_t_IosPhonePage){c->s, 1}; }
static coax_string oa_str(const char *s) { return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0}; }
static int oa_bytes(int64_t n, char *out, int cap) {
  if (cap < 1) return 0;
  if (n < 0) n = 0;
  if (n > cap - 1) n = cap - 1;
  out[n] = 0;
  return (int)n;
}
static void oa_changed(IosGpuHost *c) { if (c) c->settle = 3; }
static void oa_woke(IosGpuHost *c) {
  if (!c) return;
  oa_changed(c);
  if (c->changed) c->changed(c->context);
}

static int oa_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  IosGpuHost *c = user;
  ios_env(oa_state(c), c->safe_top, c->safe_bottom, c->keyboard, c->focused != 0, c->editor_height, c->reduced != 0);
  int n = (int)ios_frame(oa_state(c), (coax_v_f32){vertices, (int64_t)c->app.max_vertices * coaxial_components_vertex_floats()}, w, h, dpr, dt);
  if (atlas_serve(&c->app)) oa_changed(c);
  if (c->settle > 0) c->settle--;
  return n < 0 ? 0 : n;
}
static void oa_on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  (void)clicks; (void)mods;
  IosGpuHost *c = user;
  ios_pointer(oa_state(c), x, y, phase);
  oa_woke(c);
}
static void oa_on_wheel(void *user, float dy) {
  IosGpuHost *c = user;
  float f[3];
  if (oa_ios_scroll_info(c, f) && f[0]) oa_ios_scroll_to(c, f[1] + dy / ui_dpr(c->app.host));
}
static void oa_on_window_changed(void *user) { oa_woke(user); }

static int oa_access_count(void *user) { return (int)ios_rows(oa_state(user)); }
static int oa_access_read(void *user, int row, UiElement *out) {
  IosGpuHost *c = user;
  float facts[8];
  memset(out, 0, sizeof *out);
  if (!ios_row(oa_state(c), row, (coax_v_f32){facts, 8})) return 0;
  if (!(facts[2] > 0 && facts[3] > 0)) return 0;
  memcpy(out->box, facts, sizeof out->box);
  out->role = (int)facts[4]; out->state = (int)facts[5];
  out->parent = (int)facts[6]; out->focus = (int)facts[7];
  int n = (int)ios_row_id(oa_state(c), row, (coax_v_u8){(uint8_t *)out->id, sizeof out->id - 1});
  if (n < 1) return 0;
  out->id[n] = 0;
  n = (int)ios_row_label(oa_state(c), row, (coax_v_u8){(uint8_t *)out->label, sizeof out->label - 1});
  if (n < 0) return 0;
  out->label[n] = 0;
  n = (int)ios_row_value(oa_state(c), row, (coax_v_u8){(uint8_t *)out->value, sizeof out->value - 1});
  if (n < 0) return 0;
  out->value[n] = 0;
  return 1;
}
static int oa_access_press(void *user, const char *id) {
  IosGpuHost *c = user;
  int pressed = ios_activate(oa_state(c), oa_str(id));
  if (pressed) oa_woke(c);
  return pressed;
}

void *oa_ios_open(const char *resources) {
  IosGpuHost *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  App *a = &c->app;
  snprintf(a->resources, sizeof a->resources, "%s", resources ? resources : "");
  char shader[PATH_MAX + 32];
  snprintf(shader, sizeof shader, "%s/components.metal", a->resources);
  c->s = calloc(1, sizeof *c->s);
  if (!c->s) { free(c); return NULL; }
  c->zoom = 1.0f;
  a->atlas = c->s->atlas.e;
  a->max_vertices = (uint32_t)ios_max_vertices();
  UiApp app = {
    .title = "OpenAgents",
    .app_id = "com.openagents.app",
    .width = 402,
    .height = 874,
    .hidden = 0,
    .min_width = 320,
    .min_height = 480,
    .external_clock = 1,
    .changed = oa_on_window_changed,
    .max_vertices = a->max_vertices,
    .shader = shader,
    .user = c,
    .frame = oa_on_frame,
    .pointer = oa_on_pointer,
    .wheel = oa_on_wheel,
    .busy = oa_ios_moving,
    .access_capacity = (uint32_t)ios_access_capacity(),
    .elements = oa_access_count,
    .element = oa_access_read,
    .activate = oa_access_press,
  };
  a->host = ui_open(&app, 0);
  if (!a->host) { free(c->s); free(c); return NULL; }
  a->dpr = ui_dpr(a->host);
  if (a->dpr < 1) a->dpr = 1;
  a->dpr *= ios_scale();
  if (!atlas_setup(a)) {
    gallery_free(a);
    free(c->s); free(c);
    return NULL;
  }
  oa_changed(c);
  return c;
}

void oa_ios_close(void *surface) {
  IosGpuHost *c = surface;
  if (!c) return;
  gallery_free(&c->app);
  free(c->s);
  free(c);
}

void *oa_ios_view(void *surface) { return surface ? ui_view(((IosGpuHost *)surface)->app.host) : NULL; }
void oa_ios_frame(void *surface) { if (surface) ui_frame(((IosGpuHost *)surface)->app.host); }
int oa_ios_moving(void *surface) { IosGpuHost *c = surface; return c && (c->settle > 0 || ios_moving(oa_state(c))); }
void oa_ios_on_change(void *surface, void (*changed)(void *context), void *context) {
  IosGpuHost *c = surface;
  if (c) { c->changed = changed; c->context = context; }
}

void oa_ios_env(void *surface, float safe_top, float safe_bottom, float keyboard, int focused, float editor_height, int reduced) {
  IosGpuHost *c = surface;
  if (!c) return;
  if (c->safe_top != safe_top || c->safe_bottom != safe_bottom || c->keyboard != keyboard ||
      c->focused != (focused != 0) || c->editor_height != editor_height || c->reduced != (reduced != 0)) {
    oa_changed(c);
  }
  c->safe_top = safe_top; c->safe_bottom = safe_bottom; c->keyboard = keyboard;
  c->focused = focused != 0; c->editor_height = editor_height; c->reduced = reduced != 0;
}
void oa_ios_transparency(void *surface, int reduced) {
  IosGpuHost *c = surface;
  if (c) ios_transparency(oa_state(c), reduced != 0);
}

static int oa_ios_atlas_again(IosGpuHost *c, float zoom) {
  App *a = &c->app;
  for (int i = 0; i < 4; i++) { free(a->font_bytes[i]); a->font_bytes[i] = NULL; }
  free(a->fallback_bytes); a->fallback_bytes = NULL;
  for (int i = 0; i < 80; i++) { free(a->svg[i]); a->svg[i] = NULL; a->svg_n[i] = 0; }
  free(a->mono); free(a->color); free(a->work);
  a->mono = a->color = NULL; a->work = NULL;
  memset(&a->plan, 0, sizeof a->plan);
  memset(&a->fonts, 0, sizeof a->fonts);
  memset(&c->s->atlas, 0, sizeof c->s->atlas);
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  a->dpr *= ios_scale() * zoom;
  return atlas_setup(a);
}

float oa_ios_text_zoom(void *surface, float zoom) {
  IosGpuHost *c = surface;
  if (!c) return 1.0f;
  float was = c->zoom > 0 ? c->zoom : 1.0f;
  float now = ios_text_zoom(oa_state(c), zoom);
  if (now == was) return now;
  while (!oa_ios_atlas_again(c, now)) {
    if (now <= 1.0f) { c->zoom = 1.0f; oa_changed(c); return 1.0f; }
    now = ios_text_zoom(oa_state(c), now - 0.05f > 1.0f ? now - 0.05f : 1.0f);
  }
  c->zoom = now;
  oa_changed(c);
  return now;
}

void oa_ios_native_scroll(void *surface, int on) { IosGpuHost *c = surface; if (c) ios_native_scroll(oa_state(c), on != 0); }
int oa_ios_scroll_info(void *surface, float out[3]) { IosGpuHost *c = surface; return c && ios_scroll_info(oa_state(c), (coax_v_f32){out, 3}); }
void oa_ios_scroll_to(void *surface, float points) { IosGpuHost *c = surface; if (c) { ios_scroll_to(oa_state(c), points); oa_woke(c); } }
int oa_ios_scroll_page(void *surface, int dir) { IosGpuHost *c = surface; if (!c || !ios_scroll_page(oa_state(c), dir)) return 0; oa_changed(c); return 1; }

int oa_ios_editor(void *surface, float box[6]) { IosGpuHost *c = surface; return c && ios_editor(oa_state(c), (coax_v_f32){box, 6}); }
int oa_ios_style(float style[14]) { return ios_style((coax_v_f32){style, 14}); }
int oa_ios_focus(void *surface, char *out, int cap) { IosGpuHost *c = surface; return c && cap > 0 ? oa_bytes(ios_focus(oa_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
void oa_ios_set_focus(void *surface, const char *id) { IosGpuHost *c = surface; if (c) { ios_set_focus(oa_state(c), oa_str(id)); oa_woke(c); } }
void oa_ios_blur(void *surface) { IosGpuHost *c = surface; if (c) { ios_blur(oa_state(c)); oa_changed(c); } }
int oa_ios_draft_set(void *surface, const char *text) { IosGpuHost *c = surface; if (!c) return 0; oa_changed(c); return ios_draft_set(oa_state(c), oa_str(text)); }
int oa_ios_draft_get(void *surface, char *out, int cap) { IosGpuHost *c = surface; return c && cap > 0 ? oa_bytes(ios_draft_get(oa_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }

int oa_ios_rows(void *surface) { IosGpuHost *c = surface; return c ? (int)ios_rows(oa_state(c)) : 0; }
int oa_ios_rows_stamp(void *surface) { IosGpuHost *c = surface; return c ? (int)ios_rows_stamp(oa_state(c)) : 0; }
int oa_ios_layer(void *surface) { IosGpuHost *c = surface; return c ? (int)ios_layer(oa_state(c)) : 0; }
int oa_ios_row(void *surface, int i, float row[8], char *id, int id_cap, char *label, int label_cap) {
  IosGpuHost *c = surface;
  if (!c || id_cap < 1 || label_cap < 1 || !ios_row(oa_state(c), i, (coax_v_f32){row, 8})) return 0;
  oa_bytes(ios_row_id(oa_state(c), i, (coax_v_u8){(uint8_t *)id, id_cap - 1}), id, id_cap);
  oa_bytes(ios_row_label(oa_state(c), i, (coax_v_u8){(uint8_t *)label, label_cap - 1}), label, label_cap);
  return 1;
}
int oa_ios_row_value(void *surface, int i, char *out, int cap) { IosGpuHost *c = surface; return c && cap > 0 ? oa_bytes(ios_row_value(oa_state(c), i, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int oa_ios_activate(void *surface, const char *id) { IosGpuHost *c = surface; if (!c) return 0; int ok = ios_activate(oa_state(c), oa_str(id)); oa_woke(c); return ok; }

int oa_ios_take_action(void *surface) { IosGpuHost *c = surface; return c ? (int)ios_take_action(oa_state(c)) : OA_IOS_ACT_NONE; }
int oa_ios_action_arg(void *surface, char *out, int cap) { IosGpuHost *c = surface; return c && cap > 0 ? oa_bytes(ios_action_arg(oa_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int oa_ios_copy_get(void *surface, char *out, int cap) { IosGpuHost *c = surface; return c && cap > 0 ? oa_bytes(ios_copy_get(oa_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
void oa_ios_drawer_set(void *surface, int open) { IosGpuHost *c = surface; if (c) { ios_drawer_set(oa_state(c), open != 0); oa_woke(c); } }
int oa_ios_drawer_open(void *surface) { IosGpuHost *c = surface; return c ? (int)ios_drawer_open(oa_state(c)) : 0; }
void oa_ios_dashboard_set(void *surface, int open) { IosGpuHost *c = surface; if (c) { ios_dashboard_set(oa_state(c), open != 0); oa_woke(c); } }
int oa_ios_dashboard_open(void *surface) { IosGpuHost *c = surface; return c ? (int)ios_dashboard_open(oa_state(c)) : 0; }
void oa_ios_scroll_top(void *surface) { IosGpuHost *c = surface; if (c) { ios_scroll_top(oa_state(c)); oa_woke(c); } }
void oa_ios_scroll_bottom(void *surface) { IosGpuHost *c = surface; if (c) { ios_scroll_bottom(oa_state(c)); oa_woke(c); } }

void oa_ios_set_header(void *surface, const char *computer, const char *session_title, const char *workspace, int reachability, int paired_count, int has_active, int has_older, int has_more_sessions) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_header(oa_state(c), oa_str(computer), oa_str(session_title), oa_str(workspace), reachability, paired_count, has_active != 0, has_older != 0, has_more_sessions != 0);
  oa_changed(c);
}
void oa_ios_set_banners(void *surface, const char *error, int outbox_count, const char *toast) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_banners(oa_state(c), oa_str(error), outbox_count, oa_str(toast));
  oa_changed(c);
}
void oa_ios_set_approval(void *surface, int active, const char *prompt, const char *warning) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_approval(oa_state(c), active != 0, oa_str(prompt), oa_str(warning));
  oa_changed(c);
}
void oa_ios_approval_options_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_approval_options_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_approval_options_push(void *surface, const char *id, const char *label, const char *kind) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_approval_options_push(oa_state(c), oa_str(id), oa_str(label), oa_str(kind));
  oa_changed(c);
}
void oa_ios_set_composer(void *surface, const char *model_label, const char *effort_label, const char *mode_label, int attachments, int mic_active) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_composer(oa_state(c), oa_str(model_label), oa_str(effort_label), oa_str(mode_label), attachments, mic_active != 0);
  oa_changed(c);
}
void oa_ios_set_working(void *surface, int busy, int awaiting, int secs, const char *label) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_working(oa_state(c), busy != 0, awaiting != 0, secs, oa_str(label));
  oa_changed(c);
}
void oa_ios_set_usage(void *surface, const char *summary, int context_pct) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_usage(oa_state(c), oa_str(summary), context_pct);
  oa_changed(c);
}
void oa_ios_set_full_auto(void *surface, int on) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_full_auto(oa_state(c), on != 0);
  oa_changed(c);
}
void oa_ios_set_voice_duplex(void *surface, int state, float level) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_voice_duplex(oa_state(c), state, level);
  oa_changed(c);
}
void oa_ios_set_queued(void *surface, const char *text) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_set_queued(oa_state(c), oa_str(text));
  oa_changed(c);
}
void oa_ios_intro_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_intro_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_intro_push(void *surface, int kind, const char *id, const char *title, const char *harness, const char *cwd, const char *status, int busy, int current) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_intro_push(oa_state(c), kind, oa_str(id), oa_str(title), oa_str(harness), oa_str(cwd), oa_str(status), busy != 0, current != 0);
  oa_changed(c);
}
void oa_ios_machines_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_machines_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_machines_push(void *surface, const char *id, const char *name, const char *detail, int reachability, int session_count, int current) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_machines_push(oa_state(c), oa_str(id), oa_str(name), oa_str(detail), reachability, session_count, current != 0);
  oa_changed(c);
}

void oa_ios_suggestions_clear(void *surface, int is_mention) { IosGpuHost *c = surface; if (c) { ios_suggestions_clear(oa_state(c), is_mention != 0); oa_changed(c); } }
void oa_ios_suggestions_push(void *surface, const char *id, const char *label, const char *detail) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_suggestions_push(oa_state(c), oa_str(id), oa_str(label), oa_str(detail));
  oa_changed(c);
}

void oa_ios_todos_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_todos_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_todos_push(void *surface, const char *content, int status) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_todos_push(oa_state(c), oa_str(content), status);
  oa_changed(c);
}

void oa_ios_folders_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_folders_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_folders_push(void *surface, const char *path, const char *label, int selected) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_folders_push(oa_state(c), oa_str(path), oa_str(label), selected != 0);
  oa_changed(c);
}

void oa_ios_sessions_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_sessions_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_sessions_push(void *surface, const char *id, const char *title, const char *repo, const char *branch, const char *status, int busy, int current, int attention, int unread) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_sessions_push(oa_state(c), oa_str(id), oa_str(title), oa_str(repo), oa_str(branch), oa_str(status), busy != 0, current != 0, attention != 0, unread != 0);
  oa_changed(c);
}
void oa_ios_session_set_preview(void *surface, const char *id, const char *ask, const char *activity, const char *reply) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_session_set_preview(oa_state(c), oa_str(id), oa_str(ask), oa_str(activity), oa_str(reply));
  oa_changed(c);
}

void oa_ios_items_clear(void *surface, int reset_scroll) { IosGpuHost *c = surface; if (c) { ios_items_clear(oa_state(c), reset_scroll != 0); oa_changed(c); } }
void oa_ios_items_push(void *surface, int role, const char *id, const char *text, const char *tool, const char *path, const char *diff, int complete, int failed, int is_subagent, int diff_cut, int subagent_calls, int elapsed_secs) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_items_push(oa_state(c), role, oa_str(id), oa_str(text), oa_str(tool), oa_str(path), oa_str(diff), complete != 0, failed != 0, is_subagent != 0, diff_cut != 0, subagent_calls, elapsed_secs);
  oa_changed(c);
}

void oa_ios_models_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_models_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_models_push(void *surface, const char *id, const char *label, const char *detail, int selected) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_models_push(oa_state(c), oa_str(id), oa_str(label), oa_str(detail), selected != 0);
  oa_changed(c);
}
void oa_ios_efforts_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_efforts_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_efforts_push(void *surface, const char *id, const char *label, const char *detail, int selected) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_efforts_push(oa_state(c), oa_str(id), oa_str(label), oa_str(detail), selected != 0);
  oa_changed(c);
}
void oa_ios_modes_clear(void *surface) { IosGpuHost *c = surface; if (c) { ios_modes_clear(oa_state(c)); oa_changed(c); } }
void oa_ios_modes_push(void *surface, const char *id, const char *label, const char *detail, int selected) {
  IosGpuHost *c = surface;
  if (!c) return;
  ios_modes_push(oa_state(c), oa_str(id), oa_str(label), oa_str(detail), selected != 0);
  oa_changed(c);
}
