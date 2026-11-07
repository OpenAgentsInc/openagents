// Vulkan effects only. Coax admits dimensions/costs and owns pending-use lifetime.
#include "vks-resource.h"
#include <string.h>

static VkResult allocate(Vkc *v,VkMemoryRequirements req,VkMemoryPropertyFlags wanted,
                         VkBuffer buffer,VkImage image,VkDeviceMemory *out,uint32_t *type){
  VkPhysicalDeviceMemoryProperties properties;
  vkGetPhysicalDeviceMemoryProperties(v->gpu,&properties);
  for(uint32_t i=0;i<properties.memoryTypeCount;i++){
    if(!(req.memoryTypeBits&(1u<<i))||(properties.memoryTypes[i].propertyFlags&wanted)!=wanted)continue;
    // Each factory owns a whole allocation, including formats which require a
    // dedicated allocation. The native device already requires Vulkan 1.1.
    VkMemoryDedicatedAllocateInfo dedicated={.sType=VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,.buffer=buffer,.image=image};
    VkMemoryAllocateInfo info={.sType=VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,.pNext=&dedicated,.allocationSize=req.size,.memoryTypeIndex=i};
    VkDeviceMemory created;VkResult result=vkAllocateMemory(v->dev,&info,NULL,&created);
    if(result==VK_SUCCESS){*out=created;*type=i;}return result;
  }
  return VK_ERROR_FEATURE_NOT_PRESENT;
}

VkResult vks_buffer_try(Vkc *v,VkcBuffer *out,VkDeviceSize size,VksMemoryInfo *info){
  if(!v||!v->dev||!out||out->buf||out->mem||out->map||out->size||!size||size>SIZE_MAX)return VK_ERROR_INITIALIZATION_FAILED;
  VkcBuffer b={.size=size};VksMemoryInfo observed={0};VkResult result;
  VkBufferCreateInfo create={.sType=VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,.size=size,
    .usage=VK_BUFFER_USAGE_STORAGE_BUFFER_BIT|VK_BUFFER_USAGE_VERTEX_BUFFER_BIT|VK_BUFFER_USAGE_INDEX_BUFFER_BIT|VK_BUFFER_USAGE_TRANSFER_SRC_BIT|VK_BUFFER_USAGE_TRANSFER_DST_BIT};
  VkBuffer created;
  if((result=vkCreateBuffer(v->dev,&create,NULL,&created))!=VK_SUCCESS)return result;
  b.buf=created;vkGetBufferMemoryRequirements(v->dev,b.buf,&observed.requirements);
  result=allocate(v,observed.requirements,VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT|VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,b.buf,VK_NULL_HANDLE,&b.mem,&observed.memory_type);
  if(result!=VK_SUCCESS)goto failed;
  if((result=vkBindBufferMemory(v->dev,b.buf,b.mem,0))!=VK_SUCCESS)goto failed;
  void *mapped;
  if((result=vkMapMemory(v->dev,b.mem,0,size,0,&mapped))!=VK_SUCCESS)goto failed;
  b.map=mapped;memset(b.map,0,(size_t)size);*out=b;if(info)*info=observed;return VK_SUCCESS;
failed:
  if(b.buf)vkDestroyBuffer(v->dev,b.buf,NULL);
  if(b.mem)vkFreeMemory(v->dev,b.mem,NULL);
  return result;
}

VkResult vks_image_try(Vkc *v,VksImage *out,uint32_t w,uint32_t h,uint32_t layers,
                       int array,VkFormat format,int use,uint32_t levels,VksMemoryInfo *info){
  if(!v||!v->dev||!out||out->image||out->mem||out->view||out->w||out->h||out->layers||out->levels||out->format||out->depth||!w||!h||!layers||!levels||format==VK_FORMAT_UNDEFINED||!use||(use&~(VKS_TEXTURE|VKS_TARGET)))return VK_ERROR_INITIALIZATION_FAILED;
  uint32_t max_levels=1;for(uint32_t side=w>h?w:h;side>1;side>>=1)max_levels++;
  if(levels>max_levels)return VK_ERROR_INITIALIZATION_FAILED;
  int depth=format==VK_FORMAT_D32_SFLOAT||format==VK_FORMAT_D16_UNORM||format==VK_FORMAT_D24_UNORM_S8_UINT;
  VksImage t={.format=format,.w=w,.h=h,.layers=layers,.levels=levels,.depth=depth};VksMemoryInfo observed={0};VkResult result;
  VkImageUsageFlags usage=VK_IMAGE_USAGE_SAMPLED_BIT|VK_IMAGE_USAGE_TRANSFER_SRC_BIT|VK_IMAGE_USAGE_TRANSFER_DST_BIT;
  if(use&VKS_TARGET)usage|=depth?VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT:VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
  VkImageCreateInfo create={.sType=VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,.imageType=VK_IMAGE_TYPE_2D,.format=format,
    .extent={w,h,1},.mipLevels=levels,.arrayLayers=layers,.samples=VK_SAMPLE_COUNT_1_BIT,.tiling=VK_IMAGE_TILING_OPTIMAL,.usage=usage,.initialLayout=VK_IMAGE_LAYOUT_UNDEFINED};
  VkImage created;
  if((result=vkCreateImage(v->dev,&create,NULL,&created))!=VK_SUCCESS)return result;
  t.image=created;vkGetImageMemoryRequirements(v->dev,t.image,&observed.requirements);
  result=allocate(v,observed.requirements,VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,VK_NULL_HANDLE,t.image,&t.mem,&observed.memory_type);
  if(result!=VK_SUCCESS)goto failed;
  if((result=vkBindImageMemory(v->dev,t.image,t.mem,0))!=VK_SUCCESS)goto failed;
  VkImageViewCreateInfo view={.sType=VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,.image=t.image,
    .viewType=layers>1||array?VK_IMAGE_VIEW_TYPE_2D_ARRAY:VK_IMAGE_VIEW_TYPE_2D,.format=format,
    .subresourceRange={depth?VK_IMAGE_ASPECT_DEPTH_BIT:VK_IMAGE_ASPECT_COLOR_BIT,0,levels,0,layers}};
  VkImageView created_view;
  if((result=vkCreateImageView(v->dev,&view,NULL,&created_view))!=VK_SUCCESS)goto failed;
  t.view=created_view;*out=t;if(info)*info=observed;return VK_SUCCESS;
failed:
  if(t.image)vkDestroyImage(v->dev,t.image,NULL);
  if(t.mem)vkFreeMemory(v->dev,t.mem,NULL);
  return result;
}
