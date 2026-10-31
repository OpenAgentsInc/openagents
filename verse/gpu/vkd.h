// The screen with no desktop (docs/verse/native-gpu.md, G2): DRM master on the card with a
// connected connector, the connector handed to Vulkan (VK_EXT_acquire_drm_display), its
// preferred mode and a plane that shows on it, as a VK_KHR_display surface. vk-display.c and
// vk-realm.c present on it; the caller switched to a free VT first (display.sh does).
#ifndef VKD_H
#define VKD_H
#include "vkp.h"

typedef struct {
  int fd;
  VkDisplayKHR display;
  VkSurfaceKHR surface;
  VkExtent2D extent;
  double period;     // the mode's frame time, in seconds
  int plane;
  PFN_vkReleaseDisplayEXT release;
} Vkd;

// Opens the card as DRM master (exits, saying why, if it cannot), then Vulkan with the
// display extensions, the display acquired and its surface made.
void vkd_open(Vkc *v, Vkd *d, const char *who);
// Lets go: the surface, the display, Vulkan, master.
void vkd_close(Vkc *v, Vkd *d);
// A presented image (Vkp's shot) as a PPM.
void vkd_save_ppm(const char *path, const Vkp *p);
#endif
