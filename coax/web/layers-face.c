// Firm reason: 1 (browser SDK: concrete Wasm storage and scalar Coax image projections).
#include "layers-carrier.h"
#include <stdlib.h>
#define LAYERS_EXPORT(name) __attribute__((export_name(name)))
static coax_t_HostLayerSource layer_source;
static coax_t_HostLayerPlan layer_plan;
static uint8_t *layer_work,*layer_output;
static size_t layer_work_bytes,layer_output_bytes;
static int64_t layer_input_bytes;
static coax_v_u8 layer_source_view(void){return hl_source_storage((coax_v_t_HostLayerSource){&layer_source,1});}
LAYERS_EXPORT("layers_source_capacity") uint32_t layers_source_capacity(void){return(uint32_t)layer_source_view().count;}
LAYERS_EXPORT("layers_source_ptr") uint32_t layers_source_ptr(void){return(uint32_t)(uintptr_t)layer_source_view().data;}
LAYERS_EXPORT("layers_profile_side") int64_t layers_profile_side(int64_t profile){return htx_profile(profile).side;}
LAYERS_EXPORT("layers_plan") int64_t layers_plan(uint32_t count,int64_t side){
  coax_default_t_HostLayerPlan(&layer_plan);layer_input_bytes=0;coax_v_u8 source=layer_source_view();
  if(count>(uint64_t)source.count)return 4;
  layer_input_bytes=count;layer_plan=htx_plan((coax_v_u8){source.data,count},side);return layer_plan.status;
}
LAYERS_EXPORT("layers_info") int64_t layers_info(int32_t field){switch(field){case 0:return layer_plan.side;case 1:return layer_plan.width;case 2:return layer_plan.height;case 3:return layer_plan.levels;case 4:return layer_plan.bytes;case 5:return layer_plan.work;case 6:return layer_plan.base_offset;case 7:return layer_plan.source_bytes;default:return -1;}}
LAYERS_EXPORT("layers_decode") int64_t layers_decode(void){
  if(layer_plan.status!=0)return layer_plan.status;
  if(layer_plan.work>(int64_t)layer_work_bytes){uint8_t *next=realloc(layer_work,(size_t)layer_plan.work);if(!next)return -1;layer_work=next;layer_work_bytes=(size_t)layer_plan.work;}
  if(layer_plan.bytes>(int64_t)layer_output_bytes){uint8_t *next=realloc(layer_output,(size_t)layer_plan.bytes);if(!next)return -1;layer_output=next;layer_output_bytes=(size_t)layer_plan.bytes;}
  return htx_decode((coax_v_u8){layer_source_view().data,layer_input_bytes},layer_plan.side,(coax_v_u8){layer_work,(int64_t)layer_work_bytes},(coax_v_u8){layer_output,(int64_t)layer_output_bytes});
}
LAYERS_EXPORT("layers_output_ptr") uint32_t layers_output_ptr(void){return(uint32_t)(uintptr_t)layer_output;}
LAYERS_EXPORT("layers_close") void layers_close(void){free(layer_work);free(layer_output);layer_work=NULL;layer_output=NULL;layer_work_bytes=0;layer_output_bytes=0;layer_input_bytes=0;coax_default_t_HostLayerPlan(&layer_plan);}
/* Which GPU format a device's layers take and at which side (aethyr/render/texture-format.coax, GitHub #13014). */
LAYERS_EXPORT("layers_texture_format") int64_t layers_texture_format(int64_t features,int64_t carried,int32_t phone,int64_t role){return tf_pick(features,carried,phone!=0,role);}
LAYERS_EXPORT("layers_format_text") uint32_t layers_format_text(int64_t format){return(uint32_t)(uintptr_t)tf_name(format).data;}
LAYERS_EXPORT("layers_format_text_bytes") int32_t layers_format_text_bytes(int64_t format){return(int32_t)tf_name(format).count;}
LAYERS_EXPORT("layers_format_block") int64_t layers_format_block(int64_t format){return tf_block(format);}
LAYERS_EXPORT("layers_tier_side") int64_t layers_tier_side(int64_t declared,int32_t phone,int64_t format){return tf_side(declared,phone!=0,format);}
/* A pack's block builds (GitHub #13305): the tag in a build's file name and the carried bit it stands for. */
LAYERS_EXPORT("layers_format_tag") uint32_t layers_format_tag(int64_t format){return(uint32_t)(uintptr_t)tf_tag(format).data;}
LAYERS_EXPORT("layers_format_tag_bytes") int32_t layers_format_tag_bytes(int64_t format){return(int32_t)tf_tag(format).count;}
LAYERS_EXPORT("layers_format_carries") int64_t layers_format_carries(int64_t format){return tf_carries(format);}
/* A block layer (aethyr/render/block-layer.coax): its header, copied to the source storage, and its whole size; then
   its fields (0 format, 1 width, 2 height, 3 levels, 4 block) and each level's (0 offset, 1 bytes, 2 bytes a row of
   blocks, 3 rows of blocks, 4 physical width, 5 physical height). */
static coax_t_BlockLayer layer_block;
LAYERS_EXPORT("layers_block_read") int32_t layers_block_read(uint32_t head,int64_t total){coax_v_u8 source=layer_source_view();if(head>(uint64_t)source.count)return 0;layer_block=bl_read((coax_string){(const char*)source.data,head},total);return layer_block.ok;}
LAYERS_EXPORT("layers_block_info") int64_t layers_block_info(int32_t field){if(!layer_block.ok)return -1;switch(field){case 0:return layer_block.format;case 1:return layer_block.width;case 2:return layer_block.height;case 3:return layer_block.levels;case 4:return layer_block.block;default:return -1;}}
LAYERS_EXPORT("layers_block_level") int64_t layers_block_level(int32_t field,int64_t level){
  if(!layer_block.ok||level<0||level>=layer_block.levels)return -1;int64_t across=bl_blocks(layer_block,layer_block.width,level),down=bl_blocks(layer_block,layer_block.height,level);
  switch(field){case 0:return bl_level_at(layer_block,level);case 1:return bl_level_bytes(layer_block,level);case 2:return across*16;case 3:return down;case 4:return across*layer_block.block;case 5:return down*layer_block.block;default:return -1;}
}
