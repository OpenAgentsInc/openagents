#include "render-realm.c"
int main(void){
 Vkc v;vkc_open(&v);RealmHost producer={.floats=16,.max_vertices=8,.uniform_floats=432};char error[256];
 RhgRealm *r=rhgr_open(&v,&producer,6,"render/render.json",error,sizeof error);
 if(!r){fprintf(stderr,"constructor refused: %s\n",error);return 1;}
 RrBuffer *mesh=NULL;for(size_t i=0;i<r->buffer_count;i++)if(r->buffers[i].source.kind==COAX_RenderSourceKind_mesh_asset)mesh=&r->buffers[i];
 if(!mesh||mesh->capacity!=16||mesh->gpu.size!=16||!mesh->immutable)return 2;
 for(size_t i=0;i<16;i++)if(mesh->immutable[i]||mesh->retained[i]||((uint8_t*)mesh->gpu.map)[i])return 3;
 coax_t_RenderAssetReadiness ready=rr_asset_ready(r);if(ready.status!=COAX_RenderAssetStatus_ok||ready.readiness!=COAX_RenderReadiness_ready||ready.mesh_ready)return 4;
 rhgr_close(r);vkc_close(&v);return vkc_validation_errors||vkc_validation_warnings?5:0;
}
