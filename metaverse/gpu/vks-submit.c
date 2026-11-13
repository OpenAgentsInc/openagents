// Vulkan effects only. The owning Coax carrier decides deadlines/recovery.
#include "vks-submit.h"
#include <string.h>
VkResult vks_submission_retire(Vkc *v,VksSubmission *s){
  if(!v||!s)return VK_ERROR_INITIALIZATION_FAILED;
  if(!s->device)return VK_SUCCESS;
  if(s->device!=v->dev)return VK_ERROR_INITIALIZATION_FAILED;
  if(s->pending)return VK_NOT_READY;
  if(s->fence)vkDestroyFence(v->dev,s->fence,NULL);
  if(s->pool)vkDestroyCommandPool(v->dev,s->pool,NULL);
  memset(s,0,sizeof *s);return VK_SUCCESS;
}
VkResult vks_submission_begin(Vkc *v,VksSubmission *s,void (*record)(VkCommandBuffer,void*),void *user){
  if(!v||!v->dev||!s||s->device||!record)return VK_ERROR_INITIALIZATION_FAILED;
  s->device=v->dev;VkResult result;
  VkCommandPoolCreateInfo pool={.sType=VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,.queueFamilyIndex=v->family};
  VkCommandPool created_pool;
  if((result=vkCreateCommandPool(v->dev,&pool,NULL,&created_pool))!=VK_SUCCESS)goto failed;
  s->pool=created_pool;
  VkCommandBufferAllocateInfo allocate={.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,.commandPool=s->pool,.level=VK_COMMAND_BUFFER_LEVEL_PRIMARY,.commandBufferCount=1};
  VkCommandBuffer created_command;
  if((result=vkAllocateCommandBuffers(v->dev,&allocate,&created_command))!=VK_SUCCESS)goto failed;
  s->command=created_command;
  VkCommandBufferBeginInfo begin={.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,.flags=VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT};
  if((result=vkBeginCommandBuffer(s->command,&begin))!=VK_SUCCESS)goto failed;
  record(s->command,user);
  if((result=vkEndCommandBuffer(s->command))!=VK_SUCCESS)goto failed;
  VkFenceCreateInfo fence={.sType=VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
  VkFence created_fence;
  if((result=vkCreateFence(v->dev,&fence,NULL,&created_fence))!=VK_SUCCESS)goto failed;
  s->fence=created_fence;
  VkSubmitInfo submit={.sType=VK_STRUCTURE_TYPE_SUBMIT_INFO,.commandBufferCount=1,.pCommandBuffers=&s->command};
  if((result=vkQueueSubmit(v->queue,1,&submit,s->fence))!=VK_SUCCESS)goto failed;
  s->pending=1;s->result=VK_NOT_READY;return VK_SUCCESS;
failed:
  // Failed queue submission either leaves no work or reports device loss;
  // Vulkan treats device loss as completion for pending-use lifetime rules.
  vks_submission_retire(v,s);return result;
}
VkResult vks_submission_poll(Vkc *v,VksSubmission *s,uint64_t timeout_ns){
  if(!v||!s||!s->device||s->device!=v->dev)return VK_ERROR_INITIALIZATION_FAILED;
  if(!s->pending)return s->result;
  VkResult result=timeout_ns?vkWaitForFences(v->dev,1,&s->fence,VK_TRUE,timeout_ns):vkGetFenceStatus(v->dev,s->fence);
  s->result=result;
  if(result==VK_SUCCESS||result==VK_ERROR_DEVICE_LOST)s->pending=0;
  return result;
}
