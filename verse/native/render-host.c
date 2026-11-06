// Effect interpreter only. Coax owns package/byte admission, graph topology,
// target/sampler/resource descriptions, pipeline states and decoded draws.
#include "render-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {VksPipe pipe;VksPass pass;VksBinding bindings[32];int open;} RhgDraw;
static coax_t_RenderHostRegistryBank rhg_ledger_bank;static int rhg_ledger_opened;
static coax_v_t_RenderRegistry rhg_ledger(void){
  coax_v_t_RenderRegistry v=aethyr_render_registry_storage((coax_v_t_RenderHostRegistryBank){&rhg_ledger_bank,1});
  if(!rhg_ledger_opened){aethyr_render_registry_open(v);rhg_ledger_opened=1;}return v;
}
int rhg_ledger_acquire(int kind,int64_t words,RhgHandle *out){
  coax_t_RrGrant g=aethyr_render_registry_acquire(rhg_ledger(),aethyr_render_resource_kind(kind),words);
  if(g.status!=COAX_RrStatus_ok)return 0;out->slot=g.handle.slot;out->generation=g.handle.generation;return 1;
}
void rhg_ledger_release(RhgHandle *h){if(h->generation>0)aethyr_render_registry_release(rhg_ledger(),h->slot,h->generation);*h=(RhgHandle){0};}
int rhg_ledger_live(const RhgHandle *h){return h->generation>0&&aethyr_render_registry_live(rhg_ledger(),h->slot,h->generation);}
int64_t rhg_ledger_info(int field){
  coax_v_t_RenderRegistry v=rhg_ledger();
  switch(field){case 0:return v.data[0].epoch;case 1:return v.data[0].live;case 2:return v.data[0].words;case 3:return v.data[0].limit.slots;case 4:return v.data[0].limit.words;default:return -1;}
}
struct RhgRenderer {
  Vkc *v;int64_t profile;char *json;
  coax_t_RenderHostPackageState state;coax_t_RenderHostEntryBank entries;
  RhgBytes *shaders;size_t shader_count;
  VksImage targets[COAX_RenderTarget_invalid];int target_open[COAX_RenderTarget_invalid];RhgHandle target_handles[COAX_RenderTarget_invalid];
  VksSurface *surfaces;int *surface_open;RhgDraw *draws;int64_t op_count;
  const coax_t_RenderRuntimeFrame *receipt;const RhgFrame *frame;
  coax_t_RenderDecodedDraw *decoded;uint64_t draw_commands;
  RhgDraw *media_draws;int64_t media_count;
  VksImage presentation;RhgHandle presentation_handle;int presentation_open;
};
static int rhg_error(char *error,size_t size,const char *text){if(size)snprintf(error,size,"%s",text);return 0;}
static VkFormat rhg_format(coax_t_RenderTextureFormat format){
  switch(format){case COAX_RenderTextureFormat_rgba8:return VK_FORMAT_R8G8B8A8_UNORM;case COAX_RenderTextureFormat_bgra8:return VK_FORMAT_B8G8R8A8_UNORM;case COAX_RenderTextureFormat_rgba16f:return VK_FORMAT_R16G16B16A16_SFLOAT;case COAX_RenderTextureFormat_depth32:return VK_FORMAT_D32_SFLOAT;default:return VK_FORMAT_UNDEFINED;}
}
static int rhg_blend(coax_t_RenderBlend blend){
  switch(blend){case COAX_RenderBlend_replace:return VKS_REPLACE;case COAX_RenderBlend_maximum:return VKS_MAX;case COAX_RenderBlend_multiply:return VKS_MUL;case COAX_RenderBlend_add:return VKS_ADD;case COAX_RenderBlend_keep:return VKS_KEEP;case COAX_RenderBlend_over:return VKS_OVER;case COAX_RenderBlend_under:return VKS_UNDER;}return VKS_REPLACE;
}
static void rhg_frame_close(RhgRenderer *r){
  if(r->draws)for(int64_t i=0;i<r->op_count;i++)if(r->draws[i].open)vks_pipe_close(r->v,&r->draws[i].pipe);
  if(r->surfaces)for(int64_t i=0;i<r->op_count;i++)if(r->surface_open[i])vks_surface_close(r->v,&r->surfaces[i]);
  if(r->media_draws)for(int64_t i=0;i<r->media_count;i++)if(r->media_draws[i].open)vks_pipe_close(r->v,&r->media_draws[i].pipe);
  free(r->draws);free(r->surfaces);free(r->surface_open);free(r->decoded);free(r->media_draws);
  r->draws=NULL;r->surfaces=NULL;r->surface_open=NULL;r->decoded=NULL;r->op_count=0;
  r->media_draws=NULL;r->media_count=0;
}
void rhg_close(RhgRenderer *r){
  if(!r)return;rhg_frame_close(r);
  for(size_t i=0;i<COAX_RenderTarget_invalid;i++)if(r->target_open[i]){vks_image_free(r->v,&r->targets[i]);rhg_ledger_release(&r->target_handles[i]);}
  if(r->presentation_open){vks_image_free(r->v,&r->presentation);rhg_ledger_release(&r->presentation_handle);}
  if(r->shaders)for(size_t i=0;i<r->shader_count;i++)free((void*)r->shaders[i].data);
  free(r->shaders);free(r->json);free(r);
}
RhgRenderer *rhg_open(Vkc *v,int64_t profile,coax_string json,RhgRead read,void *user,char *error,size_t error_bytes){
  if(aethyr_render_json_size(json.count)!=COAX_RenderPackageStatus_ok){rhg_error(error,error_bytes,"package JSON size refused by Coax");return NULL;}
  RhgRenderer *r=calloc(1,sizeof*r);if(!r)return NULL;r->v=v;r->profile=profile;
  r->json=malloc((size_t)json.count+1);if(!r->json){rhg_close(r);return NULL;}memcpy(r->json,json.data,(size_t)json.count);r->json[json.count]=0;
  coax_default_t_RenderHostPackageState(&r->state);
  coax_v_t_RenderHostPackageState state={&r->state,1};coax_v_t_RenderHostEntryBank bank={&r->entries,1};
  coax_t_RenderDeviceCapabilities device=aethyr_render_device(COAX_RenderBackend_vulkan,v->demote_helper_enabled!=0,v->shader_int64_enabled!=0);
  coax_t_RenderPackageStatus status=aethyr_render_package_load(state,(coax_string){r->json,json.count},aethyr_render_entry_storage(bank),profile,device,aethyr_render_candidate_bank(state));
  if(status!=COAX_RenderPackageStatus_ok){rhg_error(error,error_bytes,"package identity/capability refused by Coax");rhg_close(r);return NULL;}
  r->shader_count=(size_t)r->state.package.entries.count;r->shaders=calloc(r->shader_count,sizeof*r->shaders);if(!r->shaders){rhg_close(r);return NULL;}
  // Authenticate every required entry before creating any GPU module.
  for(size_t i=0;i<r->shader_count;i++){
    coax_t_RenderPackageEntry entry=r->state.package.entries.data[i];if(!entry.present)continue;
    RhgBytes bytes=read(user,entry.spirv_file);
    if(!bytes.data||!aethyr_render_shader_size((int64_t)bytes.bytes)||aethyr_render_spirv_bytes(r->state.package,(int64_t)i,(coax_v_u8){(uint8_t*)bytes.data,(int64_t)bytes.bytes})!=COAX_RenderPackageStatus_ok){rhg_error(error,error_bytes,"actual SPIR-V bytes refused by Coax");rhg_close(r);return NULL;}
    unsigned char *owned=malloc(bytes.bytes);if(!owned){rhg_close(r);return NULL;}memcpy(owned,bytes.data,bytes.bytes);r->shaders[i]=(RhgBytes){owned,bytes.bytes};
  }
  return r;
}
static int rhg_target(RhgRenderer *r,coax_t_RenderTarget id,const coax_t_FrameCounts *counts,int coarse,coax_t_RenderTextureFormat screen,char *error,size_t size){
  if(id==COAX_RenderTarget_none)return 1;
  // Core intersects the actual device limit with its supported profile.
  coax_t_RenderTargetDescriptor desc=aethyr_render_target_raw(id,counts->width,counts->height,coarse!=0,screen,r->v->props.limits.maxImageDimension2D);
  if(desc.status!=COAX_RenderResourceStatus_ok)return rhg_error(error,size,"target extent refused by Coax");
  VkFormat format=rhg_format(desc.format);
  if(r->target_open[id]&&(r->targets[id].w!=(uint32_t)desc.width||r->targets[id].h!=(uint32_t)desc.height||r->targets[id].format!=format)){vks_image_free(r->v,&r->targets[id]);rhg_ledger_release(&r->target_handles[id]);r->target_open[id]=0;}
  if(!r->target_open[id]){
    int64_t words=aethyr_render_format_words(desc.format,desc.width,desc.height,1,1);
    if(!rhg_ledger_acquire(desc.format==COAX_RenderTextureFormat_depth32?RHG_TEXTURE:RHG_COLOR,words,&r->target_handles[id]))return rhg_error(error,size,"target allocation refused by the Coax ledger");
    r->targets[id]=vks_image(r->v,(uint32_t)desc.width,(uint32_t)desc.height,1,0,format,VKS_TARGET);r->target_open[id]=1;}
  // Every attachment a pass uses is live in the ledger before its candidate is prepared.
  if(!rhg_ledger_live(&r->target_handles[id]))return rhg_error(error,size,"target is not live in the Coax ledger");
  return 1;
}
static VksPass rhg_attachments(RhgRenderer *r,coax_t_RenderOp op){
  VksPass pass={0};if(op.colour0!=COAX_RenderTarget_none)pass.targets[pass.ntargets++]=&r->targets[op.colour0];
  if(op.colour1!=COAX_RenderTarget_none)pass.targets[pass.ntargets++]=&r->targets[op.colour1];
  if(op.depth!=COAX_RenderTarget_none)pass.depth=&r->targets[op.depth];pass.keep=op.keep;return pass;
}
static int rhg_draw_prepare(RhgRenderer *r,RhgDraw *draw,int64_t entry,coax_t_RenderOp active,coax_t_RenderOp op,const coax_t_RenderMediaRect *media,char *error,size_t size){
  coax_t_RenderHostPipeline pipeline=aethyr_render_pipeline(r->profile,entry);
    if(!pipeline.valid)return rhg_error(error,size,"canonical pipeline descriptor absent");
    draw->pass=rhg_attachments(r,active);VksPass *pass=&draw->pass;coax_t_RenderPassState state=pipeline.state;
    pass->words=(const uint32_t*)r->shaders[entry].data;pass->count=r->shaders[entry].bytes/4;
    pass->bindings=draw->bindings;pass->nb=(uint32_t)aethyr_render_use_count(r->profile,entry);
    pass->blend=rhg_blend(state.colour);pass->alpha_blend=rhg_blend(state.alpha);pass->masked=state.aux_write?0:2;
    pass->depth_test=state.depth!=COAX_RenderDepth_none;pass->depth_always=state.depth==COAX_RenderDepth_always;pass->depth_write=state.depth_write;pass->depth_bias=(float)state.bias;pass->depth_slope=(float)state.slope;
    coax_t_RenderPackageEntry reflected=r->state.package.entries.data[entry];pass->vertex_only=reflected.fragment.count==0;
    pass->stride=(uint32_t)reflected.reflection.vertex_stride;pass->nattrs=(uint32_t)reflected.reflection.attribute_count;
    for(uint32_t a=0;a<pass->nattrs;a++){coax_t_RenderVertexInput attr=reflected.reflection.attributes.e[a];pass->attrs[a]=(VkVertexInputAttributeDescription){.location=(uint32_t)attr.location,.binding=0,.format=VK_FORMAT_R32G32B32A32_SFLOAT,.offset=(uint32_t)attr.offset};}
    pass->vertices=r->frame->hud;pass->index=r->frame->index;
    for(uint32_t slot=0;slot<pass->nb;slot++){
      coax_t_RenderHostUse use=aethyr_render_use(r->profile,entry,slot);
      if(!r->frame->binding||!r->frame->binding(r->frame->user,use,op,media,&draw->bindings[slot]))return rhg_error(error,size,"physical binding observation unavailable");
      draw->bindings[slot].kind=(int)use.use.native_kind;
      draw->bindings[slot].stages=((use.use.stages&1)?VK_SHADER_STAGE_VERTEX_BIT:0)|((use.use.stages&2)?VK_SHADER_STAGE_FRAGMENT_BIT:0);
    }
    vks_pipe_open(r->v,&draw->pipe,pass);draw->open=1;
  return 1;
}
static int rhg_prepare(RhgRenderer *r,int coarse,coax_t_RenderTextureFormat screen,char *error,size_t size){
  const coax_t_RenderPlan *plan=&r->receipt->plan;
  r->op_count=plan->count;r->surfaces=calloc((size_t)plan->count,sizeof*r->surfaces);r->surface_open=calloc((size_t)plan->count,sizeof*r->surface_open);r->draws=calloc((size_t)plan->count,sizeof*r->draws);
  if(!r->surfaces||!r->surface_open||!r->draws)return rhg_error(error,size,"native frame allocation failed");
  for(int64_t i=0;i<plan->count;i++){
    coax_t_RenderOp op=plan->ops.e[i];
    if(op.kind==COAX_RenderOpKind_begin){if(!rhg_target(r,op.colour0,&r->receipt->counts,coarse,screen,error,size)||!rhg_target(r,op.colour1,&r->receipt->counts,coarse,screen,error,size)||!rhg_target(r,op.depth,&r->receipt->counts,coarse,screen,error,size))return 0;}
    if(op.kind==COAX_RenderOpKind_copy){if(!rhg_target(r,op.source,&r->receipt->counts,coarse,screen,error,size)||!rhg_target(r,op.target,&r->receipt->counts,coarse,screen,error,size))return 0;}
  }
  r->media_count=r->frame->media_receipt.count;r->media_draws=calloc(r->media_count?(size_t)r->media_count:1,sizeof*r->media_draws);if(!r->media_draws)return rhg_error(error,size,"media pipeline allocation failed");
  coax_t_RenderOp active={0};
  for(int64_t i=0;i<plan->count;i++){
    coax_t_RenderOp op=plan->ops.e[i];
    if(op.kind==COAX_RenderOpKind_begin){active=op;VksPass attachments=rhg_attachments(r,op);vks_surface_open(r->v,&r->surfaces[i],&attachments,op.store_depth);r->surface_open[i]=1;}
    if(op.kind==COAX_RenderOpKind_media_before){
      for(int64_t media=0;media<r->media_count;media++)if(r->frame->media_rects.data[media].draw){
        if(!rhg_draw_prepare(r,&r->media_draws[media],13,active,op,&r->frame->media_rects.data[media],error,size))return 0;
      }
    }
    if(op.kind!=COAX_RenderOpKind_draw)continue;
    if(!rhg_draw_prepare(r,&r->draws[i],(int64_t)op.pass,active,op,NULL,error,size))return 0;
  }
  return 1;
}
static void rhg_record_draw(RhgRenderer *r,VkCommandBuffer cmd,coax_t_RenderDecodedDraw draw){
  if(draw.kind==COAX_RenderDrawKind_nonindexed){vkCmdDraw(cmd,draw.count,draw.instances,draw.first,draw.base_instance);r->draw_commands++;}
  if(draw.kind==COAX_RenderDrawKind_indexed){vkCmdDrawIndexed(cmd,draw.count,draw.instances,draw.first,0,draw.base_instance);r->draw_commands++;}
}
static void rhg_record(VkCommandBuffer cmd,void *user){
  RhgRenderer *r=user;const coax_t_RenderPlan *plan=&r->receipt->plan;int64_t loop_start=0,current=0,loop_end=0;
  coax_t_RenderHostClear clear=aethyr_render_clear();
  for(int64_t i=0;i<plan->count;i++){
    coax_t_RenderOp op=plan->ops.e[i];
    switch(op.kind){
      case COAX_RenderOpKind_begin:vks_surface_begin(&r->surfaces[i],cmd,clear.colour.e,clear.depth);break;
      case COAX_RenderOpKind_end:vks_surface_end(cmd);break;
      case COAX_RenderOpKind_copy:vks_copy_record(cmd,&r->targets[op.source],&r->targets[op.target]);break;
      case COAX_RenderOpKind_each_begin:loop_start=i;current=op.first;loop_end=op.first+op.count;break;
      case COAX_RenderOpKind_each_end:current++;if(current<loop_end)i=loop_start;break;
      case COAX_RenderOpKind_draw:
        vks_pipe_bind(&r->draws[i].pipe,cmd,&r->draws[i].pass);
        if(op.draw_source==COAX_RenderDrawSource_vertices){vkCmdDraw(cmd,(uint32_t)op.count,(uint32_t)op.instances,(uint32_t)op.first,0);r->draw_commands++;}
        else if(op.draw_source==COAX_RenderDrawSource_current)rhg_record_draw(r,cmd,r->decoded[current]);
        else for(int64_t k=0;k<op.count;k++)rhg_record_draw(r,cmd,r->decoded[op.first+k]);
        break;
      case COAX_RenderOpKind_media_before:case COAX_RenderOpKind_media_after:
        for(int64_t media=0;media<r->media_count;media++){
          coax_t_RenderMediaRect rect=r->frame->media_rects.data[media];
          if(rect.draw&&rect.over==(op.kind==COAX_RenderOpKind_media_after)){
            vks_pipe_bind(&r->media_draws[media].pipe,cmd,&r->media_draws[media].pass);vkCmdDraw(cmd,(uint32_t)aethyr_render_media_vertices(),1,0,0);r->draw_commands++;
          }
        }break;
    }
  }
}
int rhg_execute(RhgRenderer *r,const coax_t_RenderRuntimeFrame *receipt,const RhgFrame *frame,int coarse,coax_t_RenderTextureFormat screen,char *error,size_t size){
  if(frame->media_receipt.status!=COAX_RenderHostMediaStatus_ok||frame->media_receipt.count<0||frame->media_receipt.count>frame->media_rects.count)return rhg_error(error,size,"Coax media candidate refused before Vulkan effects");
  if(receipt->status!=COAX_RenderRuntimeStatus_ok)return rhg_error(error,size,"Coax frame refused before Vulkan effects");
  int errors_before=vkc_validation_errors,warnings_before=vkc_validation_warnings;
  // Each frame is a new ledger epoch; an object released before it is not live in it.
  if(!aethyr_render_registry_advance(rhg_ledger()))return rhg_error(error,size,"the Coax ledger's epochs are spent");
  rhg_frame_close(r);r->receipt=receipt;r->frame=frame;r->draw_commands=0;
  r->decoded=calloc(receipt->counts.draw_count?(size_t)receipt->counts.draw_count:1,sizeof*r->decoded);if(!r->decoded)return rhg_error(error,size,"decoded draw allocation failed");
  for(int64_t i=0;i<receipt->counts.draw_count;i++){r->decoded[i]=aethyr_render_decode_draw(frame->draw_words,i);if(r->decoded[i].kind==COAX_RenderDrawKind_invalid)return rhg_error(error,size,"validated draw transport changed");}
  if(!rhg_prepare(r,coarse,screen,error,size)){rhg_frame_close(r);return 0;}
  if(vkc_validation_errors!=errors_before||vkc_validation_warnings!=warnings_before){rhg_frame_close(r);return rhg_error(error,size,"Vulkan validation refused candidate preparation");}
  vks_submit(r->v,rhg_record,r);
  // Submission can already have issued effects. Report its diagnostics as
  // failure so the producer's retained upload prefixes are not acknowledged.
  if(vkc_validation_errors!=errors_before||vkc_validation_warnings!=warnings_before)return rhg_error(error,size,"Vulkan validation refused submitted candidate");
  return 1;
}
VksImage *rhg_screen(RhgRenderer *r){return r->target_open[COAX_RenderTarget_screen]?&r->targets[COAX_RenderTarget_screen]:NULL;}
typedef struct {VksPipe pipe;VksPass pass;} RhgPresentationEffects;
static void rhg_record_presentation(VkCommandBuffer cmd,void *user){RhgPresentationEffects *effects=user;vks_pipe_record(&effects->pipe,cmd,&effects->pass);}
int rhg_prepare_presentation(RhgRenderer *r,coax_t_RenderPresentation p,char *error,size_t size){
  if(p.status!=COAX_RenderResourceStatus_ok)return rhg_error(error,size,"presentation refused by Coax");
  if(r->presentation_open&&(r->presentation.w!=p.display.width||r->presentation.h!=p.display.height||r->presentation.format!=rhg_format(p.display.format))){
    vks_image_free(r->v,&r->presentation);rhg_ledger_release(&r->presentation_handle);r->presentation_open=0;
  }
  if(!r->presentation_open){
    int64_t words=aethyr_render_format_words(p.display.format,p.display.width,p.display.height,1,1);
    if(!rhg_ledger_acquire(RHG_COLOR,words,&r->presentation_handle))return rhg_error(error,size,"presentation allocation refused by ledger");
    r->presentation=vks_image(r->v,(uint32_t)p.display.width,(uint32_t)p.display.height,1,0,rhg_format(p.display.format),VKS_TARGET);r->presentation_open=1;
  }
  return 1;
}
VksImage *rhg_present_plan(RhgRenderer *r,coax_t_RenderPresentation p,char *error,size_t size){
  if(!rhg_prepare_presentation(r,p,error,size))return NULL;
  VksImage *source=rhg_target_image(r,p.source);
  if(!source||!rhg_ledger_live(&r->target_handles[p.source])||source->format!=rhg_format(p.draw.format)||source->w!=p.draw.width||source->h!=p.draw.height){rhg_error(error,size,"actual presentation source differs from Coax descriptor");return NULL;}
  RhgHandle uniform_handle={0};size_t bytes=sizeof p.uniforms.e;
  coax_t_RenderBufferObserved observed={.source_capacity=bytes,.upload_bytes=bytes,.gpu_bytes=bytes,.alignment=4};
  coax_t_RenderBufferView view=aethyr_render_buffer_view(p.uniform.binding,p.uniform.source.buffer_mode,observed);
  if(view.status!=COAX_RenderResourceStatus_ok||!rhg_ledger_acquire(RHG_UNIFORM,aethyr_render_buffer_words((int64_t)bytes),&uniform_handle)){rhg_error(error,size,"presentation uniform refused by Coax");return NULL;}
  VkcBuffer uniform=vkc_buffer(r->v,bytes);memcpy(uniform.map,p.uniforms.e,bytes);
  coax_t_RenderSamplerDescriptor s=p.sampler;
  VkSamplerCreateInfo sampler_info={.sType=VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,.magFilter=s.mag_linear?VK_FILTER_LINEAR:VK_FILTER_NEAREST,.minFilter=s.min_linear?VK_FILTER_LINEAR:VK_FILTER_NEAREST,
    .mipmapMode=s.mip_linear?VK_SAMPLER_MIPMAP_MODE_LINEAR:VK_SAMPLER_MIPMAP_MODE_NEAREST,
    .addressModeU=s.address_u==COAX_RenderSamplerAddress_repeat?VK_SAMPLER_ADDRESS_MODE_REPEAT:VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
    .addressModeV=s.address_v==COAX_RenderSamplerAddress_repeat?VK_SAMPLER_ADDRESS_MODE_REPEAT:VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,.addressModeW=VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
    .anisotropyEnable=s.physical_anisotropy>1,.maxAnisotropy=(float)s.physical_anisotropy,.compareEnable=s.compare!=COAX_RenderSamplerCompare_none,.compareOp=VK_COMPARE_OP_LESS,.maxLod=VK_LOD_CLAMP_NONE};
  VkSampler sampler;VkResult created=vkCreateSampler(r->v->dev,&sampler_info,NULL,&sampler);
  if(created!=VK_SUCCESS){vkc_free(r->v,&uniform);rhg_ledger_release(&uniform_handle);rhg_error(error,size,"presentation sampler SDK failure");return NULL;}
  VksBinding bindings[3]={0};coax_t_RenderHostUse uses[3]={p.uniform,p.image,p.sample};
  bindings[p.uniform.use.slot]=(VksBinding){.buffer=&uniform,.offset=view.offset,.range=view.bytes};
  bindings[p.image.use.slot]=(VksBinding){.image=source};bindings[p.sample.use.slot]=(VksBinding){.sampler=sampler};
  for(size_t i=0;i<3;i++){bindings[uses[i].use.slot].kind=(int)uses[i].use.native_kind;bindings[uses[i].use.slot].stages=((uses[i].use.stages&1)?VK_SHADER_STAGE_VERTEX_BIT:0)|((uses[i].use.stages&2)?VK_SHADER_STAGE_FRAGMENT_BIT:0);}
  RhgPresentationEffects effects={0};effects.pass=(VksPass){.words=(const uint32_t*)r->shaders[p.entry].data,.count=r->shaders[p.entry].bytes/4,.bindings=bindings,.nb=3,
    .targets={&r->presentation},.ntargets=(uint32_t)p.state.outputs,.vertex_count=(uint32_t)p.vertices,.instance_count=1,
    .blend=rhg_blend(p.state.colour),.alpha_blend=rhg_blend(p.state.alpha),.depth_test=p.state.depth!=COAX_RenderDepth_none,.depth_always=p.state.depth==COAX_RenderDepth_always,.depth_write=p.state.depth_write,
    .depth_bias=(float)p.state.bias,.depth_slope=(float)p.state.slope};
  coax_t_RenderHostClear clear=aethyr_render_clear();memcpy(effects.pass.clear,clear.colour.e,sizeof effects.pass.clear);
  int errors=vkc_validation_errors,warnings=vkc_validation_warnings;
  vks_pipe_open(r->v,&effects.pipe,&effects.pass);
  int valid=rhg_ledger_live(&uniform_handle)&&rhg_ledger_live(&r->presentation_handle)&&rhg_ledger_live(&r->target_handles[p.source])&&vkc_validation_errors==errors&&vkc_validation_warnings==warnings;
  if(valid){vks_submit(r->v,rhg_record_presentation,&effects);r->draw_commands++;valid=vkc_validation_errors==errors&&vkc_validation_warnings==warnings;}
  vks_pipe_close(r->v,&effects.pipe);vkDestroySampler(r->v->dev,sampler,NULL);vkc_free(r->v,&uniform);rhg_ledger_release(&uniform_handle);
  if(!valid){rhg_error(error,size,"presentation validation or physical resource check failed");return NULL;}
  return &r->presentation;
}
VksImage *rhg_present_float(RhgRenderer *r,int format,char *error,size_t size){
  VksImage *screen=rhg_screen(r);if(!screen){rhg_error(error,size,"presentation has no completed screen");return NULL;}
  return rhg_present_plan(r,render_presentation(r->profile,screen->w,screen->h,format,r->v->props.limits.maxImageDimension2D),error,size);
}
VksImage *rhg_present_quantized_v1(RhgRenderer *r,int format,char *error,size_t size){
  VksImage *screen=rhg_screen(r);if(!screen){rhg_error(error,size,"presentation has no completed screen");return NULL;}
  return rhg_present_plan(r,render_presentation_quantized_v1(r->profile,screen->w,screen->h,format,r->v->props.limits.maxImageDimension2D),error,size);
}
VksImage *rhg_probe_presentation(RhgRenderer *r,int64_t width,int64_t height,const void *bytes,size_t count,int format,int quantized,char *error,size_t size){
  coax_t_RenderPresentation p=quantized?render_presentation_quantized_v1(r->profile,width,height,format,r->v->props.limits.maxImageDimension2D):render_presentation(r->profile,width,height,format,r->v->props.limits.maxImageDimension2D);
  if(p.status!=COAX_RenderResourceStatus_ok||!bytes||count!=(uint64_t)p.draw.width*p.draw.height*8){rhg_error(error,size,"probe source does not cover admitted FP16 extent");return NULL;}
  coax_t_FrameCounts counts={.width=p.draw.width,.height=p.draw.height};
  if(!rhg_target(r,p.source,&counts,0,p.draw.format,error,size))return NULL;
  vks_upload(r->v,&r->targets[p.source],bytes,count);
  return rhg_present_plan(r,p,error,size);
}
VksImage *rhg_target_image(RhgRenderer *r,coax_t_RenderTarget target){return target<COAX_RenderTarget_invalid&&r->target_open[target]?&r->targets[target]:NULL;}
uint64_t rhg_draw_commands(const RhgRenderer *r){return r->draw_commands;}
