// Headless ui-host.h implementation for Tier 1 deterministic host driver.
// Runs real apps/coder-desktop/host.c + page.coax without any GPU or window system.
#include "ui_host_stub.h"
#include <stdlib.h>
#include <string.h>
#include <time.h>

struct UiHost {
  UiApp app;
  float w, h, dpr;
  float *vertices;
  int last_vertices;
  double last_frame_ms;
  int cursor;
};

static double now_ms(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1e6;
}

UiHost *ui_open(const UiApp *app, int offscreen) {
  (void)offscreen;
  if (!app) return NULL;
  UiHost *h = calloc(1, sizeof *h);
  if (!h) return NULL;
  h->app = *app;
  h->w = app->width > 0 ? (float)app->width : 900.0f;
  h->h = app->height > 0 ? (float)app->height : 640.0f;
  h->dpr = 1.0f;
  uint32_t max_v = app->max_vertices > 0 ? app->max_vertices : 200000;
  h->app.max_vertices = max_v;
  h->vertices = calloc((size_t)max_v * 28u, sizeof(float));
  if (!h->vertices) { free(h); return NULL; }
  return h;
}

void ui_close(UiHost *h) {
  if (!h) return;
  free(h->vertices);
  free(h);
}

void ui_atlas(UiHost *h, const uint8_t *mono, uint32_t mw, uint32_t mh,
              const uint8_t *color, uint32_t cw, uint32_t ch) {
  (void)h; (void)mono; (void)mw; (void)mh; (void)color; (void)cw; (void)ch;
}

float ui_dpr(UiHost *h) { return h ? h->dpr : 1.0f; }
void ui_set_dpr(UiHost *h, float dpr) { if (h && dpr > 0.0f) h->dpr = dpr; }
void ui_cursor(UiHost *h, int kind) { if (h) h->cursor = kind; }
// No pasteboard headless: Copy goes nowhere and Paste finds nothing (the page's fields, page/editor.coax).
int ui_copy(UiHost *h, const char *bytes, size_t n) { (void)h; (void)bytes; (void)n; return 0; }
int ui_paste_text(UiHost *h, char *out, size_t cap) { (void)h; if (cap) out[0] = 0; return 0; }
// No files or pictures on it either: Command-V in the composer falls through to its text (host.c).
int ui_paste_files(UiHost *h) { (void)h; return 0; }
void *ui_view(UiHost *h) { return (void *)h; }

int ui_frame(UiHost *h) {
  if (!h || !h->app.frame) return 0;
  double t0 = now_ms();
  int n = h->app.frame(h->app.user, h->vertices, h->w * h->dpr, h->h * h->dpr, h->dpr, 1.0f / 60.0f);
  double t1 = now_ms();
  h->last_vertices = n;
  h->last_frame_ms = t1 - t0;
  return 1;
}

int ui_shot_frame(UiHost *h, uint32_t w, uint32_t h_px, float dt) {
  if (!h || !h->app.frame) return 0;
  double t0 = now_ms();
  int n = h->app.frame(h->app.user, h->vertices, (float)w, (float)h_px, h->dpr, dt);
  double t1 = now_ms();
  h->last_vertices = n;
  h->last_frame_ms = t1 - t0;
  return 1;
}

int ui_shot_write(UiHost *h, const char *path) {
  (void)h; (void)path;
  return 1;
}

int ui_stub_last_vertices(UiHost *h) { return h ? h->last_vertices : 0; }
int ui_stub_max_vertices(UiHost *h) { return h ? (int)h->app.max_vertices : 0; }
double ui_stub_last_frame_ms(UiHost *h) { return h ? h->last_frame_ms : 0.0; }

void ui_stub_set_size(UiHost *h, float w, float h_px, float dpr) {
  if (!h) return;
  if (w > 0.0f) h->w = w;
  if (h_px > 0.0f) h->h = h_px;
  if (dpr > 0.0f) h->dpr = dpr;
}

void ui_stub_pointer(UiHost *h, float x, float y, int phase, int clicks, int mods) {
  if (h && h->app.pointer) h->app.pointer(h->app.user, x, y, phase, clicks, mods);
}

void ui_stub_wheel(UiHost *h, float dy) {
  if (h && h->app.wheel) h->app.wheel(h->app.user, dy);
}

int ui_focused(UiHost *h) { (void)h; return 1; }
