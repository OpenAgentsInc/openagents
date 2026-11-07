// Additive owned queue submission. A successful begin is accepted work,
// not completed work. Pending resources survive timeout/unknown errors.
#ifndef VKS_SUBMIT_H
#define VKS_SUBMIT_H
#include "vkc.h"
typedef struct {
  VkDevice device;
  VkCommandPool pool;
  VkCommandBuffer command;
  VkFence fence;
  int pending;
  VkResult result;
} VksSubmission;
VkResult vks_submission_begin(Vkc *v,VksSubmission *s,void (*record)(VkCommandBuffer,void*),void *user);
VkResult vks_submission_poll(Vkc *v,VksSubmission *s,uint64_t timeout_ns);
// Refuses a pending submission or another device. Idempotent once empty.
VkResult vks_submission_retire(Vkc *v,VksSubmission *s);
#endif
