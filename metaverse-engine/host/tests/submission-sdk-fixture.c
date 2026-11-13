// Firm reason: 2 (independent counted Vulkan API model; Coax drives the SDK under test).
#include "vks-submit.h"
#include <stdint.h>
#include <string.h>
typedef struct {int64_t *data;uint64_t count;} UnusedView;
static Vkc fixture;
static VksSubmission submission;
static int fail_stage,fail_code,stage,pools,fences,in_flight,violations,recorded,queries,signal_result;
static VkResult effect(void){stage++;return stage==fail_stage?(VkResult)fail_code:VK_SUCCESS;}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateCommandPool(VkDevice d,const VkCommandPoolCreateInfo *c,const VkAllocationCallbacks *a,VkCommandPool *out){(void)d;(void)c;(void)a;VkResult r=effect();if(!r){*out=(VkCommandPool)(uintptr_t)11;pools++;}else *out=(VkCommandPool)(uintptr_t)0xdead;return r;}
VKAPI_ATTR VkResult VKAPI_CALL vkAllocateCommandBuffers(VkDevice d,const VkCommandBufferAllocateInfo *a,VkCommandBuffer *out){(void)d;(void)a;VkResult r=effect();if(!r)*out=(VkCommandBuffer)(uintptr_t)12;return r;}
VKAPI_ATTR VkResult VKAPI_CALL vkBeginCommandBuffer(VkCommandBuffer c,const VkCommandBufferBeginInfo *i){(void)c;(void)i;return effect();}
VKAPI_ATTR VkResult VKAPI_CALL vkEndCommandBuffer(VkCommandBuffer c){(void)c;return effect();}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateFence(VkDevice d,const VkFenceCreateInfo *c,const VkAllocationCallbacks *a,VkFence *out){(void)d;(void)c;(void)a;VkResult r=effect();if(!r){*out=(VkFence)(uintptr_t)13;fences++;}else *out=(VkFence)(uintptr_t)0xbeef;return r;}
VKAPI_ATTR VkResult VKAPI_CALL vkQueueSubmit(VkQueue q,uint32_t n,const VkSubmitInfo *i,VkFence f){(void)q;(void)n;(void)i;(void)f;VkResult r=effect();if(!r)in_flight=1;return r;}
static VkResult observe(int wait){queries++;VkResult r=signal_result==12345?(wait?VK_TIMEOUT:VK_NOT_READY):(VkResult)signal_result;if(r==VK_SUCCESS||r==VK_ERROR_DEVICE_LOST)in_flight=0;return r;}
VKAPI_ATTR VkResult VKAPI_CALL vkGetFenceStatus(VkDevice d,VkFence f){(void)d;(void)f;return observe(0);}
VKAPI_ATTR VkResult VKAPI_CALL vkWaitForFences(VkDevice d,uint32_t n,const VkFence *f,VkBool32 all,uint64_t t){(void)d;(void)n;(void)f;(void)all;(void)t;return observe(1);}
VKAPI_ATTR void VKAPI_CALL vkDestroyFence(VkDevice d,VkFence f,const VkAllocationCallbacks *a){(void)d;(void)f;(void)a;if(in_flight||fences!=1||f!=(VkFence)(uintptr_t)13)violations++;fences--;}
VKAPI_ATTR void VKAPI_CALL vkDestroyCommandPool(VkDevice d,VkCommandPool p,const VkAllocationCallbacks *a){(void)d;(void)p;(void)a;if(in_flight||pools!=1||p!=(VkCommandPool)(uintptr_t)11)violations++;pools--;}
static void record(VkCommandBuffer cmd,void *user){(void)cmd;(void)user;recorded++;}
void vsf_reset(int32_t at,int32_t code){memset(&fixture,0,sizeof fixture);memset(&submission,0,sizeof submission);fixture.dev=(VkDevice)(uintptr_t)1;fail_stage=at;fail_code=code;stage=pools=fences=in_flight=violations=recorded=queries=0;signal_result=12345;}
int32_t vsf_begin(void){return vks_submission_begin(&fixture,&submission,record,NULL);}
int32_t vsf_poll(int32_t wait,int32_t wrong){Vkc device=fixture;if(wrong)device.dev=(VkDevice)(uintptr_t)2;return vks_submission_poll(&device,&submission,wait?1:0);}
int32_t vsf_retire(int32_t wrong){Vkc device=fixture;if(wrong)device.dev=(VkDevice)(uintptr_t)2;return vks_submission_retire(&device,&submission);}
void vsf_signal(int32_t result){signal_result=result;}
int64_t vsf_info(int32_t f){switch(f){case 0:return pools;case 1:return fences;case 2:return submission.pending;case 3:return violations;case 4:return recorded;case 5:return queries;default:return -1;}}
