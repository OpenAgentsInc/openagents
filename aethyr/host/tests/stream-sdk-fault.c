// Firm reason: 2 (independent scoped Vulkan-prefix/retirement oracle over actual driver effects).
// Errors are explicit qualification inputs, never physical OOM/reset claims.
#include "stream-sdk-fault.h"
#include "vks.h"
#include <string.h>
#include <time.h>
static struct {
  int64_t q[26];uintptr_t handles[8];int scoped;
} fault;
static int64_t now_ns(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return(int64_t)t.tv_sec*1000000000+t.tv_nsec;}
int32_t sw_sdk_fault_open(int32_t kind,int32_t domain,int32_t phase){
  int limit=domain==3?6:domain==4?3:4;
  if(fault.q[0]||(kind!=1&&kind!=2)||domain<1||domain>4||phase<1||phase>limit)return 0;
  fault.q[0]=kind;fault.q[1]=domain;fault.q[2]=phase;return 1;
}
void sw_sdk_scope(int32_t domain,int32_t kind,int32_t row,int64_t generation,int64_t epoch){
  if(!domain){if(fault.scoped&&fault.q[1]<4)fault.q[21]=now_ns();fault.scoped=0;return;}
  fault.scoped=0;if(!fault.q[0]||domain!=fault.q[1]||kind!=fault.q[0]||(domain<4&&fault.q[4])||(domain==4&&fault.q[25]))return;
  if(!fault.q[3]){fault.q[3]=1;fault.q[5]=row;fault.q[6]=generation;fault.q[7]=epoch;fault.q[9]=INT32_MIN;fault.q[20]=now_ns();}
  if(row!=fault.q[5]||generation!=fault.q[6]||epoch!=fault.q[7])return;
  fault.scoped=1;
}
int64_t sw_sdk_fault_info(int32_t field){if(field==24)return sizeof fault;return field>=0&&field<26?fault.q[field]:-1;}
static int inject(void){
  if(!fault.scoped||fault.q[1]==4)return 0;
  fault.q[11]++;if(!fault.q[4]&&fault.q[11]==fault.q[2]){fault.q[4]=1;fault.q[8]=VK_ERROR_OUT_OF_HOST_MEMORY;return 1;}return 0;
}
static VkResult observed(VkResult r){if(fault.scoped){fault.q[10]++;fault.q[9]=r;}return r;}
static void made(int slot,uintptr_t handle){if(!fault.scoped)return;if(fault.q[12+slot])fault.q[23]++;fault.handles[slot]=handle;fault.q[12+slot]++;
  int64_t live=0;for(int i=12;i<20;i++)live+=fault.q[i];if(live>fault.q[22])fault.q[22]=live;
}
static int drop(int slot,uintptr_t handle){
  if(!fault.scoped)return 1;
  if(fault.q[12+slot]!=1||fault.handles[slot]!=handle){fault.q[23]++;return 0;}fault.q[12+slot]=0;fault.handles[slot]=0;return 1;
}
#define POISON(T) ((T)(uintptr_t)0xdead)
#define RESULT(NAME,ARGS) extern VkResult __real_##NAME ARGS
#define VOID(NAME,ARGS) extern void __real_##NAME ARGS
RESULT(vkCreateBuffer,(VkDevice,const VkBufferCreateInfo*,const VkAllocationCallbacks*,VkBuffer*));
RESULT(vkCreateImage,(VkDevice,const VkImageCreateInfo*,const VkAllocationCallbacks*,VkImage*));
RESULT(vkAllocateMemory,(VkDevice,const VkMemoryAllocateInfo*,const VkAllocationCallbacks*,VkDeviceMemory*));
RESULT(vkBindBufferMemory,(VkDevice,VkBuffer,VkDeviceMemory,VkDeviceSize));
RESULT(vkBindImageMemory,(VkDevice,VkImage,VkDeviceMemory,VkDeviceSize));
RESULT(vkMapMemory,(VkDevice,VkDeviceMemory,VkDeviceSize,VkDeviceSize,VkMemoryMapFlags,void**));
RESULT(vkCreateImageView,(VkDevice,const VkImageViewCreateInfo*,const VkAllocationCallbacks*,VkImageView*));
RESULT(vkCreateCommandPool,(VkDevice,const VkCommandPoolCreateInfo*,const VkAllocationCallbacks*,VkCommandPool*));
RESULT(vkAllocateCommandBuffers,(VkDevice,const VkCommandBufferAllocateInfo*,VkCommandBuffer*));
RESULT(vkBeginCommandBuffer,(VkCommandBuffer,const VkCommandBufferBeginInfo*));
RESULT(vkEndCommandBuffer,(VkCommandBuffer));
RESULT(vkCreateFence,(VkDevice,const VkFenceCreateInfo*,const VkAllocationCallbacks*,VkFence*));
RESULT(vkQueueSubmit,(VkQueue,uint32_t,const VkSubmitInfo*,VkFence));
RESULT(vkGetFenceStatus,(VkDevice,VkFence));
VOID(vkDestroyBuffer,(VkDevice,VkBuffer,const VkAllocationCallbacks*));
VOID(vkDestroyImage,(VkDevice,VkImage,const VkAllocationCallbacks*));
VOID(vkDestroyImageView,(VkDevice,VkImageView,const VkAllocationCallbacks*));
VOID(vkFreeMemory,(VkDevice,VkDeviceMemory,const VkAllocationCallbacks*));
VOID(vkUnmapMemory,(VkDevice,VkDeviceMemory));
VOID(vkDestroyCommandPool,(VkDevice,VkCommandPool,const VkAllocationCallbacks*));
VOID(vkDestroyFence,(VkDevice,VkFence,const VkAllocationCallbacks*));
VkResult __wrap_vkCreateBuffer(VkDevice d,const VkBufferCreateInfo *c,const VkAllocationCallbacks *a,VkBuffer *out){if(inject()){*out=POISON(VkBuffer);return VK_ERROR_OUT_OF_HOST_MEMORY;}VkResult r=observed(__real_vkCreateBuffer(d,c,a,out));if(!r)made(0,(uintptr_t)*out);return r;}
VkResult __wrap_vkCreateImage(VkDevice d,const VkImageCreateInfo *c,const VkAllocationCallbacks *a,VkImage *out){if(inject()){*out=POISON(VkImage);return VK_ERROR_OUT_OF_HOST_MEMORY;}VkResult r=observed(__real_vkCreateImage(d,c,a,out));if(!r)made(1,(uintptr_t)*out);return r;}
VkResult __wrap_vkAllocateMemory(VkDevice d,const VkMemoryAllocateInfo *c,const VkAllocationCallbacks *a,VkDeviceMemory *out){if(inject()){*out=POISON(VkDeviceMemory);return VK_ERROR_OUT_OF_HOST_MEMORY;}VkResult r=observed(__real_vkAllocateMemory(d,c,a,out));if(!r)made(2,(uintptr_t)*out);return r;}
VkResult __wrap_vkBindBufferMemory(VkDevice d,VkBuffer b,VkDeviceMemory m,VkDeviceSize o){return inject()?VK_ERROR_OUT_OF_HOST_MEMORY:observed(__real_vkBindBufferMemory(d,b,m,o));}
VkResult __wrap_vkBindImageMemory(VkDevice d,VkImage b,VkDeviceMemory m,VkDeviceSize o){return inject()?VK_ERROR_OUT_OF_HOST_MEMORY:observed(__real_vkBindImageMemory(d,b,m,o));}
VkResult __wrap_vkMapMemory(VkDevice d,VkDeviceMemory m,VkDeviceSize o,VkDeviceSize n,VkMemoryMapFlags f,void **out){if(inject()){*out=POISON(void*);return VK_ERROR_OUT_OF_HOST_MEMORY;}VkResult r=observed(__real_vkMapMemory(d,m,o,n,f,out));if(!r)made(4,(uintptr_t)m);return r;}
VkResult __wrap_vkCreateImageView(VkDevice d,const VkImageViewCreateInfo *c,const VkAllocationCallbacks *a,VkImageView *out){if(inject()){*out=POISON(VkImageView);return VK_ERROR_OUT_OF_HOST_MEMORY;}VkResult r=observed(__real_vkCreateImageView(d,c,a,out));if(!r)made(3,(uintptr_t)*out);return r;}
VkResult __wrap_vkCreateCommandPool(VkDevice d,const VkCommandPoolCreateInfo *c,const VkAllocationCallbacks *a,VkCommandPool *out){if(inject()){*out=POISON(VkCommandPool);return VK_ERROR_OUT_OF_HOST_MEMORY;}VkResult r=observed(__real_vkCreateCommandPool(d,c,a,out));if(!r)made(5,(uintptr_t)*out);return r;}
VkResult __wrap_vkAllocateCommandBuffers(VkDevice d,const VkCommandBufferAllocateInfo *a,VkCommandBuffer *out){if(inject()){*out=POISON(VkCommandBuffer);return VK_ERROR_OUT_OF_HOST_MEMORY;}VkResult r=observed(__real_vkAllocateCommandBuffers(d,a,out));if(!r)made(7,(uintptr_t)*out);return r;}
VkResult __wrap_vkBeginCommandBuffer(VkCommandBuffer c,const VkCommandBufferBeginInfo *i){return inject()?VK_ERROR_OUT_OF_HOST_MEMORY:observed(__real_vkBeginCommandBuffer(c,i));}
VkResult __wrap_vkEndCommandBuffer(VkCommandBuffer c){return inject()?VK_ERROR_OUT_OF_HOST_MEMORY:observed(__real_vkEndCommandBuffer(c));}
VkResult __wrap_vkCreateFence(VkDevice d,const VkFenceCreateInfo *c,const VkAllocationCallbacks *a,VkFence *out){if(inject()){*out=POISON(VkFence);return VK_ERROR_OUT_OF_HOST_MEMORY;}VkResult r=observed(__real_vkCreateFence(d,c,a,out));if(!r)made(6,(uintptr_t)*out);return r;}
VkResult __wrap_vkQueueSubmit(VkQueue q,uint32_t n,const VkSubmitInfo *i,VkFence f){return inject()?VK_ERROR_OUT_OF_HOST_MEMORY:observed(__real_vkQueueSubmit(q,n,i,f));}
VkResult __wrap_vkGetFenceStatus(VkDevice d,VkFence f){
  if(!fault.scoped||fault.q[1]!=4)return __real_vkGetFenceStatus(d,f);
  fault.q[11]++;
  if(fault.q[2]==1&&!fault.q[4]){fault.q[4]=1;fault.q[8]=VK_ERROR_OUT_OF_HOST_MEMORY;return VK_ERROR_OUT_OF_HOST_MEMORY;}
  if(fault.q[2]==3&&now_ns()-fault.q[20]<10100000000ll){fault.q[4]=1;fault.q[8]=VK_NOT_READY;return VK_NOT_READY;}
  VkResult r=observed(__real_vkGetFenceStatus(d,f));
  if(r==VK_SUCCESS){fault.q[25]=1;fault.q[21]=now_ns();
    if(fault.q[2]==2&&!fault.q[4]){fault.q[4]=1;fault.q[8]=VK_ERROR_DEVICE_LOST;return VK_ERROR_DEVICE_LOST;}}
  return r;
}
void __wrap_vkDestroyBuffer(VkDevice d,VkBuffer b,const VkAllocationCallbacks *a){if(drop(0,(uintptr_t)b))__real_vkDestroyBuffer(d,b,a);}
void __wrap_vkDestroyImage(VkDevice d,VkImage b,const VkAllocationCallbacks *a){if(drop(1,(uintptr_t)b))__real_vkDestroyImage(d,b,a);}
void __wrap_vkDestroyImageView(VkDevice d,VkImageView b,const VkAllocationCallbacks *a){if(drop(3,(uintptr_t)b))__real_vkDestroyImageView(d,b,a);}
void __wrap_vkFreeMemory(VkDevice d,VkDeviceMemory m,const VkAllocationCallbacks *a){if(drop(2,(uintptr_t)m))__real_vkFreeMemory(d,m,a);}
void __wrap_vkUnmapMemory(VkDevice d,VkDeviceMemory m){if(drop(4,(uintptr_t)m))__real_vkUnmapMemory(d,m);}
void __wrap_vkDestroyFence(VkDevice d,VkFence f,const VkAllocationCallbacks *a){if(drop(6,(uintptr_t)f))__real_vkDestroyFence(d,f,a);}
void __wrap_vkDestroyCommandPool(VkDevice d,VkCommandPool p,const VkAllocationCallbacks *a){if(drop(5,(uintptr_t)p)){if(fault.scoped){fault.q[19]=0;fault.handles[7]=0;}__real_vkDestroyCommandPool(d,p,a);}}
