// Exact observed mip bytes. The Coax codec and shared texture-origin policy
// admit them; this effect layer only allocates, row-packs and uploads pixels.
#ifndef AETHYR_RENDER_TEXTURES_H
#define AETHYR_RENDER_TEXTURES_H
#include "render-host.h"
#include "RenderTexture.h"
#include "frame-input.h"
typedef struct RhgTextures RhgTextures;
RhgTextures *rhgt_open(Vkc *device,const RfiFrame *raw,char *error,size_t error_bytes);
coax_v_t_RenderHostNamedTexture rhgt_names(RhgTextures *textures);
// Consume only the admitted active graph and Core views; no GPU effect.
int rhgt_frame_admit(RhgTextures *textures,int64_t profile,const coax_t_RenderRuntimeFrame *frame,
  int coarse,coax_t_RenderTextureFormat screen,char *error,size_t error_bytes);
const VksImage *rhgt_image(RhgTextures *textures,coax_t_RenderHostTextureReceipt receipt,char *error,size_t error_bytes);
void rhgt_close(RhgTextures *textures);
#endif
