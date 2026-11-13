// Exercise the actual scalar browser facade against its generated Coax ABI.
#include "RenderFace.c"
#include <assert.h>
#include <limits.h>
int main(void){
 const int32_t bad[]={INT32_MIN,-1,2,3,255,256,INT32_MAX};
 coaxial_render_reset();render_descriptor_profile=16;render_asset_count=1;
 for(unsigned i=0;i<sizeof bad/sizeof bad[0];i++){
  int32_t value=bad[i];render_assets.observations.e[0].state=COAX_RenderAssetState_pending;
  coaxial_render_asset_complete(0,value);assert(render_assets.observations.e[0].state==COAX_RenderAssetState_pending);
  render_asset_ready.bad_row=91;assert(coaxial_render_asset_readiness(value,1)==COAX_RenderAssetStatus_invalid);assert(render_asset_ready.bad_row==91);
  assert(coaxial_render_asset_readiness(1,value)==COAX_RenderAssetStatus_invalid);
  assert(coaxial_render_upload_stage(0,4,4)==COAX_RenderFrameStatus_ok);coaxial_render_upload_ack(0,value);assert(coaxial_render_upload_pending(0)==4);coaxial_render_upload_ack(0,1);assert(coaxial_render_upload_pending(0)==0);
  assert(coaxial_render_index_stage(4,4)==COAX_RenderFrameStatus_ok);coaxial_render_index_ack(value);assert(coaxial_render_index_pending()==4);coaxial_render_index_ack(1);assert(coaxial_render_index_pending()==0);
  render_buffer_view.bytes=91;assert(coaxial_render_buffer_view(0,1,16,0,0,0,16,0,4,value)==COAX_RenderResourceStatus_invalid);assert(render_buffer_view.bytes==91);
  render_target.width=91;assert(coaxial_render_target(1,2,2,value,0,100)==COAX_RenderResourceStatus_invalid);assert(render_target.width==91);
  coaxial_render_device(1,value,1);assert(render_device.backend==COAX_RenderBackend_invalid);
  coaxial_render_device(1,1,value);assert(render_device.backend==COAX_RenderBackend_invalid);
  coax_default_t_RenderHostPackageCandidate(&render_candidate);render_candidate.status=COAX_RenderPackageStatus_ok;render_candidate.package.entries=(coax_v_t_RenderPackageEntry){render_entries[0].entries.e,14};render_candidate.package.entries.data[0].present=1;
  assert(coaxial_render_candidate_pipeline(0,value)==COAX_RenderPackageStatus_invalid);assert(!render_candidate.pipelines.e[0]&&render_candidate.status==COAX_RenderPackageStatus_ok);
  assert(coaxial_render_scene_frame(0,0,0,0,0,0,0,0,0,0,0,2,value)==COAX_RenderRuntimeStatus_invalid);
 }
 const int32_t readiness_bad[]={INT32_MIN,-1,3,255,256,INT32_MAX};
 for(unsigned i=0;i<sizeof readiness_bad/sizeof readiness_bad[0];i++)assert(coaxial_render_scene_frame(0,0,0,0,0,0,0,0,0,0,0,readiness_bad[i],1)==COAX_RenderRuntimeStatus_invalid);
 render_assets.observations.e[0].state=COAX_RenderAssetState_pending;coaxial_render_asset_complete(0,1);assert(render_assets.observations.e[0].state==COAX_RenderAssetState_ready);coaxial_render_asset_complete(0,0);assert(render_assets.observations.e[0].state==COAX_RenderAssetState_failed);
 render_candidate.status=COAX_RenderPackageStatus_ok;assert(coaxial_render_candidate_pipeline(0,1)==COAX_RenderPackageStatus_ok&&render_candidate.pipelines.e[0]);
 assert(coaxial_render_asset_span(3,7,7,10)==COAX_RenderFrameStatus_ok);assert(coaxial_render_asset_span(3,7,8,20)==COAX_RenderFrameStatus_bad_upload);assert(coaxial_render_asset_span(3,7,7,9)==COAX_RenderFrameStatus_bad_upload);assert(coaxial_render_asset_span(0,7,1,10)==COAX_RenderFrameStatus_bad_upload);
 render_descriptor_profile=6;
 assert(coaxial_render_mesh_placeholder(0,1,16)==COAX_RenderResourceStatus_ok);
 assert(coaxial_render_buffer_view_info(0)==0&&coaxial_render_buffer_view_info(1)==16&&coaxial_render_buffer_view_info(2)==4);
 assert(coaxial_render_mesh_placeholder(0,1,15)==COAX_RenderResourceStatus_capacity);
 assert(coaxial_render_mesh_placeholder(0,0,16)==COAX_RenderResourceStatus_invalid);
 render_descriptor_profile=16;assert(coaxial_render_mesh_placeholder(0,1,16)==COAX_RenderResourceStatus_invalid);
 coaxial_render_draw_source(0x1000,16);
 coax_t_RenderHostWordView source=render_draw_source_view(0x1000,16,0x2000);
 assert(source.status==COAX_RenderFrameStatus_ok&&source.address==0x1000&&source.words==4);
 coaxial_render_draw_source(0x1000,15);assert(render_draw_source_view(0x1000,16,0x2000).status==COAX_RenderFrameStatus_bad_upload);
 coaxial_render_draw_source(0x1000,64);assert(render_draw_source_view(0x103c,16,0x2000).status==COAX_RenderFrameStatus_bad_upload);
 coaxial_render_draw_source(0x1001,16);assert(render_draw_source_view(0x1001,16,0x2000).status==COAX_RenderFrameStatus_bad_upload);
 coaxial_render_draw_source(0x1000,16);assert(render_draw_source_view(0x1000,16,0x100f).status==COAX_RenderFrameStatus_bad_upload);
 coaxial_render_draw_source(0,16);assert(render_draw_source_view(0,16,0x2000).status==COAX_RenderFrameStatus_bad_upload);
 coaxial_render_draw_source(0,0);source=render_draw_source_view(0,0,0);
 assert(source.status==COAX_RenderFrameStatus_ok&&source.address==0&&source.words==0);
 coaxial_render_draw_source(0x1000,16);coaxial_render_reset();assert(render_draw_source_view(0x1000,16,0x2000).status==COAX_RenderFrameStatus_bad_upload);
 return 0;
}
