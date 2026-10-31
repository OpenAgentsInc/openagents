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
#include "vkc.h"
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
#define MAX_FRAMES (60 * 60 * 10)
#define MAX_IMAGES 8

static double now_s(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec / 1e9;
}

static void *slurp(const char *path, size_t *size) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "vk-window: cannot read %s\n", path); exit(1); }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  void *p = malloc(n > 0 ? (size_t)n : 1);
  if (!p || fread(p, 1, (size_t)n, f) != (size_t)n) { fprintf(stderr, "vk-window: cannot read %s\n", path); exit(1); }
  fclose(f);
  *size = (size_t)n;
  return p;
}

// The uniforms named by a list such as "0,1,2,4,5", after `first` floats already in push.
static uint32_t gather(const char *list, const float *u, int nu, float *push, uint32_t first) {
  uint32_t n = first;
  if (!strcmp(list, "-")) return n;
  for (const char *p = list; *p;) {
    int i = (int)strtol(p, (char **)&p, 10);
    if (i < 0 || i >= nu || n >= 32) { fprintf(stderr, "vk-window: no uniform %d\n", i); exit(2); }
    push[n++] = u[i];
    if (*p == ',') p++;
    else if (*p) { fprintf(stderr, "vk-window: bad uniform list %s\n", list); exit(2); }
  }
  return n;
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
  double callbacks[MAX_FRAMES];
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
  if (wl.ncallbacks < MAX_FRAMES) wl.callbacks[wl.ncallbacks++] = time_ms / 1000.0;
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

// ---- the swapchain

static struct {
  VkSurfaceKHR surface;
  VkSwapchainKHR chain;
  VkFormat format;
  uint32_t w, h, count;
  VkImage images[MAX_IMAGES];
  VkSemaphore done[MAX_IMAGES];
} sc;

static void make_swapchain(Vkc *v, uint32_t w, uint32_t h) {
  VkSurfaceCapabilitiesKHR caps;
  CHECK(vkGetPhysicalDeviceSurfaceCapabilitiesKHR(v->gpu, sc.surface, &caps));
  uint32_t nf = 0;
  CHECK(vkGetPhysicalDeviceSurfaceFormatsKHR(v->gpu, sc.surface, &nf, NULL));
  VkSurfaceFormatKHR formats[64];
  if (nf > 64) nf = 64;
  CHECK(vkGetPhysicalDeviceSurfaceFormatsKHR(v->gpu, sc.surface, &nf, formats));
  // The bytes kms.c writes, as they are: an UNORM format, not sRGB.
  sc.format = formats[0].format;
  VkColorSpaceKHR space = formats[0].colorSpace;
  for (uint32_t i = 0; i < nf; i++)
    if (formats[i].format == VK_FORMAT_B8G8R8A8_UNORM || formats[i].format == VK_FORMAT_R8G8B8A8_UNORM) { sc.format = formats[i].format; space = formats[i].colorSpace; break; }
  if (caps.currentExtent.width != UINT32_MAX) { w = caps.currentExtent.width; h = caps.currentExtent.height; }
  if (w < caps.minImageExtent.width) w = caps.minImageExtent.width;
  if (h < caps.minImageExtent.height) h = caps.minImageExtent.height;
  if (caps.maxImageExtent.width && w > caps.maxImageExtent.width) w = caps.maxImageExtent.width;
  if (caps.maxImageExtent.height && h > caps.maxImageExtent.height) h = caps.maxImageExtent.height;
  uint32_t images = caps.minImageCount + 1;
  if (caps.maxImageCount && images > caps.maxImageCount) images = caps.maxImageCount;
  VkCompositeAlphaFlagBitsKHR alpha = VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR;
  if (!(caps.supportedCompositeAlpha & alpha))
    for (uint32_t b = 1; b; b <<= 1) if (caps.supportedCompositeAlpha & b) { alpha = (VkCompositeAlphaFlagBitsKHR)b; break; }
  VkSwapchainKHR old = sc.chain;
  VkSwapchainCreateInfoKHR sci = {.sType = VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR, .surface = sc.surface, .minImageCount = images,
    .imageFormat = sc.format, .imageColorSpace = space, .imageExtent = {w, h}, .imageArrayLayers = 1,
    .imageUsage = VK_IMAGE_USAGE_TRANSFER_DST_BIT, .imageSharingMode = VK_SHARING_MODE_EXCLUSIVE, .preTransform = caps.currentTransform,
    .compositeAlpha = alpha, .presentMode = VK_PRESENT_MODE_FIFO_KHR, .clipped = VK_TRUE, .oldSwapchain = old};
  CHECK(vkCreateSwapchainKHR(v->dev, &sci, NULL, &sc.chain));
  if (old) vkDestroySwapchainKHR(v->dev, old, NULL);
  for (uint32_t i = 0; i < sc.count; i++) vkDestroySemaphore(v->dev, sc.done[i], NULL);
  sc.count = MAX_IMAGES;
  CHECK(vkGetSwapchainImagesKHR(v->dev, sc.chain, &sc.count, sc.images));
  VkSemaphoreCreateInfo semi = {.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO};
  for (uint32_t i = 0; i < sc.count; i++) CHECK(vkCreateSemaphore(v->dev, &semi, NULL, &sc.done[i]));
  sc.w = w;
  sc.h = h;
}

static void barrier(VkCommandBuffer cmd, VkImage image, VkImageLayout from, VkImageLayout to, VkAccessFlags src, VkAccessFlags dst,
                    VkPipelineStageFlags src_stage, VkPipelineStageFlags dst_stage) {
  VkImageMemoryBarrier b = {.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER, .srcAccessMask = src, .dstAccessMask = dst, .oldLayout = from, .newLayout = to,
    .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .image = image,
    .subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1}};
  vkCmdPipelineBarrier(cmd, src_stage, dst_stage, 0, 0, NULL, 0, NULL, 1, &b);
}

// Intervals past one and a half periods, and the longest, after the first
// `skip`; each drop is named by when it came (seconds after t[0]).
// Where each frame's time went: the fence, the realm's step, the acquire.
static double waited[MAX_FRAMES], stepped[MAX_FRAMES], acquiring[MAX_FRAMES];
static int drops(const char *what, const double *t, int n, int skip, double period, double *longest, int detail) {
  int d = 0;
  *longest = 0;
  for (int i = skip + 1; i < n; i++) {
    double gap = t[i] - t[i - 1];
    if (gap > *longest) *longest = gap;
    if (gap > period * 1.5 && d++ < 10) {
      printf("vk-window: %s drop at %.3f s: %.1f ms", what, t[i] - t[0], gap * 1000);
      if (detail) printf(" (fence %.1f, realm %.1f, acquire %.1f ms)", waited[i] * 1000, stepped[i] * 1000, acquiring[i] * 1000);
      printf("\n");
    }
  }
  return d;
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
  CHECK(vkCreateWaylandSurfaceKHR(v.inst, &wsi, NULL, &sc.surface));
  VkBool32 presents = VK_FALSE;
  CHECK(vkGetPhysicalDeviceSurfaceSupportKHR(v.gpu, v.family, sc.surface, &presents));
  if (!presents) { fprintf(stderr, "vk-window: the queue cannot present to this surface\n"); return 1; }
  make_swapchain(&v, win_w, win_h);

  // The realm and its passes.
  wl.state = calloc(1, realm_state_size());
  int vf = realm_vertex_floats(), nu = realm_uniform_floats(), maxv = realm_max_vertices();
  float *vertices = calloc((size_t)maxv * vf, sizeof(float));
  float *u = calloc((size_t)nu, sizeof(float));
  size_t fill_n, stroke_n;
  uint32_t *fill = slurp(argv[1], &fill_n), *stroke = slurp(argv[3], &stroke_n);
  float fill_push[32], stroke_push[32];
  VkcBuffer vb = vkc_buffer(&v, (VkDeviceSize)maxv * vf * 4);
  VkcPass passes[2] = {
    {.words = fill, .count = fill_n / 4, .push = fill_push, .push_size = gather(argv[2], u, nu, fill_push, 0) * 4, .vertex_count = 3},
    {.words = stroke, .count = stroke_n / 4, .push = stroke_push, .push_size = gather(argv[4], u, nu, stroke_push, 2) * 4,
     .vertices = &vb, .stride = (uint32_t)vf * 4, .additive = 1}};
  VkcTarget target;
  vkc_target_open(&v, &target, sc.w, sc.h, passes, 2);

  VkCommandPoolCreateInfo cpci = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT, .queueFamilyIndex = v.family};
  VkCommandPool pool;
  CHECK(vkCreateCommandPool(v.dev, &cpci, NULL, &pool));
  VkCommandBufferAllocateInfo cbai = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO, .commandPool = pool, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1};
  VkCommandBuffer cmd;
  CHECK(vkAllocateCommandBuffers(v.dev, &cbai, &cmd));
  VkFenceCreateInfo fci = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO, .flags = VK_FENCE_CREATE_SIGNALED_BIT};
  VkFence fence;
  CHECK(vkCreateFence(v.dev, &fci, NULL, &fence));
  VkSemaphoreCreateInfo semi = {.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO};
  VkSemaphore acquired;
  CHECK(vkCreateSemaphore(v.dev, &semi, NULL, &acquired));

  static double presented[MAX_FRAMES];
  int frames = 0, resizes = 0;
  const char *why = "its time was up";
  double start = now_s(), last = start;
  while (!wl.closed && now_s() - start < seconds && frames < MAX_FRAMES) {
    pump();
    // A new size from the compositor: a new swapchain and target.
    uint32_t want_w = wl.want_w > 0 ? (uint32_t)wl.want_w : sc.w, want_h = wl.want_h > 0 ? (uint32_t)wl.want_h : sc.h;
    if (want_w != sc.w || want_h != sc.h) {
      CHECK(vkDeviceWaitIdle(v.dev));
      make_swapchain(&v, want_w, want_h);
      vkc_target_close(&v, &target);
      vkc_target_open(&v, &target, sc.w, sc.h, passes, 2);
      resizes++;
    }
    double t0 = now_s();
    CHECK(vkWaitForFences(v.dev, 1, &fence, VK_TRUE, UINT64_MAX));

    double t = now_s(), dt = t - last;
    last = t;
    if (dt > 0.1) dt = 0.1;
    int count = realm_frame(wl.state, vertices, u, (float)sc.w, (float)sc.h, scale, (float)dt);
    double t1 = now_s();
    if (realm_action(wl.state)) { why = "the realm navigated away"; break; }
    memcpy(vb.map, vertices, (size_t)count * vf * 4);
    gather(argv[2], u, nu, fill_push, 0);
    stroke_push[0] = (float)sc.w;
    stroke_push[1] = (float)sc.h;
    gather(argv[4], u, nu, stroke_push, 2);
    passes[1].vertex_count = (uint32_t)(count - count % 3);

    uint32_t image;
    VkResult got = vkAcquireNextImageKHR(v.dev, sc.chain, UINT64_MAX, acquired, VK_NULL_HANDLE, &image);
    waited[frames] = t - t0;
    stepped[frames] = t1 - t;
    acquiring[frames] = now_s() - t1;
    if (got == VK_ERROR_OUT_OF_DATE_KHR) { wl.want_w = 0; wl.want_h = 0; CHECK(vkDeviceWaitIdle(v.dev)); make_swapchain(&v, sc.w, sc.h); continue; }
    if (got != VK_SUCCESS && got != VK_SUBOPTIMAL_KHR) CHECK(got);
    CHECK(vkResetFences(v.dev, 1, &fence));

    CHECK(vkResetCommandBuffer(cmd, 0));
    VkCommandBufferBeginInfo begin = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO, .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT};
    CHECK(vkBeginCommandBuffer(cmd, &begin));
    vkc_target_record(&target, cmd, passes);
    barrier(cmd, sc.images[image], VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 0, VK_ACCESS_TRANSFER_WRITE_BIT,
            VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT);
    VkImageBlit blit = {.srcSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .srcOffsets = {{0, 0, 0}, {(int32_t)target.w, (int32_t)target.h, 1}},
                        .dstSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .dstOffsets = {{0, 0, 0}, {(int32_t)sc.w, (int32_t)sc.h, 1}}};
    vkCmdBlitImage(cmd, target.image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, sc.images[image], VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &blit, VK_FILTER_NEAREST);
    barrier(cmd, sc.images[image], VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, VK_ACCESS_TRANSFER_WRITE_BIT, 0,
            VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT);
    CHECK(vkEndCommandBuffer(cmd));
    VkPipelineStageFlags wait = VK_PIPELINE_STAGE_TRANSFER_BIT;
    VkSubmitInfo si = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .waitSemaphoreCount = 1, .pWaitSemaphores = &acquired, .pWaitDstStageMask = &wait,
                       .commandBufferCount = 1, .pCommandBuffers = &cmd, .signalSemaphoreCount = 1, .pSignalSemaphores = &sc.done[image]};
    CHECK(vkQueueSubmit(v.queue, 1, &si, fence));

    // The compositor's frame callback rides on the commit the present makes.
    struct wl_callback *cb = wl_surface_frame(wl.surface);
    wl_callback_add_listener(cb, &frame_listener, NULL);
    VkPresentInfoKHR pi = {.sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR, .waitSemaphoreCount = 1, .pWaitSemaphores = &sc.done[image],
                           .swapchainCount = 1, .pSwapchains = &sc.chain, .pImageIndices = &image};
    VkResult shown = vkQueuePresentKHR(v.queue, &pi);
    if (shown != VK_SUCCESS && shown != VK_SUBOPTIMAL_KHR && shown != VK_ERROR_OUT_OF_DATE_KHR) CHECK(shown);
    presented[frames++] = now_s();
  }
  if (wl.closed) why = "the compositor closed it";
  CHECK(vkDeviceWaitIdle(v.dev));
  pump();
  double ran = now_s() - start;

  // The first second settles (the first configure and the swapchain's fill).
  int skip = (int)(1 / period);
  double longest_present, longest_callback;
  int dropped_present = drops("present", presented, frames, skip, period, &longest_present, 1);
  int dropped_callback = drops("callback", wl.callbacks, wl.ncallbacks, skip, period, &longest_callback, 0);
  printf("vk-window: %s, %s %ux%u %s, %.3f Hz display, %d frames in %.1f s (%.2f a second), %d resizes; it ended because %s\n", v.props.deviceName, realm_name(),
         sc.w, sc.h, fullscreen ? "full screen" : "windowed", 1 / period, frames, ran, frames / ran, resizes, why);
  printf("vk-window: dropped frames: %d where presented (longest %.1f ms), %d of %d frame callbacks (longest %.1f ms)\n",
         dropped_present, longest_present * 1000, dropped_callback, wl.ncallbacks, longest_callback * 1000);
  if (v.validating) printf("vk-window: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
  else printf("vk-window: validation layers not loaded\n");

  vkDestroySemaphore(v.dev, acquired, NULL);
  vkDestroyFence(v.dev, fence, NULL);
  vkDestroyCommandPool(v.dev, pool, NULL);
  vkc_target_close(&v, &target);
  vkc_free(&v, &vb);
  for (uint32_t i = 0; i < sc.count; i++) vkDestroySemaphore(v.dev, sc.done[i], NULL);
  vkDestroySwapchainKHR(v.dev, sc.chain, NULL);
  vkDestroySurfaceKHR(v.inst, sc.surface, NULL);
  vkc_close(&v);
  xdg_toplevel_destroy(wl.toplevel);
  xdg_surface_destroy(wl.xdg);
  wl_surface_destroy(wl.surface);
  wl_display_disconnect(wl.display);
  return dropped_present == 0 && dropped_callback == 0 && vkc_validation_errors == 0 ? 0 : 1;
}
