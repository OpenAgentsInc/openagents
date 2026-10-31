// Presenting a vkc_target (vkp.h).
#define _POSIX_C_SOURCE 200809L
#include "vkp.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vkp: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

static double now_s(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec / 1e9;
}

static void make_chain(Vkc *v, Vkp *p, uint32_t w, uint32_t h) {
  VkSurfaceCapabilitiesKHR caps;
  CHECK(vkGetPhysicalDeviceSurfaceCapabilitiesKHR(v->gpu, p->surface, &caps));
  uint32_t nf = 0;
  CHECK(vkGetPhysicalDeviceSurfaceFormatsKHR(v->gpu, p->surface, &nf, NULL));
  VkSurfaceFormatKHR formats[64];
  if (nf > 64) nf = 64;
  CHECK(vkGetPhysicalDeviceSurfaceFormatsKHR(v->gpu, p->surface, &nf, formats));
  // The bytes kms.c writes, as they are: an UNORM format, not sRGB.
  p->format = formats[0];
  for (uint32_t i = 0; i < nf; i++)
    if (formats[i].format == VK_FORMAT_B8G8R8A8_UNORM || formats[i].format == VK_FORMAT_R8G8B8A8_UNORM) { p->format = formats[i]; break; }
  if (caps.currentExtent.width != UINT32_MAX) { w = caps.currentExtent.width; h = caps.currentExtent.height; }
  if (w < caps.minImageExtent.width) w = caps.minImageExtent.width;
  if (h < caps.minImageExtent.height) h = caps.minImageExtent.height;
  if (caps.maxImageExtent.width && w > caps.maxImageExtent.width) w = caps.maxImageExtent.width;
  if (caps.maxImageExtent.height && h > caps.maxImageExtent.height) h = caps.maxImageExtent.height;
  uint32_t images = caps.minImageCount + 1;
  if (caps.maxImageCount && images > caps.maxImageCount) images = caps.maxImageCount;
  if (images > VKP_IMAGES) images = VKP_IMAGES;
  VkCompositeAlphaFlagBitsKHR alpha = VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR;
  if (!(caps.supportedCompositeAlpha & alpha))
    for (uint32_t b = 1; b; b <<= 1) if (caps.supportedCompositeAlpha & b) { alpha = (VkCompositeAlphaFlagBitsKHR)b; break; }
  VkImageUsageFlags usage = VK_IMAGE_USAGE_TRANSFER_DST_BIT | (caps.supportedUsageFlags & VK_IMAGE_USAGE_TRANSFER_SRC_BIT);
  VkSwapchainKHR old = p->chain;
  VkSwapchainCreateInfoKHR sci = {.sType = VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR, .surface = p->surface, .minImageCount = images,
    .imageFormat = p->format.format, .imageColorSpace = p->format.colorSpace, .imageExtent = {w, h}, .imageArrayLayers = 1,
    .imageUsage = usage, .imageSharingMode = VK_SHARING_MODE_EXCLUSIVE, .preTransform = caps.currentTransform,
    .compositeAlpha = alpha, .presentMode = VK_PRESENT_MODE_FIFO_KHR, .clipped = VK_TRUE, .oldSwapchain = old};
  CHECK(vkCreateSwapchainKHR(v->dev, &sci, NULL, &p->chain));
  if (old) vkDestroySwapchainKHR(v->dev, old, NULL);
  for (uint32_t i = 0; i < p->count; i++) vkDestroySemaphore(v->dev, p->done[i], NULL);
  p->count = VKP_IMAGES;
  CHECK(vkGetSwapchainImagesKHR(v->dev, p->chain, &p->count, p->images));
  VkSemaphoreCreateInfo semi = {.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO};
  for (uint32_t i = 0; i < p->count; i++) CHECK(vkCreateSemaphore(v->dev, &semi, NULL, &p->done[i]));
  if (p->shot.buf) vkc_free(v, &p->shot);
  p->shot = (usage & VK_IMAGE_USAGE_TRANSFER_SRC_BIT) ? vkc_buffer(v, (VkDeviceSize)w * h * 4) : (VkcBuffer){0};
  p->shot_ready = 0;
  p->w = w;
  p->h = h;
}

void vkp_open(Vkc *v, Vkp *p, VkSurfaceKHR surface, uint32_t w, uint32_t h) {
  memset(p, 0, sizeof *p);
  p->surface = surface;
  make_chain(v, p, w, h);
  VkCommandPoolCreateInfo cpci = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT, .queueFamilyIndex = v->family};
  CHECK(vkCreateCommandPool(v->dev, &cpci, NULL, &p->pool));
  VkCommandBufferAllocateInfo cbai = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO, .commandPool = p->pool, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1};
  CHECK(vkAllocateCommandBuffers(v->dev, &cbai, &p->cmd));
  VkFenceCreateInfo fci = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO, .flags = VK_FENCE_CREATE_SIGNALED_BIT};
  CHECK(vkCreateFence(v->dev, &fci, NULL, &p->fence));
  VkSemaphoreCreateInfo semi = {.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO};
  CHECK(vkCreateSemaphore(v->dev, &semi, NULL, &p->acquired));
}

void vkp_resize(Vkc *v, Vkp *p, uint32_t w, uint32_t h) {
  CHECK(vkDeviceWaitIdle(v->dev));
  make_chain(v, p, w, h);
}

void vkp_wait(Vkc *v, Vkp *p) {
  double t = now_s();
  CHECK(vkWaitForFences(v->dev, 1, &p->fence, VK_TRUE, UINT64_MAX));
  p->waited = now_s() - t;
  // The shot's copy finished with the frame that took it.
  if (p->shoot == 2) { p->shoot = 0; p->shot_ready = 1; }
}

static void barrier(VkCommandBuffer cmd, VkImage image, VkImageLayout from, VkImageLayout to, VkAccessFlags src, VkAccessFlags dst,
                    VkPipelineStageFlags src_stage, VkPipelineStageFlags dst_stage) {
  VkImageMemoryBarrier b = {.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER, .srcAccessMask = src, .dstAccessMask = dst, .oldLayout = from, .newLayout = to,
    .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .image = image,
    .subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1}};
  vkCmdPipelineBarrier(cmd, src_stage, dst_stage, 0, 0, NULL, 0, NULL, 1, &b);
}

// A frame: `record` draws into cmd, then src (w by h, left in `layout`, read after `stage`) is
// blitted to the swapchain image with `filter`, and src goes back to `layout`.
static int frame(Vkc *v, Vkp *p, void (*record)(VkCommandBuffer, void *), void *user, VkImage src, uint32_t w, uint32_t h,
                 VkImageLayout layout, VkPipelineStageFlags stage, VkAccessFlags access, VkFilter filter, void (*before)(void *), void *arg) {
  double t0 = now_s();
  uint32_t image;
  VkResult got = vkAcquireNextImageKHR(v->dev, p->chain, UINT64_MAX, p->acquired, VK_NULL_HANDLE, &image);
  p->acquiring = now_s() - t0;
  if (got == VK_ERROR_OUT_OF_DATE_KHR) return 1;
  if (got != VK_SUCCESS && got != VK_SUBOPTIMAL_KHR) CHECK(got);
  CHECK(vkResetFences(v->dev, 1, &p->fence));

  VkCommandBuffer cmd = p->cmd;
  CHECK(vkResetCommandBuffer(cmd, 0));
  VkCommandBufferBeginInfo begin = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO, .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT};
  CHECK(vkBeginCommandBuffer(cmd, &begin));
  record(cmd, user);
  if (layout != VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL)
    barrier(cmd, src, layout, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, access, VK_ACCESS_TRANSFER_READ_BIT, stage, VK_PIPELINE_STAGE_TRANSFER_BIT);
  VkImage dst = p->images[image];
  barrier(cmd, dst, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 0, VK_ACCESS_TRANSFER_WRITE_BIT,
          VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT);
  VkImageBlit blit = {.srcSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .srcOffsets = {{0, 0, 0}, {(int32_t)w, (int32_t)h, 1}},
                      .dstSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .dstOffsets = {{0, 0, 0}, {(int32_t)p->w, (int32_t)p->h, 1}}};
  vkCmdBlitImage(cmd, src, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, dst, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &blit, filter);
  if (layout != VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL)
    barrier(cmd, src, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, layout, VK_ACCESS_TRANSFER_READ_BIT, access, VK_PIPELINE_STAGE_TRANSFER_BIT, stage);
  VkImageLayout last = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
  VkAccessFlags last_access = VK_ACCESS_TRANSFER_WRITE_BIT;
  if (p->shoot == 1 && p->shot.buf) {
    // The image about to be presented, into the shot buffer.
    barrier(cmd, dst, last, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, last_access, VK_ACCESS_TRANSFER_READ_BIT,
            VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT);
    VkBufferImageCopy copy = {.imageSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .imageExtent = {p->w, p->h, 1}};
    vkCmdCopyImageToBuffer(cmd, dst, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, p->shot.buf, 1, &copy);
    VkMemoryBarrier host = {.sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER, .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT, .dstAccessMask = VK_ACCESS_HOST_READ_BIT};
    vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_HOST_BIT, 0, 1, &host, 0, NULL, 0, NULL);
    last = VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL;
    last_access = VK_ACCESS_TRANSFER_READ_BIT;
    p->shoot = 2;
  }
  barrier(cmd, dst, last, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, last_access, 0, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT);
  CHECK(vkEndCommandBuffer(cmd));
  VkPipelineStageFlags wait = VK_PIPELINE_STAGE_TRANSFER_BIT;
  VkSubmitInfo si = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .waitSemaphoreCount = 1, .pWaitSemaphores = &p->acquired, .pWaitDstStageMask = &wait,
                     .commandBufferCount = 1, .pCommandBuffers = &cmd, .signalSemaphoreCount = 1, .pSignalSemaphores = &p->done[image]};
  CHECK(vkQueueSubmit(v->queue, 1, &si, p->fence));
  if (before) before(arg);
  VkPresentInfoKHR pi = {.sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR, .waitSemaphoreCount = 1, .pWaitSemaphores = &p->done[image],
                         .swapchainCount = 1, .pSwapchains = &p->chain, .pImageIndices = &image};
  VkResult shown = vkQueuePresentKHR(v->queue, &pi);
  if (shown == VK_ERROR_OUT_OF_DATE_KHR) return 1;
  if (shown != VK_SUCCESS && shown != VK_SUBOPTIMAL_KHR) CHECK(shown);
  return 0;
}

typedef struct { VkcTarget *t; const VkcPass *passes; } TargetFrame;
static void record_target(VkCommandBuffer cmd, void *user) {
  TargetFrame *f = user;
  vkc_target_record(f->t, cmd, f->passes);
}
int vkp_frame(Vkc *v, Vkp *p, VkcTarget *t, const VkcPass *passes, void (*before)(void *), void *arg) {
  TargetFrame f = {t, passes};
  // the target is left ready to copy from
  return frame(v, p, record_target, &f, t->image, t->w, t->h, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_PIPELINE_STAGE_TRANSFER_BIT, 0,
               VK_FILTER_NEAREST, before, arg);
}
int vkp_present(Vkc *v, Vkp *p, void (*record)(VkCommandBuffer, void *), void *user, VkImage src, uint32_t w, uint32_t h,
                void (*before)(void *), void *arg) {
  return frame(v, p, record, user, src, w, h, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
               VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT | VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
               VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT, VK_FILTER_LINEAR, before, arg);
}

void vkp_close(Vkc *v, Vkp *p) {
  CHECK(vkDeviceWaitIdle(v->dev));
  vkDestroySemaphore(v->dev, p->acquired, NULL);
  vkDestroyFence(v->dev, p->fence, NULL);
  vkDestroyCommandPool(v->dev, p->pool, NULL);
  for (uint32_t i = 0; i < p->count; i++) vkDestroySemaphore(v->dev, p->done[i], NULL);
  if (p->shot.buf) vkc_free(v, &p->shot);
  vkDestroySwapchainKHR(v->dev, p->chain, NULL);
}
