// The screen with no desktop (docs/metaverse/native-gpu.md, G2): DRM master on the card with a
// connected connector, the connector handed to Vulkan (VK_EXT_acquire_drm_display), its
// preferred mode and a plane that shows on it, as a VK_KHR_display surface. vk-display.c and
// vk-realm.c present on it; the caller switched to a free VT first (display.sh does).
// When no Vulkan device can take the screen (a driver without the extension, a VM with a software
// Vulkan, or METAVERSE_KMS_COPY=1), it falls back to copying: the frame is drawn by any Vulkan device,
// read back, and put on the screen through two DRM dumb buffers (surface is then VK_NULL_HANDLE;
// vkd_attach gives the presenter its scanout).
#ifndef VKD_H
#define VKD_H
#include "vkp.h"
#include <xf86drmMode.h>

typedef struct {
  int fd;
  VkDisplayKHR display;
  VkSurfaceKHR surface;
  VkExtent2D extent;
  double period;     // the mode's frame time, in seconds
  int plane;
  PFN_vkReleaseDisplayEXT release;
  int copy;                 // the dumb-buffer fallback
  uint32_t connector, crtc, fb[2], handle[2], pitch, size, back;
  drmModeModeInfoPtr saved; // the CRTC as it was, put back on close
  uint32_t saved_fb;
  drmModeModeInfo saved_mode; // the mode set (the screen's preferred)
  void *map[2];
  int flipping;             // page flips work; else each frame sets the CRTC
} Vkd;

// Opens the card as DRM master (exits, saying why, if it cannot), then Vulkan with the
// display extensions, the display acquired and its surface made.
void vkd_open(Vkc *v, Vkd *d, const char *who);
// Lets go: the surface, the display, Vulkan, master.
void vkd_close(Vkc *v, Vkd *d);
// The presenter's scanout, for the copy fallback (no-op when Vulkan has the screen).
void vkd_attach(Vkd *d, Vkp *p);
// A presented image (Vkp's shot) as a PPM.
void vkd_save_ppm(const char *path, const Vkp *p);
#endif
