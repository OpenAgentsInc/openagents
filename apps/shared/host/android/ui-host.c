// Firm reason: 6 (native Vulkan and Android NDK ABI; every app's behaviour is Coax).
// The shared Android host (../linux/ui-host.h, GitHub #13156): the Linux host's drawing on the
// same Vulkan helpers (verse/gpu/vkc, vks), with the component shader emitted from Coax as SPIR-V
// and the atlas the app builds in Coax (components/raster/atlas.coax).
//
// Offscreen frames and pictures (ui_open(app, 1), ui_shot_frame, ui_shot_write) run anywhere,
// as a check runs them. A window (ui_open(app, 0)) is the activity's: an app built as a
// NativeActivity (with the NDK's android_native_app_glue and UI_ANDROID_WINDOW defined) hands the
// host its android_app first (ui-host-android.h), and the host draws to the activity's
// ANativeWindow through a swapchain (verse/gpu/vkp) and takes its touches as the pointer.
#include "../linux/ui-host.h"
#include "../png.h"
#include "vkc.h"
#include "vks.h"
#ifdef UI_ANDROID_WINDOW
#include "ui-host-android.h"
#include "vkp.h"
#include <android_native_app_glue.h>
#include <vulkan/vulkan_android.h>
#include <time.h>
#endif
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define UI_VERTEX_BYTES 112   /* 28 floats: components/zeron/gpui.coax GP_VERTEX_FLOATS */

struct UiHost {
  UiApp app;
  Vkc v;
  uint32_t *words; size_t nwords;
  VkcBuffer verts, uniform;
  VksImage mono, color, target;
  int has_atlas, has_target, has_pipe;
  VksPipe pipe;
  VksPass pass;
  VksBinding binds[3];
  uint32_t w, rows;
  float dpr;
  float *frame;
  int offscreen;
#ifdef UI_ANDROID_WINDOW
  struct android_app *activity;
  Vkp p; int has_swapchain;
  VkSurfaceKHR surface;
  double last;
  int finger;      // the finger that is the pointer, or -1
#endif
};

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
  h->has_target = 1; h->w = w; h->rows = rows;
  ui_pipe(h);
}
void ui_atlas(UiHost *h, const uint8_t *mono, uint32_t w, uint32_t rows, const uint8_t *color, uint32_t cw, uint32_t crows) {
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
  if (fresh) ui_pipe(h);
}
static void ui_record(VkCommandBuffer cmd, void *user) { UiHost *h = user; vks_pipe_record(&h->pipe, cmd, &h->pass); }

// No seat offscreen.
void ui_cursor(UiHost *h, int kind) { (void)h; (void)kind; }
int ui_copy(UiHost *h, const char *bytes, size_t n) { (void)h; (void)bytes; (void)n; return 0; }
int ui_mods(UiHost *h) { (void)h; return 0; }
int ui_focused(UiHost *h) { (void)h; return 0; }
float ui_dpr(UiHost *h) { return h->dpr; }
void ui_set_dpr(UiHost *h, float dpr) { if (dpr > 0) h->dpr = dpr; }

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
#ifdef UI_ANDROID_WINDOW
static struct android_app *ui_activity;
void ui_android_app(struct android_app *app) { ui_activity = app; }
static double ui_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
// The activity's window: a surface and a swapchain on it while it has one.
static void ui_window_drop(UiHost *h) {
  if (h->has_swapchain) { vkDeviceWaitIdle(h->v.dev); vkp_close(&h->v, &h->p); h->has_swapchain = 0; }
  if (h->surface) { vkDestroySurfaceKHR(h->v.inst, h->surface, NULL); h->surface = VK_NULL_HANDLE; }
}
static void ui_window_make(UiHost *h) {
  ANativeWindow *win = h->activity->window;
  if (!win || h->has_swapchain) return;
  VkAndroidSurfaceCreateInfoKHR si = {.sType = VK_STRUCTURE_TYPE_ANDROID_SURFACE_CREATE_INFO_KHR, .window = win};
  if (vkCreateAndroidSurfaceKHR(h->v.inst, &si, NULL, &h->surface) != VK_SUCCESS) { fprintf(stderr, "ui-host: no Vulkan surface on the window\n"); return; }
  vkp_open(&h->v, &h->p, h->surface, (uint32_t)ANativeWindow_getWidth(win), (uint32_t)ANativeWindow_getHeight(win));
  h->has_swapchain = 1;
}
static void ui_on_cmd(struct android_app *a, int32_t cmd) {
  UiHost *h = a->userData;
  if (cmd == APP_CMD_TERM_WINDOW) ui_window_drop(h);
}
// Touches: the first finger down is the pointer (device pixels), as on iOS.
static int32_t ui_on_input(struct android_app *a, AInputEvent *e) {
  UiHost *h = a->userData;
  if (AInputEvent_getType(e) != AINPUT_EVENT_TYPE_MOTION || !h->app.pointer) return 0;
  int32_t action = AMotionEvent_getAction(e), what = action & AMOTION_EVENT_ACTION_MASK;
  size_t which = (size_t)((action & AMOTION_EVENT_ACTION_POINTER_INDEX_MASK) >> AMOTION_EVENT_ACTION_POINTER_INDEX_SHIFT);
  if (what == AMOTION_EVENT_ACTION_DOWN) {
    h->finger = AMotionEvent_getPointerId(e, 0);
    h->app.pointer(h->app.user, AMotionEvent_getX(e, 0), AMotionEvent_getY(e, 0), 0, 1, 0);
    return 1;
  }
  for (size_t i = 0; i < AMotionEvent_getPointerCount(e); i++) {
    if (AMotionEvent_getPointerId(e, i) != h->finger) continue;
    float x = AMotionEvent_getX(e, i), y = AMotionEvent_getY(e, i);
    if (what == AMOTION_EVENT_ACTION_MOVE) h->app.pointer(h->app.user, x, y, 1, 1, 0);
    else if (what == AMOTION_EVENT_ACTION_UP || what == AMOTION_EVENT_ACTION_CANCEL || (what == AMOTION_EVENT_ACTION_POINTER_UP && which == i)) {
      h->app.pointer(h->app.user, x, y, 2, 1, 0);
      h->finger = -1;
    }
  }
  return 1;
}
// The activity's events, waiting up to wait_ms for one (-1: until one comes). 0 once it is being destroyed.
static int ui_pump(UiHost *h, int wait_ms) {
  int events; struct android_poll_source *source;
  while (ALooper_pollOnce(wait_ms, NULL, &events, (void **)&source) >= 0) {
    if (source) source->process(h->activity, source);
    if (h->activity->destroyRequested) return 0;
    wait_ms = 0;
  }
  return !h->activity->destroyRequested;
}
#endif
UiHost *ui_open(const UiApp *app, int offscreen) {
#ifndef UI_ANDROID_WINDOW
  if (!offscreen) { fprintf(stderr, "ui-host: a window needs the activity (build with UI_ANDROID_WINDOW and ui_android_app); offscreen only\n"); return NULL; }
#else
  if (!offscreen && !ui_activity) { fprintf(stderr, "ui-host: no activity: call ui_android_app first\n"); return NULL; }
#endif
  UiHost *h = calloc(1, sizeof *h);
  if (!h) return NULL;
  h->app = *app; h->dpr = 1; h->offscreen = offscreen;
  h->words = ui_slurp(app->shader, &h->nwords);
  if (!h->words) { fprintf(stderr, "ui-host: no shader at %s\n", app->shader); free(h); return NULL; }
#ifdef UI_ANDROID_WINDOW
  if (!offscreen) {
    static const char *const inst[] = {"VK_KHR_surface", "VK_KHR_android_surface"}, *const dev[] = {"VK_KHR_swapchain"};
    vkc_open_with(&h->v, inst, 2, dev, 1);
    h->activity = ui_activity; h->finger = -1;
    ui_activity->userData = h; ui_activity->onAppCmd = ui_on_cmd; ui_activity->onInputEvent = ui_on_input;
    // the screen's density: 160 dots an inch is 1
    int32_t density = AConfiguration_getDensity(ui_activity->config);
    if (density > 0) h->dpr = density / 160.0f;
    // the window comes a moment after the activity starts
    while (!ui_activity->window) if (!ui_pump(h, -1)) { fprintf(stderr, "ui-host: the activity ended before its window came\n"); return NULL; }
    ui_window_make(h);
    h->last = ui_now();
  } else
#endif
  vkc_open(&h->v);
  h->verts = vkc_buffer(&h->v, (VkDeviceSize)app->max_vertices * UI_VERTEX_BYTES);
  h->uniform = vkc_buffer(&h->v, 16);
  h->frame = malloc((size_t)app->max_vertices * UI_VERTEX_BYTES);
  if (!h->frame) { fprintf(stderr, "ui-host: no memory for a frame's vertices\n"); return NULL; }
  return h;
}
#ifdef UI_ANDROID_WINDOW
int ui_frame(UiHost *h) {
  if (h->offscreen) return 0;
  // no window (the activity is in the background): wait for its events, a tenth of a second at most
  if (!ui_pump(h, h->has_swapchain ? 0 : 100)) return 0;
  ui_window_make(h);
  if (!h->has_swapchain) return 1;
  ANativeWindow *win = h->activity->window;
  uint32_t ww = (uint32_t)ANativeWindow_getWidth(win), wh = (uint32_t)ANativeWindow_getHeight(win);
  if (ww && wh && (ww != h->p.w || wh != h->p.h)) vkp_resize(&h->v, &h->p, ww, wh);
  int slot = vkp_reserve(&h->v, &h->p);
  if (slot < 0) { vkp_resize(&h->v, &h->p, h->p.w, h->p.h); return 1; }
  if (slot == 0) { struct timespec nap = {0, 2000000}; nanosleep(&nap, NULL); return 1; }
  ui_target(h, h->p.w, h->p.h);
  double now = ui_now();
  float dt = (float)(now - h->last); h->last = now;
  if (dt > 0.1f) dt = 0.1f;
  float *u = h->uniform.map;
  u[0] = (float)h->w; u[1] = (float)h->rows; u[2] = 0; u[3] = 0;
  int n = h->app.frame ? h->app.frame(h->app.user, h->frame, (float)h->w, (float)h->rows, h->dpr, dt) : 0;
  if (n < 0) n = 0;
  if ((uint32_t)n > h->app.max_vertices) n = (int)h->app.max_vertices;
  h->pass.vertex_count = (uint32_t)n;
  if (!h->has_pipe) return 1;   // no atlas yet: the app gives one from its first frame
  memcpy(h->verts.map, h->frame, (size_t)n * UI_VERTEX_BYTES);
  if (vkp_present(&h->v, &h->p, ui_record, h, h->target.image, h->w, h->rows, NULL, NULL)) vkp_resize(&h->v, &h->p, h->p.w, h->p.h);
  return 1;
}
#else
int ui_frame(UiHost *h) { (void)h; return 0; }
#endif
int ui_shot_frame(UiHost *h, uint32_t w, uint32_t rows, float dt) {
  if (!w || !rows) return 0;
  ui_target(h, w, rows);
  float *u = h->uniform.map;
  u[0] = (float)w; u[1] = (float)rows; u[2] = 0; u[3] = 0;
  int n = h->app.frame ? h->app.frame(h->app.user, h->frame, (float)w, (float)rows, h->dpr, dt) : 0;
  if (n < 0) n = 0;
  if ((uint32_t)n > h->app.max_vertices) n = (int)h->app.max_vertices;
  h->pass.vertex_count = (uint32_t)n;
  if (!h->has_pipe) return 1;
  memcpy(h->verts.map, h->frame, (size_t)n * UI_VERTEX_BYTES);
  vks_submit(&h->v, ui_record, h);
  return 1;
}
int ui_shot_write(UiHost *h, const char *png) {
  if (!h->has_target || !h->has_pipe) return 0;
  uint8_t *px = malloc((size_t)h->w * h->rows * 4);
  if (!px) return 0;
  vks_read(&h->v, &h->target, px, (size_t)h->w * h->rows * 4);
  int ok = ui_png_write(png, px, h->w, h->rows);
  free(px);
  return ok;
}
void ui_close(UiHost *h) {
  if (!h) return;
  vkDeviceWaitIdle(h->v.dev);
#ifdef UI_ANDROID_WINDOW
  if (!h->offscreen) { ui_window_drop(h); h->activity->onAppCmd = NULL; h->activity->onInputEvent = NULL; h->activity->userData = NULL; }
#endif
  if (h->has_pipe) vks_pipe_close(&h->v, &h->pipe);
  if (h->has_target) vks_image_free(&h->v, &h->target);
  if (h->has_atlas) { vks_image_free(&h->v, &h->mono); vks_image_free(&h->v, &h->color); }
  vkc_free(&h->v, &h->verts); vkc_free(&h->v, &h->uniform);
  vkc_close(&h->v);
  free(h->frame); free(h->words); free(h);
}
