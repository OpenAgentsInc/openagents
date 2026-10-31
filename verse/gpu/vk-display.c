// vk-display: a realm straight on the screen with Vulkan, no desktop
// (docs/verse/native-gpu.md, G2: the boot session). It takes DRM master on
// the card, hands the connected connector to Vulkan (VK_EXT_acquire_drm_display),
// and presents through VK_KHR_display in FIFO at the mode's rate: the same
// offscreen pipeline as vk-window (vkr.h, vkp.h). Keys come from evdev,
// grabbed (verse/kms/evdev.h). Escape, the --seconds limit, SIGINT or
// SIGTERM end it, and it lets go of the display; display.sh switched to a
// free VT first and switches back.
//
//   vk-display FILL.spv FILL_U STROKE.spv STROKE_U [--seconds N] [--scale K] [--demo]
//              [--shot OUT.ppm] [--shot-at S]
// --shot copies the image presented at S seconds (default 4) out of the
// swapchain into OUT.ppm (written after the run). DRM cannot say what is on
// the screen meanwhile: the NVIDIA driver flips a leased display through its
// own modesetting, so the planes still name the desktop's last framebuffer.
// Needs root (DRM master, /dev/input).
#define _GNU_SOURCE
#include "evdev.h"
#include "realm.h"
#include "vkp.h"
#include "vkr.h"
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vk-display: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

static volatile sig_atomic_t stopping;
static void on_signal(int s) { (void)s; stopping = 1; }

static double now_s(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec / 1e9;
}

// The card with a connected connector, as DRM master; its connector and CRTC.
static int open_card(uint32_t *connector, uint32_t *crtc, drmModeModeInfo *mode) {
  for (int i = 0; i < 8; i++) {
    char path[32];
    snprintf(path, sizeof path, "/dev/dri/card%d", i);
    int fd = open(path, O_RDWR | O_CLOEXEC);
    if (fd < 0) continue;
    drmModeRes *r = drmModeGetResources(fd);
    for (int c = 0; r && c < r->count_connectors; c++) {
      drmModeConnector *k = drmModeGetConnector(fd, r->connectors[c]);
      if (k && k->connection == DRM_MODE_CONNECTED && k->count_modes > 0) {
        *connector = k->connector_id;
        *mode = k->modes[0];
        for (int m = 0; m < k->count_modes; m++) if (k->modes[m].type & DRM_MODE_TYPE_PREFERRED) { *mode = k->modes[m]; break; }
        drmModeEncoder *e = k->encoder_id ? drmModeGetEncoder(fd, k->encoder_id) : NULL;
        *crtc = e ? e->crtc_id : 0;
        if (e) drmModeFreeEncoder(e);
        drmModeFreeConnector(k);
        drmModeFreeResources(r);
        // The paused session's logind lets go of master a moment after the VT switch.
        for (int t = 0; t < 50 && drmSetMaster(fd) < 0 && errno == EBUSY; t++) usleep(100000);
        if (drmSetMaster(fd) < 0) {
          fprintf(stderr, "vk-display: another program holds the display (%s); switch to a free VT first (display.sh does)\n", strerror(errno));
          exit(1);
        }
        return fd;
      }
      if (k) drmModeFreeConnector(k);
    }
    if (r) drmModeFreeResources(r);
    close(fd);
  }
  fprintf(stderr, "vk-display: no card with a connected screen\n");
  exit(1);
}

static void save_ppm(const char *path, const Vkp *p) {
  FILE *f = fopen(path, "wb");
  if (!f) { fprintf(stderr, "vk-display: cannot write %s\n", path); return; }
  fprintf(f, "P6\n%u %u\n255\n", p->w, p->h);
  const unsigned char *px = p->shot.map;
  int bgr = p->format.format == VK_FORMAT_B8G8R8A8_UNORM || p->format.format == VK_FORMAT_B8G8R8A8_SRGB;
  for (size_t i = 0; i < (size_t)p->w * p->h; i++) {
    unsigned char c[3] = {px[i * 4 + (bgr ? 2 : 0)], px[i * 4 + 1], px[i * 4 + (bgr ? 0 : 2)]};
    fwrite(c, 1, 3, f);
  }
  fclose(f);
}

// A demo player for a capture nobody plays: the realm names the key (realm_demo_key).
static void demo_keys(void *state, double t) {
  static int held;
  static double last_space = -10;
  int want = realm_demo_key(state);
  if (want == ' ') {
    if (held) { realm_key_up(state, held); held = 0; }
    if (t - last_space > 2.5) { realm_key(state, ' '); realm_key_up(state, ' '); last_space = t; }
    return;
  }
  if (want == held) return;
  if (held) realm_key_up(state, held);
  if (want) realm_key(state, want);
  held = want;
}

int main(int argc, char **argv) {
  if (argc < 5) { fprintf(stderr, "usage: vk-display FILL.spv FILL_U STROKE.spv STROKE_U [--seconds N] [--scale K] [--demo] [--shot OUT.ppm] [--shot-at S]\n"); return 2; }
  double seconds = 60, shot_at = 4;
  float scale = 2;
  int demo = 0;
  const char *shot = NULL;
  for (int i = 5; i < argc; i++) {
    if (!strcmp(argv[i], "--seconds") && i + 1 < argc) seconds = atof(argv[++i]);
    else if (!strcmp(argv[i], "--scale") && i + 1 < argc) scale = (float)atof(argv[++i]);
    else if (!strcmp(argv[i], "--demo")) demo = 1;
    else if (!strcmp(argv[i], "--shot") && i + 1 < argc) shot = argv[++i];
    else if (!strcmp(argv[i], "--shot-at") && i + 1 < argc) shot_at = atof(argv[++i]);
    else { fprintf(stderr, "vk-display: unknown argument %s\n", argv[i]); return 2; }
  }
  signal(SIGINT, on_signal);
  signal(SIGTERM, on_signal);

  uint32_t connector = 0, crtc = 0;
  drmModeModeInfo mode;
  int fd = open_card(&connector, &crtc, &mode);
  (void)crtc;

  // Vulkan with the connector handed to it.
  const char *iexts[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_DISPLAY_EXTENSION_NAME, VK_EXT_DIRECT_MODE_DISPLAY_EXTENSION_NAME,
                         VK_EXT_ACQUIRE_DRM_DISPLAY_EXTENSION_NAME};
  const char *dexts[] = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
  Vkc v;
  vkc_open_with(&v, iexts, 4, dexts, 1);
  PFN_vkGetDrmDisplayEXT get_display = (PFN_vkGetDrmDisplayEXT)vkGetInstanceProcAddr(v.inst, "vkGetDrmDisplayEXT");
  PFN_vkAcquireDrmDisplayEXT acquire_display = (PFN_vkAcquireDrmDisplayEXT)vkGetInstanceProcAddr(v.inst, "vkAcquireDrmDisplayEXT");
  PFN_vkReleaseDisplayEXT release_display = (PFN_vkReleaseDisplayEXT)vkGetInstanceProcAddr(v.inst, "vkReleaseDisplayEXT");
  if (!get_display || !acquire_display || !release_display) { fprintf(stderr, "vk-display: the driver has no DRM display acquire\n"); return 1; }
  VkDisplayKHR display;
  CHECK(get_display(v.gpu, fd, connector, &display));
  CHECK(acquire_display(v.gpu, fd, display));

  // The mode the connector prefers, and a plane that shows on it.
  uint32_t nm = 0;
  CHECK(vkGetDisplayModePropertiesKHR(v.gpu, display, &nm, NULL));
  VkDisplayModePropertiesKHR modes[64];
  if (nm > 64) nm = 64;
  CHECK(vkGetDisplayModePropertiesKHR(v.gpu, display, &nm, modes));
  int pick = -1;
  for (uint32_t i = 0; i < nm; i++) {
    VkDisplayModeParametersKHR m = modes[i].parameters;
    if (m.visibleRegion.width != mode.hdisplay || m.visibleRegion.height != mode.vdisplay) continue;
    if (pick < 0 || abs((int)m.refreshRate - (int)mode.vrefresh * 1000) < abs((int)modes[pick].parameters.refreshRate - (int)mode.vrefresh * 1000)) pick = (int)i;
  }
  if (pick < 0) { fprintf(stderr, "vk-display: Vulkan has no %ux%u mode\n", mode.hdisplay, mode.vdisplay); return 1; }
  uint32_t np = 0;
  CHECK(vkGetPhysicalDeviceDisplayPlanePropertiesKHR(v.gpu, &np, NULL));
  VkDisplayPlanePropertiesKHR planes[32];
  if (np > 32) np = 32;
  CHECK(vkGetPhysicalDeviceDisplayPlanePropertiesKHR(v.gpu, &np, planes));
  int plane = -1;
  for (uint32_t i = 0; i < np && plane < 0; i++) {
    if (planes[i].currentDisplay && planes[i].currentDisplay != display) continue;
    uint32_t nd = 0;
    vkGetDisplayPlaneSupportedDisplaysKHR(v.gpu, i, &nd, NULL);
    VkDisplayKHR on[16];
    if (nd > 16) nd = 16;
    vkGetDisplayPlaneSupportedDisplaysKHR(v.gpu, i, &nd, on);
    for (uint32_t j = 0; j < nd; j++) if (on[j] == display) { plane = (int)i; break; }
  }
  if (plane < 0) { fprintf(stderr, "vk-display: no plane shows on the screen\n"); return 1; }
  VkDisplayPlaneCapabilitiesKHR pcaps;
  CHECK(vkGetDisplayPlaneCapabilitiesKHR(v.gpu, modes[pick].displayMode, (uint32_t)plane, &pcaps));
  VkDisplayPlaneAlphaFlagBitsKHR alpha = (pcaps.supportedAlpha & VK_DISPLAY_PLANE_ALPHA_OPAQUE_BIT_KHR) ? VK_DISPLAY_PLANE_ALPHA_OPAQUE_BIT_KHR
                                                                                                        : VK_DISPLAY_PLANE_ALPHA_GLOBAL_BIT_KHR;
  VkExtent2D extent = modes[pick].parameters.visibleRegion;
  VkDisplaySurfaceCreateInfoKHR dsi = {.sType = VK_STRUCTURE_TYPE_DISPLAY_SURFACE_CREATE_INFO_KHR, .displayMode = modes[pick].displayMode,
    .planeIndex = (uint32_t)plane, .planeStackIndex = planes[plane].currentStackIndex, .transform = VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
    .globalAlpha = 1, .alphaMode = alpha, .imageExtent = extent};
  VkSurfaceKHR surface;
  CHECK(vkCreateDisplayPlaneSurfaceKHR(v.inst, &dsi, NULL, &surface));
  VkBool32 presents = VK_FALSE;
  CHECK(vkGetPhysicalDeviceSurfaceSupportKHR(v.gpu, v.family, surface, &presents));
  if (!presents) { fprintf(stderr, "vk-display: the queue cannot present to the screen\n"); return 1; }
  static Vkp p;
  vkp_open(&v, &p, surface, extent.width, extent.height);
  double period = 1000.0 / modes[pick].parameters.refreshRate;

  static Vkr r;
  vkr_open(&v, &r, argv[1], argv[2], argv[3], argv[4]);
  VkcTarget target;
  vkc_target_open(&v, &target, p.w, p.h, r.passes, 2);
  keyboards_open();
  fprintf(stderr, "vk-display: %s on %ux%u at %.3f Hz, plane %d, %d keyboards; Escape or %.0f s returns the screen\n", realm_name(),
          p.w, p.h, 1 / period, plane, keyboard_count, seconds);

  const char *why = "its time was up";
  double start = now_s(), last = start;
  while (r.frames < VKR_FRAMES) {
    double t = now_s();
    if (t - start >= seconds) break;
    if (stopping) { why = "a signal stopped it"; break; }
    if (keyboards_read(r.state)) { why = "Escape was pressed"; break; }
    if (demo) demo_keys(r.state, t - start);
    vkp_wait(&v, &p);
    t = now_s();
    double dt = t - last;
    last = t;
    if (dt > 0.1) dt = 0.1;
    vkr_step(&r, p.w, p.h, scale, (float)dt);
    double t1 = now_s();
    if (realm_action(r.state)) { why = "the realm navigated away"; break; }
    if (shot && !p.shoot && !p.shot_ready && t - start >= shot_at) p.shoot = 1;
    if (vkp_frame(&v, &p, &target, r.passes, NULL, NULL)) { vkp_resize(&v, &p, p.w, p.h); continue; }
    r.waited[r.frames] = p.waited;
    r.stepped[r.frames] = t1 - t;
    r.acquiring[r.frames] = p.acquiring;
    r.presented[r.frames++] = now_s();
  }
  vkp_wait(&v, &p);
  if (shot && p.shot_ready) save_ppm(shot, &p);
  double ran = now_s() - start;

  int skip = (int)(1 / period);
  double longest;
  int dropped = vkr_drops("vk-display", "present", &r, r.presented, r.frames, skip, period, &longest, 1);
  printf("vk-display: %s, %s %ux%u, %.3f Hz, %d frames in %.1f s (%.2f a second); it ended because %s\n", v.props.deviceName, realm_name(),
         p.w, p.h, 1 / period, r.frames, ran, r.frames / ran, why);
  printf("vk-display: dropped frames: %d (longest %.1f ms)\n", dropped, longest * 1000);
  if (v.validating) printf("vk-display: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
  else printf("vk-display: validation layers not loaded\n");

  // Let go of the screen: the swapchain, the display, master.
  vkp_close(&v, &p);
  vkc_target_close(&v, &target);
  vkr_close(&v, &r);
  vkDestroySurfaceKHR(v.inst, surface, NULL);
  release_display(v.gpu, display);
  vkc_close(&v);
  keyboards_close();
  drmDropMaster(fd);
  close(fd);
  return vkc_validation_errors == 0 ? 0 : 1;
}
