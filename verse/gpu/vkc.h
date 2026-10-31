// A headless Vulkan device for the GPU checks (docs/verse/native-gpu.md):
// no surface, no display, no VT and no DRM master. Failures exit with a message.
// With VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation (and the layer on the
// layer path) validation messages are printed and counted; the layers are a
// test aid only.
#ifndef VKC_H
#define VKC_H
#include <stddef.h>
#include <stdint.h>
#include <vulkan/vulkan.h>

typedef struct {
  VkInstance inst;
  VkDebugUtilsMessengerEXT messenger;
  VkPhysicalDevice gpu;
  VkPhysicalDeviceProperties props;
  uint32_t family;
  VkDevice dev;
  VkQueue queue;
  int validating;
} Vkc;

// A host-visible storage buffer, mapped.
typedef struct {
  VkBuffer buf;
  VkDeviceMemory mem;
  void *map;
  VkDeviceSize size;
} VkcBuffer;

extern int vkc_validation_errors, vkc_validation_warnings;

void vkc_open(Vkc *v);
// The same, with instance and device extensions (a window's surface and swapchain).
void vkc_open_with(Vkc *v, const char *const *instance_exts, uint32_t ni, const char *const *device_exts, uint32_t nd);
VkcBuffer vkc_buffer(Vkc *v, VkDeviceSize size);
// Device memory for req with the wanted properties.
VkDeviceMemory vkc_memory(Vkc *v, VkMemoryRequirements req, VkMemoryPropertyFlags want);
// One dispatch of a compute module's `main`: buffers at set 0, bindings
// 0..n-1, push constants, groups by gx and gy. Waits for it to finish.
void vkc_dispatch(Vkc *v, const uint32_t *words, size_t count, const VkcBuffer *buffers, uint32_t n,
                  const void *push, uint32_t push_size, uint32_t gx, uint32_t gy);
// One pass of a draw from `coax spirv --fill` or `--stroke`: its module
// (entry points vs and fs), push constants, and vertex_count vertices,
// from `vertices` (stride bytes each, the SV_STROKE layout) or none.
typedef struct {
  const uint32_t *words;
  size_t count;
  const void *push;
  uint32_t push_size;
  const VkcBuffer *vertices;
  uint32_t stride, vertex_count;
  int additive;  // blend one plus one, else replace
} VkcPass;
// A w by h RGBA f32 image and the passes' pipelines, kept across frames:
// record draws the passes in order into it (cleared to 0) and leaves it in
// TRANSFER_SRC_OPTIMAL for a copy or a blit.
typedef struct {
  VkImage image;
  VkDeviceMemory mem;
  VkImageView view;
  VkRenderPass pass;
  VkFramebuffer fb;
  VkShaderModule modules[8];
  VkPipelineLayout layouts[8];
  VkPipeline pipelines[8];
  uint32_t n, w, h;
} VkcTarget;
void vkc_target_open(Vkc *v, VkcTarget *t, uint32_t w, uint32_t h, const VkcPass *passes, uint32_t n);
void vkc_target_record(VkcTarget *t, VkCommandBuffer cmd, const VkcPass *passes);
void vkc_target_close(Vkc *v, VkcTarget *t);
// The passes in order into a w by h RGBA f32 image cleared to 0, read back
// into rgba (w*h*4 floats, rows top to bottom). Waits for it to finish.
void vkc_render(Vkc *v, uint32_t w, uint32_t h, const VkcPass *passes, uint32_t n, float *rgba);
void vkc_free(Vkc *v, VkcBuffer *b);
void vkc_close(Vkc *v);
#endif
