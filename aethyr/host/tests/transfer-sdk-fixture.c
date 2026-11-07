// Firm reason: 2 (independent command/owner oracle, reusing the counted Vulkan fence model).
#include "vks-transfer.h"
#define vkAllocateCommandBuffers vtf_counted_allocate
#include "submission-sdk-fixture.c"
#undef vkAllocateCommandBuffers
VKAPI_ATTR VkResult VKAPI_CALL vkAllocateCommandBuffers(VkDevice d,const VkCommandBufferAllocateInfo *a,VkCommandBuffer *out){VkResult r=vtf_counted_allocate(d,a,out);if(r!=VK_SUCCESS)*out=(VkCommandBuffer)(uintptr_t)0xdead;return r;}
static VksTransfer transfer;
static VkcBuffer source,target;
static VksImage image;
static int transfer_kind,copies,barriers;
static unsigned char source_bytes[128];
VKAPI_ATTR void VKAPI_CALL vkCmdCopyBuffer(VkCommandBuffer c,VkBuffer from,VkBuffer to,uint32_t n,const VkBufferCopy *r){
  (void)c;copies++;if(transfer_kind||from!=(VkBuffer)(uintptr_t)101||to!=(VkBuffer)(uintptr_t)201||n!=1||r[0].size!=64||r[0].srcOffset||r[0].dstOffset)violations++;
}
VKAPI_ATTR void VKAPI_CALL vkCmdCopyBufferToImage(VkCommandBuffer c,VkBuffer from,VkImage to,VkImageLayout layout,uint32_t n,const VkBufferImageCopy *r){
  (void)c;copies++;if(!transfer_kind||from!=(VkBuffer)(uintptr_t)101||to!=(VkImage)(uintptr_t)301||layout!=VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL||n!=3){violations++;return;}
  for(uint32_t i=0;i<3;i++)if(r[i].bufferOffset!=(i==0?0:i==1?64:80)||r[i].bufferRowLength||r[i].bufferImageHeight||r[i].imageSubresource.aspectMask!=VK_IMAGE_ASPECT_COLOR_BIT||r[i].imageSubresource.mipLevel!=i||r[i].imageSubresource.baseArrayLayer||r[i].imageSubresource.layerCount!=1||r[i].imageOffset.x||r[i].imageOffset.y||r[i].imageOffset.z||r[i].imageExtent.width!=(4u>>i)||r[i].imageExtent.height!=(4u>>i)||r[i].imageExtent.depth!=1)violations++;
}
VKAPI_ATTR void VKAPI_CALL vkCmdCopyImageToBuffer(VkCommandBuffer c,VkImage from,VkImageLayout layout,VkBuffer to,uint32_t n,const VkBufferImageCopy *r){(void)c;(void)from;(void)layout;(void)to;(void)n;(void)r;violations++;}
VKAPI_ATTR void VKAPI_CALL vkCmdPipelineBarrier(VkCommandBuffer c,VkPipelineStageFlags from,VkPipelineStageFlags to,VkDependencyFlags flags,uint32_t nm,const VkMemoryBarrier *m,uint32_t nb,const VkBufferMemoryBarrier *b,uint32_t ni,const VkImageMemoryBarrier *i){
  (void)c;(void)b;if(flags||nb)violations++;barriers++;
  if(!transfer_kind){if(barriers!=1||from!=VK_PIPELINE_STAGE_TRANSFER_BIT||to!=VK_PIPELINE_STAGE_ALL_COMMANDS_BIT||nm!=1||ni||m->srcAccessMask!=VK_ACCESS_TRANSFER_WRITE_BIT||m->dstAccessMask!=(VK_ACCESS_VERTEX_ATTRIBUTE_READ_BIT|VK_ACCESS_INDEX_READ_BIT|VK_ACCESS_SHADER_READ_BIT))violations++;}
  else{
    if(nm||ni!=1||i->image!=(VkImage)(uintptr_t)301||i->srcQueueFamilyIndex!=VK_QUEUE_FAMILY_IGNORED||i->dstQueueFamilyIndex!=VK_QUEUE_FAMILY_IGNORED||i->subresourceRange.aspectMask!=VK_IMAGE_ASPECT_COLOR_BIT||i->subresourceRange.baseMipLevel||i->subresourceRange.levelCount!=3||i->subresourceRange.baseArrayLayer||i->subresourceRange.layerCount!=1)violations++;
    if(barriers==1){if(from!=VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT||to!=VK_PIPELINE_STAGE_TRANSFER_BIT||i->oldLayout!=VK_IMAGE_LAYOUT_UNDEFINED||i->newLayout!=VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL||i->srcAccessMask||i->dstAccessMask!=VK_ACCESS_TRANSFER_WRITE_BIT)violations++;}
    else if(barriers==2){if(from!=VK_PIPELINE_STAGE_TRANSFER_BIT||to!=VK_PIPELINE_STAGE_ALL_COMMANDS_BIT||i->oldLayout!=VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL||i->newLayout!=VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL||i->srcAccessMask!=VK_ACCESS_TRANSFER_WRITE_BIT||i->dstAccessMask!=VK_ACCESS_SHADER_READ_BIT)violations++;}else violations++;
  }
}
void vtf_reset(int32_t kind,int32_t at,int32_t code){
  vsf_reset(at,code);memset(&transfer,0,sizeof transfer);transfer_kind=kind;copies=barriers=0;
  source=(VkcBuffer){.buf=(VkBuffer)(uintptr_t)101,.mem=(VkDeviceMemory)(uintptr_t)102,.map=source_bytes,.size=128};
  target=(VkcBuffer){.buf=(VkBuffer)(uintptr_t)201,.mem=(VkDeviceMemory)(uintptr_t)202,.size=128};
  image=(VksImage){.image=(VkImage)(uintptr_t)301,.mem=(VkDeviceMemory)(uintptr_t)302,.view=(VkImageView)(uintptr_t)303,.format=VK_FORMAT_R8G8B8A8_UNORM,.w=4,.h=4,.layers=1,.levels=3};
}
#ifdef VTF_LEGACY
static void legacy_copy(VkCommandBuffer cmd,void *u){(void)u;VkBufferCopy r={.size=64};vkCmdCopyBuffer(cmd,source.buf,target.buf,1,&r);}
#endif
int32_t vtf_begin(void){
#ifdef VTF_LEGACY
  if(transfer_kind)vks_upload_buffer(&fixture,&image,&source,84);else vks_submit(&fixture,legacy_copy,NULL);return 0;
#else
  return transfer_kind?vks_transfer_rgba8_begin(&fixture,&transfer,&source,&image):vks_transfer_buffer_begin(&fixture,&transfer,&source,&target,64);
#endif
}
int32_t vtf_poll(int32_t wait,int32_t wrong){
#ifdef VTF_LEGACY
  return vsf_poll(wait,wrong);
#else
  Vkc v=fixture;if(wrong)v.dev=(VkDevice)(uintptr_t)2;return vks_transfer_poll(&v,&transfer,wait?1:0);
#endif
}
int32_t vtf_retire(int32_t wrong){
#ifdef VTF_LEGACY
  return vsf_retire(wrong);
#else
  Vkc v=fixture;if(wrong)v.dev=(VkDevice)(uintptr_t)2;return vks_transfer_retire(&v,&transfer);
#endif
}
void vtf_signal(int32_t code){vsf_signal(code);}
int64_t vtf_info(int32_t field){switch(field){case 0:return pools;case 1:return fences;case 2:return transfer.submission.pending;case 3:return violations;case 4:return copies;case 5:return queries;case 6:return transfer.kind;case 7:return transfer.bytes;case 8:return stage;
  case 9:if(!transfer.kind){VksTransfer zero={0};return !memcmp(&zero,&transfer,sizeof transfer);}
    return transfer.source.buf==(VkBuffer)(uintptr_t)101&&transfer.source.mem==(VkDeviceMemory)(uintptr_t)102&&transfer.source.map==source_bytes&&transfer.source.size==128&&
      (transfer.kind==1?(transfer.buffer.buf==(VkBuffer)(uintptr_t)201&&transfer.buffer.mem==(VkDeviceMemory)(uintptr_t)202&&transfer.buffer.size==128):
        (transfer.image.image==(VkImage)(uintptr_t)301&&transfer.image.mem==(VkDeviceMemory)(uintptr_t)302&&transfer.image.view==(VkImageView)(uintptr_t)303&&transfer.image.w==4&&transfer.image.h==4&&transfer.image.layers==1&&transfer.image.levels==3));
  default:return -1;}}
int32_t vtf_invalid(int32_t which){
#ifdef VTF_LEGACY
  (void)which;return 0;
#else
  Vkc v=fixture;VkcBuffer s=source,b=target;VksImage t=image;VksTransfer before=transfer;VkResult r;
  if(which==1)v.dev=VK_NULL_HANDLE;if(which==3)s.buf=VK_NULL_HANDLE;if(which==4)s.mem=VK_NULL_HANDLE;if(which==5)s.map=NULL;
  if(transfer_kind){switch(which){case 6:t.format=VK_FORMAT_R8_UNORM;break;case 7:t.levels=0;break;case 8:t.levels=17;break;case 9:t.w=0;break;case 10:s.size=83;break;case 11:t.mem=s.mem;break;case 12:t.w=UINT32_MAX;t.h=UINT32_MAX;t.layers=UINT32_MAX;break;case 13:t.levels=4;break;case 14:t.depth=1;break;case 15:t.layers=0;break;case 16:t.view=VK_NULL_HANDLE;break;case 17:t.image=VK_NULL_HANDLE;break;case 18:t.mem=VK_NULL_HANDLE;break;}
    r=vks_transfer_rgba8_begin(which==0?NULL:&v,which==2?NULL:&transfer,&s,&t);
  }else{VkDeviceSize bytes=64;switch(which){case 6:bytes=0;break;case 7:bytes=63;break;case 8:bytes=132;break;case 9:b.size=63;break;case 10:b.buf=s.buf;break;case 11:b.mem=s.mem;break;case 12:b.mem=VK_NULL_HANDLE;break;case 13:b.buf=VK_NULL_HANDLE;break;}
    r=vks_transfer_buffer_begin(which==0?NULL:&v,which==2?NULL:&transfer,&s,&b,bytes);
  }
  if(memcmp(&before,&transfer,sizeof transfer))violations++;return r;
#endif
}
