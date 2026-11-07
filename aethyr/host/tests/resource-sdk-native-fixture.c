// Firm reason: 2 (independent wrapper oracle over real Vulkan factory prefixes and retirement).
// Explicit error inputs do not claim physical host/device memory exhaustion.
#include "vks-resource.h"
#include <stdint.h>
#include <string.h>
static Vkc device;
static int armed,stage,fail_at,fail_code,objects,memories,extras,violations,cases;
static VkBuffer live_buffer;
static VkImage live_image;
static VkImageView live_view;
static VkDeviceMemory live_memory;
static void *live_map;
static VkDeviceSize allocated_size;
static uint32_t allocated_type;
static int fail(void){return armed&&++stage==fail_at;}
#define DECLARE_RESULT(NAME,ARGS) extern VkResult __real_##NAME ARGS
#define DECLARE_VOID(NAME,ARGS) extern void __real_##NAME ARGS
DECLARE_RESULT(vkCreateBuffer,(VkDevice,const VkBufferCreateInfo*,const VkAllocationCallbacks*,VkBuffer*));
DECLARE_RESULT(vkCreateImage,(VkDevice,const VkImageCreateInfo*,const VkAllocationCallbacks*,VkImage*));
DECLARE_RESULT(vkAllocateMemory,(VkDevice,const VkMemoryAllocateInfo*,const VkAllocationCallbacks*,VkDeviceMemory*));
DECLARE_RESULT(vkBindBufferMemory,(VkDevice,VkBuffer,VkDeviceMemory,VkDeviceSize));
DECLARE_RESULT(vkBindImageMemory,(VkDevice,VkImage,VkDeviceMemory,VkDeviceSize));
DECLARE_RESULT(vkMapMemory,(VkDevice,VkDeviceMemory,VkDeviceSize,VkDeviceSize,VkMemoryMapFlags,void**));
DECLARE_RESULT(vkCreateImageView,(VkDevice,const VkImageViewCreateInfo*,const VkAllocationCallbacks*,VkImageView*));
DECLARE_VOID(vkDestroyBuffer,(VkDevice,VkBuffer,const VkAllocationCallbacks*));
DECLARE_VOID(vkDestroyImage,(VkDevice,VkImage,const VkAllocationCallbacks*));
DECLARE_VOID(vkDestroyImageView,(VkDevice,VkImageView,const VkAllocationCallbacks*));
DECLARE_VOID(vkFreeMemory,(VkDevice,VkDeviceMemory,const VkAllocationCallbacks*));
DECLARE_VOID(vkUnmapMemory,(VkDevice,VkDeviceMemory));
#define POISON(T) ((T)(uintptr_t)0xdead)
VkResult __wrap_vkCreateBuffer(VkDevice d,const VkBufferCreateInfo *c,const VkAllocationCallbacks *a,VkBuffer *out){
  if(fail()){*out=POISON(VkBuffer);return fail_code;}VkResult r=__real_vkCreateBuffer(d,c,a,out);if(armed&&!r){live_buffer=*out;objects++;}return r;
}
VkResult __wrap_vkCreateImage(VkDevice d,const VkImageCreateInfo *c,const VkAllocationCallbacks *a,VkImage *out){
  if(fail()){*out=POISON(VkImage);return fail_code;}VkResult r=__real_vkCreateImage(d,c,a,out);if(armed&&!r){live_image=*out;objects++;}return r;
}
VkResult __wrap_vkAllocateMemory(VkDevice d,const VkMemoryAllocateInfo *c,const VkAllocationCallbacks *a,VkDeviceMemory *out){
  if(fail()){*out=POISON(VkDeviceMemory);return fail_code;}VkResult r=__real_vkAllocateMemory(d,c,a,out);if(armed&&!r){live_memory=*out;memories++;allocated_size=c->allocationSize;allocated_type=c->memoryTypeIndex;}return r;
}
VkResult __wrap_vkBindBufferMemory(VkDevice d,VkBuffer b,VkDeviceMemory m,VkDeviceSize o){return fail()?(VkResult)fail_code:__real_vkBindBufferMemory(d,b,m,o);}
VkResult __wrap_vkBindImageMemory(VkDevice d,VkImage b,VkDeviceMemory m,VkDeviceSize o){return fail()?(VkResult)fail_code:__real_vkBindImageMemory(d,b,m,o);}
VkResult __wrap_vkMapMemory(VkDevice d,VkDeviceMemory m,VkDeviceSize o,VkDeviceSize n,VkMemoryMapFlags f,void **out){
  if(fail()){*out=POISON(void*);return fail_code;}VkResult r=__real_vkMapMemory(d,m,o,n,f,out);if(armed&&!r){live_map=*out;extras++;}return r;
}
VkResult __wrap_vkCreateImageView(VkDevice d,const VkImageViewCreateInfo *c,const VkAllocationCallbacks *a,VkImageView *out){
  if(fail()){*out=POISON(VkImageView);return fail_code;}VkResult r=__real_vkCreateImageView(d,c,a,out);if(armed&&!r){live_view=*out;extras++;}return r;
}
void __wrap_vkDestroyBuffer(VkDevice d,VkBuffer b,const VkAllocationCallbacks *a){if(armed){if(b!=live_buffer||objects!=1||extras){violations++;return;}objects--;live_buffer=VK_NULL_HANDLE;}__real_vkDestroyBuffer(d,b,a);}
void __wrap_vkDestroyImage(VkDevice d,VkImage b,const VkAllocationCallbacks *a){if(armed){if(b!=live_image||objects!=1||extras){violations++;return;}objects--;live_image=VK_NULL_HANDLE;}__real_vkDestroyImage(d,b,a);}
void __wrap_vkDestroyImageView(VkDevice d,VkImageView b,const VkAllocationCallbacks *a){if(armed){if(b!=live_view||extras!=1){violations++;return;}extras--;live_view=VK_NULL_HANDLE;}__real_vkDestroyImageView(d,b,a);}
void __wrap_vkFreeMemory(VkDevice d,VkDeviceMemory m,const VkAllocationCallbacks *a){if(armed){if(m!=live_memory||memories!=1||objects||extras){violations++;return;}memories--;live_memory=VK_NULL_HANDLE;}__real_vkFreeMemory(d,m,a);}
void __wrap_vkUnmapMemory(VkDevice d,VkDeviceMemory m){if(armed){if(m!=live_memory||!live_map||extras!=1){violations++;return;}extras--;live_map=NULL;}__real_vkUnmapMemory(d,m);}
int32_t vrn_open(void){vkc_open(&device);return device.validating&&device.dev!=VK_NULL_HANDLE;}
int32_t vrn_case(int32_t kind,int32_t at,int32_t code){
  if(objects||memories||extras||armed)return 0;armed=1;stage=0;fail_at=at;fail_code=code;
  VkcBuffer b={0},empty_b={0};VksImage t={0},empty_t={0};VksMemoryInfo info,before;memset(&info,0x55,sizeof info);before=info;
  VkResult r=kind?vks_image_try(&device,&t,4,4,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,3,&info):vks_buffer_try(&device,&b,64,&info);
  int ok=r==(at?code:VK_SUCCESS)&&stage==(at?at:4);
  if(at)ok=ok&&!memcmp(&b,&empty_b,sizeof b)&&!memcmp(&t,&empty_t,sizeof t)&&!memcmp(&info,&before,sizeof info);
  else{
    VkMemoryRequirements actual;if(kind)vkGetImageMemoryRequirements(device.dev,t.image,&actual);else vkGetBufferMemoryRequirements(device.dev,b.buf,&actual);
    ok=ok&&info.requirements.size==actual.size&&info.requirements.alignment==actual.alignment&&info.requirements.memoryTypeBits==actual.memoryTypeBits&&info.memory_type==allocated_type&&allocated_size==actual.size;
    if(kind){vkDestroyImageView(device.dev,t.view,NULL);vkDestroyImage(device.dev,t.image,NULL);vkFreeMemory(device.dev,t.mem,NULL);}
    else{for(int i=0;i<64;i++)if(((unsigned char*)b.map)[i])ok=0;vkUnmapMemory(device.dev,b.mem);vkDestroyBuffer(device.dev,b.buf,NULL);vkFreeMemory(device.dev,b.mem,NULL);}
  }
  ok=ok&&!objects&&!memories&&!extras&&!violations&&!vkc_validation_errors&&!vkc_validation_warnings;armed=0;cases++;return ok;
}
int32_t vrn_close(void){if(armed||objects||memories||extras)return 0;vkc_close(&device);return !violations&&!vkc_validation_errors&&!vkc_validation_warnings;}
int64_t vrn_info(int32_t field){switch(field){case 0:return cases;case 1:return vkc_validation_errors;case 2:return vkc_validation_warnings;case 3:return device.props.vendorID;case 4:return device.props.deviceID;default:return -1;}}
