// Firm reason: 6 (Android's window, input and Vulkan are C APIs; this host holds no product logic).
// Everglade for Android, the host (apps/everglade-mobile/README.md): a NativeActivity, a Vulkan
// swapchain and touch. Everything drawn comes from the same Coax as the desktop and iOS apps: the
// scene (workshop.coax), the temple's parts and Metaverse Engine shaders (emitted by `coax spirv`), and your
// model. This file is to Android what Engine.swift and ios/Sources/main.swift are to iOS.
//
// Your account: the list's Log in opens a card (account.coax) whose requests app-login.c sends on
// its own thread; the soft keyboard comes up while its box takes typing.
//
// Touch: a drag that starts on the left side is a stick that moves you; one that starts on the
// right side looks round; a tap there jumps; a second finger there pinches to zoom.
#include <android_native_app_glue.h>
#include <android/asset_manager.h>
#include <android/configuration.h>
#include <android/imagedecoder.h>
#include <android/log.h>
#include <jni.h>
#include <math.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/system_properties.h>
#include <time.h>
#include "../../shared/host/android/device-key.h"
#include "../../shared/host/android/app-login.h"
#define VK_USE_PLATFORM_ANDROID_KHR
#include <vulkan/vulkan.h>
#include "face.h"
#include "../../../metaverse-engine/host/core/host-core.h"   // the one C host core (#13304): the pack's entries by kind, over the one pack reader

#define SAY(...) __android_log_print(ANDROID_LOG_INFO, "everglade", __VA_ARGS__)
// Without one of the app's own parts there is nothing to show, so say which and stop.
#define STOP(...) do { __android_log_print(ANDROID_LOG_ERROR, "everglade", __VA_ARGS__); abort(); } while (0)
#define VK(call) do { VkResult r_ = (call); if (r_ != VK_SUCCESS) STOP("Everglade: %s failed (%d)", #call, (int)r_); } while (0)

// A thread with a deep stack, for the web's character code (model.coax), which passes megabytes
// by value; Android gives a thread about a megabyte.
static pthread_mutex_t deep_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t deep_go = PTHREAD_COND_INITIALIZER, deep_done = PTHREAD_COND_INITIALIZER;
static void (*deep_work)(void *); static void *deep_arg; static int deep_busy;
static void *deep_main(void *unused) {
  (void)unused;
  pthread_mutex_lock(&deep_lock);
  for (;;) {
    while (!deep_work) pthread_cond_wait(&deep_go, &deep_lock);
    deep_work(deep_arg); deep_work = NULL; deep_busy = 0;
    pthread_cond_signal(&deep_done);
  }
  return NULL;
}
static void deep_start(void) {
  pthread_attr_t a; pthread_t t;
  pthread_attr_init(&a);
  if (pthread_attr_setstacksize(&a, (size_t)512 << 20) != 0 || pthread_create(&t, &a, deep_main, NULL) != 0)
    STOP("Everglade: no thread with a 512 MB stack, which your character's code needs.");
}
static void deep(void (*work)(void *), void *arg) {
  pthread_mutex_lock(&deep_lock);
  deep_work = work; deep_arg = arg; deep_busy = 1;
  pthread_cond_signal(&deep_go);
  while (deep_busy) pthread_cond_wait(&deep_done, &deep_lock);
  pthread_mutex_unlock(&deep_lock);
}

typedef struct { VkBuffer buffer; VkDeviceMemory memory; void *map; VkDeviceSize size; } Buf;
typedef struct { VkImage image; VkDeviceMemory memory; VkImageView view; } Img;

static struct {
  struct android_app *app;
  JNIEnv *env;
  // The scene, your model, and what they draw into.
  void *state, *model;
  int model_draws; uint32_t *draws_model; uint32_t index_count;
  uint32_t *draws_place; int group_first[3], group_count[3];
  float time, safe[3];
  double last, jump_up, look_down, inset_at;
  // Fingers: the one that is the stick and where it went down, the one that looks, the one that pinches.
  int32_t stick, look, pinch; float stick_x, stick_y, look_moved, pinch_span;
  // Vulkan.
  VkInstance instance; VkPhysicalDevice gpu; VkDevice device; uint32_t family; VkQueue queue;
  VkPhysicalDeviceMemoryProperties memory;
  VkCommandPool pool; VkCommandBuffer cmd; VkFence fence; VkSemaphore acquired, drawn;
  VkSurfaceKHR surface; VkSwapchainKHR swapchain; VkFormat format; VkExtent2D extent;
  uint32_t image_count; VkImageView views[8]; VkFramebuffer frames[8];
  Img depth, mirror, mirror_depth, blank, faces, sheets;
  VkFramebuffer mirror_frame;
  VkRenderPass pass, mirror_pass;
  VkSampler smooth, mipped;
  VkDescriptorPool sets; VkDescriptorSetLayout place_layout, model_layout, stroke_layout;
  VkPipelineLayout place_pipes, model_pipes, stroke_pipes;
  VkDescriptorSet place_set, mirror_set, model_set, stroke_set;
  VkPipeline sky, sky_mirror, scene, scene_mirror, light, model_pipe, strokes;
  Buf u_place, u_mirror, u_model, mesh, records, model_vertices, model_indices, model_bake, model_records, stroke_vertices;
  int ready, resized, focused;
} g;

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + (double)t.tv_nsec * 1e-9; }

// ---- The app's files ----

static const uint8_t *asset(const char *name, size_t *length) {
  AAsset *a = AAssetManager_open(g.app->activity->assetManager, name, AASSET_MODE_BUFFER);
  if (!a) STOP("Everglade: the app's %s is missing. Rebuild with apps/everglade-mobile/android/build.sh.", name);
  *length = (size_t)AAsset_getLength(a);
  return AAsset_getBuffer(a);   // kept open: the bytes stay for the life of the app
}
static uint32_t word(const uint8_t *d, size_t at) { uint32_t v; memcpy(&v, d + at, 4); return v; }

// ---- Vulkan's small parts ----

static uint32_t memory_type(uint32_t bits, VkMemoryPropertyFlags want) {
  for (uint32_t i = 0; i < g.memory.memoryTypeCount; i++)
    if ((bits & (1u << i)) && (g.memory.memoryTypes[i].propertyFlags & want) == want) return i;
  STOP("Everglade: this GPU has no memory of the kind the app needs.");
}
static Buf buffer(VkDeviceSize size, VkBufferUsageFlags usage) {
  Buf b = { .size = size };
  VkBufferCreateInfo bi = { .sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .size = size, .usage = usage };
  VK(vkCreateBuffer(g.device, &bi, NULL, &b.buffer));
  VkMemoryRequirements req; vkGetBufferMemoryRequirements(g.device, b.buffer, &req);
  VkMemoryAllocateInfo ai = { .sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, .allocationSize = req.size,
    .memoryTypeIndex = memory_type(req.memoryTypeBits, VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT) };
  VK(vkAllocateMemory(g.device, &ai, NULL, &b.memory));
  VK(vkBindBufferMemory(g.device, b.buffer, b.memory, 0));
  VK(vkMapMemory(g.device, b.memory, 0, size, 0, &b.map));
  return b;
}
static void drop_buffer(Buf *b) {
  if (b->buffer) { vkDestroyBuffer(g.device, b->buffer, NULL); vkFreeMemory(g.device, b->memory, NULL); }
  memset(b, 0, sizeof *b);
}
static Img image(VkFormat format, uint32_t w, uint32_t h, uint32_t layers, uint32_t mips, VkImageUsageFlags usage, VkImageAspectFlags aspect, int array) {
  Img m;
  VkImageCreateInfo ii = { .sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO, .imageType = VK_IMAGE_TYPE_2D, .format = format,
    .extent = { w, h, 1 }, .mipLevels = mips, .arrayLayers = layers, .samples = VK_SAMPLE_COUNT_1_BIT, .usage = usage };
  VK(vkCreateImage(g.device, &ii, NULL, &m.image));
  VkMemoryRequirements req; vkGetImageMemoryRequirements(g.device, m.image, &req);
  VkMemoryAllocateInfo ai = { .sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, .allocationSize = req.size,
    .memoryTypeIndex = memory_type(req.memoryTypeBits, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT) };
  VK(vkAllocateMemory(g.device, &ai, NULL, &m.memory));
  VK(vkBindImageMemory(g.device, m.image, m.memory, 0));
  VkImageViewCreateInfo vi = { .sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO, .image = m.image,
    .viewType = array ? VK_IMAGE_VIEW_TYPE_2D_ARRAY : VK_IMAGE_VIEW_TYPE_2D, .format = format,
    .subresourceRange = { aspect, 0, mips, 0, layers } };
  VK(vkCreateImageView(g.device, &vi, NULL, &m.view));
  return m;
}
static void drop_image(Img *m) {
  if (m->image) { vkDestroyImageView(g.device, m->view, NULL); vkDestroyImage(g.device, m->image, NULL); vkFreeMemory(g.device, m->memory, NULL); }
  memset(m, 0, sizeof *m);
}
static void once_begin(void) {
  VkCommandBufferBeginInfo bi = { .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO, .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT };
  VK(vkResetCommandBuffer(g.cmd, 0)); VK(vkBeginCommandBuffer(g.cmd, &bi));
}
static void once_end(void) {
  VK(vkEndCommandBuffer(g.cmd));
  VkSubmitInfo si = { .sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .commandBufferCount = 1, .pCommandBuffers = &g.cmd };
  VK(vkQueueSubmit(g.queue, 1, &si, VK_NULL_HANDLE)); VK(vkQueueWaitIdle(g.queue));
}
static void layout(VkImage im, uint32_t mip, uint32_t mips, uint32_t layers, VkImageLayout from, VkImageLayout to) {
  VkImageMemoryBarrier b = { .sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER, .oldLayout = from, .newLayout = to,
    .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .image = im,
    .subresourceRange = { VK_IMAGE_ASPECT_COLOR_BIT, mip, mips, 0, layers },
    .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT | VK_ACCESS_TRANSFER_READ_BIT, .dstAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT | VK_ACCESS_TRANSFER_READ_BIT | VK_ACCESS_SHADER_READ_BIT };
  vkCmdPipelineBarrier(g.cmd, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, VK_PIPELINE_STAGE_ALL_COMMANDS_BIT, 0, 0, NULL, 0, NULL, 1, &b);
}
static VkShaderModule shader(const char *name) {
  size_t n; const uint8_t *code = asset(name, &n);
  uint32_t *words = malloc(n); memcpy(words, code, n);
  VkShaderModuleCreateInfo si = { .sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO, .codeSize = n, .pCode = words };
  VkShaderModule m; VK(vkCreateShaderModule(g.device, &si, NULL, &m));
  free(words);
  return m;
}

// One pipeline. blend: 0 none, 1 added, 2 added with the larger alpha (the strokes). A triangle's
// front is counter-clockwise, or clockwise in the mirror image, which is upside down.
static VkPipeline pipeline(VkShaderModule vs, const char *vs_name, VkShaderModule fs, const char *fs_name, VkPipelineLayout pl, VkRenderPass pass,
                           int blend, VkCompareOp compare, int write, int mirrored, int stroke_vertices) {
  VkPipelineShaderStageCreateInfo stages[2] = {
    { .sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage = VK_SHADER_STAGE_VERTEX_BIT, .module = vs, .pName = vs_name },
    { .sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage = VK_SHADER_STAGE_FRAGMENT_BIT, .module = fs, .pName = fs_name } };
  VkVertexInputBindingDescription vb = { 0, (uint32_t)eg_vertex_floats() * 4, VK_VERTEX_INPUT_RATE_VERTEX };
  VkVertexInputAttributeDescription va[3] = { { 0, 0, VK_FORMAT_R32G32B32A32_SFLOAT, 0 }, { 1, 0, VK_FORMAT_R32G32B32A32_SFLOAT, 16 }, { 2, 0, VK_FORMAT_R32G32B32A32_SFLOAT, 32 } };
  VkPipelineVertexInputStateCreateInfo vi = { .sType = VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO };
  if (stroke_vertices) { vi.vertexBindingDescriptionCount = 1; vi.pVertexBindingDescriptions = &vb; vi.vertexAttributeDescriptionCount = 3; vi.pVertexAttributeDescriptions = va; }
  VkPipelineInputAssemblyStateCreateInfo ia = { .sType = VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO, .topology = VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST };
  VkPipelineViewportStateCreateInfo vp = { .sType = VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO, .viewportCount = 1, .scissorCount = 1 };
  VkPipelineRasterizationStateCreateInfo rs = { .sType = VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO, .polygonMode = VK_POLYGON_MODE_FILL,
    .cullMode = VK_CULL_MODE_NONE, .frontFace = mirrored ? VK_FRONT_FACE_CLOCKWISE : VK_FRONT_FACE_COUNTER_CLOCKWISE, .lineWidth = 1 };
  VkPipelineMultisampleStateCreateInfo ms = { .sType = VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO, .rasterizationSamples = VK_SAMPLE_COUNT_1_BIT };
  VkPipelineDepthStencilStateCreateInfo ds = { .sType = VK_STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO, .depthTestEnable = VK_TRUE, .depthWriteEnable = write ? VK_TRUE : VK_FALSE, .depthCompareOp = compare };
  VkPipelineColorBlendAttachmentState att = { .colorWriteMask = 0xf };
  if (blend) {
    att.blendEnable = VK_TRUE; att.srcColorBlendFactor = VK_BLEND_FACTOR_ONE; att.dstColorBlendFactor = VK_BLEND_FACTOR_ONE; att.colorBlendOp = VK_BLEND_OP_ADD;
    att.srcAlphaBlendFactor = VK_BLEND_FACTOR_ONE; att.dstAlphaBlendFactor = VK_BLEND_FACTOR_ONE; att.alphaBlendOp = blend == 2 ? VK_BLEND_OP_MAX : VK_BLEND_OP_ADD;
  }
  VkPipelineColorBlendStateCreateInfo cb = { .sType = VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO, .attachmentCount = 1, .pAttachments = &att };
  VkDynamicState dyn[2] = { VK_DYNAMIC_STATE_VIEWPORT, VK_DYNAMIC_STATE_SCISSOR };
  VkPipelineDynamicStateCreateInfo dy = { .sType = VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO, .dynamicStateCount = 2, .pDynamicStates = dyn };
  VkGraphicsPipelineCreateInfo pi = { .sType = VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO, .stageCount = 2, .pStages = stages, .pVertexInputState = &vi,
    .pInputAssemblyState = &ia, .pViewportState = &vp, .pRasterizationState = &rs, .pMultisampleState = &ms, .pDepthStencilState = &ds, .pColorBlendState = &cb,
    .pDynamicState = &dy, .layout = pl, .renderPass = pass };
  VkPipeline p; VK(vkCreateGraphicsPipelines(g.device, VK_NULL_HANDLE, 1, &pi, NULL, &p));
  return p;
}
static VkRenderPass render_pass(VkFormat colour, VkAttachmentLoadOp load, VkImageLayout final) {
  VkAttachmentDescription at[2] = {
    { .format = colour, .samples = VK_SAMPLE_COUNT_1_BIT, .loadOp = load, .storeOp = VK_ATTACHMENT_STORE_OP_STORE,
      .stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE, .stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE, .finalLayout = final },
    { .format = VK_FORMAT_D32_SFLOAT, .samples = VK_SAMPLE_COUNT_1_BIT, .loadOp = VK_ATTACHMENT_LOAD_OP_CLEAR, .storeOp = VK_ATTACHMENT_STORE_OP_DONT_CARE,
      .stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE, .stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE, .finalLayout = VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL } };
  VkAttachmentReference c = { 0, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL }, d = { 1, VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
  VkSubpassDescription sp = { .pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS, .colorAttachmentCount = 1, .pColorAttachments = &c, .pDepthStencilAttachment = &d };
  VkPipelineStageFlags all = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT | VK_PIPELINE_STAGE_EARLY_FRAGMENT_TESTS_BIT | VK_PIPELINE_STAGE_LATE_FRAGMENT_TESTS_BIT | VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT;
  VkAccessFlags any = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT | VK_ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT | VK_ACCESS_SHADER_READ_BIT;
  VkSubpassDependency dep[2] = { { VK_SUBPASS_EXTERNAL, 0, all, all, any, any, 0 }, { 0, VK_SUBPASS_EXTERNAL, all, all, any, any, 0 } };
  VkRenderPassCreateInfo ri = { .sType = VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO, .attachmentCount = 2, .pAttachments = at, .subpassCount = 1, .pSubpasses = &sp, .dependencyCount = 2, .pDependencies = dep };
  VkRenderPass p; VK(vkCreateRenderPass(g.device, &ri, NULL, &p));
  return p;
}
static VkDescriptorSetLayout set_layout(int n, const VkDescriptorType *types) {
  VkDescriptorSetLayoutBinding b[8];
  for (int i = 0; i < n; i++) b[i] = (VkDescriptorSetLayoutBinding){ (uint32_t)i, types[i], 1, VK_SHADER_STAGE_VERTEX_BIT | VK_SHADER_STAGE_FRAGMENT_BIT, NULL };
  VkDescriptorSetLayoutCreateInfo li = { .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO, .bindingCount = (uint32_t)n, .pBindings = b };
  VkDescriptorSetLayout l; VK(vkCreateDescriptorSetLayout(g.device, &li, NULL, &l));
  return l;
}
static VkDescriptorSet set(VkDescriptorSetLayout l) {
  VkDescriptorSetAllocateInfo ai = { .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO, .descriptorPool = g.sets, .descriptorSetCount = 1, .pSetLayouts = &l };
  VkDescriptorSet s; VK(vkAllocateDescriptorSets(g.device, &ai, &s));
  return s;
}
static void put_buffer(VkDescriptorSet s, uint32_t binding, VkDescriptorType type, Buf *b) {
  VkDescriptorBufferInfo bi = { b->buffer, 0, b->size };
  VkWriteDescriptorSet w = { .sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = s, .dstBinding = binding, .descriptorCount = 1, .descriptorType = type, .pBufferInfo = &bi };
  vkUpdateDescriptorSets(g.device, 1, &w, 0, NULL);
}
static void put_image(VkDescriptorSet s, uint32_t binding, VkDescriptorType type, VkImageView view, VkSampler sampler) {
  VkDescriptorImageInfo ii = { sampler, view, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL };
  VkWriteDescriptorSet w = { .sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = s, .dstBinding = binding, .descriptorCount = 1, .descriptorType = type, .pImageInfo = &ii };
  vkUpdateDescriptorSets(g.device, 1, &w, 0, NULL);
}

// ---- The GPU, once ----

static void gpu_open(void) {
  VkApplicationInfo app = { .sType = VK_STRUCTURE_TYPE_APPLICATION_INFO, .pApplicationName = "Everglade", .apiVersion = VK_API_VERSION_1_1 };
  const char *iext[] = { "VK_KHR_surface", "VK_KHR_android_surface" };
  VkInstanceCreateInfo ii = { .sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo = &app, .enabledExtensionCount = 2, .ppEnabledExtensionNames = iext };
  VK(vkCreateInstance(&ii, NULL, &g.instance));
  uint32_t n = 1; VkResult r = vkEnumeratePhysicalDevices(g.instance, &n, &g.gpu);
  if ((r != VK_SUCCESS && r != VK_INCOMPLETE) || n == 0) STOP("Everglade needs a Vulkan GPU, and this device reports none.");
  VkPhysicalDeviceProperties props; vkGetPhysicalDeviceProperties(g.gpu, &props);
  if (props.apiVersion < VK_API_VERSION_1_1) STOP("Everglade needs Vulkan 1.1, and %s has an older one.", props.deviceName);
  // The Metaverse Engine's emitted shaders count in 64-bit integers.
  VkPhysicalDeviceFeatures has; vkGetPhysicalDeviceFeatures(g.gpu, &has);
  if (!has.shaderInt64) STOP("Everglade's shaders need 64-bit integers (shaderInt64), which %s lacks.", props.deviceName);
  vkGetPhysicalDeviceMemoryProperties(g.gpu, &g.memory);
  VkQueueFamilyProperties fam[16]; uint32_t fn = 16; vkGetPhysicalDeviceQueueFamilyProperties(g.gpu, &fn, fam);
  g.family = UINT32_MAX;
  for (uint32_t i = 0; i < fn; i++) if (fam[i].queueFlags & VK_QUEUE_GRAPHICS_BIT) { g.family = i; break; }
  if (g.family == UINT32_MAX) STOP("Everglade: %s has no graphics queue.", props.deviceName);
  float priority = 1;
  VkDeviceQueueCreateInfo qi = { .sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO, .queueFamilyIndex = g.family, .queueCount = 1, .pQueuePriorities = &priority };
  const char *dext[] = { "VK_KHR_swapchain" };
  VkPhysicalDeviceFeatures want = { .shaderInt64 = VK_TRUE };
  VkDeviceCreateInfo di = { .sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO, .queueCreateInfoCount = 1, .pQueueCreateInfos = &qi, .enabledExtensionCount = 1, .ppEnabledExtensionNames = dext, .pEnabledFeatures = &want };
  VK(vkCreateDevice(g.gpu, &di, NULL, &g.device));
  vkGetDeviceQueue(g.device, g.family, 0, &g.queue);
  VkCommandPoolCreateInfo pi = { .sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT, .queueFamilyIndex = g.family };
  VK(vkCreateCommandPool(g.device, &pi, NULL, &g.pool));
  VkCommandBufferAllocateInfo ci = { .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO, .commandPool = g.pool, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1 };
  VK(vkAllocateCommandBuffers(g.device, &ci, &g.cmd));
  VkFenceCreateInfo fi = { .sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO, .flags = VK_FENCE_CREATE_SIGNALED_BIT };
  VK(vkCreateFence(g.device, &fi, NULL, &g.fence));
  VkSemaphoreCreateInfo si = { .sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO };
  VK(vkCreateSemaphore(g.device, &si, NULL, &g.acquired)); VK(vkCreateSemaphore(g.device, &si, NULL, &g.drawn));
  VkSamplerCreateInfo sa = { .sType = VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO, .magFilter = VK_FILTER_LINEAR, .minFilter = VK_FILTER_LINEAR,
    .addressModeU = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE, .addressModeV = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE, .addressModeW = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE };
  VK(vkCreateSampler(g.device, &sa, NULL, &g.smooth));
  sa.mipmapMode = VK_SAMPLER_MIPMAP_MODE_LINEAR; sa.maxLod = VK_LOD_CLAMP_NONE;
  VK(vkCreateSampler(g.device, &sa, NULL, &g.mipped));
  VkDescriptorPoolSize sizes[4] = { { VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, 16 }, { VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER, 4 }, { VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE, 4 }, { VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, 4 } };
  VkDescriptorPoolSize sampler_size = { VK_DESCRIPTOR_TYPE_SAMPLER, 4 };
  VkDescriptorPoolSize all[5] = { sizes[0], sizes[1], sizes[2], sizes[3], sampler_size };
  VkDescriptorPoolCreateInfo dp = { .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO, .maxSets = 8, .poolSizeCount = 5, .pPoolSizes = all };
  VK(vkCreateDescriptorPool(g.device, &dp, NULL, &g.sets));
  SAY("GPU %s, Vulkan %u.%u", props.deviceName, VK_VERSION_MAJOR(props.apiVersion), VK_VERSION_MINOR(props.apiVersion));
}

// A texture filled from bytes already in a staging buffer: each (mip, layer) from its offset.
static void fill(Img *m, Buf *staging, uint32_t mips_given, uint32_t layers, const VkDeviceSize *offsets, const uint32_t *widths, const uint32_t *heights) {
  VkBufferImageCopy copies[80]; uint32_t n = 0;
  for (uint32_t mip = 0; mip < mips_given; mip++)
    for (uint32_t layer = 0; layer < layers; layer++, n++)
      copies[n] = (VkBufferImageCopy){ .bufferOffset = offsets[n], .imageSubresource = { VK_IMAGE_ASPECT_COLOR_BIT, mip, layer, 1 }, .imageExtent = { widths[mip], heights[mip], 1 } };
  vkCmdCopyBufferToImage(g.cmd, staging->buffer, m->image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, n, copies);
}

// The faces' atlas, baked on the Mac at build time (bake-faces): each mip drawn at its own size.
static void faces_open(void) {
  size_t n; const uint8_t *d = asset("faces.bin", &n);
  if (n < 8 || memcmp(d, "EGF1", 4) != 0) STOP("Everglade: the app's faces.bin is not a faces' atlas. Rebuild with apps/everglade-mobile/android/build.sh.");
  uint32_t levels = word(d, 4), widths[8], heights[8]; VkDeviceSize offsets[8]; size_t at = 8, total = 0;
  if (levels == 0 || levels > 8) STOP("Everglade: the app's faces.bin is damaged.");
  Buf staging = buffer(n, VK_BUFFER_USAGE_TRANSFER_SRC_BIT);
  for (uint32_t l = 0; l < levels; l++) {
    if (at + 8 > n) STOP("Everglade: the app's faces.bin is damaged.");
    widths[l] = word(d, at); heights[l] = word(d, at + 4); at += 8;
    size_t bytes = (size_t)widths[l] * heights[l] * 2;
    if (at + bytes > n) STOP("Everglade: the app's faces.bin is damaged.");
    memcpy((uint8_t *)staging.map + total, d + at, bytes); offsets[l] = total; total += bytes; at += bytes;
  }
  g.faces = image(VK_FORMAT_R8G8_UNORM, widths[0], heights[0], 1, levels, VK_IMAGE_USAGE_SAMPLED_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT, VK_IMAGE_ASPECT_COLOR_BIT, 0);
  once_begin();
  layout(g.faces.image, 0, levels, 1, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL);
  fill(&g.faces, &staging, levels, 1, offsets, widths, heights);
  layout(g.faces.image, 0, levels, 1, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
  once_end();
  drop_buffer(&staging);
}

// The place: the temple's parts as one mesh, the records that stand them, and the draws.
static void place_open(void) {
  void *place = plc_new();
  if (!place) STOP("Everglade: the temple did not build.");
  int vf = plc_vertex_floats() * plc_vertices(place), rf = plc_record_floats() * plc_records(place), dn = plc_draws(place) * 4;
  if (vf <= 0 || rf <= 0 || dn <= 0) STOP("Everglade: the temple did not build.");
  g.mesh = buffer((VkDeviceSize)vf * 4, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT);
  g.records = buffer((VkDeviceSize)rf * 4, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT);
  g.draws_place = malloc((size_t)dn * 4);
  if (plc_copy_mesh(place, g.mesh.map, vf) != 1 || plc_copy_records(place, g.records.map, rf) != 1 || plc_copy_draws(place, g.draws_place, dn) != 1)
    STOP("Everglade: the temple did not build.");
  for (int k = 0; k < 3; k++) { g.group_first[k] = plc_group_first(place, k); g.group_count[k] = plc_group_count(place, k); }
  free(place);
  g.u_place = buffer(96, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT);
  g.u_mirror = buffer(96, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT);
}

// Your model: the pack's mesh and layers, the baked clips, and this frame's records.
struct ModelOpen { const uint8_t *catalog; int32_t length; int named, baked; };
static void model_make(void *p) {
  struct ModelOpen *o = p;
  g.model = egm_new();
  if (g.model) o->named = egm_catalog(g.model, o->catalog, o->length);
}
static void model_bake(void *p) { ((struct ModelOpen *)p)->baked = egm_bake(g.model, g.model_bake.map); }
static void model_open(void) {
  // Everglade II's pack: a catalog, the vertices, the indices and the layers, found by the one pack
  // reader (metaverse-engine/host/pack-table.coax pack_table), sorted by kind by the host core
  // (metaverse-engine/host/core/host-core.h).
  size_t n; const uint8_t *d = asset("everglade2.pack", &n);
  const char *bad = "Everglade: the app's everglade2.pack is damaged or doesn't hold the player's parts. Rebuild with apps/everglade-mobile/android/build.sh.";
  static EngineCorePack pack;
  if (!engine_core_pack(&pack, d, n)) STOP("%s", bad);
  const uint8_t *layers[64 + 16]; size_t cn = 0, vn = 0, in = 0, ln[64 + 16]; uint32_t layer_count = 0;
  const uint8_t *catalog = engine_core_entry(&pack, pack.catalog, &cn), *vertices = engine_core_entry(&pack, pack.vertices, &vn), *indices = engine_core_entry(&pack, pack.indices, &in);
  for (int k = 0; k < pack.colours && layer_count < 80; k++) { layers[layer_count] = engine_core_entry(&pack, pack.colour[k], &ln[layer_count]); layer_count++; }
  if (!catalog || !vertices || !indices || layer_count == 0) STOP("%s", bad);
  struct ModelOpen o = { catalog, (int32_t)cn, 0, 0 };
  deep(model_make, &o);
  if (!g.model || o.named != 1) STOP("%s", bad);
  g.model_vertices = buffer(vn, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT); memcpy(g.model_vertices.map, vertices, vn);
  g.model_indices = buffer(in, VK_BUFFER_USAGE_INDEX_BUFFER_BIT); memcpy(g.model_indices.map, indices, in);
  g.index_count = (uint32_t)(in / 4);
  g.model_bake = buffer((VkDeviceSize)egm_bake_floats() * 4, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT);
  g.model_records = buffer((VkDeviceSize)egm_records() * egm_record_floats() * 4, VK_BUFFER_USAGE_STORAGE_BUFFER_BIT);
  g.draws_model = calloc((size_t)egm_records() * 4, 4);
  g.u_model = buffer(96, VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT);
  deep(model_bake, &o);
  if (o.baked != 1) STOP("%s", bad);
  // Every layer drawn to one size, so they stack as one array; the pack's are 512 a side at most.
  const uint32_t side = 512, mips = 10; const size_t each = (size_t)side * side * 4;
  Buf staging = buffer(each * layer_count, VK_BUFFER_USAGE_TRANSFER_SRC_BIT);
  VkDeviceSize offsets[80]; uint32_t widths[1] = { side }, heights[1] = { side };
  for (uint32_t l = 0; l < layer_count; l++) {
    AImageDecoder *dec = NULL; uint8_t *px = (uint8_t *)staging.map + each * l; offsets[l] = each * l;
    if (AImageDecoder_createFromBuffer(layers[l], ln[l], &dec) != ANDROID_IMAGE_DECODER_SUCCESS
        || AImageDecoder_setAndroidBitmapFormat(dec, ANDROID_BITMAP_FORMAT_RGBA_8888) != ANDROID_IMAGE_DECODER_SUCCESS
        || AImageDecoder_setTargetSize(dec, (int32_t)side, (int32_t)side) != ANDROID_IMAGE_DECODER_SUCCESS) STOP("%s", bad);
    AImageDecoder_setUnpremultipliedRequired(dec, true);   // the shader wants colour straight; an opaque picture is unchanged
    if (AImageDecoder_decodeImage(dec, px, side * 4, each) != ANDROID_IMAGE_DECODER_SUCCESS) STOP("%s", bad);
    AImageDecoder_delete(dec);
  }
  g.sheets = image(VK_FORMAT_R8G8B8A8_SRGB, side, side, layer_count, mips, VK_IMAGE_USAGE_SAMPLED_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT, VK_IMAGE_ASPECT_COLOR_BIT, 1);
  once_begin();
  layout(g.sheets.image, 0, mips, layer_count, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL);
  fill(&g.sheets, &staging, 1, layer_count, offsets, widths, heights);
  // Each smaller mip from the one above it.
  for (uint32_t mip = 1; mip < mips; mip++) {
    layout(g.sheets.image, mip - 1, 1, layer_count, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL);
    int32_t from = (int32_t)(side >> (mip - 1)), to = (int32_t)(side >> mip);
    VkImageBlit blit = { .srcSubresource = { VK_IMAGE_ASPECT_COLOR_BIT, mip - 1, 0, layer_count }, .srcOffsets = { { 0, 0, 0 }, { from, from, 1 } },
      .dstSubresource = { VK_IMAGE_ASPECT_COLOR_BIT, mip, 0, layer_count }, .dstOffsets = { { 0, 0, 0 }, { to, to, 1 } } };
    vkCmdBlitImage(g.cmd, g.sheets.image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, g.sheets.image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &blit, VK_FILTER_LINEAR);
  }
  layout(g.sheets.image, 0, mips - 1, layer_count, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
  layout(g.sheets.image, mips - 1, 1, layer_count, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
  once_end();
  drop_buffer(&staging);
}

// ---- The window ----

static void window_close(void) {
  if (!g.device) return;
  vkDeviceWaitIdle(g.device);
  for (uint32_t i = 0; i < g.image_count; i++) { vkDestroyFramebuffer(g.device, g.frames[i], NULL); vkDestroyImageView(g.device, g.views[i], NULL); }
  g.image_count = 0;
  if (g.mirror_frame) { vkDestroyFramebuffer(g.device, g.mirror_frame, NULL); g.mirror_frame = VK_NULL_HANDLE; }
  drop_image(&g.depth); drop_image(&g.mirror); drop_image(&g.mirror_depth);
  if (g.swapchain) { vkDestroySwapchainKHR(g.device, g.swapchain, NULL); g.swapchain = VK_NULL_HANDLE; }
  g.ready = 0;
}
static void surface_close(void) {
  window_close();
  if (g.surface) { vkDestroySurfaceKHR(g.instance, g.surface, NULL); g.surface = VK_NULL_HANDLE; }
}

// The swapchain and what is its size: the depth, and the mirror image the floor shows.
static void window_open(void) {
  window_close();
  if (!g.app->window) return;
  if (!g.surface) {
    VkAndroidSurfaceCreateInfoKHR si = { .sType = VK_STRUCTURE_TYPE_ANDROID_SURFACE_CREATE_INFO_KHR, .window = g.app->window };
    VK(vkCreateAndroidSurfaceKHR(g.instance, &si, NULL, &g.surface));
    VkBool32 ok = VK_FALSE; vkGetPhysicalDeviceSurfaceSupportKHR(g.gpu, g.family, g.surface, &ok);
    if (!ok) STOP("Everglade: this GPU can't draw to the screen.");
  }
  VkSurfaceCapabilitiesKHR caps; VK(vkGetPhysicalDeviceSurfaceCapabilitiesKHR(g.gpu, g.surface, &caps));
  VkSurfaceFormatKHR formats[64]; uint32_t fn = 64; vkGetPhysicalDeviceSurfaceFormatsKHR(g.gpu, g.surface, &fn, formats);
  if (fn == 0) STOP("Everglade: the screen offers no picture format.");
  // The shaders write display colour themselves, so a format that doesn't encode it again.
  VkSurfaceFormatKHR pick = formats[0];
  for (uint32_t i = 0; i < fn; i++) if (formats[i].format == VK_FORMAT_R8G8B8A8_UNORM || formats[i].format == VK_FORMAT_B8G8R8A8_UNORM) { pick = formats[i]; break; }
  VkExtent2D ext = caps.currentExtent;
  if (ext.width == UINT32_MAX) { ext.width = (uint32_t)ANativeWindow_getWidth(g.app->window); ext.height = (uint32_t)ANativeWindow_getHeight(g.app->window); }
  if (ext.width == 0 || ext.height == 0) return;
  uint32_t want = caps.minImageCount + 1; if (caps.maxImageCount && want > caps.maxImageCount) want = caps.maxImageCount; if (want > 8) want = 8;
  VkCompositeAlphaFlagBitsKHR alpha = (caps.supportedCompositeAlpha & VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR) ? VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR : VK_COMPOSITE_ALPHA_INHERIT_BIT_KHR;
  // The picture is drawn upright for the way the screen is held, and the system turns it.
  VkSurfaceTransformFlagBitsKHR turn = (caps.supportedTransforms & VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR) ? VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR : caps.currentTransform;
  VkSwapchainCreateInfoKHR ci = { .sType = VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR, .surface = g.surface, .minImageCount = want, .imageFormat = pick.format,
    .imageColorSpace = pick.colorSpace, .imageExtent = ext, .imageArrayLayers = 1, .imageUsage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
    .preTransform = turn, .compositeAlpha = alpha, .presentMode = VK_PRESENT_MODE_FIFO_KHR, .clipped = VK_TRUE };
  VK(vkCreateSwapchainKHR(g.device, &ci, NULL, &g.swapchain));
  g.extent = ext;
  if (g.format != pick.format) {
    // The first window, or a screen with another format: the passes and pipelines are made for it.
    if (g.format != VK_FORMAT_UNDEFINED) STOP("Everglade: the screen's picture format changed, which the app doesn't follow yet.");
    g.format = pick.format;
  }
  VkImage images[8]; g.image_count = 8; vkGetSwapchainImagesKHR(g.device, g.swapchain, &g.image_count, images);
  if (!g.pass) {
    g.pass = render_pass(g.format, VK_ATTACHMENT_LOAD_OP_CLEAR, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR);
    g.mirror_pass = render_pass(VK_FORMAT_R16G16B16A16_SFLOAT, VK_ATTACHMENT_LOAD_OP_DONT_CARE, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL);
  }
  g.depth = image(VK_FORMAT_D32_SFLOAT, ext.width, ext.height, 1, 1, VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT, VK_IMAGE_ASPECT_DEPTH_BIT, 0);
  g.mirror = image(VK_FORMAT_R16G16B16A16_SFLOAT, ext.width, ext.height, 1, 1, VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_SAMPLED_BIT, VK_IMAGE_ASPECT_COLOR_BIT, 0);
  g.mirror_depth = image(VK_FORMAT_D32_SFLOAT, ext.width, ext.height, 1, 1, VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT, VK_IMAGE_ASPECT_DEPTH_BIT, 0);
  for (uint32_t i = 0; i < g.image_count; i++) {
    VkImageViewCreateInfo vi = { .sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO, .image = images[i], .viewType = VK_IMAGE_VIEW_TYPE_2D, .format = g.format,
      .subresourceRange = { VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1 } };
    VK(vkCreateImageView(g.device, &vi, NULL, &g.views[i]));
    VkImageView at[2] = { g.views[i], g.depth.view };
    VkFramebufferCreateInfo fi = { .sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO, .renderPass = g.pass, .attachmentCount = 2, .pAttachments = at, .width = ext.width, .height = ext.height, .layers = 1 };
    VK(vkCreateFramebuffer(g.device, &fi, NULL, &g.frames[i]));
  }
  VkImageView mat[2] = { g.mirror.view, g.mirror_depth.view };
  VkFramebufferCreateInfo mi = { .sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO, .renderPass = g.mirror_pass, .attachmentCount = 2, .pAttachments = mat, .width = ext.width, .height = ext.height, .layers = 1 };
  VK(vkCreateFramebuffer(g.device, &mi, NULL, &g.mirror_frame));
  if (g.place_set) put_image(g.place_set, 3, VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE, g.mirror.view, VK_NULL_HANDLE);
  g.ready = 1; g.resized = 0;
  SAY("window %ux%u", ext.width, ext.height);
}

// What is made once the first window has said its format: the layouts, sets and pipelines.
static void pipelines_open(void) {
  // An emitted Metaverse Engine shader takes its resources at fixed places: the uniform floats at 0, the
  // mesh at 1, the records at 2, and for the scene the mirror image at 3 and its sampler at 4.
  const VkDescriptorType place_types[5] = { VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE, VK_DESCRIPTOR_TYPE_SAMPLER };
  const VkDescriptorType model_types[5] = { VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER };
  const VkDescriptorType stroke_types[1] = { VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER };
  g.place_layout = set_layout(5, place_types); g.model_layout = set_layout(5, model_types); g.stroke_layout = set_layout(1, stroke_types);
  VkPipelineLayoutCreateInfo li = { .sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO, .setLayoutCount = 1, .pSetLayouts = &g.place_layout };
  VK(vkCreatePipelineLayout(g.device, &li, NULL, &g.place_pipes));
  li.pSetLayouts = &g.model_layout; VK(vkCreatePipelineLayout(g.device, &li, NULL, &g.model_pipes));
  VkPushConstantRange push = { VK_SHADER_STAGE_VERTEX_BIT, 0, 16 };
  li.pSetLayouts = &g.stroke_layout; li.pushConstantRangeCount = 1; li.pPushConstantRanges = &push;
  VK(vkCreatePipelineLayout(g.device, &li, NULL, &g.stroke_pipes));
  // A mirror image nothing has been drawn into yet, for the passes that don't read it.
  g.blank = image(VK_FORMAT_R16G16B16A16_SFLOAT, 1, 1, 1, 1, VK_IMAGE_USAGE_SAMPLED_BIT, VK_IMAGE_ASPECT_COLOR_BIT, 0);
  once_begin(); layout(g.blank.image, 0, 1, 1, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL); once_end();
  g.place_set = set(g.place_layout); g.mirror_set = set(g.place_layout); g.model_set = set(g.model_layout); g.stroke_set = set(g.stroke_layout);
  for (int k = 0; k < 2; k++) {
    VkDescriptorSet s = k ? g.mirror_set : g.place_set;
    put_buffer(s, 0, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, k ? &g.u_mirror : &g.u_place);
    put_buffer(s, 1, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, &g.mesh);
    put_buffer(s, 2, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, &g.records);
    put_image(s, 3, VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE, k ? g.blank.view : g.mirror.view, VK_NULL_HANDLE);
    put_image(s, 4, VK_DESCRIPTOR_TYPE_SAMPLER, VK_NULL_HANDLE, g.smooth);
  }
  put_buffer(g.model_set, 0, VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER, &g.u_model);
  put_buffer(g.model_set, 1, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, &g.model_vertices);
  put_buffer(g.model_set, 2, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, &g.model_records);
  put_buffer(g.model_set, 3, VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, &g.model_bake);
  put_image(g.model_set, 4, VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, g.sheets.view, g.mipped);
  put_image(g.stroke_set, 0, VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, g.faces.view, g.mipped);
  VkShaderModule sky = shader("temple-sky.spv"), scene = shader("temple-scene.spv"), light = shader("temple-light.spv");
  VkShaderModule mv = shader("model.vert.spv"), mf = shader("model.frag.spv"), sv = shader("strokes.vert.spv"), sf = shader("strokes.frag.spv");
  // The night behind everything, what stands, and what is only light added over it; the first
  // two again for the mirror image.
  g.sky = pipeline(sky, "vs", sky, "fs", g.place_pipes, g.pass, 0, VK_COMPARE_OP_ALWAYS, 0, 0, 0);
  g.sky_mirror = pipeline(sky, "vs", sky, "fs", g.place_pipes, g.mirror_pass, 0, VK_COMPARE_OP_ALWAYS, 0, 1, 0);
  g.scene = pipeline(scene, "vs", scene, "fs", g.place_pipes, g.pass, 0, VK_COMPARE_OP_LESS, 1, 0, 0);
  g.scene_mirror = pipeline(scene, "vs", scene, "fs", g.place_pipes, g.mirror_pass, 0, VK_COMPARE_OP_LESS, 1, 1, 0);
  g.light = pipeline(light, "vs", light, "fs", g.place_pipes, g.pass, 1, VK_COMPARE_OP_LESS, 0, 0, 0);
  g.model_pipe = pipeline(mv, "main", mf, "main", g.model_pipes, g.pass, 0, VK_COMPARE_OP_LESS, 1, 0, 0);
  g.strokes = pipeline(sv, "main", sf, "main", g.stroke_pipes, g.pass, 2, VK_COMPARE_OP_ALWAYS, 0, 0, 1);
  VkShaderModule all[7] = { sky, scene, light, mv, mf, sv, sf };
  for (int i = 0; i < 7; i++) vkDestroyShaderModule(g.device, all[i], NULL);
}

// ---- A frame ----

struct Step { float you[5]; float dt; };
static void model_step(void *p) {
  struct Step *s = p;
  g.model_draws = egm_frame(g.model, s->you, s->dt, g.model_records.map, g.draws_model);
}
static void group(int k) {
  for (int d = g.group_first[k]; d < g.group_first[k] + g.group_count[k]; d++) {
    uint32_t first = g.draws_place[d * 4], count = g.draws_place[d * 4 + 1], record = g.draws_place[d * 4 + 2], n = g.draws_place[d * 4 + 3];
    if (count > 0 && n > 0) vkCmdDraw(g.cmd, count, n, first, record);
  }
}
// Clip space is Metal's and the web's, y up, so the viewport is turned over.
static void view(void) {
  VkViewport v = { 0, (float)g.extent.height, (float)g.extent.width, -(float)g.extent.height, 0, 1 };
  VkRect2D s = { { 0, 0 }, g.extent };
  vkCmdSetViewport(g.cmd, 0, 1, &v); vkCmdSetScissor(g.cmd, 0, 1, &s);
}

// The edges the system's bars and the camera's cutout cover, in pixels: left, top, bottom.
static void insets(int32_t out[3]) {
  JNIEnv *e = g.env; out[0] = out[1] = out[2] = 0;
  if (!e) return;
  // This thread never returns to Java, so nothing frees its local references but this frame.
  if ((*e)->PushLocalFrame(e, 16) != 0) { (*e)->ExceptionClear(e); return; }
  jobject act = g.app->activity->clazz;
  jobject win = (*e)->CallObjectMethod(e, act, (*e)->GetMethodID(e, (*e)->GetObjectClass(e, act), "getWindow", "()Landroid/view/Window;"));
  jobject decor = win ? (*e)->CallObjectMethod(e, win, (*e)->GetMethodID(e, (*e)->GetObjectClass(e, win), "getDecorView", "()Landroid/view/View;")) : NULL;
  jobject in = decor ? (*e)->CallObjectMethod(e, decor, (*e)->GetMethodID(e, (*e)->GetObjectClass(e, decor), "getRootWindowInsets", "()Landroid/view/WindowInsets;")) : NULL;
  if (in && !(*e)->ExceptionCheck(e)) {
    jclass c = (*e)->GetObjectClass(e, in);
    out[0] = (*e)->CallIntMethod(e, in, (*e)->GetMethodID(e, c, "getSystemWindowInsetLeft", "()I"));
    out[1] = (*e)->CallIntMethod(e, in, (*e)->GetMethodID(e, c, "getSystemWindowInsetTop", "()I"));
    out[2] = (*e)->CallIntMethod(e, in, (*e)->GetMethodID(e, c, "getSystemWindowInsetBottom", "()I"));
  }
  if ((*e)->ExceptionCheck(e)) { (*e)->ExceptionClear(e); out[0] = out[1] = out[2] = 0; }
  (*e)->PopLocalFrame(e, NULL);
}

static float scale(void) {
  int32_t d = AConfiguration_getDensity(g.app->config);
  return d > 0 && d < 10000 ? (float)d / 160.0f : 1.0f;
}

// The account card and the login's thread: what the person asked for goes to the login, what the
// login says comes back to the card, and the keyboard follows the card's box.
static uint32_t login_seen = (uint32_t)-1; static int keyboard_up;
// The soft keyboard, up or down. ANativeActivity_showSoftInput asks for it on NativeActivity's
// content view, which Android ignores ("not served": the window's own view is the one the keyboard
// serves), so this asks InputMethodManager for it on the window's view.
static void keyboard(int up) {
  JNIEnv *e = g.env; jobject act = g.app->activity->clazz;
  if ((*e)->PushLocalFrame(e, 16) != 0) { (*e)->ExceptionClear(e); return; }
  jclass act_c = (*e)->GetObjectClass(e, act), ctx_c = (*e)->FindClass(e, "android/content/Context");
  jclass win_c = (*e)->FindClass(e, "android/view/Window"), view_c = (*e)->FindClass(e, "android/view/View");
  jclass imm_c = (*e)->FindClass(e, "android/view/inputmethod/InputMethodManager");
  if ((*e)->ExceptionCheck(e)) goto out;
  jmethodID service = (*e)->GetMethodID(e, act_c, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;");
  jmethodID window = (*e)->GetMethodID(e, act_c, "getWindow", "()Landroid/view/Window;");
  jmethodID decor = (*e)->GetMethodID(e, win_c, "getDecorView", "()Landroid/view/View;");
  jmethodID token = (*e)->GetMethodID(e, view_c, "getWindowToken", "()Landroid/os/IBinder;");
  jmethodID show = (*e)->GetMethodID(e, imm_c, "showSoftInput", "(Landroid/view/View;I)Z");
  jmethodID hide = (*e)->GetMethodID(e, imm_c, "hideSoftInputFromWindow", "(Landroid/os/IBinder;I)Z");
  jfieldID name = (*e)->GetStaticFieldID(e, ctx_c, "INPUT_METHOD_SERVICE", "Ljava/lang/String;");
  if ((*e)->ExceptionCheck(e)) goto out;
  jobject imm = (*e)->CallObjectMethod(e, act, service, (*e)->GetStaticObjectField(e, ctx_c, name));
  jobject w = (*e)->CallObjectMethod(e, act, window);
  if ((*e)->ExceptionCheck(e) || !imm || !w) goto out;
  jobject v = (*e)->CallObjectMethod(e, w, decor);
  if ((*e)->ExceptionCheck(e) || !v) goto out;
  if (up) (*e)->CallBooleanMethod(e, imm, show, v, (jint)0);
  else (*e)->CallBooleanMethod(e, imm, hide, (*e)->CallObjectMethod(e, v, token), (jint)0);
out:
  if ((*e)->ExceptionCheck(e)) (*e)->ExceptionClear(e);
  (*e)->PopLocalFrame(e, NULL);
}
static void login_poll(void) {
  char text[256]; int32_t n = 0;
  int32_t kind = eg_login_take(g.state, text, (int32_t)sizeof text, &n);
  if (kind != 0) oa_app_login_ask(kind, text, n);
  memset(text, 0, sizeof text);
  int st = 0; bool busy = false; char words[400], who[160];
  uint32_t gen = oa_app_login_status(&st, &busy, words, sizeof words, who, sizeof who);
  if (gen != login_seen) { login_seen = gen; eg_login_show(g.state, st, busy ? 1 : 0, words, who); }
  int want = eg_login_wants_keys(g.state);
  if (want != keyboard_up) { keyboard_up = want; keyboard(want); }
}

static void frame(void) {
  login_poll();
  double t = now(); float dt = (float)(t - g.last); g.last = t;
  if (dt > 0.1f) dt = 0.1f;
  if (g.jump_up > 0 && t >= g.jump_up) { g.jump_up = 0; eg_key(g.state, 32, 0); }
  float s = scale(), w = (float)g.extent.width, h = (float)g.extent.height;
  if (t >= g.inset_at) {
    int32_t in[3]; insets(in); g.inset_at = t + 1.0;
    eg_safe(g.state, (float)in[0] / s, (float)in[1] / s, (float)in[2] / s);
  }
  g.time += dt;
  int n = eg_frame(g.state, g.stroke_vertices.map, w / s, h / s, s, dt);
  float camera[24] = { 0 }, pu[24] = { 0 };
  struct Step step = { .dt = dt };
  if (eg_you(g.state, step.you) == 1) deep(model_step, &step); else g.model_draws = 0;
  int seen = eg_camera(g.state, w / s, h / s, s, camera) == 1;
  camera[16] = w; camera[17] = h; camera[18] = g.time;
  // The place's uniforms (everglade/workshop/gpu/temple.coax).
  pu[0] = w; pu[1] = h; pu[2] = g.time;
  for (int k = 0; k < 3; k++) { pu[4 + k] = camera[k]; pu[8 + k] = camera[4 + k]; pu[12 + k] = camera[8 + k]; pu[16 + k] = camera[12 + k]; pu[20 + k] = step.you[k]; }
  pu[19] = camera[15];

  VK(vkWaitForFences(g.device, 1, &g.fence, VK_TRUE, UINT64_MAX));
  uint32_t at = 0;
  VkResult r = vkAcquireNextImageKHR(g.device, g.swapchain, UINT64_MAX, g.acquired, VK_NULL_HANDLE, &at);
  if (r == VK_ERROR_OUT_OF_DATE_KHR) { g.resized = 1; return; }
  if (r != VK_SUCCESS && r != VK_SUBOPTIMAL_KHR) STOP("Everglade: the screen gave no picture to draw into (%d).", (int)r);
  VK(vkResetFences(g.device, 1, &g.fence));
  memcpy(g.u_place.map, pu, 96); pu[3] = 1; memcpy(g.u_mirror.map, pu, 96);
  memcpy(g.u_model.map, camera, 96);

  VkCommandBufferBeginInfo bi = { .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO, .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT };
  VK(vkResetCommandBuffer(g.cmd, 0)); VK(vkBeginCommandBuffer(g.cmd, &bi));
  VkClearValue clear[2] = { { .color = { .float32 = { 0, 0, 0, 1 } } }, { .depthStencil = { 1, 0 } } };
  VkRenderPassBeginInfo rb = { .sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO, .renderArea = { { 0, 0 }, g.extent }, .clearValueCount = 2, .pClearValues = clear };
  // What stands on the floor, upside down, into the mirror image.
  rb.renderPass = g.mirror_pass; rb.framebuffer = g.mirror_frame;
  vkCmdBeginRenderPass(g.cmd, &rb, VK_SUBPASS_CONTENTS_INLINE);
  view();
  vkCmdBindDescriptorSets(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.place_pipes, 0, 1, &g.mirror_set, 0, NULL);
  vkCmdBindPipeline(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.sky_mirror); vkCmdDraw(g.cmd, 3, 1, 0, 0);
  if (seen) { vkCmdBindPipeline(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.scene_mirror); group(0); }
  vkCmdEndRenderPass(g.cmd);
  // The night, what stands, the floor over the mirror image, you, what is only light, and the text.
  rb.renderPass = g.pass; rb.framebuffer = g.frames[at];
  vkCmdBeginRenderPass(g.cmd, &rb, VK_SUBPASS_CONTENTS_INLINE);
  view();
  if (seen) {
    vkCmdBindDescriptorSets(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.place_pipes, 0, 1, &g.place_set, 0, NULL);
    vkCmdBindPipeline(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.sky); vkCmdDraw(g.cmd, 3, 1, 0, 0);
    vkCmdBindPipeline(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.scene); group(0); group(1);
    if (g.model_draws > 0) {
      vkCmdBindPipeline(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.model_pipe);
      vkCmdBindDescriptorSets(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.model_pipes, 0, 1, &g.model_set, 0, NULL);
      vkCmdBindIndexBuffer(g.cmd, g.model_indices.buffer, 0, VK_INDEX_TYPE_UINT32);
      for (int k = 0; k < g.model_draws; k++) {
        uint32_t first = g.draws_model[k * 4], count = g.draws_model[k * 4 + 1], record = g.draws_model[k * 4 + 2];
        if (count > 0 && (uint64_t)first + count <= g.index_count) vkCmdDrawIndexed(g.cmd, count, 1, first, 0, record);
      }
    }
    vkCmdBindDescriptorSets(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.place_pipes, 0, 1, &g.place_set, 0, NULL);
    vkCmdBindPipeline(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.light); group(2);
  }
  if (n > 0) {
    float u[4] = { w, h, g.time, 0 }; VkDeviceSize zero = 0;
    vkCmdBindPipeline(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.strokes);
    vkCmdBindDescriptorSets(g.cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, g.stroke_pipes, 0, 1, &g.stroke_set, 0, NULL);
    vkCmdPushConstants(g.cmd, g.stroke_pipes, VK_SHADER_STAGE_VERTEX_BIT, 0, 16, u);
    vkCmdBindVertexBuffers(g.cmd, 0, 1, &g.stroke_vertices.buffer, &zero);
    vkCmdDraw(g.cmd, (uint32_t)n, 1, 0, 0);
  }
  vkCmdEndRenderPass(g.cmd);
  VK(vkEndCommandBuffer(g.cmd));
  VkPipelineStageFlags wait = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
  VkSubmitInfo si = { .sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .waitSemaphoreCount = 1, .pWaitSemaphores = &g.acquired, .pWaitDstStageMask = &wait,
    .commandBufferCount = 1, .pCommandBuffers = &g.cmd, .signalSemaphoreCount = 1, .pSignalSemaphores = &g.drawn };
  VK(vkQueueSubmit(g.queue, 1, &si, g.fence));
  VkPresentInfoKHR pi = { .sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR, .waitSemaphoreCount = 1, .pWaitSemaphores = &g.drawn, .swapchainCount = 1, .pSwapchains = &g.swapchain, .pImageIndices = &at };
  r = vkQueuePresentKHR(g.queue, &pi);
  if (r == VK_ERROR_OUT_OF_DATE_KHR) g.resized = 1;
  else if (r != VK_SUCCESS && r != VK_SUBOPTIMAL_KHR) STOP("Everglade: the picture could not be shown (%d).", (int)r);
  // A turned screen: the window is another size than the picture.
  if (g.app->window && ((uint32_t)ANativeWindow_getWidth(g.app->window) != g.extent.width || (uint32_t)ANativeWindow_getHeight(g.app->window) != g.extent.height)) g.resized = 1;
}

// ---- Touch ----

static void let_go(void) {
  eg_key(g.state, -1, 1); eg_pad(g.state, 0, 0);
  g.stick = g.look = g.pinch = -1;
}
// The character a key makes, as Java's KeyEvent.getUnicodeChar says (the keyboard's own map, with
// Shift and the rest): 0 for none.
static int key_char(const AInputEvent *ev) {
  JNIEnv *e = g.env;
  jclass c = (*e)->FindClass(e, "android/view/KeyEvent");
  if ((*e)->ExceptionCheck(e) || !c) { (*e)->ExceptionClear(e); return 0; }
  jmethodID make = (*e)->GetMethodID(e, c, "<init>", "(JJIIII)V"), get = (*e)->GetMethodID(e, c, "getUnicodeChar", "(I)I");
  if ((*e)->ExceptionCheck(e) || !make || !get) { (*e)->ExceptionClear(e); (*e)->DeleteLocalRef(e, c); return 0; }
  int32_t meta = AKeyEvent_getMetaState(ev);
  jobject k = (*e)->NewObject(e, c, make, (jlong)AKeyEvent_getDownTime(ev), (jlong)AKeyEvent_getEventTime(ev), (jint)AKeyEvent_getAction(ev), (jint)AKeyEvent_getKeyCode(ev), (jint)AKeyEvent_getRepeatCount(ev), (jint)meta);
  int ch = 0;
  if (!(*e)->ExceptionCheck(e) && k) { ch = (*e)->CallIntMethod(e, k, get, (jint)meta); }
  if ((*e)->ExceptionCheck(e)) { (*e)->ExceptionClear(e); ch = 0; }
  if (k) (*e)->DeleteLocalRef(e, k);
  (*e)->DeleteLocalRef(e, c);
  return ch;
}
// A key: Back closes the account card; while its box takes typing, the rest go to it.
static int32_t key(AInputEvent *ev) {
  int32_t code = AKeyEvent_getKeyCode(ev), down = AKeyEvent_getAction(ev) == AKEY_EVENT_ACTION_DOWN;
  if (code == AKEYCODE_BACK) {
    if (!eg_login_open(g.state)) return 0;
    if (down) { eg_key(g.state, 27, 1); eg_key(g.state, 27, 0); }
    return 1;
  }
  if (!eg_login_wants_keys(g.state)) return 0;
  if (!down) return 1;
  if (code == AKEYCODE_DEL) return eg_login_char(g.state, 8);
  if (code == AKEYCODE_ENTER || code == AKEYCODE_NUMPAD_ENTER) return eg_login_char(g.state, 13);
  int ch = key_char(ev);
  return ch > 0 ? eg_login_char(g.state, ch) : 0;
}
static int32_t input(struct android_app *app, AInputEvent *ev) {
  (void)app;
  if (AInputEvent_getType(ev) == AINPUT_EVENT_TYPE_KEY) return g.state ? key(ev) : 0;
  if (AInputEvent_getType(ev) != AINPUT_EVENT_TYPE_MOTION) return 0;
  int32_t action = AMotionEvent_getAction(ev), what = action & AMOTION_EVENT_ACTION_MASK;
  size_t which = (size_t)((action & AMOTION_EVENT_ACTION_POINTER_INDEX_MASK) >> AMOTION_EVENT_ACTION_POINTER_INDEX_SHIFT);
  size_t count = AMotionEvent_getPointerCount(ev);
  float s = scale(), w = (float)g.extent.width / s;
  if (what == AMOTION_EVENT_ACTION_DOWN || what == AMOTION_EVENT_ACTION_POINTER_DOWN) {
    int32_t id = AMotionEvent_getPointerId(ev, which); float x = AMotionEvent_getX(ev, which) / s, y = AMotionEvent_getY(ev, which) / s;
    // A touch on the list of worlds or on a button is a press, never the stick.
    if (g.stick < 0 && x < w * 0.45f && eg_claims(g.state, x, y) == 0) { g.stick = id; g.stick_x = x; g.stick_y = y; }
    else if (g.look < 0) { g.look = id; g.look_moved = 0; g.look_down = now(); eg_pointer(g.state, x, y, 0); }
    else if (g.pinch < 0) { g.pinch = id; g.pinch_span = 0; g.look_moved = 1000; }
    return 1;
  }
  if (what == AMOTION_EVENT_ACTION_MOVE) {
    float lx = 0, ly = 0, px = 0, py = 0; int has_look = 0, has_pinch = 0;
    for (size_t i = 0; i < count; i++) {
      int32_t id = AMotionEvent_getPointerId(ev, i); float x = AMotionEvent_getX(ev, i) / s, y = AMotionEvent_getY(ev, i) / s;
      // The stick's whole travel is 70 points; forward is up the screen.
      if (id == g.stick) eg_pad(g.state, (x - g.stick_x) / 70.0f, (g.stick_y - y) / 70.0f);
      else if (id == g.look) { lx = x; ly = y; has_look = 1; }
      else if (id == g.pinch) { px = x; py = y; has_pinch = 1; }
    }
    if (has_look && has_pinch) {
      // Two fingers on the look side: how far apart they are zooms, and neither looks.
      float span = hypotf(lx - px, ly - py);
      if (g.pinch_span > 1 && span > 1) eg_wheel(g.state, (1.0f - span / g.pinch_span) * 220.0f);
      g.pinch_span = span;
    } else if (has_look && g.pinch < 0) {
      size_t hist = AMotionEvent_getHistorySize(ev);
      for (size_t i = 0; i < count && hist > 0; i++)
        if (AMotionEvent_getPointerId(ev, i) == g.look)
          g.look_moved += fabsf(lx - AMotionEvent_getHistoricalX(ev, i, hist - 1) / s) + fabsf(ly - AMotionEvent_getHistoricalY(ev, i, hist - 1) / s);
      if (hist == 0) g.look_moved += 0.5f;
      eg_pointer(g.state, lx, ly, 1);
    }
    return 1;
  }
  if (what == AMOTION_EVENT_ACTION_UP || what == AMOTION_EVENT_ACTION_POINTER_UP || what == AMOTION_EVENT_ACTION_CANCEL) {
    int tapped = what != AMOTION_EVENT_ACTION_CANCEL;
    for (size_t i = 0; i < count; i++) {
      if (what != AMOTION_EVENT_ACTION_CANCEL && i != which) continue;
      int32_t id = AMotionEvent_getPointerId(ev, i); float x = AMotionEvent_getX(ev, i) / s, y = AMotionEvent_getY(ev, i) / s;
      if (id == g.stick) { g.stick = -1; eg_pad(g.state, 0, 0); }
      else if (id == g.pinch) {
        g.pinch = -1;
        // The looking finger moved while it pinched and none of that was a look: it starts again
        // from where it is now, or the camera would jump by all of it.
        for (size_t k = 0; k < count && what != AMOTION_EVENT_ACTION_CANCEL; k++)
          if (AMotionEvent_getPointerId(ev, k) == g.look) eg_pointer(g.state, AMotionEvent_getX(ev, k) / s, AMotionEvent_getY(ev, k) / s, 0);
      }
      else if (id == g.look) {
        g.look = -1; g.pinch = -1;
        eg_pointer(g.state, x, y, 2);
        // A finger down and up in one place, quickly, is a tap: on the list or a button it is theirs,
        // anywhere else it is the jump key, held a moment.
        if (tapped && g.look_moved < 12 && now() - g.look_down < 0.35 && eg_tap(g.state, x, y) == 0) { eg_key(g.state, 32, 1); g.jump_up = now() + 0.1; }
      }
    }
    return 1;
  }
  return 0;
}

static void command(struct android_app *app, int32_t cmd) {
  (void)app;
  switch (cmd) {
    case APP_CMD_INIT_WINDOW: g.resized = 1; break;
    case APP_CMD_TERM_WINDOW: surface_close(); break;
    case APP_CMD_WINDOW_RESIZED: case APP_CMD_CONFIG_CHANGED: case APP_CMD_CONTENT_RECT_CHANGED: g.resized = 1; g.inset_at = 0; break;
    case APP_CMD_GAINED_FOCUS: g.focused = 1; g.last = now(); break;
    // Losing the screen lets every key and finger go.
    case APP_CMD_LOST_FOCUS: g.focused = 0; if (g.state) let_go(); break;
    default: break;
  }
}

void android_main(struct android_app *app) {
  memset(&g, 0, sizeof g);
  g.app = app; g.stick = g.look = g.pinch = -1; g.focused = 1;
  app->onAppCmd = command; app->onInputEvent = input;
  (*app->activity->vm)->AttachCurrentThread(app->activity->vm, &g.env, NULL);
  // A debug check of the Keystore device key (apps/shared/host/android/device-key.c, #13216):
  // `adb shell setprop debug.oa.devicekey 1`, open the app, and read logcat's "oa-device-key".
  { char v[PROP_VALUE_MAX] = ""; if (__system_property_get("debug.oa.devicekey", v) > 0 && v[0] == '1') oa_device_key_selftest(g.env); }
  deep_start();
  g.state = eg_new();
  if (!g.state) STOP("Everglade: no memory for the scene.");
  eg_pad(g.state, 0, 0);   // a touch screen: the scene shows the touch hint
  // Your account. A development build may log in somewhere else:
  // `adb shell setprop debug.oa.origin https://staging.openagents.com`.
  {
    char origin[PROP_VALUE_MAX] = "";
    if (__system_property_get("debug.oa.origin", origin) <= 0 || strncmp(origin, "https://", 8) != 0) snprintf(origin, sizeof origin, "%s", "https://openagents.com");
    if (oa_app_login_start(app->activity->vm, app->activity->internalDataPath, origin)) eg_login_offer(g.state);
  }
  g.last = now();
  int opened = 0, loaded = 0, frames = 0; double since = now();
  for (;;) {
    int events; struct android_poll_source *source;
    // Asleep while there is no window to draw into.
    while (ALooper_pollOnce(g.ready || (app->window && g.resized) ? 0 : -1, NULL, &events, (void **)&source) >= 0) {
      if (source) source->process(app, source);
      if (app->destroyRequested) {
        surface_close();
        (*app->activity->vm)->DetachCurrentThread(app->activity->vm);
        // The process ends with its one activity: the scene and the GPU's objects go with it.
        exit(0);
      }
    }
    if (app->window && g.resized) {
      // Once, even when the first window has no size yet and the pipelines wait for the next.
      if (!loaded) { gpu_open(); faces_open(); place_open(); model_open(); g.stroke_vertices = buffer((VkDeviceSize)eg_max_vertices() * eg_vertex_floats() * 4, VK_BUFFER_USAGE_VERTEX_BUFFER_BIT); loaded = 1; }
      window_open();
      if (!opened && g.ready) { pipelines_open(); opened = 1; }
    }
    if (g.ready && opened) {
      frame();
      if (++frames == 300) { double t = now(); SAY("%.1f frames a second", 300.0 / (t - since)); frames = 0; since = t; }
    }
  }
}
