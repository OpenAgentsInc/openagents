// Firm reason: 2 (independent real-Vulkan allocation ordering and Coax lease lifetime oracle).
#include "backing-sdk.h"
#define __wrap_vkAllocateMemory vbg_counted_allocate
#include "resource-sdk-native-fixture.c"
#undef __wrap_vkAllocateMemory
#include <limits.h>
#include <stdio.h>

static coax_v_t_BackingBudget budget;
static coax_t_BackingBudget backing_state;
static coax_t_BackingGrant grant;
static int gate_enabled,gate_calls,gate_mode,allocation_calls,gated_cases;
static VksMemoryInfo gate_info;
static int gate_marker;

VkResult __wrap_vkAllocateMemory(VkDevice d,const VkMemoryAllocateInfo *c,const VkAllocationCallbacks *a,VkDeviceMemory *out){
  if(armed&&gate_enabled){
    allocation_calls++;
    if(gate_calls!=1||grant.status!=COAX_BackingStatus_ok||budget.data[0].queue.e[0].count!=1||
       budget.data[0].queue.e[0].bytes!=(int64_t)c->allocationSize||gate_info.requirements.size!=c->allocationSize||gate_info.memory_type!=c->memoryTypeIndex)violations++;
  }
  return vbg_counted_allocate(d,c,a,out);
}
static VkResult gate(void *owner,const VksMemoryInfo *info){
  gate_calls++;gate_info=*info;
  if(owner!=&gate_marker||objects!=1||memories||extras||allocation_calls||stage!=1)violations++;
  VkMemoryRequirements actual;
  if(live_buffer)vkGetBufferMemoryRequirements(device.dev,live_buffer,&actual);else vkGetImageMemoryRequirements(device.dev,live_image,&actual);
  VkPhysicalDeviceMemoryProperties properties;vkGetPhysicalDeviceMemoryProperties(device.gpu,&properties);
  VkMemoryPropertyFlags wanted=live_buffer?VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT|VK_MEMORY_PROPERTY_HOST_COHERENT_BIT:VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
  if(actual.size!=info->requirements.size||actual.alignment!=info->requirements.alignment||actual.memoryTypeBits!=info->requirements.memoryTypeBits||
     info->memory_type>=properties.memoryTypeCount||(properties.memoryTypes[info->memory_type].propertyFlags&wanted)!=wanted)violations++;
  if(gate_mode==3)return (VkResult)fail_code;
  coax_t_BackingNeed need={.bytes=info->requirements.size>INT64_MAX?-1:(int64_t)info->requirements.size,
    .alignment=info->requirements.alignment>INT64_MAX?-1:(int64_t)info->requirements.alignment,.types=info->requirements.memoryTypeBits,.memory_type=info->memory_type};
  grant=backing_hold(budget,live_buffer?0:1,need);
  if(gate_mode<2&&grant.status!=(gate_mode?COAX_BackingStatus_aggregate:COAX_BackingStatus_category))violations++;
  return grant.status==COAX_BackingStatus_ok?VK_SUCCESS:VK_ERROR_OUT_OF_POOL_MEMORY;
}
int32_t vbg_case(coax_v_s64 caps,int64_t total,int64_t allocation,int32_t kind,int32_t mode,int32_t at,int32_t code){
  if(objects||memories||extras||armed||caps.count!=8||backing_state.queue.e[0].count)return 0;
  coax_a8_s64 limits;memcpy(limits.e,caps.data,sizeof limits.e);backing_state=backing_open(limits,total,allocation);
  budget=(coax_v_t_BackingBudget){&backing_state,1};gate_enabled=1;gate_calls=allocation_calls=0;gate_mode=mode;memset(&gate_info,0,sizeof gate_info);memset(&grant,0,sizeof grant);grant.status=COAX_BackingStatus_invalid;
  armed=1;stage=0;fail_at=at;fail_code=code;
  VkcBuffer b={0},empty_b={0};VksImage t={0},empty_t={0};VksMemoryInfo info,before;memset(&info,0x55,sizeof info);before=info;
#ifdef VBG_LEGACY
  VkResult result=kind?vks_image_try(&device,&t,4,4,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,3,&info):vks_buffer_try(&device,&b,64,&info);
#else
  VkResult result=kind?vks_image_try_admitted(&device,&t,4,4,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,3,gate,&gate_marker,&info):vks_buffer_try_admitted(&device,&b,64,gate,&gate_marker,&info);
#endif
  VkResult expected=at?(VkResult)code:mode<2?VK_ERROR_OUT_OF_POOL_MEMORY:mode==3?(VkResult)code:VK_SUCCESS;
  int ok=result==expected&&gate_calls==(at==1?0:1)&&allocation_calls==((at==1||mode!=2)?0:1);
  if(result==VK_SUCCESS){
    if(memcmp(&info,&gate_info,sizeof info))ok=0;
    if(kind){vkDestroyImageView(device.dev,t.view,NULL);vkDestroyImage(device.dev,t.image,NULL);vkFreeMemory(device.dev,t.mem,NULL);}
    else{vkUnmapMemory(device.dev,b.mem);vkDestroyBuffer(device.dev,b.buf,NULL);vkFreeMemory(device.dev,b.mem,NULL);}
  }else ok=ok&&!memcmp(&b,&empty_b,sizeof b)&&!memcmp(&t,&empty_t,sizeof t)&&!memcmp(&info,&before,sizeof info);
  ok=ok&&!objects&&!memories&&!extras&&!violations&&!vkc_validation_errors&&!vkc_validation_warnings;
  if(grant.status==COAX_BackingStatus_ok){
    // Factory failure has already unwound, or the successful object has just
    // been destroyed. The admission is still held until this exact release.
    ok=ok&&budget.data[0].queue.e[0].count==1&&budget.data[0].queue.e[0].bytes==(int64_t)gate_info.requirements.size;
    ok=ok&&backing_release(budget,grant.lease)&&!backing_release(budget,grant.lease);
  }else ok=ok&&budget.data[0].queue.e[0].count==0;
  ok=ok&&!budget.data[0].queue.e[0].count&&!budget.data[0].queue.e[0].bytes;
  printf("backing case kind=%d mode=%d stage=%d result=%d calls=%d size=%llu alignment=%llu types=%u selected=%u grant=%d ok=%d\n",kind,mode,at,result,allocation_calls,(unsigned long long)gate_info.requirements.size,(unsigned long long)gate_info.requirements.alignment,gate_info.requirements.memoryTypeBits,gate_info.memory_type,grant.status,ok);
  armed=0;gate_enabled=0;gated_cases++;return ok;
}
int64_t vbg_info(int32_t field){switch(field){case 0:return gated_cases;case 1:return(int64_t)gate_info.requirements.size;case 2:return backing_state.queue.e[0].count;case 3:return backing_state.queue.e[0].bytes;case 4:return backing_state.refused;case 5:return backing_state.peak;default:return -1;}}
int32_t vbg_null(int32_t kind){
  VkcBuffer b={0},before_b=b;VksImage t={0},before_t=t;VksMemoryInfo info,before_info;memset(&info,0x55,sizeof info);before_info=info;
  armed=1;stage=0;fail_at=0;
#ifdef VBG_LEGACY
  VkResult result=VK_SUCCESS;
#else
  VkResult result=kind?vks_image_try_admitted(&device,&t,4,4,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,3,NULL,NULL,&info):vks_buffer_try_admitted(&device,&b,64,NULL,NULL,&info);
#endif
  int ok=result==VK_ERROR_INITIALIZATION_FAILED&&!stage&&!memcmp(&b,&before_b,sizeof b)&&!memcmp(&t,&before_t,sizeof t)&&!memcmp(&info,&before_info,sizeof info);
  armed=0;gated_cases++;return ok;
}
