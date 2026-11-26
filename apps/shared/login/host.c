// Firm reason: 6 (native GPU carrier for the compiled login screen; state, layout and paint are Coax).
// The native login's surface (host.h, GitHub #13645). It is included by the component gallery's
// carrier (components/gallery/linux/main.c, COMPONENTS_LOGIN), so it shares that file's App, its
// atlas code and the one generated library: nothing here reads a font or builds a glyph. It
// holds no behaviour: it hands frames and touches to screen.coax, tells the scene where the form
// is, and passes the screen's values across the C boundary.
#include "host.h"
#include "scene-host.h"

typedef struct {
  App app;
  coax_t_LoginScreen *s;
  void *scene;
  float safe_top, safe_bottom, keyboard;
  int reduced;
  int settle;     // frames still owed after a change (glyphs asked for in one frame are drawn in the next)
  void (*changed)(void *context); void *context;
} Login;

static coax_v_t_LoginScreen lg_state(Login *l) { return (coax_v_t_LoginScreen){l->s, 1}; }
static coax_string lg_string(const char *text) { return (coax_string){text ? text : "", text ? (int64_t)strlen(text) : 0}; }
static void lg_changed(Login *l) { l->settle = 3; }
// A change the native side did not make: wake it.
static void lg_woke(Login *l) { lg_changed(l); if (l->changed) l->changed(l->context); }

static int lg_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  Login *l = user;
  login_env(lg_state(l), l->safe_top, l->safe_bottom, l->keyboard, l->reduced != 0, lgs_ready(l->scene) != 0);
  int n = (int)login_frame(lg_state(l), (coax_v_f32){vertices, (int64_t)l->app.max_vertices * coaxial_components_vertex_floats()}, w, h, dpr, dt);
  if (atlas_serve(&l->app)) lg_changed(l);
  if (l->scene) {
    // where the form is, for the scene behind it
    float form[5] = {0};
    if (login_form(lg_state(l), (coax_v_f32){form, 5})) for (int i = 0; i < 5; i++) form[i] *= dpr;
    lgs_form(l->scene, form, l->reduced);
  }
  if (l->settle > 0) l->settle--;
  return n < 0 ? 0 : n;
}
static void lg_on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  (void)clicks; (void)mods;
  Login *l = user;
  login_pointer(lg_state(l), x, y, phase);
  lg_woke(l);
}
static int lg_on_drive(void *user, const char *op, char *out, int cap) {
  Login *l = user;
  lg_woke(l);
  return (int)login_drive(lg_state(l), lg_string(op), (coax_v_u8){(uint8_t *)out, cap});
}

void *lg_open(const char *resources, int scene, int card) {
  Login *l = calloc(1, sizeof *l);
  if (!l) return NULL;
  App *a = &l->app;
  snprintf(a->resources, sizeof a->resources, "%s", resources);
  char shader[PATH_MAX + 32]; snprintf(shader, sizeof shader, "%s/components.metal", resources);
  l->s = calloc(1, sizeof *l->s);
  if (!l->s) { free(l); return NULL; }
  a->atlas = l->s->atlas.e; a->max_vertices = (uint32_t)login_max_vertices();
  UiApp app = {.title = "Log in", .width = 402, .height = 874, .max_vertices = a->max_vertices, .shader = shader, .user = l,
               .frame = lg_on_frame, .pointer = lg_on_pointer, .drive = lg_on_drive, .transparent = card != 0};
  login_card(lg_state(l), card != 0);
  a->host = ui_open(&app, 0);
  if (!a->host) { free(l->s); free(l); return NULL; }
  // the atlas at the scale the screen draws its units at
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  a->dpr *= login_scale();
  if (!atlas_setup(a)) { gallery_free(a); free(l->s); free(l); return NULL; }
  if (scene && !card) l->scene = lgs_open(a->host, resources);
  lg_changed(l);
  return l;
}
void lg_keys(void *login, int offered) { Login *l = login; if (l) { login_keys(lg_state(l), offered != 0); lg_changed(l); } }
void lg_workos(void *login, int offered) { Login *l = login; if (l) { login_workos(lg_state(l), offered != 0); lg_changed(l); } }
void *lg_view(void *login) { return login ? ui_view(((Login *)login)->app.host) : NULL; }
void lg_frame(void *login) { if (login) ui_frame(((Login *)login)->app.host); }
void lg_on_change(void *login, void (*changed)(void *context), void *context) { Login *l = login; if (l) { l->changed = changed; l->context = context; } }
int lg_moving(void *login) {
  Login *l = login;
  return l && (l->settle > 0 || login_moving(lg_state(l)) || (!l->reduced && lgs_moving(l->scene)));
}
void lg_env(void *login, float safe_top, float safe_bottom, float keyboard, int reduced) {
  Login *l = login;
  if (!l) return;
  if (l->safe_top != safe_top || l->safe_bottom != safe_bottom || l->keyboard != keyboard || l->reduced != (reduced != 0)) lg_changed(l);
  l->safe_top = safe_top; l->safe_bottom = safe_bottom; l->keyboard = keyboard; l->reduced = reduced != 0;
}
int lg_action(void *login) { Login *l = login; return l ? (int)login_action(lg_state(l)) : 0; }
void lg_reply(void *login, int action, int ok, const char *message) {
  Login *l = login;
  if (!l) return;
  login_reply(lg_state(l), action, ok != 0, lg_string(message));
  lg_changed(l);
}
int lg_focus(void *login) { Login *l = login; return l ? (int)login_focus(lg_state(l)) : 0; }
int lg_text(void *login, int field, char *out, int cap) {
  Login *l = login;
  if (!l || cap < 1) return 0;
  int n = (int)login_text(lg_state(l), field, (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
int lg_field(void *login, int field, float box[5]) { Login *l = login; return l && login_field(lg_state(l), field, (coax_v_f32){box, 5}); }
void lg_set_text(void *login, int field, const char *text) {
  Login *l = login;
  if (!l) return;
  login_set_text(lg_state(l), field, lg_string(text));
  lg_changed(l);
}
void lg_submit(void *login) { Login *l = login; if (l) { login_submit(lg_state(l)); lg_changed(l); } }
void lg_blur(void *login) { Login *l = login; if (l) { login_blur(lg_state(l)); lg_changed(l); } }
int lg_editor(float style[14]) { return login_editor((coax_v_f32){style, 14}); }
int lg_rows(void *login) { Login *l = login; return l ? (int)login_rows(lg_state(l)) : 0; }
int lg_rows_stamp(void *login) { Login *l = login; return l ? (int)login_rows_stamp(lg_state(l)) : 0; }
int lg_row(void *login, int i, float row[7], char *id, int id_cap, char *label, int label_cap, char *value, int value_cap) {
  Login *l = login;
  if (!l || id_cap < 1 || label_cap < 1 || value_cap < 1 || !login_row(lg_state(l), i, (coax_v_f32){row, 7})) return 0;
  id[login_row_id(lg_state(l), i, (coax_v_u8){(uint8_t *)id, id_cap - 1})] = 0;
  label[login_row_label(lg_state(l), i, (coax_v_u8){(uint8_t *)label, label_cap - 1})] = 0;
  value[login_row_value(lg_state(l), i, (coax_v_u8){(uint8_t *)value, value_cap - 1})] = 0;
  return 1;
}
int lg_activate(void *login, const char *id) {
  Login *l = login;
  if (!l) return 0;
  lg_changed(l);
  return login_activate(lg_state(l), lg_string(id));
}
int lg_drive(void *login, const char *op, char *out, int cap) { return login ? lg_on_drive(login, op, out, cap) : -1; }
int lg_done(void *login) { Login *l = login; return l && login_done(lg_state(l)); }
int lg_scene(void *login) { Login *l = login; return l && lgs_ready(l->scene); }
unsigned long long lg_presented(void *login) { return login ? ui_presented(((Login *)login)->app.host) : 0; }
unsigned long long lg_scene_frames(void *login) { Login *l = login; return l ? lgs_frames(l->scene) : 0; }
void lg_close(void *login) {
  Login *l = login;
  if (!l) return;
  lgs_close(l->app.host, l->scene); l->scene = NULL;
  gallery_free(&l->app);
  free(l->s); free(l);
}
