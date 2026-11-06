// Actual bundled file and Vulkan image effects over the shared media policies.
#ifndef AETHYR_RENDER_LIVE_MEDIA_H
#define AETHYR_RENDER_LIVE_MEDIA_H
#include "render-media.h"
typedef struct RhgLiveMedia RhgLiveMedia;
RhgLiveMedia *rhglm_open(Vkc *,const char *physical_mount,size_t notice_capacity);
int rhglm_prepare(RhgLiveMedia *,int64_t profile,coax_string packet,int64_t width,int64_t height,RhgMediaRects *,char *error,size_t error_bytes);
const VksImage *rhglm_binding(RhgLiveMedia *,int64_t line);
// Borrowed until the next prepare; delivery belongs after producer/encoding.
const uint8_t *rhglm_notice(RhgLiveMedia *,size_t *bytes);
void rhglm_notice_done(RhgLiveMedia *);
// Destroy old GPU objects while their device is still alive. Canonical CPU
// pixels and immutable failed identities survive for lazy re-upload.
void rhglm_drop_gpu(RhgLiveMedia *);
void rhglm_device(RhgLiveMedia *,Vkc *);
const VksImage *rhglm_image(RhgLiveMedia *,coax_string url);
int64_t rhglm_info(RhgLiveMedia *,coax_string url,int field);
int64_t rhglm_close(RhgLiveMedia *);
#endif
