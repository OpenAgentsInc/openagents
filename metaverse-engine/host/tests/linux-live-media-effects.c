// Firm reason: 6 (Vulkan SDK image upload/readback, device replacement and measured cache observations).
#include "render-live-media.h"
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static Vkc live_device;
static RhgLiveMedia *live_images;
static int validating=1;
bool hlmi_open_bytes(coax_v_u8 root){char *path=malloc((size_t)root.count+1);if(!path)return false;memcpy(path,root.data,(size_t)root.count);path[root.count]=0;vkc_open(&live_device);validating=validating&&live_device.validating;live_images=rhglm_open(&live_device,path,16384);free(path);return live_images!=NULL;}
int64_t hlmi_prepare_bytes(coax_v_u8 packet){RhgMediaRects rects={0};char error[1024];if(!rhglm_prepare(live_images,2,(coax_string){(const char*)packet.data,packet.count},320,180,&rects,error,sizeof error)){fprintf(stderr,"%s\n",error);return -1;}int64_t n=0;for(int64_t i=0;i<rects.receipt.count;i++)n+=rects.rects[i].draw;rhgm_close(&rects);return n;}
int64_t hlmi_info_bytes(coax_v_u8 url,int64_t field){return rhglm_info(live_images,(coax_string){(const char*)url.data,url.count},(int)field);}
int64_t hlmi_copy_bytes(coax_v_u8 url,coax_v_u8 out){const VksImage *image=rhglm_image(live_images,(coax_string){(const char*)url.data,url.count});if(!image)return -1;size_t bytes=(size_t)image->w*image->h*4;if(out.count<0||(uint64_t)out.count<bytes)return -1;vks_read(&live_device,(VksImage*)image,out.data,bytes);return(int64_t)bytes;}
int64_t hlmi_notice(coax_v_u8 out){size_t n=0;const uint8_t *p=rhglm_notice(live_images,&n);if(out.count<0||(uint64_t)out.count<n)return -1;if(n)memcpy(out.data,p,n);return(int64_t)n;}
void hlmi_done(void){rhglm_notice_done(live_images);}
int64_t hlmi_drop(void){vkDeviceWaitIdle(live_device.dev);rhglm_drop_gpu(live_images);return rhg_ledger_info(1);}
void hlmi_recover(void){vkc_close(&live_device);vkc_open(&live_device);validating=validating&&live_device.validating;rhglm_device(live_images,&live_device);}
int64_t hlmi_close(void){if(!live_images)return 0;vkDeviceWaitIdle(live_device.dev);int64_t n=rhglm_close(live_images);live_images=NULL;vkc_close(&live_device);return n;}
int64_t hlmi_device(int64_t field){switch(field){case 0:return validating;case 1:return vkc_validation_errors;case 2:return vkc_validation_warnings;case 3:return rhg_ledger_info(1);}return -1;}
bool hlmi_link_bytes(coax_v_u8 target,coax_v_u8 path){char *t=malloc((size_t)target.count+1),*p=malloc((size_t)path.count+1);if(!t||!p){free(t);free(p);return false;}memcpy(t,target.data,(size_t)target.count);t[target.count]=0;memcpy(p,path.data,(size_t)path.count);p[path.count]=0;int result=symlink(t,p);free(t);free(p);return result==0;}
