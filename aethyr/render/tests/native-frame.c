// Actual captured producer inputs through the shared Coax facade and Vulkan
// executor. No game clock/camera is reconstructed in this measurement driver.
#include "frame-input.h"
#include "render-host.h"
#include "render-media.h"
#include <assert.h>
#include <stdbool.h>

typedef struct OwnedBuffer {VkcBuffer gpu;struct OwnedBuffer *next;} OwnedBuffer;
typedef struct {
  Vkc device;RfiFrame input;RhgRenderer *renderer;
  coax_t_RenderHostNamedBuffer *named;VkcBuffer *buffers,hud,index;
  OwnedBuffer *generated;
  VkSampler samplers[16];coax_string sampler_names[16];size_t sampler_count;
  RhgTextures *textures;RhgMediaRects media;
} NativeFrame;
typedef struct {char *directory;unsigned char *last;} PackageFiles;

static RhgBytes native_file(PackageFiles *files,coax_string name){
  free(files->last);files->last=NULL;size_t prefix=strlen(files->directory),n=(size_t)name.count;
  char *path=malloc(prefix+n+2);assert(path);memcpy(path,files->directory,prefix);path[prefix]='/';memcpy(path+prefix+1,name.data,n);path[prefix+n+1]=0;
  FILE *f=fopen(path,"rb");free(path);if(!f)return(RhgBytes){0};assert(!fseek(f,0,SEEK_END));long bytes=ftell(f);assert(bytes>=0&&!fseek(f,0,SEEK_SET));
  files->last=calloc((size_t)bytes+1,1);assert(files->last&&fread(files->last,1,(size_t)bytes,f)==(size_t)bytes);assert(!fclose(f));return(RhgBytes){files->last,(size_t)bytes};
}
static RhgBytes native_read(void *user,coax_string name){return native_file(user,name);}
static coax_v_u32 native_words(const RfiBytes *bytes){
  uint64_t address=(uint64_t)(uintptr_t)bytes->data;
  coax_t_RenderHostWordView view=aethyr_render_words(address,address,bytes->size,bytes->size);
  assert(view.status==COAX_RenderFrameStatus_ok);return(coax_v_u32){(uint32_t*)(uintptr_t)view.address,view.words};
}
static coax_t_RenderUpload native_upload(const RfiBytes *bytes,const VkcBuffer *gpu){
  coax_t_RenderUpload upload;coax_default_t_RenderUpload(&upload);
  upload.source_address=(uint64_t)(uintptr_t)bytes->data;upload.source_base=upload.source_address;
  upload.source_bytes=bytes->size;upload.bytes=bytes->size;upload.gpu_bytes=gpu->size;return upload;
}
static int same_name(coax_string a,coax_string b){return a.count==b.count&&!memcmp(a.data,b.data,(size_t)a.count);}
static VkSampler native_sampler(NativeFrame *frame,coax_string name){
  for(size_t i=0;i<frame->sampler_count;i++)if(same_name(frame->sampler_names[i],name))return frame->samplers[i];
  coax_t_RenderSamplerDescriptor desc=aethyr_render_sampler(name);assert(desc.status==COAX_RenderResourceStatus_ok&&frame->sampler_count<16);
  VkSamplerCreateInfo info={.sType=VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,.magFilter=desc.mag_linear?VK_FILTER_LINEAR:VK_FILTER_NEAREST,
    .minFilter=desc.min_linear?VK_FILTER_LINEAR:VK_FILTER_NEAREST,.mipmapMode=desc.mip_linear?VK_SAMPLER_MIPMAP_MODE_LINEAR:VK_SAMPLER_MIPMAP_MODE_NEAREST,
    .addressModeU=desc.address_u==COAX_RenderSamplerAddress_repeat?VK_SAMPLER_ADDRESS_MODE_REPEAT:VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
    .addressModeV=desc.address_v==COAX_RenderSamplerAddress_repeat?VK_SAMPLER_ADDRESS_MODE_REPEAT:VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
    .addressModeW=VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,.anisotropyEnable=desc.physical_anisotropy>1,.maxAnisotropy=(float)desc.physical_anisotropy,
    .compareEnable=desc.compare!=COAX_RenderSamplerCompare_none,.compareOp=VK_COMPARE_OP_LESS,.maxLod=VK_LOD_CLAMP_NONE};
  VkSampler sampler;assert(vkCreateSampler(frame->device.dev,&info,NULL,&sampler)==VK_SUCCESS);
  frame->samplers[frame->sampler_count]=sampler;frame->sampler_names[frame->sampler_count++]=name;return sampler;
}
static int native_binding(void *user,coax_t_RenderHostUse use,coax_t_RenderOp op,const coax_t_RenderMediaRect *media,VksBinding *out){
  NativeFrame *frame=user;memset(out,0,sizeof*out);
  if(use.use.native_kind==VKS_STORAGE){
    coax_t_RenderHostNamedBuffer generated={0};OwnedBuffer *owned=NULL;
    const void *generated_bytes=NULL;size_t generated_size=0;
    if(use.source.kind==COAX_RenderSourceKind_bloom_step){generated_bytes=op.bloom_step.e;generated_size=sizeof op.bloom_step.e;}
    if(use.source.kind==COAX_RenderSourceKind_media_uniform&&media){generated_bytes=media->uniforms.e;generated_size=sizeof media->uniforms.e;}
    if(generated_bytes){
      owned=calloc(1,sizeof*owned);assert(owned);owned->gpu=vkc_buffer(&frame->device,generated_size);owned->next=frame->generated;frame->generated=owned;
      generated.words=(coax_v_u32){(uint32_t*)generated_bytes,(int64_t)(generated_size/4)};
      generated.observed=(coax_t_RenderBufferObserved){.source_capacity=generated_size,.logical_payload=generated_size,.gpu_bytes=owned->gpu.size,.alignment=4,.zero_initialized=true};
    }
    coax_t_RenderHostBufferReceipt receipt=aethyr_native_resolve_buffer(frame->input.counts[0],use.binding_set,use.use.resource,
      (coax_v_t_RenderHostNamedBuffer){frame->named,(int64_t)frame->input.buffer_count},generated);
    if(receipt.status!=COAX_RenderResourceStatus_ok)return 0;
    if(owned){memcpy(owned->gpu.map,generated_bytes,generated_size);out->buffer=&owned->gpu;}
    else{assert(receipt.named_index>=0&&(uint64_t)receipt.named_index<frame->input.buffer_count);out->buffer=&frame->buffers[receipt.named_index];}
    out->offset=receipt.view.offset;out->range=receipt.view.bytes;return 1;
  }
  if(use.use.native_kind==VKS_IMAGE){
    coax_t_RenderHostTextureReceipt texture=aethyr_render_resolve_texture(frame->input.counts[0],use.binding_set,use.use.resource,
      rhgt_names(frame->textures),media?media->line:-1);
    if(texture.status!=COAX_RenderResourceStatus_ok)return 0;
    if(use.source.kind==COAX_RenderSourceKind_render_target)out->image=rhg_target_image(frame->renderer,use.source.target);
    if(use.source.kind==COAX_RenderSourceKind_pass_source)out->image=rhg_target_image(frame->renderer,op.source);
    if(texture.named_index>=0)out->image=rhgt_image(frame->textures,texture,NULL,0);
    return out->image!=NULL;
  }
  if(use.use.native_kind==VKS_SAMPLER){out->sampler=native_sampler(frame,use.source.sampler_name);return 1;}
  return 0;
}
int main(int argc,char **argv){
  if(argc!=5&&argc!=6){fprintf(stderr,"usage: native-frame FRAME.CONFIG RENDER.JSON OUT.RAW RECORD.JSON [rgba8|rgba16f]\n");return 2;}
  const char *format_name=argc==6?argv[5]:"rgba8";coax_t_RenderTextureFormat screen_format;size_t screen_pixel_bytes;
  if(!strcmp(format_name,"rgba8")){screen_format=COAX_RenderTextureFormat_rgba8;screen_pixel_bytes=4;}
  else if(!strcmp(format_name,"rgba16f")){screen_format=COAX_RenderTextureFormat_rgba16f;screen_pixel_bytes=8;}
  else{fprintf(stderr,"unsupported readback format\n");return 2;}
  NativeFrame frame={0};char error[256];if(!rfi_load(argv[1],&frame.input,error,sizeof error)){fprintf(stderr,"%s\n",error);return 2;}
  coax_t_RenderHostBool coarse=aethyr_render_bool(frame.input.counts[12]);
  if(!coarse.ok){fprintf(stderr,"raw coarse flag refused by Coax\n");return 2;}
  for(size_t i=0;i<frame.input.buffer_count;i++)if(!aethyr_render_bool(frame.input.buffers[i].zero_initialized).ok){fprintf(stderr,"raw buffer initialization flag refused by Coax\n");return 2;}
  frame.textures=rhgt_open(&frame.device,&frame.input,error,sizeof error);if(!frame.textures){fprintf(stderr,"%s\n",error);return 2;}
  const char *slash=strrchr(argv[2],'/');size_t directory_size=slash?(size_t)(slash-argv[2]):1;
  PackageFiles files={.directory=calloc(directory_size+1,1)};assert(files.directory);if(slash)memcpy(files.directory,argv[2],directory_size);else files.directory[0]='.';
  const char *base=slash?slash+1:argv[2];RhgBytes package=native_file(&files,(coax_string){base,(int64_t)strlen(base)});
  // rhg_open retains a private JSON copy before the read callback reuses memory.
  vkc_open(&frame.device);frame.renderer=rhg_open(&frame.device,frame.input.counts[0],(coax_string){(char*)package.data,(int64_t)package.bytes},native_read,&files,error,sizeof error);
  if(!frame.renderer){fprintf(stderr,"%s\n",error);return 3;}
  size_t n=frame.input.buffer_count;frame.named=calloc(n?n:1,sizeof*frame.named);frame.buffers=calloc(n?n:1,sizeof*frame.buffers);assert(frame.named&&frame.buffers);
  coax_t_RenderFrameLimits limits;coax_default_t_RenderFrameLimits(&limits);
  coax_t_RenderUpload *uploads=calloc(n+2,sizeof*uploads);assert(uploads);
  for(size_t i=0;i<n;i++){
    RfiBuffer *raw=&frame.input.buffers[i];if(raw->bytes.size)frame.buffers[i]=vkc_buffer(&frame.device,raw->bytes.size);
    uploads[i]=native_upload(&raw->bytes,&frame.buffers[i]);
    frame.named[i].name=(coax_string){raw->name,(int64_t)strlen(raw->name)};
    frame.named[i].observed=(coax_t_RenderBufferObserved){.source_capacity=raw->source_capacity,.logical_payload=raw->logical_payload,
      .initialized_highwater=frame.buffers[i].size,.gpu_bytes=frame.buffers[i].size,.alignment=4,.zero_initialized=true};
  }
  if(frame.input.hud.size)frame.hud=vkc_buffer(&frame.device,frame.input.hud.size);
  if(frame.input.indices.size)frame.index=vkc_buffer(&frame.device,frame.input.indices.size);
  uploads[n]=native_upload(&frame.input.hud,&frame.hud);uploads[n+1]=native_upload(&frame.input.indices,&frame.index);
  int64_t *c=frame.input.counts;coax_t_FrameCounts counts={.width=c[1],.height=c[2],.hud_vertices=c[3],.vertex_stride=c[4],.draw_count=c[5],.water_draws=c[6],.grass_instances=c[7],.particle_instances=c[8],.surface_vertices=c[9],.caster_vertices=c[10],.caster_instances=c[11]};
  coax_v_t_RenderUpload span_uploads={uploads,(int64_t)n+2};
  coax_t_RenderRuntimePreflight preflight=aethyr_render_preflight(c[0],counts,limits,span_uploads);
  if(preflight.status!=COAX_RenderRuntimeStatus_ok){fprintf(stderr,"preflight refused: %u frame%u\n",preflight.status,preflight.frame.status);return 4;}
  for(size_t i=0;i<n;i++)frame.named[i].words=native_words(&frame.input.buffers[i].bytes);
  coax_v_u32 uniforms=native_words(&frame.input.uniforms),hud=native_words(&frame.input.hud),draws=native_words(&frame.input.draws),indices=native_words(&frame.input.indices);
  int64_t resource_limit=aethyr_native_resource_limit();coax_t_RenderGeometryBuffer *geometry=calloc((size_t)resource_limit,sizeof*geometry);assert(geometry);
  coax_t_RenderHostGeometryBuffers mapped=aethyr_native_geometry_buffers(c[0],(coax_v_t_RenderHostNamedBuffer){frame.named,(int64_t)n},(coax_v_t_RenderGeometryBuffer){geometry,resource_limit});
  if(mapped.status!=COAX_RenderResourceStatus_ok){fprintf(stderr,"buffer mapping refused: %u\n",mapped.status);return 4;}
  coax_t_RenderDrawDomain *domains=calloc(counts.draw_count?(size_t)counts.draw_count:1,sizeof*domains);assert(domains);
  coax_t_RenderRuntimeFrame receipt=aethyr_render_frame(c[0],counts,limits,COAX_RenderReadiness_ready,true,span_uploads,draws,indices,
    (coax_v_f32){(float*)uniforms.data,uniforms.count},(coax_v_t_RenderGeometryBuffer){geometry,mapped.count},(coax_v_f32){(float*)hud.data,hud.count},(coax_v_t_RenderDrawDomain){domains,counts.draw_count},aethyr_render_geometry_work());
  if(receipt.status!=COAX_RenderRuntimeStatus_ok){fprintf(stderr,"frame refused: %u geometry%u frame%u\n",receipt.status,receipt.geometry_status,receipt.frame.status);return 4;}
  if(!rhgm_rects(c[0],&frame.input,frame.textures,c[1],c[2],&frame.media,error,sizeof error)){fprintf(stderr,"%s\n",error);return 5;}
  if(!rhgt_frame_admit(frame.textures,c[0],&receipt,coarse.value,screen_format,error,sizeof error)){fprintf(stderr,"%s\n",error);return 5;}
  // Every source/GPU span and the whole geometry/graph is admitted before writes.
  for(size_t i=0;i<n;i++)if(frame.input.buffers[i].bytes.size)memcpy(frame.buffers[i].map,frame.input.buffers[i].bytes.data,frame.input.buffers[i].bytes.size);
  if(frame.input.hud.size)memcpy(frame.hud.map,frame.input.hud.data,frame.input.hud.size);
  if(frame.input.indices.size)memcpy(frame.index.map,frame.input.indices.data,frame.input.indices.size);
  RhgFrame effects={.media_receipt=frame.media.receipt,.media_rects={frame.media.rects,frame.media.receipt.count},.hud=&frame.hud,.index=frame.index.buf?&frame.index:NULL,.draw_words=draws,.binding=native_binding,.user=&frame};
  if(!rhg_execute(frame.renderer,&receipt,&effects,coarse.value,screen_format,error,sizeof error)){fprintf(stderr,"%s\n",error);return 5;}
  VksImage *screen=rhg_screen(frame.renderer);assert(screen);size_t bytes=(size_t)screen->w*screen->h*screen_pixel_bytes;unsigned char *pixels=malloc(bytes);assert(pixels);vks_read(&frame.device,screen,pixels,bytes);
  FILE *image=fopen(argv[3],"wb");assert(image&&fwrite(pixels,1,bytes,image)==bytes&&!fclose(image));
  FILE *record=fopen(argv[4],"w");assert(record);fprintf(record,"{\"status\":\"rendered\",\"profile\":%" PRId64 ",\"resource\":%u,\"width\":%u,\"height\":%u,\"format\":\"%s\",\"bytes\":%zu,\"draw_commands\":%" PRIu64 ",\"device\":\"%s\",\"subPixelPrecisionBits\":%u,\"validation_errors\":%d,\"validation_warnings\":%d}\n",c[0],COAX_RenderTarget_screen,screen->w,screen->h,format_name,bytes,rhg_draw_commands(frame.renderer),frame.device.props.deviceName,frame.device.props.limits.subPixelPrecisionBits,vkc_validation_errors,vkc_validation_warnings);assert(!fclose(record));
  int failed=vkc_validation_errors||vkc_validation_warnings;
  rhg_close(frame.renderer);for(size_t i=0;i<n;i++)if(frame.buffers[i].buf)vkc_free(&frame.device,&frame.buffers[i]);if(frame.hud.buf)vkc_free(&frame.device,&frame.hud);if(frame.index.buf)vkc_free(&frame.device,&frame.index);
  for(OwnedBuffer *b=frame.generated;b;){OwnedBuffer *next=b->next;vkc_free(&frame.device,&b->gpu);free(b);b=next;}
  for(size_t i=0;i<frame.sampler_count;i++)vkDestroySampler(frame.device.dev,frame.samplers[i],NULL);
  rhgm_close(&frame.media);rhgt_close(frame.textures);
  vkc_close(&frame.device);free(pixels);free(uploads);free(geometry);free(domains);free(frame.named);free(frame.buffers);free(files.last);free(files.directory);rfi_close(&frame.input);return failed?6:0;
}
