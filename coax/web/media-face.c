// Firm reason: 1 (browser SDK: concrete Wasm storage and scalar Coax image projections).
#include "media-carrier.h"
#include <stdlib.h>
#define MEDIA_EXPORT(name) __attribute__((export_name(name)))
static coax_t_HostMediaSource media_source;
static coax_t_HostMediaPlan media_plan;
static uint8_t *media_work,*media_output;
static size_t media_work_bytes,media_output_bytes;
static int64_t media_input_bytes;
static coax_t_HostMediaNoticeRows media_notices;
static coax_t_HostMediaNoticeOutput media_notice_bytes;
static coax_v_u8 media_source_view(void){return hm_source_storage((coax_v_t_HostMediaSource){&media_source,1});}
MEDIA_EXPORT("media_source_capacity") uint32_t media_source_capacity(void){return(uint32_t)media_source_view().count;}
MEDIA_EXPORT("media_source_ptr") uint32_t media_source_ptr(void){return(uint32_t)(uintptr_t)media_source_view().data;}
MEDIA_EXPORT("media_plan") int64_t media_make_plan(uint32_t count){
  coax_default_t_HostMediaPlan(&media_plan);media_input_bytes=0;coax_v_u8 source=media_source_view();
  if(count>(uint64_t)source.count)return 4;
  media_input_bytes=count;media_plan=htm_plan((coax_v_u8){source.data,count});return media_plan.status;
}
MEDIA_EXPORT("media_info") int64_t media_info(int32_t field){switch(field){case 0:return media_plan.width;case 1:return media_plan.height;case 2:return media_plan.bytes;case 3:return media_plan.work;case 4:return media_plan.source_bytes;default:return -1;}}
MEDIA_EXPORT("media_decode") int64_t media_decode(void){
  if(media_plan.status!=0)return media_plan.status;
  if(media_plan.work>(int64_t)media_work_bytes){uint8_t *next=realloc(media_work,(size_t)media_plan.work);if(!next)return -1;media_work=next;media_work_bytes=(size_t)media_plan.work;}
  if(media_plan.bytes>(int64_t)media_output_bytes){uint8_t *next=realloc(media_output,(size_t)media_plan.bytes);if(!next)return -1;media_output=next;media_output_bytes=(size_t)media_plan.bytes;}
  return htm_decode((coax_v_u8){media_source_view().data,media_input_bytes},(coax_v_u8){media_work,(int64_t)media_work_bytes},(coax_v_u8){media_output,(int64_t)media_output_bytes});
}
MEDIA_EXPORT("media_output_ptr") uint32_t media_output_ptr(void){return(uint32_t)(uintptr_t)media_output;}
MEDIA_EXPORT("media_close") void media_close(void){free(media_work);free(media_output);media_work=NULL;media_output=NULL;media_work_bytes=0;media_output_bytes=0;media_input_bytes=0;coax_default_t_HostMediaPlan(&media_plan);}
MEDIA_EXPORT("media_notice_reset") void media_notice_reset(void){hm_notice_reset((coax_v_t_HostMediaNoticeRows){&media_notices,1});}
MEDIA_EXPORT("media_notice_row") uint32_t media_notice_row(int64_t row,int64_t at,int64_t bytes,int64_t ready,int64_t failed,int64_t width,int64_t height){return hm_notice_row((coax_v_t_HostMediaNoticeRows){&media_notices,1},row,media_source_view(),at,bytes,ready,failed,width,height);}
MEDIA_EXPORT("media_notice_format") int64_t media_notice_format(int64_t rows,int64_t capacity){return hm_notice_render((coax_v_t_HostMediaNoticeRows){&media_notices,1},rows,capacity,hm_notice_output((coax_v_t_HostMediaNoticeOutput){&media_notice_bytes,1}));}
MEDIA_EXPORT("media_notice_kind") int64_t media_notice_kind(void){return htn_kind();}
MEDIA_EXPORT("media_notice_ptr") uint32_t media_notice_ptr(void){return(uint32_t)(uintptr_t)hm_notice_output((coax_v_t_HostMediaNoticeOutput){&media_notice_bytes,1}).data;}
