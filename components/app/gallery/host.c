// Firm reason: 6 (native GPU carrier for the compiled gallery page; state, layout and paint are Coax).
// The app set's gallery surface (host.h). It is included by a host that already includes the
// component carrier (components/gallery/linux/main.c), so it shares that file's App, its atlas
// code and the one generated library, which must load components/app/gallery/gallery.coax. It
// holds no behaviour.
#include "host.h"

typedef struct {
  App app;
  coax_t_AppGallery *s;
  float safe_top, safe_bottom;
  int reduced;
  void (*changed)(void *context); void *context;
} AppGalleryHost;

static coax_v_t_AppGallery og_state(AppGalleryHost *g) { return (coax_v_t_AppGallery){g->s, 1}; }

static int og_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  AppGalleryHost *g = user;
  appgallery_env(og_state(g), g->safe_top, g->safe_bottom, g->reduced != 0);
  int n = (int)appgallery_frame(og_state(g), (coax_v_f32){vertices, (int64_t)g->app.max_vertices * coaxial_components_vertex_floats()}, w, h, dpr, dt);
  atlas_serve(&g->app);
  return n < 0 ? 0 : n;
}
static void og_on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  (void)clicks; (void)mods;
  AppGalleryHost *g = user;
  appgallery_pointer(og_state(g), x, y, phase);
  if (g->changed) g->changed(g->context);
}
static int og_on_drive(void *user, const char *op, char *out, int cap) {
  AppGalleryHost *g = user;
  return (int)appgallery_drive(og_state(g), (coax_string){op ? op : "", op ? (int64_t)strlen(op) : 0}, (coax_v_u8){(uint8_t *)out, cap});
}

void *og_open(const char *resources) {
  AppGalleryHost *g = calloc(1, sizeof *g);
  if (!g) return NULL;
  App *a = &g->app;
  snprintf(a->resources, sizeof a->resources, "%s", resources);
  char shader[PATH_MAX + 32]; snprintf(shader, sizeof shader,
#ifdef __APPLE__
  "%s/components.metal",
#else
  "%s/components.spv",
#endif
  resources);
  g->s = calloc(1, sizeof *g->s);
  if (!g->s) { free(g); return NULL; }
  a->atlas = g->s->atlas.e; a->max_vertices = (uint32_t)appgallery_max_vertices();
  UiApp app = {.title = "Components", .width = 402, .height = 874, .max_vertices = a->max_vertices, .shader = shader, .user = g,
               .frame = og_on_frame, .pointer = og_on_pointer, .drive = og_on_drive};
  a->host = ui_open(&app, 0);
  if (!a->host) { free(g->s); free(g); return NULL; }
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  a->dpr *= appgallery_scale();
  if (!atlas_setup(a)) { gallery_free(a); free(g->s); free(g); return NULL; }
  return g;
}
#ifdef __APPLE__
void *og_view(void *gallery) { return gallery ? ui_view(((AppGalleryHost *)gallery)->app.host) : NULL; }
#else
void *og_view(void *gallery) {(void)gallery;return NULL;}
#endif
void og_frame(void *gallery) { if (gallery) ui_frame(((AppGalleryHost *)gallery)->app.host); }
int og_moving(void *gallery) { AppGalleryHost *g = gallery; return g && appgallery_moving(og_state(g)); }
void og_on_change(void *gallery, void (*changed)(void *context), void *context) { AppGalleryHost *g = gallery; if (g) { g->changed = changed; g->context = context; } }
void og_env(void *gallery, float safe_top, float safe_bottom, int reduced) {
  AppGalleryHost *g = gallery;
  if (!g) return;
  g->safe_top = safe_top; g->safe_bottom = safe_bottom; g->reduced = reduced != 0;
}
int og_action(void *gallery) { AppGalleryHost *g = gallery; return g ? (int)appgallery_action(og_state(g)) : 0; }
int og_reveal(void *gallery, const char *id) { AppGalleryHost *g = gallery; return g && id && appgallery_reveal(og_state(g), (coax_string){id, (int64_t)strlen(id)}); }
static int og_bytes(int64_t n, char *out, int cap) { if (cap < 1) return 0; if (n < 0) n = 0; if (n > cap - 1) n = cap - 1; out[n] = 0; return (int)n; }
static void og_woke(AppGalleryHost *g) { if (g->changed) g->changed(g->context); }
int og_rows(void *gallery) { AppGalleryHost *g = gallery; return g ? (int)appgallery_rows(og_state(g)) : 0; }
int og_rows_stamp(void *gallery) { AppGalleryHost *g = gallery; return g ? (int)appgallery_rows_stamp(og_state(g)) : 0; }
int og_row(void *gallery, int i, float row[8], char *id, int id_cap, char *label, int label_cap) {
  AppGalleryHost *g = gallery;
  if (!g || id_cap < 1 || label_cap < 1 || !appgallery_row(og_state(g), i, (coax_v_f32){row, 8})) return 0;
  og_bytes(appgallery_row_id(og_state(g), i, (coax_v_u8){(uint8_t *)id, id_cap - 1}), id, id_cap);
  og_bytes(appgallery_row_say(og_state(g), i, (coax_v_u8){(uint8_t *)label, label_cap - 1}), label, label_cap);
  return 1;
}
int og_row_value(void *gallery, int i, char *out, int cap) { AppGalleryHost *g = gallery; return g && cap > 0 ? og_bytes(appgallery_row_value(og_state(g), i, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int og_activate(void *gallery, const char *id) { AppGalleryHost *g = gallery; if (!g || !id) return 0; int done = appgallery_activate(og_state(g), (coax_string){id, (int64_t)strlen(id)}); og_woke(g); return done; }
int og_scroll_page(void *gallery, int dir) { AppGalleryHost *g = gallery; if (!g || !appgallery_scroll_page(og_state(g), dir)) return 0; og_woke(g); return 1; }
int og_refused(void *gallery) { AppGalleryHost *g = gallery; return g ? (int)appgallery_refused(og_state(g)) : 0; }
void og_close(void *gallery) {
  AppGalleryHost *g = gallery;
  if (!g) return;
  gallery_free(&g->app);
  free(g->s); free(g);
}
