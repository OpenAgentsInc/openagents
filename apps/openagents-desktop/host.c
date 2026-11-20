// Firm reason: 6 (native GPU carrier for the compiled page; state, layout and paint are Coax).
// Coder's page on the shared carrier (host.h). carrier.c includes it after the component
// carrier (components/gallery/linux/main.c), so it shares that file's App and atlas code and the
// one generated library. It holds no behaviour: frames, clicks and lines go to page.coax.
#include "host.h"
#include "../shared/native/tailnet-codec.c"
#include <time.h>
#include <sys/stat.h>
#ifdef __APPLE__
#include <CoreFoundation/CoreFoundation.h>
#include <CoreGraphics/CoreGraphics.h>
#include <ImageIO/ImageIO.h>
#endif

#define CD_IMAGES 16
// The Mac's host (apps/shared/host/apple/ui-host.m) draws a flat frame's pictures (page/compose.coax)
#ifdef __APPLE__
#define CD_FLAT_PICTURES 1
#else
#define CD_FLAT_PICTURES 0
#endif
#define CD_IMAGE_MAX_SIDE 1024   // a picture a command shows inline is up to 560 points wide (page/shows.coax)

typedef struct {
  uint8_t *rgba;
  uint32_t w, h;
  uint64_t gen;
} CdImage;

typedef struct {
  App app;
  coax_t_CoderPage *s;
  coax_t_CpStore *st;   // the page's turns, tool calls and long text (page/state.coax, coder_page_store)
  int edge;
  int active, reduced;   // the view has the keyboard (its window key, it first responder); reduced motion (bit 1) and transparency (bit 2)
  float screen;   // the screen's own scale
  int settle;     // frames still owed after a change (glyphs asked for in one frame are drawn in the next)
  void (*changed)(void *context); void *context;
  int headless;   // a page with no window: a chat whose reply runs out of view
  char composition_shader[PATH_MAX + 32];   // composition-v1's shader, for the frosted menu (page/compose.coax)
  int composition;   // the shader loaded: the page may compose
  CdImage images[CD_IMAGES];   // decoded straight RGBA8 textures for composition-v1 image slots 0..15
  uint64_t img_gen;
  void (*overlay)(void *context); void *overlay_context;   // told after the pointer reached a dialog or a menu
  char *files;   // the working folder's files as the app listed them (cd_files): the page reads them here
  char *file;    // the file in the pane as last read or saved (cd_file_open, cd_file_saved): the page reads it here
} CoderHost;

static coax_v_t_CoderPage cd_state(CoderHost *c) { return (coax_v_t_CoderPage){c->s, 1}; }
// The page and its storage, zeroed, the storage handed to the page. False when either can't be had.
static int cd_alloc(CoderHost *c) {
  c->s = calloc(1, sizeof *c->s); c->st = calloc(1, sizeof *c->st);
  if (!c->s || !c->st) { free(c->s); free(c->st); return 0; }
  coder_page_store(cd_state(c), (coax_v_t_CpStore){c->st, 1});
  return 1;
}
static coax_string cd_string(const char *text) { return (coax_string){text ? text : "", text ? (int64_t)strlen(text) : 0}; }
static void cd_changed(CoderHost *c) { c->settle = 3; }
static void cd_woke(CoderHost *c) { cd_changed(c); if (c->changed) c->changed(c->context); }
// The system's uptime in seconds: NSEvent.timestamp's clock on macOS; CLOCK_MONOTONIC (also
// stopped while asleep) on Linux, where there is no CLOCK_UPTIME_RAW.
static double cd_uptime(void) {
#ifdef __APPLE__
  return (double)clock_gettime_nsec_np(CLOCK_UPTIME_RAW) / 1e9;
#else
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
#endif
}

static int cd_decode_image(const char *path, CdImage *slot, uint64_t gen) {
  free(slot->rgba); slot->rgba = NULL; slot->w = 0; slot->h = 0; slot->gen = 0;
#ifdef __APPLE__
  if (!path || !*path) return 0;
  CFURLRef url = CFURLCreateFromFileSystemRepresentation(NULL, (const UInt8 *)path, (CFIndex)strlen(path), false);
  if (!url) return 0;
  CGImageSourceRef src = CGImageSourceCreateWithURL(url, NULL);
  CFRelease(url);
  if (!src) return 0;
  int max_px = CD_IMAGE_MAX_SIDE;
  CFNumberRef max_num = CFNumberCreate(NULL, kCFNumberIntType, &max_px);
  const void *keys[3] = {kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceThumbnailMaxPixelSize, kCGImageSourceCreateThumbnailWithTransform};
  const void *vals[3] = {kCFBooleanTrue, max_num, kCFBooleanTrue};
  CFDictionaryRef opts = CFDictionaryCreate(NULL, keys, vals, 3, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
  CGImageRef img = CGImageSourceCreateThumbnailAtIndex(src, 0, opts);
  CFRelease(opts); CFRelease(max_num); CFRelease(src);
  if (!img) return 0;
  size_t w = CGImageGetWidth(img), h = CGImageGetHeight(img);
  if (w < 1 || h < 1 || w > 4096 || h > 4096) { CGImageRelease(img); return 0; }
  uint8_t *buf = calloc(w * h, 4);
  if (!buf) { CGImageRelease(img); return 0; }
  CGColorSpaceRef cs = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
  if (!cs) cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(buf, w, h, 8, w * 4, cs, kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
  CGColorSpaceRelease(cs);
  if (!ctx) { free(buf); CGImageRelease(img); return 0; }
  CGContextDrawImage(ctx, CGRectMake(0, 0, (CGFloat)w, (CGFloat)h), img);
  CGContextRelease(ctx); CGImageRelease(img);
  // composition-v1 image ops expect straight (non-premultiplied) RGBA8
  for (size_t i = 0; i < w * h * 4; i += 4) {
    uint8_t a = buf[i + 3];
    if (a > 0 && a < 255) {
      buf[i + 0] = (uint8_t)(((unsigned)buf[i + 0] * 255u + a / 2u) / a);
      buf[i + 1] = (uint8_t)(((unsigned)buf[i + 1] * 255u + a / 2u) / a);
      buf[i + 2] = (uint8_t)(((unsigned)buf[i + 2] * 255u + a / 2u) / a);
    }
  }
  slot->rgba = buf; slot->w = (uint32_t)w; slot->h = (uint32_t)h; slot->gen = gen;
  return 1;
#else
  (void)path; (void)gen;
  return 0;
#endif
}
static int cd_serve_images(CoderHost *c) {
  int any = 0;
  char path[PATH_MAX];
  for (int i = 0; i < CD_IMAGES; i++) {
    int n = (int)coder_page_image_pending(cd_state(c), i, (coax_v_u8){(uint8_t *)path, PATH_MAX - 1});
    if (n <= 0) continue;
    path[n] = 0;
    int ok = cd_decode_image(path, &c->images[i], ++c->img_gen);
    coder_page_image_loaded(cd_state(c), i, ok ? (int64_t)c->images[i].w : 0, ok ? (int64_t)c->images[i].h : 0, ok != 0);
    any = 1;
  }
  return any;
}
static int cd_image(void *user, int index, UiImage *out) {
  CoderHost *c = user;
  if (!c || !out) return 0;
  int slot = (int)coder_page_image_slot(cd_state(c), index);
  if (slot < 0 || slot >= CD_IMAGES || !c->images[slot].rgba) return 0;
  *out = (UiImage){c->images[slot].rgba, c->images[slot].w, c->images[slot].h, c->images[slot].gen};
  return 1;
}
static int cd_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  CoderHost *c = user;
  coder_page_env(cd_state(c), c->active != 0, (c->reduced & 1) != 0);
  // reduced transparency: no frost, so the page draws its frosted surfaces solid; pictures still
  // compose (page/compose.coax)
  coder_page_compose(cd_state(c), c->composition);
  coder_page_flat_pictures(cd_state(c), CD_FLAT_PICTURES && c->composition);
  coder_page_frost(cd_state(c), (c->reduced & 2) == 0);
  // the clock scroll events are stamped on (NSEvent.timestamp: the system's uptime), for the coasts and the rails
  coder_page_now(cd_state(c), cd_uptime());
  int n = (int)coder_page_frame(cd_state(c), (coax_v_f32){vertices, (int64_t)c->app.max_vertices * coaxial_components_vertex_floats()}, w, h, dpr, dt);
  if (atlas_serve(&c->app)) cd_changed(c);
  if (cd_serve_images(c)) cd_changed(c);
  if (c->settle > 0) c->settle--;
  return n < 0 ? 0 : n;
}
// A frame with a frosted surface or image is composition-v1's plan (page/compose.coax); the rest are flat.
static int cd_composition(void *user, UiComposition *out) {
  CoderHost *c = user;
  if (!coder_page_composed(cd_state(c))) return 0;
  coax_t_UcPlan *p = c->s->cmp_gpu.e->plan.e;
  *out = (UiComposition){p->ops.e, (uint32_t)p->n, (uint32_t)p->targets, (uint32_t)p->words};
  return 1;
}
// A composed frame the GPU refused: the page draws flat from then on.
static void cd_presented(void *user, int accepted) { if (!accepted) coder_page_compose_refused(cd_state((CoderHost *)user)); }
static float cd_icon_size(int i) { return coder_page_icon_size(i); }
static float cd_font_size(int k) { return coder_page_font_size(k); }
static void cd_on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  CoderHost *c = user;
  // a dialog or a menu over the page takes every press; the host then reads any answer it gave
  if (coder_page_overlay_open(cd_state(c))) {
    (void)coder_page_overlay_press(cd_state(c), x, y, phase, clicks, mods);
    if (c->edge) { c->edge = 0; ui_cursor(c->app.host, 0); }
    cd_woke(c);
    if (c->overlay) c->overlay(c->overlay_context);
    return;
  }
  // a field's text takes a press first (the caret, a word, a line, Shift's extent), the page the rest
  coder_page_press(cd_state(c), x, y, phase, clicks, mods);
  // over a sidebar's edge, and while one is dragged, the pointer is the resize arrows
  int edge = coder_page_edge(cd_state(c)) ? 3 : 0;
  if (edge != c->edge) { c->edge = edge; ui_cursor(c->app.host, edge); }
  cd_woke(c);
}
// The keyboard while the view has it (page/editor.coax): the field with it edits, Return sends.
// Command-C, -X and -V are the pasteboard's, here; the page has no effects.
static char cd_clip[16384 + 1];
static int cd_on_key(void *user, int key, int mods, int down) {
  CoderHost *c = user;
  if (!down) return 0;
  int over = coder_page_overlay_open(cd_state(c)), took = 0;
  if ((mods & 8) && !(mods & 6) && (key == 'c' || key == 'x' || key == 'v') && coder_page_wants_text(cd_state(c))) {
    if (key == 'v') {
      // files or a picture on the pasteboard join the draft's attachments (cd_on_file); else its text
#if defined(__APPLE__) || defined(_WIN32)
      int n = coder_page_takes_files(cd_state(c)) ? ui_paste_files(c->app.host) : 0;
      if (n == 0) n = ui_paste_text(c->app.host, cd_clip, sizeof cd_clip); else n = 0;
#else
      int n = 0;
#endif
      if (n > 0) (void)coder_page_paste(cd_state(c), (coax_string){cd_clip, n});
    } else {
      int n = (int)coder_page_copy(cd_state(c), key == 'x', (coax_v_u8){(uint8_t *)cd_clip, (int64_t)sizeof cd_clip - 1});
      if (n > 0) (void)ui_copy(c->app.host, cd_clip, (size_t)n);
    }
    took = 1;
  } else took = coder_page_key(cd_state(c), key, mods) != 0;
  if (took) cd_woke(c);
  if (over && c->overlay) c->overlay(c->overlay_context);
  return took;
}
// A file the person gave the window (dropped, pasted or chosen): a regular file joins the draft's
// attachments with its name and size (page/attach.coax). Files carried over the window show "Drop to attach".
static void cd_on_file(void *user, int request, const char *path) {
  CoderHost *c = user; (void)request;
  struct stat st;
  if (!path || stat(path, &st) != 0 || !S_ISREG(st.st_mode)) return;
  const char *slash = strrchr(path, '/');
  (void)coder_page_attach(cd_state(c), cd_string(path), cd_string(slash ? slash + 1 : path), (int64_t)st.st_size);
  cd_woke(c);
}
static void cd_on_carrying(void *user, int on) { CoderHost *c = user; coder_page_carrying(cd_state(c), on != 0); cd_woke(c); }
static int cd_on_wants_text(void *user) { return coder_page_wants_text(cd_state((CoderHost *)user)) != 0; }
// The input method over the field with the keyboard (ui-host.h UiIme; UTF-16 units, device pixels).
static void cd_ime_replace(void *user, int from, int to, const char *bytes, size_t n) { CoderHost *c = user; coder_page_ime_replace(cd_state(c), from, to, (coax_string){bytes, (int64_t)n}); cd_woke(c); }
static void cd_ime_mark(void *user, int from, int to, const char *bytes, size_t n, int sel_from, int sel_to) { CoderHost *c = user; coder_page_ime_mark(cd_state(c), from, to, (coax_string){bytes, (int64_t)n}, sel_from, sel_to); cd_woke(c); }
static void cd_ime_unmark(void *user) { CoderHost *c = user; coder_page_ime_unmark(cd_state(c)); cd_woke(c); }
static int cd_ime_range(void *user, int which) { return (int)coder_page_ime_range(cd_state((CoderHost *)user), which); }
static int cd_ime_text(void *user, int from, int to, char *out, int cap) { return cap > 0 ? (int)coder_page_ime_text(cd_state((CoderHost *)user), from, to, (coax_v_u8){(uint8_t *)out, cap}) : 0; }
static int cd_ime_rect(void *user, int at, float rect[4]) { return coder_page_ime_rect(cd_state((CoderHost *)user), at, (coax_v_f32){rect, 4}) ? 1 : 0; }
static int cd_ime_index(void *user, float x, float y) { return (int)coder_page_ime_index(cd_state((CoderHost *)user), x, y); }
static const UiIme cd_ime = {cd_ime_replace, cd_ime_mark, cd_ime_unmark, cd_ime_range, cd_ime_text, cd_ime_rect, cd_ime_index};
static void cd_on_wheel(void *user, float dy) { CoderHost *c = user; if (coder_page_overlay_open(cd_state(c))) { coder_page_overlay_scroll(cd_state(c), dy, 0, 1); cd_woke(c); return; } coder_page_wheel(cd_state(c), dy); cd_woke(c); }
static void cd_on_scroll(void *user, float dy, int precise, int phase, int momentum, double time) { CoderHost *c = user; if (coder_page_overlay_open(cd_state(c))) { coder_page_overlay_scroll(cd_state(c), dy, precise != 0, 0); cd_woke(c); return; } coder_page_scroll(cd_state(c), dy, precise != 0, phase, momentum, time); cd_woke(c); }

void *cd_open_ex(const char *resources, int offscreen, int width, int height) {
  CoderHost *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  App *a = &c->app;
  snprintf(a->resources, sizeof a->resources, "%s", resources);
  char shader[PATH_MAX + 32];
#ifdef __APPLE__
  snprintf(shader, sizeof shader, "%s/components.metal", resources);
  snprintf(c->composition_shader, sizeof c->composition_shader, "%s/composition.metal", resources);
  const int embedded = 1;
#else
  snprintf(shader, sizeof shader, "%s/components.spv", resources);
  snprintf(c->composition_shader, sizeof c->composition_shader, "%s/composition.spv", resources);
  const int embedded = 0;
#endif
  FILE *composition = fopen(c->composition_shader, "rb");
  if (composition) fclose(composition);
  if (!cd_alloc(c)) { free(c); return NULL; }
  a->icon_size = cd_icon_size; a->font_size = cd_font_size;
  a->atlas = c->s->atlas.e; a->max_vertices = (uint32_t)coder_page_max_vertices();
  UiApp app = {.title = "OpenAgents", .width = width > 0 ? width : 1120, .height = height > 0 ? height : 760, .embedded = embedded, .max_vertices = a->max_vertices, .shader = shader, .user = c,
               .frame = cd_on_frame, .pointer = cd_on_pointer, .wheel = cd_on_wheel, .scroll_gesture = cd_on_scroll,
               .key = cd_on_key, .wants_text = cd_on_wants_text, .ime = &cd_ime, .file = cd_on_file, .carrying = cd_on_carrying,
               .composition_shader = composition ? c->composition_shader : NULL, .composition = composition ? cd_composition : NULL, .image = composition ? cd_image : NULL, .presented = cd_presented};
  a->host = ui_open(&app, offscreen != 0);
  if (!a->host) { free(c->s); free(c->st); free(c); return NULL; }
  c->composition = composition != NULL;
  coder_page_compose(cd_state(c), c->composition);
  // the atlas at the scale the page draws its units at
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  c->screen = a->dpr;
  a->dpr *= coder_page_scale(cd_state(c));
  if (!atlas_setup(a)) { gallery_free(a); free(c->s); free(c->st); free(c); return NULL; }
  cd_changed(c);
  return c;
}
// The text size (a row of the page's sizes: 0 to 7, 2 the usual). The fonts and icons are drawn
// again at the new scale, so text stays sharp. Returns the row it is at.
static int cd_rescale(CoderHost *c, int row) {
  App *a = &c->app;
  float want = c->screen * coder_page_scale(cd_state(c));
  if (want == a->dpr) return row;
  for (int i = 0; i < 4; i++) { free(a->font_bytes[i]); a->font_bytes[i] = NULL; }
  for (int i = 0; i < 64; i++) { free(a->svg[i]); a->svg[i] = NULL; }
  free(a->mono); free(a->color); free(a->work); a->mono = NULL; a->color = NULL; a->work = NULL;
  memset(a->atlas, 0, sizeof *a->atlas); memset(&a->plan, 0, sizeof a->plan); memset(&a->fonts, 0, sizeof a->fonts);
  a->dpr = want;
  if (!atlas_setup(a)) fprintf(stderr, "Coder: the fonts could not be drawn at the new size\n");
  cd_changed(c);
  return row;
}
int cd_zoom(void *page, int by) { CoderHost *c = page; return c ? cd_rescale(c, (int)coder_page_zoom(cd_state(c), by)) : 2; }
int cd_zoom_set(void *page, int row) { CoderHost *c = page; return c ? cd_rescale(c, (int)coder_page_zoom_set(cd_state(c), row)) : 2; }
void *cd_open(const char *resources) { return cd_open_ex(resources, 0, 900, 640); }
void *cd_view(void *page) {
#if defined(__APPLE__) || defined(UI_HOST_STUB_H)
  return page ? ui_view(((CoderHost *)page)->app.host) : NULL;
#else
  (void)page; return NULL;
#endif
}
void cd_frame(void *page) { if (page) ui_frame(((CoderHost *)page)->app.host); }
int cd_step(void *page) { return page ? ui_frame(((CoderHost *)page)->app.host) : 0; }
int cd_shot_frame(void *page, uint32_t w, uint32_t h, float dt) { return page ? ui_shot_frame(((CoderHost *)page)->app.host, w, h, dt) : 0; }
int cd_shot_write(void *page, const char *png) { return page ? ui_shot_write(((CoderHost *)page)->app.host, png) : 0; }
#ifdef _WIN32
// The clipboard's text (Windows: the app reads a key the person copied, windows/main.c).
int cd_paste_text(void *page, char *out, int cap) { return page && out && cap > 0 ? ui_paste_text(((CoderHost *)page)->app.host, out, (size_t)cap) : 0; }
#endif
int cd_copy(void *page, const char *bytes, int n) { return page && bytes && n > 0 ? ui_copy(((CoderHost *)page)->app.host, bytes, (size_t)n) : 0; }
int cd_focused(void *page) { return page ? ui_focused(((CoderHost *)page)->app.host) : 0; }
int cd_moving(void *page) { CoderHost *c = page; return c && (c->settle > 0 || coder_page_moving(cd_state(c))); }
void cd_on_change(void *page, void (*changed)(void *context), void *context) { CoderHost *c = page; if (c) { c->changed = changed; c->context = context; } }
void cd_env(void *page, int active, int reduced) {
  CoderHost *c = page;
  if (!c) return;
  if (c->active != (active != 0) || c->reduced != (reduced & 3)) cd_changed(c);
  c->active = active != 0; c->reduced = reduced & 3;
}
void cd_focus(void *page, int field) { CoderHost *c = page; if (c) { coder_page_focus(cd_state(c), field); cd_woke(c); } }
int cd_wants_text(void *page) { CoderHost *c = page; return c && coder_page_wants_text(cd_state(c)); }
int cd_key(void *page, int key, int mods) { CoderHost *c = page; return c ? cd_on_key(c, key, mods, 1) : 0; }
float cd_blink(void *page) { CoderHost *c = page; return c ? coder_page_blink(cd_state(c)) : -1; }
int cd_draft(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || cap < 1) return 0;
  int n = (int)coder_page_draft(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
int cd_search_clear(void *page) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_search_clear(cd_state(c)); cd_woke(c); return ok; }
int cd_set_text(void *page, const char *text) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_set_text(cd_state(c), cd_string(text)); }
int cd_new(void *page) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_new(cd_state(c)); }
int cd_submit(void *page) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_submit(cd_state(c)); }
int cd_action(void *page) { CoderHost *c = page; return c ? (int)coder_page_action(cd_state(c)) : 0; }
int cd_task(void *page, const char *session, const char *cwd, char *out, int cap) {
  CoderHost *c = page;
  return c && cap > 0 ? (int)coder_page_task(cd_state(c), cd_string(session), cd_string(cwd), (coax_v_u8){(uint8_t *)out, cap}) : 0;
}
void cd_skills(void *page, const char *note) { CoderHost *c = page; if (c) coder_page_skills(cd_state(c), cd_string(note)); }
void cd_folder(void *page, const char *shown) { CoderHost *c = page; if (c) { cd_changed(c); coder_page_folder(cd_state(c), cd_string(shown)); } }
void cd_branch(void *page, const char *name) { CoderHost *c = page; if (c) { cd_changed(c); coder_page_branch(cd_state(c), cd_string(name)); } }
int cd_answer_line(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_answer_line(cd_state(c), (coax_v_u8){(uint8_t *)out, cap}) : 0; }
int cd_agent_next(void *page) { CoderHost *c = page; return c ? (int)coder_page_agent_next(cd_state(c)) : -1; }
int cd_agent_task(void *page, int agent, const char *session, const char *cwd, char *out, int cap) {
  CoderHost *c = page;
  return c && cap > 0 ? (int)coder_page_agent_task(cd_state(c), agent, cd_string(session), cd_string(cwd), (coax_v_u8){(uint8_t *)out, cap}) : 0;
}
void cd_agent_line(void *page, int agent, const char *line) { CoderHost *c = page; if (!c) return; coder_page_agent_line(cd_state(c), agent, cd_string(line)); cd_changed(c); }
void cd_agent_failed(void *page, int agent, const char *why) { CoderHost *c = page; if (!c) return; coder_page_agent_failed(cd_state(c), agent, cd_string(why)); cd_changed(c); }
int cd_agent_answer_line(void *page, int agent, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_agent_answer_line(cd_state(c), agent, (coax_v_u8){(uint8_t *)out, cap}) : 0; }
void cd_claude(void *page, int on) { CoderHost *c = page; if (c) coder_page_set_claude(cd_state(c), on != 0); }
void cd_agy(void *page, int on) { CoderHost *c = page; if (c) coder_page_set_agy(cd_state(c), on != 0); }
int cd_model_agy(void *page) { CoderHost *c = page; return c && coder_page_model_agy(cd_state(c)) ? 1 : 0; }
void cd_openrouter(void *page, const char *main, const char *sub) { CoderHost *c = page; if (c) { cd_changed(c); coder_page_set_openrouter(cd_state(c), cd_string(main), cd_string(sub)); } }
int cd_pick_model(void *page, const char *name) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_pick_model(cd_state(c), cd_string(name)); }
// The sidebars' widths as dragged: read (1 once after a drag, when they are to be kept) and set.
int cd_sides(void *page, float out[2]) { CoderHost *c = page; return c && coder_page_sides(cd_state(c), (coax_v_f32){out, 2}); }
void cd_sides_set(void *page, float left, float right) { CoderHost *c = page; if (c) { coder_page_sides_set(cd_state(c), left, right); cd_woke(c); } }
int cd_pane(void *page, int n) { CoderHost *c = page; return c && coder_page_pane(cd_state(c), n); }
int cd_agent_claude(void *page, int agent) { CoderHost *c = page; return c && coder_page_agent_claude(cd_state(c), agent); }
int cd_agent_running(void *page, int agent) { CoderHost *c = page; return c && coder_page_agent_running(cd_state(c), agent); }
int cd_agent_cancel(void *page, int agent) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_agent_cancel(cd_state(c), agent); }
int cd_stop_agent(void *page) { CoderHost *c = page; return c ? (int)coder_page_stop_agent(cd_state(c)) : -1; }
// Kept chats (page.coax): the list, the chat that is shown, and a chat's text each way.
static int cd_text_out(int n, char *out, int cap) { if (n < 0 || n >= cap) n = 0; out[n] = 0; return n; }
void cd_chats(void *page, const char *list) { CoderHost *c = page; if (c) { coder_page_chats(cd_state(c), cd_string(list)); cd_changed(c); } }
void cd_current(void *page, const char *id) { CoderHost *c = page; if (c) { coder_page_current(cd_state(c), cd_string(id)); cd_changed(c); } }
void cd_foreman(void *page, int busy, int tier) { CoderHost *c = page; if (c) { coder_page_foreman(cd_state(c), busy != 0, tier); cd_changed(c); } }
void cd_cmd_hints(void *page, int shown) { CoderHost *c = page; if (c) { coder_page_cmd_hints(cd_state(c), shown != 0); cd_changed(c); } }
void cd_toast(void *page, const char *text, int tone) { CoderHost *c = page; if (c) { coder_page_toast(cd_state(c), cd_string(text), tone); cd_changed(c); } }
void cd_update(void *page, int state, const char *label, const char *detail) { CoderHost *c = page; if (c) { coder_page_update(cd_state(c), state, cd_string(label), cd_string(detail)); cd_changed(c); } }
int cd_open_path(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_open_path(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_open_url(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_open_url(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_open_id(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_open_id(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_pin_move(void *page, char *id_out, int cap, int *to_out) {
  CoderHost *c = page;
  if (!c || cap < 2 || !to_out) return 0;
  int n = cd_text_out((int)coder_page_pin_move_id(cd_state(c), (coax_v_u8){(uint8_t *)id_out, cap - 1}), id_out, cap);
  *to_out = (int)coder_page_pin_move_to(cd_state(c));
  return n;
}
int cd_chat_at(void *page, float x, float y, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_chat_at(cd_state(c), x, y, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_foreman_at(void *page, float x, float y) { CoderHost *c = page; return c ? (int)coder_page_foreman_at(cd_state(c), x, y) : 0; }
int cd_title(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_title(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_save_due(void *page) { CoderHost *c = page; return c && coder_page_save_due(cd_state(c)); }
int cd_save(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_save(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_load(void *page, const char *text) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_load(cd_state(c), cd_string(text)); }
int cd_recall_line(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_recall_line(cd_state(c), (coax_v_u8){(uint8_t *)out, cap}) : 0; }
int cd_stop_line(char *out, int cap) { return cap > 0 ? (int)coder_page_stop_line((coax_v_u8){(uint8_t *)out, cap}) : 0; }
int cd_forget_line(char *out, int cap) { return cap > 0 ? (int)coder_page_forget_line((coax_v_u8){(uint8_t *)out, cap}) : 0; }
// Follow-ups (page/followups.coax; GitHub #13715): steer the running turn, queue messages for after
// it, interrupt it. Each *_line is the line to write to the worker, with its newline, 0 for none.
int cd_steer_line(void *page, const char *id, const char *text, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_steer_line(cd_state(c), cd_string(id), cd_string(text), (coax_v_u8){(uint8_t *)out, cap}) : 0; }
int cd_queue_message_line(void *page, const char *op, const char *id, const char *text, const char *session, const char *cwd, char *out, int cap) {
  CoderHost *c = page;
  return c && cap > 0 ? (int)coder_page_queue_message_line(cd_state(c), cd_string(op), cd_string(id), cd_string(text), cd_string(session), cd_string(cwd), (coax_v_u8){(uint8_t *)out, cap}) : 0;
}
int cd_queue_line(void *page, const char *op, const char *id, int to, int now, const char *key, char *out, int cap) {
  CoderHost *c = page;
  return c && cap > 0 ? (int)coder_page_queue_line(cd_state(c), cd_string(op), cd_string(id), to, now != 0, cd_string(key), (coax_v_u8){(uint8_t *)out, cap}) : 0;
}
int cd_interrupt_line(void *page, int resubmit, char *out, int cap) { CoderHost *c = page; if (!c || cap <= 0) return 0; cd_changed(c); return (int)coder_page_interrupt_line(cd_state(c), resubmit != 0, (coax_v_u8){(uint8_t *)out, cap}); }
int cd_followup_began(void *page) { CoderHost *c = page; return c && coder_page_followup_began(cd_state(c)); }
int cd_queue_waiting(void *page) { CoderHost *c = page; return c && coder_page_queue_waiting(cd_state(c)); }
int cd_queue_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_queue_count(cd_state(c)) : 0; }
int cd_queue_paused(void *page) { CoderHost *c = page; return c && coder_page_queue_paused(cd_state(c)); }
int cd_queue_reason(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_queue_reason(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_queue_item_id(void *page, int i, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_queue_item_id(cd_state(c), i, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_queue_item_text(void *page, int i, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_queue_item_text(cd_state(c), i, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_queue_item_cut(void *page, int i) { CoderHost *c = page; return c && coder_page_queue_item_cut(cd_state(c), i); }
int cd_steer_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_steer_count(cd_state(c)) : 0; }
int cd_steer_status(void *page, int i) { CoderHost *c = page; return c ? (int)coder_page_steer_status(cd_state(c), i) : -1; }
int cd_steer_text(void *page, int i, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_steer_text(cd_state(c), i, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_followup_submit(void *page, int other) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_followup_submit(cd_state(c), other != 0); }
int cd_followup_escape(void *page) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_followup_escape(cd_state(c)); }
int cd_followup_op(void *page) { CoderHost *c = page; return c ? (int)coder_page_followup_op(cd_state(c)) : 0; }
int cd_followup_item(void *page) { CoderHost *c = page; return c ? (int)coder_page_followup_item(cd_state(c)) : -1; }
void cd_followup_editing(void *page, int i) { CoderHost *c = page; if (c) { cd_changed(c); coder_page_followup_editing(cd_state(c), i); } }
void cd_followup_mode(void *page, int queue_first) { CoderHost *c = page; if (c) { cd_changed(c); coder_page_followup_mode(cd_state(c), queue_first != 0); } }
int cd_followup_edited(void *page) { CoderHost *c = page; return c ? (int)coder_page_followup_edited(cd_state(c)) : -1; }
int cd_followup_queues(void *page) { CoderHost *c = page; return c && coder_page_followup_queues(cd_state(c)); }
int cd_steer_id(void *page, int i, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_steer_id(cd_state(c), i, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
void cd_steer_dismiss(void *page, int i) { CoderHost *c = page; if (c) { cd_changed(c); coder_page_steer_dismiss(cd_state(c), i); } }
int cd_steer_reason(void *page, int i, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_steer_reason(cd_state(c), i, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_worker_turn(void *page) { CoderHost *c = page; return c ? (int)coder_page_worker_turn(cd_state(c)) : 0; }
void cd_line(void *page, const char *line) { CoderHost *c = page; if (!c) return; coder_page_line(cd_state(c), cd_string(line)); cd_changed(c); }
void cd_failed(void *page, const char *why) { CoderHost *c = page; if (!c) return; coder_page_failed(cd_state(c), cd_string(why)); cd_changed(c); }
int cd_running(void *page) { CoderHost *c = page; return c && coder_page_running(cd_state(c)); }
int64_t cd_cost_usd_micros(void *page) { CoderHost *c = page; return c ? coder_page_cost_usd_micros(cd_state(c)) : 0; }
int64_t cd_budget_usd_micros(void *page) { CoderHost *c = page; return c ? coder_page_budget_usd_micros(cd_state(c)) : 50000000; }
int cd_set_budget_usd_micros(void *page, int64_t micros) { CoderHost *c = page; return c ? (int)coder_page_set_budget_usd_micros(cd_state(c), micros) : 0; }
int cd_budget_exceeded(void *page) { CoderHost *c = page; return c ? (int)coder_page_budget_exceeded(cd_state(c)) : 0; }
int cd_budget_approve(void *page) { CoderHost *c = page; return c ? (int)coder_page_budget_approve(cd_state(c)) : 0; }
int cd_goal_status(void *page) { CoderHost *c = page; return c ? (int)coder_page_goal_status(cd_state(c)) : 0; }
int cd_goal_time(void *page) { CoderHost *c = page; return c ? (int)coder_page_goal_time(cd_state(c)) : 0; }
int cd_goal_objective(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || cap < 1) return 0;
  int n = (int)coder_page_goal_objective(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
int cd_goal_set(void *page, const char *objective) { CoderHost *c = page; if (!c) return 0; int ok = (int)coder_page_goal_set(cd_state(c), cd_string(objective)); cd_changed(c); return ok; }
int cd_goal_set_status(void *page, int status) { CoderHost *c = page; if (!c) return 0; int ok = (int)coder_page_goal_set_status(cd_state(c), status); cd_changed(c); return ok; }
int cd_goal_clear(void *page) { CoderHost *c = page; if (!c) return 0; int ok = (int)coder_page_goal_clear(cd_state(c)); cd_changed(c); return ok; }
int cd_copied(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || cap < 1) return 0;
  int n = (int)coder_page_copied(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0; cd_changed(c);
  return n;
}
int cd_box(void *page, const char *id, float box[4]) { CoderHost *c = page; return c && coder_page_box(cd_state(c), cd_string(id), (coax_v_f32){box, 4}); }
// A page with no window, for a chat whose reply goes on out of view: it reads its workers' lines
// and keeps its text as the window's page does. cd_close frees it.
void *cd_headless(void) {
  CoderHost *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  if (!cd_alloc(c)) { free(c); return NULL; }
  c->headless = 1;
  coder_page_init(cd_state(c));
  return c;
}
// The chat in from moves to to, run and all; from is left a new chat. 0 when to holds a running chat.
int cd_take(void *to, void *from) {
  CoderHost *t = to, *f = from;
  if (!t || !f) return 0;
  int ok = coder_page_take(cd_state(t), cd_state(f));
  cd_changed(t); cd_changed(f);
  return ok;
}
// The draft's attachments (page/attach.coax): a file chosen in the open panel; how many it carries;
// taken away (a follow-up sent them); the draft as a follow-up sends it, with the note on what it
// carries; where the + is (device pixels) for its menu; the working folder's files, one a line.
int cd_attach(void *page, const char *path) { CoderHost *c = page; if (!c) return 0; int had = (int)coder_page_attached(cd_state(c)); cd_on_file(c, 1, path); return coder_page_attached(cd_state(c)) > had; }
int cd_attached(void *page) { CoderHost *c = page; return c ? (int)coder_page_attached(cd_state(c)) : 0; }
void cd_attach_clear(void *page) { CoderHost *c = page; if (c) { coder_page_attach_clear(cd_state(c)); cd_changed(c); } }
int cd_draft_message(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_draft_message(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_add_at(void *page, float out[2]) { CoderHost *c = page; return c && coder_page_add_at(cd_state(c), (coax_v_f32){out, 2}); }
void cd_files(void *page, const char *lines) {
  CoderHost *c = page;
  if (!c || !lines) return;
  size_t n = strlen(lines); if (n > (size_t)coder_page_files_bytes()) n = (size_t)coder_page_files_bytes();
  while (n > 0 && lines[n - 1] != '\n') n--;   // whole lines only
  char *keep = malloc(n + 1);
  if (!keep) return;
  memcpy(keep, lines, n); keep[n] = 0;
  coder_page_files(cd_state(c), (coax_v_u8){(uint8_t *)keep, (int64_t)n});
  free(c->files); c->files = keep;
  cd_woke(c);
}
// The file in the pane (page/filepane.coax): the page reads its bytes from the carrier's copy.
static char *cd_keep(const char *bytes, int len) { char *keep = malloc(len > 0 ? (size_t)len : 1); if (keep && len > 0) memcpy(keep, bytes, (size_t)len); return keep; }
int cd_file_most(void) { return (int)coder_page_file_most(); }
int cd_file_open(void *page, const char *path, const char *bytes, int len, long long size) {
  CoderHost *c = page;
  if (!c || len < 0 || len > cd_file_most()) return 0;
  char *keep = cd_keep(bytes, len);
  if (!keep) return 0;
  int ok = coder_page_file_open(cd_state(c), cd_string(path), (coax_v_u8){(uint8_t *)keep, len}, size);
  if (!ok) { free(keep); return 0; }
  free(c->file); c->file = keep;
  cd_woke(c);
  return 1;
}
void cd_file_failed(void *page, const char *path, const char *why) { CoderHost *c = page; if (c) { coder_page_file_failed(cd_state(c), cd_string(path), cd_string(why)); cd_woke(c); } }
int cd_file_text(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_file_text(cd_state(c), (coax_v_u8){(uint8_t *)out, cap}) : -1; }
void cd_file_saved(void *page, const char *bytes, int len) {
  CoderHost *c = page;
  if (!c || len < 0) return;
  char *keep = cd_keep(bytes, len);
  if (!keep) return;
  coder_page_file_saved(cd_state(c), (coax_v_u8){(uint8_t *)keep, len});
  free(c->file); c->file = keep;
  cd_woke(c);
}
void cd_file_save_failed(void *page, const char *why) { CoderHost *c = page; if (c) { coder_page_file_save_failed(cd_state(c), cd_string(why)); cd_woke(c); } }
void cd_file_close(void *page) { CoderHost *c = page; if (c) { coder_page_file_close(cd_state(c)); free(c->file); c->file = NULL; cd_woke(c); } }
int cd_file_front(void *page) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_file_front(cd_state(c)); cd_woke(c); return ok; }
int cd_file_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_file_mode(cd_state(c)) : 0; }
int cd_file_dirty(void *page) { CoderHost *c = page; return c && coder_page_file_dirty(cd_state(c)); }
int cd_file_shown(void *page) { CoderHost *c = page; return c && coder_page_file_shown(cd_state(c)); }
int cd_file_name(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_file_name(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_file_open_scene(void *page, const char *path) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_file_open_scene(cd_state(c), cd_string(path)); if (ok) { free(c->file); c->file = NULL; cd_woke(c); } return ok; }
int cd_file_scene_rect(void *page, float out[4]) { CoderHost *c = page; return c && out && coder_page_file_scene_rect(cd_state(c), (coax_v_f32){out, 4}); }
int cd_live_scene(void *page, float out[8], char *path, int cap) { CoderHost *c = page; return c && out && cap > 1 ? cd_text_out((int)coder_page_live_scene(cd_state(c), (coax_v_f32){out, 8}, (coax_v_u8){(uint8_t *)path, cap - 1}), path, cap) : 0; }
// The iOS Simulator in the pane (page/simpane.coax, GitHub #13811): the page speaks the helper's
// wire itself; the host moves its lines and shows the screen.
void cd_sim_setup(void *page, const char *stills, const char *shots) { CoderHost *c = page; if (c) coder_page_sim_setup(cd_state(c), cd_string(stills), cd_string(shots)); }
void cd_sim_show(void *page, int open) { CoderHost *c = page; if (c) { coder_page_sim_show(cd_state(c), open != 0); cd_woke(c); } }
int cd_sim_open(void *page) { CoderHost *c = page; return c && coder_page_sim_open(cd_state(c)); }
void cd_sim_connected(void *page, int ok, const char *why) { CoderHost *c = page; if (c) { coder_page_sim_connected(cd_state(c), ok != 0, cd_string(why)); cd_woke(c); } }
int cd_sim_line(void *page, const char *line, int len) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_sim_line(cd_state(c), (coax_string){line, len}); cd_woke(c); return ok; }
int cd_sim_out(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_sim_out(cd_state(c), (coax_v_u8){(uint8_t *)out, cap}) : 0; }
void cd_sim_tick(void *page) { CoderHost *c = page; if (c) coder_page_sim_tick(cd_state(c)); }
int cd_sim_rect(void *page, float out[6]) { CoderHost *c = page; return c && out && coder_page_sim_rect(cd_state(c), (coax_v_f32){out, 6}); }
int cd_sim_still(void *page, char *out, int cap) { CoderHost *c = page; if (!c || cap < 2) return 0; out[0] = 0; return (int)coder_page_sim_still(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}); }
int cd_sim_op(void *page) { CoderHost *c = page; return c ? (int)coder_page_sim_op(cd_state(c)) : 0; }
int cd_sim_saved(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_sim_saved(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_sim_menu(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_sim_menu(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_sim_pick(void *page, int i) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_sim_pick(cd_state(c), i); cd_woke(c); return ok; }
int cd_sim_device(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_sim_device(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_sim_tips(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_sim_tips(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_sim_touch(void *page, int phase, float x, float y, float w, float h) { CoderHost *c = page; return c && coder_page_sim_touch(cd_state(c), phase, x, y, w, h); }
int cd_sim_key(void *page, int code, const char *chars) { CoderHost *c = page; return c && coder_page_sim_key(cd_state(c), code, cd_string(chars)); }
int cd_sim_pinch(void *page, int phase, float x, float y, float w, float h) { CoderHost *c = page; return c && coder_page_sim_pinch(cd_state(c), phase, x, y, w, h); }
int cd_sim_scroll(void *page, int phase, float x, float y, float dx, float dy, float w, float h) { CoderHost *c = page; return c && coder_page_sim_scroll(cd_state(c), phase, x, y, dx, dy, w, h); }
int cd_sim_command(void *page, int code, const char *chars, int shift) { CoderHost *c = page; return c && coder_page_sim_command(cd_state(c), code, cd_string(chars), shift != 0); }
void cd_tick(void *page, float dt) { CoderHost *c = page; if (c) coder_page_tick(cd_state(c), dt); }
void cd_close(void *page) {
  CoderHost *c = page;
  if (!c) return;
  if (!c->headless) gallery_free(&c->app);
  for (int i = 0; i < CD_IMAGES; i++) free(c->images[i].rgba);
  free(c->files); free(c->file); free(c->s); free(c->st); free(c);
}
// Settings (settings.coax): open it on a page, scrolled to a row (page < -1 closes it); the page
// it shows (-1 closed); what its pages show; the tier a press chose; the search's text.
void cd_settings_open(void *page, int which, const char *row) { CoderHost *c = page; if (c) { coder_page_settings_open(cd_state(c), which, cd_string(row)); cd_woke(c); } }
int cd_settings_page(void *page) { CoderHost *c = page; return c ? (int)coder_page_settings_page(cd_state(c)) : -1; }
void cd_settings_info(void *page, const char *claude, const char *openrouter, const char *version) { CoderHost *c = page; if (c) { coder_page_settings_info(cd_state(c), cd_string(claude), cd_string(openrouter), cd_string(version)); cd_changed(c); } }
void cd_settings_gemini(void *page, const char *gemini) { CoderHost *c = page; if (c) { coder_page_settings_gemini(cd_state(c), cd_string(gemini)); cd_changed(c); } }
void cd_settings_instructions(void *page, const char *words) { CoderHost *c = page; if (c) { coder_page_settings_instructions(cd_state(c), cd_string(words)); cd_changed(c); } }
void cd_settings_gauth(void *page, const char *kind, const char *text) { CoderHost *c = page; if (c) { coder_page_settings_gauth(cd_state(c), cd_string(kind), cd_string(text)); cd_changed(c); } }
void cd_agy_seen(void *page, const char *id, const char *name) { CoderHost *c = page; if (c) { coder_page_agy_seen(cd_state(c), cd_string(id), cd_string(name)); cd_changed(c); } }
void cd_settings_agy(void *page, const char *status, int mode, const char *model, const char *auth) { CoderHost *c = page; if (c) { coder_page_settings_agy(cd_state(c), cd_string(status), mode, cd_string(model), cd_string(auth)); cd_changed(c); } }
int cd_settings_agy_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_settings_agy_mode(cd_state(c)) : 0; }
int cd_settings_tier(void *page) { CoderHost *c = page; return c ? (int)coder_page_settings_tier(cd_state(c)) : 0; }
void cd_search_set(void *page, const char *text) { CoderHost *c = page; if (c) { coder_page_search_set(cd_state(c), cd_string(text)); cd_woke(c); } }
int cd_bar_drags(void *page, float x, float y) { CoderHost *c = page; return c && coder_page_bar_drags(cd_state(c), x, y); }
void cd_bar_full(void *page, int on) { CoderHost *c = page; if (c) { coder_page_bar_full(cd_state(c), on != 0); cd_woke(c); } }
int cd_search_submit(void *page) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_search_submit(cd_state(c)); cd_woke(c); return ok; }
// Dialogs and right-click menus over the page (page/overlay.coax, GitHub #13746).
void cd_on_overlay(void *page, void (*fn)(void *context), void *context) { CoderHost *c = page; if (c) { c->overlay = fn; c->overlay_context = context; } }
void cd_dialog_begin(void *page, int size, const char *title, const char *body) { CoderHost *c = page; if (c) coder_page_dialog_begin(cd_state(c), size, cd_string(title), cd_string(body)); }
int cd_dialog_field(void *page, const char *label, const char *value, const char *hint, int secure, int lines) { CoderHost *c = page; return c && coder_page_dialog_field(cd_state(c), cd_string(label), cd_string(value), cd_string(hint), secure != 0, lines); }
int cd_dialog_button(void *page, const char *label, int kind, int aside) { CoderHost *c = page; return c && coder_page_dialog_button(cd_state(c), cd_string(label), kind, aside != 0); }
void cd_dialog_show(void *page, int confirm, int cancel, int focus) { CoderHost *c = page; if (c) { coder_page_dialog_show(cd_state(c), confirm, cancel, focus); cd_woke(c); } }
int cd_dialog_answer(void *page) { CoderHost *c = page; return c ? (int)coder_page_dialog_answer(cd_state(c)) : -2; }
int cd_dialog_value(void *page, int i, char *out, int cap) {
  CoderHost *c = page;
  if (!c || cap < 1) return -1;
  int n = (int)coder_page_dialog_value(cd_state(c), i, (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n < 0 ? 0 : n] = 0;
  return n;
}
int cd_dialog_set(void *page, int i, const char *text) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_dialog_set(cd_state(c), i, cd_string(text)); }
void cd_menu_begin(void *page) { CoderHost *c = page; if (c) coder_page_menu_begin(cd_state(c)); }
int cd_menu_item(void *page, const char *label, int kind, int checked, int disabled, int danger) { CoderHost *c = page; return c && coder_page_menu_item(cd_state(c), cd_string(label), kind, checked != 0, disabled != 0, danger != 0); }
void cd_menu_show(void *page, float x, float y) { CoderHost *c = page; if (c) { coder_page_menu_show(cd_state(c), x, y); cd_woke(c); } }
int cd_menu_answer(void *page) { CoderHost *c = page; return c ? (int)coder_page_menu_answer(cd_state(c)) : -2; }
int cd_overlay_open(void *page) { CoderHost *c = page; return c && coder_page_overlay_open(cd_state(c)); }
// The command palette (page/palette.coax, GitHub #13700): ⌘K, its host-only commands, ⌃Tab, ⌘B and ⌘/.
int cd_palette_toggle(void *page, int scope) { CoderHost *c = page; if (!c) return 0; int open = coder_page_palette_toggle(cd_state(c), scope); cd_woke(c); return open; }
int cd_palette_open(void *page) { CoderHost *c = page; return c && coder_page_palette_open(cd_state(c)); }
int cd_palette_command(void *page) { CoderHost *c = page; return c ? (int)coder_page_palette_command(cd_state(c)) : 0; }
int cd_chat_step(void *page, int dir) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_chat_step(cd_state(c), dir); cd_woke(c); return ok; }
void cd_toggle_sidebar(void *page) { CoderHost *c = page; if (c) { coder_page_toggle_sidebar(cd_state(c)); cd_woke(c); } }
void cd_toggle_models(void *page) { CoderHost *c = page; if (c) { coder_page_toggle_models(cd_state(c)); cd_woke(c); } }
int cd_overlay_key(void *page, int key, int mods) { CoderHost *c = page; if (!c) return 0; int took = coder_page_overlay_key(cd_state(c), key, mods); if (took) cd_woke(c); return took; }
void cd_gym_set(void *page, int open) { CoderHost *c = page; if (!c) return; coder_page_gym_set(cd_state(c), open != 0); cd_woke(c); }
int cd_gym_open(void *page) { CoderHost *c = page; return c ? coder_page_gym_open(cd_state(c)) : 0; }
void cd_experimental_set(void *page, int on) { CoderHost *c = page; if (!c) return; coder_page_experimental_set(cd_state(c), on != 0); cd_woke(c); }
int cd_experimental(void *page) { CoderHost *c = page; return c ? coder_page_experimental(cd_state(c)) : 0; }
void cd_cloud_authed_set(void *page, int on) { CoderHost *c = page; if (!c) return; coder_page_set_cloud_authed(cd_state(c), on != 0); cd_woke(c); }
int cd_gym_rect(void *page, float out[4]) { CoderHost *c = page; return c && out && coder_page_gym_rect(cd_state(c), (coax_v_f32){out, 4}); }
