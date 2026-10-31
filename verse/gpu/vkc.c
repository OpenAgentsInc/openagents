// The headless Vulkan device (vkc.h).
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

void vkc_open(Vkc *v) { vkc_open_with(v, NULL, 0, NULL, 0); }

void vkc_open_with(Vkc *v, const char *const *instance_exts, uint32_t ni, const char *const *device_exts, uint32_t nd) {
  memset(v, 0, sizeof *v);
  // An instance with only the surface extensions asked for; none is headless.
  v->validating = has_layer("VK_LAYER_KHRONOS_validation") && getenv("VK_INSTANCE_LAYERS");
  const char *exts[8];
  uint32_t ne = 0;
  if (ni > 7) { fprintf(stderr, "vkc: at most 7 instance extensions\n"); exit(1); }
  for (uint32_t i = 0; i < ni; i++) exts[ne++] = instance_exts[i];
  if (v->validating) exts[ne++] = VK_EXT_DEBUG_UTILS_EXTENSION_NAME;
  VkApplicationInfo app = {.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO, .pApplicationName = "verse-gpu", .apiVersion = VK_API_VERSION_1_1};
  VkInstanceCreateInfo ici = {.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo = &app,
                              .enabledExtensionCount = ne, .ppEnabledExtensionNames = exts};
  CHECK(vkCreateInstance(&ici, NULL, &v->inst));
  if (v->validating) {
    PFN_vkCreateDebugUtilsMessengerEXT make = (PFN_vkCreateDebugUtilsMessengerEXT)vkGetInstanceProcAddr(v->inst, "vkCreateDebugUtilsMessengerEXT");
    VkDebugUtilsMessengerCreateInfoEXT mci = {.sType = VK_STRUCTURE_TYPE_DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
      .messageSeverity = VK_DEBUG_UTILS_MESSAGE_SEVERITY_WARNING_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_SEVERITY_ERROR_BIT_EXT,
      .messageType = VK_DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_TYPE_VALIDATION_BIT_EXT | VK_DEBUG_UTILS_MESSAGE_TYPE_PERFORMANCE_BIT_EXT,
      .pfnUserCallback = on_message};
    if (make) CHECK(make(v->inst, &mci, NULL, &v->messenger));
  }

  // The first discrete GPU with a graphics and compute queue, else any GPU with one.
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
        if ((q[f].queueFlags & (VK_QUEUE_COMPUTE_BIT | VK_QUEUE_GRAPHICS_BIT)) == (VK_QUEUE_COMPUTE_BIT | VK_QUEUE_GRAPHICS_BIT)) { v->gpu = devs[i]; v->family = f; break; }
    }
  if (!v->gpu) { fprintf(stderr, "vkc: no GPU with a graphics and compute queue\n"); exit(1); }
  vkGetPhysicalDeviceProperties(v->gpu, &v->props);

  float priority = 1;
  VkDeviceQueueCreateInfo qci = {.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO, .queueFamilyIndex = v->family, .queueCount = 1, .pQueuePriorities = &priority};
  // Kernels and draws from coax spirv keep f32 NaN, infinity and signed zero (float controls).
  const char *dexts[8] = {VK_KHR_SHADER_FLOAT_CONTROLS_EXTENSION_NAME};
  if (nd > 7) { fprintf(stderr, "vkc: at most 7 device extensions\n"); exit(1); }
  for (uint32_t i = 0; i < nd; i++) dexts[1 + i] = device_exts[i];
  // Shaders from coax spirv --vertex/--fragment use 64-bit integers for Coax's int.
  VkPhysicalDeviceFeatures has, want = {0};
  vkGetPhysicalDeviceFeatures(v->gpu, &has);
  want.shaderInt64 = has.shaderInt64;
  VkDeviceCreateInfo dci = {.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO, .queueCreateInfoCount = 1, .pQueueCreateInfos = &qci,
                            .enabledExtensionCount = 1 + nd, .ppEnabledExtensionNames = dexts, .pEnabledFeatures = &want};
  CHECK(vkCreateDevice(v->gpu, &dci, NULL, &v->dev));
  vkGetDeviceQueue(v->dev, v->family, 0, &v->queue);
}

VkDeviceMemory vkc_memory(Vkc *v, VkMemoryRequirements req, VkMemoryPropertyFlags want) {
  VkPhysicalDeviceMemoryProperties mem;
  vkGetPhysicalDeviceMemoryProperties(v->gpu, &mem);
  uint32_t type = UINT32_MAX;
  for (uint32_t i = 0; i < mem.memoryTypeCount; i++)
    if ((req.memoryTypeBits & (1u << i)) && (mem.memoryTypes[i].propertyFlags & want) == want) { type = i; break; }
  if (type == UINT32_MAX) { fprintf(stderr, "vkc: no memory of type %#x\n", (unsigned)want); exit(1); }
  VkMemoryAllocateInfo mai = {.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, .allocationSize = req.size, .memoryTypeIndex = type};
  VkDeviceMemory m;
  CHECK(vkAllocateMemory(v->dev, &mai, NULL, &m));
  return m;
}

VkcBuffer vkc_buffer(Vkc *v, VkDeviceSize size) {
  VkcBuffer b = {.size = size};
  VkBufferCreateInfo bci = {.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .size = size,
                            .usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | VK_BUFFER_USAGE_VERTEX_BUFFER_BIT | VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT};
  CHECK(vkCreateBuffer(v->dev, &bci, NULL, &b.buf));
  VkMemoryRequirements req;
  vkGetBufferMemoryRequirements(v->dev, b.buf, &req);
  b.mem = vkc_memory(v, req, VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);
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

// ---- draws

static VkShaderModule shader(Vkc *v, const VkcPass *p) {
  VkShaderModuleCreateInfo smi = {.sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO, .codeSize = p->count * 4, .pCode = p->words};
  VkShaderModule m;
  CHECK(vkCreateShaderModule(v->dev, &smi, NULL, &m));
  return m;
}

static VkPipeline pipeline(Vkc *v, const VkcPass *p, VkShaderModule m, VkRenderPass rp, VkPipelineLayout layout, uint32_t w, uint32_t h) {
  VkPipelineShaderStageCreateInfo stages[2] = {
    {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage = VK_SHADER_STAGE_VERTEX_BIT, .module = m, .pName = "vs"},
    {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage = VK_SHADER_STAGE_FRAGMENT_BIT, .module = m, .pName = "fs"}};
  // A stroke vertex: position, place, shape, tint (spirv.coax, SV_STROKE).
  VkVertexInputBindingDescription bind = {.binding = 0, .stride = p->stride, .inputRate = VK_VERTEX_INPUT_RATE_VERTEX};
  VkVertexInputAttributeDescription attrs[4] = {
    {.location = 0, .binding = 0, .format = VK_FORMAT_R32G32_SFLOAT, .offset = 0},
    {.location = 1, .binding = 0, .format = VK_FORMAT_R32G32_SFLOAT, .offset = 8},
    {.location = 2, .binding = 0, .format = VK_FORMAT_R32G32B32A32_SFLOAT, .offset = 16},
    {.location = 3, .binding = 0, .format = VK_FORMAT_R32G32B32A32_SFLOAT, .offset = 32}};
  VkPipelineVertexInputStateCreateInfo vin = {.sType = VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
    .vertexBindingDescriptionCount = p->vertices ? 1u : 0u, .pVertexBindingDescriptions = &bind,
    .vertexAttributeDescriptionCount = p->vertices ? 4u : 0u, .pVertexAttributeDescriptions = attrs};
  VkPipelineInputAssemblyStateCreateInfo ia = {.sType = VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO, .topology = VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST};
  VkViewport vp = {0, 0, (float)w, (float)h, 0, 1};
  VkRect2D sc = {{0, 0}, {w, h}};
  VkPipelineViewportStateCreateInfo vps = {.sType = VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO, .viewportCount = 1, .pViewports = &vp, .scissorCount = 1, .pScissors = &sc};
  VkPipelineRasterizationStateCreateInfo rs = {.sType = VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
    .polygonMode = VK_POLYGON_MODE_FILL, .cullMode = VK_CULL_MODE_NONE, .frontFace = VK_FRONT_FACE_COUNTER_CLOCKWISE, .lineWidth = 1};
  VkPipelineMultisampleStateCreateInfo ms = {.sType = VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO, .rasterizationSamples = VK_SAMPLE_COUNT_1_BIT};
  // Additive: the color plus what is there, as kms.c adds strokes.
  VkPipelineColorBlendAttachmentState blend = {.blendEnable = p->additive ? VK_TRUE : VK_FALSE,
    .srcColorBlendFactor = VK_BLEND_FACTOR_ONE, .dstColorBlendFactor = VK_BLEND_FACTOR_ONE, .colorBlendOp = VK_BLEND_OP_ADD,
    .srcAlphaBlendFactor = VK_BLEND_FACTOR_ONE, .dstAlphaBlendFactor = VK_BLEND_FACTOR_ONE, .alphaBlendOp = VK_BLEND_OP_ADD,
    .colorWriteMask = 0xF};
  VkPipelineColorBlendStateCreateInfo cb = {.sType = VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO, .attachmentCount = 1, .pAttachments = &blend};
  VkGraphicsPipelineCreateInfo gpi = {.sType = VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO, .stageCount = 2, .pStages = stages,
    .pVertexInputState = &vin, .pInputAssemblyState = &ia, .pViewportState = &vps, .pRasterizationState = &rs,
    .pMultisampleState = &ms, .pColorBlendState = &cb, .layout = layout, .renderPass = rp, .subpass = 0};
  VkPipeline pl;
  CHECK(vkCreateGraphicsPipelines(v->dev, VK_NULL_HANDLE, 1, &gpi, NULL, &pl));
  return pl;
}

void vkc_target_open(Vkc *v, VkcTarget *t, uint32_t w, uint32_t h, const VkcPass *passes, uint32_t n) {
  VkDevice dev = v->dev;
  memset(t, 0, sizeof *t);
  t->w = w; t->h = h; t->n = n;
  const VkFormat format = VK_FORMAT_R32G32B32A32_SFLOAT;
  VkImageCreateInfo ici = {.sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO, .imageType = VK_IMAGE_TYPE_2D, .format = format,
    .extent = {w, h, 1}, .mipLevels = 1, .arrayLayers = 1, .samples = VK_SAMPLE_COUNT_1_BIT, .tiling = VK_IMAGE_TILING_OPTIMAL,
    .usage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT, .initialLayout = VK_IMAGE_LAYOUT_UNDEFINED};
  CHECK(vkCreateImage(dev, &ici, NULL, &t->image));
  VkMemoryRequirements req;
  vkGetImageMemoryRequirements(dev, t->image, &req);
  t->mem = vkc_memory(v, req, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
  CHECK(vkBindImageMemory(dev, t->image, t->mem, 0));
  VkImageViewCreateInfo ivi = {.sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO, .image = t->image, .viewType = VK_IMAGE_VIEW_TYPE_2D, .format = format,
    .subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1}};
  CHECK(vkCreateImageView(dev, &ivi, NULL, &t->view));

  // One subpass; the image ends ready to copy or blit out.
  VkAttachmentDescription att = {.format = format, .samples = VK_SAMPLE_COUNT_1_BIT, .loadOp = VK_ATTACHMENT_LOAD_OP_CLEAR,
    .storeOp = VK_ATTACHMENT_STORE_OP_STORE, .stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE, .stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE,
    .initialLayout = VK_IMAGE_LAYOUT_UNDEFINED, .finalLayout = VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL};
  VkAttachmentReference ref = {0, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL};
  VkSubpassDescription sub = {.pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS, .colorAttachmentCount = 1, .pColorAttachments = &ref};
  // The last frame's copy or blit reads the image before this frame clears it.
  VkSubpassDependency deps[2] = {
    {.srcSubpass = VK_SUBPASS_EXTERNAL, .dstSubpass = 0,
     .srcStageMask = VK_PIPELINE_STAGE_TRANSFER_BIT, .dstStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
     .srcAccessMask = 0, .dstAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT},
    {.srcSubpass = 0, .dstSubpass = VK_SUBPASS_EXTERNAL,
     .srcStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT, .dstStageMask = VK_PIPELINE_STAGE_TRANSFER_BIT,
     .srcAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT, .dstAccessMask = VK_ACCESS_TRANSFER_READ_BIT}};
  VkRenderPassCreateInfo rpi = {.sType = VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO, .attachmentCount = 1, .pAttachments = &att,
    .subpassCount = 1, .pSubpasses = &sub, .dependencyCount = 2, .pDependencies = deps};
  CHECK(vkCreateRenderPass(dev, &rpi, NULL, &t->pass));
  VkFramebufferCreateInfo fbi = {.sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO, .renderPass = t->pass, .attachmentCount = 1, .pAttachments = &t->view,
    .width = w, .height = h, .layers = 1};
  CHECK(vkCreateFramebuffer(dev, &fbi, NULL, &t->fb));

  if (n > 8) { fprintf(stderr, "vkc: at most 8 passes\n"); exit(1); }
  for (uint32_t i = 0; i < n; i++) {
    t->modules[i] = shader(v, &passes[i]);
    VkPushConstantRange range = {.stageFlags = VK_SHADER_STAGE_VERTEX_BIT | VK_SHADER_STAGE_FRAGMENT_BIT, .offset = 0, .size = passes[i].push_size};
    VkPipelineLayoutCreateInfo pli = {.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
                                      .pushConstantRangeCount = passes[i].push_size ? 1u : 0u, .pPushConstantRanges = &range};
    CHECK(vkCreatePipelineLayout(dev, &pli, NULL, &t->layouts[i]));
    t->pipelines[i] = pipeline(v, &passes[i], t->modules[i], t->pass, t->layouts[i], w, h);
  }
}

void vkc_target_record(VkcTarget *t, VkCommandBuffer cmd, const VkcPass *passes) {
  VkClearValue clear = {.color = {{0, 0, 0, 0}}};
  VkRenderPassBeginInfo rbi = {.sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO, .renderPass = t->pass, .framebuffer = t->fb,
                               .renderArea = {{0, 0}, {t->w, t->h}}, .clearValueCount = 1, .pClearValues = &clear};
  vkCmdBeginRenderPass(cmd, &rbi, VK_SUBPASS_CONTENTS_INLINE);
  for (uint32_t i = 0; i < t->n; i++) {
    if (!passes[i].vertex_count) continue;
    vkCmdBindPipeline(cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, t->pipelines[i]);
    if (passes[i].push_size)
      vkCmdPushConstants(cmd, t->layouts[i], VK_SHADER_STAGE_VERTEX_BIT | VK_SHADER_STAGE_FRAGMENT_BIT, 0, passes[i].push_size, passes[i].push);
    if (passes[i].vertices) { VkDeviceSize zero = 0; vkCmdBindVertexBuffers(cmd, 0, 1, &passes[i].vertices->buf, &zero); }
    vkCmdDraw(cmd, passes[i].vertex_count, 1, 0, 0);
  }
  vkCmdEndRenderPass(cmd);
}

void vkc_target_close(Vkc *v, VkcTarget *t) {
  VkDevice dev = v->dev;
  for (uint32_t i = 0; i < t->n; i++) {
    vkDestroyPipeline(dev, t->pipelines[i], NULL);
    vkDestroyPipelineLayout(dev, t->layouts[i], NULL);
    vkDestroyShaderModule(dev, t->modules[i], NULL);
  }
  vkDestroyFramebuffer(dev, t->fb, NULL);
  vkDestroyRenderPass(dev, t->pass, NULL);
  vkDestroyImageView(dev, t->view, NULL);
  vkDestroyImage(dev, t->image, NULL);
  vkFreeMemory(dev, t->mem, NULL);
}

void vkc_render(Vkc *v, uint32_t w, uint32_t h, const VkcPass *passes, uint32_t n, float *rgba) {
  VkDevice dev = v->dev;
  VkcTarget t;
  vkc_target_open(v, &t, w, h, passes, n);
  VkcBuffer readback = vkc_buffer(v, (VkDeviceSize)w * h * 16);
  VkCommandPoolCreateInfo cpci = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .queueFamilyIndex = v->family};
  VkCommandPool cmdpool;
  CHECK(vkCreateCommandPool(dev, &cpci, NULL, &cmdpool));
  VkCommandBufferAllocateInfo cbai = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO, .commandPool = cmdpool, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1};
  VkCommandBuffer cmd;
  CHECK(vkAllocateCommandBuffers(dev, &cbai, &cmd));
  VkCommandBufferBeginInfo begin = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO, .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT};
  CHECK(vkBeginCommandBuffer(cmd, &begin));
  vkc_target_record(&t, cmd, passes);
  VkBufferImageCopy copy = {.imageSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .imageExtent = {w, h, 1}};
  vkCmdCopyImageToBuffer(cmd, t.image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, readback.buf, 1, &copy);
  VkMemoryBarrier barrier = {.sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER, .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT, .dstAccessMask = VK_ACCESS_HOST_READ_BIT};
  vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_HOST_BIT, 0, 1, &barrier, 0, NULL, 0, NULL);
  CHECK(vkEndCommandBuffer(cmd));
  VkFenceCreateInfo fci = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
  VkFence fence;
  CHECK(vkCreateFence(dev, &fci, NULL, &fence));
  VkSubmitInfo si = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .commandBufferCount = 1, .pCommandBuffers = &cmd};
  CHECK(vkQueueSubmit(v->queue, 1, &si, fence));
  CHECK(vkWaitForFences(dev, 1, &fence, VK_TRUE, 10ull * 1000 * 1000 * 1000));
  memcpy(rgba, readback.map, (size_t)w * h * 16);
  vkDestroyFence(dev, fence, NULL);
  vkDestroyCommandPool(dev, cmdpool, NULL);
  vkc_free(v, &readback);
  vkc_target_close(v, &t);
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
