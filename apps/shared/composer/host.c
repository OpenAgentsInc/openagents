// Firm reason: 6 (native GPU carrier for the compiled composer; state, layout and paint are Coax).
// The chat composer's surface (host.h, GitHub #13645). Like the login's (apps/shared/login/host.c)
// it is included by the component gallery's carrier (components/gallery/linux/main.c,
// COMPONENTS_COMPOSER), so it shares that file's App, its atlas code and the one generated
// library. It holds no behaviour: frames and touches go to surface.coax, values cross the C boundary.
#include "host.h"

typedef struct {
  App app;
  coax_t_ComposerSurface *s;
  int focused, reduced;
  float editor_height;
  int settle;     // frames still owed after a change (glyphs asked for in one frame are drawn in the next)
  void (*changed)(void *context); void *context;
} Bar;

static coax_v_t_ComposerSurface gc_state(Bar *b) { return (coax_v_t_ComposerSurface){b->s, 1}; }
static coax_string gc_string(const char *text) { return (coax_string){text ? text : "", text ? (int64_t)strlen(text) : 0}; }
static void gc_changed(Bar *b) { b->settle = 3; }
static void gc_woke(Bar *b) { gc_changed(b); if (b->changed) b->changed(b->context); }

// The surface's own icons (Apps SDK), for the shared atlas code.
static int gc_icon_count(void) { return (int)composer_surface_icon_count(); }
static float gc_icon_size(int i) { return composer_surface_icon_size(i); }
static int gc_icon_name(int i, char *out, int cap) { return (int)composer_surface_icon_name(i, (coax_v_u8){(uint8_t *)out, cap}); }

static int gc_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  Bar *b = user;
  composer_surface_env(gc_state(b), b->focused != 0, b->editor_height, b->reduced != 0);
  int n = (int)composer_surface_frame(gc_state(b), (coax_v_f32){vertices, (int64_t)b->app.max_vertices * coaxial_components_vertex_floats()}, w, h, dpr, dt);
  if (atlas_serve(&b->app)) gc_changed(b);
  if (b->settle > 0) b->settle--;
  return n < 0 ? 0 : n;
}
static void gc_on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  (void)clicks; (void)mods;
  Bar *b = user;
  composer_surface_pointer(gc_state(b), x, y, phase);
  gc_woke(b);
}
static int gc_on_drive(void *user, const char *op, char *out, int cap) {
  Bar *b = user;
  gc_woke(b);
  return (int)composer_surface_drive(gc_state(b), gc_string(op), (coax_v_u8){(uint8_t *)out, cap});
}

void *gc_open(const char *resources) {
  Bar *b = calloc(1, sizeof *b);
  if (!b) return NULL;
  App *a = &b->app;
  snprintf(a->resources, sizeof a->resources, "%s", resources);
  char shader[PATH_MAX + 32]; snprintf(shader, sizeof shader, "%s/components.metal", resources);
  b->s = calloc(1, sizeof *b->s);
  if (!b->s) { free(b); return NULL; }
  a->atlas = b->s->atlas.e; a->max_vertices = (uint32_t)composer_surface_max_vertices();
  a->icons_dir = "icons-apps"; a->icon_count = gc_icon_count; a->icon_size = gc_icon_size; a->icon_name = gc_icon_name;
  UiApp app = {.title = "Composer", .width = 378, .height = 94, .embedded = 1, .max_vertices = a->max_vertices, .shader = shader, .user = b,
               .frame = gc_on_frame, .pointer = gc_on_pointer, .drive = gc_on_drive};
  a->host = ui_open(&app, 0);
  if (!a->host) { free(b->s); free(b); return NULL; }
  // the atlas at the scale the surface draws its units at
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  a->dpr *= composer_surface_scale();
  if (!atlas_setup(a)) { gallery_free(a); free(b->s); free(b); return NULL; }
  gc_changed(b);
  return b;
}
void *gc_view(void *composer) { return composer ? ui_view(((Bar *)composer)->app.host) : NULL; }
void gc_frame(void *composer) { if (composer) ui_frame(((Bar *)composer)->app.host); }
int gc_moving(void *composer) { Bar *b = composer; return b && (b->settle > 0 || composer_surface_moving(gc_state(b))); }
void gc_on_change(void *composer, void (*changed)(void *context), void *context) { Bar *b = composer; if (b) { b->changed = changed; b->context = context; } }
void gc_set(void *composer, const char *field_id, const char *send_id, const char *placeholder, const char *model, int stop, int enabled, int locked) {
  Bar *b = composer;
  if (!b) return;
  composer_surface_set(gc_state(b), gc_string(field_id), gc_string(send_id), gc_string(placeholder), gc_string(model), stop != 0, enabled != 0, locked != 0);
  gc_changed(b);
}
void gc_chips(void *composer, int show, const char *environment, int environment_on, const char *repository, int repository_on) {
  Bar *b = composer;
  if (!b) return;
  composer_surface_chips(gc_state(b), show != 0, gc_string(environment), environment_on != 0, gc_string(repository), repository_on != 0);
  gc_changed(b);
}
int gc_set_text(void *composer, const char *text) {
  Bar *b = composer;
  if (!b) return 0;
  gc_changed(b);
  return composer_surface_set_text(gc_state(b), gc_string(text));
}
int gc_text(void *composer, char *out, int cap) {
  Bar *b = composer;
  if (!b || cap < 1) return 0;
  int n = (int)composer_surface_text(gc_state(b), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
void gc_env(void *composer, int focused, float editor_height, int reduced) {
  Bar *b = composer;
  if (!b) return;
  if (b->focused != (focused != 0) || b->editor_height != editor_height || b->reduced != (reduced != 0)) gc_changed(b);
  b->focused = focused != 0; b->editor_height = editor_height; b->reduced = reduced != 0;
}
int gc_action(void *composer) { Bar *b = composer; return b ? (int)composer_surface_action(gc_state(b)) : 0; }
float gc_height(void *composer) { Bar *b = composer; return b ? composer_surface_height(gc_state(b)) : 94; }
int gc_editor(void *composer, float box[6]) { Bar *b = composer; return b && composer_surface_editor(gc_state(b), (coax_v_f32){box, 6}); }
int gc_style(float style[14]) { return composer_surface_style((coax_v_f32){style, 14}); }
int gc_rows(void *composer) { Bar *b = composer; return b ? (int)composer_surface_rows(gc_state(b)) : 0; }
int gc_rows_stamp(void *composer) { Bar *b = composer; return b ? (int)composer_surface_rows_stamp(gc_state(b)) : 0; }
int gc_row(void *composer, int i, float row[7], char *id, int id_cap, char *label, int label_cap, char *value, int value_cap) {
  Bar *b = composer;
  if (!b || id_cap < 1 || label_cap < 1 || value_cap < 1 || !composer_surface_row(gc_state(b), i, (coax_v_f32){row, 7})) return 0;
  value[composer_surface_row_value(gc_state(b), i, (coax_v_u8){(uint8_t *)value, value_cap - 1})] = 0;
  id[composer_surface_row_id(gc_state(b), i, (coax_v_u8){(uint8_t *)id, id_cap - 1})] = 0;
  label[composer_surface_row_label(gc_state(b), i, (coax_v_u8){(uint8_t *)label, label_cap - 1})] = 0;
  return 1;
}
int gc_activate(void *composer, const char *id) {
  Bar *b = composer;
  if (!b) return 0;
  gc_changed(b);
  return composer_surface_activate(gc_state(b), gc_string(id));
}
int gc_drive(void *composer, const char *op, char *out, int cap) { return composer ? gc_on_drive(composer, op, out, cap) : -1; }
unsigned long long gc_presented(void *composer) { return composer ? ui_presented(((Bar *)composer)->app.host) : 0; }
void gc_close(void *composer) {
  Bar *b = composer;
  if (!b) return;
  gallery_free(&b->app);
  free(b->s); free(b);
}
