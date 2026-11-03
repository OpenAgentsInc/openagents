// A realm's window on a Wayland desktop (docs/os/2026-11-02-verse-os.md, the session's window
// before it owns the screen): an xdg-shell toplevel, windowed or full screen, with a Vulkan
// surface for vkp, the seat's keys (evdev codes, as verse/kms/evdev.h reads them, shifted the
// same way) and its pointer as host.js's pointer and wheel. vk-realm.c's --window and
// --fullscreen use it in place of vkd.h, evdev.h and mice.h: no root, no VT, no grab.
// Included once, by vk-realm.c, after realm_key and realm_key_up are declared.
#ifndef VKW_H
#define VKW_H
#include <wayland-client.h>
#include "keys.h"
#include "vkp.h"
#include "xdg-decoration-unstable-v1-client-protocol.h"
#include "xdg-shell-client-protocol.h"
#include <vulkan/vulkan_wayland.h>
#include <linux/input-event-codes.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

bool realm_key(void *state, int key);
bool realm_key_up(void *state, int key);

typedef struct {
  void (*pointer)(void *user, float x, float y, int phase);  // 0 down, 1 move while down, 2 up
  void (*wheel)(void *user, float dy);                        // CSS pixels, 100 a notch
  void *user;
  void (*moved)(void *user, float x, float y);                // any motion, where it is: the person is there
  int (*key)(int code, int pressed);                          // a key before the realm's mapping: 1 takes it
} VkwTo;

typedef struct {
  struct wl_display *display;
  struct wl_compositor *compositor;
  struct xdg_wm_base *wm;
  struct wl_seat *seat;
  struct wl_keyboard *keyboard;
  struct wl_pointer *pointer;
  struct wl_output *output;
  struct zxdg_decoration_manager_v1 *decorations;
  struct wl_surface *surface;
  struct xdg_surface *xdg;
  struct xdg_toplevel *toplevel;
  VkSurfaceKHR vk_surface;
  int configured, closed, fullscreen;
  int32_t want_w, want_h;   // the last toplevel configure, 0 to choose
  int32_t refresh_mhz;
  double period;            // the output's frame time, in seconds
  float x, y;               // the pointer, in the window's pixels
  int down;                 // the left button is held
  int shift, keys_down[KEY_MAX + 1];
  VkwTo to;
} Vkw;

static Vkw *vkw_;   // the one window the listeners serve

static void vkw_ping(void *d, struct xdg_wm_base *wm, uint32_t serial) { (void)d; xdg_wm_base_pong(wm, serial); }
static const struct xdg_wm_base_listener vkw_wm_listener = {.ping = vkw_ping};
static void vkw_xdg_configure(void *d, struct xdg_surface *x, uint32_t serial) { (void)d; xdg_surface_ack_configure(x, serial); vkw_->configured = 1; }
static const struct xdg_surface_listener vkw_xdg_listener = {.configure = vkw_xdg_configure};
static void vkw_top_configure(void *d, struct xdg_toplevel *t, int32_t w, int32_t h, struct wl_array *s) { (void)d; (void)t; (void)s; vkw_->want_w = w; vkw_->want_h = h; }
static void vkw_top_close(void *d, struct xdg_toplevel *t) { (void)d; (void)t; vkw_->closed = 1; }
static const struct xdg_toplevel_listener vkw_top_listener = {.configure = vkw_top_configure, .close = vkw_top_close};

static void vkw_geometry(void *d, struct wl_output *o, int32_t x, int32_t y, int32_t pw, int32_t ph, int32_t sub, const char *make, const char *model, int32_t tr) {
  (void)d; (void)o; (void)x; (void)y; (void)pw; (void)ph; (void)sub; (void)make; (void)model; (void)tr;
}
static void vkw_mode(void *d, struct wl_output *o, uint32_t flags, int32_t w, int32_t h, int32_t refresh) {
  (void)d; (void)o; (void)w; (void)h;
  if (flags & WL_OUTPUT_MODE_CURRENT) vkw_->refresh_mhz = refresh;
}
static void vkw_output_done(void *d, struct wl_output *o) { (void)d; (void)o; }
static void vkw_output_scale(void *d, struct wl_output *o, int32_t s) { (void)d; (void)o; (void)s; }
static const struct wl_output_listener vkw_output_listener = {.geometry = vkw_geometry, .mode = vkw_mode, .done = vkw_output_done, .scale = vkw_output_scale};

static void vkw_keymap(void *d, struct wl_keyboard *k, uint32_t f, int32_t fd, uint32_t n) { (void)d; (void)k; (void)f; (void)n; close(fd); }
static void vkw_kb_enter(void *d, struct wl_keyboard *k, uint32_t s, struct wl_surface *w, struct wl_array *keys) { (void)d; (void)k; (void)s; (void)w; (void)keys; }
// Focus gone: every key held is let go, so none sticks while the window is behind another.
static void vkw_kb_leave(void *d, struct wl_keyboard *k, uint32_t s, struct wl_surface *w) {
  (void)d; (void)k; (void)s; (void)w;
  for (int c = 0; c <= KEY_MAX; c++)
    if (vkw_->keys_down[c]) { realm_key_up(NULL, vkw_->keys_down[c]); vkw_->keys_down[c] = 0; }
  vkw_->shift = 0;
}
// evdev codes, as the seat sends them; shifted and released as evdev.h does.
static void vkw_key(void *d, struct wl_keyboard *k, uint32_t serial, uint32_t time, uint32_t code, uint32_t state) {
  (void)d; (void)k; (void)serial; (void)time;
  if (code > KEY_MAX) return;
  int pressed = state == WL_KEYBOARD_KEY_STATE_PRESSED;
  if (code == KEY_LEFTSHIFT || code == KEY_RIGHTSHIFT) vkw_->shift += pressed ? 1 : (vkw_->shift > 0 ? -1 : 0);
  if (vkw_->to.key && !vkw_->keys_down[code] && vkw_->to.key((int)code, pressed)) return;
  int r = pressed ? realm_key_shifted((int)code, vkw_->shift > 0) : vkw_->keys_down[code];
  if (!r) return;
  if (pressed) { vkw_->keys_down[code] = r; realm_key(NULL, r); }
  else { vkw_->keys_down[code] = 0; realm_key_up(NULL, r); }
}
static void vkw_mods(void *d, struct wl_keyboard *k, uint32_t s, uint32_t a, uint32_t b, uint32_t c, uint32_t g) { (void)d; (void)k; (void)s; (void)a; (void)b; (void)c; (void)g; }
static void vkw_repeat(void *d, struct wl_keyboard *k, int32_t rate, int32_t delay) { (void)d; (void)k; (void)rate; (void)delay; }
static const struct wl_keyboard_listener vkw_kb_listener = {.keymap = vkw_keymap, .enter = vkw_kb_enter, .leave = vkw_kb_leave, .key = vkw_key, .modifiers = vkw_mods, .repeat_info = vkw_repeat};

// The pointer: hidden over the realm, which draws its own.
static void vkw_pt_enter(void *d, struct wl_pointer *p, uint32_t serial, struct wl_surface *s, wl_fixed_t x, wl_fixed_t y) {
  (void)d; (void)s;
  wl_pointer_set_cursor(p, serial, NULL, 0, 0);
  vkw_->x = (float)wl_fixed_to_double(x);
  vkw_->y = (float)wl_fixed_to_double(y);
}
static void vkw_pt_leave(void *d, struct wl_pointer *p, uint32_t serial, struct wl_surface *s) {
  (void)d; (void)p; (void)serial; (void)s;
  if (vkw_->down) { vkw_->down = 0; if (vkw_->to.pointer) vkw_->to.pointer(vkw_->to.user, vkw_->x, vkw_->y, 2); }
}
static void vkw_pt_motion(void *d, struct wl_pointer *p, uint32_t time, wl_fixed_t x, wl_fixed_t y) {
  (void)d; (void)p; (void)time;
  vkw_->x = (float)wl_fixed_to_double(x);
  vkw_->y = (float)wl_fixed_to_double(y);
  if (vkw_->down && vkw_->to.pointer) vkw_->to.pointer(vkw_->to.user, vkw_->x, vkw_->y, 1);
  if (vkw_->to.moved) vkw_->to.moved(vkw_->to.user, vkw_->x, vkw_->y);
}
static void vkw_pt_button(void *d, struct wl_pointer *p, uint32_t serial, uint32_t time, uint32_t button, uint32_t state) {
  (void)d; (void)p; (void)serial; (void)time;
  if (button != BTN_LEFT) return;
  vkw_->down = state == WL_POINTER_BUTTON_STATE_PRESSED;
  if (vkw_->to.pointer) vkw_->to.pointer(vkw_->to.user, vkw_->x, vkw_->y, vkw_->down ? 0 : 2);
}
static void vkw_pt_axis(void *d, struct wl_pointer *p, uint32_t time, uint32_t axis, wl_fixed_t v) {
  (void)d; (void)p; (void)time;
  // a notch is 10 here and 100 CSS pixels in Chromium
  if (axis == WL_POINTER_AXIS_VERTICAL_SCROLL && vkw_->to.wheel) vkw_->to.wheel(vkw_->to.user, (float)wl_fixed_to_double(v) * 10);
}
static void vkw_pt_frame(void *d, struct wl_pointer *p) { (void)d; (void)p; }
static void vkw_pt_source(void *d, struct wl_pointer *p, uint32_t s) { (void)d; (void)p; (void)s; }
static void vkw_pt_stop(void *d, struct wl_pointer *p, uint32_t time, uint32_t axis) { (void)d; (void)p; (void)time; (void)axis; }
static void vkw_pt_discrete(void *d, struct wl_pointer *p, uint32_t axis, int32_t v) { (void)d; (void)p; (void)axis; (void)v; }
static const struct wl_pointer_listener vkw_pt_listener = {.enter = vkw_pt_enter, .leave = vkw_pt_leave, .motion = vkw_pt_motion, .button = vkw_pt_button,
  .axis = vkw_pt_axis, .frame = vkw_pt_frame, .axis_source = vkw_pt_source, .axis_stop = vkw_pt_stop, .axis_discrete = vkw_pt_discrete};

static void vkw_caps(void *d, struct wl_seat *seat, uint32_t caps) {
  (void)d;
  if ((caps & WL_SEAT_CAPABILITY_KEYBOARD) && !vkw_->keyboard) {
    vkw_->keyboard = wl_seat_get_keyboard(seat);
    wl_keyboard_add_listener(vkw_->keyboard, &vkw_kb_listener, NULL);
  }
  if ((caps & WL_SEAT_CAPABILITY_POINTER) && !vkw_->pointer) {
    vkw_->pointer = wl_seat_get_pointer(seat);
    wl_pointer_add_listener(vkw_->pointer, &vkw_pt_listener, NULL);
  }
}
static void vkw_seat_name(void *d, struct wl_seat *s, const char *name) { (void)d; (void)s; (void)name; }
static const struct wl_seat_listener vkw_seat_listener = {.capabilities = vkw_caps, .name = vkw_seat_name};

static void vkw_global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t version) {
  (void)d;
  if (!strcmp(iface, wl_compositor_interface.name)) vkw_->compositor = wl_registry_bind(r, name, &wl_compositor_interface, 4);
  else if (!strcmp(iface, xdg_wm_base_interface.name)) {
    vkw_->wm = wl_registry_bind(r, name, &xdg_wm_base_interface, 2);
    xdg_wm_base_add_listener(vkw_->wm, &vkw_wm_listener, NULL);
  } else if (!strcmp(iface, wl_seat_interface.name) && !vkw_->seat) {
    vkw_->seat = wl_registry_bind(r, name, &wl_seat_interface, version < 5 ? version : 5);
    wl_seat_add_listener(vkw_->seat, &vkw_seat_listener, NULL);
  } else if (!strcmp(iface, wl_output_interface.name) && !vkw_->output) {
    vkw_->output = wl_registry_bind(r, name, &wl_output_interface, 2);
    wl_output_add_listener(vkw_->output, &vkw_output_listener, NULL);
  } else if (!strcmp(iface, zxdg_decoration_manager_v1_interface.name))
    vkw_->decorations = wl_registry_bind(r, name, &zxdg_decoration_manager_v1_interface, 1);
}
static void vkw_global_remove(void *d, struct wl_registry *r, uint32_t name) { (void)d; (void)r; (void)name; }
static const struct wl_registry_listener vkw_registry_listener = {.global = vkw_global, .global_remove = vkw_global_remove};

// The window, mapped, and Vulkan opened with its surface. Exits, saying why, if it cannot.
static void vkw_open(Vkc *v, Vkw *w, const char *title, uint32_t width, uint32_t height, int fullscreen, VkwTo to) {
  memset(w, 0, sizeof *w);
  vkw_ = w;
  w->to = to;
  w->fullscreen = fullscreen;
  w->display = wl_display_connect(NULL);
  if (!w->display) { fprintf(stderr, "vk-realm: no Wayland display (WAYLAND_DISPLAY)\n"); exit(1); }
  struct wl_registry *registry = wl_display_get_registry(w->display);
  wl_registry_add_listener(registry, &vkw_registry_listener, NULL);
  wl_display_roundtrip(w->display);
  wl_display_roundtrip(w->display);
  if (!w->compositor || !w->wm) { fprintf(stderr, "vk-realm: the compositor has no xdg-shell\n"); exit(1); }
  w->surface = wl_compositor_create_surface(w->compositor);
  w->xdg = xdg_wm_base_get_xdg_surface(w->wm, w->surface);
  xdg_surface_add_listener(w->xdg, &vkw_xdg_listener, NULL);
  w->toplevel = xdg_surface_get_toplevel(w->xdg);
  xdg_toplevel_add_listener(w->toplevel, &vkw_top_listener, NULL);
  xdg_toplevel_set_title(w->toplevel, title);
  xdg_toplevel_set_app_id(w->toplevel, "com.openagents.verse");
  if (w->decorations) {
    struct zxdg_toplevel_decoration_v1 *deco = zxdg_decoration_manager_v1_get_toplevel_decoration(w->decorations, w->toplevel);
    zxdg_toplevel_decoration_v1_set_mode(deco, ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE);
  }
  if (fullscreen) xdg_toplevel_set_fullscreen(w->toplevel, NULL);
  wl_surface_commit(w->surface);
  while (!w->configured && wl_display_dispatch(w->display) >= 0) {}
  if (w->want_w > 0 && w->want_h > 0) { width = (uint32_t)w->want_w; height = (uint32_t)w->want_h; }
  w->want_w = (int32_t)width;
  w->want_h = (int32_t)height;
  w->period = w->refresh_mhz > 0 ? 1000.0 / w->refresh_mhz : 1 / 60.0;

  const char *iexts[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME};
  const char *dexts[] = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
  vkc_open_with(v, iexts, 2, dexts, 1);
  VkWaylandSurfaceCreateInfoKHR wsi = {.sType = VK_STRUCTURE_TYPE_WAYLAND_SURFACE_CREATE_INFO_KHR, .display = w->display, .surface = w->surface};
  if (vkCreateWaylandSurfaceKHR(v->inst, &wsi, NULL, &w->vk_surface) != VK_SUCCESS) { fprintf(stderr, "vk-realm: no Vulkan surface on the window\n"); exit(1); }
  VkBool32 presents = VK_FALSE;
  vkGetPhysicalDeviceSurfaceSupportKHR(v->gpu, v->family, w->vk_surface, &presents);
  if (!presents) { fprintf(stderr, "vk-realm: the queue cannot present to the window\n"); exit(1); }
}

// What the compositor has sent, without waiting for more.
static void vkw_pump(Vkw *w) {
  while (wl_display_prepare_read(w->display) != 0) wl_display_dispatch_pending(w->display);
  wl_display_flush(w->display);
  struct pollfd p = {.fd = wl_display_get_fd(w->display), .events = POLLIN};
  if (poll(&p, 1, 0) > 0) wl_display_read_events(w->display);
  else wl_display_cancel_read(w->display);
  wl_display_dispatch_pending(w->display);
}

static void vkw_close(Vkc *v, Vkw *w) {
  vkDestroySurfaceKHR(v->inst, w->vk_surface, NULL);
  vkc_close(v);
  xdg_toplevel_destroy(w->toplevel);
  xdg_surface_destroy(w->xdg);
  wl_surface_destroy(w->surface);
  wl_display_disconnect(w->display);
}
#endif
