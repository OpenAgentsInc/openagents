// vk-window: a realm in a Vulkan window on Wayland, the desktop app's path
// (docs/verse/native-gpu.md, G2). The realm steps on wall time; each frame
// it draws the offscreen pipeline (vkc_target: `coax spirv --fill`, then
// `--stroke` added one plus one, into RGBA f32) and blits it to the
// swapchain image, presented in FIFO at the display's rate. Keys come from
// the seat's keyboard (evdev codes, as verse/kms reads them); the pointer
// is hidden over the window. It reports the frame intervals at the end: a
// drop is an interval past one and a half refresh periods, counted both
// where the host presents and where the compositor's frame callbacks fire.
//
//   vk-window FILL.spv FILL_U STROKE.spv STROKE_U [--fullscreen] [--seconds N] [--scale K] [--size WxH]
// FILL_U and STROKE_U are as for vk-frame. The window closes after N
// seconds (default 60), when the compositor closes it, or when the realm
// navigates away. Validation: see vkc.h.
#define _POSIX_C_SOURCE 200809L
#include <wayland-client.h>
#include "keys.h"
#include "realm.h"
#include "vkp.h"
#include "vkr.h"
#include "xdg-decoration-unstable-v1-client-protocol.h"
#include "xdg-shell-client-protocol.h"
#include <vulkan/vulkan_wayland.h>
#include <math.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vk-window: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

static double now_s(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec / 1e9;
}

// ---- Wayland

static struct {
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
  int configured, closed;
  int32_t want_w, want_h;  // from the last toplevel configure, 0 to choose
  int32_t refresh_mhz;
  void *state;             // the realm
  double callbacks[VKR_FRAMES];
  int ncallbacks;
} wl;

static void on_ping(void *data, struct xdg_wm_base *wm, uint32_t serial) { (void)data; xdg_wm_base_pong(wm, serial); }
static const struct xdg_wm_base_listener wm_listener = {.ping = on_ping};

static void on_xdg_configure(void *data, struct xdg_surface *xdg, uint32_t serial) {
  (void)data;
  xdg_surface_ack_configure(xdg, serial);
  wl.configured = 1;
}
static const struct xdg_surface_listener xdg_listener = {.configure = on_xdg_configure};

static void on_toplevel_configure(void *data, struct xdg_toplevel *t, int32_t w, int32_t h, struct wl_array *states) {
  (void)data; (void)t; (void)states;
  wl.want_w = w;
  wl.want_h = h;
}
static void on_toplevel_close(void *data, struct xdg_toplevel *t) { (void)data; (void)t; wl.closed = 1; }
static const struct xdg_toplevel_listener toplevel_listener = {.configure = on_toplevel_configure, .close = on_toplevel_close};

static void on_geometry(void *d, struct wl_output *o, int32_t x, int32_t y, int32_t pw, int32_t ph, int32_t sub, const char *make, const char *model, int32_t tr) {
  (void)d; (void)o; (void)x; (void)y; (void)pw; (void)ph; (void)sub; (void)make; (void)model; (void)tr;
}
static void on_mode(void *d, struct wl_output *o, uint32_t flags, int32_t w, int32_t h, int32_t refresh) {
  (void)d; (void)o; (void)w; (void)h;
  if (flags & WL_OUTPUT_MODE_CURRENT) wl.refresh_mhz = refresh;
}
static void on_output_done(void *d, struct wl_output *o) { (void)d; (void)o; }
static void on_output_scale(void *d, struct wl_output *o, int32_t s) { (void)d; (void)o; (void)s; }
static const struct wl_output_listener output_listener = {.geometry = on_geometry, .mode = on_mode, .done = on_output_done, .scale = on_output_scale};

static void on_keymap(void *d, struct wl_keyboard *k, uint32_t format, int32_t fd, uint32_t size) { (void)d; (void)k; (void)format; (void)size; close(fd); }
static void on_kb_enter(void *d, struct wl_keyboard *k, uint32_t serial, struct wl_surface *s, struct wl_array *keys) { (void)d; (void)k; (void)serial; (void)s; (void)keys; }
static void on_kb_leave(void *d, struct wl_keyboard *k, uint32_t serial, struct wl_surface *s) { (void)d; (void)k; (void)serial; (void)s; }
// The seat sends evdev codes, which realm_key_of names.
static void on_key(void *d, struct wl_keyboard *k, uint32_t serial, uint32_t time, uint32_t key, uint32_t pressed) {
  (void)d; (void)k; (void)serial; (void)time;
  int r = realm_key_of((int)key);
  if (!r) return;
  if (pressed == WL_KEYBOARD_KEY_STATE_PRESSED) realm_key(wl.state, r);
  else realm_key_up(wl.state, r);
}
static void on_modifiers(void *d, struct wl_keyboard *k, uint32_t serial, uint32_t a, uint32_t b, uint32_t c, uint32_t g) { (void)d; (void)k; (void)serial; (void)a; (void)b; (void)c; (void)g; }
static void on_repeat(void *d, struct wl_keyboard *k, int32_t rate, int32_t delay) { (void)d; (void)k; (void)rate; (void)delay; }
static const struct wl_keyboard_listener keyboard_listener = {.keymap = on_keymap, .enter = on_kb_enter, .leave = on_kb_leave, .key = on_key, .modifiers = on_modifiers, .repeat_info = on_repeat};

// No cursor over the game.
static void on_pt_enter(void *d, struct wl_pointer *p, uint32_t serial, struct wl_surface *s, wl_fixed_t x, wl_fixed_t y) { (void)d; (void)s; (void)x; (void)y; wl_pointer_set_cursor(p, serial, NULL, 0, 0); }
static void on_pt_leave(void *d, struct wl_pointer *p, uint32_t serial, struct wl_surface *s) { (void)d; (void)p; (void)serial; (void)s; }
static void on_pt_motion(void *d, struct wl_pointer *p, uint32_t time, wl_fixed_t x, wl_fixed_t y) { (void)d; (void)p; (void)time; (void)x; (void)y; }
static void on_pt_button(void *d, struct wl_pointer *p, uint32_t serial, uint32_t time, uint32_t button, uint32_t state) { (void)d; (void)p; (void)serial; (void)time; (void)button; (void)state; }
static void on_pt_axis(void *d, struct wl_pointer *p, uint32_t time, uint32_t axis, wl_fixed_t v) { (void)d; (void)p; (void)time; (void)axis; (void)v; }
static void on_pt_frame(void *d, struct wl_pointer *p) { (void)d; (void)p; }
static void on_pt_source(void *d, struct wl_pointer *p, uint32_t s) { (void)d; (void)p; (void)s; }
static void on_pt_stop(void *d, struct wl_pointer *p, uint32_t time, uint32_t axis) { (void)d; (void)p; (void)time; (void)axis; }
static void on_pt_discrete(void *d, struct wl_pointer *p, uint32_t axis, int32_t v) { (void)d; (void)p; (void)axis; (void)v; }
static const struct wl_pointer_listener pointer_listener = {.enter = on_pt_enter, .leave = on_pt_leave, .motion = on_pt_motion, .button = on_pt_button,
  .axis = on_pt_axis, .frame = on_pt_frame, .axis_source = on_pt_source, .axis_stop = on_pt_stop, .axis_discrete = on_pt_discrete};

static void on_capabilities(void *d, struct wl_seat *seat, uint32_t caps) {
  (void)d;
  if ((caps & WL_SEAT_CAPABILITY_KEYBOARD) && !wl.keyboard) {
    wl.keyboard = wl_seat_get_keyboard(seat);
    wl_keyboard_add_listener(wl.keyboard, &keyboard_listener, NULL);
  }
  if ((caps & WL_SEAT_CAPABILITY_POINTER) && !wl.pointer) {
    wl.pointer = wl_seat_get_pointer(seat);
    wl_pointer_add_listener(wl.pointer, &pointer_listener, NULL);
  }
}
static void on_seat_name(void *d, struct wl_seat *s, const char *name) { (void)d; (void)s; (void)name; }
static const struct wl_seat_listener seat_listener = {.capabilities = on_capabilities, .name = on_seat_name};

static void on_global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t version) {
  (void)d;
  if (!strcmp(iface, wl_compositor_interface.name)) wl.compositor = wl_registry_bind(r, name, &wl_compositor_interface, 4);
  else if (!strcmp(iface, xdg_wm_base_interface.name)) {
    wl.wm = wl_registry_bind(r, name, &xdg_wm_base_interface, 2);
    xdg_wm_base_add_listener(wl.wm, &wm_listener, NULL);
  } else if (!strcmp(iface, wl_seat_interface.name) && !wl.seat) {
    wl.seat = wl_registry_bind(r, name, &wl_seat_interface, version < 5 ? version : 5);
    wl_seat_add_listener(wl.seat, &seat_listener, NULL);
  } else if (!strcmp(iface, wl_output_interface.name) && !wl.output) {
    wl.output = wl_registry_bind(r, name, &wl_output_interface, 2);
    wl_output_add_listener(wl.output, &output_listener, NULL);
  } else if (!strcmp(iface, zxdg_decoration_manager_v1_interface.name))
    wl.decorations = wl_registry_bind(r, name, &zxdg_decoration_manager_v1_interface, 1);
}
static void on_global_remove(void *d, struct wl_registry *r, uint32_t name) { (void)d; (void)r; (void)name; }
static const struct wl_registry_listener registry_listener = {.global = on_global, .global_remove = on_global_remove};

static void on_frame_done(void *d, struct wl_callback *cb, uint32_t time_ms) {
  (void)d;
  if (wl.ncallbacks < VKR_FRAMES) wl.callbacks[wl.ncallbacks++] = time_ms / 1000.0;
  wl_callback_destroy(cb);
}
static const struct wl_callback_listener frame_listener = {.done = on_frame_done};

// What the compositor has sent, without waiting for more.
static void pump(void) {
  while (wl_display_prepare_read(wl.display) != 0) wl_display_dispatch_pending(wl.display);
  wl_display_flush(wl.display);
  struct pollfd p = {.fd = wl_display_get_fd(wl.display), .events = POLLIN};
  if (poll(&p, 1, 0) > 0) wl_display_read_events(wl.display);
  else wl_display_cancel_read(wl.display);
  wl_display_dispatch_pending(wl.display);
}

static void frame_callback(void *arg) {
  (void)arg;
  struct wl_callback *cb = wl_surface_frame(wl.surface);
  wl_callback_add_listener(cb, &frame_listener, NULL);
}

int main(int argc, char **argv) {
  if (argc < 5) { fprintf(stderr, "usage: vk-window FILL.spv FILL_U STROKE.spv STROKE_U [--fullscreen] [--seconds N] [--scale K] [--size WxH]\n"); return 2; }
  int fullscreen = 0;
  double seconds = 60;
  float scale = 2;
  uint32_t win_w = 1280, win_h = 720;
  for (int i = 5; i < argc; i++) {
    if (!strcmp(argv[i], "--fullscreen")) fullscreen = 1;
    else if (!strcmp(argv[i], "--seconds") && i + 1 < argc) seconds = atof(argv[++i]);
    else if (!strcmp(argv[i], "--scale") && i + 1 < argc) scale = (float)atof(argv[++i]);
    else if (!strcmp(argv[i], "--size") && i + 1 < argc && sscanf(argv[i + 1], "%ux%u", &win_w, &win_h) == 2) i++;
    else { fprintf(stderr, "vk-window: unknown argument %s\n", argv[i]); return 2; }
  }

  // The window.
  wl.display = wl_display_connect(NULL);
  if (!wl.display) { fprintf(stderr, "vk-window: no Wayland display (WAYLAND_DISPLAY)\n"); return 1; }
  struct wl_registry *registry = wl_display_get_registry(wl.display);
  wl_registry_add_listener(registry, &registry_listener, NULL);
  wl_display_roundtrip(wl.display);
  wl_display_roundtrip(wl.display);
  if (!wl.compositor || !wl.wm) { fprintf(stderr, "vk-window: the compositor has no xdg-shell\n"); return 1; }
  wl.surface = wl_compositor_create_surface(wl.compositor);
  wl.xdg = xdg_wm_base_get_xdg_surface(wl.wm, wl.surface);
  xdg_surface_add_listener(wl.xdg, &xdg_listener, NULL);
  wl.toplevel = xdg_surface_get_toplevel(wl.xdg);
  xdg_toplevel_add_listener(wl.toplevel, &toplevel_listener, NULL);
  xdg_toplevel_set_title(wl.toplevel, "Verse");
  xdg_toplevel_set_app_id(wl.toplevel, "com.openagents.verse");
  if (wl.decorations) {
    struct zxdg_toplevel_decoration_v1 *deco = zxdg_decoration_manager_v1_get_toplevel_decoration(wl.decorations, wl.toplevel);
    zxdg_toplevel_decoration_v1_set_mode(deco, ZXDG_TOPLEVEL_DECORATION_V1_MODE_SERVER_SIDE);
  }
  if (fullscreen) xdg_toplevel_set_fullscreen(wl.toplevel, NULL);
  wl_surface_commit(wl.surface);
  while (!wl.configured && wl_display_dispatch(wl.display) >= 0) {}
  if (wl.want_w > 0 && wl.want_h > 0) { win_w = (uint32_t)wl.want_w; win_h = (uint32_t)wl.want_h; }
  double period = wl.refresh_mhz > 0 ? 1000.0 / wl.refresh_mhz : 1 / 60.0;

  // Vulkan on it.
  const char *iexts[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME};
  const char *dexts[] = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
  Vkc v;
  vkc_open_with(&v, iexts, 2, dexts, 1);
  VkWaylandSurfaceCreateInfoKHR wsi = {.sType = VK_STRUCTURE_TYPE_WAYLAND_SURFACE_CREATE_INFO_KHR, .display = wl.display, .surface = wl.surface};
  VkSurfaceKHR surface;
  CHECK(vkCreateWaylandSurfaceKHR(v.inst, &wsi, NULL, &surface));
  VkBool32 presents = VK_FALSE;
  CHECK(vkGetPhysicalDeviceSurfaceSupportKHR(v.gpu, v.family, surface, &presents));
  if (!presents) { fprintf(stderr, "vk-window: the queue cannot present to this surface\n"); return 1; }
  static Vkp p;
  vkp_open(&v, &p, surface, win_w, win_h);

  // The realm and its passes.
  static Vkr r;
  vkr_open(&v, &r, argv[1], argv[2], argv[3], argv[4]);
  wl.state = r.state;
  VkcTarget target;
  vkc_target_open(&v, &target, p.w, p.h, r.passes, 2);

  int resizes = 0;
  const char *why = "its time was up";
  double start = now_s(), last = start;
  while (!wl.closed && now_s() - start < seconds && r.frames < VKR_FRAMES) {
    pump();
    // A new size from the compositor: a new swapchain and target.
    uint32_t want_w = wl.want_w > 0 ? (uint32_t)wl.want_w : p.w, want_h = wl.want_h > 0 ? (uint32_t)wl.want_h : p.h;
    if (want_w != p.w || want_h != p.h) {
      vkp_resize(&v, &p, want_w, want_h);
      vkc_target_close(&v, &target);
      vkc_target_open(&v, &target, p.w, p.h, r.passes, 2);
      resizes++;
    }
    vkp_wait(&v, &p);
    double t = now_s(), dt = t - last;
    last = t;
    if (dt > 0.1) dt = 0.1;
    vkr_step(&r, p.w, p.h, scale, (float)dt);
    double t1 = now_s();
    if (realm_action(r.state)) { why = "the realm navigated away"; break; }
    if (vkp_frame(&v, &p, &target, r.passes, frame_callback, NULL)) { wl.want_w = 0; wl.want_h = 0; vkp_resize(&v, &p, p.w, p.h); continue; }
    r.waited[r.frames] = p.waited;
    r.stepped[r.frames] = t1 - t;
    r.acquiring[r.frames] = p.acquiring;
    r.presented[r.frames++] = now_s();
  }
  if (wl.closed) why = "the compositor closed it";
  CHECK(vkDeviceWaitIdle(v.dev));
  pump();
  double ran = now_s() - start;

  // The first second settles (the first configure and the swapchain's fill).
  int skip = (int)(1 / period);
  double longest_present, longest_callback;
  int dropped_present = vkr_drops("vk-window", "present", &r, r.presented, r.frames, skip, period, &longest_present, 1);
  int dropped_callback = vkr_drops("vk-window", "callback", &r, wl.callbacks, wl.ncallbacks, skip, period, &longest_callback, 0);
  printf("vk-window: %s, %s %ux%u %s, %.3f Hz display, %d frames in %.1f s (%.2f a second), %d resizes; it ended because %s\n", v.props.deviceName, realm_name(),
         p.w, p.h, fullscreen ? "full screen" : "windowed", 1 / period, r.frames, ran, r.frames / ran, resizes, why);
  printf("vk-window: dropped frames: %d where presented (longest %.1f ms), %d of %d frame callbacks (longest %.1f ms)\n",
         dropped_present, longest_present * 1000, dropped_callback, wl.ncallbacks, longest_callback * 1000);
  if (v.validating) printf("vk-window: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
  else printf("vk-window: validation layers not loaded\n");

  vkp_close(&v, &p);
  vkc_target_close(&v, &target);
  vkr_close(&v, &r);
  vkDestroySurfaceKHR(v.inst, surface, NULL);
  vkc_close(&v);
  xdg_toplevel_destroy(wl.toplevel);
  xdg_surface_destroy(wl.xdg);
  wl_surface_destroy(wl.surface);
  wl_display_disconnect(wl.display);
  return dropped_present == 0 && dropped_callback == 0 && vkc_validation_errors == 0 ? 0 : 1;
}
