// Firm reason: 6 (native Wayland and Vulkan ABI; every app's behaviour is Coax).
// The shared Linux host (ui-host.h).
#define _GNU_SOURCE
#include "ui-host.h"
#ifdef UI_ATSPI
#include "access-bus.h"
#endif
#include "../drive.h"
#include <stdarg.h>
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

typedef struct {VksPipe pipe;VksPass pass;VksBinding binds[4];int target,source,clear;} UiCompPipe;
struct UiHost {
  UiDrive drive;   // coax drive (../drive.c)
  UiApp app;
  int offscreen;
  Vkc v;
  Vkw win;
  Vkp p;
  uint32_t *words; size_t nwords;
  VkcBuffer verts, uniform;
  VksImage mono, color, target;
  int has_atlas, has_target, has_pipe;
  UiComposition composition;int composed;
  VksImage compose_targets[16],images[16];int ncompose_targets;
  VksImage *ext_textures[16];
  uint64_t image_generation[16];
  UiCompPipe cp[64];int ncp;uint64_t compose_signature;uint32_t image_mask;uint32_t *comp_words;size_t comp_nwords;

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
#ifdef UI_ATSPI
  UiAccess *access;
  int access_attempted;
#endif
};

static double ui_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
// No swapchain image: a short sleep. An unchanged frame: the seat or the compositor, a frame's time at most.
static void ui_host_nap(UiHost *h) { (void)h; struct timespec nap = {0, 2000000}; nanosleep(&nap, NULL); }
static void ui_host_idle(UiHost *h) {
  struct pollfd wake = {.fd = wl_display_get_fd(h->win.display), .events = POLLIN};
  wl_display_flush(h->win.display);
  poll(&wake, 1, 16);
}
#include "../vulkan-ui.c"

// ---- the seat

static void to_pointer(void *user, float x, float y, int phase) { ui_seat_pointer(user, x, y, phase); }
static void to_moved(void *user, float x, float y) { ui_seat_moved(user, x, y); }
static void to_wheel(void *user, float dy) {
  UiHost *h = user;
  if ((ui_mods(h) & 1) && h->app.scroll_x) {
    h->app.scroll_x(h->app.user, -dy / (h->dpr >= 1.0f ? h->dpr : 1.0f), 1);
    return;
  }
  if (h->app.wheel) h->app.wheel(h->app.user, dy);
}
static uint32_t to_key(void *user, int key, int mods, int down) { UiHost *h = user; return h->app.key ? (uint32_t)h->app.key(h->app.user, key, mods, down) : 0; }
static void to_text(void *user, const char *bytes, size_t n) { UiHost *h = user; if (h->app.text) h->app.text(h->app.user, bytes, n); }
static int to_wants(void *user) { UiHost *h = user; return h->app.wants_text ? h->app.wants_text(h->app.user) : 0; }
static int to_rect(void *user, float rect[4]) { UiHost *h = user; return h->app.text_rect ? h->app.text_rect(h->app.user, rect) : 0; }
static uint64_t to_owner(void *user) {UiHost*h=user;return h->app.text_owner?h->app.text_owner(h->app.user):1;}
static void to_edit(void*user,const char*c,size_t n,const char*p,size_t m,int a,int b) {UiHost*h=user;if(h->app.text_edit)h->app.text_edit(h->app.user,c,n,p,m,a,b);}
static void to_cancel(void*user) {UiHost*h=user;if(h->app.text_cancel)h->app.text_cancel(h->app.user);}
static void to_paste(void*user,const char*t,size_t n) {UiHost*h=user;if(h->app.text_paste)h->app.text_paste(h->app.user,t,n);}
static int to_focus(void*user,int on) {UiHost*h=user;if(h->app.focus)h->app.focus(h->app.user,on);return 0;}
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
int ui_focused(UiHost *h) { return h->offscreen ? 0 : h->win.keyboard_focused; }
int ui_copy(UiHost *h, const char *bytes, size_t n) { return h->offscreen ? 0 : vkw_clipboard_set(&h->win, bytes, n); }

// ---- open, frame, close

UiHost *ui_open(const UiApp *app, int offscreen) {
  UiHost *h = calloc(1, sizeof *h);
  if (!h) return NULL;
  h->app = *app; h->offscreen = offscreen; h->dpr = 1; h->cursor_kind = 0;
  h->words = ui_slurp(app->shader, &h->nwords);
  if (!h->words) { fprintf(stderr, "ui-host: no shader at %s\n", app->shader); free(h); return NULL; }
  if (offscreen) vkc_open(&h->v);
  else {
    VkwTo to = {.pointer = to_pointer, .wheel = to_wheel, .user = h, .moved = to_moved, .realm_key = to_key, .text = to_text,
                .wants_text = to_wants, .text_rect = to_rect, .text_owner=to_owner,.text_edit=app->text_edit?to_edit:NULL,.text_cancel=to_cancel,.text_paste=app->text_paste?to_paste:NULL,.focus=to_focus,.text_error = to_error, .pointer_enter = to_enter, .app_id = app->app_id};
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
static void ui_drive_poll(UiHost *h);
static void ui_drive_close(UiHost *h);
int ui_frame(UiHost *h) {
#ifdef UI_ATSPI
  ui_access_bus_pump();
#endif
  ui_drive_poll(h);
  if (h->offscreen) return 0;
  vkw_pump(&h->win);
  if (h->win.closed) return 0;
  if (h->win.want_w > 0 && h->win.want_h > 0 && ((uint32_t)h->win.want_w != h->p.w || (uint32_t)h->win.want_h != h->p.h))
    vkp_resize(&h->v, &h->p, (uint32_t)h->win.want_w, (uint32_t)h->win.want_h);
  return ui_present(h);
}
void ui_close(UiHost *h) {
  if (!h) return;
#ifdef UI_ATSPI
  ui_access_destroy(h->access);h->access=NULL;
#endif
  ui_drive_close(h);
  ui_render_close(h);
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

#include "../drive.c"
