// A headless Vulkan compute device for the GPU checks (docs/verse/native-gpu.md):
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
VkcBuffer vkc_buffer(Vkc *v, VkDeviceSize size);
// One dispatch of a compute module's `main`: buffers at set 0, bindings
// 0..n-1, push constants, groups by gx and gy. Waits for it to finish.
void vkc_dispatch(Vkc *v, const uint32_t *words, size_t count, const VkcBuffer *buffers, uint32_t n,
                  const void *push, uint32_t push_size, uint32_t gx, uint32_t gy);
void vkc_free(Vkc *v, VkcBuffer *b);
void vkc_close(Vkc *v);
#endif
