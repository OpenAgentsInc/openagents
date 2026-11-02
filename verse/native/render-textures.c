#include "render-textures.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct {coax_t_RFTextureNeed need;unsigned char *packed;size_t bytes;VksImage image;int open,array;} RhgtImage;
struct RhgTextures {Vkc *device;const RfiFrame *raw;RhgtImage *images;coax_t_RenderHostNamedTexture *names;};
static void rhgt_error(char *error,size_t bytes,const char *message){if(bytes)snprintf(error,bytes,"%s",message);}
static VkFormat rhgt_format(coax_t_RenderTexelFormat f){
  switch(f){case COAX_RenderTexelFormat_rgba8:return VK_FORMAT_R8G8B8A8_UNORM;case COAX_RenderTexelFormat_rgba8_srgb:return VK_FORMAT_R8G8B8A8_SRGB;
    case COAX_RenderTexelFormat_bgra8:return VK_FORMAT_B8G8R8A8_UNORM;case COAX_RenderTexelFormat_bgra8_srgb:return VK_FORMAT_B8G8R8A8_SRGB;
    case COAX_RenderTexelFormat_rgba16f:return VK_FORMAT_R16G16B16A16_SFLOAT;case COAX_RenderTexelFormat_rgba32f:return VK_FORMAT_R32G32B32A32_SFLOAT;
    case COAX_RenderTexelFormat_depth32f:return VK_FORMAT_D32_SFLOAT;default:return VK_FORMAT_UNDEFINED;}
}
coax_v_t_RenderHostNamedTexture rhgt_names(RhgTextures *t){return(coax_v_t_RenderHostNamedTexture){t->names,(int64_t)t->raw->texture_count};}
int rhgt_frame_admit(RhgTextures *t,int64_t profile,const coax_t_RenderRuntimeFrame *frame,int coarse,coax_t_RenderTextureFormat screen,char *error,size_t bytes){
  coax_t_RenderHostBool c=aethyr_render_bool(coarse);if(!c.ok){rhgt_error(error,bytes,"raw coarse flag refused by Coax");return 0;}
  for(int64_t i=0;i<frame->plan.count;i++){
    coax_t_RenderOp op=frame->plan.ops.e[i];coax_t_RenderTarget targets[3];size_t n=0;
    if(op.kind==COAX_RenderOpKind_begin){targets[n++]=op.colour0;targets[n++]=op.colour1;targets[n++]=op.depth;}
    if(op.kind==COAX_RenderOpKind_copy){targets[n++]=op.source;targets[n++]=op.target;}
    for(size_t j=0;j<n;j++)if(targets[j]!=COAX_RenderTarget_none){
      coax_t_RenderTargetDescriptor target=aethyr_render_target_raw(targets[j],frame->counts.width,frame->counts.height,c.value,screen,t->device->props.limits.maxImageDimension2D);
      if(target.status!=COAX_RenderResourceStatus_ok){rhgt_error(error,bytes,"active target extent refused by Coax before payload writes");return 0;}
    }
    if(op.kind!=COAX_RenderOpKind_draw)continue;
    for(int64_t slot=0;slot<aethyr_render_use_count(profile,op.pass);slot++){
      coax_t_RenderHostUse use=aethyr_render_use(profile,op.pass,slot);coax_t_RenderResourceKind kind=use.binding.kind;
      if(kind!=COAX_RenderResourceKind_image2d&&kind!=COAX_RenderResourceKind_image_array&&kind!=COAX_RenderResourceKind_depth)continue;
      coax_t_RenderHostTextureReceipt view=aethyr_render_resolve_texture(profile,use.binding_set,use.use.resource,rhgt_names(t),-1);
      if(view.status!=COAX_RenderResourceStatus_ok){rhgt_error(error,bytes,"active texture observation refused by Coax before payload writes");return 0;}
      if(view.named_index>=0){
        const RfiTexture *source=&t->raw->textures[view.named_index];RhgtImage *image=&t->images[view.named_index];
        coax_t_RenderAssetMip extent=aethyr_render_asset_mip(source->levels[0].width,source->levels[0].height,source->layers,0,t->device->props.limits.maxImageDimension2D,t->device->props.limits.maxImageArrayLayers);
        if(!extent.valid||(image->open&&image->array!=(view.layout.dimension==COAX_RenderTextureDimension_d2_array))){rhgt_error(error,bytes,"active image device view refused before payload writes");return 0;}
      }
    }
  }
  return 1;
}
void rhgt_close(RhgTextures *t){if(!t)return;if(t->images)for(size_t i=0;i<t->raw->texture_count;i++){if(t->images[i].open)vks_image_free(t->device,&t->images[i].image);free(t->images[i].packed);}free(t->images);free(t->names);free(t);}
RhgTextures *rhgt_open(Vkc *device,const RfiFrame *raw,char *error,size_t error_bytes){
  RhgTextures *t=calloc(1,sizeof*t);if(!t)return NULL;t->device=device;t->raw=raw;
  t->names=rfi_array(raw->texture_count,sizeof*t->names);t->images=rfi_array(raw->texture_count,sizeof*t->images);if(!t->names||!t->images){rhgt_close(t);return NULL;}
  for(size_t i=0;i<raw->texture_count;i++)t->names[i]=(coax_t_RenderHostNamedTexture){.name={raw->textures[i].name,(int64_t)strlen(raw->textures[i].name)},.layers=raw->textures[i].layers};
  if(aethyr_render_named_textures(rhgt_names(t))!=COAX_RenderResourceStatus_ok){rhgt_error(error,error_bytes,"texture-origin observations refused by Coax");rhgt_close(t);return NULL;}
  for(size_t i=0;i<raw->texture_count;i++){
    const RfiTexture *source=&raw->textures[i];RhgtImage *image=&t->images[i];
    coax_t_RenderTextureLevel *levels=rfi_array(source->level_count,sizeof*levels);if(!levels){rhgt_close(t);return NULL;}
    for(size_t j=0;j<source->level_count;j++){RfiLevel l=source->levels[j];levels[j]=(coax_t_RenderTextureLevel){.width=l.width,.height=l.height,.offset=l.offset,.row_bytes=l.row_bytes,.layer_bytes=l.layer_bytes};}
    coax_string format={source->format,(int64_t)strlen(source->format)};coax_v_t_RenderTextureLevel span={levels,(int64_t)source->level_count};image->need=aethyr_native_texture_require(format,source->layers,span);
    if(image->need.status!=COAX_RenderTextureStatus_ok){free(levels);rhgt_error(error,error_bytes,"texture extent/format refused by Coax");rhgt_close(t);return NULL;}
    coax_t_RQColor *colours=rfi_array((size_t)image->need.pixels,sizeof*colours);coax_t_RQMip *mips=rfi_array(source->level_count,sizeof*mips);
    if(!colours||!mips){free(colours);free(mips);free(levels);rhgt_close(t);return NULL;}
    coax_t_RFTextureDecoded decoded=aethyr_native_texture_decode(format,source->layers,span,(coax_v_u8){source->bytes.data,(int64_t)source->bytes.size},(coax_v_t_RQColor){colours,image->need.pixels},(coax_v_t_RQMip){mips,(int64_t)source->level_count});
    free(colours);free(mips);free(levels);if(decoded.status!=COAX_RenderTextureStatus_ok){rhgt_error(error,error_bytes,"texture byte/mip candidate refused by Coax");rhgt_close(t);return NULL;}t->names[i].depth=decoded.texture.depth;
    size_t texel=(size_t)aethyr_native_texel_bytes(image->need.format);image->bytes=(size_t)image->need.pixels*texel;image->packed=malloc(image->bytes);if(!image->packed){rhgt_close(t);return NULL;}
    size_t at=0;for(size_t j=0;j<source->level_count;j++){RfiLevel l=source->levels[j];size_t row=(size_t)l.width*texel;
      for(int64_t layer=0;layer<source->layers;layer++)for(int64_t y=0;y<l.height;y++){memcpy(image->packed+at,source->bytes.data+l.offset+(uint64_t)layer*l.layer_bytes+(uint64_t)y*l.row_bytes,row);at+=row;}}
  }
  return t;
}
const VksImage *rhgt_image(RhgTextures *t,coax_t_RenderHostTextureReceipt receipt,char *error,size_t error_bytes){
  if(receipt.status!=COAX_RenderResourceStatus_ok||receipt.named_index<0||(uint64_t)receipt.named_index>=t->raw->texture_count){rhgt_error(error,error_bytes,"no admitted external texture index");return NULL;}
  RhgtImage *image=&t->images[receipt.named_index];const RfiTexture *source=&t->raw->textures[receipt.named_index];int array=receipt.layout.dimension==COAX_RenderTextureDimension_d2_array;
  if(image->open&&image->array!=array){rhgt_error(error,error_bytes,"physical image view dimension changed");return NULL;}
  if(!image->open){
    coax_t_RenderAssetMip extent=aethyr_render_asset_mip(source->levels[0].width,source->levels[0].height,source->layers,0,t->device->props.limits.maxImageDimension2D,t->device->props.limits.maxImageArrayLayers);
    if(!extent.valid){rhgt_error(error,error_bytes,"texture device extent refused by Coax");return NULL;}
    // The codec admitted the authored chain. A canonical atlas can stop
    // before1x1; device extent admission does not add or replace its mips.
    image->image=vks_image_levels(t->device,(uint32_t)extent.width,(uint32_t)extent.height,(uint32_t)source->layers,array,rhgt_format(image->need.format),VKS_TEXTURE,(uint32_t)source->level_count);
    vks_upload(t->device,&image->image,image->packed,image->bytes);image->open=1;image->array=array;}
  return &image->image;
}
