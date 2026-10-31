// A pass spec (vk-passes.c has the directives; verse/gpu/everglade-pass.coax writes them) as a
// graph kept on the GPU: its buffers, textures, samplers and targets made once, each pass's
// pipeline made once, and the whole spec recorded into a command buffer as often as wanted.
// A spec may name counts: `let NAME V` sets one, `$NAME` uses one in a pass's calls or the
// draw list's length, and `each draws:A:B` ... `end` records the passes between once for each
// of the list's records A to B, `draws:each` being that one record. A host that draws every
// frame (vk-realm.c) sets the counts with vk_spec_let and writes the buffers in place.
#ifndef VK_SPEC_H
#define VK_SPEC_H
#include "vks.h"

// Where a spec's files come from. With `file` NULL, or when it returns NULL, they are read from
// disk. Else it gives the bytes of NAME (size bytes) and the capacity to allocate for them
// (at least size: a host that rewrites a buffer each frame asks for room to grow).
typedef struct {
  const void *(*file)(void *user, const char *name, size_t *size, size_t *capacity);
  void *user;
  uint32_t w, h;   // the colour targets' size instead of the spec's `size`, if not 0
} VkSpecFiles;

typedef struct VkSpec VkSpec;
// The spec at path, every resource and pipeline made; NULL (after saying why) if it is not one.
VkSpec *vk_spec_open(Vkc *v, const char *path, const VkSpecFiles *files);
// A count the spec names; 0 if it names no such count.
int vk_spec_let(VkSpec *s, const char *name, long value);
// A buffer the spec makes (`buffer NAME FILE`, `floats NAME`, `index` as "index"), by NAME; NULL if none.
VkcBuffer *vk_spec_buffer(VkSpec *s, const char *name);
// The draw list the passes draw from (else the spec's file).
void vk_spec_draws(VkSpec *s, const uint32_t *draws);
// Every directive in order into cmd: the copies and the passes.
void vk_spec_record(VkSpec *s, VkCommandBuffer cmd);
// The last pass's colour targets (it must have some), and how many.
VksImage **vk_spec_last(VkSpec *s, int *n);
uint32_t vk_spec_width(const VkSpec *s);
uint32_t vk_spec_height(const VkSpec *s);
int vk_spec_passes(const VkSpec *s);
// The draws recorded the last time (each record counted once a pass).
uint32_t vk_spec_drawn(const VkSpec *s);
void vk_spec_close(Vkc *v, VkSpec *s);
#endif
