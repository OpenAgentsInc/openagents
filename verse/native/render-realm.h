// The actual native producer and the same checked renderer used by captures.
#ifndef AETHYR_RENDER_REALM_H
#define AETHYR_RENDER_REALM_H
#include "render-host.h"
#include "realm-host.h"
typedef struct RhgRealm RhgRealm;
int rhgr_deliver(RealmHost *producer,int64_t profile,char *error,size_t error_bytes);
RhgRealm *rhgr_open(Vkc *device,RealmHost *producer,int64_t profile,const char *package_file,char *error,size_t error_bytes);
int rhgr_frame(RhgRealm *realm,uint32_t hud_vertices,uint32_t width,uint32_t height,int coarse,
  coax_t_RenderTextureFormat screen_format,char *error,size_t error_bytes);
VksImage *rhgr_screen(RhgRealm *realm);
uint64_t rhgr_draw_commands(const RhgRealm *realm);
void rhgr_close(RhgRealm *realm);
#endif
