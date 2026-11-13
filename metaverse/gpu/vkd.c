// The screen with no desktop (vkd.h).
#define _GNU_SOURCE
#include "vkd.h"
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <poll.h>
#include <time.h>
#include <sys/mman.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vkd: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

// The next card from /dev/dri/card<*index> on with a connected connector: its fd (not yet master),
// connector, CRTC (one the connector's encoders can drive, if none drives it now) and preferred mode.
static int find_card(int *index, uint32_t *connector, uint32_t *crtc, drmModeModeInfo *mode) {
  for (; *index < 8; (*index)++) {
    char path[32];
    snprintf(path, sizeof path, "/dev/dri/card%d", *index);
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
        for (int j = 0; !*crtc && j < k->count_encoders; j++) {
          drmModeEncoder *any = drmModeGetEncoder(fd, k->encoders[j]);
          for (int i = 0; any && i < r->count_crtcs; i++) if (any->possible_crtcs & (1u << i)) { *crtc = r->crtcs[i]; break; }
          if (any) drmModeFreeEncoder(any);
        }
        drmModeFreeConnector(k);
        drmModeFreeResources(r);
        (*index)++;
        return fd;
      }
      if (k) drmModeFreeConnector(k);
    }
    if (r) drmModeFreeResources(r);
    close(fd);
  }
  return -1;
}

static void become_master(int fd, const char *who) {
  // The paused session's logind lets go of master a moment after the VT switch.
  for (int t = 0; t < 50 && drmSetMaster(fd) < 0 && errno == EBUSY; t++) usleep(100000);
  if (drmSetMaster(fd) < 0) {
    fprintf(stderr, "%s: another program holds the display (%s); switch to a free VT first (display.sh does)\n", who, strerror(errno));
    exit(1);
  }
}

static int has_instance_extension(const char *name) {
  uint32_t n = 0;
  if (vkEnumerateInstanceExtensionProperties(NULL, &n, NULL) != VK_SUCCESS) return 0;
  VkExtensionProperties e[256];
  if (n > 256) n = 256;
  if (vkEnumerateInstanceExtensionProperties(NULL, &n, e) != VK_SUCCESS && n == 0) return 0;
  for (uint32_t i = 0; i < n; i++) if (!strcmp(e[i].extensionName, name)) return 1;
  return 0;
}

// The copy fallback's screen: two dumb buffers at the mode's size, the first shown.
static void open_copy(Vkd *d, const char *who, int fd, drmModeModeInfo *mode) {
  if (!d->crtc) { fprintf(stderr, "%s: no CRTC can drive the screen\n", who); exit(1); }
  drmModeCrtc *old = drmModeGetCrtc(fd, d->crtc);
  if (old) {
    d->saved_fb = old->buffer_id;
    if (old->mode_valid) { d->saved = malloc(sizeof *d->saved); if (d->saved) *d->saved = old->mode; }
    drmModeFreeCrtc(old);
  }
  for (int b = 0; b < 2; b++) {
    struct drm_mode_create_dumb make = {.width = mode->hdisplay, .height = mode->vdisplay, .bpp = 32};
    if (drmIoctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &make)) { fprintf(stderr, "%s: no dumb buffer: %s\n", who, strerror(errno)); exit(1); }
    d->handle[b] = make.handle;
    d->pitch = make.pitch;
    d->size = (uint32_t)make.size;
    if (drmModeAddFB(fd, mode->hdisplay, mode->vdisplay, 24, 32, make.pitch, make.handle, &d->fb[b])) {
      fprintf(stderr, "%s: no framebuffer: %s\n", who, strerror(errno)); exit(1);
    }
    struct drm_mode_map_dumb where = {.handle = make.handle};
    if (drmIoctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &where)) { fprintf(stderr, "%s: cannot map: %s\n", who, strerror(errno)); exit(1); }
    d->map[b] = mmap(NULL, make.size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, (off_t)where.offset);
    if (d->map[b] == MAP_FAILED) { fprintf(stderr, "%s: cannot map: %s\n", who, strerror(errno)); exit(1); }
    memset(d->map[b], 0, make.size);
  }
  if (drmModeSetCrtc(fd, d->crtc, d->fb[0], 0, 0, &d->connector, 1, mode)) {
    fprintf(stderr, "%s: cannot set the mode: %s\n", who, strerror(errno)); exit(1);
  }
  d->back = 1;
  d->flipping = 1;
  d->fd = fd;
  d->copy = 1;
  d->surface = VK_NULL_HANDLE;
  d->extent = (VkExtent2D){mode->hdisplay, mode->vdisplay};
  d->period = mode->vrefresh ? 1.0 / mode->vrefresh : 1.0 / 60;
  d->saved_mode = *mode;
}

static void flipped(int fd, unsigned int seq, unsigned int s, unsigned int us, void *user) {
  (void)fd; (void)seq; (void)s; (void)us;
  *(int *)user = 1;
}

static int scanout(const void *px, uint32_t w, uint32_t h, void *user) {
  Vkd *d = user;
  if (w > d->extent.width) w = d->extent.width;
  if (h > d->extent.height) h = d->extent.height;
  unsigned char *to = d->map[d->back];
  const unsigned char *from = px;
  for (uint32_t y = 0; y < h; y++) memcpy(to + (size_t)y * d->pitch, from + (size_t)y * w * 4, (size_t)w * 4);
  if (d->flipping) {
    int done = 0;
    if (drmModePageFlip(d->fd, d->crtc, d->fb[d->back], DRM_MODE_PAGE_FLIP_EVENT, &done) == 0) {
      drmEventContext ev = {.version = 2, .page_flip_handler = flipped};
      struct pollfd pf = {.fd = d->fd, .events = POLLIN};
      // a flip lands within a frame or two; a driver that never says so falls back below
      while (!done && poll(&pf, 1, 200) > 0) drmHandleEvent(d->fd, &ev);
      if (!done) d->flipping = 0;
    } else d->flipping = 0;
    if (!d->flipping) fprintf(stderr, "vkd: no page flips on this card; setting the CRTC each frame\n");
    if (done) { d->back ^= 1; return 0; }
  }
  drmModeSetCrtc(d->fd, d->crtc, d->fb[d->back], 0, 0, &d->connector, 1, &d->saved_mode);
  d->back ^= 1;
  // keep to the mode's rate when nothing paces us
  struct timespec t = {0, (long)(d->period * 1e9)};
  nanosleep(&t, NULL);
  return 0;
}

void vkd_attach(Vkd *d, Vkp *p) {
  if (!d->copy) return;
  p->scanout = scanout;
  p->scanout_user = d;
}

void vkd_save_ppm(const char *path, const Vkp *p) {
  FILE *f = fopen(path, "wb");
  if (!f) { fprintf(stderr, "vkd: cannot write %s\n", path); return; }
  fprintf(f, "P6\n%u %u\n255\n", p->w, p->h);
  const unsigned char *px = p->shot.map;
  int bgr = p->format.format == VK_FORMAT_B8G8R8A8_UNORM || p->format.format == VK_FORMAT_B8G8R8A8_SRGB;
  for (size_t i = 0; i < (size_t)p->w * p->h; i++) {
    unsigned char c[3] = {px[i * 4 + (bgr ? 2 : 0)], px[i * 4 + 1], px[i * 4 + (bgr ? 0 : 2)]};
    fwrite(c, 1, 3, f);
  }
  fclose(f);
}

void vkd_open(Vkc *v, Vkd *d, const char *who) {
  memset(d, 0, sizeof *d);
  uint32_t connector = 0, crtc = 0;
  drmModeModeInfo mode;
  const char *iexts[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_DISPLAY_EXTENSION_NAME, VK_EXT_DIRECT_MODE_DISPLAY_EXTENSION_NAME,
                         VK_EXT_ACQUIRE_DRM_DISPLAY_EXTENSION_NAME};
  const char *force = (getenv("METAVERSE_KMS_COPY") ? getenv("METAVERSE_KMS_COPY") : getenv("VERSE_KMS_COPY"));
  int direct = !(force && *force && strcmp(force, "0"));
  for (int i = 0; i < 4 && direct; i++) direct = has_instance_extension(iexts[i]);
  int fd = -1;
  VkDisplayKHR display = VK_NULL_HANDLE;
  PFN_vkReleaseDisplayEXT release_display = NULL;
  if (direct) {
    // Vulkan with the connector handed to it: the card that is the chosen GPU's own.
    const char *dexts[] = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
    vkc_open_with(v, iexts, 4, dexts, 1);
    PFN_vkGetDrmDisplayEXT get_display = (PFN_vkGetDrmDisplayEXT)vkGetInstanceProcAddr(v->inst, "vkGetDrmDisplayEXT");
    PFN_vkAcquireDrmDisplayEXT acquire_display = (PFN_vkAcquireDrmDisplayEXT)vkGetInstanceProcAddr(v->inst, "vkAcquireDrmDisplayEXT");
    release_display = (PFN_vkReleaseDisplayEXT)vkGetInstanceProcAddr(v->inst, "vkReleaseDisplayEXT");
    int index = 0;
    while (get_display && acquire_display && release_display && (fd = find_card(&index, &connector, &crtc, &mode)) >= 0) {
      become_master(fd, who);
      if (get_display(v->gpu, fd, connector, &display) == VK_SUCCESS && acquire_display(v->gpu, fd, display) == VK_SUCCESS) break;
      display = VK_NULL_HANDLE;
      drmDropMaster(fd);
      close(fd);
    }
    if (!display) {
      fprintf(stderr, "%s: %s cannot take a screen itself; copying its frames to the screen\n", who, v->props.deviceName);
      vkc_close(v);
      fd = -1;
    }
  }
  if (!display) {
    int index = 0;
    fd = find_card(&index, &connector, &crtc, &mode);
    if (fd < 0) { fprintf(stderr, "%s: no card with a connected screen\n", who); exit(1); }
    become_master(fd, who);
    vkc_open(v);
    d->connector = connector;
    d->crtc = crtc;
    open_copy(d, who, fd, &mode);
    fprintf(stderr, "%s: %s draws; %ux%u at %u Hz through dumb buffers\n", who, v->props.deviceName, mode.hdisplay, mode.vdisplay, mode.vrefresh);
    return;
  }

  // The mode the connector prefers, and a plane that shows on it.
  uint32_t nm = 0;
  CHECK(vkGetDisplayModePropertiesKHR(v->gpu, display, &nm, NULL));
  VkDisplayModePropertiesKHR modes[64];
  if (nm > 64) nm = 64;
  CHECK(vkGetDisplayModePropertiesKHR(v->gpu, display, &nm, modes));
  int pick = -1;
  for (uint32_t i = 0; i < nm; i++) {
    VkDisplayModeParametersKHR m = modes[i].parameters;
    if (m.visibleRegion.width != mode.hdisplay || m.visibleRegion.height != mode.vdisplay) continue;
    if (pick < 0 || abs((int)m.refreshRate - (int)mode.vrefresh * 1000) < abs((int)modes[pick].parameters.refreshRate - (int)mode.vrefresh * 1000)) pick = (int)i;
  }
  if (pick < 0) { fprintf(stderr, "%s: Vulkan has no %ux%u mode\n", who, mode.hdisplay, mode.vdisplay); exit(1); }
  uint32_t np = 0;
  CHECK(vkGetPhysicalDeviceDisplayPlanePropertiesKHR(v->gpu, &np, NULL));
  VkDisplayPlanePropertiesKHR planes[32];
  if (np > 32) np = 32;
  CHECK(vkGetPhysicalDeviceDisplayPlanePropertiesKHR(v->gpu, &np, planes));
  int plane = -1;
  for (uint32_t i = 0; i < np && plane < 0; i++) {
    if (planes[i].currentDisplay && planes[i].currentDisplay != display) continue;
    uint32_t nd = 0;
    vkGetDisplayPlaneSupportedDisplaysKHR(v->gpu, i, &nd, NULL);
    VkDisplayKHR on[16];
    if (nd > 16) nd = 16;
    vkGetDisplayPlaneSupportedDisplaysKHR(v->gpu, i, &nd, on);
    for (uint32_t j = 0; j < nd; j++) if (on[j] == display) { plane = (int)i; break; }
  }
  if (plane < 0) { fprintf(stderr, "%s: no plane shows on the screen\n", who); exit(1); }
  VkDisplayPlaneCapabilitiesKHR pcaps;
  CHECK(vkGetDisplayPlaneCapabilitiesKHR(v->gpu, modes[pick].displayMode, (uint32_t)plane, &pcaps));
  VkDisplayPlaneAlphaFlagBitsKHR alpha = (pcaps.supportedAlpha & VK_DISPLAY_PLANE_ALPHA_OPAQUE_BIT_KHR) ? VK_DISPLAY_PLANE_ALPHA_OPAQUE_BIT_KHR
                                                                                                        : VK_DISPLAY_PLANE_ALPHA_GLOBAL_BIT_KHR;
  VkExtent2D extent = modes[pick].parameters.visibleRegion;
  VkDisplaySurfaceCreateInfoKHR dsi = {.sType = VK_STRUCTURE_TYPE_DISPLAY_SURFACE_CREATE_INFO_KHR, .displayMode = modes[pick].displayMode,
    .planeIndex = (uint32_t)plane, .planeStackIndex = planes[plane].currentStackIndex, .transform = VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
    .globalAlpha = 1, .alphaMode = alpha, .imageExtent = extent};
  VkSurfaceKHR surface;
  CHECK(vkCreateDisplayPlaneSurfaceKHR(v->inst, &dsi, NULL, &surface));
  VkBool32 presents = VK_FALSE;
  CHECK(vkGetPhysicalDeviceSurfaceSupportKHR(v->gpu, v->family, surface, &presents));
  if (!presents) { fprintf(stderr, "%s: the queue cannot present to the screen\n", who); exit(1); }
  d->fd = fd;
  d->display = display;
  d->surface = surface;
  d->extent = extent;
  d->period = 1000.0 / modes[pick].parameters.refreshRate;
  d->plane = plane;
  d->release = release_display;
}

void vkd_close(Vkc *v, Vkd *d) {
  if (d->copy) {
    // the CRTC as it was (the console's), then the buffers
    if (d->saved) drmModeSetCrtc(d->fd, d->crtc, d->saved_fb, 0, 0, &d->connector, 1, d->saved);
    free(d->saved);
    for (int b = 0; b < 2; b++) {
      if (d->map[b] && d->map[b] != MAP_FAILED) munmap(d->map[b], d->size);
      drmModeRmFB(d->fd, d->fb[b]);
      struct drm_mode_destroy_dumb gone = {.handle = d->handle[b]};
      drmIoctl(d->fd, DRM_IOCTL_MODE_DESTROY_DUMB, &gone);
    }
    vkc_close(v);
    drmDropMaster(d->fd);
    close(d->fd);
    return;
  }
  vkDestroySurfaceKHR(v->inst, d->surface, NULL);
  d->release(v->gpu, d->display);
  vkc_close(v);
  drmDropMaster(d->fd);
  close(d->fd);
}
