// The headless Vulkan compute device (vkc.h).
#include "vkc.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vkc: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

int vkc_validation_errors, vkc_validation_warnings;

static VKAPI_ATTR VkBool32 VKAPI_CALL on_message(VkDebugUtilsMessageSeverityFlagBitsEXT severity, VkDebugUtilsMessageTypeFlagsEXT type,
                                                 const VkDebugUtilsMessengerCallbackDataEXT *data, void *user) {
  (void)type; (void)user;
  if (severity >= VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT) vkc_validation_errors++;
  else if (severity >= VK_DEBUG_UTILS_MESSAGE_SEVERITY_WARNING_BIT_EXT) vkc_validation_warnings++;
  else return VK_FALSE;
  fprintf(stderr, "vkc: validation: %s\n", data->pMessage);
  return VK_FALSE;
}

static int has_layer(const char *name) {
  uint32_t n = 0;
  vkEnumerateInstanceLayerProperties(&n, NULL);
  VkLayerProperties *p = calloc(n ? n : 1, sizeof *p);
  vkEnumerateInstanceLayerProperties(&n, p);
  int found = 0;
  for (uint32_t i = 0; i < n; i++) found |= !strcmp(p[i].layerName, name);
  free(p);
  return found;
}

void vkc_open(Vkc *v) {
  memset(v, 0, sizeof *v);
  // An instance with no surface extensions: compute only.
  v->validating = has_layer("VK_LAYER_KHRONOS_validation") && getenv("VK_INSTANCE_LAYERS");
  const char *exts[] = {VK_EXT_DEBUG_UTILS_EXTENSION_NAME};
  VkApplicationInfo app = {.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO, .pApplicationName = "verse-gpu", .apiVersion = VK_API_VERSION_1_1};
  VkInstanceCreateInfo ici = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo = &app,
                              .enabledExtensionCount = v->validating ? 1u : 0u, .ppEnabledExtensionNames = exts};
  CHECK(vkCreateInstance(&ici, NULL, &v->inst));
  if (v->validating) {
    PFN_vkCreateDebugUtilsMessengerEXT make = (PFN_vkCreateDebugUtilsMessengerEXT)vkGetInstanceProcAddr(v->inst, "vkCreateDebugUtilsMessengerEXT");
    VkDebugUtilsMessengerCreateInfoEXT mci = {.sType = VK_STRUCTURE_TYPE_DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
      .messageSeverity = VK_DEBUG_UTILS_MESSAGE_SEVERITY_WARNING_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT,
      .messageType = VK_DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_TYPE_VALIDATION_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_TYPE_PERFORMANCE_BIT_EXT,
      .pfnUserCallback = on_message};
    if (make) CHECK(make(v->inst, &mci, NULL, &v->messenger));
  }

  // The first discrete GPU with a compute queue, else any GPU with one.
  uint32_t ndev = 0;
  CHECK(vkEnumeratePhysicalDevices(v->inst, &ndev, NULL));
  VkPhysicalDevice devs[16];
  if (ndev > 16) ndev = 16;
  CHECK(vkEnumeratePhysicalDevices(v->inst, &ndev, devs));
  for (int pass = 0; pass < 2 && !v->gpu; pass++)
    for (uint32_t i = 0; i < ndev && !v->gpu; i++) {
      vkGetPhysicalDeviceProperties(devs[i], &v->props);
      if (pass == 0 && v->props.deviceType != VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU) continue;
      uint32_t nq = 0;
      vkGetPhysicalDeviceQueueFamilyProperties(devs[i], &nq, NULL);
      VkQueueFamilyProperties q[32];
      if (nq > 32) nq = 32;
      vkGetPhysicalDeviceQueueFamilyProperties(devs[i], &nq, q);
      for (uint32_t f = 0; f < nq; f++)
        if (q[f].queueFlags & VK_QUEUE_COMPUTE_BIT) { v->gpu = devs[i]; v->family = f; break; }
    }
  if (!v->gpu) { fprintf(stderr, "vkc: no GPU with a compute queue\n"); exit(1); }
  vkGetPhysicalDeviceProperties(v->gpu, &v->props);

  float priority = 1;
  VkDeviceQueueCreateInfo qci = {.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO, .queueFamilyIndex = v->family, .queueCount = 1, .pQueuePriorities = &priority};
  // Kernels from coax spirv keep f32 NaN, infinity and signed zero (float controls).
  const char *dexts[] = {VK_KHR_SHADER_FLOAT_CONTROLS_EXTENSION_NAME};
  VkDeviceCreateInfo dci = {.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO, .queueCreateInfoCount = 1, .pQueueCreateInfos = &qci,
                            .enabledExtensionCount = 1, .ppEnabledExtensionNames = dexts};
  CHECK(vkCreateDevice(v->gpu, &dci, NULL, &v->dev));
  vkGetDeviceQueue(v->dev, v->family, 0, &v->queue);
}

VkcBuffer vkc_buffer(Vkc *v, VkDeviceSize size) {
  VkcBuffer b = {.size = size};
  VkBufferCreateInfo bci = {.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .size = size, .usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT};
  CHECK(vkCreateBuffer(v->dev, &bci, NULL, &b.buf));
  VkMemoryRequirements req;
  vkGetBufferMemoryRequirements(v->dev, b.buf, &req);
  VkPhysicalDeviceMemoryProperties mem;
  vkGetPhysicalDeviceMemoryProperties(v->gpu, &mem);
  uint32_t type = UINT32_MAX;
  VkMemoryPropertyFlags want = VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
  for (uint32_t i = 0; i < mem.memoryTypeCount; i++)
    if ((req.memoryTypeBits & (1u << i)) && (mem.memoryTypes[i].propertyFlags & want) == want) { type = i; break; }
  if (type == UINT32_MAX) { fprintf(stderr, "vkc: no host-visible memory\n"); exit(1); }
  VkMemoryAllocateInfo mai = {.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, .allocationSize = req.size, .memoryTypeIndex = type};
  CHECK(vkAllocateMemory(v->dev, &mai, NULL, &b.mem));
  CHECK(vkBindBufferMemory(v->dev, b.buf, b.mem, 0));
  CHECK(vkMapMemory(v->dev, b.mem, 0, size, 0, &b.map));
  memset(b.map, 0, size);
  return b;
}

void vkc_dispatch(Vkc *v, const uint32_t *words, size_t count, const VkcBuffer *buffers, uint32_t n,
                  const void *push, uint32_t push_size, uint32_t gx, uint32_t gy) {
  VkDevice dev = v->dev;
  VkShaderModuleCreateInfo smi = {.sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO, .codeSize = count * 4, .pCode = words};
  VkShaderModule module;
  CHECK(vkCreateShaderModule(dev, &smi, NULL, &module));
  VkDescriptorSetLayoutBinding bindings[8];
  if (n > 8) { fprintf(stderr, "vkc: at most 8 buffers\n"); exit(1); }
  for (uint32_t i = 0; i < n; i++)
    bindings[i] = (VkDescriptorSetLayoutBinding){.binding = i, .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .descriptorCount = 1, .stageFlags = VK_SHADER_STAGE_COMPUTE_BIT};
  VkDescriptorSetLayoutCreateInfo dli = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO, .bindingCount = n, .pBindings = bindings};
  VkDescriptorSetLayout dsl;
  CHECK(vkCreateDescriptorSetLayout(dev, &dli, NULL, &dsl));
  VkPushConstantRange range = {.stageFlags = VK_SHADER_STAGE_COMPUTE_BIT, .offset = 0, .size = push_size};
  VkPipelineLayoutCreateInfo pli = {.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO, .setLayoutCount = 1, .pSetLayouts = &dsl,
                                    .pushConstantRangeCount = push_size ? 1u : 0u, .pPushConstantRanges = &range};
  VkPipelineLayout layout;
  CHECK(vkCreatePipelineLayout(dev, &pli, NULL, &layout));
  VkComputePipelineCreateInfo cpi = {.sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
    .stage = {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage = VK_SHADER_STAGE_COMPUTE_BIT, .module = module, .pName = "main"},
    .layout = layout};
  VkPipeline pipeline;
  CHECK(vkCreateComputePipelines(dev, VK_NULL_HANDLE, 1, &cpi, NULL, &pipeline));

  VkDescriptorPoolSize ps = {.type = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .descriptorCount = n};
  VkDescriptorPoolCreateInfo dpi = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO, .maxSets = 1, .poolSizeCount = 1, .pPoolSizes = &ps};
  VkDescriptorPool pool;
  CHECK(vkCreateDescriptorPool(dev, &dpi, NULL, &pool));
  VkDescriptorSetAllocateInfo dsai = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO, .descriptorPool = pool, .descriptorSetCount = 1, .pSetLayouts = &dsl};
  VkDescriptorSet set;
  CHECK(vkAllocateDescriptorSets(dev, &dsai, &set));
  VkDescriptorBufferInfo dbi[8];
  VkWriteDescriptorSet wds[8];
  for (uint32_t i = 0; i < n; i++) {
    dbi[i] = (VkDescriptorBufferInfo){.buffer = buffers[i].buf, .offset = 0, .range = buffers[i].size};
    wds[i] = (VkWriteDescriptorSet){.sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = set, .dstBinding = i, .descriptorCount = 1,
                                    .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .pBufferInfo = &dbi[i]};
  }
  vkUpdateDescriptorSets(dev, n, wds, 0, NULL);

  VkCommandPoolCreateInfo cpci = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .queueFamilyIndex = v->family};
  VkCommandPool cmdpool;
  CHECK(vkCreateCommandPool(dev, &cpci, NULL, &cmdpool));
  VkCommandBufferAllocateInfo cbai = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO, .commandPool = cmdpool, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1};
  VkCommandBuffer cmd;
  CHECK(vkAllocateCommandBuffers(dev, &cbai, &cmd));
  VkCommandBufferBeginInfo begin = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO, .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT};
  CHECK(vkBeginCommandBuffer(cmd, &begin));
  vkCmdBindPipeline(cmd, VK_PIPELINE_BIND_POINT_COMPUTE, pipeline);
  vkCmdBindDescriptorSets(cmd, VK_PIPELINE_BIND_POINT_COMPUTE, layout, 0, 1, &set, 0, NULL);
  if (push_size) vkCmdPushConstants(cmd, layout, VK_SHADER_STAGE_COMPUTE_BIT, 0, push_size, push);
  vkCmdDispatch(cmd, gx, gy, 1);
  VkMemoryBarrier barrier = {.sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER, .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT, .dstAccessMask = VK_ACCESS_HOST_READ_BIT};
  vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT, VK_PIPELINE_STAGE_HOST_BIT, 0, 1, &barrier, 0, NULL, 0, NULL);
  CHECK(vkEndCommandBuffer(cmd));
  VkFenceCreateInfo fci = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
  VkFence fence;
  CHECK(vkCreateFence(dev, &fci, NULL, &fence));
  VkSubmitInfo si = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .commandBufferCount = 1, .pCommandBuffers = &cmd};
  CHECK(vkQueueSubmit(v->queue, 1, &si, fence));
  CHECK(vkWaitForFences(dev, 1, &fence, VK_TRUE, 10ull * 1000 * 1000 * 1000));

  vkDestroyFence(dev, fence, NULL);
  vkDestroyCommandPool(dev, cmdpool, NULL);
  vkDestroyDescriptorPool(dev, pool, NULL);
  vkDestroyPipeline(dev, pipeline, NULL);
  vkDestroyPipelineLayout(dev, layout, NULL);
  vkDestroyDescriptorSetLayout(dev, dsl, NULL);
  vkDestroyShaderModule(dev, module, NULL);
}

void vkc_free(Vkc *v, VkcBuffer *b) {
  vkUnmapMemory(v->dev, b->mem);
  vkFreeMemory(v->dev, b->mem, NULL);
  vkDestroyBuffer(v->dev, b->buf, NULL);
}

void vkc_close(Vkc *v) {
  vkDestroyDevice(v->dev, NULL);
  if (v->messenger) {
    PFN_vkDestroyDebugUtilsMessengerEXT drop = (PFN_vkDestroyDebugUtilsMessengerEXT)vkGetInstanceProcAddr(v->inst, "vkDestroyDebugUtilsMessengerEXT");
    if (drop) drop(v->inst, v->messenger, NULL);
  }
  vkDestroyInstance(v->inst, NULL);
}
