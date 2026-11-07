// Native Vulkan effects for the shared corridor. Coax owns source admission,
// resource selection, borrowing, fixed grants, clocks and recovery ordering.
#define SW_VULKAN 1
#define SW_HEADER "stream-native.h"
#include "stream-walk-face.c"
#include "vks.h"

typedef struct { VkcBuffer buffer; VksImage image; bool live,texture; int64_t generation,bytes; } SwVkObject;
typedef struct { VksPipe pipe; VksPass pass; VksBinding bindings[4]; uint32_t draws[4]; int64_t key[7]; bool live; } SwVkDraw;
static Vkc vk;
static VkcBuffer stages[64],vk_uniforms;
static VksImage vk_color,vk_depth;
static VksSurface vk_surface;
static VkSampler vk_sampler;
static SwVkObject vk_objects[64];
static SwVkDraw vk_draws[16];
static const uint32_t *vk_words;
static size_t vk_word_count;
static bool vk_opened,vk_surface_live;
static int64_t vk_opens,vk_closes,vk_frames,vk_commands,vk_memory_peak;

static int64_t sw_vk_memory(void) {
  if(!vk_opened)return 0;
  VkMemoryRequirements r;int64_t total=0;
  for(int row=0;row<64;row++) {
    if(stages[row].buf){vkGetBufferMemoryRequirements(vk.dev,stages[row].buf,&r);total+=(int64_t)r.size;}
    SwVkObject *o=&vk_objects[row];if(!o->live)continue;
    if(o->texture)vkGetImageMemoryRequirements(vk.dev,o->image.image,&r);else vkGetBufferMemoryRequirements(vk.dev,o->buffer.buf,&r);
    total+=(int64_t)r.size;
  }
  if(vk_surface_live){vkGetBufferMemoryRequirements(vk.dev,vk_uniforms.buf,&r);total+=(int64_t)r.size;vkGetImageMemoryRequirements(vk.dev,vk_color.image,&r);total+=(int64_t)r.size;vkGetImageMemoryRequirements(vk.dev,vk_depth.image,&r);total+=(int64_t)r.size;}
  if(total>vk_memory_peak)vk_memory_peak=total;return total;
}
static void sw_vk_temporary(const VkcBuffer *buffer){VkMemoryRequirements req;vkGetBufferMemoryRequirements(vk.dev,buffer->buf,&req);int64_t bytes=sw_vk_memory()+(int64_t)req.size;if(bytes>vk_memory_peak)vk_memory_peak=bytes;}
static void *sw_vk_stage_allocate(int32_t row,size_t bytes){if(!vk_opened||!fixed_live[5]||stages[row].buf)return NULL;stages[row]=vkc_buffer(&vk,bytes);sw_vk_memory();return stages[row].map;}
static void sw_vk_stage_free(int32_t row){vkc_free(&vk,&stages[row]);memset(&stages[row],0,sizeof stages[row]);}
static void sw_vk_pipes_close(void){for(int i=0;i<16;i++)if(vk_draws[i].live){vks_pipe_close(&vk,&vk_draws[i].pipe);vk_draws[i].live=false;}}
int32_t sw_vk_open(coax_v_u8 shader){
  if(vk_opened||shader.count<20||shader.count%4||(uintptr_t)shader.data%4)return 0;
  vk_words=(const uint32_t*)shader.data;vk_word_count=(size_t)shader.count/4;
  vkc_open(&vk);vk_opened=true;vk_opens++;return 1;
}
int32_t sw_vk_make_surface(void){
  if(!vk_opened||vk_surface_live||!fixed_live[0]||!fixed_live[1]||!fixed_live[2])return 0;
  vk_uniforms=vkc_buffer(&vk,4096);
  vk_color=vks_image(&vk,320,180,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TARGET);
  vk_depth=vks_image(&vk,320,180,1,0,VK_FORMAT_D32_SFLOAT,VKS_TARGET);
  VksPass a={.targets={&vk_color},.ntargets=1,.depth=&vk_depth};vks_surface_open(&vk,&vk_surface,&a,1);
  vk_sampler=vks_sampler(&vk,VKS_NEAREST,0,0);vk_surface_live=true;sw_vk_memory();return 1;
}
int32_t sw_vk_drop(int32_t row,int64_t generation){
  if(row<0||row>=64)return 0;SwVkObject *o=&vk_objects[row];if(!o->live)return 1;if(o->generation!=generation)return 0;
  sw_vk_pipes_close();if(o->texture)vks_image_free(&vk,&o->image);else vkc_free(&vk,&o->buffer);memset(o,0,sizeof *o);return 1;
}
typedef struct {VkcBuffer *from,*to;size_t bytes;} SwVkCopy;
static void sw_vk_record_copy(VkCommandBuffer cmd,void *user){
  SwVkCopy *c=user;VkBufferCopy copy={.size=c->bytes};vkCmdCopyBuffer(cmd,c->from->buf,c->to->buf,1,&copy);
  VkMemoryBarrier b={.sType=VK_STRUCTURE_TYPE_MEMORY_BARRIER,.srcAccessMask=VK_ACCESS_TRANSFER_WRITE_BIT,.dstAccessMask=VK_ACCESS_VERTEX_ATTRIBUTE_READ_BIT|VK_ACCESS_INDEX_READ_BIT};
  vkCmdPipelineBarrier(cmd,VK_PIPELINE_STAGE_TRANSFER_BIT,VK_PIPELINE_STAGE_VERTEX_INPUT_BIT,0,1,&b,0,NULL,0,NULL);
}
int32_t sw_vk_upload(int32_t row,int64_t generation,int64_t epoch,int32_t replacement){
  if(!vk_opened||row<0||row>=64)return 0;SwPhysical *p=&objects[row];SwVkObject *o=&vk_objects[row];
  if(!same(p,row,generation,epoch)||!p->plan||o->live||!sw_host_gpu_live(row))return 0;
  size_t bytes=(size_t)p->plan->cost.gpu;if(!bytes)return 1;
  VkcBuffer temp={0},*stage=&stages[row];
  if(replacement){if(!fixed_live[4])return 0;temp=vkc_buffer(&vk,bytes);stage=&temp;sw_vk_temporary(&temp);}
  if(!stage->buf||stage->size!=bytes)return 0;
  bool image=p->plan->spec.format==COAX_ResourceFormat_image_mips;
  if(image){
    if(!p->decoded)return 0;memcpy(stage->map,p->decoded,bytes);
    o->image=vks_image_levels(&vk,(uint32_t)p->plan->cost.width,(uint32_t)p->plan->cost.height,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE,(uint32_t)p->plan->cost.levels);
    vks_upload_buffer(&vk,&o->image,stage,bytes);
  }else{
    if(!p->source||bytes!=(size_t)p->plan->cost.encoded)return 0;memcpy(stage->map,p->source,bytes);
    o->buffer=vkc_buffer(&vk,bytes);SwVkCopy copy={stage,&o->buffer,bytes};vks_submit(&vk,sw_vk_record_copy,&copy);
  }
  o->live=true;o->texture=image;o->generation=generation;o->bytes=(int64_t)bytes;sw_vk_memory();if(replacement)sw_vk_temporary(&temp);
  if(replacement)vkc_free(&vk,&temp);return 1;
}
static void sw_vk_record_draw(VkCommandBuffer cmd,void *user){
  int n=*(int*)user;const float clear[4]={.03f,.05f,.08f,1};vks_surface_begin(&vk_surface,cmd,clear,1);
  for(int i=0;i<n;i++){SwVkDraw *d=&vk_draws[i];vks_pipe_bind(&d->pipe,cmd,&d->pass);vkCmdDrawIndexed(cmd,d->draws[1],1,0,0,0);}vks_surface_end(cmd);
}
int32_t sw_vk_render(void){
  if(!vk_surface_live)return 0;int n=sw_host_draw_count();if(n<0||n>16)return 0;
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
        .draws=d->draws,.ndraws=1,.index=&vk_objects[key[1]].buffer,.targets={&vk_color},.ntargets=1,.depth=&vk_depth,.depth_test=1,.depth_write=1};
      vks_pipe_open(&vk,&d->pipe,&d->pass);d->live=true;
    }
  }
  vks_submit(&vk,sw_vk_record_draw,&n);vk_frames++;vk_commands+=n;return 1;
}
int32_t sw_vk_capture(coax_v_u8 out){
  if(!vk_surface_live||out.count!=320*180*4||!fixed_live[3])return 0;
  VkcBuffer readback=vkc_buffer(&vk,(VkDeviceSize)out.count);sw_vk_temporary(&readback);vks_read_buffer(&vk,&vk_color,&readback);
  memcpy(out.data,readback.map,(size_t)out.count);vkc_free(&vk,&readback);return 1;
}
int32_t sw_vk_close(void){
  if(!vk_opened)return 0;for(int i=0;i<64;i++)if(stages[i].buf)return 0;
  for(int i=0;i<64;i++)if(!sw_vk_drop(i,vk_objects[i].generation))return 0;
  sw_vk_pipes_close();
  if(vk_surface_live){vks_surface_close(&vk,&vk_surface);vkDestroySampler(vk.dev,vk_sampler,NULL);vkc_free(&vk,&vk_uniforms);vks_image_free(&vk,&vk_color);vks_image_free(&vk,&vk_depth);vk_surface_live=false;}
  vkc_close(&vk);vk_opened=false;vk_closes++;return 1;
}
int64_t sw_vk_info(int32_t field){
  int64_t live=0,bytes=0,stage_n=0,stage_bytes=0;for(int i=0;i<64;i++){if(vk_objects[i].live){live++;bytes+=vk_objects[i].bytes;}if(stages[i].buf){stage_n++;stage_bytes+=(int64_t)stages[i].size;}}
  switch(field){case 0:return live;case 1:return bytes;case 2:return stage_n;case 3:return stage_bytes;case 4:return vk_opened;case 5:return vk_opens;case 6:return vk_closes;case 7:return vk_frames;case 8:return vk_commands;
    case 9:return vkc_validation_errors;case 10:return vkc_validation_warnings;case 11:return vk.validating;case 12:return vk.props.vendorID;case 13:return vk.props.deviceID;case 14:return vk.props.apiVersion;case 15:return vk.props.driverVersion;case 16:return sw_vk_memory();case 17:return vk_memory_peak;
    case 18:return sizeof vk+sizeof stages+sizeof vk_uniforms+sizeof vk_color+sizeof vk_depth+sizeof vk_surface+sizeof vk_sampler+sizeof vk_objects+sizeof vk_draws+sizeof vk_words+sizeof vk_word_count+sizeof vk_opened+sizeof vk_surface_live+sizeof vk_opens+sizeof vk_closes+sizeof vk_frames+sizeof vk_commands+sizeof vk_memory_peak;default:return -1;}
}
int32_t sw_vk_name(coax_v_u8 out){size_t n=strlen(vk.props.deviceName);if(n>(size_t)out.count)return -1;memcpy(out.data,vk.props.deviceName,n);return(int32_t)n;}
bool sw_vk_state(coax_v_u8 out){return swn_state(sv(),out);}
int64_t sw_vk_source(int64_t row,coax_v_u8 out){if(row<0||row>=64)return -1;if(!objects[row].occupied)return 0;if(!objects[row].source||!objects[row].plan)return -1;int64_t n=objects[row].plan->cost.encoded;if(n<0||(uint64_t)n>out.count)return -1;memcpy(out.data,objects[row].source,(size_t)n);return n;}
