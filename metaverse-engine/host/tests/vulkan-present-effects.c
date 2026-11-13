// Firm reason: 6 (actual Wayland/Vulkan SDK effects; Coax authors probes and verdicts).
#include "fixture.h"
#include "realm-host.h"
#include "face.h"
#include "vkw.h"
static RealmHost hp;
static Vkc device;
static Vkw window;
static Vkp present;
static VkcTarget target;
static int opened;
extern void coaxial_host_reset(uint32_t);
extern int64_t coaxial_host_probe(uint32_t,int32_t,int32_t);
bool realm_key(void *s,int key){(void)s;rh_key(&hp,key,0,1);return true;}
bool realm_key_up(void *s,int key){(void)s;rh_key(&hp,key,0,0);return true;}
int64_t hvp_open(void){
  rh_open(&hp,0);vkw_open(&device,&window,"Aethyr host affordability probe",320,180,0,(VkwTo){0});
  vkp_open(&device,&present,window.vk_surface,320,180);
  vkc_target_open(&device,&target,320,180,NULL,0);coaxial_host_reset(hp.state);opened=1;return 1;
}
void hvp_busy(bool busy){
  if(busy){if(vkResetFences(device.dev,1,&present.fence)!=VK_SUCCESS)abort();}
  else {vkDestroyFence(device.dev,present.fence,NULL);VkFenceCreateInfo f={.sType=VK_STRUCTURE_TYPE_FENCE_CREATE_INFO,.flags=VK_FENCE_CREATE_SIGNALED_BIT};if(vkCreateFence(device.dev,&f,NULL,&present.fence)!=VK_SUCCESS)abort();}
}
int64_t hvp_offer(int64_t us){
  vkw_pump(&window);int ready=vkp_reserve(&device,&present);if(ready!=1)return ready;
  uint32_t count=0;return rh_frame_at(&hp,320,180,1,us,0,0,&count);
}
int64_t hvp_count(void){return coaxial_host_probe(hp.state,-1,2);}
int64_t hvp_clock(void){return rh_lifecycle_info(4);}
int64_t hvp_present(void){int r=vkp_frame(&device,&present,&target,NULL,NULL,NULL);if(r)return 0;vkp_wait(&device,&present);return 1;}
int64_t hvp_validation(int64_t kind){return kind==2?device.validating:kind?vkc_validation_warnings:vkc_validation_errors;}
int64_t hvp_reserve_only(void){return vkp_reserve(&device,&present);}
void hvp_close(void){if(!opened)return;opened=0;vkc_target_close(&device,&target);vkp_close(&device,&present);vkw_close(&device,&window);rh_close(&hp);}
