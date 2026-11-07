// Vulkan transfer effects. Coax owns admission, deadlines and retirement order.
#include "vks-transfer.h"
#include <string.h>
static int empty(const VksTransfer *t){return t&&!t->kind&&!t->bytes&&!t->submission.device&&!t->source.buf&&!t->buffer.buf&&!t->image.image;}
static int mapped(const VkcBuffer *s){return s&&s->buf&&s->mem&&s->map&&s->size;}
static void image_barrier(VkCommandBuffer cmd,const VksImage *t,VkImageLayout from,VkImageLayout to,
                           VkAccessFlags src,VkAccessFlags dst,VkPipelineStageFlags begin,VkPipelineStageFlags end){
  VkImageMemoryBarrier b={.sType=VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,.srcAccessMask=src,.dstAccessMask=dst,
    .oldLayout=from,.newLayout=to,.srcQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED,.dstQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED,
    .image=t->image,.subresourceRange={VK_IMAGE_ASPECT_COLOR_BIT,0,t->levels,0,t->layers}};
  vkCmdPipelineBarrier(cmd,begin,end,0,0,NULL,0,NULL,1,&b);
}
static void record(VkCommandBuffer cmd,void *user){
  VksTransfer *t=user;
  if(t->kind==1){
    VkBufferCopy copy={.size=t->bytes};vkCmdCopyBuffer(cmd,t->source.buf,t->buffer.buf,1,&copy);
    VkMemoryBarrier b={.sType=VK_STRUCTURE_TYPE_MEMORY_BARRIER,.srcAccessMask=VK_ACCESS_TRANSFER_WRITE_BIT,
      .dstAccessMask=VK_ACCESS_VERTEX_ATTRIBUTE_READ_BIT|VK_ACCESS_INDEX_READ_BIT|VK_ACCESS_SHADER_READ_BIT};
    vkCmdPipelineBarrier(cmd,VK_PIPELINE_STAGE_TRANSFER_BIT,VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,0,1,&b,0,NULL,0,NULL);
  }else{
    VkBufferImageCopy regions[16];VkDeviceSize at=0;VksImage *i=&t->image;
    for(uint32_t l=0;l<i->levels;l++){
      uint32_t w=i->w>>l?i->w>>l:1,h=i->h>>l?i->h>>l:1;
      regions[l]=(VkBufferImageCopy){.bufferOffset=at,.imageSubresource={VK_IMAGE_ASPECT_COLOR_BIT,l,0,i->layers},.imageExtent={w,h,1}};
      at+=(VkDeviceSize)w*h*i->layers*4;
    }
    image_barrier(cmd,i,VK_IMAGE_LAYOUT_UNDEFINED,VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,0,VK_ACCESS_TRANSFER_WRITE_BIT,VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT,VK_PIPELINE_STAGE_TRANSFER_BIT);
    vkCmdCopyBufferToImage(cmd,t->source.buf,i->image,VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,i->levels,regions);
    image_barrier(cmd,i,VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,VK_ACCESS_TRANSFER_WRITE_BIT,VK_ACCESS_SHADER_READ_BIT,VK_PIPELINE_STAGE_TRANSFER_BIT,VK_PIPELINE_STAGE_ALL_COMMANDS_BIT);
  }
}
static VkResult begin(Vkc *v,VksTransfer *out,VksTransfer made){
  VkResult result=vks_submission_begin(v,&made.submission,record,&made);
  if(result==VK_SUCCESS)*out=made;return result;
}
VkResult vks_transfer_buffer_begin(Vkc *v,VksTransfer *out,const VkcBuffer *source,const VkcBuffer *to,VkDeviceSize bytes){
  if(!v||!v->dev||!empty(out)||!mapped(source)||!to||!to->buf||!to->mem||source->buf==to->buf||source->mem==to->mem||!bytes||bytes%4||bytes>source->size||bytes>to->size)return VK_ERROR_INITIALIZATION_FAILED;
  return begin(v,out,(VksTransfer){.source=*source,.buffer=*to,.bytes=bytes,.kind=1});
}
VkResult vks_transfer_rgba8_begin(Vkc *v,VksTransfer *out,const VkcBuffer *source,const VksImage *to){
  if(!v||!v->dev||!empty(out)||!mapped(source)||!to||!to->image||!to->mem||!to->view||to->format!=VK_FORMAT_R8G8B8A8_UNORM||to->depth||!to->w||!to->h||!to->layers||!to->levels||to->levels>16||source->mem==to->mem)return VK_ERROR_INITIALIZATION_FAILED;
  uint32_t max_levels=1;for(uint32_t side=to->w>to->h?to->w:to->h;side>1;side>>=1)max_levels++;
  if(to->levels>max_levels)return VK_ERROR_INITIALIZATION_FAILED;
  VkDeviceSize bytes=0;
  for(uint32_t l=0;l<to->levels;l++){
    uint64_t w=to->w>>l?to->w>>l:1,h=to->h>>l?to->h>>l:1;
    uint64_t pixels=w*h;if(pixels>UINT64_MAX/to->layers/4)return VK_ERROR_INITIALIZATION_FAILED;
    uint64_t part=pixels*to->layers*4;if(part>source->size-bytes)return VK_ERROR_INITIALIZATION_FAILED;bytes+=part;
  }
  return begin(v,out,(VksTransfer){.source=*source,.image=*to,.bytes=bytes,.kind=2});
}
VkResult vks_transfer_poll(Vkc *v,VksTransfer *t,uint64_t timeout_ns){if(!t||!t->kind)return VK_ERROR_INITIALIZATION_FAILED;return vks_submission_poll(v,&t->submission,timeout_ns);}
VkResult vks_transfer_retire(Vkc *v,VksTransfer *t){
  if(!t)return VK_ERROR_INITIALIZATION_FAILED;
  VkResult result=vks_submission_retire(v,&t->submission);if(result==VK_SUCCESS)memset(t,0,sizeof *t);return result;
}
