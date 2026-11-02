// Platform observations into the shared Coax media composer. No command
// parsing, identity synthesis, aspect fitting or rectangle policy in C.
#ifndef AETHYR_RENDER_MEDIA_H
#define AETHYR_RENDER_MEDIA_H
#include "render-textures.h"
typedef struct {coax_t_RenderHostMediaReceipt receipt;coax_t_RenderMediaRect *rects;} RhgMediaRects;
int rhgm_rects(int64_t profile,const RfiFrame *raw,RhgTextures *textures,int64_t width,int64_t height,
  RhgMediaRects *out,char *error,size_t error_bytes);
void rhgm_close(RhgMediaRects *media);
#endif
