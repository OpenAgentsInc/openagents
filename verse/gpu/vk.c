// vk-check: Coax-built SPIR-V on the GPU through Vulkan, with no display
// (docs/verse/native-gpu.md, phase G0). It asks verse/gpu/spirv.coax for
// the grade module's words, runs it over a w by h image as a compute
// dispatch, reads the pixels back and compares each with the same pixel
// computed by Coax on the CPU (vg_grade_pixel). It takes no screen, VT or
// DRM master, so it can run while someone uses the machine.
//
//   vk-check [WxH] [--ppm OUT.ppm]
// With VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation (and the layer on the
// layer path) every validation message is printed and counted; any error
// fails the run. The layers are a test aid only.
#include "spirv.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vulkan/vulkan.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vk-check: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

static int validation_errors, validation_messages;

static VKAPI_ATTR VkBool32 VKAPI_CALL on_message(VkDebugUtilsMessageSeverityFlagBitsEXT severity, VkDebugUtilsMessageTypeFlagsEXT type,
                                                 const VkDebugUtilsMessengerCallbackDataEXT *data, void *user) {
  (void)type; (void)user;
  if (severity >= VK_DEBUG_UTILS_MESSAGE_SEVERITY_WARNING_BIT_EXT) {
    validation_messages++;
    if (severity >= VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT) validation_errors++;
    fprintf(stderr, "vk-check: validation: %s\n", data->pMessage);
  }
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

static int channel_off(uint32_t a, uint32_t b) {
  for (int s = 0; s < 32; s += 8) {
    int d = (int)((a >> s) & 255) - (int)((b >> s) & 255);
    if (d > 1 || d < -1) return 1;
  }
  return 0;
}

int main(int argc, char **argv) {
  uint32_t w = 1920, h = 1080;
  const char *ppm = NULL;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--ppm") && i + 1 < argc) ppm = argv[++i];
    else if (sscanf(argv[i], "%ux%u", &w, &h) != 2) { fprintf(stderr, "usage: vk-check [WxH] [--ppm OUT.ppm]\n"); return 2; }
  }

  // The module, from Coax.
  static uint32_t words[4096];
  int64_t n = vg_grade_module((coax_v_u32){words, 4096});
  if (n <= 0) { fprintf(stderr, "vk-check: Coax refused the module\n"); return 1; }

  // An instance with no surface extensions: compute only.
  int validating = has_layer("VK_LAYER_KHRONOS_validation") && getenv("VK_INSTANCE_LAYERS");
  const char *exts[] = {VK_EXT_DEBUG_UTILS_EXTENSION_NAME};
  VkApplicationInfo app = {.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO, .pApplicationName = "vk-check", .apiVersion = VK_API_VERSION_1_1};
  VkInstanceCreateInfo ici = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo = &app,
                              .enabledExtensionCount = validating ? 1u : 0u, .ppEnabledExtensionNames = exts};
  VkInstance inst;
  CHECK(vkCreateInstance(&ici, NULL, &inst));
  VkDebugUtilsMessengerEXT messenger = VK_NULL_HANDLE;
  if (validating) {
    PFN_vkCreateDebugUtilsMessengerEXT make = (PFN_vkCreateDebugUtilsMessengerEXT)vkGetInstanceProcAddr(inst, "vkCreateDebugUtilsMessengerEXT");
    VkDebugUtilsMessengerCreateInfoEXT mci = {.sType = VK_STRUCTURE_TYPE_DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
      .messageSeverity = VK_DEBUG_UTILS_MESSAGE_SEVERITY_WARNING_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT,
      .messageType = VK_DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_TYPE_VALIDATION_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_TYPE_PERFORMANCE_BIT_EXT,
      .pfnUserCallback = on_message};
    if (make) CHECK(make(inst, &mci, NULL, &messenger));
  }

  // The first discrete GPU with a compute queue.
  uint32_t ndev = 0;
  CHECK(vkEnumeratePhysicalDevices(inst, &ndev, NULL));
  VkPhysicalDevice devs[16], gpu = VK_NULL_HANDLE;
  if (ndev > 16) ndev = 16;
  CHECK(vkEnumeratePhysicalDevices(inst, &ndev, devs));
  uint32_t family = 0;
  VkPhysicalDeviceProperties props = {0};
  for (int pass = 0; pass < 2 && !gpu; pass++)
    for (uint32_t i = 0; i < ndev && !gpu; i++) {
      vkGetPhysicalDeviceProperties(devs[i], &props);
      if (pass == 0 && props.deviceType != VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU) continue;
      uint32_t nq = 0;
      vkGetPhysicalDeviceQueueFamilyProperties(devs[i], &nq, NULL);
      VkQueueFamilyProperties q[32];
      if (nq > 32) nq = 32;
      vkGetPhysicalDeviceQueueFamilyProperties(devs[i], &nq, q);
      for (uint32_t f = 0; f < nq; f++)
        if (q[f].queueFlags & VK_QUEUE_COMPUTE_BIT) { gpu = devs[i]; family = f; break; }
    }
  if (!gpu) { fprintf(stderr, "vk-check: no GPU with a compute queue\n"); return 1; }
  vkGetPhysicalDeviceProperties(gpu, &props);

  float priority = 1;
  VkDeviceQueueCreateInfo qci = {.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO, .queueFamilyIndex = family, .queueCount = 1, .pQueuePriorities = &priority};
  VkDeviceCreateInfo dci = {.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO, .queueCreateInfoCount = 1, .pQueueCreateInfos = &qci};
  VkDevice dev;
  CHECK(vkCreateDevice(gpu, &dci, NULL, &dev));
  VkQueue queue;
  vkGetDeviceQueue(dev, family, 0, &queue);

  // The pixels: a host-visible storage buffer.
  VkDeviceSize size = (VkDeviceSize)w * h * 4;
  VkBufferCreateInfo bci = {.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .size = size, .usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT};
  VkBuffer buf;
  CHECK(vkCreateBuffer(dev, &bci, NULL, &buf));
  VkMemoryRequirements req;
  vkGetBufferMemoryRequirements(dev, buf, &req);
  VkPhysicalDeviceMemoryProperties mem;
  vkGetPhysicalDeviceMemoryProperties(gpu, &mem);
  uint32_t type = UINT32_MAX;
  VkMemoryPropertyFlags want = VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
  for (uint32_t i = 0; i < mem.memoryTypeCount; i++)
    if ((req.memoryTypeBits & (1u << i)) && (mem.memoryTypes[i].propertyFlags & want) == want) { type = i; break; }
  if (type == UINT32_MAX) { fprintf(stderr, "vk-check: no host-visible memory\n"); return 1; }
  VkMemoryAllocateInfo mai = {.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, .allocationSize = req.size, .memoryTypeIndex = type};
  VkDeviceMemory memory;
  CHECK(vkAllocateMemory(dev, &mai, NULL, &memory));
  CHECK(vkBindBufferMemory(dev, buf, memory, 0));
  uint32_t *pixels;
  CHECK(vkMapMemory(dev, memory, 0, size, 0, (void **)&pixels));
  memset(pixels, 0, size);

  // The pipeline: Coax's words, one storage buffer, the size as push constants.
  VkShaderModuleCreateInfo smi = {.sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO, .codeSize = (size_t)n * 4, .pCode = words};
  VkShaderModule module;
  CHECK(vkCreateShaderModule(dev, &smi, NULL, &module));
  VkDescriptorSetLayoutBinding binding = {.binding = 0, .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .descriptorCount = 1, .stageFlags = VK_SHADER_STAGE_COMPUTE_BIT};
  VkDescriptorSetLayoutCreateInfo dli = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO, .bindingCount = 1, .pBindings = &binding};
  VkDescriptorSetLayout dsl;
  CHECK(vkCreateDescriptorSetLayout(dev, &dli, NULL, &dsl));
  VkPushConstantRange range = {.stageFlags = VK_SHADER_STAGE_COMPUTE_BIT, .offset = 0, .size = 8};
  VkPipelineLayoutCreateInfo pli = {.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO, .setLayoutCount = 1, .pSetLayouts = &dsl,
                                    .pushConstantRangeCount = 1, .pPushConstantRanges = &range};
  VkPipelineLayout layout;
  CHECK(vkCreatePipelineLayout(dev, &pli, NULL, &layout));
  VkComputePipelineCreateInfo cpi = {.sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
    .stage = {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage = VK_SHADER_STAGE_COMPUTE_BIT, .module = module, .pName = "main"},
    .layout = layout};
  VkPipeline pipeline;
  CHECK(vkCreateComputePipelines(dev, VK_NULL_HANDLE, 1, &cpi, NULL, &pipeline));

  VkDescriptorPoolSize ps = {.type = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .descriptorCount = 1};
  VkDescriptorPoolCreateInfo dpi = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO, .maxSets = 1, .poolSizeCount = 1, .pPoolSizes = &ps};
  VkDescriptorPool pool;
  CHECK(vkCreateDescriptorPool(dev, &dpi, NULL, &pool));
  VkDescriptorSetAllocateInfo dsai = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO, .descriptorPool = pool, .descriptorSetCount = 1, .pSetLayouts = &dsl};
  VkDescriptorSet set;
  CHECK(vkAllocateDescriptorSets(dev, &dsai, &set));
  VkDescriptorBufferInfo dbi = {.buffer = buf, .offset = 0, .range = size};
  VkWriteDescriptorSet wds = {.sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = set, .dstBinding = 0, .descriptorCount = 1,
                              .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .pBufferInfo = &dbi};
  vkUpdateDescriptorSets(dev, 1, &wds, 0, NULL);

  // One dispatch over the image.
  VkCommandPoolCreateInfo cpci = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .queueFamilyIndex = family};
  VkCommandPool cmdpool;
  CHECK(vkCreateCommandPool(dev, &cpci, NULL, &cmdpool));
  VkCommandBufferAllocateInfo cbai = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO, .commandPool = cmdpool, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1};
  VkCommandBuffer cmd;
  CHECK(vkAllocateCommandBuffers(dev, &cbai, &cmd));
  VkCommandBufferBeginInfo begin = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO, .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT};
  CHECK(vkBeginCommandBuffer(cmd, &begin));
  vkCmdBindPipeline(cmd, VK_PIPELINE_BIND_POINT_COMPUTE, pipeline);
  vkCmdBindDescriptorSets(cmd, VK_PIPELINE_BIND_POINT_COMPUTE, layout, 0, 1, &set, 0, NULL);
  uint32_t push[2] = {w, h};
  vkCmdPushConstants(cmd, layout, VK_SHADER_STAGE_COMPUTE_BIT, 0, sizeof push, push);
  vkCmdDispatch(cmd, (w + 7) / 8, (h + 7) / 8, 1);
  VkMemoryBarrier barrier = {.sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER, .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT, .dstAccessMask = VK_ACCESS_HOST_READ_BIT};
  vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT, VK_PIPELINE_STAGE_HOST_BIT, 0, 1, &barrier, 0, NULL, 0, NULL);
  CHECK(vkEndCommandBuffer(cmd));
  VkFenceCreateInfo fci = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
  VkFence fence;
  CHECK(vkCreateFence(dev, &fci, NULL, &fence));
  VkSubmitInfo si = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .commandBufferCount = 1, .pCommandBuffers = &cmd};
  CHECK(vkQueueSubmit(queue, 1, &si, fence));
  CHECK(vkWaitForFences(dev, 1, &fence, VK_TRUE, 10ull * 1000 * 1000 * 1000));

  // Every pixel against Coax on the CPU.
  uint64_t off = 0, exact = 0, step = 0;
  for (uint32_t y = 0; y < h; y++)
    for (uint32_t x = 0; x < w; x++) {
      uint32_t want_px = vg_grade_pixel(x, y, w, h), got = pixels[(size_t)y * w + x];
      if (got == want_px) exact++;
      else if (channel_off(got, want_px)) off++;
      else step++;
    }
  if (ppm) {
    FILE *f = fopen(ppm, "wb");
    if (f) {
      fprintf(f, "P6\n%u %u\n255\n", w, h);
      for (size_t i = 0; i < (size_t)w * h; i++) { unsigned char c[3] = {(unsigned char)pixels[i], (unsigned char)(pixels[i] >> 8), (unsigned char)(pixels[i] >> 16)}; fwrite(c, 1, 3, f); }
      fclose(f);
    }
  }
  printf("vk-check: %s, %u words of Coax SPIR-V, %ux%u: %llu exact, %llu one rounding step off, %llu wrong%s\n", props.deviceName, (unsigned)n, w, h,
         (unsigned long long)exact, (unsigned long long)step, (unsigned long long)off,
         validating ? "" : " (validation layers not loaded)");
  if (validating) printf("vk-check: validation: %d errors, %d warnings\n", validation_errors, validation_messages - validation_errors);

  vkDestroyFence(dev, fence, NULL);
  vkDestroyCommandPool(dev, cmdpool, NULL);
  vkDestroyDescriptorPool(dev, pool, NULL);
  vkDestroyPipeline(dev, pipeline, NULL);
  vkDestroyPipelineLayout(dev, layout, NULL);
  vkDestroyDescriptorSetLayout(dev, dsl, NULL);
  vkDestroyShaderModule(dev, module, NULL);
  vkUnmapMemory(dev, memory);
  vkFreeMemory(dev, memory, NULL);
  vkDestroyBuffer(dev, buf, NULL);
  vkDestroyDevice(dev, NULL);
  if (messenger) {
    PFN_vkDestroyDebugUtilsMessengerEXT drop = (PFN_vkDestroyDebugUtilsMessengerEXT)vkGetInstanceProcAddr(inst, "vkDestroyDebugUtilsMessengerEXT");
    if (drop) drop(inst, messenger, NULL);
  }
  vkDestroyInstance(inst, NULL);
  return off == 0 && validation_errors == 0 ? 0 : 1;
}
