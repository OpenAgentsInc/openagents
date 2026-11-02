// Scalar browser transport for the admitted Coax renderer. Input storage is
// statically owned; C projects typed receipts and performs no render policy.
#include "Render.h"
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#ifdef __wasm__
#define RENDER_EXPORT(name) __attribute__((export_name(name)))
#else
#define RENDER_EXPORT(name)
#endif

static coax_t_RenderHostPackageState render_package;
static coax_t_RenderHostPackageCandidate render_candidate;
static int64_t render_descriptor_profile;
static uint32_t render_initialized;
static coax_t_RenderHostJsonBank render_json[2];
static coax_t_RenderHostEntryBank render_entries[2];
static coax_t_RenderHostShaderBytes render_shader;
static coax_t_RenderDeviceCapabilities render_device;
static coax_t_RenderFrameLimits render_limits;
static coax_t_RenderRuntimeFrame render_frame;
static coax_t_RenderHostFlatLayout render_layout;
static coax_t_RenderHostBufferSizes render_sizes;
static coax_t_RenderBufferView render_buffer_view;
static coax_t_RenderTargetDescriptor render_target;
static coax_t_RenderSamplerDescriptor render_sampler;
static coax_t_RenderTextureLayout render_texture_layout;
static coax_t_RenderSansProfile render_sans;
static coax_t_RenderSansLevel render_sans_level;
static coax_t_RenderSansGlyph render_sans_glyph;
static uint8_t render_name_scratch[32];
static coax_string render_name;
static coax_t_RenderHostStage render_stage;
static coax_t_FrameCounts render_counts;
static coax_t_RenderHostNamedBank render_named;
static coax_t_RenderHostGeometryBank render_geometry;
static coax_t_RenderHostUploadBank render_uploads;
static int64_t render_named_count;
static coax_t_RenderHostGeometryBuffers render_geometry_result;
static coax_t_RenderHostBufferReceipt render_resolved;
static coax_v_u32 render_draw_words;
static uint64_t render_draw_base,render_draw_capacity;
static coax_t_RenderDecodedDraw render_decoded;
static coax_v_u32 render_index_words;
static coax_t_RenderHostAssetBank render_assets;
static int64_t render_asset_count;
static coax_t_RenderAsset render_asset;
static coax_string render_asset_text;
static coax_t_RenderAssetReadiness render_asset_ready;
static coax_t_RenderAssetMip render_asset_mip;
static coax_t_RenderAssetLayers render_layer_profile;
static coax_t_RenderHostRegistryBank render_registry;
static coax_t_RrGrant render_grant;

static coax_v_t_RenderHostPackageState package_view(void){return(coax_v_t_RenderHostPackageState){&render_package,1};}
static coax_v_t_RenderHostPackageCandidate candidate_view(void){return(coax_v_t_RenderHostPackageCandidate){&render_candidate,1};}
static coax_v_u8 shader_view(void){return aethyr_render_shader_storage((coax_v_t_RenderHostShaderBytes){&render_shader,1});}
static coax_v_t_RenderRegistry registry_view(void){return aethyr_render_registry_storage((coax_v_t_RenderHostRegistryBank){&render_registry,1});}
static coax_t_RenderOp operation(int32_t index){return aethyr_render_operation((coax_v_t_RenderRuntimeFrame){&render_frame,1},index);}

RENDER_EXPORT("coaxial_render_reset") void coaxial_render_reset(void){
  // A second reset is a new device: every object of the old one is released.
  if(render_initialized)aethyr_render_registry_lose(registry_view());else aethyr_render_registry_open(registry_view());
  render_grant=(coax_t_RrGrant){0};
  coax_default_t_RenderHostPackageState(&render_package);coax_default_t_RenderHostPackageCandidate(&render_candidate);render_descriptor_profile=-1;render_initialized=1;
  coax_default_t_RenderFrameLimits(&render_limits);
  coax_default_t_RenderRuntimeFrame(&render_frame);
  render_device=aethyr_render_device(0,0,0);
  render_sans=aethyr_render_sans_profile();
  coax_default_t_RenderHostUploadBank(&render_uploads);render_named_count=0;render_draw_base=0;render_draw_capacity=0;
}
// The GPU resource ledger: ask before creating an object, report its release.
// Statuses are RrStatus (0 admits); the grant's slot and generation follow.
RENDER_EXPORT("coaxial_render_acquire") uint32_t coaxial_render_acquire(int32_t kind,uint64_t bytes){
  int64_t words=bytes>(uint64_t)INT64_MAX?-1:aethyr_render_buffer_words((int64_t)bytes);
  render_grant=aethyr_render_registry_acquire(registry_view(),kind,words);return render_grant.status;
}
RENDER_EXPORT("coaxial_render_acquire_texture") uint32_t coaxial_render_acquire_texture(int32_t kind,int32_t format,int32_t width,int32_t height,int32_t layers,int32_t mips){
  render_grant=aethyr_render_registry_acquire(registry_view(),kind,aethyr_render_format_words(format,width,height,layers,mips));return render_grant.status;
}
RENDER_EXPORT("coaxial_render_grant") int64_t coaxial_render_grant(int32_t field){return field==0?render_grant.handle.slot:field==1?render_grant.handle.generation:-1;}
RENDER_EXPORT("coaxial_render_release") uint32_t coaxial_render_release(int32_t slot,int32_t generation){return aethyr_render_registry_release(registry_view(),slot,generation);}
RENDER_EXPORT("coaxial_render_live") uint32_t coaxial_render_live(int32_t slot,int32_t generation){return aethyr_render_registry_live(registry_view(),slot,generation);}
RENDER_EXPORT("coaxial_render_advance") uint32_t coaxial_render_advance(void){return aethyr_render_registry_advance(registry_view());}
RENDER_EXPORT("coaxial_render_registry_info") int64_t coaxial_render_registry_info(int32_t field){
  coax_v_t_RenderRegistry v=registry_view();
  switch(field){case 0:return v.data[0].epoch;case 1:return v.data[0].live;case 2:return v.data[0].words;case 3:return v.data[0].limit.slots;case 4:return v.data[0].limit.words;default:return -1;}
}
RENDER_EXPORT("coaxial_render_initialized") uint32_t coaxial_render_initialized(void){return render_initialized;}
// Enum projections are generated-header values, not a second host registry.
RENDER_EXPORT("coaxial_render_enum") int32_t coaxial_render_enum(int32_t family,int32_t field){
  switch(family){
    case 0:switch(field){case 0:return COAX_RenderTextureFormat_rgba8;case 1:return COAX_RenderTextureFormat_bgra8;case 2:return COAX_RenderTextureFormat_rgba16f;case 3:return COAX_RenderTextureFormat_depth32;case 4:return COAX_RenderTextureFormat_invalid;}break;
    case 1:switch(field){case 0:return COAX_RenderResourceKind_uniform;case 1:return COAX_RenderResourceKind_storage;case 2:return COAX_RenderResourceKind_image2d;case 3:return COAX_RenderResourceKind_image_array;case 4:return COAX_RenderResourceKind_depth;case 5:return COAX_RenderResourceKind_sampler;case 6:return COAX_RenderResourceKind_comparison;case 7:return COAX_RenderResourceKind_invalid;}break;
    case 2:switch(field){case 0:return COAX_RenderSourceKind_invalid;case 1:return COAX_RenderSourceKind_main_uniform;case 2:return COAX_RenderSourceKind_media_uniform;case 3:return COAX_RenderSourceKind_immutable_storage_asset;case 4:return COAX_RenderSourceKind_dynamic_store;case 5:return COAX_RenderSourceKind_particle_emitters;case 6:return COAX_RenderSourceKind_mesh_asset;case 7:return COAX_RenderSourceKind_layers;case 8:return COAX_RenderSourceKind_sans_atlas;case 9:return COAX_RenderSourceKind_render_target;case 10:return COAX_RenderSourceKind_sampler;case 11:return COAX_RenderSourceKind_media_image;case 12:return COAX_RenderSourceKind_bloom_step;case 13:return COAX_RenderSourceKind_pass_source;}break;
    case 3:switch(field){case 0:return COAX_RenderDrawKind_skip;case 1:return COAX_RenderDrawKind_indexed;case 2:return COAX_RenderDrawKind_nonindexed;case 3:return COAX_RenderDrawKind_invalid;}break;
    case 4:switch(field){case 0:return COAX_RenderAssetKind_invalid;case 1:return COAX_RenderAssetKind_data;case 2:return COAX_RenderAssetKind_storage;case 3:return COAX_RenderAssetKind_index;case 4:return COAX_RenderAssetKind_layers;case 5:return COAX_RenderAssetKind_sound;case 6:return COAX_RenderAssetKind_loop;case 7:return COAX_RenderAssetKind_particles;case 8:return COAX_RenderAssetKind_mesh;}break;
    case 6:switch(field){case 0:return COAX_RenderMediaKind_invalid;case 1:return COAX_RenderMediaKind_image;case 2:return COAX_RenderMediaKind_video;}break;
    case 7:return (int32_t)aethyr_render_resource_kind(field);
    case 5:switch(field){case 0:return COAX_RenderReadiness_loading;case 1:return COAX_RenderReadiness_background;case 2:return COAX_RenderReadiness_ready;case 3:return COAX_RenderReadiness_invalid;}break;
  }return -1;
}
RENDER_EXPORT("coaxial_render_abi") uint32_t coaxial_render_abi(void){return(uint32_t)aethyr_render_abi();}
RENDER_EXPORT("coaxial_render_device") void coaxial_render_device(int32_t backend,int32_t helper,int32_t int64){coax_t_RenderHostBool h=aethyr_render_bool(helper),i=aethyr_render_bool(int64);render_device=aethyr_render_device(h.ok&&i.ok?backend:0,h.value,i.value);}
RENDER_EXPORT("coaxial_render_json_admit") uint32_t coaxial_render_json_admit(int32_t count){return aethyr_render_json_size(count);}
RENDER_EXPORT("coaxial_render_json_ptr") uint32_t coaxial_render_json_ptr(void){
  int64_t bank=aethyr_render_candidate_bank(package_view());if(bank<0)return 0;
  coax_v_u8 bytes=aethyr_render_json_storage((coax_v_t_RenderHostJsonBank){&render_json[bank],1});return(uint32_t)(uintptr_t)bytes.data;
}
RENDER_EXPORT("coaxial_render_package") uint32_t coaxial_render_package(int32_t count,int32_t profile){
  coax_t_RenderPackageStatus status=aethyr_render_json_size(count);if(status!=COAX_RenderPackageStatus_ok)return status;
  int64_t bank=aethyr_render_candidate_bank(package_view());if(bank<0)return COAX_RenderPackageStatus_invalid;
  coax_v_u8 bytes=aethyr_render_json_storage((coax_v_t_RenderHostJsonBank){&render_json[bank],1});
  coax_v_t_RenderPackageEntry entries=aethyr_render_entry_storage((coax_v_t_RenderHostEntryBank){&render_entries[bank],1});
  status=aethyr_render_package_load(package_view(),(coax_string){(const char*)bytes.data,count},entries,profile,render_device,bank);if(status==COAX_RenderPackageStatus_ok)render_descriptor_profile=render_package.package.profile_index;return status;
}
RENDER_EXPORT("coaxial_render_shader_ptr") uint32_t coaxial_render_shader_ptr(void){return(uint32_t)(uintptr_t)shader_view().data;}
RENDER_EXPORT("coaxial_render_shader_admit") uint32_t coaxial_render_shader_admit(int32_t count){return aethyr_render_shader_size(count);}
RENDER_EXPORT("coaxial_render_module") uint32_t coaxial_render_module(int32_t set,int32_t count){
  if(!aethyr_render_shader_size(count))return COAX_RenderPackageStatus_capacity;
  coax_v_u8 bytes=shader_view();bytes.count=count;return aethyr_render_module_bytes(render_package.package,set,bytes);
}
RENDER_EXPORT("coaxial_render_profile") int32_t coaxial_render_profile(void){return(int32_t)render_descriptor_profile;}
RENDER_EXPORT("coaxial_render_assets_begin") int32_t coaxial_render_assets_begin(void){
  render_asset_count=aethyr_render_asset_count(render_descriptor_profile);
  if(render_asset_count<0||(uint64_t)render_asset_count>sizeof render_assets.observations.e/sizeof render_assets.observations.e[0])return -1;
  for(int64_t i=0;i<render_asset_count;i++)render_assets.observations.e[i]=(coax_t_RenderAssetObservation){.row=i,.state=COAX_RenderAssetState_pending};return(int32_t)render_asset_count;
}
RENDER_EXPORT("coaxial_render_asset") uint32_t coaxial_render_asset(int32_t row){render_asset=aethyr_render_asset(render_descriptor_profile,row);return render_asset.valid;}
RENDER_EXPORT("coaxial_render_asset_info") int64_t coaxial_render_asset_info(int32_t field){switch(field){case 0:return render_asset.row;case 1:return render_asset.kind;case 2:return render_asset.packed;case 3:return render_asset.pack_part;case 4:return render_asset.pack_first;case 5:return render_asset.url_count;case 6:return render_asset.initial;case 7:return render_asset.gpu_required;default:return -1;}}
RENDER_EXPORT("coaxial_render_asset_url") uint32_t coaxial_render_asset_url(int32_t row,int32_t index){render_asset_text=aethyr_render_asset_url(render_descriptor_profile,row,index);return(uint32_t)(uintptr_t)render_asset_text.data;}
RENDER_EXPORT("coaxial_render_asset_name") uint32_t coaxial_render_asset_name(void){render_asset_text=render_asset.name;return(uint32_t)(uintptr_t)render_asset_text.data;}
RENDER_EXPORT("coaxial_render_asset_text_bytes") int32_t coaxial_render_asset_text_bytes(void){return(int32_t)render_asset_text.count;}
RENDER_EXPORT("coaxial_render_asset_for_source") int32_t coaxial_render_asset_for_source(int32_t set,int32_t resource){coax_t_RenderHostBinding b=aethyr_render_binding(render_descriptor_profile,set,resource);coax_t_RenderAsset a=aethyr_render_asset_source(render_descriptor_profile,b.source);return a.valid?(int32_t)a.row:-1;}
RENDER_EXPORT("coaxial_render_index_asset") int32_t coaxial_render_index_asset(void){coax_t_RenderAsset a=aethyr_render_asset_find(render_descriptor_profile,COAX_RenderAssetKind_index,0);return a.valid?(int32_t)a.row:-1;}
RENDER_EXPORT("coaxial_render_asset_complete") void coaxial_render_asset_complete(int32_t row,int32_t success){if(row<0||row>=render_asset_count)return;coax_t_RenderHostBool q=aethyr_render_bool(success);if(!q.ok)return;render_assets.observations.e[row].state=q.value?COAX_RenderAssetState_ready:COAX_RenderAssetState_failed;}
RENDER_EXPORT("coaxial_render_asset_readiness") uint32_t coaxial_render_asset_readiness(int32_t pipeline,int32_t particles){coax_t_RenderHostBool p=aethyr_render_bool(pipeline),f=aethyr_render_bool(particles);if(!p.ok||!f.ok)return COAX_RenderAssetStatus_invalid;render_asset_ready=aethyr_render_asset_readiness(render_descriptor_profile,p.value,f.value,(coax_v_t_RenderAssetObservation){render_assets.observations.e,render_asset_count});return render_asset_ready.status;}
RENDER_EXPORT("coaxial_render_asset_readiness_info") int64_t coaxial_render_asset_readiness_info(int32_t field){switch(field){case 0:return render_asset_ready.readiness;case 1:return render_asset_ready.particles_ready;case 2:return render_asset_ready.mesh_ready;case 3:return render_asset_ready.bad_row;case 4:return render_asset_ready.failed_row;default:return -1;}}
RENDER_EXPORT("coaxial_render_memory_limit") uint64_t coaxial_render_memory_limit(void){return aethyr_render_memory_limit();}
RENDER_EXPORT("coaxial_render_stage") uint32_t coaxial_render_stage(uint64_t cursor,uint64_t bytes,uint64_t limit){render_stage=aethyr_render_stage(cursor,bytes,limit);return render_stage.status;}
RENDER_EXPORT("coaxial_render_stage_info") uint64_t coaxial_render_stage_info(int32_t field){switch(field){case 0:return render_stage.address;case 1:return render_stage.next;case 2:return render_stage.pages;default:return 0;}}

RENDER_EXPORT("coaxial_render_buffer_sizes") uint32_t coaxial_render_buffer_sizes(int32_t vertices,int32_t stride,int32_t uniforms,uint64_t device_bytes){
  render_sizes=aethyr_render_buffer_sizes(render_descriptor_profile,vertices,stride,uniforms,device_bytes);return render_sizes.status;
}
RENDER_EXPORT("coaxial_render_buffer_size") uint64_t coaxial_render_buffer_size(int32_t kind){return kind==0?render_sizes.vertex_bytes:render_sizes.uniform_bytes;}
RENDER_EXPORT("coaxial_render_counts") uint32_t coaxial_render_counts(int32_t width,int32_t height,int32_t hud,int32_t stride,int32_t draws,int32_t water,int32_t grass,int32_t particles,int32_t surface,int32_t caster_vertices,int32_t caster_instances){
  render_counts=(coax_t_FrameCounts){.width=width,.height=height,.hud_vertices=hud,.vertex_stride=stride,.draw_count=draws,.water_draws=water,.grass_instances=grass,.particle_instances=particles,.surface_vertices=surface,.caster_vertices=caster_vertices,.caster_instances=caster_instances};
  coax_t_RenderRuntimePreflight receipt=aethyr_render_preflight(render_package.package.profile_index,render_counts,render_limits,(coax_v_t_RenderUpload){0,0});
  return receipt.status;
}
RENDER_EXPORT("coaxial_render_source_words") uint32_t coaxial_render_source_words(uint32_t address,uint32_t bytes,uint32_t memory_bytes){return aethyr_render_words(address,0,memory_bytes,bytes).status;}
RENDER_EXPORT("coaxial_render_named_count") uint32_t coaxial_render_named_count(int32_t count){
  if(count<0||(uint64_t)count>sizeof render_named.buffers.e/sizeof render_named.buffers.e[0])return 0;
  render_named_count=count;return 1;
}
RENDER_EXPORT("coaxial_render_named") uint32_t coaxial_render_named(int32_t slot,int32_t set,int32_t resource,uint32_t address,uint32_t bytes,uint32_t memory_bytes,
    uint64_t capacity,uint64_t logical,uint64_t initialized,uint64_t upload,uint64_t gpu,uint64_t offset,uint64_t alignment,int32_t zero){
  if(slot<0||(uint64_t)slot>=sizeof render_named.buffers.e/sizeof render_named.buffers.e[0])return 0;
  coax_t_RenderHostBool z=aethyr_render_bool(zero);if(!z.ok)return 0;
  coax_t_RenderHostWordView span=aethyr_render_words(address,0,memory_bytes,bytes);if(span.status!=COAX_RenderFrameStatus_ok)return 0;
  coax_t_RenderHostBinding binding=aethyr_render_binding(render_descriptor_profile,set,resource);
  coax_string name=aethyr_render_buffer_name(binding.source,(coax_v_u8){render_named.names.e[slot].e,sizeof render_named.names.e[slot].e});if(!name.count)return 0;
  render_named.buffers.e[slot]=(coax_t_RenderHostNamedBuffer){.name=name,.words={(uint32_t*)(uintptr_t)span.address,span.words},
    .observed={.source_capacity=capacity,.logical_payload=logical,.initialized_highwater=initialized,.upload_bytes=upload,.gpu_bytes=gpu,.offset=offset,.alignment=alignment,.zero_initialized=z.value}};
  return 1;
}
RENDER_EXPORT("coaxial_render_upload_pending") uint64_t coaxial_render_upload_pending(int32_t slot){
  if(slot<0||(uint64_t)slot>=sizeof render_uploads.states.e/sizeof render_uploads.states.e[0])return 0;return render_uploads.states.e[slot].pending;
}
RENDER_EXPORT("coaxial_render_upload_stage") uint32_t coaxial_render_upload_stage(int32_t slot,uint64_t incoming,uint64_t capacity){
  if(slot<0||(uint64_t)slot>=sizeof render_uploads.states.e/sizeof render_uploads.states.e[0])return COAX_RenderFrameStatus_bad_upload;
  coax_t_RenderHostUploadReceipt receipt=aethyr_render_upload_stage(render_uploads.states.e[slot],incoming,capacity);render_uploads.states.e[slot]=receipt.next;return receipt.status;
}
RENDER_EXPORT("coaxial_render_upload_ack") void coaxial_render_upload_ack(int32_t slot,int32_t submitted){
  if(slot<0||(uint64_t)slot>=sizeof render_uploads.states.e/sizeof render_uploads.states.e[0])return;
  coax_t_RenderHostBool q=aethyr_render_bool(submitted);if(!q.ok)return;render_uploads.states.e[slot]=aethyr_render_upload_ack(render_uploads.states.e[slot],q.value);
}
RENDER_EXPORT("coaxial_render_index_stage") uint32_t coaxial_render_index_stage(uint64_t incoming,uint64_t capacity){coax_t_RenderHostUploadReceipt r=aethyr_render_upload_stage(render_uploads.index,incoming,capacity);render_uploads.index=r.next;return r.status;}
RENDER_EXPORT("coaxial_render_index_pending") uint64_t coaxial_render_index_pending(void){return render_uploads.index.pending;}
RENDER_EXPORT("coaxial_render_index_ack") void coaxial_render_index_ack(int32_t submitted){coax_t_RenderHostBool q=aethyr_render_bool(submitted);if(!q.ok)return;render_uploads.index=aethyr_render_upload_ack(render_uploads.index,q.value);}
RENDER_EXPORT("coaxial_render_draw_source") void coaxial_render_draw_source(uint32_t base,uint64_t capacity){render_draw_base=base;render_draw_capacity=capacity;}
static coax_t_RenderHostWordView render_draw_source_view(uint32_t address,uint64_t bytes,uint32_t memory_bytes){
  coax_t_RenderHostByteView span=aethyr_render_asset_span(address,render_draw_capacity,bytes,memory_bytes);
  if(span.status!=COAX_RenderFrameStatus_ok)return(coax_t_RenderHostWordView){.status=span.status};
  return aethyr_render_words(address,render_draw_base,render_draw_capacity,bytes);
}
RENDER_EXPORT("coaxial_render_scene_frame") uint32_t coaxial_render_scene_frame(uint32_t vertices,uint32_t uniforms,int32_t maximum,int32_t uniform_floats,
    uint32_t draws,uint32_t indices,uint32_t index_bytes,uint32_t memory_bytes,uint32_t vertex_gpu,uint32_t uniform_gpu,uint32_t index_gpu,int32_t readiness,int32_t particles_ready){
  coax_default_t_RenderRuntimeFrame(&render_frame);
  coax_t_RenderReadiness admitted_readiness=aethyr_render_readiness(readiness);coax_t_RenderHostBool admitted_particles=aethyr_render_bool(particles_ready);if(admitted_readiness==COAX_RenderReadiness_invalid||!admitted_particles.ok)return render_frame.status;
  render_layout=aethyr_render_vertex_layout(render_package.package.profile_index,render_counts,render_limits,maximum,uniform_floats,vertices,uniforms,memory_bytes,vertex_gpu,uniform_gpu);
  if(render_layout.status!=COAX_RenderRuntimeStatus_ok){render_frame.status=render_layout.status;render_frame.frame=render_layout.frame;return render_frame.status;}
  coax_t_RenderHostWordView dw=render_draw_source_view(draws,(uint64_t)render_counts.draw_count*4*sizeof(uint32_t),memory_bytes);
  coax_t_RenderHostWordView ix=aethyr_render_words(indices,0,memory_bytes,index_bytes);
  if(dw.status!=COAX_RenderFrameStatus_ok||ix.status!=COAX_RenderFrameStatus_ok)return render_frame.status;
  render_draw_words=(coax_v_u32){(uint32_t*)(uintptr_t)dw.address,dw.words};
  render_index_words=(coax_v_u32){(uint32_t*)(uintptr_t)ix.address,ix.words};
  coax_v_t_RenderHostNamedBuffer named={render_named.buffers.e,render_named_count};
  coax_v_t_RenderGeometryBuffer buffers={render_geometry.buffers.e,sizeof render_geometry.buffers.e/sizeof render_geometry.buffers.e[0]};
  render_geometry_result=aethyr_render_geometry_buffers(render_package.package.profile_index,named,buffers);
  if(render_geometry_result.status!=COAX_RenderResourceStatus_ok)return render_frame.status;
  buffers.count=render_geometry_result.count;
  render_uploads.uploads.e[0]=render_layout.uploads.e[0];render_uploads.uploads.e[1]=render_layout.uploads.e[1];
  render_uploads.uploads.e[2]=(coax_t_RenderUpload){.source_address=ix.address,.source_base=ix.address,.source_bytes=index_bytes,.bytes=render_uploads.index.pending,.gpu_bytes=index_gpu,.alignment=4};
  for(int64_t i=0;i<render_named_count;i++){
    coax_t_RenderHostNamedBuffer b=render_named.buffers.e[i];
    render_uploads.uploads.e[i+3]=(coax_t_RenderUpload){.source_address=(uintptr_t)b.words.data,.source_base=(uintptr_t)b.words.data,.source_bytes=(uint64_t)b.words.count*sizeof(uint32_t),.bytes=b.observed.upload_bytes,.gpu_offset=b.observed.offset,.gpu_bytes=b.observed.gpu_bytes,.alignment=4};
  }
  coax_v_t_RenderUpload uploads={render_uploads.uploads.e,render_named_count+3};
  coax_v_t_RenderDrawDomain domains={render_geometry.domains.e,sizeof render_geometry.domains.e/sizeof render_geometry.domains.e[0]};
  render_frame=aethyr_render_frame(render_package.package.profile_index,render_counts,render_limits,admitted_readiness,admitted_particles.value,uploads,render_draw_words,
    (coax_v_u32){(uint32_t*)(uintptr_t)ix.address,ix.words},(coax_v_f32){(float*)(uintptr_t)render_layout.uniform_address,render_layout.uniform_floats},buffers,
    (coax_v_f32){(float*)(uintptr_t)render_layout.vertex_address,render_layout.vertex_floats},domains,aethyr_render_geometry_work());return render_frame.status;
}
RENDER_EXPORT("coaxial_render_resolve_buffer") uint32_t coaxial_render_resolve_buffer(int32_t set,int32_t resource){
  render_resolved=aethyr_render_resolve_buffer(render_package.package.profile_index,set,resource,(coax_v_t_RenderHostNamedBuffer){render_named.buffers.e,render_named_count},(coax_t_RenderHostNamedBuffer){0});return render_resolved.status;
}
RENDER_EXPORT("coaxial_render_resolved_info") int64_t coaxial_render_resolved_info(int32_t field){switch(field){case 0:return render_resolved.named_index;case 1:return(int64_t)render_resolved.view.offset;case 2:return(int64_t)render_resolved.view.bytes;case 3:return(uintptr_t)render_resolved.words.data;case 4:return render_resolved.words.count*sizeof(uint32_t);default:return 0;}}
RENDER_EXPORT("coaxial_render_bloom") uint32_t coaxial_render_bloom(int32_t index){coax_t_RenderOp op=operation(index);if(!op.state.valid)return 0;return(uint32_t)(uintptr_t)render_frame.plan.ops.e[index].bloom_step.e;}
RENDER_EXPORT("coaxial_render_bloom_bytes") uint32_t coaxial_render_bloom_bytes(void){return sizeof render_frame.plan.ops.e[0].bloom_step.e;}
RENDER_EXPORT("coaxial_render_generated_buffer") uint32_t coaxial_render_generated_buffer(int32_t set,int32_t resource,uint32_t address,uint32_t bytes,uint32_t memory_bytes,uint64_t gpu_bytes){
  coax_t_RenderHostWordView span=aethyr_render_words(address,0,memory_bytes,bytes);if(span.status!=COAX_RenderFrameStatus_ok)return COAX_RenderResourceStatus_invalid;
  coax_t_RenderHostNamedBuffer generated=aethyr_render_owned_buffer((coax_string){0,0},(coax_v_u32){(uint32_t*)(uintptr_t)span.address,span.words});
  generated.observed.gpu_bytes=gpu_bytes;
  render_resolved=aethyr_render_resolve_buffer(render_package.package.profile_index,set,resource,(coax_v_t_RenderHostNamedBuffer){render_named.buffers.e,render_named_count},generated);return render_resolved.status;
}
RENDER_EXPORT("coaxial_render_decode") uint32_t coaxial_render_decode(int32_t record){render_decoded=aethyr_render_decode_draw(render_draw_words,record);return render_decoded.kind;}
RENDER_EXPORT("coaxial_render_decoded") uint64_t coaxial_render_decoded(int32_t field){switch(field){case 0:return render_decoded.first;case 1:return render_decoded.count;case 2:return render_decoded.base_instance;case 3:return render_decoded.instances;default:return 0;}}
RENDER_EXPORT("coaxial_render_flat_frame") uint32_t coaxial_render_flat_frame(uint32_t vertices,uint32_t uniforms,int32_t maximum,int32_t stride,int32_t uniform_floats,
    int32_t count,int32_t width,int32_t height,uint32_t memory_bytes,uint32_t vertex_gpu,uint32_t uniform_gpu){
  coax_default_t_RenderRuntimeFrame(&render_frame);
  coax_t_FrameCounts counts={.width=width,.height=height,.hud_vertices=count,.vertex_stride=stride};
  render_layout=aethyr_render_flat_layout(render_package.package.profile_index,counts,render_limits,maximum,uniform_floats,vertices,uniforms,memory_bytes,vertex_gpu,uniform_gpu);
  if(render_layout.status!=COAX_RenderRuntimeStatus_ok){render_frame.status=render_layout.status;render_frame.frame=render_layout.frame;return render_frame.status;}
  // All addresses and lengths below are Coax-admitted byte spans.
  coax_v_f32 vertex_view={(float*)(uintptr_t)render_layout.vertex_address,render_layout.vertex_floats};
  coax_v_f32 uniform_view={(float*)(uintptr_t)render_layout.uniform_address,render_layout.uniform_floats};
  coax_v_u32 words={0,0};coax_v_t_RenderGeometryBuffer buffers={0,0};coax_v_t_RenderDrawDomain domains={0,0};
  coax_v_t_RenderUpload uploads={render_layout.uploads.e,sizeof render_layout.uploads.e/sizeof render_layout.uploads.e[0]};
  render_frame=aethyr_render_frame(render_package.package.profile_index,counts,render_limits,COAX_RenderReadiness_ready,0,uploads,words,words,uniform_view,buffers,vertex_view,domains,aethyr_render_geometry_work());
  return render_frame.status;
}
RENDER_EXPORT("coaxial_render_frame_info") int64_t coaxial_render_frame_info(int32_t field){
  switch(field){
    case 0:return render_frame.status;case 1:return render_frame.plan.count;case 2:return render_frame.frame.status;case 3:return render_frame.geometry_status;
    case 4:return render_frame.frame.bad_upload;case 5:return render_frame.frame.bad_draw;case 6:return render_frame.frame.bad_direct;
    case 7:return(int64_t)render_layout.uploads.e[0].bytes;case 8:return(int64_t)render_layout.uploads.e[1].bytes;
    case 9:return(int64_t)render_layout.vertex_address;case 10:return(int64_t)render_layout.uniform_address;
    case 11:return render_frame.counts.hud_vertices;case 12:return render_frame.counts.width;case 13:return render_frame.counts.height;
    case 14:return render_frame.frame.admitted_draws;case 15:return render_geometry_result.status;
    case 16:return(uintptr_t)render_draw_words.data;case 17:return render_draw_words.count*sizeof(uint32_t);
    case 18:return(uintptr_t)render_index_words.data;case 19:return render_index_words.count*sizeof(uint32_t);default:return 0;
  }
}
RENDER_EXPORT("coaxial_render_operation") int64_t coaxial_render_operation(int32_t index,int32_t field){
  coax_t_RenderOp op=operation(index);
  switch(field){
    case 0:return op.kind;case 1:return op.pass;case 2:return op.colour0;case 3:return op.colour1;case 4:return op.depth;
    case 5:return op.keep;case 6:return op.store_depth;case 7:return op.source;case 8:return op.target;case 9:return op.draw_source;
    case 10:return op.first;case 11:return op.count;case 12:return op.instances;case 13:return op.state.depth;case 14:return op.state.depth_write;
    case 15:return op.state.bias;case 16:return op.state.slope;case 17:return op.state.colour;case 18:return op.state.alpha;
    case 19:return op.state.outputs;case 20:return op.state.aux_write;case 21:return op.state.valid;default:return 0;
  }
}
RENDER_EXPORT("coaxial_render_pipeline") int64_t coaxial_render_pipeline(int32_t entry,int32_t field){
  coax_t_RenderHostPipeline pipeline=aethyr_render_pipeline(render_descriptor_profile,entry);
  switch(field){
    case 0:return pipeline.valid;case 1:return pipeline.state.depth;case 2:return pipeline.state.depth_write;case 3:return pipeline.state.bias;
    case 4:return pipeline.state.slope;case 5:return pipeline.state.colour;case 6:return pipeline.state.alpha;case 7:return pipeline.state.outputs;
    case 8:return pipeline.state.aux_write;case 9:return pipeline.colour0;case 10:return pipeline.colour1;case 11:return pipeline.depth;default:return 0;
  }
}
RENDER_EXPORT("coaxial_render_use_count") int32_t coaxial_render_use_count(int32_t entry){return(int32_t)aethyr_render_use_count(render_descriptor_profile,entry);}
RENDER_EXPORT("coaxial_render_group_count") int32_t coaxial_render_group_count(int32_t entry){return(int32_t)aethyr_render_group_count(render_descriptor_profile,entry);}
RENDER_EXPORT("coaxial_render_binding_count") int32_t coaxial_render_binding_count(int32_t set){return(int32_t)aethyr_render_binding_count(render_descriptor_profile,set);}
RENDER_EXPORT("coaxial_render_use") int64_t coaxial_render_use(int32_t entry,int32_t slot,int32_t field){
  coax_t_RenderHostUse use=aethyr_render_use(render_descriptor_profile,entry,slot);
  switch(field){case 0:return use.valid;case 1:return use.binding_set;case 2:return use.use.resource;case 3:return use.use.slot;case 4:return use.use.native_kind;
    case 5:return use.use.shape;case 6:return use.use.stride;case 7:return use.use.stages;case 8:return use.binding.group;case 9:return use.binding.binding;case 10:return use.binding.kind;default:return 0;}
}
RENDER_EXPORT("coaxial_render_binding") int64_t coaxial_render_binding(int32_t set,int32_t resource,int32_t field){
  coax_t_RenderHostBinding binding=aethyr_render_binding(render_descriptor_profile,set,resource);
  switch(field){case 0:return binding.valid;case 1:return binding.binding.kind;case 2:return binding.binding.group;case 3:return binding.binding.binding;
    case 4:return binding.binding.stride;case 5:return binding.binding.uniform_vec4s;case 6:return binding.source.kind;case 7:return binding.source.index;
    case 8:return binding.source.target;case 9:return binding.source.buffer_mode;default:return 0;}
}
RENDER_EXPORT("coaxial_render_buffer_view") uint32_t coaxial_render_buffer_view(int32_t set,int32_t resource,uint64_t source_capacity,uint64_t logical_payload,
    uint64_t initialized_highwater,uint64_t upload_bytes,uint64_t gpu_bytes,uint64_t offset,uint64_t alignment,int32_t zero_initialized){
  coax_t_RenderHostBinding binding=aethyr_render_binding(render_descriptor_profile,set,resource);
  coax_t_RenderHostBool admitted_zero=aethyr_render_bool(zero_initialized);if(!admitted_zero.ok)return COAX_RenderResourceStatus_invalid;
  coax_t_RenderBufferObserved observed={.source_capacity=source_capacity,.logical_payload=logical_payload,.initialized_highwater=initialized_highwater,
    .upload_bytes=upload_bytes,.gpu_bytes=gpu_bytes,.offset=offset,.alignment=alignment,.zero_initialized=admitted_zero.value};
  render_buffer_view=aethyr_render_buffer_view(binding.binding,binding.source.buffer_mode,observed);return render_buffer_view.status;
}
RENDER_EXPORT("coaxial_render_buffer_view_info") uint64_t coaxial_render_buffer_view_info(int32_t field){
  switch(field){case 0:return render_buffer_view.offset;case 1:return render_buffer_view.bytes;case 2:return render_buffer_view.elements;default:return 0;}
}
RENDER_EXPORT("coaxial_render_mesh_placeholder") uint32_t coaxial_render_mesh_placeholder(int32_t set,int32_t resource,uint64_t device_capacity){
  render_buffer_view=aethyr_render_mesh_placeholder(render_descriptor_profile,set,resource,device_capacity);return render_buffer_view.status;
}
RENDER_EXPORT("coaxial_render_buffer_recipe") uint32_t coaxial_render_buffer_recipe(int32_t set,int32_t resource,uint64_t source_capacity,uint64_t logical_payload,uint64_t device_capacity){
  coax_t_RenderHostBinding binding=aethyr_render_binding(render_descriptor_profile,set,resource);
  // Prospective allocation inside the measured device range. Every actual
  // object is admitted again with its real size before binding/submission.
  coax_t_RenderBufferObserved request={.source_capacity=source_capacity,.logical_payload=logical_payload,.gpu_bytes=device_capacity,.alignment=4,.zero_initialized=1};
  render_buffer_view=aethyr_render_buffer_view(binding.binding,binding.source.buffer_mode,request);return render_buffer_view.status;
}
RENDER_EXPORT("coaxial_render_target") uint32_t coaxial_render_target(int32_t target,int32_t width,int32_t height,int32_t coarse,int32_t format,int32_t maximum){
  coax_t_RenderHostBool c=aethyr_render_bool(coarse);if(!c.ok)return COAX_RenderResourceStatus_invalid;render_target=aethyr_render_target_raw(target,width,height,c.value,format,maximum);return render_target.status;
}
RENDER_EXPORT("coaxial_render_target_info") int64_t coaxial_render_target_info(int32_t field){
  switch(field){case 0:return render_target.width;case 1:return render_target.height;case 2:return render_target.format;default:return 0;}
}
RENDER_EXPORT("coaxial_render_clear") float coaxial_render_clear(int32_t field){
  coax_t_RenderHostClear value=aethyr_render_clear();switch(field){case 0:return value.colour.e[0];case 1:return value.colour.e[1];case 2:return value.colour.e[2];case 3:return value.colour.e[3];case 4:return value.depth;default:return 0;}
}
RENDER_EXPORT("coaxial_render_sampler") uint32_t coaxial_render_sampler(int32_t set,int32_t resource){
  coax_t_RenderHostBinding binding=aethyr_render_binding(render_descriptor_profile,set,resource);
  render_sampler=aethyr_render_sampler(binding.source.sampler_name);return render_sampler.status;
}
RENDER_EXPORT("coaxial_render_sampler_info") int64_t coaxial_render_sampler_info(int32_t field){
  switch(field){case 0:return render_sampler.address_u;case 1:return render_sampler.address_v;case 2:return render_sampler.min_linear;case 3:return render_sampler.mag_linear;case 4:return render_sampler.mip_linear;case 5:return render_sampler.physical_anisotropy;case 6:return render_sampler.compare;default:return 0;}
}
RENDER_EXPORT("coaxial_render_texture_layout") uint32_t coaxial_render_texture_layout(int32_t set,int32_t resource){
  coax_t_RenderHostBinding binding=aethyr_render_binding(render_descriptor_profile,set,resource);
  render_texture_layout=aethyr_render_texture_layout(binding.binding);return render_texture_layout.status;
}
RENDER_EXPORT("coaxial_render_texture_layout_info") int64_t coaxial_render_texture_layout_info(int32_t field){return field==0?render_texture_layout.sample:render_texture_layout.dimension;}
RENDER_EXPORT("coaxial_render_texture_shape") uint32_t coaxial_render_texture_shape(int32_t set,int32_t resource,int32_t layers,int32_t depth,int32_t context){
  coax_t_RenderHostBinding binding=aethyr_render_binding(render_descriptor_profile,set,resource);binding.source.index=context;
  uint8_t name_bytes[32];coax_string name=aethyr_render_texture_name(binding.source,(coax_v_u8){name_bytes,sizeof name_bytes});
  coax_t_RenderHostBool admitted_depth=aethyr_render_bool(depth);if(!admitted_depth.ok)return COAX_RenderResourceStatus_invalid;
  coax_t_RenderHostNamedTexture observed={.name=name,.layers=layers,.depth=admitted_depth.value};
  return aethyr_render_resolve_texture(render_descriptor_profile,set,resource,(coax_v_t_RenderHostNamedTexture){&observed,name.count?1:0},context).status;
}
RENDER_EXPORT("coaxial_render_index_recipe") uint32_t coaxial_render_index_recipe(uint64_t bytes,uint64_t device_capacity){
  coax_t_RenderUpload span={.source_bytes=bytes,.bytes=bytes,.gpu_bytes=device_capacity,.alignment=4};return aethyr_render_uploads(render_limits,(coax_v_t_RenderUpload){&span,1}).status;
}
RENDER_EXPORT("coaxial_render_resource_name") uint32_t coaxial_render_resource_name(int32_t set,int32_t resource,int32_t texture){
  coax_t_RenderHostBinding binding=aethyr_render_binding(render_descriptor_profile,set,resource);
  coax_v_u8 scratch={render_name_scratch,sizeof render_name_scratch};
  render_name=texture?aethyr_render_texture_name(binding.source,scratch):aethyr_render_buffer_name(binding.source,scratch);
  return(uint32_t)(uintptr_t)render_name.data;
}
RENDER_EXPORT("coaxial_render_resource_name_bytes") int32_t coaxial_render_resource_name_bytes(void){return(int32_t)render_name.count;}
RENDER_EXPORT("coaxial_render_sans_info") int64_t coaxial_render_sans_info(int32_t field){
  switch(field){case 0:return render_sans.glyphs;case 1:return render_sans.levels;case 2:return render_sans.width;case 3:return render_sans.height;case 4:return render_sans.premultiplied_alpha;default:return 0;}
}
static coax_string sans_text(int32_t field){switch(field){case 0:return render_sans.family;case 1:return render_sans.format;case 2:return render_sans.fill;case 3:return render_sans.text_baseline;default:return(coax_string){0,0};}}
RENDER_EXPORT("coaxial_render_sans_text") uint32_t coaxial_render_sans_text(int32_t field){return(uint32_t)(uintptr_t)sans_text(field).data;}
RENDER_EXPORT("coaxial_render_sans_text_bytes") int32_t coaxial_render_sans_text_bytes(int32_t field){return(int32_t)sans_text(field).count;}
RENDER_EXPORT("coaxial_render_sans_level") uint32_t coaxial_render_sans_level(int32_t level){render_sans_level=aethyr_render_sans_level(level);return render_sans_level.valid;}
RENDER_EXPORT("coaxial_render_sans_level_info") int64_t coaxial_render_sans_level_info(int32_t field){switch(field){case 0:return render_sans_level.width;case 1:return render_sans_level.height;case 2:return render_sans_level.font_px;default:return 0;}}
RENDER_EXPORT("coaxial_render_sans_glyph") uint32_t coaxial_render_sans_glyph(int32_t level,int32_t index){render_sans_glyph=aethyr_render_sans_glyph(level,index);return render_sans_glyph.valid;}
RENDER_EXPORT("coaxial_render_sans_glyph_code") uint32_t coaxial_render_sans_glyph_code(void){return render_sans_glyph.code;}
RENDER_EXPORT("coaxial_render_sans_glyph_position") float coaxial_render_sans_glyph_position(int32_t axis){return axis==0?render_sans_glyph.x:render_sans_glyph.y;}
RENDER_EXPORT("coaxial_render_mip") uint32_t coaxial_render_mip(int32_t width,int32_t height,int32_t layers,int32_t level,int32_t maximum,int32_t max_layers){render_asset_mip=aethyr_render_asset_mip(width,height,layers,level,maximum,max_layers);return render_asset_mip.valid;}
RENDER_EXPORT("coaxial_render_mip_info") int64_t coaxial_render_mip_info(int32_t field){switch(field){case 0:return render_asset_mip.width;case 1:return render_asset_mip.height;case 2:return render_asset_mip.levels;default:return 0;}}
RENDER_EXPORT("coaxial_render_layers_begin") void coaxial_render_layers_begin(void){render_layer_profile=aethyr_render_asset_layers();}
static coax_string layer_text(int32_t field){return field==0?render_layer_profile.format:render_layer_profile.resize_quality;}
RENDER_EXPORT("coaxial_render_layers_text") uint32_t coaxial_render_layers_text(int32_t field){return(uint32_t)(uintptr_t)layer_text(field).data;}
RENDER_EXPORT("coaxial_render_layers_text_bytes") int32_t coaxial_render_layers_text_bytes(int32_t field){return(int32_t)layer_text(field).count;}
RENDER_EXPORT("coaxial_render_layers_premultiplied") uint32_t coaxial_render_layers_premultiplied(void){return render_layer_profile.premultiplied_alpha;}

RENDER_EXPORT("coaxial_render_prepare") uint32_t coaxial_render_prepare(int32_t bytes,int32_t profile){
 int64_t bank=aethyr_render_candidate_bank(package_view());if(bank<0)return COAX_RenderPackageStatus_invalid;
 if(aethyr_render_json_size(bytes)!=COAX_RenderPackageStatus_ok)return COAX_RenderPackageStatus_capacity;
 coax_v_u8 json=aethyr_render_json_storage((coax_v_t_RenderHostJsonBank){&render_json[bank],1});
 coax_v_t_RenderPackageEntry entries=aethyr_render_entry_storage((coax_v_t_RenderHostEntryBank){&render_entries[bank],1});
 render_candidate=aethyr_render_package_prepare(package_view(),(coax_string){(const char*)json.data,bytes},entries,profile,render_device,bank);
 if(render_candidate.status==COAX_RenderPackageStatus_ok)render_descriptor_profile=render_candidate.package.profile_index;return render_candidate.status;
}
RENDER_EXPORT("coaxial_render_candidate_module") uint32_t coaxial_render_candidate_module(int32_t set,int32_t bytes){
 if(!aethyr_render_shader_size(bytes))return COAX_RenderPackageStatus_capacity;
 coax_v_u8 data=shader_view();data.count=bytes;return aethyr_render_package_product(candidate_view(),0,set,data);
}
RENDER_EXPORT("coaxial_render_candidate_pipeline") uint32_t coaxial_render_candidate_pipeline(int32_t entry,int32_t ready){coax_t_RenderHostBool q=aethyr_render_bool(ready);if(!q.ok)return COAX_RenderPackageStatus_invalid;return aethyr_render_package_pipeline(candidate_view(),entry,q.value);}
RENDER_EXPORT("coaxial_render_commit") uint32_t coaxial_render_commit(void){return aethyr_render_package_commit(package_view(),candidate_view());}
RENDER_EXPORT("coaxial_render_abort") void coaxial_render_abort(void){coax_default_t_RenderHostPackageCandidate(&render_candidate);render_descriptor_profile=render_package.package.profile_index;}

static coax_t_RenderHostMediaBank render_media;
static coax_t_RenderHostMediaTextBank render_media_text;
static coax_t_RenderMediaParse render_media_parse;
static coax_t_RenderHostMediaReceipt render_media_result;
static coax_t_RenderMediaEffects render_media_effects;
static coax_t_RenderMediaExtent render_media_extent;
static int64_t render_media_images,render_media_textures;
RENDER_EXPORT("coaxial_render_media_capacity") int32_t coaxial_render_media_capacity(void){return(int32_t)aethyr_render_media_capacity();}
RENDER_EXPORT("coaxial_render_allocation") uint32_t coaxial_render_allocation(uint64_t raw,uint64_t capacity){render_buffer_view=aethyr_render_buffer_bytes(raw,capacity);return render_buffer_view.status;}
RENDER_EXPORT("coaxial_render_media_parse") uint32_t coaxial_render_media_parse(uint32_t address,uint32_t bytes,uint32_t memory_bytes){
 coax_t_RenderHostByteView src=aethyr_render_bytes(address,0,memory_bytes,bytes);if(src.status!=COAX_RenderFrameStatus_ok)return src.status;
 coax_v_u8 text=aethyr_render_media_text_storage((coax_v_t_RenderHostMediaTextBank){&render_media_text,1});
 coax_t_RenderHostByteView dst=aethyr_render_bytes((uintptr_t)text.data,(uintptr_t)text.data,text.count,bytes);if(dst.status!=COAX_RenderFrameStatus_ok)return dst.status;
 if(bytes)memcpy(text.data,(void*)(uintptr_t)src.address,bytes);
 render_media_parse=aethyr_render_media_parse((coax_string){(const char*)text.data,bytes},(coax_v_t_RenderMediaRequest){render_media.requests.e,sizeof render_media.requests.e/sizeof render_media.requests.e[0]});
 render_media_images=0;render_media_textures=0;return render_media_parse.status;
}
RENDER_EXPORT("coaxial_render_media_request") int64_t coaxial_render_media_request(int32_t row,int32_t field){
 if(row<0||row>=render_media_parse.count)return -1;coax_t_RenderMediaRequest r=render_media.requests.e[row];
 switch(field){case 0:return r.kind;case 1:return r.line;case 2:return r.over;case 3:return(uintptr_t)r.url.data;case 4:return r.url.count;default:return -1;}
}
RENDER_EXPORT("coaxial_render_media_count") int32_t coaxial_render_media_count(void){return(int32_t)render_media_parse.count;}
RENDER_EXPORT("coaxial_render_media_observation") uint32_t coaxial_render_media_observation(int32_t slot,int64_t kind,uint32_t url,uint32_t bytes,uint32_t memory_bytes,int64_t ready,int64_t width,int64_t height){
 if(slot<0||(uint64_t)slot>=sizeof render_media.images.e/sizeof render_media.images.e[0])return COAX_RenderHostMediaStatus_capacity;
 coax_t_RenderHostByteView src=aethyr_render_bytes(url,0,memory_bytes,bytes);if(src.status!=COAX_RenderFrameStatus_ok)return COAX_RenderHostMediaStatus_invalid;
 coax_t_RenderHostMediaObserved q=aethyr_render_media_observe(kind,(coax_string){(const char*)(uintptr_t)src.address,bytes},ready,width,height);
 if(q.status!=COAX_RenderHostMediaStatus_ok)return q.status;render_media.images.e[slot]=q.image;render_media_images=slot+1;return q.status;
}
RENDER_EXPORT("coaxial_render_media_texture") uint32_t coaxial_render_media_texture(int32_t slot,int32_t line,int64_t width,int64_t height,int64_t layers,int32_t depth){
 if(slot<0||(uint64_t)slot>=sizeof render_media.textures.e/sizeof render_media.textures.e[0])return COAX_RenderHostMediaStatus_capacity;
 coax_t_RenderHostBinding b=aethyr_render_binding(render_package.package.profile_index,2,1);b.source.index=line;
 coax_string name=aethyr_render_texture_name(b.source,(coax_v_u8){render_media.names.e[slot].e,sizeof render_media.names.e[slot].e});
 coax_t_RenderHostBool admitted_depth=aethyr_render_bool(depth);if(!admitted_depth.ok)return COAX_RenderHostMediaStatus_invalid;
 render_media.textures.e[slot]=(coax_t_RenderHostMediaTexture){.name=name,.width=width,.height=height,.layers=layers,.depth=admitted_depth.value};render_media_textures=slot+1;return COAX_RenderHostMediaStatus_ok;
}
RENDER_EXPORT("coaxial_render_media_compose") uint32_t coaxial_render_media_compose(int32_t width,int32_t height){
 render_media_result=aethyr_render_media_draws(render_package.package.profile_index,(coax_v_t_RenderMediaRequest){render_media.requests.e,render_media_parse.count},
 (coax_v_t_RenderHostMediaImage){render_media.images.e,render_media_images},width,height,(coax_v_t_RenderHostMediaTexture){render_media.textures.e,render_media_textures},
 (coax_v_t_RenderMediaRect){render_media.rects.e,sizeof render_media.rects.e/sizeof render_media.rects.e[0]});return render_media_result.status;
}
RENDER_EXPORT("coaxial_render_media_rect") int64_t coaxial_render_media_rect(int32_t row,int32_t field){
 if(row<0||row>=render_media_result.count)return -1;coax_t_RenderMediaRect *r=&render_media.rects.e[row];
 switch(field){case 0:return r->draw;case 1:return r->line;case 2:return r->over;case 3:return(uintptr_t)r->uniforms.e;case 4:return sizeof r->uniforms.e;default:return -1;}
}
RENDER_EXPORT("coaxial_render_media_vertices") int32_t coaxial_render_media_vertices(void){return(int32_t)aethyr_render_media_vertices();}
RENDER_EXPORT("coaxial_render_media_extent") uint32_t coaxial_render_media_extent(int64_t kind,int64_t width,int64_t height){
 coax_t_RenderHostMediaObserved observed=aethyr_render_media_observe(kind,(coax_string){"/extent",7},0,width,height);if(observed.status!=COAX_RenderHostMediaStatus_ok)return 0;
 render_media_extent=aethyr_render_media_extent(observed.image.kind,width,height);return render_media_extent.ok;
}
RENDER_EXPORT("coaxial_render_media_extent_info") int64_t coaxial_render_media_extent_info(int32_t field){return field==0?render_media_extent.width:render_media_extent.height;}
RENDER_EXPORT("coaxial_render_media_effects") int64_t coaxial_render_media_effects(int32_t field){
 render_media_effects=aethyr_render_media_effects();switch(field){case 0:return(uintptr_t)render_media_effects.format.data;case 1:return render_media_effects.format.count;case 2:return render_media_effects.premultiplied_alpha;case 3:return(uintptr_t)render_media_effects.resize_quality.data;case 4:return render_media_effects.resize_quality.count;default:return -1;}
}
RENDER_EXPORT("coaxial_render_media_text_ptr") uint32_t coaxial_render_media_text_ptr(void){return(uint32_t)(uintptr_t)render_media_text.bytes.e;}
RENDER_EXPORT("coaxial_render_media_texture_name") int64_t coaxial_render_media_texture_name(int32_t slot,int32_t field){if(slot<0||slot>=render_media_textures)return -1;coax_string name=render_media.textures.e[slot].name;return field==0?(uintptr_t)name.data:name.count;}
RENDER_EXPORT("coaxial_render_active_bank") int32_t coaxial_render_active_bank(void){return(int32_t)render_package.active_bank;}

RENDER_EXPORT("coaxial_render_asset_span") uint32_t coaxial_render_asset_span(uint32_t address,uint32_t capacity,uint32_t bytes,uint32_t memory_bytes){return aethyr_render_asset_span(address,capacity,bytes,memory_bytes).status;}
