// Additive checked Vulkan factories. The caller owns admission and pending-use
// lifetime. An empty destination publishes only after every SDK effect succeeds;
// failures unwind private handles and leave both destination and info unchanged.
#ifndef VKS_RESOURCE_H
#define VKS_RESOURCE_H
#include "vks.h"
typedef struct {
  VkMemoryRequirements requirements;
  uint32_t memory_type;
} VksMemoryInfo;
VkResult vks_buffer_try(Vkc *v,VkcBuffer *out,VkDeviceSize size,VksMemoryInfo *info);
VkResult vks_image_try(Vkc *v,VksImage *out,uint32_t w,uint32_t h,uint32_t layers,
                       int array,VkFormat format,int use,uint32_t levels,VksMemoryInfo *info);
#endif
