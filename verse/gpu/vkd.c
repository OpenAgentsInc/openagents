// The screen with no desktop (vkd.h).
#define _GNU_SOURCE
#include "vkd.h"
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vkd: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

// The card with a connected connector, as DRM master; its connector and CRTC.
static int open_card(const char *who, uint32_t *connector, uint32_t *crtc, drmModeModeInfo *mode) {
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
          fprintf(stderr, "%s: another program holds the display (%s); switch to a free VT first (display.sh does)\n", who, strerror(errno));
          exit(1);
        }
        return fd;
      }
      if (k) drmModeFreeConnector(k);
    }
    if (r) drmModeFreeResources(r);
    close(fd);
  }
  fprintf(stderr, "%s: no card with a connected screen\n", who);
  exit(1);
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
  int fd = open_card(who, &connector, &crtc, &mode);
  (void)crtc;

  // Vulkan with the connector handed to it.
  const char *iexts[] = {VK_KHR_SURFACE_EXTENSION_NAME, VK_KHR_DISPLAY_EXTENSION_NAME, VK_EXT_DIRECT_MODE_DISPLAY_EXTENSION_NAME,
                         VK_EXT_ACQUIRE_DRM_DISPLAY_EXTENSION_NAME};
  const char *dexts[] = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
  vkc_open_with(v, iexts, 4, dexts, 1);
  PFN_vkGetDrmDisplayEXT get_display = (PFN_vkGetDrmDisplayEXT)vkGetInstanceProcAddr(v->inst, "vkGetDrmDisplayEXT");
  PFN_vkAcquireDrmDisplayEXT acquire_display = (PFN_vkAcquireDrmDisplayEXT)vkGetInstanceProcAddr(v->inst, "vkAcquireDrmDisplayEXT");
  PFN_vkReleaseDisplayEXT release_display = (PFN_vkReleaseDisplayEXT)vkGetInstanceProcAddr(v->inst, "vkReleaseDisplayEXT");
  if (!get_display || !acquire_display || !release_display) { fprintf(stderr, "%s: the driver has no DRM display acquire\n", who); exit(1); }
  VkDisplayKHR display;
  CHECK(get_display(v->gpu, fd, connector, &display));
  CHECK(acquire_display(v->gpu, fd, display));

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
  vkDestroySurfaceKHR(v->inst, d->surface, NULL);
  d->release(v->gpu, d->display);
  vkc_close(v);
  drmDropMaster(d->fd);
  close(d->fd);
}
