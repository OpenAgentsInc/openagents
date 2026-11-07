// Firm reason: 2 (independent counted Vulkan resource model with poisoned failed outputs).
#include "vks-resource.h"
#include <stdint.h>
#include <string.h>
#define HANDLE(T,N) ((T)(uintptr_t)(N))
static Vkc device;
static VkcBuffer buffer;
static VksImage image;
static VksMemoryInfo info;
static int kind,fail_at,fail_code,stage,objects,memories,extras,violations,published;
static unsigned char mapped[256];
static VkResult effect(void){stage++;return stage==fail_at?(VkResult)fail_code:VK_SUCCESS;}
static void requirement(VkMemoryRequirements *r){*r=(VkMemoryRequirements){.size=kind?2048:1024,.alignment=256,.memoryTypeBits=fail_at==5?1u:4u};}
VKAPI_ATTR void VKAPI_CALL vkGetPhysicalDeviceMemoryProperties(VkPhysicalDevice p,VkPhysicalDeviceMemoryProperties *m){
  (void)p;memset(m,0,sizeof *m);m->memoryTypeCount=3;m->memoryTypes[0].propertyFlags=0;
  m->memoryTypes[1].propertyFlags=VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
  m->memoryTypes[2].propertyFlags=VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT|VK_MEMORY_PROPERTY_HOST_COHERENT_BIT|VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateBuffer(VkDevice d,const VkBufferCreateInfo *c,const VkAllocationCallbacks *a,VkBuffer *out){
  (void)d;(void)a;if(c->size!=64||c->usage!=(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT|VK_BUFFER_USAGE_VERTEX_BUFFER_BIT|VK_BUFFER_USAGE_INDEX_BUFFER_BIT|VK_BUFFER_USAGE_TRANSFER_SRC_BIT|VK_BUFFER_USAGE_TRANSFER_DST_BIT))violations++;
  VkResult r=effect();*out=HANDLE(VkBuffer,r?0xdead:11);if(!r)objects++;return r;
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateImage(VkDevice d,const VkImageCreateInfo *c,const VkAllocationCallbacks *a,VkImage *out){
  (void)d;(void)a;if(c->extent.width!=4||c->extent.height!=4||c->arrayLayers!=1||c->mipLevels!=3||c->format!=VK_FORMAT_R8G8B8A8_UNORM||c->tiling!=VK_IMAGE_TILING_OPTIMAL||c->initialLayout!=VK_IMAGE_LAYOUT_UNDEFINED||c->usage!=(VK_IMAGE_USAGE_SAMPLED_BIT|VK_IMAGE_USAGE_TRANSFER_SRC_BIT|VK_IMAGE_USAGE_TRANSFER_DST_BIT))violations++;
  VkResult r=effect();*out=HANDLE(VkImage,r?0xdead:21);if(!r)objects++;return r;
}
VKAPI_ATTR void VKAPI_CALL vkGetBufferMemoryRequirements(VkDevice d,VkBuffer b,VkMemoryRequirements *r){(void)d;if(b!=HANDLE(VkBuffer,11))violations++;requirement(r);}
VKAPI_ATTR void VKAPI_CALL vkGetImageMemoryRequirements(VkDevice d,VkImage b,VkMemoryRequirements *r){(void)d;if(b!=HANDLE(VkImage,21))violations++;requirement(r);}
VKAPI_ATTR VkResult VKAPI_CALL vkAllocateMemory(VkDevice d,const VkMemoryAllocateInfo *c,const VkAllocationCallbacks *a,VkDeviceMemory *out){
  (void)d;(void)a;if(c->allocationSize!=(kind?2048u:1024u)||c->memoryTypeIndex!=2)violations++;
#ifndef VRF_LEGACY
  const VkMemoryDedicatedAllocateInfo *dedicated=c->pNext;
  if(!dedicated||dedicated->sType!=VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO||dedicated->image!=(kind?HANDLE(VkImage,21):VK_NULL_HANDLE)||dedicated->buffer!=(kind?VK_NULL_HANDLE:HANDLE(VkBuffer,11)))violations++;
#endif
  VkResult r=effect();*out=HANDLE(VkDeviceMemory,r?0xbeef:31);if(!r)memories++;return r;
}
VKAPI_ATTR VkResult VKAPI_CALL vkBindBufferMemory(VkDevice d,VkBuffer b,VkDeviceMemory m,VkDeviceSize offset){(void)d;if(b!=HANDLE(VkBuffer,11)||m!=HANDLE(VkDeviceMemory,31)||offset)violations++;return effect();}
VKAPI_ATTR VkResult VKAPI_CALL vkBindImageMemory(VkDevice d,VkImage b,VkDeviceMemory m,VkDeviceSize offset){(void)d;if(b!=HANDLE(VkImage,21)||m!=HANDLE(VkDeviceMemory,31)||offset)violations++;return effect();}
VKAPI_ATTR VkResult VKAPI_CALL vkMapMemory(VkDevice d,VkDeviceMemory m,VkDeviceSize o,VkDeviceSize n,VkMemoryMapFlags flags,void **out){
  (void)d;if(m!=HANDLE(VkDeviceMemory,31)||o||n!=64||flags)violations++;VkResult r=effect();*out=r?(void*)(uintptr_t)0xdead:mapped;if(!r)extras++;return r;
}
VKAPI_ATTR VkResult VKAPI_CALL vkCreateImageView(VkDevice d,const VkImageViewCreateInfo *c,const VkAllocationCallbacks *a,VkImageView *out){
  (void)d;(void)a;if(c->image!=HANDLE(VkImage,21)||c->viewType!=VK_IMAGE_VIEW_TYPE_2D||c->format!=VK_FORMAT_R8G8B8A8_UNORM||c->subresourceRange.aspectMask!=VK_IMAGE_ASPECT_COLOR_BIT||c->subresourceRange.levelCount!=3||c->subresourceRange.layerCount!=1)violations++;
  VkResult r=effect();*out=HANDLE(VkImageView,r?0xdead:41);if(!r)extras++;return r;
}
VKAPI_ATTR void VKAPI_CALL vkUnmapMemory(VkDevice d,VkDeviceMemory m){(void)d;if(m!=HANDLE(VkDeviceMemory,31)||extras!=1)violations++;extras--;}
VKAPI_ATTR void VKAPI_CALL vkDestroyImageView(VkDevice d,VkImageView v,const VkAllocationCallbacks *a){(void)d;(void)a;if(v!=HANDLE(VkImageView,41)||extras!=1)violations++;extras--;}
VKAPI_ATTR void VKAPI_CALL vkDestroyBuffer(VkDevice d,VkBuffer b,const VkAllocationCallbacks *a){(void)d;(void)a;if(b!=HANDLE(VkBuffer,11)||objects!=1||extras)violations++;objects--;}
VKAPI_ATTR void VKAPI_CALL vkDestroyImage(VkDevice d,VkImage b,const VkAllocationCallbacks *a){(void)d;(void)a;if(b!=HANDLE(VkImage,21)||objects!=1||extras)violations++;objects--;}
VKAPI_ATTR void VKAPI_CALL vkFreeMemory(VkDevice d,VkDeviceMemory m,const VkAllocationCallbacks *a){(void)d;(void)a;if(m!=HANDLE(VkDeviceMemory,31)||memories!=1||objects||extras)violations++;memories--;}
void vrf_reset(int32_t k,int32_t at,int32_t code){
  memset(&device,0,sizeof device);memset(&buffer,0,sizeof buffer);memset(&image,0,sizeof image);memset(&info,0x55,sizeof info);memset(mapped,0xaa,sizeof mapped);
  device.dev=HANDLE(VkDevice,1);device.gpu=HANDLE(VkPhysicalDevice,2);kind=k;fail_at=at;fail_code=code;stage=objects=memories=extras=violations=published=0;
}
int32_t vrf_create(void){
#ifdef VRF_LEGACY
  if(kind)image=vks_image_levels(&device,4,4,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,3);else buffer=vkc_buffer(&device,64);return 0;
#else
  VkResult r=kind?vks_image_try(&device,&image,4,4,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,3,&info):vks_buffer_try(&device,&buffer,64,&info);
  published=kind?image.image!=VK_NULL_HANDLE:buffer.buf!=VK_NULL_HANDLE;return r;
#endif
}
void vrf_destroy(void){
  if(kind){vkDestroyImageView(device.dev,image.view,NULL);vkDestroyImage(device.dev,image.image,NULL);vkFreeMemory(device.dev,image.mem,NULL);memset(&image,0,sizeof image);}
  else{vkUnmapMemory(device.dev,buffer.mem);vkDestroyBuffer(device.dev,buffer.buf,NULL);vkFreeMemory(device.dev,buffer.mem,NULL);memset(&buffer,0,sizeof buffer);}
}
int32_t vrf_invalid(int32_t which){
#ifdef VRF_LEGACY
  (void)which;return 0;
#else
  Vkc bad=device;if(which==1)bad.dev=VK_NULL_HANDLE;
  VksMemoryInfo before=info;VkcBuffer b=buffer;VksImage t=image;VkResult r;
  if(kind)r=vks_image_try(which==0?NULL:&bad,which==2?NULL:&image,which==3?0:4,which==5?0:4,which==6?0:1,0,which==7?VK_FORMAT_UNDEFINED:VK_FORMAT_R8G8B8A8_UNORM,which==8?0:which==9?4:VKS_TEXTURE,which==10?0:which==4?4:3,&info);
  else r=vks_buffer_try(which==0?NULL:&bad,which==2?NULL:&buffer,which==3||which==4?0:64,&info);
  if(memcmp(&before,&info,sizeof info)||memcmp(&b,&buffer,sizeof b)||memcmp(&t,&image,sizeof t))violations++;
  return r;
#endif
}
int32_t vrf_nonempty(int32_t which){
#ifdef VRF_LEGACY
  (void)which;return 0;
#else
  VkcBuffer b={0};VksImage t={0};VksMemoryInfo before=info;VkResult r;
  if(kind){
    switch(which){case 0:t.image=HANDLE(VkImage,51);break;case 1:t.mem=HANDLE(VkDeviceMemory,51);break;case 2:t.view=HANDLE(VkImageView,51);break;case 3:t.w=1;break;case 4:t.h=1;break;case 5:t.layers=1;break;case 6:t.levels=1;break;case 7:t.format=VK_FORMAT_R8G8B8A8_UNORM;break;case 8:t.depth=1;break;}
    VksImage original=t;r=vks_image_try(&device,&t,4,4,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,3,&info);if(memcmp(&original,&t,sizeof t))violations++;
  }else{
    switch(which){case 0:b.buf=HANDLE(VkBuffer,51);break;case 1:b.mem=HANDLE(VkDeviceMemory,51);break;case 2:b.map=mapped;break;case 3:b.size=1;break;}
    VkcBuffer original=b;r=vks_buffer_try(&device,&b,64,&info);if(memcmp(&original,&b,sizeof b))violations++;
  }
  if(memcmp(&before,&info,sizeof info))violations++;return r;
#endif
}
int64_t vrf_info(int32_t field){
  switch(field){case 0:return objects;case 1:return memories;case 2:return extras;case 3:return violations;case 4:return published;
    case 5:if(published){int valid=info.requirements.size==(kind?2048u:1024u)&&info.requirements.alignment==256&&info.requirements.memoryTypeBits==4&&info.memory_type==2;
      if(kind)valid=valid&&image.image==HANDLE(VkImage,21)&&image.mem==HANDLE(VkDeviceMemory,31)&&image.view==HANDLE(VkImageView,41)&&image.w==4&&image.h==4&&image.layers==1&&image.levels==3&&image.format==VK_FORMAT_R8G8B8A8_UNORM&&!image.depth;
      else{valid=valid&&buffer.buf==HANDLE(VkBuffer,11)&&buffer.mem==HANDLE(VkDeviceMemory,31)&&buffer.map==mapped&&buffer.size==64;for(int i=0;i<256;i++)if(mapped[i]!=(i<64?0:0xaa))valid=0;}return valid;}
      {VksMemoryInfo before;memset(&before,0x55,sizeof before);VkcBuffer empty_b={0};VksImage empty_t={0};return !memcmp(&before,&info,sizeof before)&&!memcmp(&buffer,&empty_b,sizeof buffer)&&!memcmp(&image,&empty_t,sizeof image);}
    case 6:return stage;default:return -1;}
}
