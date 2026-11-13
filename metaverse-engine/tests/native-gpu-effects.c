/* Platform effects only: typed Vulkan C SDK calls. Coax supplies every fixture,
   descriptor, feature decision and verdict. No shader arithmetic lives here. */
#include "vks.h"
#include "RenderCaps.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

enum { FX_MAX=64, FX_BYTES=1048576 };
static Vkc fx_v;
static VkcBuffer fx_buffers[FX_MAX];
static VksImage fx_images[FX_MAX];
static VkSampler fx_samplers[FX_MAX];
static unsigned char *fx_blobs[FX_MAX];
static size_t fx_sizes[FX_MAX];
static int fx_nb,fx_ni,fx_ns,fx_nblob,fx_opened,fx_pipeline,fx_pipelines;
static VksPass fx_pass;
static VksBinding fx_bindings[8];
static uint32_t fx_draws[256];
static VksPipe fx_pipe;
static coax_t_RenderBufferView fx_view;
static int fx_index(int64_t i,int n){return i>=0 && i<n;}
int64_t ngfx_open(void){if(fx_opened)return -1;vkc_open(&fx_v);fx_opened=1;return 0;}
int64_t ngfx_info(int64_t field){
  if(field==6)return vkc_validation_errors;if(field==7)return vkc_validation_warnings;
  if(!fx_opened)return -1;
  switch(field){case 0:return fx_v.demote_helper_supported;case 1:return fx_v.demote_helper_enabled;
    case 2:return fx_v.shader_int64_enabled;case 3:return fx_v.props.vendorID;case 4:return fx_v.props.deviceID;
    case 5:return fx_v.props.driverVersion;case 6:return vkc_validation_errors;case 7:return vkc_validation_warnings;
    case 8:return fx_pipelines;case 9:return fx_v.validating;default:return -1;}
}
int64_t ngfx_name(int64_t index){if(index<0 || index>=VK_MAX_PHYSICAL_DEVICE_NAME_SIZE)return -1;return (unsigned char)fx_v.props.deviceName[index];}
int64_t ngfx_features(int64_t version,int64_t derivatives,int64_t control,int64_t int64_required,int64_t helper,int64_t int64_enabled){
  return render_test_vulkan_features(version,derivatives,control,int64_required!=0,helper!=0,int64_enabled!=0);
}
int64_t ngfx_view(int64_t vec4s,int64_t physical,int64_t offset){
  if(physical<0 || offset<0)return -1;fx_view=render_test_uniform_view(vec4s,(uint64_t)physical,(uint64_t)offset);return fx_view.status;
}
int64_t ngfx_view_field(int64_t field){switch(field){case 0:return fx_view.status;case 1:return (int64_t)fx_view.offset;case 2:return (int64_t)fx_view.bytes;case 3:return (int64_t)fx_view.elements;default:return -1;}}
int64_t ngfx_blob(int64_t bytes){if(bytes<0 || bytes>FX_BYTES || fx_nblob==FX_MAX)return -1;int i=fx_nblob++;fx_blobs[i]=malloc((size_t)bytes);if(!fx_blobs[i])return -1;fx_sizes[i]=(size_t)bytes;return i;}
int64_t ngfx_blob_put(int64_t blob,int64_t offset,int64_t byte){if(!fx_index(blob,fx_nblob)||offset<0||(uint64_t)offset>=fx_sizes[blob]||byte<0||byte>255)return -1;fx_blobs[blob][offset]=(unsigned char)byte;return 0;}
int64_t ngfx_blob_word(int64_t blob,int64_t index){uint32_t word;if(!fx_index(blob,fx_nblob)||index<0||(uint64_t)index>=fx_sizes[blob]/4)return -1;memcpy(&word,fx_blobs[blob]+index*4,4);return word;}
int64_t ngfx_buffer(int64_t bytes){if(bytes<1||bytes>FX_BYTES||fx_nb==FX_MAX)return -1;int i=fx_nb++;fx_buffers[i]=vkc_buffer(&fx_v,(VkDeviceSize)bytes);return i;}
int64_t ngfx_buffer_word(int64_t buffer,int64_t index,int64_t word){if(!fx_index(buffer,fx_nb)||index<0||(uint64_t)index>=fx_buffers[buffer].size/4||word<0||(uint64_t)word>UINT32_MAX)return -1;uint32_t w=(uint32_t)word;memcpy((unsigned char*)fx_buffers[buffer].map+index*4,&w,4);return 0;}
static VkFormat fx_format(int64_t format){switch(format){case 1:return VK_FORMAT_R16G16B16A16_SFLOAT;case 2:return VK_FORMAT_R32G32B32A32_SFLOAT;case 3:return VK_FORMAT_D32_SFLOAT;case 4:return VK_FORMAT_R32_SFLOAT;case 5:return VK_FORMAT_R32_UINT;default:return VK_FORMAT_UNDEFINED;}}
int64_t ngfx_image(int64_t w,int64_t h,int64_t layers,int64_t array,int64_t format,int64_t use){if(w<1||w>16384||h<1||h>16384||layers<1||layers>256||fx_ni==FX_MAX||fx_format(format)==VK_FORMAT_UNDEFINED)return -1;int i=fx_ni++;fx_images[i]=vks_image(&fx_v,(uint32_t)w,(uint32_t)h,(uint32_t)layers,array!=0,fx_format(format),(int)use);return i;}
int64_t ngfx_upload(int64_t image,int64_t blob){if(!fx_index(image,fx_ni)||!fx_index(blob,fx_nblob))return -1;vks_upload(&fx_v,&fx_images[image],fx_blobs[blob],fx_sizes[blob]);return 0;}
int64_t ngfx_sampler(int64_t filter,int64_t repeat,int64_t compare,int64_t op,int64_t mip){if(fx_ns==FX_MAX)return -1;int i=fx_ns++;fx_samplers[i]=vks_sampler_with(&fx_v,(int)filter,(int)repeat,(int)compare,(VkCompareOp)op,(int)mip);return i;}
int64_t ngfx_begin(int64_t shader){if(fx_pipeline||!fx_index(shader,fx_nblob)||fx_sizes[shader]%4)return -1;memset(&fx_pass,0,sizeof fx_pass);memset(fx_bindings,0,sizeof fx_bindings);memset(fx_draws,0,sizeof fx_draws);fx_pass.words=(uint32_t*)fx_blobs[shader];fx_pass.count=fx_sizes[shader]/4;fx_pass.bindings=fx_bindings;fx_pass.draws=fx_draws;return 0;}
int64_t ngfx_binding(int64_t slot,int64_t kind,int64_t resource,int64_t offset,int64_t range){
  if(slot<0||slot>=8||offset<0||range<0)return -1;VksBinding *b=&fx_bindings[slot];memset(b,0,sizeof *b);b->kind=(int)kind;b->offset=(VkDeviceSize)offset;b->range=(VkDeviceSize)range;
  if(kind==VKS_STORAGE&&fx_index(resource,fx_nb))b->buffer=&fx_buffers[resource];else if(kind==VKS_IMAGE&&fx_index(resource,fx_ni))b->image=&fx_images[resource];else if(kind==VKS_SAMPLER&&fx_index(resource,fx_ns))b->sampler=fx_samplers[resource];else return -1;return 0;
}
int64_t ngfx_target(int64_t slot,int64_t image){if(slot<0||slot>=8||!fx_index(image,fx_ni))return -1;fx_pass.targets[slot]=&fx_images[image];return 0;}
int64_t ngfx_attribute(int64_t slot,int64_t location,int64_t binding,int64_t format,int64_t offset){if(slot<0||slot>=8||location<0||binding<0||offset<0||fx_format(format)==VK_FORMAT_UNDEFINED)return -1;fx_pass.attrs[slot]=(VkVertexInputAttributeDescription){(uint32_t)location,(uint32_t)binding,fx_format(format),(uint32_t)offset};return 0;}
int64_t ngfx_clear(int64_t lane,int64_t word){if(lane<0||lane>=4||word<0||(uint64_t)word>UINT32_MAX)return -1;uint32_t w=(uint32_t)word;memcpy(&fx_pass.clear[lane],&w,4);return 0;}
int64_t ngfx_draw(int64_t index,int64_t word){if(index<0||index>=256||word<0||(uint64_t)word>UINT32_MAX)return -1;fx_draws[index]=(uint32_t)word;return 0;}
/* Scalar descriptor field transfer only; the Coax caller selects all values. */
int64_t ngfx_field(int64_t field,int64_t value){
  if(value<0 || (uint64_t)value>UINT32_MAX)return -1;
  switch(field){case 0:if(value>8)return -1;fx_pass.nb=(uint32_t)value;break;
    case 1:fx_pass.vertex_count=(uint32_t)value;break;case 2:if(value>8)return -1;fx_pass.ntargets=(uint32_t)value;break;
    case 3:if(!fx_index(value,fx_ni))return -1;fx_pass.depth=&fx_images[value];break;
    case 4:fx_pass.depth_test=(int)value;break;case 5:fx_pass.depth_write=(int)value;break;
    case 6:if(!fx_index(value,fx_nb))return -1;fx_pass.vertices=&fx_buffers[value];break;
    case 7:fx_pass.stride=(uint32_t)value;break;case 8:if(value>8)return -1;fx_pass.nattrs=(uint32_t)value;break;
    case 9:if(value>64)return -1;fx_pass.ndraws=(uint32_t)value;break;default:return -1;}return 0;
}
static void fx_record(VkCommandBuffer cmd,void *unused){(void)unused;vks_pipe_record(&fx_pipe,cmd,&fx_pass);}
int64_t ngfx_pipe_open(void){if(fx_pipeline)return -1;vks_pipe_open(&fx_v,&fx_pipe,&fx_pass);fx_pipeline=1;fx_pipelines++;return 0;}
int64_t ngfx_submit(void){if(!fx_pipeline)return -1;vks_submit(&fx_v,fx_record,NULL);return 0;}
int64_t ngfx_pipe_close(void){if(!fx_pipeline)return -1;vks_pipe_close(&fx_v,&fx_pipe);fx_pipeline=0;return 0;}
int64_t ngfx_read(int64_t image,int64_t bytes){if(!fx_index(image,fx_ni))return -1;int64_t blob=ngfx_blob(bytes);if(blob<0)return -1;vks_read(&fx_v,&fx_images[image],fx_blobs[blob],(size_t)bytes);return blob;}
int64_t ngfx_close(void){
  if(!fx_opened)return -1;if(fx_pipeline)ngfx_pipe_close();
  for(int i=0;i<fx_ns;i++)vkDestroySampler(fx_v.dev,fx_samplers[i],NULL);
  for(int i=0;i<fx_ni;i++)vks_image_free(&fx_v,&fx_images[i]);
  for(int i=0;i<fx_nb;i++)vkc_free(&fx_v,&fx_buffers[i]);
  for(int i=0;i<fx_nblob;i++)free(fx_blobs[i]);
  vkc_close(&fx_v);fx_opened=0;return 0;
}
