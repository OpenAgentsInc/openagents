// Firm reason: 6 (native Vulkan ABI; every app's behaviour is Coax).
// The drawing the Vulkan hosts share (ui-host.h): Linux's (linux/ui-host.c) and Windows'
// (windows/ui-host.c, GitHub #13782) include this file after their struct UiHost, which has
// the fields used here, and their ui_now, ui_host_nap (no swapchain image: a short sleep) and
// ui_host_idle (an unchanged frame: wait up to a frame's time for the window's events). It holds
// the component pipeline over the app's atlas, composition-v1's plan, the frame loop from the
// swapchain's image to the present, offscreen pictures, and the pointer's click counting.
#include "png.h"

// ---- the pipeline

static void ui_comp_reset(UiHost*h) {
 for(int i=0;i<h->ncp;i++)vks_pipe_close(&h->v,&h->cp[i].pipe);h->ncp=0;
 for(int i=0;i<h->ncompose_targets;i++)vks_image_free(&h->v,&h->compose_targets[i]);h->ncompose_targets=0;
 for(int i=0;i<16;i++)if(h->images[i].image){vks_image_free(&h->v,&h->images[i]);memset(&h->images[i],0,sizeof h->images[i]);h->image_generation[i]=0;}
}
static void ui_pipe(UiHost *h) {
  if (h->has_pipe) { vkDeviceWaitIdle(h->v.dev); vks_pipe_close(&h->v, &h->pipe); h->has_pipe = 0; }
  if (!h->has_atlas || !h->has_target) return;
  h->binds[0] = (VksBinding){.kind = VKS_STORAGE, .buffer = &h->uniform};
  h->binds[1] = (VksBinding){.kind = VKS_IMAGE, .image = &h->mono};
  h->binds[2] = (VksBinding){.kind = VKS_IMAGE, .image = &h->color};
  h->pass = (VksPass){.words = h->words, .count = h->nwords, .bindings = h->binds, .nb = 3, .vertices = &h->verts, .stride = UI_VERTEX_BYTES,
                      .nattrs = 7, .vertex_count = 0, .instance_count = 1, .ntargets = 1, .blend = VKS_ALPHA, .clear = {0, 0, 0, 1}};
  h->pass.targets[0] = &h->target;
  for (uint32_t i = 0; i < 7; i++) h->pass.attrs[i] = (VkVertexInputAttributeDescription){.location = i, .binding = 0, .format = VK_FORMAT_R32G32B32A32_SFLOAT, .offset = 16 * i};
  vks_pipe_open(&h->v, &h->pipe, &h->pass);
  h->has_pipe = 1;
}
static void ui_target(UiHost *h, uint32_t w, uint32_t rows) {
  if (h->has_target && h->w == w && h->rows == rows) return;
  vkDeviceWaitIdle(h->v.dev);
  ui_comp_reset(h);
  if (h->has_pipe) { vks_pipe_close(&h->v, &h->pipe); h->has_pipe = 0; }
  if (h->has_target) vks_image_free(&h->v, &h->target);
  h->target = vks_image(&h->v, w, rows, 1, 0, VK_FORMAT_R8G8B8A8_UNORM, VKS_TARGET | VKS_TEXTURE);
  h->has_target = 1; h->w = w; h->rows = rows; h->stale = 1;
  ui_pipe(h);
}
void ui_atlas(UiHost *h, const uint8_t *mono, uint32_t w, uint32_t rows, const uint8_t *color, uint32_t cw, uint32_t crows) {
  if (!h->offscreen) vkp_wait(&h->v, &h->p);
  int fresh = !h->has_atlas || h->mono.w != w || h->mono.h != rows || h->color.w != cw || h->color.h != crows;
  if (fresh) {
    vkDeviceWaitIdle(h->v.dev);
    ui_comp_reset(h);
    if (h->has_pipe) { vks_pipe_close(&h->v, &h->pipe); h->has_pipe = 0; }
    if (h->has_atlas) { vks_image_free(&h->v, &h->mono); vks_image_free(&h->v, &h->color); }
    h->mono = vks_image(&h->v, w, rows, 1, 0, VK_FORMAT_R8_UNORM, VKS_TEXTURE);
    h->color = vks_image(&h->v, cw, crows, 1, 0, VK_FORMAT_R8G8B8A8_UNORM, VKS_TEXTURE);
    h->has_atlas = 1;
  }
  vks_upload(&h->v, &h->mono, mono, (size_t)w * rows);
  vks_upload(&h->v, &h->color, color, (size_t)cw * crows * 4);
  h->stale = 1;
  if (fresh) ui_pipe(h);
}
// A frame's vertices as one number: equal numbers are taken as the same picture.
static uint64_t ui_digest(const UiHost *h) {
  const uint64_t *words = (const uint64_t *)h->frame;
  size_t n = (size_t)h->pass.vertex_count * UI_VERTEX_BYTES / 8;
  uint64_t d = 0x9e3779b97f4a7c15ull ^ h->pass.vertex_count ^ ((uint64_t)h->w << 32) ^ ((uint64_t)h->rows << 48);
  for (size_t i = 0; i < n; i++) { d ^= words[i]; d *= 0xff51afd7ed558ccdull; d ^= d >> 29; }
  if(h->composed>0){const uint32_t*ops=(const uint32_t*)h->composition.ops;for(size_t i=0;i<h->composition.count*8;i++){d^=ops[i];d*=0xff51afd7ed558ccdull;d^=d>>29;}}
  return d;
}
static void ui_record(VkCommandBuffer cmd, void *user) {
 UiHost*h=user;if(!h->composed){vks_pipe_record(&h->pipe,cmd,&h->pass);return;}
 for(uint32_t i=0;i<h->composition.count;i++) {
  const float*r=h->composition.ops+i*8;int target=(int)r[0],source=(int)r[1],clear=r[4]!=0;
  for(int k=0;k<h->ncp;k++){UiCompPipe*p=&h->cp[k];if(p->target!=target||p->source!=source||p->clear!=clear)continue;
   uint32_t draw[4]={(uint32_t)r[2],(uint32_t)r[3],0x80000000u,1};VksPass pass=p->pass;pass.draws=draw;pass.ndraws=r[3]>0?1:0;vks_pipe_record(&p->pipe,cmd,&pass);break;}
 }
}
// The frame's vertices to the GPU's buffer, once it is known they will be drawn.
static void ui_hand_over(UiHost *h) { memcpy(h->verts.map, h->frame, (size_t)h->pass.vertex_count * UI_VERTEX_BYTES); }
static void ui_fill(UiHost *h, float dt) {
  if (h->drive.agent) dt = h->drive.giving;   // the agent clock (../drive.c)
  float *u = h->uniform.map;
  u[0] = (float)h->w; u[1] = (float)h->rows; u[2] = 0; u[3] = 0;
  int n = h->app.frame ? h->app.frame(h->app.user, h->frame, (float)h->w, (float)h->rows, h->dpr, dt) : 0;
  if (n < 0) n = 0;
  if ((uint32_t)n > h->app.max_vertices) n = (int)h->app.max_vertices;
  h->pass.vertex_count = (uint32_t)n;
#ifdef UI_ATSPI
  if(!h->access && h->app.elements && h->app.element)h->access=ui_access_create(&h->app);
  if(h->access && ui_access_sync(h->access) && !h->offscreen && !h->access_attempted){h->access_attempted=1;(void)ui_access_bus_start(h->access);}
#endif
}

// ---- the pointer: where the left button went down and when, for double and triple clicks.
// x and y in points; the app gets device pixels.

static void ui_seat_pointer(UiHost *h, float x, float y, int phase) {
  if (phase == 0) {
    double t = ui_now();
    int again = t - h->down_at < 0.4 && fabsf(x - h->down_x) < 5 && fabsf(y - h->down_y) < 5;
    h->clicks = again && h->clicks < 3 ? h->clicks + 1 : 1;
    h->down_at = t; h->down_x = x; h->down_y = y; h->held = 1;
  }
  if (phase == 2) h->held = 0;
  if (h->app.pointer) h->app.pointer(h->app.user, x * h->dpr, y * h->dpr, phase, h->clicks ? h->clicks : 1, ui_mods(h));
}
static void ui_seat_moved(UiHost *h, float x, float y) {
  if (!h->held && h->app.pointer) h->app.pointer(h->app.user, x * h->dpr, y * h->dpr, 1, 1, ui_mods(h));
}
float ui_dpr(UiHost *h) { return h->dpr; }
void ui_set_dpr(UiHost *h, float dpr) { if (dpr > 0) h->dpr = dpr; }

// ---- frames

static uint32_t *ui_slurp(const char *path, size_t *words) {
  FILE *f = fopen(path, "rb");
  if (!f) return NULL;
  fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
  if (size <= 0 || size % 4) { fclose(f); return NULL; }
  uint32_t *p = malloc((size_t)size);
  if (!p || fread(p, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(p); return NULL; }
  fclose(f);
  *words = (size_t)size / 4;
  return p;
}
void ui_host_set_ext_texture(UiHost *h, int slot, void *native_tex) {
  if (!h || slot < 0 || slot >= 16) return;
  VksImage *img = (VksImage *)native_tex;
  if (h->ext_textures[slot] != img) {
    h->ext_textures[slot] = img;
    h->compose_signature ^= 0x9e3779b97f4a7c15ull ^ (uint64_t)(slot + 1);
    h->stale = 1;
  }
}

// Executes only the admitted shared Coax plan. Cached SDK pipelines own
// immutable descriptor sets; all prior work completes before an atlas, image,
// target or vertex buffer can be replaced. No per-frame readback.
static int ui_comp_prepare(UiHost*h) {
 h->composed=h->app.composition?h->app.composition(h->app.user,&h->composition):0;
 if(!h->composed){if(h->ncompose_targets)ui_comp_reset(h);return 1;}
 UiComposition p=h->composition;
 if(h->composed<0||!h->has_atlas||!p.ops||!p.count||p.count>4096||!p.targets||p.targets>16||p.words>33554432||h->w>4096||h->rows>4096||!h->app.composition_shader)return 0;
 if(!h->comp_words){h->comp_words=ui_slurp(h->app.composition_shader,&h->comp_nwords);if(!h->comp_words)return 0;}
 uint64_t signature=1469598103934665603ull;uint32_t image_mask=0;
 for(uint32_t i=0;i<p.count;i++){const float*r=p.ops+i*8;for(int lane=0;lane<3;lane++){int k=lane==2?4:lane;signature^=(uint32_t)(int)r[k];signature*=1099511628211ull;}int src=(int)r[1];if(src>=64&&src<80)image_mask|=1u<<(src-64);}
 int changed=h->ncompose_targets!=(int)p.targets||h->image_mask!=image_mask;
 if(h->compose_signature!=signature){for(int i=0;i<h->ncp;i++)vks_pipe_close(&h->v,&h->cp[i].pipe);h->ncp=0;h->compose_signature=signature;}
 h->image_mask=image_mask;
 for(uint32_t i=0;i<p.count;i++){int src=(int)p.ops[i*8+1];if(src>=80){if(src>=96||!h->ext_textures[src-80])return 0;}else if(src>=64){UiImage image={0};int k=src-64;
  if(!h->app.image||!h->app.image(h->app.user,k,&image)||!image.rgba||!image.width||!image.height||image.width>4096||image.height>4096)return 0;
  if(!h->images[k].image||h->image_generation[k]!=image.generation||h->images[k].w!=image.width||h->images[k].h!=image.height)changed=1;}}
 if(changed){ui_comp_reset(h);for(uint32_t i=0;i<p.targets;i++)h->compose_targets[i]=vks_image(&h->v,h->w,h->rows,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TARGET|VKS_TEXTURE);h->ncompose_targets=p.targets;
  for(uint32_t i=0;i<p.count;i++){int src=(int)p.ops[i*8+1];if(src>=64&&src<80){int k=src-64;if(!h->images[k].image){UiImage image={0};h->app.image(h->app.user,k,&image);
   h->images[k]=vks_image(&h->v,image.width,image.height,1,0,VK_FORMAT_R8G8B8A8_UNORM,VKS_TEXTURE);vks_upload(&h->v,&h->images[k],image.rgba,(size_t)image.width*image.height*4);h->image_generation[k]=image.generation;}}}h->stale=1;}
 for(uint32_t i=0;i<p.count;i++){
  const float*r=p.ops+i*8;int dst=(int)r[0],src=(int)r[1],clear=r[4]!=0;
  if(dst< -1||dst>=(int)p.targets||src< -1||(src>=0&&src<64&&src>=(int)p.targets)||src>=96||dst==src||r[2]<0||r[3]<0||(uint64_t)r[2]+(uint64_t)r[3]>h->pass.vertex_count)return 0;
  int k=0;while(k<h->ncp&&(h->cp[k].target!=dst||h->cp[k].source!=src||h->cp[k].clear!=clear))k++;
  if(k==h->ncp){if(k==64)return 0;UiCompPipe*c=&h->cp[h->ncp++];c->target=dst;c->source=src;c->clear=clear;
   c->binds[0]=(VksBinding){.kind=VKS_STORAGE,.buffer=&h->uniform};c->binds[1]=(VksBinding){.kind=VKS_IMAGE,.image=&h->mono};c->binds[2]=(VksBinding){.kind=VKS_IMAGE,.image=&h->color};c->binds[3]=(VksBinding){.kind=VKS_IMAGE,.image=src<0?&h->color:src>=80?h->ext_textures[src-80]:src>=64?&h->images[src-64]:&h->compose_targets[src]};
   c->pass=h->pass;c->pass.words=h->comp_words;c->pass.count=h->comp_nwords;c->pass.bindings=c->binds;c->pass.nb=4;c->pass.targets[0]=dst<0?&h->target:&h->compose_targets[dst];c->pass.vertex_count=0;c->pass.blend=VKS_OVER;c->pass.alpha_blend=VKS_OVER;c->pass.keep=!clear;
   for(int lane=0;lane<4;lane++)c->pass.clear[lane]=0;vks_pipe_open(&h->v,&c->pipe,&c->pass);
  }
 }
 return 1;
}
// One frame of the window: its swapchain image, the app's vertices, drawn and presented when
// they changed. Always 1 (the loop goes on); the host has read its events and sized the swapchain.
static int ui_present(UiHost *h) {
  // A swapchain image without waiting for one: while the compositor holds them all (the window is
  // hidden, or it is waiting for this client to take a new size) the loop keeps reading the seat
  // and the compositor's events. Waiting in the acquire instead stopped the app for good on a resize.
  int slot = vkp_reserve(&h->v, &h->p);
  double now = ui_now();
  if (slot < 0) { vkp_resize(&h->v, &h->p, h->p.w, h->p.h); return 1; }
  if (slot == 0) {
    // no image: the app still gets a frame's worth of time ten times a second, so a turn goes on while hidden
    if (!h->app.composition && now - h->last >= 0.1) { float dt = (float)(now - h->last); h->last = now; ui_target(h, h->p.w, h->p.h); ui_fill(h, dt > 0.1f ? 0.1f : dt); }
    ui_host_nap(h);
    return 1;
  }
  ui_target(h, h->p.w, h->p.h);
  float dt = (float)(now - h->last); h->last = now;
  if (dt > 0.1f) dt = 0.1f;
  if(!h->offscreen)vkp_wait(&h->v,&h->p);
  ui_fill(h, dt);
  if(!ui_comp_prepare(h)){if(h->app.presented)h->app.presented(h->app.user,0);return h->offscreen?0:1;}
  if (!h->has_pipe) return 1;   // no atlas yet: the app gives one from its first frame
  // The same vertices over the same atlas at the same size are the same picture: it is not drawn
  // or presented again, and the loop waits for the seat or the compositor (a frame's time at most,
  // so the app still polls its engines). A still window then costs the app's frame and no GPU work.
  uint64_t digest = ui_digest(h);
  if (!h->stale && digest == h->shown) {
    if(h->app.presented)h->app.presented(h->app.user,1);
    ui_host_idle(h);
    return 1;
  }
  h->shown = digest; h->stale = 0;
  ui_hand_over(h);
  if (vkp_present(&h->v, &h->p, ui_record, h, h->target.image, h->w, h->rows, NULL, NULL)) { vkp_resize(&h->v, &h->p, h->p.w, h->p.h); h->stale = 1; }
  if(h->composed)vkp_wait(&h->v,&h->p);
  if(h->app.presented)h->app.presented(h->app.user,!h->stale);
  // UI_HOST_TRACE=1: frames presented each second, on stderr
  static int trace = -1, frames; static double since;
  if (trace < 0) trace = getenv("UI_HOST_TRACE") != NULL;
  if (trace) { frames++; if (now - since >= 1) { fprintf(stderr, "ui-host: %d frames, %ux%u\n", frames, h->w, h->rows); frames = 0; since = now; } }
  return 1;
}
int ui_shot_frame(UiHost *h, uint32_t w, uint32_t rows, float dt) {
  if (!h->offscreen || !w || !rows) return 0;
  ui_target(h, w, rows);
  if(!h->offscreen)vkp_wait(&h->v,&h->p);
  ui_fill(h, dt);
  if(!ui_comp_prepare(h)){if(h->app.presented)h->app.presented(h->app.user,0);return h->offscreen?0:1;}
  if (!h->has_pipe) return 1;
  ui_hand_over(h);
  vks_submit(&h->v, ui_record, h);
  if(h->app.presented)h->app.presented(h->app.user,1);
  return 1;
}
int ui_shot_write(UiHost *h, const char *png) {
  if (!h->has_target || !h->has_pipe) return 0;
  size_t w = h->w, rows = h->rows;
  uint8_t *px = malloc(w * rows * 4);
  if (!px) return 0;
  vks_read(&h->v, &h->target, px, w * rows * 4);
  int ok = ui_png_write(png, px, w, rows);
  free(px);
  return ok;
}
int ui_readback(UiHost*h,uint8_t*out,size_t cap) {if(!h->offscreen||!h->has_target||cap<(size_t)h->w*h->rows*4)return 0;vks_read(&h->v,&h->target,out,(size_t)h->w*h->rows*4);return 1;}
// The GPU's side of ui_close: every pipeline, image and buffer, after the device is idle.
static void ui_render_close(UiHost *h) {
  vkDeviceWaitIdle(h->v.dev);
  ui_comp_reset(h);free(h->comp_words);
  if (h->has_pipe) vks_pipe_close(&h->v, &h->pipe);
  if (h->has_target) vks_image_free(&h->v, &h->target);
  if (h->has_atlas) { vks_image_free(&h->v, &h->mono); vks_image_free(&h->v, &h->color); }
  vkc_free(&h->v, &h->verts); vkc_free(&h->v, &h->uniform);
}
