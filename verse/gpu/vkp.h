// Presenting a vkc_target to a Vulkan surface, a window's or a display's
// (docs/verse/native-gpu.md, G2): a FIFO swapchain, one frame in flight.
// Each frame the target's passes draw offscreen and the image is blitted to
// the swapchain image; with `shoot` set, that frame's presented image is also
// copied into `shot` (its pixels as the swapchain holds them).
#ifndef VKP_H
#define VKP_H
#include "vkc.h"

#define VKP_IMAGES 8

typedef struct {
  VkSurfaceKHR surface;
  VkSwapchainKHR chain;
  VkSurfaceFormatKHR format;
  uint32_t w, h, count;
  VkImage images[VKP_IMAGES];
  VkSemaphore done[VKP_IMAGES];
  VkCommandPool pool;
  VkCommandBuffer cmd;
  VkFence fence;
  VkSemaphore acquired;
  int shoot;            // copy the next presented image into shot
  VkcBuffer shot;       // w*h*4 bytes once shot_ready
  int shot_ready;
  double waited, acquiring;  // the last frame's fence wait and acquire, in seconds
} Vkp;

// A swapchain on the surface, sized by the surface or else w by h.
void vkp_open(Vkc *v, Vkp *p, VkSurfaceKHR surface, uint32_t w, uint32_t h);
void vkp_resize(Vkc *v, Vkp *p, uint32_t w, uint32_t h);
// Waits for the last frame, so its vertices and push constants can change.
void vkp_wait(Vkc *v, Vkp *p);
// Draws the passes into the target, blits it and presents. `before` runs just
// before the present (a Wayland frame callback rides on its commit). It
// returns 1 when the swapchain is out of date (resize and draw again), else 0.
int vkp_frame(Vkc *v, Vkp *p, VkcTarget *t, const VkcPass *passes, void (*before)(void *), void *arg);
// The same for passes of the caller's own: `record` draws into the frame's command buffer, then
// src (w by h, a sampled image its passes leave in SHADER_READ_ONLY_OPTIMAL) is blitted to the
// swapchain image, filtered linearly when the sizes differ (verse/gpu/vk-realm.c).
int vkp_present(Vkc *v, Vkp *p, void (*record)(VkCommandBuffer, void *), void *user, VkImage src, uint32_t w, uint32_t h,
                void (*before)(void *), void *arg);
void vkp_close(Vkc *v, Vkp *p);
#endif
