// The actual native producer and the same checked renderer used by captures.
#ifndef AETHYR_RENDER_REALM_H
#define AETHYR_RENDER_REALM_H
#include "render-host.h"
#include "realm-host.h"
typedef struct RhgRealm RhgRealm;
int rhgr_deliver(RealmHost *producer,int64_t profile,char *error,size_t error_bytes);
RhgRealm *rhgr_open(Vkc *device,RealmHost *producer,int64_t profile,const char *package_file,char *error,size_t error_bytes);
// Offer actual physical facts before advancing the producer. Unsupported
// contract/profile retains its legacy path; an eligible invalid plan refuses.
int rhgr_prepare(RhgRealm *realm,uint32_t width,uint32_t height,coax_t_RenderTextureFormat display_format,char *error,size_t error_bytes);
int rhgr_frame(RhgRealm *realm,uint32_t hud_vertices,uint32_t width,uint32_t height,int coarse,
  coax_t_RenderTextureFormat screen_format,char *error,size_t error_bytes);
VksImage *rhgr_screen(RhgRealm *realm);
VksImage *rhgr_graph(RhgRealm *realm);
// Complete deferred media notices only after the producer and encoder return.
void rhgr_deliver_media(RhgRealm *realm);
// The caller executes shared lifecycle loss/replacement actions. Destroy
// physical GPU objects before closing their device; keep producer/CPU owners.
void rhgr_drop_gpu(RhgRealm *realm);
int rhgr_device(RhgRealm *realm,Vkc *replacement,char *error,size_t error_bytes);
int64_t rhgr_info(RhgRealm *realm,int field);
RhgHandle rhgr_vertex_handle(RhgRealm *realm);
uint64_t rhgr_draw_commands(const RhgRealm *realm);
int64_t rhgr_close(RhgRealm *realm);
#endif
