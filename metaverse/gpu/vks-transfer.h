// Additive owned buffer/RGBA8 transfers. Begin accepts work; it does not wait.
// The caller retains both resources until a terminal poll and retire. Timeout
// and unknown errors preserve the submission and its complete resource views.
#ifndef VKS_TRANSFER_H
#define VKS_TRANSFER_H
#include "vks.h"
#include "vks-submit.h"
typedef struct {
  VksSubmission submission;
  VkcBuffer source,buffer;
  VksImage image;
  VkDeviceSize bytes;
  int kind;
} VksTransfer;
VkResult vks_transfer_buffer_begin(Vkc *v,VksTransfer *out,const VkcBuffer *source,const VkcBuffer *to,VkDeviceSize bytes);
// The initial upload of all tightly packed levels/layers of an admitted RGBA8
// UNORM texture. Other formats and re-uploading a used image are not this API.
VkResult vks_transfer_rgba8_begin(Vkc *v,VksTransfer *out,const VkcBuffer *source,const VksImage *to);
VkResult vks_transfer_poll(Vkc *v,VksTransfer *t,uint64_t timeout_ns);
VkResult vks_transfer_retire(Vkc *v,VksTransfer *t);
#endif
