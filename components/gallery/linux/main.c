// The component library's host program (Linux, and the Mac through apps/shared/host/apple/): components/gallery/page.coax behind the
// shared Wayland and Vulkan host (apps/shared/host/linux/ui-host.h), with the
// atlas built in Coax (components/raster/atlas.coax). It holds no behaviour:
// it reads the fonts and icons, hands frames, the pointer and the wheel to
// Coax, and serves the glyphs a frame asked for.
//   Components [--size WxH] [--dpr K] [--seconds N]
//   Components --shot OUT.png [--size W,H] [--dpr K] [--scroll PX] [--point X,Y] [--press X,Y]
// --shot draws offscreen and opens no window. --scroll, --point and --press
// (logical pixels) move the page, hover and click before the picture.
#include "gallery.h"
#include "ui-host.h"
#include <limits.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#ifdef __APPLE__
#include <mach-o/dyld.h>
#endif

typedef struct {
  UiHost *host;
  coax_t_GalleryState *g;
  char resources[PATH_MAX];
  coax_t_RaFonts fonts;
  coax_t_RaPlan plan;
  coax_t_RaWork *work;
  float sizes[16], icon_sizes[64];
  int64_t weights[16];
  int nfonts, nicons;
  char *svg[64]; size_t svg_n[64];
  uint8_t *mono, *color;
  uint32_t max_vertices;
  float dpr;
} App;

static uint8_t *slurp(const char *dir, const char *sub, const char *name, const char *ext, size_t *n) {
  char path[PATH_MAX];
  snprintf(path, sizeof path, "%s/%s/%s%s", dir, sub, name, ext);
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "Components: missing %s\n", path); return NULL; }
  fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
  uint8_t *p = malloc((size_t)size + 1);
  if (!p || fread(p, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(p); return NULL; }
  fclose(f); p[size] = 0; *n = (size_t)size;
  return p;
}
static coax_v_t_GalleryState state_of(App *a) { return (coax_v_t_GalleryState){a->g, 1}; }
static coax_v_t_Atlas atlas_of(App *a) { return (coax_v_t_Atlas){a->g->atlas.e, 1}; }
static coax_v_t_RaPlan plan_of(App *a) { return (coax_v_t_RaPlan){&a->plan, 1}; }
static size_t mono_bytes(App *a) { return (size_t)coaxial_ra_width(plan_of(a)) * (size_t)coaxial_ra_height(plan_of(a)); }
static size_t color_bytes(App *a) { return (size_t)coaxial_ra_width(plan_of(a)) * (size_t)coaxial_ra_color_height(plan_of(a)) * 4; }
static void atlas_upload(App *a) {
  ui_atlas(a->host, a->mono, (uint32_t)coaxial_ra_width(plan_of(a)), (uint32_t)coaxial_ra_height(plan_of(a)), a->color,
           (uint32_t)coaxial_ra_width(plan_of(a)), (uint32_t)coaxial_ra_color_height(plan_of(a)));
}

// The fonts and icons, read once, rasterized for this device scale.
static int atlas_setup(App *a) {
  static const char *const files[4] = {"Geist", "Geist-Medium", "Geist-SemiBold", "GeistMono"};
  for (int w = 0; w < 4; w++) {
    size_t n; uint8_t *bytes = slurp(a->resources, "fonts", files[w], ".ttf", &n);
    if (!bytes || !coaxial_ra_open((coax_v_t_RaFonts){&a->fonts, 1}, w, (coax_v_u8){bytes, (int64_t)n})) { fprintf(stderr, "Components: %s is not a TrueType font\n", files[w]); return 0; }
  }
  a->nfonts = (int)coaxial_components_font_count(); a->nicons = (int)coaxial_components_icon_count();
  if (a->nfonts > 16 || a->nicons > 64) return 0;
  for (int k = 0; k < a->nfonts; k++) { a->sizes[k] = coaxial_components_font_size(k); a->weights[k] = coaxial_components_font_weight(k); }
  for (int i = 0; i < a->nicons; i++) {
    char name[65]; int n = (int)coaxial_components_icon_name(state_of(a), i);
    memcpy(name, a->g->sbytes.e, (size_t)n); name[n] = 0;
    a->icon_sizes[i] = coaxial_components_icon_size(i);
    a->svg[i] = (char *)slurp(a->resources, "icons", name, ".svg", &a->svg_n[i]);
  }
  a->work = calloc(1, sizeof *a->work);
  if (!a->work) return 0;
  coaxial_ra_plan(plan_of(a), a->dpr, (coax_v_f32){a->sizes, a->nfonts}, (coax_v_f32){a->icon_sizes, a->nicons});
  a->mono = calloc(1, mono_bytes(a)); a->color = calloc(1, color_bytes(a));
  if (!a->mono || !a->color) return 0;
  coax_v_u8 px = {a->mono, (int64_t)mono_bytes(a)};
  coax_v_t_RaWork work = {a->work, 1};
  for (int k = 0; k < a->nfonts; k++) coaxial_ra_font(atlas_of(a), plan_of(a), k, (coax_v_t_RaFonts){&a->fonts, 1}, a->weights[k], a->sizes[k], px, work);
  for (int i = 0; i < a->nicons; i++) if (a->svg[i]) coaxial_ra_icon(atlas_of(a), plan_of(a), i, (coax_string){a->svg[i], (int64_t)a->svg_n[i]}, px, work);
  coaxial_ra_finish(atlas_of(a), plan_of(a));
  atlas_upload(a);
  return 1;
}

static int on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  App *a = user;
  float uniforms[4] = {0};
  int n = (int)coaxial_components_frame(state_of(a), (coax_v_f32){vertices, (int64_t)a->max_vertices * coaxial_components_vertex_floats()}, (coax_v_f32){uniforms, 4}, w, h, dpr, dt);
  // the graphemes and cells this frame wanted, for the next
  coaxial_ra_clean(plan_of(a));
  if (coaxial_ra_serve(atlas_of(a), plan_of(a), (coax_v_t_RaFonts){&a->fonts, 1}, (coax_v_s64){a->weights, a->nfonts}, (coax_v_f32){a->sizes, a->nfonts},
                       (coax_v_u8){a->mono, (int64_t)mono_bytes(a)}, (coax_v_t_RaWork){a->work, 1}) > 0) atlas_upload(a);
  ui_cursor(a->host, coaxial_components_cursor(state_of(a)) == 1 ? 2 : 0);
  return n < 0 ? 0 : n;
}
static void on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  (void)clicks; (void)mods;
  App *a = user;
  coaxial_components_pointer(state_of(a), x, y, phase, 0);
}
static void on_wheel(void *user, float dy) { App *a = user; coaxial_components_wheel(state_of(a), dy); }

static const char *value(int argc, char **argv, const char *name) {
  for (int i = 1; i + 1 < argc; i++) if (!strcmp(argv[i], name)) return argv[i + 1];
  return NULL;
}
static volatile sig_atomic_t stopping;
static void on_signal(int sig) { (void)sig; stopping = 1; }

int main(int argc, char **argv) {
  static App a;
  // the resources sit beside the program: fonts/, icons/, and the shader (components.spv on
  // Linux, components.metal on the Mac, whose host is apps/shared/host/apple/ui-host.m)
#ifdef __APPLE__
  uint32_t cap = sizeof a.resources;
  char exe[PATH_MAX];
  if (_NSGetExecutablePath(exe, &cap) != 0 || !realpath(exe, a.resources)) return 1;
  const char *shader_file = "components.metal";
#else
  ssize_t len = readlink("/proc/self/exe", a.resources, sizeof a.resources - 1);
  if (len <= 0) return 1;
  a.resources[len] = 0;
  const char *shader_file = "components.spv";
#endif
  char *slash = strrchr(a.resources, '/');
  if (slash) *slash = 0;
  char shader[PATH_MAX + 32]; snprintf(shader, sizeof shader, "%s/%s", a.resources, shader_file);
  const char *shot = value(argc, argv, "--shot"), *size = value(argc, argv, "--size"), *dpr = value(argc, argv, "--dpr");
  unsigned w = shot ? 3840 : 1920, h = shot ? 2400 : 1200;
  if (size && sscanf(size, "%u%*[x,]%u", &w, &h) != 2) { fprintf(stderr, "Components: --size WxH\n"); return 2; }
  a.dpr = dpr ? (float)atof(dpr) : shot ? 2 : 1;
  if (a.dpr <= 0) a.dpr = 1;
  a.g = calloc(1, sizeof *a.g);
  if (!a.g) return 1;
  a.max_vertices = (uint32_t)coaxial_components_max_vertices();
  UiApp app = {.title = "Components", .app_id = "com.openagents.components", .width = w, .height = h, .max_vertices = a.max_vertices, .shader = shader,
               .user = &a, .frame = on_frame, .pointer = on_pointer, .wheel = on_wheel};
  a.host = ui_open(&app, shot != NULL);
  if (!a.host) return 1;
  ui_set_dpr(a.host, a.dpr);
  if (!atlas_setup(&a)) return 1;
  int status = 0;
  if (shot) {
    const char *scroll = value(argc, argv, "--scroll"), *point = value(argc, argv, "--point"), *press = value(argc, argv, "--press");
    // a few frames: the first asks for the glyphs outside the fixed set, the next have them
    for (int i = 0; i < 4; i++) ui_shot_frame(a.host, w, h, 1 / 60.0f);
    float x, y;
    if (scroll) on_wheel(&a, (float)atof(scroll));
    if (press && sscanf(press, "%f,%f", &x, &y) == 2) { on_pointer(&a, x * a.dpr, y * a.dpr, 0, 1, 0); on_pointer(&a, x * a.dpr, y * a.dpr, 2, 1, 0); }
    if (point && sscanf(point, "%f,%f", &x, &y) == 2) on_pointer(&a, x * a.dpr, y * a.dpr, 1, 0, 0);
    for (int i = 0; i < 4; i++) ui_shot_frame(a.host, w, h, 1 / 60.0f);
    if (!ui_shot_write(a.host, shot)) { fprintf(stderr, "Components: could not write %s\n", shot); status = 1; }
  } else {
    const char *seconds = value(argc, argv, "--seconds");
    double limit = seconds ? atof(seconds) : 0;
    struct timespec t0; clock_gettime(CLOCK_MONOTONIC, &t0);
    signal(SIGINT, on_signal); signal(SIGTERM, on_signal);
    while (!stopping && ui_frame(a.host)) {
      if (limit > 0) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); if (t.tv_sec - t0.tv_sec + (t.tv_nsec - t0.tv_nsec) / 1e9 > limit) break; }
    }
  }
  ui_close(a.host);
  return status;
}
