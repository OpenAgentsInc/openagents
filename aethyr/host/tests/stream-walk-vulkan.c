// Native Vulkan effects for the shared corridor. Coax owns source admission,
// resource selection, borrowing, fixed grants, clocks and recovery ordering.
#define SW_VULKAN 1
#define SW_PRESENTATION 1
#define SW_LOSS_POLICY 1
#define SW_HEADER "stream-native.h"
#include "stream-walk-face.c"
#include "vks.h"
#include "vks-submit.h"
#include "vks-resource.h"
#include "vks-transfer.h"
#include "stream-sdk-fault.h"
#include <time.h>

typedef struct { VkcBuffer buffer; VksImage image; bool live,texture; int64_t generation,bytes; } SwVkObject;
typedef struct { VksPipe pipe; VksPass pass; VksBinding bindings[4]; uint32_t draws[4]; int64_t key[7]; bool live; } SwVkDraw;
static Vkc vk;
static VkcBuffer stages[64],vk_uniforms,vk_hud_vertices,vk_hud_uniforms;
static VksPipe vk_hud_pipe;
static VksPass vk_hud_pass;
static VksBinding vk_hud_binding;
static VksImage vk_color,vk_hdr,vk_depth;
static VksPipe vk_present_pipe;
static VksPass vk_present_pass;
static VksBinding vk_present_bindings[3];
static VksSurface vk_surface;
static VkSampler vk_sampler;
static SwVkObject vk_objects[64];
static SwVkDraw vk_draws[16];
static const uint32_t *vk_words;
static size_t vk_word_count;
static const uint32_t *vk_hud_words;
static size_t vk_hud_word_count;
static const uint32_t *vk_present_words;
static size_t vk_present_word_count;
static bool vk_opened,vk_surface_live;
static int64_t vk_opens,vk_closes,vk_frames,vk_commands,vk_memory_peak;
static int64_t vk_close_violations;
static VksSubmission vk_submission;
static VksTransfer vk_transfer;
static VkcBuffer vk_transfer_stage;
static int32_t vk_transfer_row;
static int64_t vk_transfer_generation,vk_transfer_epoch;
static VkResult vk_transfer_result;
static int64_t vk_transfer_hold_next,vk_transfer_hold_until,vk_transfer_hold_polls;
static int32_t vk_transfer_hold_kind;
static bool sw_vk_transfer_idle(void){return !vk_transfer.kind;}
static bool sw_vk_resource_idle(int32_t row){return sw_vk_transfer_idle()||row!=vk_transfer_row;}
int32_t sw_vk_transfer_hold(int64_t ms,int32_t kind){if(!sw_vk_transfer_idle()||ms!=50||(kind!=1&&kind!=2))return 0;vk_transfer_hold_next=ms;vk_transfer_hold_kind=kind;return 1;}
static VkResult vk_submit_result,vk_real_result;
static int64_t vk_injected_losses,vk_injected_failures,vk_refused_reopens,vk_injections;
static int64_t vk_hold_next_ms,vk_hold_until_ns,vk_delay_next_ms;
static int64_t sw_vk_now_ns(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return(int64_t)t.tv_sec*1000000000+t.tv_nsec;}
// Test-carrier SDK result inputs, always recorded separately from actual
// Vulkan results. No physical GPU reset or invalid device use is induced.
int32_t sw_vk_fault(int32_t kind,int64_t count){
  if(count<0||count>20000)return 0;
  if(kind==1)vk_injected_losses=count;
  else if(kind==2)vk_hold_next_ms=count;
  else if(kind==3){vk_injected_losses=1;vk_refused_reopens=count;}
  else if(kind==4)vk_injected_failures=count;
  else if(kind==5)vk_delay_next_ms=count;
  else return 0;return 1;
}
int32_t sw_vk_poll(void){
  if(!vk_submission.device)return vk_submit_result==VK_SUCCESS;
  if(vk_hold_until_ns&&sw_vk_now_ns()<vk_hold_until_ns){vk_submit_result=VK_TIMEOUT;return 0;}
  VkResult result=vks_submission_poll(&vk,&vk_submission,0);vk_submit_result=vk_real_result=result;
  if(result!=VK_SUCCESS&&result!=VK_ERROR_DEVICE_LOST)return 0;
  if(vks_submission_retire(&vk,&vk_submission)!=VK_SUCCESS)return 0;
  vk_hold_until_ns=0;
  if(result==VK_SUCCESS){vk_frames++;if(vk_injected_losses){vk_injected_losses--;vk_injections++;vk_submit_result=VK_ERROR_DEVICE_LOST;}
    else if(vk_injected_failures){vk_injected_failures--;vk_injections++;vk_submit_result=VK_ERROR_OUT_OF_HOST_MEMORY;}}
  if(vk_delay_next_ms){int64_t delay=vk_delay_next_ms;vk_delay_next_ms=0;vk_injections++;
    struct timespec held={.tv_sec=delay/1000,.tv_nsec=(delay%1000)*1000000};while(nanosleep(&held,&held)&&errno==EINTR){}
  }
  return vk_submit_result==VK_SUCCESS;
}
int64_t sw_vk_submission_info(int32_t field){switch(field){case 0:return vk_submission.pending;case 1:return vk_submit_result;case 2:return vk_submission.device!=VK_NULL_HANDLE;case 3:return vk_real_result;case 4:return vk_injections;case 5:return vk_refused_reopens;case 6:return vk_hold_until_ns;default:return -1;}}


static int64_t sw_vk_memory(void) {
  if(!vk_opened)return 0;
  VkMemoryRequirements r;int64_t total=0;
  for(int row=0;row<64;row++) {
    if(stages[row].buf){vkGetBufferMemoryRequirements(vk.dev,stages[row].buf,&r);total+=(int64_t)r.size;}
    SwVkObject *o=&vk_objects[row];if(!o->live)continue;
    if(o->texture)vkGetImageMemoryRequirements(vk.dev,o->image.image,&r);else vkGetBufferMemoryRequirements(vk.dev,o->buffer.buf,&r);
    total+=(int64_t)r.size;
  }
  if(vk_transfer_stage.buf){vkGetBufferMemoryRequirements(vk.dev,vk_transfer_stage.buf,&r);total+=(int64_t)r.size;}
  if(vk_surface_live){vkGetBufferMemoryRequirements(vk.dev,vk_uniforms.buf,&r);total+=(int64_t)r.size;vkGetBufferMemoryRequirements(vk.dev,vk_hud_vertices.buf,&r);total+=(int64_t)r.size;vkGetBufferMemoryRequirements(vk.dev,vk_hud_uniforms.buf,&r);total+=(int64_t)r.size;vkGetImageMemoryRequirements(vk.dev,vk_color.image,&r);total+=(int64_t)r.size;vkGetImageMemoryRequirements(vk.dev,vk_hdr.image,&r);total+=(int64_t)r.size;vkGetImageMemoryRequirements(vk.dev,vk_depth.image,&r);total+=(int64_t)r.size;}
  if(total>vk_memory_peak)vk_memory_peak=total;return total;
}
static void sw_vk_temporary(const VkcBuffer *buffer){VkMemoryRequirements req;vkGetBufferMemoryRequirements(vk.dev,buffer->buf,&req);int64_t bytes=sw_vk_memory()+(int64_t)req.size;if(bytes>vk_memory_peak)vk_memory_peak=bytes;}
static void *sw_vk_stage_allocate(int32_t row,size_t bytes){
  if(!vk_opened||!fixed_live[5]||stages[row].buf)return NULL;
  SwPhysical *p=&objects[row];int kind=p->plan&&p->plan->spec.format==COAX_ResourceFormat_image_mips?2:1;
  sw_sdk_scope(1,kind,row,p->key.generation,p->key.epoch);VkResult result=vks_buffer_try(&vk,&stages[row],bytes,NULL);sw_sdk_scope(0,0,0,0,0);
  if(result!=VK_SUCCESS)return NULL;sw_vk_memory();return stages[row].map;
}
static void sw_vk_stage_free(int32_t row){vkc_free(&vk,&stages[row]);memset(&stages[row],0,sizeof stages[row]);}
static void sw_vk_pipes_close(void){for(int i=0;i<16;i++)if(vk_draws[i].live){vks_pipe_close(&vk,&vk_draws[i].pipe);vk_draws[i].live=false;}}
int32_t sw_vk_open(coax_v_u8 shader,coax_v_u8 hud_shader,coax_v_u8 present_shader){
  if(vk_opened||shader.count<20||shader.count%4||(uintptr_t)shader.data%4||hud_shader.count<20||hud_shader.count%4||(uintptr_t)hud_shader.data%4||present_shader.count<20||present_shader.count%4||(uintptr_t)present_shader.data%4)return 0;
  vk_words=(const uint32_t*)shader.data;vk_word_count=(size_t)shader.count/4;
  vk_hud_words=(const uint32_t*)hud_shader.data;vk_hud_word_count=(size_t)hud_shader.count/4;
  vk_present_words=(const uint32_t*)present_shader.data;vk_present_word_count=(size_t)present_shader.count/4;
  vkc_open(&vk);vk_opened=true;vk_opens++;return 1;
}
int32_t sw_vk_reopen(void){
  if(vk_refused_reopens){vk_refused_reopens--;return 0;}
  return sw_vk_open((coax_v_u8){(uint8_t*)vk_words,vk_word_count*4},(coax_v_u8){(uint8_t*)vk_hud_words,vk_hud_word_count*4},(coax_v_u8){(uint8_t*)vk_present_words,vk_present_word_count*4});
}
int32_t sw_vk_make_surface(void){
  if(!vk_opened||vk_surface_live||!fixed_live[0]||!fixed_live[1]||!fixed_live[2]||!fixed_live[6]||!fixed_live[7]||!fixed_live[8])return 0;
  vk_uniforms=vkc_buffer(&vk,4096);
  vk_hud_vertices=vkc_buffer(&vk,98304);vk_hud_uniforms=vkc_buffer(&vk,256);((float*)vk_hud_uniforms.map)[0]=320;((float*)vk_hud_uniforms.map)[1]=180;
  vk_color=vks_image(&vk,320,180,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TARGET);
  vk_hdr=vks_image(&vk,320,180,1,0,VK_FORMAT_R16G16B16A16_SFLOAT,VKS_TARGET);
  vk_depth=vks_image(&vk,320,180,1,0,VK_FORMAT_D32_SFLOAT,VKS_TARGET);
  VksPass a={.targets={&vk_hdr},.ntargets=1,.depth=&vk_depth};vks_surface_open(&vk,&vk_surface,&a,1);
  vk_hud_binding=(VksBinding){.kind=VKS_STORAGE,.buffer=&vk_hud_uniforms,.range=256};
  vk_hud_pass=(VksPass){.words=vk_hud_words,.count=vk_hud_word_count,.bindings=&vk_hud_binding,.nb=1,.vertices=&vk_hud_vertices,.stride=48,.nattrs=3,
    .attrs={{.location=0,.format=VK_FORMAT_R32G32B32A32_SFLOAT,.offset=0},{.location=1,.format=VK_FORMAT_R32G32B32A32_SFLOAT,.offset=16},{.location=2,.format=VK_FORMAT_R32G32B32A32_SFLOAT,.offset=32}},
    .targets={&vk_hdr},.ntargets=1,.depth=&vk_depth,.blend=VKS_OVER,.alpha_blend=VKS_OVER};
  vks_pipe_open(&vk,&vk_hud_pipe,&vk_hud_pass);
  vk_sampler=vks_sampler(&vk,VKS_NEAREST,0,0);
  vk_present_bindings[0]=(VksBinding){.kind=VKS_STORAGE,.buffer=&vk_hud_uniforms,.range=256};vk_present_bindings[1]=(VksBinding){.kind=VKS_IMAGE,.image=&vk_hdr};vk_present_bindings[2]=(VksBinding){.kind=VKS_SAMPLER,.sampler=vk_sampler};
  vk_present_pass=(VksPass){.words=vk_present_words,.count=vk_present_word_count,.bindings=vk_present_bindings,.nb=3,.vertex_count=3,.targets={&vk_color},.ntargets=1};
  vks_pipe_open(&vk,&vk_present_pipe,&vk_present_pass);vk_surface_live=true;sw_vk_memory();return 1;
}
int32_t sw_vk_drop(int32_t row,int64_t generation){
  if(row<0||row>=64||!sw_vk_resource_idle(row))return 0;SwVkObject *o=&vk_objects[row];if(!o->live)return 1;if(o->generation!=generation)return 0;
  sw_vk_pipes_close();if(o->texture)vks_image_free(&vk,&o->image);else vkc_free(&vk,&o->buffer);memset(o,0,sizeof *o);return 1;
}
int64_t sw_vk_transfer_info(int32_t field){switch(field){case 0:return vk_transfer.submission.pending;case 1:return vk_transfer.kind!=0;case 2:return vk_transfer_result;case 3:return vk_transfer_row;case 4:return vk_transfer_generation;case 5:return vk_transfer_epoch;case 6:return vk_transfer_stage.buf!=VK_NULL_HANDLE;case 7:return vk_transfer_hold_until;case 8:return vk_transfer_hold_polls;case 9:return vk_transfer.kind;default:return -1;}}
// 2 retains ownership; only 0/1 are terminal outcomes. An unknown fence error
// is not permission to finish or free a source/destination.
int32_t sw_vk_upload_poll(void){
  if(!vk_transfer.kind)return vk_transfer_result==VK_SUCCESS;
  if(vk_transfer_hold_until&&sw_vk_now_ns()<vk_transfer_hold_until){vk_transfer_result=VK_NOT_READY;vk_transfer_hold_polls++;return 2;}
  sw_sdk_scope(4,vk_transfer.kind,vk_transfer_row,vk_transfer_generation,vk_transfer_epoch);
  VkResult result=vks_transfer_poll(&vk,&vk_transfer,0);sw_sdk_scope(0,0,0,0,0);vk_transfer_result=result;
  if(result!=VK_SUCCESS&&result!=VK_ERROR_DEVICE_LOST)return 2;
  if(vks_transfer_retire(&vk,&vk_transfer)!=VK_SUCCESS)return 2;
  vk_transfer_hold_until=0;
  if(vk_transfer_stage.buf){vkc_free(&vk,&vk_transfer_stage);memset(&vk_transfer_stage,0,sizeof vk_transfer_stage);}
  if(result!=VK_SUCCESS&&!sw_vk_drop(vk_transfer_row,vk_transfer_generation))return 0;
  return result==VK_SUCCESS;
}
int32_t sw_vk_upload(int32_t row,int64_t generation,int64_t epoch,int32_t replacement){
  if(!vk_opened||row<0||row>=64||!sw_vk_transfer_idle())return 0;SwPhysical *p=&objects[row];SwVkObject *o=&vk_objects[row];
  vk_transfer_result=VK_ERROR_INITIALIZATION_FAILED;
  if(!same(p,row,generation,epoch)||!p->plan||o->live||!sw_host_gpu_live(row))return 0;
  size_t bytes=(size_t)p->plan->cost.gpu;if(!bytes)return 1;
  VkcBuffer *stage=&stages[row];
  bool image=p->plan->spec.format==COAX_ResourceFormat_image_mips;
  if(image?!p->decoded:(!p->source||bytes!=(size_t)p->plan->cost.encoded))return 0;
  if(replacement){if(!fixed_live[4])return 0;vk_transfer_result=vks_buffer_try(&vk,&vk_transfer_stage,bytes,NULL);if(vk_transfer_result!=VK_SUCCESS)return 0;stage=&vk_transfer_stage;sw_vk_memory();}
  if(!stage->buf||stage->size!=bytes)goto refused;
  VkResult created;
  sw_sdk_scope(2,image?2:1,row,generation,epoch);
  if(image){
    memcpy(stage->map,p->decoded,bytes);
    created=vks_image_try(&vk,&o->image,(uint32_t)p->plan->cost.width,(uint32_t)p->plan->cost.height,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,(uint32_t)p->plan->cost.levels,NULL);
  }else{
    memcpy(stage->map,p->source,bytes);
    created=vks_buffer_try(&vk,&o->buffer,bytes,NULL);
  }
  sw_sdk_scope(0,0,0,0,0);if(created!=VK_SUCCESS){vk_transfer_result=created;goto refused;}
  o->live=true;o->texture=image;o->generation=generation;o->bytes=(int64_t)bytes;sw_vk_memory();
  vk_transfer_row=row;vk_transfer_generation=generation;vk_transfer_epoch=epoch;
  sw_sdk_scope(3,image?2:1,row,generation,epoch);
  vk_transfer_result=image?vks_transfer_rgba8_begin(&vk,&vk_transfer,stage,&o->image):vks_transfer_buffer_begin(&vk,&vk_transfer,stage,&o->buffer,bytes);sw_sdk_scope(0,0,0,0,0);
  if(vk_transfer_result!=VK_SUCCESS){sw_vk_drop(row,generation);goto refused;}
  if(vk_transfer_hold_next&&vk_transfer_hold_kind==vk_transfer.kind){vk_transfer_hold_until=sw_vk_now_ns()+vk_transfer_hold_next*1000000;vk_transfer_hold_next=0;}
  return sw_vk_upload_poll();
refused:
  if(vk_transfer_stage.buf){vkc_free(&vk,&vk_transfer_stage);memset(&vk_transfer_stage,0,sizeof vk_transfer_stage);}return 0;
}
static void sw_vk_record_draw(VkCommandBuffer cmd,void *user){
  int n=*(int*)user;const float clear[4]={.03f,.05f,.08f,1};vks_surface_begin(&vk_surface,cmd,clear,1);
  for(int i=0;i<n;i++){SwVkDraw *d=&vk_draws[i];vks_pipe_bind(&d->pipe,cmd,&d->pass);vkCmdDrawIndexed(cmd,d->draws[1],1,0,0,0);}
  vks_pipe_bind(&vk_hud_pipe,cmd,&vk_hud_pass);vkCmdDraw(cmd,(uint32_t)hud_count,1,0,0);vks_surface_end(cmd);vks_pipe_record(&vk_present_pipe,cmd,&vk_present_pass);
}
int32_t sw_vk_render(void){
  vk_submit_result=VK_ERROR_INITIALIZATION_FAILED;
  if(!vk_surface_live||vk_submission.device||!sw_vk_transfer_idle())return 0;int n=sw_host_draw_count();if(n<0||n>16)return 0;
  int32_t hn=sw_host_hud(320,180);if(hn<=0||hn>2048)return 0;memcpy(vk_hud_vertices.map,hud_vertices,(size_t)hn*48);
  for(int i=0;i<n;i++){
    SwVkDraw *d=&vk_draws[i];int64_t key[7];for(int f=0;f<7;f++)key[f]=sw_host_draw(i,f);
    for(int f=0;f<4;f++)if(key[f]<0||key[f]>=64||!vk_objects[key[f]].live||!borrowed[key[f]])return 0;
    uintptr_t u=sw_host_uniforms((int32_t)key[4],320.f/180.f);if(!u)return 0;memcpy((uint8_t*)vk_uniforms.map+i*256,(void*)u,256);
    if(d->live&&memcmp(key,d->key,sizeof key)){vks_pipe_close(&vk,&d->pipe);d->live=false;}
    if(!d->live){
      memcpy(d->key,key,sizeof key);d->draws[0]=0;d->draws[1]=(uint32_t)key[5];d->draws[2]=0;d->draws[3]=1;
      d->bindings[0]=(VksBinding){.kind=VKS_STORAGE,.buffer=&vk_uniforms,.offset=(VkDeviceSize)i*256,.range=256};
      d->bindings[1]=(VksBinding){.kind=VKS_IMAGE,.image=&vk_objects[key[2]].image};d->bindings[2]=(VksBinding){.kind=VKS_IMAGE,.image=&vk_objects[key[3]].image};d->bindings[3]=(VksBinding){.kind=VKS_SAMPLER,.sampler=vk_sampler};
      d->pass=(VksPass){.words=vk_words,.count=vk_word_count,.bindings=d->bindings,.nb=4,.vertices=&vk_objects[key[0]].buffer,.stride=48,.nattrs=3,
        .attrs={{.location=0,.format=VK_FORMAT_R32G32B32_SFLOAT,.offset=0},{.location=1,.format=VK_FORMAT_R32G32B32_SFLOAT,.offset=12},{.location=2,.format=VK_FORMAT_R32G32_SFLOAT,.offset=24}},
        .draws=d->draws,.ndraws=1,.index=&vk_objects[key[1]].buffer,.targets={&vk_hdr},.ntargets=1,.depth=&vk_depth,.depth_test=1,.depth_write=1};
      vks_pipe_open(&vk,&d->pipe,&d->pass);d->live=true;
    }
  }
  VkResult submitted=vks_submission_begin(&vk,&vk_submission,sw_vk_record_draw,&n);vk_submit_result=vk_real_result=submitted;
  if(submitted!=VK_SUCCESS)return 0;
  vk_commands+=n;
  if(vk_hold_next_ms){vk_hold_until_ns=sw_vk_now_ns()+vk_hold_next_ms*1000000;vk_hold_next_ms=0;}
  return sw_vk_poll();
}
int32_t sw_vk_capture(coax_v_u8 out){
  if(!vk_surface_live||out.count!=320*180*4||!fixed_live[3])return 0;
  VkcBuffer readback=vkc_buffer(&vk,(VkDeviceSize)out.count);sw_vk_temporary(&readback);vks_read_buffer(&vk,&vk_color,&readback);
  memcpy(out.data,readback.map,(size_t)out.count);vkc_free(&vk,&readback);return 1;
}
int32_t sw_vk_close(void){
  if(!vk_opened||vk_submission.device||!sw_vk_transfer_idle())return 0;for(int i=0;i<64;i++)if(stages[i].buf)return 0;
  if(vk_surface_live){const int needed[6]={0,1,2,6,7,8};for(int i=0;i<6;i++)if(!fixed_live[needed[i]])vk_close_violations++;}
  for(int i=0;i<64;i++)if(!sw_vk_drop(i,vk_objects[i].generation))return 0;
  sw_vk_pipes_close();
  if(vk_surface_live){vks_pipe_close(&vk,&vk_present_pipe);vks_pipe_close(&vk,&vk_hud_pipe);vks_surface_close(&vk,&vk_surface);vkDestroySampler(vk.dev,vk_sampler,NULL);vkc_free(&vk,&vk_uniforms);vkc_free(&vk,&vk_hud_vertices);vkc_free(&vk,&vk_hud_uniforms);vks_image_free(&vk,&vk_color);vks_image_free(&vk,&vk_hdr);vks_image_free(&vk,&vk_depth);vk_surface_live=false;}
  vkc_close(&vk);vk_opened=false;vk_closes++;return 1;
}
int64_t sw_vk_info(int32_t field){
  int64_t live=0,bytes=0,stage_n=0,stage_bytes=0;for(int i=0;i<64;i++){if(vk_objects[i].live){live++;bytes+=vk_objects[i].bytes;}if(stages[i].buf){stage_n++;stage_bytes+=(int64_t)stages[i].size;}}
  switch(field){case 0:return live;case 1:return bytes;case 2:return stage_n;case 3:return stage_bytes;case 4:return vk_opened;case 5:return vk_opens;case 6:return vk_closes;case 7:return vk_frames;case 8:return vk_commands;
    case 9:return vkc_validation_errors;case 10:return vkc_validation_warnings;case 11:return vk.validating;case 12:return vk.props.vendorID;case 13:return vk.props.deviceID;case 14:return vk.props.apiVersion;case 15:return vk.props.driverVersion;case 16:return sw_vk_memory();case 17:return vk_memory_peak;
    case 18:return sizeof vk+sizeof stages+sizeof vk_uniforms+sizeof vk_color+sizeof vk_depth+sizeof vk_surface+sizeof vk_sampler+sizeof vk_objects+sizeof vk_draws+sizeof vk_words+sizeof vk_word_count+sizeof vk_opened+sizeof vk_surface_live+sizeof vk_opens+sizeof vk_closes+sizeof vk_frames+sizeof vk_commands+sizeof vk_memory_peak+sizeof vk_hud_vertices+sizeof vk_hud_uniforms+sizeof vk_hud_pipe+sizeof vk_hud_pass+sizeof vk_hud_binding+sizeof vk_hud_words+sizeof vk_hud_word_count+sizeof vk_hdr+sizeof vk_present_pipe+sizeof vk_present_pass+sizeof vk_present_bindings+sizeof vk_present_words+sizeof vk_present_word_count+sizeof vk_close_violations+sizeof vk_submission+sizeof vk_submit_result+sizeof vk_real_result+sizeof vk_injected_losses+sizeof vk_injected_failures+sizeof vk_refused_reopens+sizeof vk_injections+sizeof vk_hold_next_ms+sizeof vk_hold_until_ns+sizeof vk_delay_next_ms+sizeof vk_transfer+sizeof vk_transfer_stage+sizeof vk_transfer_row+sizeof vk_transfer_generation+sizeof vk_transfer_epoch+sizeof vk_transfer_result+sizeof vk_transfer_hold_next+sizeof vk_transfer_hold_until+sizeof vk_transfer_hold_polls+sizeof vk_transfer_hold_kind+sw_sdk_fault_info(24);
    case 19:return vk_close_violations;default:return -1;}
}
int32_t sw_vk_name(coax_v_u8 out){size_t n=strlen(vk.props.deviceName);if(n>(size_t)out.count)return -1;memcpy(out.data,vk.props.deviceName,n);return(int32_t)n;}
bool sw_vk_state(coax_v_u8 out){return swn_state(sv(),out);}
int64_t sw_vk_source(int64_t row,coax_v_u8 out){if(row<0||row>=64)return -1;if(!objects[row].occupied)return 0;if(!objects[row].source||!objects[row].plan)return -1;int64_t n=objects[row].plan->cost.encoded;if(n<0||(uint64_t)n>out.count)return -1;memcpy(out.data,objects[row].source,(size_t)n);return n;}
int64_t sw_vk_hud_copy(coax_v_u8 out){size_t bytes=(size_t)hud_count*48;if(bytes>(size_t)out.count)return -1;memcpy(out.data,hud_vertices,bytes);return(int64_t)bytes;}
