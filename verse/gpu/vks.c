// Scene pipelines (vks.h).
#include "vks.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vks: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

// A blend mode's source (dst 0) or destination (dst 1) factor (vks.h).
static VkBlendFactor factor(int mode, int dst) {
  static const VkBlendFactor f[7][2] = {{VK_BLEND_FACTOR_ONE, VK_BLEND_FACTOR_ZERO}, {VK_BLEND_FACTOR_ONE, VK_BLEND_FACTOR_ONE},
    {VK_BLEND_FACTOR_SRC_ALPHA, VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA}, {VK_BLEND_FACTOR_ZERO, VK_BLEND_FACTOR_SRC_COLOR},
    {VK_BLEND_FACTOR_ONE, VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA}, {VK_BLEND_FACTOR_ZERO, VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA},
    {VK_BLEND_FACTOR_ZERO, VK_BLEND_FACTOR_ONE}};
  return f[mode >= 0 && mode < 7 ? mode : 0][dst];
}
static int is_depth(VkFormat f) { return f == VK_FORMAT_D32_SFLOAT || f == VK_FORMAT_D16_UNORM || f == VK_FORMAT_D24_UNORM_S8_UINT; }
static VkImageAspectFlags aspect(const VksImage *t) { return t->depth ? VK_IMAGE_ASPECT_DEPTH_BIT : VK_IMAGE_ASPECT_COLOR_BIT; }

VksImage vks_image(Vkc *v, uint32_t w, uint32_t h, uint32_t layers, int array, VkFormat format, int use) {
  return vks_image_levels(v, w, h, layers, array, format, use, 1);
}
VksImage vks_image_levels(Vkc *v, uint32_t w, uint32_t h, uint32_t layers, int array, VkFormat format, int use, uint32_t levels) {
  VksImage t = {.format = format, .w = w, .h = h, .layers = layers, .levels = levels ? levels : 1, .depth = is_depth(format)};
  VkImageUsageFlags usage = VK_IMAGE_USAGE_SAMPLED_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT;
  if (use & VKS_TARGET) usage |= t.depth ? VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT : VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
  VkImageCreateInfo ici = {.sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO, .imageType = VK_IMAGE_TYPE_2D, .format = format,
    .extent = {w, h, 1}, .mipLevels = t.levels, .arrayLayers = layers, .samples = VK_SAMPLE_COUNT_1_BIT, .tiling = VK_IMAGE_TILING_OPTIMAL,
    .usage = usage, .initialLayout = VK_IMAGE_LAYOUT_UNDEFINED};
  CHECK(vkCreateImage(v->dev, &ici, NULL, &t.image));
  VkMemoryRequirements req;
  vkGetImageMemoryRequirements(v->dev, t.image, &req);
  t.mem = vkc_memory(v, req, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
  CHECK(vkBindImageMemory(v->dev, t.image, t.mem, 0));
  VkImageViewCreateInfo ivi = {.sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO, .image = t.image,
    .viewType = layers > 1 || array ? VK_IMAGE_VIEW_TYPE_2D_ARRAY : VK_IMAGE_VIEW_TYPE_2D, .format = format,
    .subresourceRange = {aspect(&t), 0, t.levels, 0, layers}};
  CHECK(vkCreateImageView(v->dev, &ivi, NULL, &t.view));
  return t;
}

void vks_image_free(Vkc *v, VksImage *t) {
  vkDestroyImageView(v->dev, t->view, NULL);
  vkDestroyImage(v->dev, t->image, NULL);
  vkFreeMemory(v->dev, t->mem, NULL);
}

void vks_submit(Vkc *v, void (*record)(VkCommandBuffer cmd, void *user), void *user) {
  VkDevice dev = v->dev;
  VkCommandPoolCreateInfo cpci = {.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .queueFamilyIndex = v->family};
  VkCommandPool pool;
  CHECK(vkCreateCommandPool(dev, &cpci, NULL, &pool));
  VkCommandBufferAllocateInfo cbai = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO, .commandPool = pool,
                                      .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1};
  VkCommandBuffer cmd;
  CHECK(vkAllocateCommandBuffers(dev, &cbai, &cmd));
  VkCommandBufferBeginInfo begin = {.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO, .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT};
  CHECK(vkBeginCommandBuffer(cmd, &begin));
  record(cmd, user);
  CHECK(vkEndCommandBuffer(cmd));
  VkFenceCreateInfo fci = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
  VkFence fence;
  CHECK(vkCreateFence(dev, &fci, NULL, &fence));
  VkSubmitInfo si = {.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .commandBufferCount = 1, .pCommandBuffers = &cmd};
  CHECK(vkQueueSubmit(v->queue, 1, &si, fence));
  CHECK(vkWaitForFences(dev, 1, &fence, VK_TRUE, 10ull * 1000 * 1000 * 1000));
  vkDestroyFence(dev, fence, NULL);
  vkDestroyCommandPool(dev, pool, NULL);
}

static void barrier(VkCommandBuffer cmd, const VksImage *t, VkImageLayout from, VkImageLayout to,
                    VkAccessFlags src, VkAccessFlags dst, VkPipelineStageFlags src_stage, VkPipelineStageFlags dst_stage) {
  VkImageMemoryBarrier b = {.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER, .srcAccessMask = src, .dstAccessMask = dst,
    .oldLayout = from, .newLayout = to, .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED,
    .image = t->image, .subresourceRange = {aspect(t), 0, t->levels, 0, t->layers}};
  vkCmdPipelineBarrier(cmd, src_stage, dst_stage, 0, 0, NULL, 0, NULL, 1, &b);
}

typedef struct { VksImage *t; VkcBuffer *staging; int read; uint32_t texel; } Copy;
static void record_copy(VkCommandBuffer cmd, void *user) {
  Copy *c = user;
  // reading: layer 0 of level 0; writing: every level, each with all its layers
  VkBufferImageCopy regions[16];
  uint32_t nr = c->read ? 1 : c->t->levels;
  VkDeviceSize at = 0;
  for (uint32_t l = 0; l < nr && l < 16; l++) {
    uint32_t lw = c->t->w >> l ? c->t->w >> l : 1, lh = c->t->h >> l ? c->t->h >> l : 1;
    regions[l] = (VkBufferImageCopy){.bufferOffset = at, .imageSubresource = {aspect(c->t), l, 0, c->read ? 1 : c->t->layers}, .imageExtent = {lw, lh, 1}};
    at += (VkDeviceSize)lw * lh * c->t->layers * c->texel;
  }
  VkBufferImageCopy region = regions[0];
  if (c->read) {
    barrier(cmd, c->t, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_ACCESS_SHADER_READ_BIT,
            VK_ACCESS_TRANSFER_READ_BIT, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT);
    vkCmdCopyImageToBuffer(cmd, c->t->image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, c->staging->buf, 1, &region);
    barrier(cmd, c->t, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_ACCESS_TRANSFER_READ_BIT,
            VK_ACCESS_SHADER_READ_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT);
    VkMemoryBarrier host = {.sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER, .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT, .dstAccessMask = VK_ACCESS_HOST_READ_BIT};
    vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_HOST_BIT, 0, 1, &host, 0, NULL, 0, NULL);
  } else {
    barrier(cmd, c->t, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 0, VK_ACCESS_TRANSFER_WRITE_BIT,
            VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT);
    vkCmdCopyBufferToImage(cmd, c->staging->buf, c->t->image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, nr, regions);
    barrier(cmd, c->t, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_ACCESS_TRANSFER_WRITE_BIT,
            VK_ACCESS_SHADER_READ_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT);
  }
}

void vks_upload(Vkc *v, VksImage *t, const void *bytes, size_t size) {
  VkcBuffer staging = vkc_buffer(v, size);
  memcpy(staging.map, bytes, size);
  // the bytes a texel takes, from what level 0's layers would take without mips
  uint64_t px = 0;
  for (uint32_t l = 0; l < t->levels; l++) { uint64_t lw = t->w >> l ? t->w >> l : 1, lh = t->h >> l ? t->h >> l : 1; px += lw * lh * t->layers; }
  if (t->levels > 16 || size % px) { fprintf(stderr, "vks: %zu bytes are not whole texels of %u levels\n", size, t->levels); exit(1); }
  Copy c = {t, &staging, 0, (uint32_t)(size / px)};
  vks_submit(v, record_copy, &c);
  vkc_free(v, &staging);
}

void vks_read(Vkc *v, VksImage *t, void *bytes, size_t size) {
  VkcBuffer staging = vkc_buffer(v, size);
  Copy c = {t, &staging, 1, 0};
  vks_submit(v, record_copy, &c);
  memcpy(bytes, staging.map, size);
  vkc_free(v, &staging);
}

VkSampler vks_sampler(Vkc *v, int filter, int repeat, int compare) {
  return vks_sampler_with(v, filter, repeat, compare, VK_COMPARE_OP_LESS_OR_EQUAL, 0);
}
VkSampler vks_sampler_with(Vkc *v, int filter, int repeat, int compare, VkCompareOp op, int mip) {
  VkFilter f = filter == VKS_LINEAR ? VK_FILTER_LINEAR : VK_FILTER_NEAREST;
  VkSamplerAddressMode mode = repeat ? VK_SAMPLER_ADDRESS_MODE_REPEAT : VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE;
  VkSamplerCreateInfo sci = {.sType = VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO, .magFilter = f, .minFilter = f,
    .mipmapMode = mip ? VK_SAMPLER_MIPMAP_MODE_LINEAR : VK_SAMPLER_MIPMAP_MODE_NEAREST, .addressModeU = mode, .addressModeV = mode, .addressModeW = mode,
    .compareEnable = compare ? VK_TRUE : VK_FALSE, .compareOp = op, .maxLod = VK_LOD_CLAMP_NONE};
  VkSampler s;
  CHECK(vkCreateSampler(v->dev, &sci, NULL, &s));
  return s;
}

// ---- pipelines

static VkDescriptorType descriptor(int kind) {
  return kind == VKS_STORAGE ? VK_DESCRIPTOR_TYPE_STORAGE_BUFFER : kind == VKS_IMAGE ? VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE : VK_DESCRIPTOR_TYPE_SAMPLER;
}

void vks_pipe_open(Vkc *v, VksPipe *p, const VksPass *pass) {
  VkDevice dev = v->dev;
  memset(p, 0, sizeof *p);
  const VksImage *first = pass->ntargets ? pass->targets[0] : pass->depth;
  if (!first || pass->ntargets > 8 || pass->nb > 32) { fprintf(stderr, "vks: a pass needs 1 to 8 targets or a depth, and at most 32 bindings\n"); exit(1); }
  p->w = first->w; p->h = first->h;

  // The render pass: targets cleared or kept, left ready to sample.
  VkAttachmentDescription att[9];
  VkAttachmentReference refs[8], dref;
  uint32_t n = 0;
  for (uint32_t i = 0; i < pass->ntargets; i++, n++) {
    att[n] = (VkAttachmentDescription){.format = pass->targets[i]->format, .samples = VK_SAMPLE_COUNT_1_BIT,
      .loadOp = pass->keep ? VK_ATTACHMENT_LOAD_OP_LOAD : VK_ATTACHMENT_LOAD_OP_CLEAR, .storeOp = VK_ATTACHMENT_STORE_OP_STORE,
      .stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE, .stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE,
      .initialLayout = pass->keep ? VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL : VK_IMAGE_LAYOUT_UNDEFINED,
      .finalLayout = VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL};
    refs[i] = (VkAttachmentReference){n, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL};
  }
  if (pass->depth) {
    att[n] = (VkAttachmentDescription){.format = pass->depth->format, .samples = VK_SAMPLE_COUNT_1_BIT,
      .loadOp = pass->keep ? VK_ATTACHMENT_LOAD_OP_LOAD : VK_ATTACHMENT_LOAD_OP_CLEAR, .storeOp = VK_ATTACHMENT_STORE_OP_STORE,
      .stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE, .stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE,
      .initialLayout = pass->keep ? VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL : VK_IMAGE_LAYOUT_UNDEFINED,
      .finalLayout = VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL};
    dref = (VkAttachmentReference){n, VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL};
    n++;
  }
  VkSubpassDescription sub = {.pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS, .colorAttachmentCount = pass->ntargets,
                              .pColorAttachments = refs, .pDepthStencilAttachment = pass->depth ? &dref : NULL};
  // Whatever read or wrote the targets before (a pass sampling them, a copy) is done
  // before this one writes; what this one writes is visible to what samples or copies it.
  VkPipelineStageFlags writes = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT | VK_PIPELINE_STAGE_EARLY_FRAGMENT_TESTS_BIT | VK_PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT;
  VkAccessFlags write_access = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT | VK_ACCESS_COLOR_ATTACHMENT_READ_BIT |
                               VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT | VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT;
  VkSubpassDependency deps[2] = {
    {.srcSubpass = VK_SUBPASS_EXTERNAL, .dstSubpass = 0,
     .srcStageMask = VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT | VK_PIPELINE_STAGE_TRANSFER_BIT | writes, .dstStageMask = writes,
     .srcAccessMask = write_access, .dstAccessMask = write_access},
    {.srcSubpass = 0, .dstSubpass = VK_SUBPASS_EXTERNAL,
     .srcStageMask = writes, .dstStageMask = VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT | VK_PIPELINE_STAGE_VERTEX_SHADER_BIT | VK_PIPELINE_STAGE_TRANSFER_BIT,
     .srcAccessMask = write_access, .dstAccessMask = VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_TRANSFER_READ_BIT}};
  VkRenderPassCreateInfo rpi = {.sType = VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO, .attachmentCount = n, .pAttachments = att,
    .subpassCount = 1, .pSubpasses = &sub, .dependencyCount = 2, .pDependencies = deps};
  CHECK(vkCreateRenderPass(dev, &rpi, NULL, &p->rp));
  VkImageView views[9];
  for (uint32_t i = 0; i < pass->ntargets; i++) views[i] = pass->targets[i]->view;
  if (pass->depth) views[pass->ntargets] = pass->depth->view;
  VkFramebufferCreateInfo fbi = {.sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO, .renderPass = p->rp, .attachmentCount = n,
                                 .pAttachments = views, .width = p->w, .height = p->h, .layers = 1};
  CHECK(vkCreateFramebuffer(dev, &fbi, NULL, &p->fb));

  // The bindings: set 0, in order, seen by both stages.
  VkDescriptorSetLayoutBinding binds[32];
  VkDescriptorPoolSize sizes[3] = {{VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, 0}, {VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE, 0}, {VK_DESCRIPTOR_TYPE_SAMPLER, 0}};
  for (uint32_t i = 0; i < pass->nb; i++) {
    binds[i] = (VkDescriptorSetLayoutBinding){.binding = i, .descriptorType = descriptor(pass->bindings[i].kind), .descriptorCount = 1,
                                              .stageFlags = VK_SHADER_STAGE_VERTEX_BIT | VK_SHADER_STAGE_FRAGMENT_BIT};
    sizes[pass->bindings[i].kind - 1].descriptorCount++;
  }
  VkDescriptorSetLayoutCreateInfo dsli = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO, .bindingCount = pass->nb, .pBindings = binds};
  CHECK(vkCreateDescriptorSetLayout(dev, &dsli, NULL, &p->dsl));
  VkDescriptorPoolSize used[3];
  uint32_t nsizes = 0;
  for (int i = 0; i < 3; i++) if (sizes[i].descriptorCount) used[nsizes++] = sizes[i];
  VkDescriptorPoolCreateInfo dpi = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO, .maxSets = 1, .poolSizeCount = nsizes, .pPoolSizes = used};
  if (!nsizes) { used[0] = (VkDescriptorPoolSize){VK_DESCRIPTOR_TYPE_SAMPLER, 1}; dpi.poolSizeCount = 1; }
  CHECK(vkCreateDescriptorPool(dev, &dpi, NULL, &p->pool));
  VkDescriptorSetAllocateInfo dsai = {.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO, .descriptorPool = p->pool, .descriptorSetCount = 1, .pSetLayouts = &p->dsl};
  CHECK(vkAllocateDescriptorSets(dev, &dsai, &p->set));
  VkWriteDescriptorSet writes_[32];
  VkDescriptorBufferInfo bufs[32];
  VkDescriptorImageInfo imgs[32];
  for (uint32_t i = 0; i < pass->nb; i++) {
    const VksBinding *b = &pass->bindings[i];
    writes_[i] = (VkWriteDescriptorSet){.sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = p->set, .dstBinding = i,
                                        .descriptorCount = 1, .descriptorType = descriptor(b->kind)};
    if (b->kind == VKS_STORAGE) { bufs[i] = (VkDescriptorBufferInfo){b->buffer->buf, 0, VK_WHOLE_SIZE}; writes_[i].pBufferInfo = &bufs[i]; }
    else {
      imgs[i] = (VkDescriptorImageInfo){.sampler = b->sampler, .imageView = b->kind == VKS_IMAGE ? b->image->view : VK_NULL_HANDLE,
                                        .imageLayout = VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL};
      writes_[i].pImageInfo = &imgs[i];
    }
  }
  vkUpdateDescriptorSets(dev, pass->nb, writes_, 0, NULL);
  VkPipelineLayoutCreateInfo pli = {.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO, .setLayoutCount = 1, .pSetLayouts = &p->dsl};
  CHECK(vkCreatePipelineLayout(dev, &pli, NULL, &p->layout));

  VkShaderModuleCreateInfo smi = {.sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO, .codeSize = pass->count * 4, .pCode = pass->words};
  CHECK(vkCreateShaderModule(dev, &smi, NULL, &p->module));
  VkPipelineShaderStageCreateInfo stages[2] = {
    {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage = VK_SHADER_STAGE_VERTEX_BIT, .module = p->module, .pName = "vs"},
    {.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage = VK_SHADER_STAGE_FRAGMENT_BIT, .module = p->module, .pName = "fs"}};
  VkVertexInputBindingDescription vb = {.binding = 0, .stride = pass->stride, .inputRate = VK_VERTEX_INPUT_RATE_VERTEX};
  VkPipelineVertexInputStateCreateInfo vin = {.sType = VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
    .vertexBindingDescriptionCount = pass->nattrs ? 1u : 0u, .pVertexBindingDescriptions = &vb,
    .vertexAttributeDescriptionCount = pass->nattrs, .pVertexAttributeDescriptions = pass->attrs};
  VkPipelineInputAssemblyStateCreateInfo ia = {.sType = VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO, .topology = VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST};
  // WebGPU's viewport: y down in framebuffer space, as clip y up maps to the top.
  VkViewport vp = {0, (float)p->h, (float)p->w, -(float)p->h, 0, 1};
  VkRect2D sc = {{0, 0}, {p->w, p->h}};
  VkPipelineViewportStateCreateInfo vps = {.sType = VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO, .viewportCount = 1, .pViewports = &vp, .scissorCount = 1, .pScissors = &sc};
  VkPipelineRasterizationStateCreateInfo rs = {.sType = VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO, .polygonMode = VK_POLYGON_MODE_FILL,
    .cullMode = pass->cull_back ? VK_CULL_MODE_BACK_BIT : VK_CULL_MODE_NONE, .frontFace = VK_FRONT_FACE_COUNTER_CLOCKWISE, .lineWidth = 1,
    .depthBiasEnable = pass->depth_bias != 0 || pass->depth_slope != 0, .depthBiasConstantFactor = pass->depth_bias, .depthBiasSlopeFactor = pass->depth_slope};
  VkPipelineMultisampleStateCreateInfo ms = {.sType = VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO, .rasterizationSamples = VK_SAMPLE_COUNT_1_BIT};
  VkPipelineDepthStencilStateCreateInfo ds = {.sType = VK_STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
    .depthTestEnable = pass->depth_test ? VK_TRUE : VK_FALSE, .depthWriteEnable = pass->depth_write ? VK_TRUE : VK_FALSE,
    .depthCompareOp = pass->depth_always ? VK_COMPARE_OP_ALWAYS : VK_COMPARE_OP_LESS};
  VkPipelineColorBlendAttachmentState blends[8];
  for (uint32_t i = 0; i < pass->ntargets; i++) {
    int off = (pass->masked >> i) & 1, am = pass->alpha_blend ? pass->alpha_blend : pass->blend == VKS_ALPHA ? VKS_OVER : VKS_ADD;
    blends[i] = (VkPipelineColorBlendAttachmentState){.blendEnable = pass->blend != VKS_REPLACE && !off,
      .srcColorBlendFactor = factor(pass->blend, 0), .dstColorBlendFactor = factor(pass->blend, 1), .colorBlendOp = VK_BLEND_OP_ADD,
      .srcAlphaBlendFactor = factor(am, 0), .dstAlphaBlendFactor = factor(am, 1), .alphaBlendOp = VK_BLEND_OP_ADD,
      .colorWriteMask = off ? 0 : 0xF};
  }
  VkPipelineColorBlendStateCreateInfo cb = {.sType = VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO, .attachmentCount = pass->ntargets, .pAttachments = blends};
  VkGraphicsPipelineCreateInfo gpi = {.sType = VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO, .stageCount = 2, .pStages = stages,
    .pVertexInputState = &vin, .pInputAssemblyState = &ia, .pViewportState = &vps, .pRasterizationState = &rs, .pMultisampleState = &ms,
    .pDepthStencilState = &ds, .pColorBlendState = &cb, .layout = p->layout, .renderPass = p->rp, .subpass = 0};
  CHECK(vkCreateGraphicsPipelines(dev, VK_NULL_HANDLE, 1, &gpi, NULL, &p->pipeline));
}

void vks_pipe_record(VksPipe *p, VkCommandBuffer cmd, const VksPass *pass) {
  VkClearValue clears[9];
  uint32_t n = 0;
  for (uint32_t i = 0; i < pass->ntargets; i++, n++) memcpy(clears[n].color.float32, pass->clear, 16);
  if (pass->depth) clears[n++].depthStencil = (VkClearDepthStencilValue){1, 0};
  VkRenderPassBeginInfo rbi = {.sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO, .renderPass = p->rp, .framebuffer = p->fb,
                               .renderArea = {{0, 0}, {p->w, p->h}}, .clearValueCount = n, .pClearValues = clears};
  vkCmdBeginRenderPass(cmd, &rbi, VK_SUBPASS_CONTENTS_INLINE);
  if (pass->vertex_count) {
    vkCmdBindPipeline(cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, p->pipeline);
    vkCmdBindDescriptorSets(cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, p->layout, 0, 1, &p->set, 0, NULL);
    if (pass->nattrs) { VkDeviceSize zero = 0; vkCmdBindVertexBuffers(cmd, 0, 1, &pass->vertices->buf, &zero); }
    vkCmdDraw(cmd, pass->vertex_count, pass->instance_count ? pass->instance_count : 1, 0, 0);
  }
  if (pass->ndraws) {
    vkCmdBindPipeline(cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, p->pipeline);
    vkCmdBindDescriptorSets(cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, p->layout, 0, 1, &p->set, 0, NULL);
    if (pass->index) vkCmdBindIndexBuffer(cmd, pass->index->buf, 0, VK_INDEX_TYPE_UINT32);
    for (uint32_t i = 0; i < pass->ndraws; i++) {
      const uint32_t *d = pass->draws + i * 4;
      if (!d[1] || !d[3]) continue;
      if (d[2] & 0x80000000u) vkCmdDraw(cmd, d[1], d[3], d[0], d[2] & 0x7fffffffu);
      else if (pass->index) vkCmdDrawIndexed(cmd, d[1], d[3], d[0], 0, d[2]);
    }
  }
  vkCmdEndRenderPass(cmd);
}

void vks_pipe_close(Vkc *v, VksPipe *p) {
  VkDevice dev = v->dev;
  vkDestroyPipeline(dev, p->pipeline, NULL);
  vkDestroyShaderModule(dev, p->module, NULL);
  vkDestroyPipelineLayout(dev, p->layout, NULL);
  vkDestroyDescriptorPool(dev, p->pool, NULL);
  vkDestroyDescriptorSetLayout(dev, p->dsl, NULL);
  vkDestroyFramebuffer(dev, p->fb, NULL);
  vkDestroyRenderPass(dev, p->rp, NULL);
}

typedef struct { VksImage *from, *to; } ImageCopy;
static void copy_barrier(VkCommandBuffer cmd, VksImage *t, VkImageLayout a, VkImageLayout b, VkAccessFlags sa, VkAccessFlags da) {
  VkImageMemoryBarrier m = {.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER, .srcAccessMask = sa, .dstAccessMask = da, .oldLayout = a,
    .newLayout = b, .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .image = t->image,
    .subresourceRange = {aspect(t), 0, 1, 0, 1}};
  vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, 0, 0, NULL, 0, NULL, 1, &m);
}
static void record_image_copy(VkCommandBuffer cmd, void *user) {
  ImageCopy *c = user;
  // the destination's old contents are not kept
  copy_barrier(cmd, c->from, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
               VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT | VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT, VK_ACCESS_TRANSFER_READ_BIT);
  copy_barrier(cmd, c->to, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_ACCESS_SHADER_READ_BIT, VK_ACCESS_TRANSFER_WRITE_BIT);
  VkImageCopy region = {.srcSubresource = {aspect(c->from), 0, 0, 1}, .dstSubresource = {aspect(c->to), 0, 0, 1}, .extent = {c->from->w, c->from->h, 1}};
  vkCmdCopyImage(cmd, c->from->image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, c->to->image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &region);
  copy_barrier(cmd, c->from, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_ACCESS_TRANSFER_READ_BIT,
               VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT | VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT);
  copy_barrier(cmd, c->to, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, VK_ACCESS_TRANSFER_WRITE_BIT, VK_ACCESS_SHADER_READ_BIT);
}
void vks_copy(Vkc *v, VksImage *from, VksImage *to) {
  if (from->w != to->w || from->h != to->h || from->format != to->format) { fprintf(stderr, "vks: a copy between images of different sizes or formats\n"); exit(1); }
  ImageCopy c = {from, to};
  vks_submit(v, record_image_copy, &c);
}
void vks_copy_record(VkCommandBuffer cmd, VksImage *from, VksImage *to) {
  if (from->w != to->w || from->h != to->h || from->format != to->format) { fprintf(stderr, "vks: a copy between images of different sizes or formats\n"); exit(1); }
  ImageCopy c = {from, to};
  record_image_copy(cmd, &c);
}
