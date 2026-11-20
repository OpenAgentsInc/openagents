// Firm reason: 6 (the Vulkan loader's C ABI).
// Vulkan on Windows (GitHub #13782), loaded when the app starts rather than linked: the Vulkan
// helpers (metaverse/gpu/vkc, vks, vkp) and the Windows host call vkCreateInstance and the rest
// by name, and on Windows each name is a function pointer this header declares (VK_NO_PROTOTYPES)
// and vk-load.c fills from vulkan-1.dll, which the graphics driver installs. Without the DLL, or
// with one too old, ui_vk_load says so in words and the app shows them, where a linked import
// would stop the program before main with the system's "vulkan-1.dll was not found".
// Windows builds pass it to every unit with -include (apps/openagents-desktop/windows/build.sh).
#ifndef UI_VK_LOAD_H
#define UI_VK_LOAD_H
#ifndef VK_NO_PROTOTYPES
#define VK_NO_PROTOTYPES
#endif
#include <stddef.h>
#include <vulkan/vulkan.h>

#define UI_VK_FUNCTIONS(X) \
  X(vkAcquireNextImageKHR) X(vkAllocateCommandBuffers) X(vkAllocateDescriptorSets) X(vkAllocateMemory) \
  X(vkBeginCommandBuffer) X(vkBindBufferMemory) X(vkBindImageMemory) X(vkCmdBeginRenderPass) \
  X(vkCmdBindDescriptorSets) X(vkCmdBindIndexBuffer) X(vkCmdBindPipeline) X(vkCmdBindVertexBuffers) \
  X(vkCmdBlitImage) X(vkCmdCopyBufferToImage) X(vkCmdCopyImage) X(vkCmdCopyImageToBuffer) X(vkCmdDispatch) \
  X(vkCmdDraw) X(vkCmdDrawIndexed) X(vkCmdEndRenderPass) X(vkCmdPipelineBarrier) X(vkCmdPushConstants) \
  X(vkCreateBuffer) X(vkCreateCommandPool) X(vkCreateComputePipelines) X(vkCreateDescriptorPool) \
  X(vkCreateDescriptorSetLayout) X(vkCreateDevice) X(vkCreateFence) X(vkCreateFramebuffer) \
  X(vkCreateGraphicsPipelines) X(vkCreateImage) X(vkCreateImageView) X(vkCreateInstance) \
  X(vkCreatePipelineLayout) X(vkCreateRenderPass) X(vkCreateSampler) X(vkCreateSemaphore) \
  X(vkCreateShaderModule) X(vkCreateSwapchainKHR) X(vkDestroyBuffer) X(vkDestroyCommandPool) \
  X(vkDestroyDescriptorPool) X(vkDestroyDescriptorSetLayout) X(vkDestroyDevice) X(vkDestroyFence) \
  X(vkDestroyFramebuffer) X(vkDestroyImage) X(vkDestroyImageView) X(vkDestroyInstance) X(vkDestroyPipeline) \
  X(vkDestroyPipelineLayout) X(vkDestroyRenderPass) X(vkDestroySampler) X(vkDestroySemaphore) \
  X(vkDestroyShaderModule) X(vkDestroySurfaceKHR) X(vkDestroySwapchainKHR) X(vkDeviceWaitIdle) \
  X(vkEndCommandBuffer) X(vkEnumerateDeviceExtensionProperties) X(vkEnumerateInstanceExtensionProperties) \
  X(vkEnumerateInstanceLayerProperties) X(vkEnumeratePhysicalDevices) X(vkFreeCommandBuffers) \
  X(vkFreeDescriptorSets) X(vkFreeMemory) X(vkGetBufferMemoryRequirements) X(vkGetDeviceQueue) \
  X(vkGetFenceStatus) X(vkGetImageMemoryRequirements) X(vkGetInstanceProcAddr) X(vkGetPhysicalDeviceFeatures) \
  X(vkGetPhysicalDeviceFeatures2) X(vkGetPhysicalDeviceFormatProperties) X(vkGetPhysicalDeviceMemoryProperties) \
  X(vkGetPhysicalDeviceProperties) X(vkGetPhysicalDeviceQueueFamilyProperties) \
  X(vkGetPhysicalDeviceSurfaceCapabilitiesKHR) X(vkGetPhysicalDeviceSurfaceFormatsKHR) \
  X(vkGetPhysicalDeviceSurfacePresentModesKHR) X(vkGetPhysicalDeviceSurfaceSupportKHR) \
  X(vkGetSwapchainImagesKHR) X(vkMapMemory) X(vkQueuePresentKHR) X(vkQueueSubmit) X(vkQueueWaitIdle) \
  X(vkResetCommandBuffer) X(vkResetFences) X(vkUnmapMemory) X(vkUpdateDescriptorSets) X(vkWaitForFences)

#define UI_VK_DECLARE(name) extern PFN_##name name;
UI_VK_FUNCTIONS(UI_VK_DECLARE)
#undef UI_VK_DECLARE

// vulkan-1.dll and every function above: 1, or 0 with why (a sentence for the person) in why.
int ui_vk_load(char *why, size_t cap);
#endif
