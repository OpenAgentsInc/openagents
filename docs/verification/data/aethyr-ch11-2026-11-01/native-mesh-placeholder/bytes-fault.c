// Effects for an actual RealmHost producer. Resource origins, byte views,
// geometry, graph, states and draw decoding are decisions of the Coax facade.
#include "render-realm.h"
#include "render-textures.h"
#include "render-media.h"
#include "face.h"
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
  char name[32];coax_t_RenderBindingSource source;
  unsigned char *retained;const unsigned char *immutable;size_t capacity;
  VkcBuffer gpu;uint32_t version;int observed_version;
  int placeholder;
  coax_t_RenderHostUploadState upload;
} RrBuffer;
typedef struct RrGenerated {VkcBuffer gpu;struct RrGenerated *next;} RrGenerated;
struct RhgRealm {
  Vkc *device;RealmHost *producer;RhgRenderer *renderer;int64_t profile;
  char *package_directory;unsigned char *last_file;
  RrBuffer *buffers;coax_t_RenderHostNamedBuffer *named;size_t buffer_count,buffer_limit;
  VkcBuffer hud,index;unsigned char *index_bytes;size_t index_size;
  VkSampler samplers[16];coax_string sampler_names[16];size_t sampler_count;
  RrGenerated *generated;
  RfiFrame texture_inputs;RhgTextures *textures;
};
static int rr_error(char *error,size_t bytes,const char *message){if(bytes)snprintf(error,bytes,"%s",message);return 0;}
static int rr_same(coax_string a,coax_string b){return a.count==b.count&&!memcmp(a.data,b.data,(size_t)a.count);}
static unsigned char *rr_file(const char *directory,coax_string name,size_t *bytes){
  size_t prefix=strlen(directory);char *path=malloc(prefix+(size_t)name.count+2);if(!path)return NULL;
  memcpy(path,directory,prefix);path[prefix]='/';memcpy(path+prefix+1,name.data,(size_t)name.count);path[prefix+1+(size_t)name.count]=0;
  FILE *file=fopen(path,"rb");free(path);if(!file)return NULL;
  if(fseek(file,0,SEEK_END)){fclose(file);return NULL;}long size=ftell(file);if(size<0||fseek(file,0,SEEK_SET)){fclose(file);return NULL;}
  unsigned char *data=calloc((size_t)size+1,1);if(!data){fclose(file);return NULL;}
  if(fread(data,1,(size_t)size,file)!=(size_t)size){free(data);fclose(file);return NULL;}fclose(file);*bytes=(size_t)size;return data;
}
static RhgBytes rr_package_read(void *user,coax_string name){RhgRealm *r=user;free(r->last_file);size_t bytes=0;r->last_file=rr_file(r->package_directory,name,&bytes);return(RhgBytes){r->last_file,bytes};}
static unsigned char *rr_asset_file(coax_string url,size_t *bytes){return rr_file("render-assets",(coax_string){url.data+1,url.count-1},bytes);}
static int rr_deliver_data(RealmHost *h,int64_t row,const unsigned char *bytes,size_t size,char *error,size_t error_bytes){
  if(!aethyr_native_span(0,size,0,UINT32_MAX))return rr_error(error,error_bytes,"asset byte count does not fit the producer ABI");
  uint32_t capacity=coaxial_asset_capacity(h->state,(int32_t)row);
  uint32_t destination=coaxial_asset(h->state,(int32_t)row,(uint32_t)size);
  if(!aethyr_native_span((uint64_t)(uintptr_t)bytes,size,(uint64_t)(uintptr_t)bytes,size)
      ||aethyr_render_asset_span(destination,capacity,size,UINT32_MAX).status!=COAX_RenderFrameStatus_ok)return rr_error(error,error_bytes,"asset destination allocation refused by Coax");
  if(size)memcpy(rh_ptr(destination),bytes,size);return 1;
}
int rhgr_deliver(RealmHost *h,int64_t profile,char *error,size_t error_bytes){
  int64_t count=aethyr_render_asset_count(profile);if(count<0)return rr_error(error,error_bytes,"unknown canonical asset profile");
  for(int64_t row=0;row<count;row++){
    coax_t_RenderAsset asset=aethyr_render_asset(profile,row);if(!asset.initial&&!asset.gpu_required)continue;
    const unsigned char *bytes=NULL;unsigned char *owned=NULL;size_t size=0;
    if(asset.packed){
      if(!h->pack){size_t n=(size_t)asset.url.count;char *path=malloc(n+14);if(!path)return 0;memcpy(path,"render-assets",13);memcpy(path+13,asset.url.data,n);path[n+13]=0;int result=rh_pack(h,path);free(path);if(result)return rr_error(error,error_bytes,"canonical pack is unavailable");}
      const RhEntry *entry=rh_entry(h,(int)asset.pack_part);
      if(asset.kind==COAX_RenderAssetKind_layers){if(h->layers<1)return rr_error(error,error_bytes,"canonical packed layer delivery is missing");size=(size_t)h->layers;}
      else{if(!entry)return rr_error(error,error_bytes,"canonical packed delivery part is missing");bytes=entry->bytes;size=entry->length;}
      if(asset.pack_part==1){char progress[64];int n=snprintf(progress,sizeof progress,"%zu %zu",h->pack_size,h->pack_size);rh_send(h,RH_PACK_KIND,progress,(size_t)n);}
    }else if(asset.kind==COAX_RenderAssetKind_layers){
      for(int64_t layer=0;layer<asset.url_count;layer++){size_t length=0;unsigned char *image=rr_asset_file(aethyr_render_asset_url(profile,row,layer),&length);if(!image){free(owned);return rr_error(error,error_bytes,"canonical layer input is unavailable");}free(image);size++;}
    }else{owned=rr_asset_file(asset.url,&size);bytes=owned;if(!owned)return rr_error(error,error_bytes,"canonical asset input is unavailable");}
    if(asset.kind==COAX_RenderAssetKind_data&&!rr_deliver_data(h,row,bytes,size,error,error_bytes)){free(owned);return 0;}
    if(!aethyr_native_span(0,size,0,UINT32_MAX)){free(owned);return rr_error(error,error_bytes,"asset observation does not fit the producer ABI");}
    coaxial_asset_ready(h->state,(int32_t)asset.row,(uint32_t)size);free(owned);
  }
  return 1;
}
static coax_v_u32 rr_words(const void *pointer,size_t capacity,size_t bytes){
  uint64_t address=(uint64_t)(uintptr_t)pointer;coax_t_RenderHostWordView view=aethyr_render_words(address,address,capacity,bytes);
  if(view.status!=COAX_RenderFrameStatus_ok)return(coax_v_u32){0};return(coax_v_u32){(uint32_t*)(uintptr_t)view.address,view.words};
}
static coax_t_RenderUpload rr_upload(const void *pointer,size_t capacity,size_t bytes,const VkcBuffer *gpu){
  coax_t_RenderUpload upload;coax_default_t_RenderUpload(&upload);upload.source_address=(uint64_t)(uintptr_t)pointer;upload.source_base=upload.source_address;
  upload.source_bytes=capacity;upload.bytes=bytes;upload.gpu_bytes=gpu->size;return upload;
}
static int rr_add_buffer(RhgRealm *r,int64_t set,int64_t resource,coax_t_RenderHostBinding binding,char *error,size_t error_bytes){
  uint8_t scratch[32];coax_string name=aethyr_render_buffer_name(binding.source,(coax_v_u8){scratch,32});if(!name.count)return 1;
  for(size_t i=0;i<r->buffer_count;i++)if(rr_same(name,r->named[i].name))return 1; // same physical allocation, e.g. main/particle u
  if(r->buffer_count==r->buffer_limit)return rr_error(error,error_bytes,"named buffer observations exceed the Coax resource capacity");
  RrBuffer *buffer=&r->buffers[r->buffer_count];buffer->source=binding.source;memcpy(buffer->name,name.data,(size_t)name.count);
  RealmHost *h=r->producer;
  switch(binding.source.kind){
    case COAX_RenderSourceKind_main_uniform:buffer->capacity=(size_t)h->uniform_floats*4;break;
    case COAX_RenderSourceKind_dynamic_store:buffer->capacity=coaxial_store_cap((int32_t)binding.source.index);break;
    case COAX_RenderSourceKind_particle_emitters:buffer->capacity=coaxial_particle_cap();break;
    case COAX_RenderSourceKind_immutable_storage_asset:case COAX_RenderSourceKind_mesh_asset:{
      const RhEntry *entry=binding.source.kind==COAX_RenderSourceKind_immutable_storage_asset?rh_entry(h,RH_VERTICES):NULL;
      if(entry){buffer->capacity=entry->length;unsigned char *copy=malloc(entry->length?entry->length:1);if(!copy)return 0;memcpy(copy,entry->bytes,entry->length);buffer->immutable=copy;}
      else{size_t bytes=0;coax_t_RenderAsset asset=aethyr_render_asset_source(r->profile,binding.source);if(asset.valid&&!asset.packed)buffer->immutable=rr_asset_file(asset.url,&bytes);buffer->capacity=bytes;}
      if(!buffer->immutable){
        coax_t_RenderBufferView empty=aethyr_render_mesh_placeholder(r->profile,set,resource,r->device->props.limits.maxStorageBufferRange);
        if(empty.status!=COAX_RenderResourceStatus_ok)return rr_error(error,error_bytes,"immutable canonical buffer input is missing");
        buffer->capacity=(size_t)empty.bytes;buffer->immutable=calloc(buffer->capacity,1);if(!buffer->immutable)return 0;((unsigned char*)buffer->immutable)[0]=1;buffer->placeholder=1;
      }break;
    }
    default:return rr_error(error,error_bytes,"canonical source does not describe a physical buffer");
  }
  // The existing Coax resource-view policy owns minimum size and padding;
  // Vulkan's buffer itself has exactly this requested byte extent.
  coax_t_RenderBufferView allocation=aethyr_render_buffer_bytes(buffer->capacity,r->device->props.limits.maxStorageBufferRange);
  if(allocation.status!=COAX_RenderResourceStatus_ok)return rr_error(error,error_bytes,"physical buffer allocation refused by Coax");
  buffer->gpu=vkc_buffer(r->device,allocation.bytes);buffer->retained=calloc((size_t)buffer->gpu.size,1);if(!buffer->retained)return 0;
  r->named[r->buffer_count]=(coax_t_RenderHostNamedBuffer){.name={buffer->name,name.count},.observed={.source_capacity=buffer->capacity,.logical_payload=buffer->capacity,.initialized_highwater=buffer->gpu.size,.gpu_bytes=buffer->gpu.size,.alignment=4,.zero_initialized=true}};
  r->buffer_count++;return 1;
}
static VkSampler rr_sampler(RhgRealm *r,coax_string name){
  for(size_t i=0;i<r->sampler_count;i++)if(rr_same(name,r->sampler_names[i]))return r->samplers[i];
  coax_t_RenderSamplerDescriptor d=aethyr_render_sampler(name);if(d.status!=COAX_RenderResourceStatus_ok||r->sampler_count==16)return VK_NULL_HANDLE;
  VkSamplerCreateInfo info={.sType=VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,.magFilter=d.mag_linear?VK_FILTER_LINEAR:VK_FILTER_NEAREST,.minFilter=d.min_linear?VK_FILTER_LINEAR:VK_FILTER_NEAREST,
    .mipmapMode=d.mip_linear?VK_SAMPLER_MIPMAP_MODE_LINEAR:VK_SAMPLER_MIPMAP_MODE_NEAREST,.addressModeU=d.address_u==COAX_RenderSamplerAddress_repeat?VK_SAMPLER_ADDRESS_MODE_REPEAT:VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
    .addressModeV=d.address_v==COAX_RenderSamplerAddress_repeat?VK_SAMPLER_ADDRESS_MODE_REPEAT:VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,.addressModeW=VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
    .anisotropyEnable=d.physical_anisotropy>1,.maxAnisotropy=(float)d.physical_anisotropy,.compareEnable=d.compare!=COAX_RenderSamplerCompare_none,.compareOp=VK_COMPARE_OP_LESS,.maxLod=VK_LOD_CLAMP_NONE};
  VkSampler sampler;if(vkCreateSampler(r->device->dev,&info,NULL,&sampler)!=VK_SUCCESS)return VK_NULL_HANDLE;
  r->sampler_names[r->sampler_count]=name;r->samplers[r->sampler_count++]=sampler;return sampler;
}
static int rr_binding(void *user,coax_t_RenderHostUse use,coax_t_RenderOp op,const coax_t_RenderMediaRect *media,VksBinding *out){
  RhgRealm *r=user;memset(out,0,sizeof*out);
  if(use.use.native_kind==VKS_STORAGE){
    coax_t_RenderHostNamedBuffer generated={0};RrGenerated *owned=NULL;
    const void *generated_bytes=NULL;size_t generated_size=0;
    if(use.source.kind==COAX_RenderSourceKind_bloom_step){generated_bytes=op.bloom_step.e;generated_size=sizeof op.bloom_step.e;}
    if(use.source.kind==COAX_RenderSourceKind_media_uniform&&media){generated_bytes=media->uniforms.e;generated_size=sizeof media->uniforms.e;}
    if(generated_bytes){owned=calloc(1,sizeof*owned);if(!owned)return 0;owned->gpu=vkc_buffer(r->device,generated_size);owned->next=r->generated;r->generated=owned;
      generated.words=(coax_v_u32){(uint32_t*)generated_bytes,(int64_t)(generated_size/4)};generated.observed=(coax_t_RenderBufferObserved){.source_capacity=generated_size,.logical_payload=generated_size,.gpu_bytes=owned->gpu.size,.alignment=4,.zero_initialized=true};}
    coax_t_RenderHostBufferReceipt view=aethyr_render_resolve_buffer(r->profile,use.binding_set,use.use.resource,(coax_v_t_RenderHostNamedBuffer){r->named,(int64_t)r->buffer_count},generated);
    if(view.status!=COAX_RenderResourceStatus_ok)return 0;
    if(owned){memcpy(owned->gpu.map,generated_bytes,generated_size);out->buffer=&owned->gpu;}else out->buffer=&r->buffers[view.named_index].gpu;
    out->offset=view.view.offset;out->range=view.view.bytes;return 1;
  }
  if(use.use.native_kind==VKS_IMAGE){
    coax_t_RenderHostTextureReceipt view=aethyr_render_resolve_texture(r->profile,use.binding_set,use.use.resource,rhgt_names(r->textures),media?media->line:-1);
    if(view.status!=COAX_RenderResourceStatus_ok)return 0;
    if(view.source.kind==COAX_RenderSourceKind_render_target)out->image=rhg_target_image(r->renderer,view.source.target);
    if(view.source.kind==COAX_RenderSourceKind_pass_source)out->image=rhg_target_image(r->renderer,op.source);
    if(view.named_index>=0){char error[128];out->image=rhgt_image(r->textures,view,error,sizeof error);}
    return out->image!=NULL;
  }
  if(use.use.native_kind==VKS_SAMPLER){out->sampler=rr_sampler(r,use.source.sampler_name);return out->sampler!=VK_NULL_HANDLE;}return 0;
}
static coax_t_RenderAssetReadiness rr_asset_ready(RhgRealm *r){
  int64_t count=aethyr_render_asset_count(r->profile);coax_t_RenderAssetObservation *observed=rfi_array((size_t)count,sizeof*observed);
  if(!observed)return(coax_t_RenderAssetReadiness){0};
  for(int64_t row=0;row<count;row++)observed[row]=(coax_t_RenderAssetObservation){.row=row,.state=COAX_RenderAssetState_pending};
  bool particles=false;
  for(size_t i=0;i<r->buffer_count;i++){
    RrBuffer *buffer=&r->buffers[i];coax_t_RenderAsset asset=aethyr_render_asset_source(r->profile,buffer->source);
    if(asset.valid&&buffer->gpu.buf&&!buffer->placeholder)observed[asset.row].state=COAX_RenderAssetState_ready;
    if(buffer->source.kind==COAX_RenderSourceKind_particle_emitters&&buffer->gpu.buf)particles=true;
  }
  coax_t_RenderAsset index=aethyr_render_asset_find(r->profile,COAX_RenderAssetKind_index,0);if(index.valid&&r->index_bytes)observed[index.row].state=COAX_RenderAssetState_ready;
  for(int64_t set=0;set<2;set++)for(int64_t resource=0;resource<aethyr_render_binding_count(r->profile,set);resource++){
    coax_t_RenderHostBinding binding=aethyr_render_binding(r->profile,set,resource);if(binding.source.kind!=COAX_RenderSourceKind_layers)continue;
    coax_t_RenderAsset asset=aethyr_render_asset_source(r->profile,binding.source);
    coax_t_RenderHostTextureReceipt texture=aethyr_render_resolve_texture(r->profile,set,resource,rhgt_names(r->textures),-1);
    if(asset.valid&&texture.status==COAX_RenderResourceStatus_ok)observed[asset.row].state=COAX_RenderAssetState_ready;
  }
  // The package already authenticated both stage modules and device features;
  // actual VkPipeline creation is part of the candidate before submission.
  coax_t_RenderAssetReadiness result=aethyr_render_asset_readiness(r->profile,true,particles,(coax_v_t_RenderAssetObservation){observed,count});free(observed);return result;
}
RhgRealm *rhgr_open(Vkc *device,RealmHost *producer,int64_t profile,const char *package_file,char *error,size_t error_bytes){
  RhgRealm *r=calloc(1,sizeof*r);if(!r)return NULL;r->device=device;r->producer=producer;r->profile=profile;
  const char *slash=strrchr(package_file,'/');size_t n=slash?(size_t)(slash-package_file):1;r->package_directory=calloc(n+1,1);if(!r->package_directory){rhgr_close(r);return NULL;}
  if(slash)memcpy(r->package_directory,package_file,n);else r->package_directory[0]='.';const char *base=slash?slash+1:package_file;
  RhgBytes json=rr_package_read(r,(coax_string){base,(int64_t)strlen(base)});if(!json.data){rr_error(error,error_bytes,"canonical render package is missing");rhgr_close(r);return NULL;}
  r->renderer=rhg_open(device,profile,(coax_string){(char*)json.data,(int64_t)json.bytes},rr_package_read,r,error,error_bytes);if(!r->renderer){rhgr_close(r);return NULL;}
  coax_t_RenderHostBufferSizes sizes=aethyr_render_buffer_sizes(profile,producer->max_vertices,producer->floats,producer->uniform_floats,device->props.limits.maxStorageBufferRange);
  if(sizes.status!=COAX_RenderFrameStatus_ok){rr_error(error,error_bytes,"producer allocation layout refused by Coax");rhgr_close(r);return NULL;}
  r->hud=vkc_buffer(device,sizes.vertex_bytes);r->buffer_limit=(size_t)aethyr_native_resource_limit();r->buffers=calloc(r->buffer_limit,sizeof*r->buffers);r->named=calloc(r->buffer_limit,sizeof*r->named);
  if(!r->buffers||!r->named){rhgr_close(r);return NULL;}
  for(int64_t set=0;set<2;set++)for(int64_t resource=0;resource<aethyr_render_binding_count(profile,set);resource++){
    coax_t_RenderHostBinding binding=aethyr_render_binding(profile,set,resource);
    if(binding.binding.kind==COAX_RenderResourceKind_uniform||binding.binding.kind==COAX_RenderResourceKind_storage)if(!rr_add_buffer(r,set,resource,binding,error,error_bytes)){rhgr_close(r);return NULL;}
  }
  const RhEntry *index=rh_entry(producer,RH_INDICES);if(index){r->index_size=index->length;r->index_bytes=malloc(index->length?index->length:1);if(r->index_bytes)memcpy(r->index_bytes,index->bytes,index->length);}
  else{coax_t_RenderAsset asset=aethyr_render_asset_find(profile,COAX_RenderAssetKind_index,0);if(asset.valid){if(asset.packed){rr_error(error,error_bytes,"canonical packed index delivery is missing");rhgr_close(r);return NULL;}r->index_bytes=rr_asset_file(asset.url,&r->index_size);}}
  if(r->index_size)r->index=vkc_buffer(device,r->index_size);
  FILE *texture_file=fopen("render-assets/textures.config","r");if(texture_file){fclose(texture_file);if(!rfi_load_textures("render-assets/textures.config",&r->texture_inputs,error,error_bytes)){rhgr_close(r);return NULL;}}
  r->textures=rhgt_open(device,&r->texture_inputs,error,error_bytes);if(!r->textures){rhgr_close(r);return NULL;}return r;
}
int rhgr_frame(RhgRealm *r,uint32_t hud_vertices,uint32_t width,uint32_t height,int coarse,coax_t_RenderTextureFormat format,char *error,size_t error_bytes){
  RealmHost *h=r->producer;coax_t_RenderFrameLimits limits;coax_default_t_RenderFrameLimits(&limits);
  coax_t_FrameCounts counts={.width=width,.height=height,.hud_vertices=hud_vertices,.vertex_stride=h->floats,.draw_count=coaxial_draw_count(h->state),.water_draws=coaxial_water_draws(h->state),
    .grass_instances=coaxial_grass_instances(h->state),.particle_instances=coaxial_particle_instances(h->state),.surface_vertices=coaxial_surface_vertices(h->state),.caster_vertices=coaxial_caster_vertices(h->state),.caster_instances=coaxial_caster_instances(h->state)};
  size_t n=r->buffer_count;coax_t_RenderUpload *uploads=calloc(n+2,sizeof*uploads);const void **sources=calloc(n,sizeof*sources);size_t *bytes=calloc(n,sizeof*bytes);if(!uploads||!sources||!bytes){free(uploads);free(sources);free(bytes);return 0;}
  for(size_t i=0;i<n;i++){
    RrBuffer *b=&r->buffers[i];switch(b->source.kind){
      case COAX_RenderSourceKind_main_uniform:sources[i]=rh_ptr(h->uniforms);bytes[i]=b->capacity;break;
      case COAX_RenderSourceKind_dynamic_store:if(counts.draw_count){uint32_t version=coaxial_store_version(h->state,(int32_t)b->source.index);if(!b->observed_version||version!=b->version){sources[i]=rh_ptr(coaxial_store(h->state,(int32_t)b->source.index));bytes[i]=coaxial_store_bytes(h->state,(int32_t)b->source.index);b->version=version;b->observed_version=1;}}break;
      case COAX_RenderSourceKind_particle_emitters:sources[i]=rh_ptr(coaxial_particles(h->state));bytes[i]=coaxial_particle_bytes(h->state);break;
      default:sources[i]=b->immutable;bytes[i]=b->capacity;break;
    }
    uploads[i]=rr_upload(sources[i],b->capacity,bytes[i],&b->gpu);
  }
  uploads[n]=rr_upload(rh_ptr(h->vertices),(size_t)h->max_vertices*(size_t)h->floats*4,(size_t)hud_vertices*(size_t)h->floats*4,&r->hud);
  uploads[n+1]=rr_upload(r->index_bytes,r->index_size,r->index_size,&r->index);
  coax_t_RenderFrameResult span=aethyr_native_uploads(limits,(coax_v_t_RenderUpload){uploads,(int64_t)n+2});
  if(span.status!=COAX_RenderFrameStatus_ok){free(uploads);free(sources);free(bytes);return rr_error(error,error_bytes,"actual producer source/upload spans refused by Coax");}
  // Effectful store getters were read once. Retain admitted CPU candidates
  // across a later frame refusal, so a subsequent zero update cannot lose them.
  for(size_t i=0;i<n;i++){
    RrBuffer *b=&r->buffers[i];coax_t_RenderHostUploadReceipt staged=aethyr_render_upload_stage(b->upload,bytes[i],b->capacity);
    if(staged.status!=COAX_RenderFrameStatus_ok){free(sources);free(bytes);free(uploads);return rr_error(error,error_bytes,"pending producer upload refused by Coax");}
    if(bytes[i])memcpy(b->retained,sources[i],bytes[i]);b->upload=staged.next;
    r->named[i].words=rr_words(b->retained,(size_t)b->gpu.size,(size_t)b->gpu.size);uploads[i]=rr_upload(b->retained,(size_t)b->gpu.size,(size_t)b->upload.pending,&b->gpu);
  }
  free(sources);free(bytes);
  coax_t_RenderRuntimePreflight preflight=aethyr_render_preflight(r->profile,counts,limits,(coax_v_t_RenderUpload){uploads,(int64_t)n+2});
  if(preflight.status!=COAX_RenderRuntimeStatus_ok){free(uploads);return rr_error(error,error_bytes,"producer frame counts refused by Coax");}
  coax_v_u32 draws={0};if(counts.draw_count){
    coax_t_RenderHostWordView draw_view=aethyr_render_words((uint64_t)(uintptr_t)rh_ptr(coaxial_draws(h->state)),(uint64_t)(uintptr_t)h->arena,h->arena_size,(uint64_t)counts.draw_count*16);
    if(draw_view.status!=COAX_RenderFrameStatus_ok){free(uploads);return rr_error(error,error_bytes,"draw record source arena refused by Coax");}
    draws=(coax_v_u32){(uint32_t*)(uintptr_t)draw_view.address,draw_view.words};
  }
  coax_v_u32 indices=rr_words(r->index_bytes,r->index_size,r->index_size);coax_t_RenderGeometryBuffer *geometry=calloc(r->buffer_limit,sizeof*geometry);coax_t_RenderDrawDomain *domains=calloc(counts.draw_count?(size_t)counts.draw_count:1,sizeof*domains);
  if(!geometry||!domains){free(geometry);free(domains);free(uploads);return 0;}
  coax_t_RenderHostGeometryBuffers mapped=aethyr_render_geometry_buffers(r->profile,(coax_v_t_RenderHostNamedBuffer){r->named,(int64_t)n},(coax_v_t_RenderGeometryBuffer){geometry,(int64_t)r->buffer_limit});
  coax_v_f32 uniforms={(float*)rh_ptr(h->uniforms),h->uniform_floats};coax_v_f32 hud={(float*)rh_ptr(h->vertices),(int64_t)h->max_vertices*h->floats};
  coax_t_RenderAssetReadiness readiness=rr_asset_ready(r);
  coax_t_RenderRuntimeFrame receipt=aethyr_render_frame(r->profile,counts,limits,readiness.readiness,readiness.particles_ready,(coax_v_t_RenderUpload){uploads,(int64_t)n+2},draws,indices,uniforms,
    (coax_v_t_RenderGeometryBuffer){geometry,mapped.count},hud,(coax_v_t_RenderDrawDomain){domains,counts.draw_count},aethyr_render_geometry_work());
  free(geometry);free(domains);free(uploads);
  if(readiness.status!=COAX_RenderAssetStatus_ok||mapped.status!=COAX_RenderResourceStatus_ok||receipt.status!=COAX_RenderRuntimeStatus_ok){
    if(error_bytes)snprintf(error,error_bytes,"producer refused: readiness%u/%u mapping%u runtime%u geometry%u frame%u draw%lld upload%lld",readiness.status,readiness.readiness,mapped.status,receipt.status,receipt.geometry_status,receipt.frame.status,(long long)receipt.frame.bad_draw,(long long)receipt.frame.bad_upload);return 0;
  }
  // The native producer's media export is a borrowed pointer/length view
  // (its growable text can live in the Coax heap outside the state arena).
  uint32_t media_address=coaxial_media(h->state),media_bytes=coaxial_media_len(h->state);
  if(media_bytes&&!media_address)return rr_error(error,error_bytes,"native media export has no source allocation");
  RfiFrame media_input=r->texture_inputs;media_input.media_text.data=rh_ptr(media_address);media_input.media_text.size=media_bytes;
  RhgMediaRects media={0};if(!rhgm_rects(r->profile,&media_input,r->textures,width,height,&media,error,error_bytes))return 0;
  for(size_t i=0;i<n;i++)if(r->buffers[i].upload.pending)memcpy(r->buffers[i].gpu.map,r->buffers[i].retained,(size_t)r->buffers[i].upload.pending);
  if(hud_vertices)memcpy(r->hud.map,rh_ptr(h->vertices),(size_t)hud_vertices*(size_t)h->floats*4);if(r->index_size)memcpy(r->index.map,r->index_bytes,r->index_size);
  RhgFrame frame={.media_receipt=media.receipt,.media_rects={media.rects,media.receipt.count},.hud=&r->hud,.index=r->index_size?&r->index:NULL,.draw_words=draws,.binding=rr_binding,.user=r};int result=rhg_execute(r->renderer,&receipt,&frame,coarse,format,error,error_bytes);
  rhgm_close(&media);
  for(size_t i=0;i<n;i++)r->buffers[i].upload=aethyr_render_upload_ack(r->buffers[i].upload,result!=0);
  // execute waits for its submission; these per-op observations can now be
  // released without accumulating device allocations across frames.
  for(RrGenerated *b=r->generated;b;){RrGenerated *next=b->next;vkc_free(r->device,&b->gpu);free(b);b=next;}r->generated=NULL;return result;
}
VksImage *rhgr_screen(RhgRealm *r){return rhg_screen(r->renderer);}
uint64_t rhgr_draw_commands(const RhgRealm *r){return rhg_draw_commands(r->renderer);}
void rhgr_close(RhgRealm *r){
  if(!r)return;rhg_close(r->renderer);rhgt_close(r->textures);rfi_close(&r->texture_inputs);for(size_t i=0;i<r->buffer_count;i++){vkc_free(r->device,&r->buffers[i].gpu);free(r->buffers[i].retained);free((void*)r->buffers[i].immutable);}
  if(r->hud.buf)vkc_free(r->device,&r->hud);if(r->index.buf)vkc_free(r->device,&r->index);for(size_t i=0;i<r->sampler_count;i++)vkDestroySampler(r->device->dev,r->samplers[i],NULL);
  for(RrGenerated *b=r->generated;b;){RrGenerated *next=b->next;vkc_free(r->device,&b->gpu);free(b);b=next;}
  free(r->buffers);free(r->named);free(r->index_bytes);free(r->last_file);free(r->package_directory);free(r);
}
