// Firm reason: 6 (native Wayland and Vulkan ABI; every app's behaviour is Coax).
// The shared Linux host (ui-host.h).
#define _GNU_SOURCE
#include "ui-host.h"
#include "vkc.h"
#include "vks.h"
#include "vkp.h"
#include <stdbool.h>
// vkw.h serves a realm too, and names its key calls; an app here takes keys through VkwTo.realm_key instead.
bool realm_key(void *state, int key) { (void)state; (void)key; return false; }
bool realm_key_up(void *state, int key) { (void)state; (void)key; return false; }
#include "vkw.h"
#include <wayland-cursor.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define UI_VERTEX_BYTES 112   /* 28 floats: components/zeron/gpui.coax GP_VERTEX_FLOATS */

struct UiHost {
  UiApp app;
  int offscreen;
  Vkc v;
  Vkw win;
  Vkp p;
  uint32_t *words; size_t nwords;
  VkcBuffer verts, uniform;
  VksImage mono, color, target;
  int has_atlas, has_target, has_pipe;
  VksPipe pipe;
  VksPass pass;
  VksBinding binds[3];
  uint32_t w, rows;
  float dpr;
  double last;
  // what the last presented frame was made of: when the next is the same, nothing is drawn
  uint64_t shown; int stale;
  // the frame's vertices in ordinary memory: the GPU's mapped buffer is slow to read back, and a
  // frame is compared before it is drawn
  float *frame;
  // the pointer: where the left button went down and when, for double and triple clicks
  double down_at; float down_x, down_y; int clicks, held;
  // the cursor
  struct wl_registry *registry;
  struct wl_shm *shm;
  struct wl_cursor_theme *theme;
  struct wl_surface *cursor_surface;
  struct wl_pointer *pointer;
  uint32_t enter_serial;
  int cursor_kind;
};

static double ui_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

// ---- the pipeline

static void ui_pipe(UiHost *h) {
  if (h->has_pipe) { vkDeviceWaitIdle(h->v.dev); vks_pipe_close(&h->v, &h->pipe); h->has_pipe = 0; }
  if (!h->has_atlas || !h->has_target) return;
  h->binds[0] = (VksBinding){.kind = VKS_STORAGE, .buffer = &h->uniform};
  h->binds[1] = (VksBinding){.kind = VKS_IMAGE, .image = &h->mono};
  h->binds[2] = (VksBinding){.kind = VKS_IMAGE, .image = &h->color};
  h->pass = (VksPass){.words = h->words, .count = h->nwords, .bindings = h->binds, .nb = 3, .vertices = &h->verts, .stride = UI_VERTEX_BYTES,
                      .nattrs = 7, .vertex_count = 0, .instance_count = 1, .ntargets = 1, .blend = VKS_ALPHA, .clear = {0, 0, 0, 1}};
  h->pass.targets[0] = &h->target;
  for (uint32_t i = 0; i < 7; i++) h->pass.attrs[i] = (VkVertexInputAttributeDescription){.location = i, .binding = 0, .format = VK_FORMAT_R32G32B32A32_SFLOAT, .offset = 16 * i};
  vks_pipe_open(&h->v, &h->pipe, &h->pass);
  h->has_pipe = 1;
}
static void ui_target(UiHost *h, uint32_t w, uint32_t rows) {
  if (h->has_target && h->w == w && h->rows == rows) return;
  vkDeviceWaitIdle(h->v.dev);
  if (h->has_pipe) { vks_pipe_close(&h->v, &h->pipe); h->has_pipe = 0; }
  if (h->has_target) vks_image_free(&h->v, &h->target);
  h->target = vks_image(&h->v, w, rows, 1, 0, VK_FORMAT_R8G8B8A8_UNORM, VKS_TARGET | VKS_TEXTURE);
  h->has_target = 1; h->w = w; h->rows = rows; h->stale = 1;
  ui_pipe(h);
}
void ui_atlas(UiHost *h, const uint8_t *mono, uint32_t w, uint32_t rows, const uint8_t *color, uint32_t cw, uint32_t crows) {
  if (!h->offscreen) vkp_wait(&h->v, &h->p);
  int fresh = !h->has_atlas || h->mono.w != w || h->mono.h != rows || h->color.w != cw || h->color.h != crows;
  if (fresh) {
    vkDeviceWaitIdle(h->v.dev);
    if (h->has_pipe) { vks_pipe_close(&h->v, &h->pipe); h->has_pipe = 0; }
    if (h->has_atlas) { vks_image_free(&h->v, &h->mono); vks_image_free(&h->v, &h->color); }
    h->mono = vks_image(&h->v, w, rows, 1, 0, VK_FORMAT_R8_UNORM, VKS_TEXTURE);
    h->color = vks_image(&h->v, cw, crows, 1, 0, VK_FORMAT_R8G8B8A8_UNORM, VKS_TEXTURE);
    h->has_atlas = 1;
  }
  vks_upload(&h->v, &h->mono, mono, (size_t)w * rows);
  vks_upload(&h->v, &h->color, color, (size_t)cw * crows * 4);
  h->stale = 1;
  if (fresh) ui_pipe(h);
}
// A frame's vertices as one number: equal numbers are taken as the same picture.
static uint64_t ui_digest(const UiHost *h) {
  const uint64_t *words = (const uint64_t *)h->frame;
  size_t n = (size_t)h->pass.vertex_count * UI_VERTEX_BYTES / 8;
  uint64_t d = 0x9e3779b97f4a7c15ull ^ h->pass.vertex_count ^ ((uint64_t)h->w << 32) ^ ((uint64_t)h->rows << 48);
  for (size_t i = 0; i < n; i++) { d ^= words[i]; d *= 0xff51afd7ed558ccdull; d ^= d >> 29; }
  return d;
}
static void ui_record(VkCommandBuffer cmd, void *user) { UiHost *h = user; vks_pipe_record(&h->pipe, cmd, &h->pass); }
// The frame's vertices to the GPU's buffer, once it is known they will be drawn.
static void ui_hand_over(UiHost *h) { memcpy(h->verts.map, h->frame, (size_t)h->pass.vertex_count * UI_VERTEX_BYTES); }
static void ui_fill(UiHost *h, float dt) {
  float *u = h->uniform.map;
  u[0] = (float)h->w; u[1] = (float)h->rows; u[2] = 0; u[3] = 0;
  int n = h->app.frame ? h->app.frame(h->app.user, h->frame, (float)h->w, (float)h->rows, h->dpr, dt) : 0;
  if (n < 0) n = 0;
  if ((uint32_t)n > h->app.max_vertices) n = (int)h->app.max_vertices;
  h->pass.vertex_count = (uint32_t)n;
}

// ---- the seat

static void to_pointer(void *user, float x, float y, int phase) {
  UiHost *h = user;
  if (phase == 0) {
    double t = ui_now();
    int again = t - h->down_at < 0.4 && fabsf(x - h->down_x) < 5 && fabsf(y - h->down_y) < 5;
    h->clicks = again && h->clicks < 3 ? h->clicks + 1 : 1;
    h->down_at = t; h->down_x = x; h->down_y = y; h->held = 1;
  }
  if (phase == 2) h->held = 0;
  if (h->app.pointer) h->app.pointer(h->app.user, x * h->dpr, y * h->dpr, phase, h->clicks ? h->clicks : 1, ui_mods(h));
}
static void to_moved(void *user, float x, float y) {
  UiHost *h = user;
  if (!h->held && h->app.pointer) h->app.pointer(h->app.user, x * h->dpr, y * h->dpr, 1, 1, ui_mods(h));
}
static void to_wheel(void *user, float dy) { UiHost *h = user; if (h->app.wheel) h->app.wheel(h->app.user, dy); }
static uint32_t to_key(void *user, int key, int mods, int down) { UiHost *h = user; return h->app.key ? (uint32_t)h->app.key(h->app.user, key, mods, down) : 0; }
static void to_text(void *user, const char *bytes, size_t n) { UiHost *h = user; if (h->app.text) h->app.text(h->app.user, bytes, n); }
static int to_wants(void *user) { UiHost *h = user; return h->app.wants_text ? h->app.wants_text(h->app.user) : 0; }
static int to_rect(void *user, float rect[4]) { UiHost *h = user; return h->app.text_rect ? h->app.text_rect(h->app.user, rect) : 0; }
static void to_error(void *user, const char *message) { (void)user; fprintf(stderr, "ui-host: %s\n", message); }

// The cursor, from the session's cursor theme. With no theme the compositor's last cursor stays.
static void ui_cursor_show(UiHost *h) {
  if (!h->pointer || !h->theme || !h->cursor_surface) return;
  static const char *const names[4][3] = {{"default", "left_ptr", NULL}, {"text", "xterm", NULL}, {"pointer", "hand2", "hand1"}, {"ew-resize", "sb_h_double_arrow", "col-resize"}};
  struct wl_cursor *c = NULL;
  int kind = h->cursor_kind < 0 || h->cursor_kind > 3 ? 0 : h->cursor_kind;
  for (int i = 0; i < 3 && !c; i++) if (names[kind][i]) c = wl_cursor_theme_get_cursor(h->theme, names[kind][i]);
  if (!c) c = wl_cursor_theme_get_cursor(h->theme, "left_ptr");
  if (!c || !c->image_count) return;
  struct wl_cursor_image *img = c->images[0];
  wl_surface_attach(h->cursor_surface, wl_cursor_image_get_buffer(img), 0, 0);
  wl_surface_damage(h->cursor_surface, 0, 0, (int32_t)img->width, (int32_t)img->height);
  wl_surface_commit(h->cursor_surface);
  wl_pointer_set_cursor(h->pointer, h->enter_serial, h->cursor_surface, (int32_t)img->hotspot_x, (int32_t)img->hotspot_y);
}
static void to_enter(void *user, struct wl_pointer *pointer, uint32_t serial) { UiHost *h = user; h->pointer = pointer; h->enter_serial = serial; ui_cursor_show(h); }
void ui_cursor(UiHost *h, int kind) { if (kind != h->cursor_kind) { h->cursor_kind = kind; ui_cursor_show(h); } }
static void shm_global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t version) {
  UiHost *h = d; (void)version;
  if (!strcmp(iface, wl_shm_interface.name) && !h->shm) h->shm = wl_registry_bind(r, name, &wl_shm_interface, 1);
}
static void shm_global_remove(void *d, struct wl_registry *r, uint32_t name) { (void)d; (void)r; (void)name; }
static const struct wl_registry_listener shm_listener = {.global = shm_global, .global_remove = shm_global_remove};

int ui_mods(UiHost *h) { return h->offscreen ? 0 : hwi_sdk_mods(&h->win.input); }
int ui_copy(UiHost *h, const char *bytes, size_t n) { return h->offscreen ? 0 : vkw_clipboard_set(&h->win, bytes, n); }
float ui_dpr(UiHost *h) { return h->dpr; }
void ui_set_dpr(UiHost *h, float dpr) { if (dpr > 0) h->dpr = dpr; }

// ---- open, frame, close

static uint32_t *ui_slurp(const char *path, size_t *words) {
  FILE *f = fopen(path, "rb");
  if (!f) return NULL;
  fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
  if (size <= 0 || size % 4) { fclose(f); return NULL; }
  uint32_t *p = malloc((size_t)size);
  if (!p || fread(p, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(p); return NULL; }
  fclose(f);
  *words = (size_t)size / 4;
  return p;
}
UiHost *ui_open(const UiApp *app, int offscreen) {
  UiHost *h = calloc(1, sizeof *h);
  if (!h) return NULL;
  h->app = *app; h->offscreen = offscreen; h->dpr = 1; h->cursor_kind = 0;
  h->words = ui_slurp(app->shader, &h->nwords);
  if (!h->words) { fprintf(stderr, "ui-host: no shader at %s\n", app->shader); free(h); return NULL; }
  if (offscreen) vkc_open(&h->v);
  else {
    VkwTo to = {.pointer = to_pointer, .wheel = to_wheel, .user = h, .moved = to_moved, .realm_key = to_key, .text = to_text,
                .wants_text = to_wants, .text_rect = to_rect, .text_error = to_error, .pointer_enter = to_enter, .app_id = app->app_id};
    vkw_open(&h->v, &h->win, app->title, app->width, app->height, 0, to);
    vkp_open(&h->v, &h->p, h->win.vk_surface, (uint32_t)h->win.want_w, (uint32_t)h->win.want_h);
    h->registry = wl_display_get_registry(h->win.display);
    wl_registry_add_listener(h->registry, &shm_listener, h);
    wl_display_roundtrip(h->win.display);
    if (h->shm) {
      const char *size = getenv("XCURSOR_SIZE");
      h->theme = wl_cursor_theme_load(getenv("XCURSOR_THEME"), size && atoi(size) > 0 ? atoi(size) : 24, h->shm);
      h->cursor_surface = wl_compositor_create_surface(h->win.compositor);
    }
  }
  h->verts = vkc_buffer(&h->v, (VkDeviceSize)app->max_vertices * UI_VERTEX_BYTES);
  h->frame = malloc((size_t)app->max_vertices * UI_VERTEX_BYTES);
  if (!h->frame) { fprintf(stderr, "ui-host: no memory for a frame's vertices\n"); return NULL; }
  h->uniform = vkc_buffer(&h->v, 16);
  h->last = ui_now();
  return h;
}
int ui_frame(UiHost *h) {
  if (h->offscreen) return 0;
  vkw_pump(&h->win);
  if (h->win.closed) return 0;
  if (h->win.want_w > 0 && h->win.want_h > 0 && ((uint32_t)h->win.want_w != h->p.w || (uint32_t)h->win.want_h != h->p.h))
    vkp_resize(&h->v, &h->p, (uint32_t)h->win.want_w, (uint32_t)h->win.want_h);
  // A swapchain image without waiting for one: while the compositor holds them all (the window is
  // hidden, or it is waiting for this client to take a new size) the loop keeps reading the seat
  // and the compositor's events. Waiting in the acquire instead stopped the app for good on a resize.
  int slot = vkp_reserve(&h->v, &h->p);
  double now = ui_now();
  if (slot < 0) { vkp_resize(&h->v, &h->p, h->p.w, h->p.h); return 1; }
  if (slot == 0) {
    // no image: the app still gets a frame's worth of time ten times a second, so a turn goes on while hidden
    if (now - h->last >= 0.1) { float dt = (float)(now - h->last); h->last = now; ui_target(h, h->p.w, h->p.h); ui_fill(h, dt > 0.1f ? 0.1f : dt); }
    struct timespec nap = {0, 2000000}; nanosleep(&nap, NULL);
    return 1;
  }
  ui_target(h, h->p.w, h->p.h);
  float dt = (float)(now - h->last); h->last = now;
  if (dt > 0.1f) dt = 0.1f;
  ui_fill(h, dt);
  if (!h->has_pipe) return 1;   // no atlas yet: the app gives one from its first frame
  // The same vertices over the same atlas at the same size are the same picture: it is not drawn
  // or presented again, and the loop waits for the seat or the compositor (a frame's time at most,
  // so the app still polls its engines). A still window then costs the app's frame and no GPU work.
  uint64_t digest = ui_digest(h);
  if (!h->stale && digest == h->shown) {
    struct pollfd wake = {.fd = wl_display_get_fd(h->win.display), .events = POLLIN};
    wl_display_flush(h->win.display);
    poll(&wake, 1, 16);
    return 1;
  }
  h->shown = digest; h->stale = 0;
  ui_hand_over(h);
  if (vkp_present(&h->v, &h->p, ui_record, h, h->target.image, h->w, h->rows, NULL, NULL)) { vkp_resize(&h->v, &h->p, h->p.w, h->p.h); h->stale = 1; }
  // UI_HOST_TRACE=1: frames presented each second, on stderr
  static int trace = -1, frames; static double since;
  if (trace < 0) trace = getenv("UI_HOST_TRACE") != NULL;
  if (trace) { frames++; if (now - since >= 1) { fprintf(stderr, "ui-host: %d frames, %ux%u\n", frames, h->w, h->rows); frames = 0; since = now; } }
  return 1;
}
int ui_shot_frame(UiHost *h, uint32_t w, uint32_t rows, float dt) {
  if (!h->offscreen || !w || !rows) return 0;
  ui_target(h, w, rows);
  ui_fill(h, dt);
  if (!h->has_pipe) return 1;
  ui_hand_over(h);
  vks_submit(&h->v, ui_record, h);
  return 1;
}

// A PNG of RGBA rows. Each row is filtered against the one above (a flat interface is then mostly
// zeros) and deflated with the fixed Huffman codes and runs of a repeated byte, which is enough to
// make a picture of an interface a few tens of kilobytes without a compression library.
typedef struct { uint8_t *p; size_t n; uint32_t bits; int count; } UiBits;
static void ui_bits(UiBits *b, uint32_t v, int n) {
  b->bits |= v << b->count; b->count += n;
  while (b->count >= 8) { b->p[b->n++] = (uint8_t)b->bits; b->bits >>= 8; b->count -= 8; }
}
static void ui_code(UiBits *b, uint32_t code, int n) {   // a Huffman code goes most significant bit first
  uint32_t r = 0;
  for (int i = 0; i < n; i++) r |= ((code >> i) & 1) << (n - 1 - i);
  ui_bits(b, r, n);
}
static void ui_symbol(UiBits *b, int c) {
  if (c < 144) ui_code(b, 0x30u + (uint32_t)c, 8);
  else if (c < 256) ui_code(b, 0x190u + (uint32_t)(c - 144), 9);
  else if (c < 280) ui_code(b, (uint32_t)(c - 256), 7);
  else ui_code(b, 0xC0u + (uint32_t)(c - 280), 8);
}
static void ui_run(UiBits *b, int len) {   // 3 to 258 more of the byte before
  static const int base[29] = {3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258};
  static const int extra[29] = {0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0};
  int k = 28;
  while (base[k] > len) k--;
  ui_symbol(b, 257 + k);
  if (extra[k]) ui_bits(b, (uint32_t)(len - base[k]), extra[k]);
  ui_code(b, 0, 5);   // distance 1
}
static uint32_t crc_table[256];
static uint32_t ui_crc(uint32_t c, const uint8_t *p, size_t n) {
  if (!crc_table[1]) for (uint32_t i = 0; i < 256; i++) { uint32_t k = i; for (int j = 0; j < 8; j++) k = k & 1 ? 0xedb88320u ^ (k >> 1) : k >> 1; crc_table[i] = k; }
  c = ~c;
  for (size_t i = 0; i < n; i++) c = crc_table[(c ^ p[i]) & 255] ^ (c >> 8);
  return ~c;
}
static void ui_chunk(FILE *f, const char *type, const uint8_t *data, size_t n) {
  uint8_t len[4] = {(uint8_t)(n >> 24), (uint8_t)(n >> 16), (uint8_t)(n >> 8), (uint8_t)n};
  fwrite(len, 1, 4, f); fwrite(type, 1, 4, f);
  if (n) fwrite(data, 1, n, f);
  uint32_t c = ui_crc(0, (const uint8_t *)type, 4);
  if (n) c = ui_crc(c, data, n);
  uint8_t crc[4] = {(uint8_t)(c >> 24), (uint8_t)(c >> 16), (uint8_t)(c >> 8), (uint8_t)c};
  fwrite(crc, 1, 4, f);
}
int ui_shot_write(UiHost *h, const char *png) {
  if (!h->has_target || !h->has_pipe) return 0;
  size_t w = h->w, rows = h->rows, raw = rows * (w * 4 + 1);
  uint8_t *px = malloc(w * rows * 4), *scan = malloc(raw), *z = malloc(raw + raw / 8 + 64);
  if (!px || !scan || !z) { free(px); free(scan); free(z); return 0; }
  vks_read(&h->v, &h->target, px, w * rows * 4);
  for (size_t y = 0; y < rows; y++) {
    uint8_t *row = scan + y * (w * 4 + 1);
    row[0] = 2;   // the filter: each byte less the one above it
    for (size_t x = 0; x < w * 4; x++) row[1 + x] = (uint8_t)(px[y * w * 4 + x] - (y ? px[(y - 1) * w * 4 + x] : 0));
  }
  uint32_t a = 1, b = 0;
  for (size_t i = 0; i < raw; i++) { a = (a + scan[i]) % 65521; b = (b + a) % 65521; }
  UiBits bits = {z, 0, 0, 0};
  z[bits.n++] = 0x78; z[bits.n++] = 0x01;
  ui_bits(&bits, 1, 1); ui_bits(&bits, 1, 2);   // the last block, fixed codes
  for (size_t i = 0; i < raw;) {
    size_t run = 0;
    if (i > 0) while (i + run < raw && run < 258 && scan[i + run] == scan[i - 1]) run++;
    if (run >= 3) { ui_run(&bits, (int)run); i += run; }
    else { ui_symbol(&bits, scan[i]); i++; }
  }
  ui_symbol(&bits, 256);
  if (bits.count) ui_bits(&bits, 0, 8 - bits.count);
  size_t n = bits.n;
  z[n++] = (uint8_t)(b >> 8); z[n++] = (uint8_t)b; z[n++] = (uint8_t)(a >> 8); z[n++] = (uint8_t)a;
  FILE *f = fopen(png, "wb");
  int ok = 0;
  if (f) {
    static const uint8_t sig[8] = {137, 80, 78, 71, 13, 10, 26, 10};
    uint8_t head[13] = {(uint8_t)(w >> 24), (uint8_t)(w >> 16), (uint8_t)(w >> 8), (uint8_t)w, (uint8_t)(rows >> 24), (uint8_t)(rows >> 16), (uint8_t)(rows >> 8), (uint8_t)rows, 8, 6, 0, 0, 0};
    fwrite(sig, 1, 8, f); ui_chunk(f, "IHDR", head, 13); ui_chunk(f, "IDAT", z, n); ui_chunk(f, "IEND", NULL, 0);
    ok = fclose(f) == 0;
  }
  free(px); free(scan); free(z);
  return ok;
}
void ui_close(UiHost *h) {
  if (!h) return;
  vkDeviceWaitIdle(h->v.dev);
  if (h->has_pipe) vks_pipe_close(&h->v, &h->pipe);
  if (h->has_target) vks_image_free(&h->v, &h->target);
  if (h->has_atlas) { vks_image_free(&h->v, &h->mono); vks_image_free(&h->v, &h->color); }
  vkc_free(&h->v, &h->verts); vkc_free(&h->v, &h->uniform);
  if (h->offscreen) vkc_close(&h->v);
  else {
    vkp_close(&h->v, &h->p);
    if (h->cursor_surface) wl_surface_destroy(h->cursor_surface);
    if (h->theme) wl_cursor_theme_destroy(h->theme);
    if (h->shm) wl_shm_destroy(h->shm);
    if (h->registry) wl_registry_destroy(h->registry);
    vkw_close(&h->v, &h->win);
  }
  free(h->frame); free(h->words); free(h);
}
