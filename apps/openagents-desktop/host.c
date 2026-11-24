// Firm reason: 6 (native GPU carrier for the compiled page; state, layout and paint are Coax).
// Coder's page on the shared carrier (host.h). carrier.c includes it after the component
// carrier (components/gallery/linux/main.c), so it shares that file's App and atlas code and the
// one generated library. It holds no behaviour: frames, clicks and lines go to page.coax.
#include "host.h"
#include "../shared/native/tailnet-codec.c"
#include <time.h>
#include <errno.h>
#include <sys/stat.h>
#if defined(__APPLE__) || defined(__linux__)
#include <fcntl.h>
#include <signal.h>
#include <sys/ioctl.h>
#include <unistd.h>
#ifdef __APPLE__
#include <util.h>
#else
#include <pty.h>
#endif
#endif
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
  char *gitdiff; // live working-tree git status + diff buffer (cd_git_diff_set): the page reads it here
  uint8_t *term_bufs[8];   // 8 MiB per-tab retained scrollback buffers (page/termpane.coax, #13869)
  char *ckpt_diff;   // the per-turn checkpoint diff in the Changes pane (cd_ckpt_diff_set; #13872)
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
  // Cross-platform PNG and baseline JPEG decoding via the shared Coax decoder (coax/lib/image/image.coax),
  // which outputs straight RGBA8 for composition-v1 image slots on Linux and Windows.
  if (!path || !*path) return 0;
  FILE *f = fopen(path, "rb");
  if (!f) return 0;
  if (fseek(f, 0, SEEK_END) != 0) { fclose(f); return 0; }
  long flen = ftell(f);
  if (flen <= 0 || flen > 32L * 1024L * 1024L) { fclose(f); return 0; }
  if (fseek(f, 0, SEEK_SET) != 0) { fclose(f); return 0; }
  uint8_t *raw = malloc((size_t)flen);
  if (!raw) { fclose(f); return 0; }
  size_t got = fread(raw, 1, (size_t)flen, f);
  fclose(f);
  if (got != (size_t)flen) { free(raw); return 0; }
  int64_t iw = 0, ih = 0, iwork = 0;
  int64_t st = coder_img_info((coax_v_u8){raw, (int64_t)got}, (coax_v_s64){&iw, 1}, (coax_v_s64){&ih, 1}, (coax_v_s64){&iwork, 1});
  if (st != 0 || iw < 1 || ih < 1 || iw > 8192 || ih > 8192 || iwork < 0 || iwork > 64L * 1024L * 1024L) {
    free(raw);
    return 0;
  }
  uint8_t *work = malloc(iwork > 0 ? (size_t)iwork : 1);
  uint8_t *rgba = malloc((size_t)iw * (size_t)ih * 4);
  if (!work || !rgba) { free(work); free(rgba); free(raw); return 0; }
  st = coder_img_decode((coax_v_u8){raw, (int64_t)got}, (coax_v_u8){work, iwork}, (coax_v_u8){rgba, iw * ih * 4});
  free(work);
  free(raw);
  if (st != 0) { free(rgba); return 0; }
  while (iw > CD_IMAGE_MAX_SIDE || ih > CD_IMAGE_MAX_SIDE) {
    int64_t dw = iw > 1 ? iw / 2 : 1;
    int64_t dh = ih > 1 ? ih / 2 : 1;
    uint8_t *next = malloc((size_t)dw * (size_t)dh * 4);
    if (!next) break;
    coder_img_half((coax_v_u8){rgba, iw * ih * 4}, iw, ih, (coax_v_u8){next, dw * dh * 4});
    free(rgba);
    rgba = next;
    iw = dw;
    ih = dh;
  }
  if (iw > 4096 || ih > 4096) { free(rgba); return 0; }
  slot->rgba = rgba; slot->w = (uint32_t)iw; slot->h = (uint32_t)ih; slot->gen = gen;
  return 1;
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
static int cd_rescale(CoderHost *c, int row);
static int cd_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  CoderHost *c = user;
  float screen = dpr >= 1.0f ? dpr : 1.0f;
  if (screen != c->screen) {
    c->screen = screen;
    cd_rescale(c, 0);
  }
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
  // over a sidebar's edge, link, selectable text, or active editor: set the matching native cursor
  int cur = (int)coder_page_cursor(cd_state(c));
  if (cur != c->edge) { c->edge = cur; ui_cursor(c->app.host, cur); }
  cd_woke(c);
}
// The keyboard while the view has it (page/editor.coax): the field with it edits, Return sends.
// Command-C, -X and -V are the pasteboard's, here; the page has no effects.
static char cd_clip[262144 + 1];
static int cd_on_key(void *user, int key, int mods, int down) {
  CoderHost *c = user;
  if (!down) return 0;
  int over = coder_page_overlay_open(cd_state(c)), took = 0;
  int wants = coder_page_wants_text(cd_state(c)) != 0;
  int has_sel = coder_page_has_selection(cd_state(c)) != 0;
  if ((mods & 8) && !(mods & 6) && ((key == 'c' && (wants || has_sel)) || ((key == 'x' || key == 'v') && wants))) {
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
// A file the person gave the window (dropped, pasted or chosen): a regular file joins the
// draft's attachments with its name and size (page/attach.coax).
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
#if !defined(__APPLE__) && !defined(_WIN32)
// Wayland's keyboard and text-input (ui-host.h text, text_edit, text_paste): typed and composed text and
// the clipboard reach the field with the keyboard here, since the Linux host has no UiIme of its own.
static void cd_on_text(void *user, const char *bytes, size_t n) { cd_ime_replace(user, -1, -1, bytes, n); }
static void cd_on_text_edit(void *user, const char *commit, size_t n, const char *preedit, size_t m, int begin, int end) {
  CoderHost *c = user;
  if (coder_page_text_edit(cd_state(c), (coax_string){commit, (int64_t)n}, (coax_string){preedit, (int64_t)m}, begin, end)) cd_woke(c);
}
static void cd_on_text_cancel(void *user) { CoderHost *c = user; coder_page_text_cancel(cd_state(c)); cd_woke(c); }
static void cd_on_text_paste(void *user, const char *bytes, size_t n) { CoderHost *c = user; if (coder_page_paste(cd_state(c), (coax_string){bytes, (int64_t)n})) cd_woke(c); }
// The caret, for the input method's candidates: surface points from the window's top left.
static int cd_on_text_rect(void *user, float rect[4]) {
  CoderHost *c = user;
  if (!coder_page_ime_rect(cd_state(c), (int)coder_page_ime_range(cd_state(c), 1), (coax_v_f32){rect, 4})) return 0;
  float d = ui_dpr(c->app.host); if (d < 1) d = 1;
  for (int i = 0; i < 4; i++) rect[i] /= d;
  return 1;
}
#endif
static void cd_on_wheel(void *user, float dy) { CoderHost *c = user; if (coder_page_overlay_open(cd_state(c))) { coder_page_overlay_scroll(cd_state(c), dy, 0, 1); cd_woke(c); return; } coder_page_wheel(cd_state(c), dy); cd_woke(c); }
static void cd_on_scroll(void *user, float dy, int precise, int phase, int momentum, double time) { CoderHost *c = user; if (coder_page_overlay_open(cd_state(c))) { coder_page_overlay_scroll(cd_state(c), dy, precise != 0, 0); cd_woke(c); return; } coder_page_scroll(cd_state(c), dy, precise != 0, phase, momentum, time); cd_woke(c); }
static void cd_on_scroll_x(void *user, float dx, int precise) { CoderHost *c = user; if (coder_page_overlay_open(cd_state(c))) return; coder_page_scroll_x(cd_state(c), dx, precise != 0); cd_woke(c); }
static void cd_on_button(void *user, float x, float y, int phase, int button, int mods) { CoderHost *c = user; (void)mods; if (button == 1 && phase == 0 && !coder_page_overlay_open(cd_state(c))) { if (coder_page_file_middle_click(cd_state(c), x, y)) cd_woke(c); } }

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
               .frame = cd_on_frame, .pointer = cd_on_pointer, .button = cd_on_button, .wheel = cd_on_wheel, .scroll_gesture = cd_on_scroll, .scroll_x = cd_on_scroll_x,
               .key = cd_on_key, .wants_text = cd_on_wants_text, .ime = &cd_ime, .file = cd_on_file, .carrying = cd_on_carrying,
               .composition_shader = composition ? c->composition_shader : NULL, .composition = composition ? cd_composition : NULL, .image = composition ? cd_image : NULL, .presented = cd_presented};
#if !defined(__APPLE__) && !defined(_WIN32)
  app.text = cd_on_text; app.text_edit = cd_on_text_edit; app.text_cancel = cd_on_text_cancel;
  app.text_paste = cd_on_text_paste; app.text_rect = cd_on_text_rect;
#endif
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
  for (int i = 0; i < 80; i++) { free(a->svg[i]); a->svg[i] = NULL; }
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
int cd_has_selection(void *page) { CoderHost *c = page; return c && coder_page_has_selection(cd_state(c)); }
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
void cd_worktree_set(void *page, int on) { CoderHost *c = page; if (c) { coder_page_worktree_set(cd_state(c), on != 0); cd_changed(c); } }
int cd_worktree(void *page) { CoderHost *c = page; return c && coder_page_worktree(cd_state(c)) ? 1 : 0; }
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
void cd_agy(void *page, int on) { CoderHost *c = page; if (c) { coder_page_set_agy(cd_state(c), on != 0); cd_changed(c); } }
void cd_agy_models_clear(void *page) { CoderHost *c = page; if (c) coder_page_agy_models_clear(cd_state(c)); }
int cd_agy_models_add(void *page, const char *inst_id, const char *inst_name, const char *slug, const char *name, int efforts_mask, int default_effort, int is_default) {
  CoderHost *c = page;
  if (!c) return -1;
  return (int)coder_page_agy_models_add(cd_state(c), cd_string(inst_id), cd_string(inst_name), cd_string(slug), cd_string(name), efforts_mask, default_effort, is_default != 0);
}
int cd_model_inst_id(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap <= 1) return 0;
  int n = (int)coder_page_model_inst_id(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  if (n < 0) n = 0;
  if (n >= cap) n = cap - 1;
  out[n] = '\0';
  return n;
}
int cd_model_agy(void *page) { CoderHost *c = page; return c && coder_page_model_agy(cd_state(c)) ? 1 : 0; }
int cd_model_inkling(void *page) { CoderHost *c = page; return c && coder_page_model_inkling(cd_state(c)) ? 1 : 0; }
int cd_model_claude(void *page) { CoderHost *c = page; return c && coder_page_model_claude(cd_state(c)) ? 1 : 0; }
int cd_model_flawed(void *page) { CoderHost *c = page; return c && coder_page_model_flawed(cd_state(c)) ? 1 : 0; }
void cd_flawed_prefs_set(void *page, const int *vals, int count) {
  CoderHost *c = page;
  if (!c || !vals || count <= 0) return;
  int64_t tmp[32];
  int n = count < 32 ? count : 32;
  for (int i = 0; i < n; i++) tmp[i] = (int64_t)vals[i];
  coder_page_flawed_prefs_set(cd_state(c), (coax_v_s64){tmp, (int64_t)n});
  cd_woke(c);
}
int cd_flawed_prefs_get(void *page, int *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap <= 0) return 0;
  int64_t tmp[32];
  int want = cap < 32 ? cap : 32;
  int n = (int)coder_page_flawed_prefs_get(cd_state(c), (coax_v_s64){tmp, (int64_t)want});
  for (int i = 0; i < n && i < cap; i++) out[i] = (int)tmp[i];
  return n;
}
void cd_flawed_info(void *page, const char *auth_status, const char *key_label, const char *base_url, const char *custom_model, const char *tools_summary, const char *limits_summary) {
  CoderHost *c = page;
  if (!c) return;
  coder_page_flawed_info(cd_state(c), cd_string(auth_status), cd_string(key_label), cd_string(base_url), cd_string(custom_model), cd_string(tools_summary), cd_string(limits_summary));
  cd_changed(c);
}
void cd_orchestrator_set(void *page, int claude, int model) { CoderHost *c = page; if (!c) return; coder_page_orchestrator_set(cd_state(c), claude != 0, model); cd_woke(c); }
void cd_claude_effort_set(void *page, int model, int level) { CoderHost *c = page; if (!c) return; coder_page_claude_effort_set(cd_state(c), model, level); cd_woke(c); }
int cd_claude_effort(void *page, int model) { CoderHost *c = page; return c ? (int)coder_page_claude_effort(cd_state(c), model) : 2; }
int cd_orchestrator(void *page) { CoderHost *c = page; return c ? (int)coder_page_orchestrator(cd_state(c)) : 0; }
int cd_orchestrator_model(void *page) { CoderHost *c = page; return c ? (int)coder_page_orchestrator_model(cd_state(c)) : 1; }
int cd_model_hybrid(void *page) { CoderHost *c = page; return c && coder_page_model_hybrid(cd_state(c)) ? 1 : 0; }
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
void cd_line(void *page, const char *line) { CoderHost *c = page; if (!c) return; coder_page_sb_now_ms(cd_state(c), (int64_t)time(NULL) * 1000); coder_page_line(cd_state(c), cd_string(line)); cd_changed(c); }
void cd_failed(void *page, const char *why) { CoderHost *c = page; if (!c) return; coder_page_sb_now_ms(cd_state(c), (int64_t)time(NULL) * 1000); coder_page_failed(cd_state(c), cd_string(why)); cd_changed(c); }
void cd_turn_loading_model(void *page, int loading) { CoderHost *c = page; if (!c) return; coder_page_turn_loading_model(cd_state(c), loading != 0); cd_woke(c); }
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
  coder_page_sb_now_ms(cd_state(c), (int64_t)time(NULL) * 1000);
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
int cd_file_edit_most(void) { return (int)coder_page_file_edit_most(); }
int cd_file_line(void *page) { CoderHost *c = page; return c ? (int)coder_page_file_line(cd_state(c)) : 1; }
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
int cd_file_reload(void *page, const char *bytes, int len, long long size) {
  CoderHost *c = page;
  if (!c || len < 0 || len > cd_file_most()) return 0;
  char *keep = cd_keep(bytes, len);
  if (!keep) return 0;
  int ok = coder_page_file_reload(cd_state(c), (coax_v_u8){(uint8_t *)keep, len}, size);
  if (!ok) { free(keep); return 0; }
  free(c->file); c->file = keep;
  cd_woke(c);
  return 1;
}
void cd_file_disk_changed(void *page) { CoderHost *c = page; if (c) { coder_page_file_disk_changed(cd_state(c)); cd_woke(c); } }
void cd_file_jump(void *page, int start_line, int end_line) { CoderHost *c = page; if (c) { coder_page_file_jump(cd_state(c), start_line, end_line); cd_woke(c); } }
int cd_file_set_edit(void *page, int on) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_file_set_edit(cd_state(c), on != 0) ? 1 : 0; cd_woke(c); return ok; }
int cd_file_toggle_expand(void *page) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_file_toggle_expand(cd_state(c)) ? 1 : 0; cd_woke(c); return ok; }
int cd_file_toggle_hide(void *page) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_file_toggle_hide(cd_state(c)) ? 1 : 0; cd_woke(c); return ok; }
void cd_file_set_preview_next(void *page, int preview) { CoderHost *c = page; if (c) coder_page_file_set_preview_next(cd_state(c), preview != 0); }
void cd_file_find_open(void *page) { CoderHost *c = page; if (c) { coder_page_file_find_open(cd_state(c)); cd_woke(c); } }
void cd_file_goto_open(void *page) { CoderHost *c = page; if (c) { coder_page_file_goto_open(cd_state(c)); cd_woke(c); } }
int cd_file_crumb_dir(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_file_crumb_dir(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_file_dir_set(void *page, const char *lines) { CoderHost *c = page; if (!c) return 0; int n = (int)coder_page_file_dir_set(cd_state(c), cd_string(lines)); cd_woke(c); return n; }
int cd_file_tab_step(void *page, int dir) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_file_tab_step(cd_state(c), dir) ? 1 : 0; cd_woke(c); return ok; }
int cd_file_middle_click(void *page, float x, float y) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_file_middle_click(cd_state(c), x, y) ? 1 : 0; if (ok) cd_woke(c); return ok; }
int cd_sel_file_lines(void *page, int out_lines[2]) {
  CoderHost *c = page;
  if (!c || !out_lines) return 0;
  int64_t tmp[2] = {0, 0};
  if (!coder_page_sel_file_lines(cd_state(c), (coax_v_s64){tmp, 2})) return 0;
  out_lines[0] = (int)tmp[0]; out_lines[1] = (int)tmp[1];
  return 1;
}
void cd_scroll_x(void *page, float dx, int precise) { CoderHost *c = page; if (c) { coder_page_scroll_x(cd_state(c), dx, precise != 0); cd_woke(c); } }
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
int cd_file_path(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_file_path(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_file_expanded(void *page) { CoderHost *c = page; return c && coder_page_file_expanded(cd_state(c)); }
int cd_file_hidden(void *page) { CoderHost *c = page; return c && coder_page_file_hidden(cd_state(c)); }
int cd_file_open_scene(void *page, const char *path) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_file_open_scene(cd_state(c), cd_string(path)); if (ok) { free(c->file); c->file = NULL; cd_woke(c); } return ok; }
int cd_file_scene_rect(void *page, float out[4]) { CoderHost *c = page; return c && out && coder_page_file_scene_rect(cd_state(c), (coax_v_f32){out, 4}); }
int cd_filetree_set(void *page, const char *lines) { CoderHost *c = page; if (!c) return 0; int n = (int)coder_page_filetree_set(cd_state(c), cd_string(lines)); cd_woke(c); return n; }
void cd_filetree_open(void *page, int on) { CoderHost *c = page; if (c) { coder_page_filetree_open(cd_state(c), on != 0); cd_woke(c); } }
int cd_filetree_shown(void *page) { CoderHost *c = page; return c && coder_page_filetree_shown(cd_state(c)); }
int cd_filetree_req_path(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_filetree_req_path(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_filetree_close_tab(void *page) { CoderHost *c = page; return c ? (int)coder_page_filetree_close_tab(cd_state(c)) : -1; }
int cd_filetree_tab_drop(void *page, int idx) { CoderHost *c = page; if (!c) return -1; int next = (int)coder_page_filetree_tab_drop(cd_state(c), idx); cd_woke(c); return next; }
int cd_filetree_tab_path(void *page, int idx, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_filetree_tab_path(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
void cd_editors_set(void *page, int mask, const char *custom_cmd, const char *ssh_host) { CoderHost *c = page; if (c) { coder_page_editors_set(cd_state(c), mask, cd_string(custom_cmd), cd_string(ssh_host)); cd_woke(c); } }
int cd_editor_pick_id(void *page) { CoderHost *c = page; return c ? (int)coder_page_editor_pick_id(cd_state(c)) : -1; }
int cd_editor_cmd(void *page, int editor_id, const char *path, int line, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_editor_cmd(cd_state(c), editor_id, cd_string(path), line, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_live_scene(void *page, float out[8], char *path, int cap) { CoderHost *c = page; return c && out && cap > 1 ? cd_text_out((int)coder_page_live_scene(cd_state(c), (coax_v_f32){out, 8}, (coax_v_u8){(uint8_t *)path, cap - 1}), path, cap) : 0; }
int cd_over_rects(void *page, float *out, int rects) { CoderHost *c = page; return c && out && rects > 0 ? (int)coder_page_over_rects(cd_state(c), (coax_v_f32){out, (int64_t)rects * 4}) : 0; }
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
// Integrated multi-tab terminal pane (page/termpane.coax, GitHub #13869).
#define CD_TERM_SCROLL_BYTES (8 * 1024 * 1024)
static void cd_term_ensure_bufs(CoderHost *c) {
  if (!c) return;
  for (int i = 0; i < 8; i++) {
    if (!c->term_bufs[i]) {
      uint8_t *buf = malloc(CD_TERM_SCROLL_BYTES);
      if (buf) {
        c->term_bufs[i] = buf;
        coder_page_term_bind_buf(cd_state(c), i, (coax_v_u8){buf, CD_TERM_SCROLL_BYTES});
      }
    }
  }
}
void cd_term_toggle(void *page) { CoderHost *c = page; if (c) { cd_term_ensure_bufs(c); coder_page_term_toggle(cd_state(c)); cd_woke(c); } }
void cd_term_show(void *page, int open) { CoderHost *c = page; if (c) { if (open) cd_term_ensure_bufs(c); coder_page_term_show(cd_state(c), open != 0); cd_woke(c); } }
int cd_term_open(void *page) { CoderHost *c = page; return c && coder_page_term_open(cd_state(c)); }
int cd_term_shown(void *page) { CoderHost *c = page; return c && coder_page_term_shown(cd_state(c)); }
int cd_term_new_tab(void *page, const char *title, const char *cwd) {
  CoderHost *c = page;
  if (!c) return -1;
  cd_term_ensure_bufs(c);
  int id = (int)coder_page_term_new_tab(cd_state(c), cd_string(title), cd_string(cwd));
  cd_woke(c);
  return id;
}
int cd_term_feed(void *page, int tab_id, const char *bytes, int len) {
  CoderHost *c = page;
  if (!c || !bytes || len <= 0) return 0;
  int ok = coder_page_term_feed(cd_state(c), tab_id, (coax_v_u8){(uint8_t *)bytes, len});
  if (ok) cd_woke(c);
  return ok;
}
void cd_term_exited(void *page, int tab_id, int code) { CoderHost *c = page; if (c) { coder_page_term_exited(cd_state(c), tab_id, code); cd_woke(c); } }
int cd_term_op(void *page) { CoderHost *c = page; return c ? (int)coder_page_term_op(cd_state(c)) : 0; }
int cd_term_op_tab(void *page) { CoderHost *c = page; return c ? (int)coder_page_term_op_tab(cd_state(c)) : 0; }
int cd_term_active_id(void *page) { CoderHost *c = page; return c ? (int)coder_page_term_active_id(cd_state(c)) : 0; }
int cd_term_tab_cwd(void *page, int tab_id, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_term_tab_cwd(cd_state(c), tab_id, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_term_input_out(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_term_input_out(cd_state(c), (coax_v_u8){(uint8_t *)out, cap}) : 0; }
int cd_term_selected(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_term_selected(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_term_resize_due(void *page, int *out_cols, int *out_rows) {
  CoderHost *c = page;
  if (!c || !out_cols || !out_rows) return 0;
  int64_t dims[2] = {0, 0};
  int id = (int)coder_page_term_resize_due(cd_state(c), (coax_v_s64){dims, 2});
  if (id > 0) { *out_cols = (int)dims[0]; *out_rows = (int)dims[1]; }
  return id;
}
int cd_term_ports(void *page, int tab_id, const char *payload) {
  CoderHost *c = page;
  if (!c) return 0;
  int n = (int)coder_page_term_ports(cd_state(c), tab_id, cd_string(payload));
  cd_woke(c);
  return n;
}
#ifdef _WIN32
#include <windows.h>
typedef void *CdHpcon;
typedef HRESULT (WINAPI *CdCreateConFn)(COORD, HANDLE, HANDLE, DWORD, CdHpcon *);
typedef HRESULT (WINAPI *CdResizeConFn)(CdHpcon, COORD);
typedef void (WINAPI *CdCloseConFn)(CdHpcon);
#ifndef PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE
#define PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE 0x00020016
#endif
typedef struct {
  int used;
  CdHpcon hpcon;
  HANDLE in_w;
  HANDLE out_r;
  HANDLE proc;
  DWORD pid;
} CdWinPty;
static CdWinPty cd_win_ptys[16];
#endif

int cd_pty_spawn(const char *shell, const char *cwd, int cols, int rows, int *out_pid) {
#if defined(__APPLE__) || defined(__linux__)
  struct winsize ws;
  memset(&ws, 0, sizeof ws);
  ws.ws_col = (unsigned short)(cols > 0 ? cols : 80);
  ws.ws_row = (unsigned short)(rows > 0 ? rows : 24);
  int master = -1;
  pid_t pid = forkpty(&master, NULL, NULL, &ws);
  if (pid < 0) return -1;
  if (pid == 0) {
    if (cwd && cwd[0]) (void)chdir(cwd);
    setenv("TERM", "xterm-256color", 1);
    setenv("COLORTERM", "truecolor", 1);
    const char *sh = (shell && shell[0]) ? shell : getenv("SHELL");
    if (!sh || !sh[0]) sh = "/bin/sh";
    execl(sh, sh, "-l", (char *)NULL);
    execl("/bin/sh", "/bin/sh", (char *)NULL);
    _exit(127);
  }
  int flags = fcntl(master, F_GETFL, 0);
  if (flags >= 0) (void)fcntl(master, F_SETFL, flags | O_NONBLOCK);
  if (out_pid) *out_pid = (int)pid;
  return master;
#elif defined(_WIN32)
  int slot = -1;
  for (int i = 0; i < 16; i++) { if (!cd_win_ptys[i].used) { slot = i; break; } }
  if (slot < 0) { if (out_pid) *out_pid = -1; return -1; }
  SECURITY_ATTRIBUTES sa;
  memset(&sa, 0, sizeof sa);
  sa.nLength = sizeof sa;
  sa.bInheritHandle = TRUE;
  HANDLE in_r = NULL, in_w = NULL, out_r = NULL, out_w = NULL;
  if (!CreatePipe(&in_r, &in_w, &sa, 0) || !CreatePipe(&out_r, &out_w, &sa, 0)) {
    if (in_r) CloseHandle(in_r); if (in_w) CloseHandle(in_w);
    if (out_r) CloseHandle(out_r); if (out_w) CloseHandle(out_w);
    if (out_pid) *out_pid = -1;
    return -1;
  }
  SetHandleInformation(in_w, HANDLE_FLAG_INHERIT, 0);
  SetHandleInformation(out_r, HANDLE_FLAG_INHERIT, 0);

  char cmd[512];
  const char *sh = (shell && shell[0]) ? shell : "cmd.exe";
  snprintf(cmd, sizeof cmd, "%s", sh);
  PROCESS_INFORMATION pi;
  memset(&pi, 0, sizeof pi);
  CdHpcon hpcon = NULL;
  BOOL spawned = FALSE;

  HMODULE k32 = GetModuleHandleA("kernel32.dll");
  HMODULE nt = GetModuleHandleA("ntdll.dll");
  int under_wine = nt && GetProcAddress(nt, "wine_get_version") != NULL;
  CdCreateConFn create_con = (!under_wine && k32) ? (CdCreateConFn)(void *)GetProcAddress(k32, "CreatePseudoConsole") : NULL;
  CdCloseConFn close_con = k32 ? (CdCloseConFn)(void *)GetProcAddress(k32, "ClosePseudoConsole") : NULL;
  if (create_con) {
    COORD sz;
    sz.X = (SHORT)(cols > 0 ? cols : 80);
    sz.Y = (SHORT)(rows > 0 ? rows : 24);
    if (SUCCEEDED(create_con(sz, in_r, out_w, 0, &hpcon)) && hpcon) {
      SIZE_T attr_sz = 0;
      InitializeProcThreadAttributeList(NULL, 1, 0, &attr_sz);
      LPPROC_THREAD_ATTRIBUTE_LIST alist = (LPPROC_THREAD_ATTRIBUTE_LIST)malloc(attr_sz);
      if (alist && InitializeProcThreadAttributeList(alist, 1, 0, &attr_sz) &&
          UpdateProcThreadAttribute(alist, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, hpcon, sizeof(hpcon), NULL, NULL)) {
        STARTUPINFOEXA siex;
        memset(&siex, 0, sizeof siex);
        siex.StartupInfo.cb = sizeof siex;
        siex.lpAttributeList = alist;
        spawned = CreateProcessA(NULL, cmd, NULL, NULL, FALSE,
            EXTENDED_STARTUPINFO_PRESENT, NULL,
            (cwd && cwd[0]) ? cwd : NULL, &siex.StartupInfo, &pi);
        DeleteProcThreadAttributeList(alist);
      }
      free(alist);
      if (!spawned && close_con) { close_con(hpcon); hpcon = NULL; }
    }
  }
  if (!spawned) {
    // Fallback to standard piped shell (e.g. under Wine or older Windows).
    STARTUPINFOA si;
    memset(&si, 0, sizeof si);
    si.cb = sizeof si;
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdInput = in_r;
    si.hStdOutput = out_w;
    si.hStdError = out_w;
    spawned = CreateProcessA(NULL, cmd, NULL, NULL, TRUE, CREATE_NO_WINDOW, NULL,
        (cwd && cwd[0]) ? cwd : NULL, &si, &pi);
  }
  CloseHandle(in_r);
  CloseHandle(out_w);
  if (!spawned) {
    CloseHandle(in_w);
    CloseHandle(out_r);
    if (out_pid) *out_pid = -1;
    return -1;
  }
  CloseHandle(pi.hThread);
  cd_win_ptys[slot].used = 1;
  cd_win_ptys[slot].hpcon = hpcon;
  cd_win_ptys[slot].in_w = in_w;
  cd_win_ptys[slot].out_r = out_r;
  cd_win_ptys[slot].proc = pi.hProcess;
  cd_win_ptys[slot].pid = pi.dwProcessId;
  if (out_pid) *out_pid = (int)pi.dwProcessId;
  return slot;
#else
  (void)shell; (void)cwd; (void)cols; (void)rows;
  if (out_pid) *out_pid = -1;
  return -1;
#endif
}
int cd_pty_read(int handle, char *out, int cap) {
#if defined(__APPLE__) || defined(__linux__)
  if (handle < 0 || !out || cap <= 0) return -1;
  ssize_t n = read(handle, out, (size_t)cap);
  if (n > 0) return (int)n;
  if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK || errno == EINTR)) return 0;
  return -1;
#elif defined(_WIN32)
  if (handle < 0 || handle >= 16 || !cd_win_ptys[handle].used || !out || cap <= 0) return -1;
  DWORD avail = 0;
  if (!PeekNamedPipe(cd_win_ptys[handle].out_r, NULL, 0, NULL, &avail, NULL)) return -1;
  if (avail == 0) {
    DWORD code = 0;
    if (GetExitCodeProcess(cd_win_ptys[handle].proc, &code) && code != STILL_ACTIVE) return -1;
    return 0;
  }
  DWORD got = 0;
  DWORD want = avail < (DWORD)cap ? avail : (DWORD)cap;
  if (!ReadFile(cd_win_ptys[handle].out_r, out, want, &got, NULL)) return -1;
  return (int)got;
#else
  (void)handle; (void)out; (void)cap;
  return -1;
#endif
}
int cd_pty_write(int handle, const char *data, int len) {
#if defined(__APPLE__) || defined(__linux__)
  if (handle < 0 || !data || len <= 0) return 0;
  ssize_t n = write(handle, data, (size_t)len);
  return n > 0 ? (int)n : 0;
#elif defined(_WIN32)
  if (handle < 0 || handle >= 16 || !cd_win_ptys[handle].used || !data || len <= 0) return 0;
  DWORD written = 0;
  if (!WriteFile(cd_win_ptys[handle].in_w, data, (DWORD)len, &written, NULL)) return 0;
  return (int)written;
#else
  (void)handle; (void)data; (void)len;
  return 0;
#endif
}
int cd_pty_resize(int master_fd, int cols, int rows) {
#if defined(__APPLE__) || defined(__linux__)
  if (master_fd < 0) return 0;
  struct winsize ws;
  memset(&ws, 0, sizeof ws);
  ws.ws_col = (unsigned short)(cols > 0 ? cols : 80);
  ws.ws_row = (unsigned short)(rows > 0 ? rows : 24);
  return ioctl(master_fd, TIOCSWINSZ, &ws) == 0 ? 1 : 0;
#elif defined(_WIN32)
  if (master_fd < 0 || master_fd >= 16 || !cd_win_ptys[master_fd].used) return 0;
  if (!cd_win_ptys[master_fd].hpcon) return 1;
  HMODULE k32 = GetModuleHandleA("kernel32.dll");
  CdResizeConFn resize_con = k32 ? (CdResizeConFn)(void *)GetProcAddress(k32, "ResizePseudoConsole") : NULL;
  if (!resize_con) return 0;
  COORD sz;
  sz.X = (SHORT)(cols > 0 ? cols : 80);
  sz.Y = (SHORT)(rows > 0 ? rows : 24);
  return SUCCEEDED(resize_con(cd_win_ptys[master_fd].hpcon, sz)) ? 1 : 0;
#else
  (void)master_fd; (void)cols; (void)rows;
  return 0;
#endif
}
void cd_pty_kill(int pid) {
#if defined(__APPLE__) || defined(__linux__)
  if (pid > 0) { kill(-pid, SIGHUP); kill(pid, SIGHUP); }
#elif defined(_WIN32)
  for (int i = 0; i < 16; i++) {
    if (cd_win_ptys[i].used && (int)cd_win_ptys[i].pid == pid && cd_win_ptys[i].proc) {
      TerminateProcess(cd_win_ptys[i].proc, 1);
    }
  }
#else
  (void)pid;
#endif
}
void cd_pty_close(int handle, int pid) {
#if defined(__APPLE__) || defined(__linux__)
  if (handle >= 0) close(handle);
  cd_pty_kill(pid);
#elif defined(_WIN32)
  (void)pid;
  if (handle >= 0 && handle < 16 && cd_win_ptys[handle].used) {
    if (cd_win_ptys[handle].hpcon) {
      HMODULE k32 = GetModuleHandleA("kernel32.dll");
      CdCloseConFn close_con = k32 ? (CdCloseConFn)(void *)GetProcAddress(k32, "ClosePseudoConsole") : NULL;
      if (close_con) close_con(cd_win_ptys[handle].hpcon);
    }
    if (cd_win_ptys[handle].in_w) CloseHandle(cd_win_ptys[handle].in_w);
    if (cd_win_ptys[handle].out_r) CloseHandle(cd_win_ptys[handle].out_r);
    if (cd_win_ptys[handle].proc) {
      TerminateProcess(cd_win_ptys[handle].proc, 0);
      CloseHandle(cd_win_ptys[handle].proc);
    }
    memset(&cd_win_ptys[handle], 0, sizeof(cd_win_ptys[handle]));
  }
#else
  (void)handle; (void)pid;
#endif
}
void cd_tick(void *page, float dt) { CoderHost *c = page; if (c) coder_page_tick(cd_state(c), dt); }
void cd_close(void *page) {
  CoderHost *c = page;
  if (!c) return;
  if (!c->headless) gallery_free(&c->app);
  for (int i = 0; i < CD_IMAGES; i++) free(c->images[i].rgba);
  for (int i = 0; i < 8; i++) free(c->term_bufs[i]);
  free(c->files); free(c->file); free(c->gitdiff); free(c->ckpt_diff); free(c->s); free(c->st); free(c);
}
// Settings (settings.coax): open it on a page, scrolled to a row (page < -1 closes it); the page
// it shows (-1 closed); what its pages show; the tier a press chose; the search's text.
void cd_settings_open(void *page, int which, const char *row) { CoderHost *c = page; if (c) { coder_page_settings_open(cd_state(c), which, cd_string(row)); cd_woke(c); } }
int cd_settings_page(void *page) { CoderHost *c = page; return c ? (int)coder_page_settings_page(cd_state(c)) : -1; }
void cd_settings_info(void *page, const char *claude, const char *openrouter, const char *version) { CoderHost *c = page; if (c) { coder_page_settings_info(cd_state(c), cd_string(claude), cd_string(openrouter), cd_string(version)); cd_changed(c); } }
void cd_settings_gemini(void *page, const char *gemini) { CoderHost *c = page; if (c) { coder_page_settings_gemini(cd_state(c), cd_string(gemini)); cd_changed(c); } }
void cd_settings_local(void *page, const char *status) { CoderHost *c = page; if (c) { coder_page_settings_local(cd_state(c), cd_string(status)); cd_changed(c); } }
void cd_settings_executor(void *page, const char *status, const char *lineage, const char *badge) { CoderHost *c = page; if (c) { coder_page_settings_executor(cd_state(c), cd_string(status), cd_string(lineage), cd_string(badge)); cd_changed(c); } }
void cd_settings_pylon(void *page, const char *state, const char *contrib, const char *sats, const char *release) { CoderHost *c = page; if (c) { coder_page_settings_pylon(cd_state(c), cd_string(state), cd_string(contrib), cd_string(sats), cd_string(release)); cd_changed(c); } }
void cd_settings_wallet(void *page, const char *status, const char *balance, const char *addr, const char *npub, const char *limit, const char *backup, const char *history, int ready) { CoderHost *c = page; if (c) { coder_page_settings_wallet(cd_state(c), cd_string(status), cd_string(balance), cd_string(addr), cd_string(npub), cd_string(limit), cd_string(backup), cd_string(history), ready != 0); cd_changed(c); } }
void cd_inkling(void *page, int on) { CoderHost *c = page; if (c) { coder_page_set_inkling(cd_state(c), on != 0); cd_changed(c); } }
void cd_inkling_info(void *page, const char *id, const char *label, const char *desc) { CoderHost *c = page; if (c) { coder_page_set_inkling_info(cd_state(c), cd_string(id), cd_string(label), cd_string(desc)); cd_changed(c); } }
void cd_show_other_models_set(void *page, int on) { CoderHost *c = page; if (c) { coder_page_show_other_models_set(cd_state(c), on != 0); cd_changed(c); } }
int cd_show_other_models(void *page) { CoderHost *c = page; return c && coder_page_show_other_models(cd_state(c)); }
void cd_hybrid_exec(void *page, int on) { CoderHost *c = page; if (c) { coder_page_set_hybrid_exec(cd_state(c), on != 0); cd_changed(c); } }
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
void cd_dialog_body(void *page, const char *body) { CoderHost *c = page; if (c) { coder_page_dialog_body(cd_state(c), cd_string(body)); cd_woke(c); } }
int cd_dialog_field(void *page, const char *label, const char *value, const char *hint, int secure, int lines) { CoderHost *c = page; return c && coder_page_dialog_field(cd_state(c), cd_string(label), cd_string(value), cd_string(hint), secure != 0, lines); }
int cd_dialog_button(void *page, const char *label, int kind, int aside) { CoderHost *c = page; return c && coder_page_dialog_button(cd_state(c), cd_string(label), kind, aside != 0); }
void cd_dialog_show(void *page, int confirm, int cancel, int focus) { CoderHost *c = page; if (c) { coder_page_dialog_show(cd_state(c), confirm, cancel, focus); cd_woke(c); } }
void cd_dialog_select_all(void *page) { CoderHost *c = page; if (c) { coder_page_dialog_select_all(cd_state(c)); cd_woke(c); } }
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
void cd_flags_set(void *page, int64_t mask) { CoderHost *c = page; if (!c) return; coder_page_flags_set(cd_state(c), mask); cd_woke(c); }
int64_t cd_flags(void *page) { CoderHost *c = page; return c ? coder_page_flags(cd_state(c)) : 0; }
void cd_local_state(void *page, int os_ok, int hw_ok, int ram_gb, int dl_state, int pct, int dl_mb, int total_mb, const char *err) { CoderHost *c = page; if (!c) return; coder_page_local_state(cd_state(c), os_ok != 0, hw_ok != 0, ram_gb, dl_state, pct, dl_mb, total_mb, cd_string(err)); cd_woke(c); }
void cd_agy_connect(void *page, int avail, int state, const char *msg, int pct, int dl_mb, int total_mb) { CoderHost *c = page; if (!c) return; coder_page_agy_connect(cd_state(c), avail != 0, state, cd_string(msg), pct, dl_mb, total_mb); cd_woke(c); }
void cd_cloud_authed_set(void *page, int on) { CoderHost *c = page; if (!c) return; coder_page_set_cloud_authed(cd_state(c), on != 0); cd_woke(c); }
int cd_gym_rect(void *page, float out[4]) { CoderHost *c = page; return c && out && coder_page_gym_rect(cd_state(c), (coax_v_f32){out, 4}); }
void cd_git_status(void *page, int in_repo, int in_worktree, int dirty, int ahead, int behind) { CoderHost *c = page; if (!c) return; coder_page_git_status(cd_state(c), in_repo != 0, in_worktree != 0, dirty, ahead, behind); cd_woke(c); }
void cd_git_mode_set(void *page, int mode) { CoderHost *c = page; if (!c) return; coder_page_git_mode_set(cd_state(c), mode); cd_woke(c); }
int cd_git_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_git_mode(cd_state(c)) : 1; }
int cd_git_branches(void *page, const char *lines) { CoderHost *c = page; if (!c) return 0; int n = (int)coder_page_git_branches(cd_state(c), cd_string(lines)); cd_changed(c); return n; }
void cd_git_busy(void *page, int busy, const char *label) { CoderHost *c = page; if (!c) return; coder_page_git_busy(cd_state(c), busy != 0, cd_string(label)); cd_woke(c); }
void cd_git_output_clear(void *page) { CoderHost *c = page; if (!c) return; coder_page_git_output_clear(cd_state(c)); cd_woke(c); }
void cd_git_output_append(void *page, const char *chunk) { CoderHost *c = page; if (!c) return; coder_page_git_output_append(cd_state(c), cd_string(chunk)); cd_woke(c); }
void cd_git_output_done(void *page, int failed) { CoderHost *c = page; if (!c) return; coder_page_git_output_done(cd_state(c), failed != 0); cd_woke(c); }
void cd_git_wt_status(void *page, int status, const char *note) { CoderHost *c = page; if (!c) return; coder_page_git_wt_status(cd_state(c), status, cd_string(note)); cd_woke(c); }
int cd_git_op(void *page) { CoderHost *c = page; return c ? (int)coder_page_git_op(cd_state(c)) : 0; }
int cd_git_anchor(void *page, float out[2]) { CoderHost *c = page; return c && out && coder_page_git_anchor(cd_state(c), (coax_v_f32){out, 2}); }
int cd_git_needs_guard(const char *branch, int op) { return coder_page_git_needs_guard(cd_string(branch), op) ? 1 : 0; }
int cd_git_valid_branch(const char *name) { return coder_page_git_valid_branch(cd_string(name)) ? 1 : 0; }
int cd_git_valid_commit(const char *msg) { return coder_page_git_valid_commit(cd_string(msg)) ? 1 : 0; }
// Live Git diff viewer and inline review comments (page/gitdiff.coax, GitHub #13870).
int cd_git_diff_max(void) { return (int)coder_page_git_diff_max(); }
void cd_git_diff_set(void *page, const char *bytes, int len) {
  CoderHost *c = page;
  if (!c) return;
  if (!bytes || len <= 0) {
    coder_page_git_diff_set(cd_state(c), (coax_v_u8){NULL, 0});
    free(c->gitdiff); c->gitdiff = NULL;
    cd_woke(c);
    return;
  }
  int cap = cd_git_diff_max();
  if (len > cap) len = cap;
  char *keep = malloc((size_t)len + 1);
  if (!keep) return;
  memcpy(keep, bytes, (size_t)len); keep[len] = 0;
  coder_page_git_diff_set(cd_state(c), (coax_v_u8){(uint8_t *)keep, len});
  free(c->gitdiff); c->gitdiff = keep;
  cd_woke(c);
}
int cd_git_diff_files(void *page) { CoderHost *c = page; return c ? (int)coder_page_git_diff_files(cd_state(c)) : 0; }
int cd_review_pick(void *page, char *path, int path_cap, char *snippet, int snip_cap, char *body, int body_cap, int meta[4]) {
  CoderHost *c = page;
  if (!c || !path || path_cap < 2 || !snippet || snip_cap < 2 || !body || body_cap < 2 || !meta) return 0;
  int64_t m64[4] = {0, 0, 0, 0};
  memset(path, 0, (size_t)path_cap);
  memset(snippet, 0, (size_t)snip_cap);
  memset(body, 0, (size_t)body_cap);
  int pn = (int)coder_page_review_pick(cd_state(c), (coax_v_u8){(uint8_t *)path, path_cap - 1},
                                       (coax_v_u8){(uint8_t *)snippet, snip_cap - 1},
                                       (coax_v_u8){(uint8_t *)body, body_cap - 1},
                                       (coax_v_s64){m64, 4});
  for (int i = 0; i < 4; i++) meta[i] = (int)m64[i];
  return pn;
}
int cd_review_comment_add(void *page, const char *path, int start_line, int end_line, int side, const char *snippet, const char *body) {
  CoderHost *c = page;
  if (!c) return 0;
  int id = (int)coder_page_review_comment_add(cd_state(c), cd_string(path), start_line, end_line, side, cd_string(snippet), cd_string(body));
  cd_woke(c);
  return id;
}
int cd_review_comment_edit(void *page, int id, const char *body) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_review_comment_edit(cd_state(c), id, cd_string(body));
  cd_woke(c);
  return ok;
}
int cd_review_comment_delete(void *page, int id) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_review_comment_delete(cd_state(c), id);
  cd_woke(c);
  return ok;
}
int cd_review_comments_submit(void *page) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_review_comments_submit(cd_state(c));
  cd_woke(c);
  return ok;
}
int cd_review_comments_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_review_comments_count(cd_state(c)) : 0; }
void cd_ckpt_set_base(void *page, int present) { CoderHost *c = page; if (!c) return; coder_page_ckpt_set_base(cd_state(c), present != 0); cd_changed(c); }
void cd_ckpt_mark(void *page, int turn, int present) { CoderHost *c = page; if (!c) return; coder_page_ckpt_mark(cd_state(c), turn, present != 0); cd_woke(c); }
int cd_ckpt_sync(void *page, const char *chat, const char *refs_list) { CoderHost *c = page; if (!c) return 0; int n = (int)coder_page_ckpt_sync(cd_state(c), cd_string(chat), cd_string(refs_list)); cd_woke(c); return n; }
int cd_ckpt_has(void *page, int turn) { CoderHost *c = page; return c && coder_page_ckpt_has(cd_state(c), turn) ? 1 : 0; }
int cd_ckpt_diff_set(void *page, int turn, const char *bytes, int len, const char *label) {
  CoderHost *c = page;
  if (!c) return 0;
  if (len < 0) len = 0;
  char *keep = malloc((size_t)len + 1);
  if (!keep) return 0;
  if (len > 0 && bytes) memcpy(keep, bytes, (size_t)len);
  keep[len] = 0;
  if (!coder_page_ckpt_diff_set(cd_state(c), turn, (coax_v_u8){(uint8_t *)keep, len}, cd_string(label))) { free(keep); return 0; }
  free(c->ckpt_diff); c->ckpt_diff = keep;
  cd_woke(c);
  return 1;
}
void cd_ckpt_diff_clear(void *page) { CoderHost *c = page; if (!c) return; coder_page_ckpt_diff_clear(cd_state(c)); free(c->ckpt_diff); c->ckpt_diff = NULL; cd_woke(c); }
int cd_ckpt_rewind_to(void *page, int turn) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_ckpt_rewind_to(cd_state(c), turn) ? 1 : 0; if (ok) cd_woke(c); return ok; }
int cd_ckpt_op(void *page) { CoderHost *c = page; return c ? (int)coder_page_ckpt_op(cd_state(c)) : 0; }
int cd_ckpt_target(void *page) { CoderHost *c = page; return c ? (int)coder_page_ckpt_target(cd_state(c)) : -1; }
int cd_ckpt_nturns(void *page) { CoderHost *c = page; return c ? (int)coder_page_ckpt_nturns(cd_state(c)) : 0; }
int cd_ckpt_ref(const char *chat, int turn, char *out, int cap) {
  if (!out || cap < 1) return 0;
  int n = (int)coder_page_ckpt_ref(cd_string(chat), turn, (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n;
}
int cd_ckpt_base_ref(const char *chat, char *out, int cap) {
  if (!out || cap < 1) return 0;
  int n = (int)coder_page_ckpt_base_ref(cd_string(chat), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n;
}
int cd_ckpt_prefix(const char *chat, char *out, int cap) {
  if (!out || cap < 1) return 0;
  int n = (int)coder_page_ckpt_prefix(cd_string(chat), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n;
}
int cd_ckpt_diff_from(void *page, const char *chat, int turn, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 1) return 0;
  int n = (int)coder_page_ckpt_diff_from(cd_state(c), cd_string(chat), turn, (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n;
}
int cd_ckpt_needs_confirm(int dirty_files, int turns_dropped) { return coder_page_ckpt_needs_confirm(dirty_files, turns_dropped) ? 1 : 0; }
int cd_ckpt_restore_safety(void *page, int turn, int shared_active_chats, int can_rollback) {
  CoderHost *c = page;
  if (!c) return 2;
  return (int)coder_page_ckpt_restore_safety(cd_state(c), turn, shared_active_chats, can_rollback != 0);
}
int cd_ckpt_same_checkout(const char *dir_a, const char *dir_b) {
  return coder_page_ckpt_same_checkout(cd_string(dir_a), cd_string(dir_b)) ? 1 : 0;
}
// Interactive permission, sub-agent approval, Plan Mode, and Secret request cards (page/permit.coax, GitHub #13873).
int cd_permit_open(void *page) { CoderHost *c = page; return c ? (int)coder_page_permit_open(cd_state(c)) : 0; }
int cd_permit_kind(void *page) { CoderHost *c = page; return c ? (int)coder_page_permit_kind(cd_state(c)) : -1; }
int cd_permit_pending(void *page) { CoderHost *c = page; return c ? (int)coder_page_permit_pending(cd_state(c)) : 0; }
void cd_permit_approval_mode_set(void *page, int on) { CoderHost *c = page; if (c) { coder_page_permit_approval_mode_set(cd_state(c), on != 0); cd_woke(c); } }
int cd_permit_approval_mode(void *page) { CoderHost *c = page; return c && coder_page_permit_approval_mode(cd_state(c)) ? 1 : 0; }
int cd_permit_secret_name(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_permit_secret_name(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
int cd_permit_title(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_permit_title(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  if (n < 0) n = 0; if (n >= cap) n = cap - 1;
  out[n] = 0;
  return n;
}
int cd_permit_body(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_permit_body(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
int cd_permit_secret_done(void *page, int saved) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_permit_secret_done(cd_state(c), saved != 0) ? 1 : 0;
  cd_woke(c);
  return ok;
}
// Turn-completion & needs-input OS notifications, audio chimes, and Dock/taskbar badges (page/notify.coax, GitHub #13874).
void cd_notify_mode_set(void *page, int mode) { CoderHost *c = page; if (!c) return; coder_page_notify_mode_set(cd_state(c), mode); cd_woke(c); }
int cd_notify_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_notify_mode(cd_state(c)) : 3; }
int cd_notify_decide(int mode, int event, int app_focused, int is_active_chat) {
  return (int)coder_page_notify_decide(mode, event, app_focused != 0, is_active_chat != 0);
}
int cd_notify_goal_event(int old_status, int new_status) { return (int)coder_page_notify_goal_event(old_status, new_status); }
int cd_notify_title(int event, const char *chat_title, char *out, int cap) {
  if (!out || cap < 2) return 0;
  int n = (int)coder_page_notify_title(event, cd_string(chat_title), (coax_v_u8){(uint8_t *)out, cap - 1});
  if (n < 0) n = 0; if (n >= cap) n = cap - 1;
  out[n] = 0;
  return n;
}
int cd_notify_body(int event, const char *detail, char *out, int cap) {
  if (!out || cap < 2) return 0;
  int n = (int)coder_page_notify_body(event, cd_string(detail), (coax_v_u8){(uint8_t *)out, cap - 1});
  if (n < 0) n = 0; if (n >= cap) n = cap - 1;
  out[n] = 0;
  return n;
}
int cd_notify_badge_label(int count, char *out, int cap) {
  if (!out || cap < 2) return 0;
  int n = (int)coder_page_notify_badge_label(count, (coax_v_u8){(uint8_t *)out, cap - 1});
  if (n < 0) n = 0; if (n >= cap) n = cap - 1;
  out[n] = 0;
  return n;
}
void cd_sb_now_ms(void *page, int64_t now_ms) { CoderHost *c = page; if (!c) return; coder_page_sb_now_ms(cd_state(c), now_ms); }
int cd_sb_op(void *page) { CoderHost *c = page; return c ? (int)coder_page_sb_op(cd_state(c)) : 0; }
int cd_sb_selected_ids(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 1) return 0;
  int n = (int)coder_page_sb_selected_ids(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n;
}
void cd_sb_undo_show(void *page, const char *text, int kind, float duration) { CoderHost *c = page; if (!c) return; coder_page_sb_undo_show(cd_state(c), cd_string(text), kind, duration); cd_woke(c); }
void cd_sb_undo_dismiss(void *page) { CoderHost *c = page; if (!c) return; coder_page_sb_undo_dismiss(cd_state(c)); cd_woke(c); }
void cd_sb_set_view(void *page, int mode) { CoderHost *c = page; if (!c) return; coder_page_sb_set_view(cd_state(c), mode); cd_woke(c); }
int cd_sb_view(void *page) { CoderHost *c = page; return c ? (int)coder_page_sb_view(cd_state(c)) : 0; }
int cd_sb_sel_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_sb_sel_count(cd_state(c)) : 0; }
// In-thread Find (⌘F), Transcript Minimap, and 3-mode Command Palette (page/find.coax, GitHub #13876).
void cd_find_open(void *page) { CoderHost *c = page; if (c) { coder_page_find_open(cd_state(c)); cd_woke(c); } }
void cd_find_close(void *page) { CoderHost *c = page; if (c) { coder_page_find_close(cd_state(c)); cd_woke(c); } }
int cd_find_is_open(void *page) { CoderHost *c = page; return c && coder_page_find_is_open(cd_state(c)) ? 1 : 0; }
void cd_find_set_query(void *page, const char *text) { CoderHost *c = page; if (c) { coder_page_find_set_query(cd_state(c), cd_string(text)); cd_changed(c); } }
int cd_find_step(void *page, int dir) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_find_step(cd_state(c), dir) ? 1 : 0; cd_changed(c); return ok; }
int cd_find_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_find_count(cd_state(c)) : 0; }
int cd_find_current(void *page) { CoderHost *c = page; return c ? (int)coder_page_find_current(cd_state(c)) : -1; }
int cd_scroll_to_turn(void *page, int turn) { CoderHost *c = page; if (!c) return 0; int ok = coder_page_scroll_to_turn(cd_state(c), turn) ? 1 : 0; cd_changed(c); return ok; }
void cd_minimap_toggle(void *page) { CoderHost *c = page; if (c) { coder_page_minimap_toggle(cd_state(c)); cd_changed(c); } }
int cd_minimap_on(void *page) { CoderHost *c = page; return c && coder_page_minimap_on(cd_state(c)) ? 1 : 0; }
int cd_palette_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_palette_mode(cd_state(c)) : 0; }
void cd_palette_set_mode(void *page, int mode) { CoderHost *c = page; if (c) { coder_page_palette_set_mode(cd_state(c), mode); cd_changed(c); } }
int cd_palette_query(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_palette_query_out(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
void cd_palette_content_hits(void *page, const char *payload) {
  CoderHost *c = page;
  if (c) { coder_page_palette_content_hits(cd_state(c), cd_string(payload)); cd_changed(c); }
}
int cd_palette_jump_chat(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_palette_jump_chat(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
int cd_palette_jump_turn(void *page) { CoderHost *c = page; return c ? (int)coder_page_palette_jump_turn(cd_state(c)) : -1; }
// Usage & quota limits dashboard, composer popover, multi-account pooling & limit recovery (page/usage.coax, GitHub #13877).
void cd_usage_clear(void *page) { CoderHost *c = page; if (!c) return; coder_page_usage_clear(cd_state(c)); cd_woke(c); }
int cd_usage_set_account(void *page, int provider, const char *profile, const char *plan, int status, const char *why, int64_t got_ms, int limited, int64_t limit_reset_s) {
  CoderHost *c = page;
  if (!c) return -1;
  int idx = (int)coder_page_usage_set_account(cd_state(c), provider, cd_string(profile), cd_string(plan), status, cd_string(why), got_ms, limited != 0, limit_reset_s);
  cd_woke(c);
  return idx;
}
int cd_usage_add_window(void *page, int acct_idx, const char *label, int used_bp, int64_t reset_s) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_usage_add_window(cd_state(c), acct_idx, cd_string(label), used_bp, reset_s) ? 1 : 0;
  cd_woke(c);
  return ok;
}
void cd_usage_spend_reset(void *page) { CoderHost *c = page; if (!c) return; coder_page_usage_spend_reset(cd_state(c)); cd_woke(c); }
int cd_usage_spend_add(void *page, const char *path, int provider, const char *model, int64_t in_tok, int64_t cached_tok, int64_t out_tok, int64_t fallback_micros) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_usage_spend_add(cd_state(c), cd_string(path), provider, cd_string(model), in_tok, cached_tok, out_tok, fallback_micros) ? 1 : 0;
  cd_woke(c);
  return ok;
}
void cd_usage_price_clear(void *page) { CoderHost *c = page; if (!c) return; coder_page_usage_price_clear(cd_state(c)); cd_woke(c); }
int cd_usage_price_set(void *page, const char *model, int64_t in_micros_per_m, int64_t out_micros_per_m, int64_t cached_micros_per_m) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_usage_price_set(cd_state(c), cd_string(model), in_micros_per_m, out_micros_per_m, cached_micros_per_m) ? 1 : 0;
  cd_woke(c);
  return ok;
}
void cd_usage_pool_set(void *page, int enabled) { CoderHost *c = page; if (!c) return; coder_page_usage_pool_set(cd_state(c), enabled != 0); cd_woke(c); }
int cd_usage_pool_enabled(void *page) { CoderHost *c = page; return c && coder_page_usage_pool_enabled(cd_state(c)) ? 1 : 0; }
int cd_usage_pool_select(void *page, int provider) { CoderHost *c = page; return c ? (int)coder_page_usage_pool_select(cd_state(c), provider) : -1; }
void cd_usage_recovery_set(void *page, int enabled) { CoderHost *c = page; if (!c) return; coder_page_usage_recovery_set(cd_state(c), enabled != 0); cd_woke(c); }
int cd_usage_recovery_enabled(void *page) { CoderHost *c = page; return c && coder_page_usage_recovery_enabled(cd_state(c)) ? 1 : 0; }
int cd_usage_tick(void *page, int64_t now_s) {
  CoderHost *c = page;
  if (!c) return 0;
  int rec = coder_page_usage_tick(cd_state(c), now_s) ? 1 : 0;
  if (rec) cd_woke(c);
  return rec;
}
int cd_usage_recovery_msg(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_usage_recovery_msg(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  if (n < 0) n = 0; if (n >= cap) n = cap - 1;
  out[n] = 0;
  return n;
}
void cd_usage_refreshing_set(void *page, int on) { CoderHost *c = page; if (!c) return; coder_page_usage_refreshing_set(cd_state(c), on != 0); cd_woke(c); }
int cd_usage_is_index_db(const char *path) { return coder_page_usage_is_index_db(cd_string(path)) ? 1 : 0; }
int64_t cd_usage_total_cost_micros(void *page) { CoderHost *c = page; return c ? (int64_t)coder_page_usage_total_cost_micros(cd_state(c)) : 0; }
int64_t cd_usage_total_tokens(void *page) { CoderHost *c = page; return c ? (int64_t)coder_page_usage_total_tokens(cd_state(c)) : 0; }
// MCP server management, OAuth PKCE card, and Skills catalog (page/mcpskills.coax, GitHub #13879).
int cd_mcp_load_json(void *page, const char *json) {
  CoderHost *c = page;
  if (!c) return 0;
  int n = (int)coder_page_mcp_load_json(cd_state(c), cd_string(json));
  cd_woke(c);
  return n;
}
int cd_mcp_save_json(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_mcp_save_json(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n;
}
int cd_mcp_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_mcp_count(cd_state(c)) : 0; }
int cd_mcp_target(void *page) { CoderHost *c = page; return c ? (int)coder_page_mcp_target(cd_state(c)) : -1; }
int cd_mcp_add(void *page, const char *name, int transport, const char *endpoint, int enabled) {
  CoderHost *c = page;
  if (!c) return -1;
  int idx = (int)coder_page_mcp_add(cd_state(c), cd_string(name), transport, cd_string(endpoint), enabled != 0);
  cd_woke(c);
  return idx;
}
int cd_mcp_remove(void *page, int idx) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_mcp_remove(cd_state(c), idx) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_mcp_toggle(void *page, int idx) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_mcp_toggle(cd_state(c), idx) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_mcp_set_status(void *page, int idx, int status, int tool_count, const char *error_msg) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_mcp_set_status(cd_state(c), idx, status, tool_count, cd_string(error_msg)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_mcp_oauth_require(void *page, const char *server_name, const char *auth_url, const char *pending_tool) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_mcp_oauth_require(cd_state(c), cd_string(server_name), cd_string(auth_url), cd_string(pending_tool)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_mcp_oauth_complete(void *page, const char *server_name, const char *nonce, int ok_flag) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_mcp_oauth_complete(cd_state(c), cd_string(server_name), cd_string(nonce), ok_flag != 0) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_mcp_oauth_server(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_mcp_oauth_server(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n;
}
int cd_skills_load(void *page, const char *lines) {
  CoderHost *c = page;
  if (!c) return 0;
  int n = (int)coder_page_skills_load(cd_state(c), cd_string(lines));
  cd_woke(c);
  return n;
}
int cd_skills_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_skills_count(cd_state(c)) : 0; }
int cd_skills_target(void *page) { CoderHost *c = page; return c ? (int)coder_page_skills_target(cd_state(c)) : -1; }
int cd_skills_toggle(void *page, int idx) {
  CoderHost *c = page;
  if (!c) return 0;
  int ok = coder_page_skills_toggle(cd_state(c), idx) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_skill_name(void *page, int idx, char *out, int cap) {
  CoderHost *c = page;
  if (!c || !out || cap < 2) return 0;
  int n = (int)coder_page_skill_name(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n > 0 ? n : 0] = 0;
  return n;
}
// External CLI & ACP Registry providers, cross-provider budgeted handoff, Fresh Session & multi-model fan-out (page/providers.coax; #13878).
void cd_set_codex(void *page, int on, const char *status, const char *auth) {
  CoderHost *c = page; if (!c) return;
  coder_page_set_codex(cd_state(c), on != 0, cd_string(status), cd_string(auth));
  cd_woke(c);
}
void cd_set_cursor(void *page, int on, const char *status, const char *auth) {
  CoderHost *c = page; if (!c) return;
  coder_page_set_cursor(cd_state(c), on != 0, cd_string(status), cd_string(auth));
  cd_woke(c);
}
int cd_acp_load(void *page, const char *json_text) {
  CoderHost *c = page; if (!c) return -1;
  int n = (int)coder_page_acp_load(cd_state(c), cd_string(json_text));
  cd_woke(c);
  return n;
}
int cd_acp_save(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_acp_save(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_acp_add(void *page, const char *id, const char *name, const char *program, const char *args, int enabled) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_acp_add(cd_state(c), cd_string(id), cd_string(name), cd_string(program), cd_string(args), enabled != 0) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_acp_remove(void *page, const char *id) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_acp_remove(cd_state(c), cd_string(id)) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_acp_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_acp_count(cd_state(c)) : 0; }
int cd_model_provider(void *page) { CoderHost *c = page; return c ? (int)coder_page_model_provider(cd_state(c)) : 0; }
int cd_model_id_out(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_model_id_out(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_model_acp_program(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_model_acp_program(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_model_acp_args(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_model_acp_args(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
void cd_fresh_session_set(void *page, int on) {
  CoderHost *c = page; if (!c) return;
  coder_page_fresh_session_set(cd_state(c), on != 0);
  cd_woke(c);
}
int cd_fresh_session(void *page) { CoderHost *c = page; return c && coder_page_fresh_session(cd_state(c)) ? 1 : 0; }
int cd_handoff_compose(void *page, const char *from_provider, const char *to_provider, int budget, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_handoff_compose(cd_state(c), cd_string(from_provider), cd_string(to_provider), budget, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_handoff_needed(void *page, int prev_kind, int next_kind) {
  CoderHost *c = page; return c && coder_page_handoff_needed(cd_state(c), prev_kind, next_kind) ? 1 : 0;
}
int cd_fanout_toggle(void *page, const char *model_id) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_fanout_toggle(cd_state(c), cd_string(model_id)) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
void cd_fanout_set_active(void *page, int active) {
  CoderHost *c = page; if (!c) return;
  coder_page_fanout_set_active(cd_state(c), active != 0);
  cd_woke(c);
}
void cd_fanout_clear(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_fanout_clear(cd_state(c));
  cd_woke(c);
}
int cd_fanout_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_fanout_count(cd_state(c)) : 0; }
int cd_fanout_active(void *page) { CoderHost *c = page; return c && coder_page_fanout_active(cd_state(c)) ? 1 : 0; }
int cd_fanout_target(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_fanout_target(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_fanout_prompt(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_fanout_prompt(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_voice_state(void *page) { CoderHost *c = page; return c ? (int)coder_page_voice_state(cd_state(c)) : 0; }
void cd_voice_set_state(void *page, int state, const char *msg) {
  CoderHost *c = page; if (!c) return;
  coder_page_voice_set_state(cd_state(c), state, cd_string(msg));
  cd_woke(c);
}
void cd_voice_level(void *page, float rms) {
  CoderHost *c = page; if (!c) return;
  coder_page_voice_level(cd_state(c), rms);
  cd_woke(c);
}
void cd_voice_partial(void *page, const char *text) {
  CoderHost *c = page; if (!c) return;
  coder_page_voice_partial(cd_state(c), cd_string(text));
  cd_woke(c);
}
int cd_voice_commit(void *page, const char *text) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_voice_commit(cd_state(c), cd_string(text)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_voice_backend(void *page) { CoderHost *c = page; return c ? (int)coder_page_voice_backend(cd_state(c)) : 0; }
void cd_voice_set_backend(void *page, int backend) {
  CoderHost *c = page; if (!c) return;
  coder_page_voice_set_backend(cd_state(c), backend);
  cd_woke(c);
}
int cd_voice_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_voice_mode(cd_state(c)) : 0; }
void cd_voice_set_mode(void *page, int mode) {
  CoderHost *c = page; if (!c) return;
  coder_page_voice_set_mode(cd_state(c), mode);
  cd_woke(c);
}
int cd_voice_fillers(void *page) { CoderHost *c = page; return c && coder_page_voice_fillers(cd_state(c)) ? 1 : 0; }
void cd_voice_set_fillers(void *page, int on) {
  CoderHost *c = page; if (!c) return;
  coder_page_voice_set_fillers(cd_state(c), on != 0);
  cd_woke(c);
}
int cd_voice_dict_set(void *page, const char *raw) {
  CoderHost *c = page; if (!c) return 0;
  int n = (int)coder_page_voice_dict_set(cd_state(c), cd_string(raw));
  cd_woke(c);
  return n;
}
int cd_voice_dict_get(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_voice_dict_get(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
void cd_voice_status_set(void *page, const char *mic, const char *whisper, int apple_ready, int whisper_ready) {
  CoderHost *c = page; if (!c) return;
  coder_page_voice_status_set(cd_state(c), cd_string(mic), cd_string(whisper), apple_ready != 0, whisper_ready != 0);
  cd_woke(c);
}
// Remote SSH environments, Boats, WSL2 path translation, Server Exposure & Keep-Awake (page/environments.coax; #13883).
int cd_env_load_ssh_config(void *page, const char *config) {
  CoderHost *c = page; if (!c) return 0;
  int n = (int)coder_page_env_load_ssh_config(cd_state(c), cd_string(config));
  if (n > 0) cd_woke(c);
  return n;
}
void cd_env_clear_remote(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_env_clear_remote(cd_state(c));
  cd_woke(c);
}
int cd_env_add(void *page, int kind, const char *name, const char *hostname, const char *user, int port, const char *remote_folder) {
  CoderHost *c = page; if (!c) return -1;
  int idx = (int)coder_page_env_add(cd_state(c), kind, cd_string(name), cd_string(hostname), cd_string(user), port, cd_string(remote_folder));
  if (idx >= 0) cd_woke(c);
  return idx;
}
int cd_env_set_state(void *page, int idx, int state, const char *error_msg) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_env_set_state(cd_state(c), idx, state, cd_string(error_msg)) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_env_select(void *page, int idx) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_env_select(cd_state(c), idx) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_env_active(void *page) { CoderHost *c = page; return c ? (int)coder_page_env_active(cd_state(c)) : 0; }
int cd_env_target(void *page) { CoderHost *c = page; return c ? (int)coder_page_env_target(cd_state(c)) : -1; }
int cd_env_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_env_count(cd_state(c)) : 0; }
int cd_env_kind(void *page, int idx) { CoderHost *c = page; return c ? (int)coder_page_env_kind(cd_state(c), idx) : 0; }
int cd_env_name(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_env_name(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_env_host(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_env_host(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
void cd_env_expose_set(void *page, int mode, const char *status) {
  CoderHost *c = page; if (!c) return;
  coder_page_env_expose_set(cd_state(c), mode, cd_string(status));
  cd_woke(c);
}
int cd_env_expose_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_env_expose_mode(cd_state(c)) : 0; }
void cd_env_pair_code_set(void *page, const char *code) {
  CoderHost *c = page; if (!c) return;
  coder_page_env_pair_code_set(cd_state(c), cd_string(code));
  cd_woke(c);
}
void cd_env_device_clear(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_env_device_clear(cd_state(c));
  cd_woke(c);
}
int cd_env_device_add(void *page, const char *id, const char *name, const char *detail) {
  CoderHost *c = page; if (!c) return -1;
  int idx = (int)coder_page_env_device_add(cd_state(c), cd_string(id), cd_string(name), cd_string(detail));
  if (idx >= 0) cd_woke(c);
  return idx;
}
int cd_env_device_revoke(void *page, int idx) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_env_device_revoke(cd_state(c), idx) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_env_device_target(void *page) { CoderHost *c = page; return c ? (int)coder_page_env_device_target(cd_state(c)) : -1; }
int cd_env_device_id(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_env_device_id(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_env_device_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_env_device_count(cd_state(c)) : 0; }
void cd_env_keep_awake_set(void *page, int enabled) {
  CoderHost *c = page; if (!c) return;
  coder_page_env_keep_awake_set(cd_state(c), enabled != 0);
  cd_woke(c);
}
int cd_env_keep_awake(void *page) { CoderHost *c = page; return c && coder_page_env_keep_awake(cd_state(c)) ? 1 : 0; }
void cd_env_awake_active_set(void *page, int active) {
  CoderHost *c = page; if (!c) return;
  coder_page_env_awake_active_set(cd_state(c), active != 0);
  cd_woke(c);
}
int cd_env_should_keep_awake(void *page, int bg_running) {
  CoderHost *c = page; return c && coder_page_env_should_keep_awake(cd_state(c), bg_running) ? 1 : 0;
}
int cd_env_win_to_wsl(const char *path, char *out, int cap) {
  if (!out || cap < 2) return 0;
  return cd_text_out((int)coder_page_env_win_to_wsl(cd_string(path), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_env_wsl_to_win(const char *path, const char *distro, char *out, int cap) {
  if (!out || cap < 2) return 0;
  return cd_text_out((int)coder_page_env_wsl_to_win(cd_string(path), cd_string(distro), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_stash_push(void *page, const char *text) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_stash_push(cd_state(c), cd_string(text)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_stash_pop(void *page, int idx) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_stash_pop(cd_state(c), idx) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_stash_remove(void *page, int idx) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_stash_remove(cd_state(c), idx) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_stash_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_stash_count(cd_state(c)) : 0; }
int cd_stash_save(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_stash_save(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_stash_load(void *page, const char *data) {
  CoderHost *c = page; if (!c) return 0;
  int n = (int)coder_page_stash_load(cd_state(c), cd_string(data));
  cd_woke(c);
  return n;
}
int cd_hist_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_hist_count(cd_state(c)) : 0; }
int cd_hist_save(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_hist_save(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_hist_load(void *page, const char *data) {
  CoderHost *c = page; if (!c) return 0;
  int n = (int)coder_page_hist_load(cd_state(c), cd_string(data));
  cd_woke(c);
  return n;
}
int cd_rich_quote(void *page, const char *selected) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_rich_quote(cd_state(c), cd_string(selected)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
void cd_side_chat_open(void *page, const char *chat_id, const char *title) {
  CoderHost *c = page; if (!c) return;
  coder_page_side_chat_open(cd_state(c), cd_string(chat_id), cd_string(title));
  cd_woke(c);
}
int cd_side_chat_add_turn(void *page, const char *ask, const char *reply) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_side_chat_add_turn(cd_state(c), cd_string(ask), cd_string(reply)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
void cd_side_chat_close(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_side_chat_close(cd_state(c));
  cd_woke(c);
}
int cd_side_chat_is_open(void *page) { CoderHost *c = page; return c && coder_page_side_chat_is_open(cd_state(c)) ? 1 : 0; }
int cd_side_chat_id(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_side_chat_id(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
// Scheduled tasks, HMAC-SHA256 webhooks & "Update Everything" coordinator (page/scheduled.coax; #13885).
int cd_sched_load_json(void *page, const char *json) {
  CoderHost *c = page; if (!c) return 0;
  int n = (int)coder_page_sched_load_json(cd_state(c), cd_string(json));
  cd_woke(c);
  return n;
}
int cd_sched_save_json(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_sched_save_json(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_sched_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_sched_count(cd_state(c)) : 0; }
int cd_sched_target(void *page) { CoderHost *c = page; return c ? (int)coder_page_sched_target(cd_state(c)) : -1; }
int cd_sched_add(void *page, const char *id, const char *name, const char *cron, const char *folder, const char *prompt, int enabled) {
  CoderHost *c = page; if (!c) return -1;
  int idx = (int)coder_page_sched_add(cd_state(c), cd_string(id), cd_string(name), cd_string(cron), cd_string(folder), cd_string(prompt), enabled != 0);
  if (idx >= 0) cd_woke(c);
  return idx;
}
int cd_sched_toggle(void *page, int idx) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_sched_toggle(cd_state(c), idx) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_sched_remove(void *page, int idx) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_sched_remove(cd_state(c), idx) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_sched_mark_run(void *page, int idx, int64_t now_ms, const char *chat_id) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_sched_mark_run(cd_state(c), idx, now_ms, cd_string(chat_id)) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_sched_get_id(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_sched_get_id(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_sched_get_name(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_sched_get_name(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_sched_get_cron(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_sched_get_cron(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_sched_get_folder(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_sched_get_folder(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_sched_get_prompt(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_sched_get_prompt(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_sched_is_enabled(void *page, int idx) {
  CoderHost *c = page; return c && coder_page_sched_is_enabled(cd_state(c), idx) ? 1 : 0;
}
int cd_sched_cron_valid(const char *expr) {
  return coder_page_sched_cron_valid(cd_string(expr)) ? 1 : 0;
}
int cd_sched_cron_matches(const char *expr, int minute, int hour, int dom, int month, int dow) {
  return coder_page_sched_cron_matches(cd_string(expr), minute, hour, dom, month, dow) ? 1 : 0;
}
int cd_sched_due(void *page, int64_t now_ms, int minute, int hour, int dom, int month, int dow, int *out_indices, int cap) {
  CoderHost *c = page; if (!c || !out_indices || cap < 1) return 0;
  int64_t tmp[32];
  int want = cap < 32 ? cap : 32;
  int n = (int)coder_page_sched_due(cd_state(c), now_ms, minute, hour, dom, month, dow, (coax_v_s64){tmp, want});
  int copy_n = n < want ? n : want;
  for (int i = 0; i < copy_n; i++) out_indices[i] = (int)tmp[i];
  return n;
}
void cd_sched_webhook_set(void *page, int enabled, const char *status) {
  CoderHost *c = page; if (!c) return;
  coder_page_sched_webhook_set(cd_state(c), enabled != 0, cd_string(status));
  cd_woke(c);
}
int cd_sched_webhook_admit(void *page, const char *secret, const char *delivery_id, const char *body, const char *sig_hex) {
  CoderHost *c = page; if (!c) return 1;
  return (int)coder_page_sched_webhook_admit(cd_state(c), cd_string(secret), cd_string(delivery_id), cd_string(body), cd_string(sig_hex));
}
void cd_upd_all_set(void *page, int target, const char *current, const char *latest, int available, int leases, int atomic_switch, const char *status) {
  CoderHost *c = page; if (!c) return;
  coder_page_upd_all_set(cd_state(c), target, cd_string(current), cd_string(latest), available != 0, leases, atomic_switch != 0, cd_string(status));
  cd_woke(c);
}
int cd_upd_all_decision(void *page, int target) {
  CoderHost *c = page; return c ? (int)coder_page_upd_all_decision(cd_state(c), target) : 0;
}
void cd_upd_all_busy_set(void *page, int busy) {
  CoderHost *c = page; if (!c) return;
  coder_page_upd_all_busy_set(cd_state(c), busy != 0);
  cd_woke(c);
}
// Embedded Web Preview Pane & SnapShot region capture (page/preview.coax; #13888).
void cd_preview_show(void *page, int open) {
  CoderHost *c = page; if (!c) return;
  coder_page_preview_show(cd_state(c), open != 0);
  cd_woke(c);
}
void cd_preview_toggle(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_preview_toggle(cd_state(c));
  cd_woke(c);
}
int cd_preview_open(void *page) { CoderHost *c = page; return c && coder_page_preview_open(cd_state(c)) ? 1 : 0; }
int cd_preview_shown(void *page) { CoderHost *c = page; return c && coder_page_preview_shown(cd_state(c)) ? 1 : 0; }
int cd_preview_set_url(void *page, const char *raw) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_preview_set_url(cd_state(c), cd_string(raw)) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_preview_url(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_preview_url(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
void cd_preview_set_viewport(void *page, int vp) {
  CoderHost *c = page; if (!c) return;
  coder_page_preview_set_viewport(cd_state(c), vp);
  cd_woke(c);
}
int cd_preview_viewport(void *page) { CoderHost *c = page; return c ? (int)coder_page_preview_viewport(cd_state(c)) : 0; }
void cd_preview_set_picking(void *page, int on) {
  CoderHost *c = page; if (!c) return;
  coder_page_preview_set_picking(cd_state(c), on != 0);
  cd_woke(c);
}
int cd_preview_picking(void *page) { CoderHost *c = page; return c && coder_page_preview_picking(cd_state(c)) ? 1 : 0; }
int cd_preview_rect(void *page, float out[5]) {
  CoderHost *c = page;
  return c && out && coder_page_preview_rect(cd_state(c), (coax_v_f32){out, 5}) ? 1 : 0;
}
int cd_preview_dom_pick(void *page, const char *selector, const char *tag, const char *text, const char *rect, const char *url, const char *note) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_preview_dom_pick(cd_state(c), cd_string(selector), cd_string(tag), cd_string(text), cd_string(rect), cd_string(url), cd_string(note)) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
void cd_preview_trigger_snapshot(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_preview_trigger_snapshot(cd_state(c));
  cd_woke(c);
}
int cd_preview_snapshot_done(void *page, const char *path, int64_t ts) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_preview_snapshot_done(cd_state(c), cd_string(path), ts) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_preview_last_snapshot(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_preview_last_snapshot(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
// Theme engine, customizable when-clause keybindings, and i18n locale (page/customize.coax; #13887).
void cd_cust_theme_set(void *page, int mode, int skin, int high_contrast) {
  CoderHost *c = page; if (!c) return;
  coder_page_cust_theme_set(cd_state(c), mode, skin, high_contrast != 0);
  cd_woke(c);
}
void cd_cust_system_dark_set(void *page, int is_dark) {
  CoderHost *c = page; if (!c) return;
  coder_page_cust_system_dark_set(cd_state(c), is_dark != 0);
  cd_woke(c);
}
int cd_cust_theme_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_cust_theme_mode(cd_state(c)) : 1; }
int cd_cust_theme_skin(void *page) { CoderHost *c = page; return c ? (int)coder_page_cust_theme_skin(cd_state(c)) : 0; }
int cd_cust_theme_contrast(void *page) { CoderHost *c = page; return c && coder_page_cust_theme_contrast(cd_state(c)) ? 1 : 0; }
int cd_cust_import_vscode_theme(void *page, const char *json) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_cust_import_vscode_theme(cd_state(c), cd_string(json)) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
void cd_cust_clear_vscode_theme(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_cust_clear_vscode_theme(cd_state(c));
  cd_woke(c);
}
void cd_cust_locale_set(void *page, int locale) {
  CoderHost *c = page; if (!c) return;
  coder_page_cust_locale_set(cd_state(c), locale);
  cd_woke(c);
}
int cd_cust_locale(void *page) { CoderHost *c = page; return c ? (int)coder_page_cust_locale(cd_state(c)) : 0; }
int cd_cust_kb_count(void *page) { CoderHost *c = page; return c ? (int)coder_page_cust_kb_count(cd_state(c)) : 0; }
int cd_cust_kb_target(void *page) { CoderHost *c = page; return c ? (int)coder_page_cust_kb_target(cd_state(c)) : -1; }
int cd_cust_kb_id(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_cust_kb_id(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_cust_kb_label(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_cust_kb_label(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_cust_kb_chord(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_cust_kb_chord(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_cust_kb_when(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_cust_kb_when(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_cust_kb_rebind(void *page, int idx, const char *chord, const char *when_expr, int update_when) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_cust_kb_rebind(cd_state(c), idx, cd_string(chord), cd_string(when_expr), update_when != 0) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
int cd_cust_kb_rebind_id(void *page, const char *id, const char *chord, const char *when_expr, int update_when) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_cust_kb_rebind_id(cd_state(c), cd_string(id), cd_string(chord), cd_string(when_expr), update_when != 0) ? 1 : 0;
  if (ok) cd_woke(c);
  return ok;
}
void cd_cust_kb_reset_all(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_cust_kb_reset_all(cd_state(c));
  cd_woke(c);
}
int cd_cust_kb_conflict(void *page, int idx) { CoderHost *c = page; return c ? (int)coder_page_cust_kb_conflict(cd_state(c), idx) : -1; }
int cd_cust_kb_match(void *page, const char *chord, int ctx, char *out_id, int cap) {
  CoderHost *c = page; if (!c || !out_id || cap < 2) return 0;
  return cd_text_out((int)coder_page_cust_kb_match(cd_state(c), cd_string(chord), ctx, (coax_v_u8){(uint8_t *)out_id, cap - 1}), out_id, cap);
}
// Pull Requests inbox, multi-forge source control, AI review tours & Issues view (page/forge.coax; #13886).
void cd_forge_open(void *page, int tab) {
  CoderHost *c = page; if (!c) return;
  coder_page_forge_open(cd_state(c), tab);
  cd_woke(c);
}
void cd_forge_close(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_forge_close(cd_state(c));
  cd_woke(c);
}
int cd_forge_shown(void *page) { CoderHost *c = page; return c && coder_page_forge_shown(cd_state(c)) ? 1 : 0; }
int cd_forge_set_remote(void *page, const char *url) {
  CoderHost *c = page; if (!c) return 0;
  int kind = (int)coder_page_forge_set_remote(cd_state(c), cd_string(url));
  cd_woke(c);
  return kind;
}
void cd_forge_set_status(void *page, int loading, const char *msg) {
  CoderHost *c = page; if (!c) return;
  coder_page_forge_set_status(cd_state(c), loading != 0, cd_string(msg));
  cd_woke(c);
}
void cd_forge_pr_clear(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_forge_pr_clear(cd_state(c));
  cd_woke(c);
}
int cd_forge_pr_add(void *page, int number, const char *title, const char *author, const char *head_ref, const char *base_ref, const char *state, const char *url, int draft, int is_authored, int review_requested, int files_total, uint64_t viewed_mask, int viewed_count) {
  CoderHost *c = page; if (!c) return -1;
  int idx = (int)coder_page_forge_pr_add(cd_state(c), number, cd_string(title), cd_string(author), cd_string(head_ref), cd_string(base_ref), cd_string(state), cd_string(url), draft != 0, is_authored != 0, review_requested != 0, files_total, viewed_mask, viewed_count);
  cd_woke(c);
  return idx;
}
void cd_forge_pr_finalize(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_forge_pr_finalize(cd_state(c));
  cd_woke(c);
}
int cd_forge_pr_set_viewed(void *page, int number, uint64_t mask, int count) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_forge_pr_set_viewed(cd_state(c), number, mask, count) ? 1 : 0;
  cd_woke(c);
  return ok;
}
void cd_forge_issue_clear(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_forge_issue_clear(cd_state(c));
  cd_woke(c);
}
int cd_forge_issue_add(void *page, int source, const char *key, const char *title, const char *team, const char *assignee, const char *state, const char *url, const char *body) {
  CoderHost *c = page; if (!c) return -1;
  int idx = (int)coder_page_forge_issue_add(cd_state(c), source, cd_string(key), cd_string(title), cd_string(team), cd_string(assignee), cd_string(state), cd_string(url), cd_string(body));
  cd_woke(c);
  return idx;
}
int cd_forge_selected_pr(void *page) { CoderHost *c = page; return c ? (int)coder_page_forge_selected_pr(cd_state(c)) : -1; }
int cd_forge_selected_pr_viewed(void *page, uint64_t *out_mask, int *out_count) {
  CoderHost *c = page; if (!c || !out_mask || !out_count) return 0;
  uint64_t m = 0; int64_t cnt = 0;
  int ok = coder_page_forge_selected_pr_viewed(cd_state(c), (coax_v_u64){&m, 1}, (coax_v_s64){&cnt, 1}) ? 1 : 0;
  if (ok) { *out_mask = m; *out_count = (int)cnt; }
  return ok;
}
int cd_forge_prompt(void *page, int tour, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_forge_prompt(cd_state(c), tour != 0, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_forge_issue_prompt(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_forge_issue_prompt_out(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
// Unified Gym mode bar & Lean 4 / Riemann Hypothesis Proof Workbench (page/gym-lean*.coax; #13946).
int cd_gym_mode(void *page) { CoderHost *c = page; return c ? (int)coder_page_gym_mode(cd_state(c)) : 0; }
void cd_gym_mode_set(void *page, int mode) {
  CoderHost *c = page; if (!c) return;
  coder_page_gym_mode_set(cd_state(c), mode);
  cd_woke(c);
}
int cd_lean_atlas_set(void *page, const char *json) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_lean_atlas_set(cd_state(c), cd_string(json)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_lean_module_set(void *page, const char *json) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_lean_module_set(cd_state(c), cd_string(json)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_lean_search_set(void *page, const char *json) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_lean_search_set(cd_state(c), cd_string(json)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_lean_audit_set(void *page, const char *json) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_lean_audit_set(cd_state(c), cd_string(json)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_lean_doctor_set(void *page, const char *json) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_lean_doctor_set(cd_state(c), cd_string(json)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_lean_verify_set(void *page, const char *json) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_lean_verify_set(cd_state(c), cd_string(json)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_lean_apply_plan(void *page, const char *plan_json) {
  CoderHost *c = page; if (!c) return 0;
  int ok = coder_page_lean_apply_plan(cd_state(c), cd_string(plan_json)) ? 1 : 0;
  cd_woke(c);
  return ok;
}
int cd_lean_state_json(void *page, const char *req_id, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_lean_state_json(cd_state(c), cd_string(req_id), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_lean_req_str(void *page, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_lean_req_str(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
void cd_lean_atlas_lod_set(void *page, int lod) {
  CoderHost *c = page; if (!c) return;
  coder_page_lean_atlas_lod_set(cd_state(c), lod);
  cd_woke(c);
}
void cd_lean_view_set(void *page, int view) {
  CoderHost *c = page; if (!c) return;
  coder_page_lean_view_set(cd_state(c), view);
  cd_woke(c);
}
int cd_lean_3d_rect(void *page, float out[4]) {
  CoderHost *c = page;
  return c && out && coder_page_lean_3d_rect(cd_state(c), (coax_v_f32){out, 4});
}
void cd_file_lean_goal(void *page, int line, int ok, int accomplished, const char *summary) {
  CoderHost *c = page; if (!c) return;
  coder_page_file_lean_goal(cd_state(c), line, ok != 0, accomplished != 0, cd_string(summary));
  cd_woke(c);
}
// Settings -> Providers screen & Add provider 3-step modal wizard (page/providers_settings.coax).
void cd_ps_set_meta(void *page, const char *host_name, const char *project_scope, const char *checked_label, int refreshing) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_set_meta(cd_state(c), cd_string(host_name), cd_string(project_scope), cd_string(checked_label), refreshing != 0);
  cd_woke(c);
}
void cd_ps_clear(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_clear(cd_state(c));
  cd_woke(c);
}
int cd_ps_add_instance(void *page, const char *id, int driver, const char *display_name, const char *version, int enabled, int is_default, int status, const char *headline, const char *summary, int accent) {
  CoderHost *c = page; if (!c) return -1;
  int r = (int)coder_page_ps_add_instance(cd_state(c), cd_string(id), driver, cd_string(display_name), cd_string(version), enabled != 0, is_default != 0, status, cd_string(headline), cd_string(summary), accent);
  cd_woke(c);
  return r;
}
void cd_ps_select(void *page, int idx) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_select(cd_state(c), idx);
  cd_woke(c);
}
void cd_ps_set_detail_config(void *page, const char *display_name, int accent, int auth_method, int api_key_set, const char *gcp_project, const char *gcp_location, const char *binary_path, const char *extra_path) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_set_detail_config(cd_state(c), cd_string(display_name), accent, auth_method, api_key_set != 0, cd_string(gcp_project), cd_string(gcp_location), cd_string(binary_path), cd_string(extra_path));
  cd_woke(c);
}
void cd_ps_set_detail_runtime(void *page, int rt_state, const char *rt_status, const char *rt_btn, int rt_pct, int rt_can_remove, const char *rt_err) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_set_detail_runtime(cd_state(c), rt_state, cd_string(rt_status), cd_string(rt_btn), rt_pct, rt_can_remove != 0, cd_string(rt_err));
  cd_woke(c);
}
void cd_ps_set_detail_auth(void *page, int auth_state, const char *auth_sub, const char *auth_detail, const char *auth_btn, int auth_can_signout, const char *auth_url) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_set_detail_auth(cd_state(c), auth_state, cd_string(auth_sub), cd_string(auth_detail), cd_string(auth_btn), auth_can_signout != 0, cd_string(auth_url));
  cd_woke(c);
}
void cd_ps_env_clear(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_env_clear(cd_state(c));
  cd_woke(c);
}
int cd_ps_env_add(void *page, const char *key, const char *val, int secret, int has_secret) {
  CoderHost *c = page; if (!c) return -1;
  int r = (int)coder_page_ps_env_add(cd_state(c), cd_string(key), cd_string(val), secret != 0, has_secret != 0);
  cd_woke(c);
  return r;
}
void cd_ps_models_clear(void *page) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_models_clear(cd_state(c));
  cd_woke(c);
}
int cd_ps_models_add(void *page, const char *slug, const char *name, int enabled, int favorite, int thinking, int custom) {
  CoderHost *c = page; if (!c) return -1;
  int r = (int)coder_page_ps_models_add(cd_state(c), cd_string(slug), cd_string(name), enabled != 0, favorite != 0, thinking != 0, custom != 0);
  cd_woke(c);
  return r;
}
int cd_ps_selected(void *page) { CoderHost *c = page; return c ? (int)coder_page_ps_selected(cd_state(c)) : -1; }
int cd_ps_action_idx(void *page) { CoderHost *c = page; return c ? (int)coder_page_ps_action_idx(cd_state(c)) : -1; }
int cd_ps_inst_id(void *page, int idx, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_ps_inst_id(cd_state(c), idx, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_ps_inst_enabled(void *page, int idx) { CoderHost *c = page; return c && coder_page_ps_inst_enabled(cd_state(c), idx) ? 1 : 0; }
int cd_ps_field(void *page, int which, char *out, int cap) {
  CoderHost *c = page; if (!c || !out || cap < 2) return 0;
  return cd_text_out((int)coder_page_ps_field(cd_state(c), which, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap);
}
int cd_ps_int(void *page, int which) { CoderHost *c = page; return c ? (int)coder_page_ps_int(cd_state(c), which) : 0; }
int cd_ps_env_get(void *page, int idx, char *key_out, int key_cap, char *val_out, int val_cap, int *secret_out, int *has_secret_out) {
  CoderHost *c = page; if (!c || !key_out || !val_out || key_cap < 2 || val_cap < 2) return 0;
  int64_t fl[4] = {0, 0, 0, 0};
  if (!coder_page_ps_env_get(cd_state(c), idx, (coax_v_u8){(uint8_t *)key_out, key_cap - 1}, (coax_v_u8){(uint8_t *)val_out, val_cap - 1}, (coax_v_s64){fl, 4})) return 0;
  cd_text_out((int)fl[0], key_out, key_cap);
  cd_text_out((int)fl[1], val_out, val_cap);
  if (secret_out) *secret_out = (int)fl[2];
  if (has_secret_out) *has_secret_out = (int)fl[3];
  return 1;
}
int cd_ps_model_get(void *page, int idx, char *slug_out, int slug_cap, char *name_out, int name_cap, int *enabled_out, int *fav_out, int *thinking_out, int *custom_out) {
  CoderHost *c = page; if (!c || !slug_out || !name_out || slug_cap < 2 || name_cap < 2) return 0;
  int64_t fl[6] = {0, 0, 0, 0, 0, 0};
  if (!coder_page_ps_model_get(cd_state(c), idx, (coax_v_u8){(uint8_t *)slug_out, slug_cap - 1}, (coax_v_u8){(uint8_t *)name_out, name_cap - 1}, (coax_v_s64){fl, 6})) return 0;
  cd_text_out((int)fl[0], slug_out, slug_cap);
  cd_text_out((int)fl[1], name_out, name_cap);
  if (enabled_out) *enabled_out = (int)fl[2];
  if (fav_out) *fav_out = (int)fl[3];
  if (thinking_out) *thinking_out = (int)fl[4];
  if (custom_out) *custom_out = (int)fl[5];
  return 1;
}
void cd_ps_ui_set(void *page, int wiz_open, int wiz_step, float detail_scroll, int focus_field) {
  CoderHost *c = page; if (!c) return;
  coder_page_ps_ui_set(cd_state(c), wiz_open != 0, wiz_step, detail_scroll, focus_field);
  cd_woke(c);
}
