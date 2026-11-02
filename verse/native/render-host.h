// Vulkan effects for a package and frame admitted by the shared Coax renderer.
// RenderHost.h is generated from aethyr/render/native-host.coax by the build.
#ifndef AETHYR_NATIVE_RENDER_HOST_H
#define AETHYR_NATIVE_RENDER_HOST_H
#include "RenderHost.h"
#include "vks.h"

typedef struct {const unsigned char *data;size_t bytes;} RhgBytes;
typedef RhgBytes (*RhgRead)(void *user,coax_string file);
typedef struct RhgRenderer RhgRenderer;
// The callback supplies actual handles and Coax-admitted byte views. Kind,
// slot and stage visibility come from use, never from host names or WGSL.
typedef int (*RhgBinding)(void *user,coax_t_RenderHostUse use,coax_t_RenderOp op,const coax_t_RenderMediaRect *media,VksBinding *out);
typedef struct {
  const VkcBuffer *hud,*index;
  coax_v_u32 draw_words;
  RhgBinding binding;void *user;
  coax_t_RenderHostMediaReceipt media_receipt;
  coax_v_t_RenderMediaRect media_rects;
} RhgFrame;

RhgRenderer *rhg_open(Vkc *device,int64_t expected_profile,coax_string json,RhgRead read,void *user,char *error,size_t error_bytes);
// The receipt must be the unmodified result of aethyr_render_frame for these
// exact resources. Media receipts/rectangles must likewise be the unmodified
// result of the shared composer for these physical textures. Refusal makes
// no render submission for Coax/resource refusal; preparation may allocate
// private scratch resources. A device validation failure after submission can
// already have effects and returns failure without acknowledging uploads.
int rhg_execute(RhgRenderer *renderer,const coax_t_RenderRuntimeFrame *receipt,const RhgFrame *frame,
  int coarse,coax_t_RenderTextureFormat screen_format,char *error,size_t error_bytes);
VksImage *rhg_screen(RhgRenderer *renderer);
VksImage *rhg_target_image(RhgRenderer *renderer,coax_t_RenderTarget target);
uint64_t rhg_draw_commands(const RhgRenderer *renderer);
// The GPU resource ledger (aethyr/render/registry.coax), one per process as in
// the browser's face. Ask before creating a buffer or image, release when it is
// freed; a zero handle is none. Words come from Coax (aethyr_render_*_words).
enum {RHG_UNIFORM,RHG_STORAGE,RHG_COLOR,RHG_VERTEX,RHG_INDEX,RHG_TEXTURE};
typedef struct {int64_t slot,generation;} RhgHandle;
int rhg_ledger_acquire(int kind,int64_t words,RhgHandle *out);
void rhg_ledger_release(RhgHandle *handle);
int rhg_ledger_live(const RhgHandle *handle);
// 0 epoch, 1 live objects, 2 live words, 3 slots, 4 word budget.
int64_t rhg_ledger_info(int field);
void rhg_close(RhgRenderer *renderer);
#endif
