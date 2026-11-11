// Firm reason: 6 (native Vulkan and Android NDK ABI; every app's behaviour is Coax).
// The shared Android host (../linux/ui-host.h, GitHub #13156): the Linux host's drawing on the
// same Vulkan helpers (verse/gpu/vkc, vks), with the component shader emitted from Coax as SPIR-V
// and the atlas the app builds in Coax (components/raster/atlas.coax).
//
// Built today: offscreen frames and pictures (ui_open(app, 1), ui_shot_frame, ui_shot_write),
// which is what a check runs. A window (ui_open(app, 0)) needs the activity's ANativeWindow
// and a swapchain on it; it is not written yet, and ui_open says so and returns NULL.
#include "../linux/ui-host.h"
#include "../png.h"
#include "vkc.h"
#include "vks.h"
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
UiHost *ui_open(const UiApp *app, int offscreen) {
  if (!offscreen) { fprintf(stderr, "ui-host: Android windows are not written yet (apps/shared/host/android/ui-host.c); offscreen only\n"); return NULL; }
  UiHost *h = calloc(1, sizeof *h);
  if (!h) return NULL;
  h->app = *app; h->dpr = 1;
  h->words = ui_slurp(app->shader, &h->nwords);
  if (!h->words) { fprintf(stderr, "ui-host: no shader at %s\n", app->shader); free(h); return NULL; }
  vkc_open(&h->v);
  h->verts = vkc_buffer(&h->v, (VkDeviceSize)app->max_vertices * UI_VERTEX_BYTES);
  h->uniform = vkc_buffer(&h->v, 16);
  h->frame = malloc((size_t)app->max_vertices * UI_VERTEX_BYTES);
  if (!h->frame) { fprintf(stderr, "ui-host: no memory for a frame's vertices\n"); return NULL; }
  return h;
}
int ui_frame(UiHost *h) { (void)h; return 0; }
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
  if (h->has_pipe) vks_pipe_close(&h->v, &h->pipe);
  if (h->has_target) vks_image_free(&h->v, &h->target);
  if (h->has_atlas) { vks_image_free(&h->v, &h->mono); vks_image_free(&h->v, &h->color); }
  vkc_free(&h->v, &h->verts); vkc_free(&h->v, &h->uniform);
  vkc_close(&h->v);
  free(h->frame); free(h->words); free(h);
}
