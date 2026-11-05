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
