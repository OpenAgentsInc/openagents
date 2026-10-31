// Scene pipelines (docs/verse/native-gpu.md, G4 and the scene host): a
// module from `coax spirv --vertex V --fragment F` drawn with its bindings
// (storage buffers, textures, samplers at set 0, in the order the lowering
// reports them) into color targets of any format and an optional depth
// target. Targets end each pass ready to sample, so a later pass reads them.
#ifndef VKS_H
#define VKS_H
#include "vkc.h"

// An image: a texture (sampled, filled by vks_upload) or a target.
typedef struct {
  VkImage image;
  VkDeviceMemory mem;
  VkImageView view;
  VkFormat format;
  uint32_t w, h, layers;
  int depth;
} VksImage;

enum { VKS_TEXTURE = 1, VKS_TARGET = 2 };
// A w by h image of `layers` layers (a 2D array view when layers > 1, or
// when `array` is set) for VKS_TEXTURE or VKS_TARGET use, or both.
VksImage vks_image(Vkc *v, uint32_t w, uint32_t h, uint32_t layers, int array, VkFormat format, int use);
// All of a texture's layers from bytes (tightly packed, layer after layer);
// it is left ready to sample.
void vks_upload(Vkc *v, VksImage *t, const void *bytes, size_t size);
// Layer 0 of an image that is ready to sample, into bytes (tightly packed).
void vks_read(Vkc *v, VksImage *t, void *bytes, size_t size);
void vks_image_free(Vkc *v, VksImage *t);

enum { VKS_NEAREST = 0, VKS_LINEAR = 1 };
// A sampler: clamped to the edge, or repeating; with `compare`, a
// comparison sampler (less-or-equal, as gpu_compare).
VkSampler vks_sampler(Vkc *v, int filter, int repeat, int compare);

enum { VKS_STORAGE = 1, VKS_IMAGE = 2, VKS_SAMPLER = 3 };
typedef struct {
  int kind;
  const VkcBuffer *buffer;
  const VksImage *image;
  VkSampler sampler;
} VksBinding;

enum { VKS_REPLACE = 0, VKS_ADD = 1, VKS_ALPHA = 2 };
typedef struct {
  const uint32_t *words;
  size_t count;
  const VksBinding *bindings;
  uint32_t nb;
  // Vertex attributes, if the vertex entry takes them: one buffer, stride bytes a vertex.
  const VkcBuffer *vertices;
  uint32_t stride, nattrs;
  VkVertexInputAttributeDescription attrs[8];
  uint32_t vertex_count, instance_count;
  VksImage *targets[8];
  uint32_t ntargets;
  VksImage *depth;
  int blend, depth_test, depth_write, cull_back;
  int keep;          // keep what the targets hold, else clear them
  float clear[4];    // color targets' clear value; depth clears to 1
} VksPass;

typedef struct {
  VkRenderPass rp;
  VkFramebuffer fb;
  VkShaderModule module;
  VkDescriptorSetLayout dsl;
  VkPipelineLayout layout;
  VkPipeline pipeline;
  VkDescriptorPool pool;
  VkDescriptorSet set;
  uint32_t w, h;
} VksPipe;

void vks_pipe_open(Vkc *v, VksPipe *p, const VksPass *pass);
void vks_pipe_record(VksPipe *p, VkCommandBuffer cmd, const VksPass *pass);
void vks_pipe_close(Vkc *v, VksPipe *p);
// Records with `record` into a one-time command buffer, submits it and waits.
void vks_submit(Vkc *v, void (*record)(VkCommandBuffer cmd, void *user), void *user);
#endif
