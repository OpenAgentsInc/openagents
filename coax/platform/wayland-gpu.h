/* GPU effects for the Coax desk. Coax selects/adopts descriptors, dimensions, formats and
   modifiers. EGL imports a DMA-BUF, synchronously reads it, and releases its own objects.
   The first accelerated path retains the desk's CPU composition/screenshot transport.
   These are dedicated compositor resources, outside the shared realm renderer. */
#ifndef COAX_WAYLAND_GPU_H
#define COAX_WAYLAND_GPU_H
#ifdef COAX_WAYLAND_GPU
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES2/gl2.h>
#include <GLES2/gl2ext.h>
#include <gbm.h>

static EGLDisplay desk_egl = EGL_NO_DISPLAY;
static EGLContext desk_ctx = EGL_NO_CONTEXT;
static struct gbm_device *desk_gbm;
static int desk_drm = -1;
static PFNEGLCREATEIMAGEKHRPROC desk_image;
static PFNEGLDESTROYIMAGEKHRPROC desk_unimage;
static PFNGLEGLIMAGETARGETTEXTURE2DOESPROC desk_texture;
static struct { uint32_t format; uint64_t modifier; } desk_formats[256];
static int desk_nformats;
static int64_t desk_imports, desk_captures;
static struct { EGLImageKHR image; GLuint tex, fbo; int w, h; uint8_t *pixels; } desk_images[1024];

int64_t cxgpu_close(void);
int64_t cxgpu_open(coax_v_u8 node) {
  char path[256];
  if (node.count <= 0 || node.count >= (int64_t)sizeof path) return -1;
  if (desk_egl != EGL_NO_DISPLAY) return -1;
  memcpy(path, node.data, (size_t)node.count); path[node.count] = 0;
  /* NixOS exposes its selected driver here, outside the package's immutable library closure. */
  if (!getenv("__EGL_VENDOR_LIBRARY_DIRS") && !getenv("__EGL_VENDOR_LIBRARY_FILENAMES") &&
      access("/run/opengl-driver/share/glvnd/egl_vendor.d", R_OK) == 0)
    setenv("__EGL_VENDOR_LIBRARY_DIRS", "/run/opengl-driver/share/glvnd/egl_vendor.d", 0);
  if (!getenv("GBM_BACKENDS_PATH") && access("/run/opengl-driver/lib/gbm", R_OK) == 0)
    setenv("GBM_BACKENDS_PATH", "/run/opengl-driver/lib/gbm", 0);
  desk_drm = open(path, O_RDWR | O_CLOEXEC);
  if (desk_drm < 0) return -1;
  desk_gbm = gbm_create_device(desk_drm);
  PFNEGLGETPLATFORMDISPLAYEXTPROC platform = (void *)eglGetProcAddress("eglGetPlatformDisplayEXT");
  if (!desk_gbm || !platform) goto fail;
  desk_egl = platform(EGL_PLATFORM_GBM_KHR, desk_gbm, NULL);
  if (desk_egl == EGL_NO_DISPLAY || !eglInitialize(desk_egl, NULL, NULL)) goto fail;
  if (!eglBindAPI(EGL_OPENGL_ES_API)) goto fail;
  EGLConfig config; EGLint n;
  EGLint attrs[] = { EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT, EGL_SURFACE_TYPE, EGL_PBUFFER_BIT,
                    EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_NONE };
  if (!eglChooseConfig(desk_egl, attrs, &config, 1, &n) || !n) goto fail;
  EGLint ca[] = { EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE };
  desk_ctx = eglCreateContext(desk_egl, config, EGL_NO_CONTEXT, ca);
  if (desk_ctx == EGL_NO_CONTEXT || !eglMakeCurrent(desk_egl, EGL_NO_SURFACE, EGL_NO_SURFACE, desk_ctx)) goto fail;
  desk_image = (void *)eglGetProcAddress("eglCreateImageKHR");
  desk_unimage = (void *)eglGetProcAddress("eglDestroyImageKHR");
  desk_texture = (void *)eglGetProcAddress("glEGLImageTargetTexture2DOES");
  PFNEGLQUERYDMABUFMODIFIERSEXTPROC query = (void *)eglGetProcAddress("eglQueryDmaBufModifiersEXT");
  if (!desk_image || !desk_unimage || !desk_texture || !query) goto fail;
  uint32_t formats[] = { 0x34325241, 0x34325258 };
  desk_nformats = 0;
  for (int f = 0; f < 2; f++) {
    EGLuint64KHR mods[128]; EGLBoolean external[128]; EGLint count = 0;
    if (!query(desk_egl, formats[f], 128, mods, external, &count)) continue;
    for (int i = 0; i < count && i < 128; i++) {
      if (external[i]) continue; /* These cannot be framebuffer attachments for readback. */
      desk_formats[desk_nformats].format = formats[f];
      desk_formats[desk_nformats++].modifier = mods[i];
    }
  }
  if (!desk_nformats) goto fail;
  fprintf(stderr, "desk GPU: %s; %d format/modifier pairs\n", glGetString(GL_RENDERER), desk_nformats);
  return desk_nformats;
fail:
  fprintf(stderr, "desk GPU unavailable (EGL 0x%x); shared-memory clients remain available\n", eglGetError());
  cxgpu_close(); return -1;
}
int64_t cxgpu_format(int64_t i, int64_t field) {
  if (i < 0 || i >= desk_nformats) return -1;
  if (field == 0) return desk_formats[i].format;
  return field == 1 ? (int64_t)(desk_formats[i].modifier >> 32) : (int64_t)(desk_formats[i].modifier & UINT32_MAX);
}
int64_t cxgpu_device(void) { struct stat s; return desk_drm >= 0 && fstat(desk_drm, &s) == 0 ? (int64_t)s.st_rdev : -1; }
int64_t cxgpu_import(int64_t fd, int64_t w, int64_t h, int64_t stride, int64_t offset,
                     int64_t fmt, int64_t hi, int64_t lo) {
  if (desk_ctx == EGL_NO_CONTEXT || fd < 0 || w <= 0 || w > 3840 || h <= 0 || h > 2160) return -1;
  int slot = -1;
  for (int i = 0; i < 1024; i++) if (!desk_images[i].pixels) { slot = i; break; }
  if (slot < 0) return -1;
  EGLint attrs[] = { EGL_WIDTH, (EGLint)w, EGL_HEIGHT, (EGLint)h,
    EGL_LINUX_DRM_FOURCC_EXT, (EGLint)fmt,
    EGL_DMA_BUF_PLANE0_FD_EXT, (EGLint)fd, EGL_DMA_BUF_PLANE0_OFFSET_EXT, (EGLint)offset,
    EGL_DMA_BUF_PLANE0_PITCH_EXT, (EGLint)stride,
    EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT, (EGLint)hi,
    EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT, (EGLint)lo, EGL_NONE };
  EGLImageKHR image = desk_image(desk_egl, EGL_NO_CONTEXT, EGL_LINUX_DMA_BUF_EXT, NULL, attrs);
  if (image == EGL_NO_IMAGE_KHR) return -1;
  while (glGetError() != GL_NO_ERROR) {}
  GLuint tex, fbo; glGenTextures(1, &tex); glBindTexture(GL_TEXTURE_2D, tex);
  desk_texture(GL_TEXTURE_2D, image);
  glGenFramebuffers(1, &fbo); glBindFramebuffer(GL_FRAMEBUFFER, fbo);
  glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex, 0);
  uint8_t *pixels = NULL;
  if (glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE && glGetError() == GL_NO_ERROR)
    pixels = malloc((size_t)w * (size_t)h * 4);
  if (!pixels) { glDeleteFramebuffers(1, &fbo); glDeleteTextures(1, &tex); desk_unimage(desk_egl, image); return -1; }
  desk_images[slot].image = image; desk_images[slot].tex = tex; desk_images[slot].fbo = fbo;
  desk_images[slot].w = (int)w; desk_images[slot].h = (int)h; desk_images[slot].pixels = pixels;
  desk_imports++;
  return slot;
}
/* glReadPixels waits for the implicit DMA-BUF producer dependency and for this read to finish.
   Coax sends wl_buffer.release only after the surface stops using the imported image. */
int64_t cxgpu_capture(int64_t slot) {
  if (slot < 0 || slot >= 1024 || !desk_images[slot].pixels) return -1;
  glBindFramebuffer(GL_FRAMEBUFFER, desk_images[slot].fbo);
  glReadPixels(0, 0, desk_images[slot].w, desk_images[slot].h, GL_RGBA, GL_UNSIGNED_BYTE, desk_images[slot].pixels);
  if (glGetError() != GL_NO_ERROR) return -1;
  desk_captures++; return 0;
}
int64_t cxgpu_read(int64_t slot, int64_t offset, coax_v_u8 out) {
  if (slot < 0 || slot >= 1024 || !desk_images[slot].pixels || offset < 0 || out.count < 0) return -1;
  size_t size = (size_t)desk_images[slot].w * (size_t)desk_images[slot].h * 4;
  if ((uint64_t)offset > size || (uint64_t)out.count > size - (size_t)offset) return -1;
  memcpy(out.data, desk_images[slot].pixels + offset, (size_t)out.count); return out.count;
}
int64_t cxgpu_drop(int64_t slot) {
  if (slot < 0 || slot >= 1024 || !desk_images[slot].pixels) return -1;
  glDeleteFramebuffers(1, &desk_images[slot].fbo); glDeleteTextures(1, &desk_images[slot].tex);
  desk_unimage(desk_egl, desk_images[slot].image); free(desk_images[slot].pixels);
  memset(&desk_images[slot], 0, sizeof desk_images[slot]); return 0;
}
int64_t cxgpu_stat(int64_t field) {
  if (field == 1) return desk_imports;
  if (field == 2) return desk_captures;
  int n = 0; for (int i = 0; i < 1024; i++) if (desk_images[i].pixels) n++;
  return n;
}
int64_t cxgpu_close(void) {
  for (int i = 0; i < 1024; i++) if (desk_images[i].pixels) cxgpu_drop(i);
  if (desk_ctx != EGL_NO_CONTEXT) fprintf(stderr, "desk GPU closed: live %lld, imports %lld, captures %lld\n",
    (long long)cxgpu_stat(0), (long long)desk_imports, (long long)desk_captures);
  if (desk_egl != EGL_NO_DISPLAY) {
    eglMakeCurrent(desk_egl, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
    if (desk_ctx != EGL_NO_CONTEXT) eglDestroyContext(desk_egl, desk_ctx);
    eglTerminate(desk_egl);
  }
  if (desk_gbm) gbm_device_destroy(desk_gbm);
  if (desk_drm >= 0) close(desk_drm);
  desk_ctx = EGL_NO_CONTEXT; desk_egl = EGL_NO_DISPLAY; desk_gbm = NULL; desk_drm = -1; desk_nformats = 0;
  return 0;
}
#else
int64_t cxgpu_open(coax_v_u8 node) { (void)node; return -1; }
int64_t cxgpu_format(int64_t i, int64_t field) { (void)i; (void)field; return -1; }
int64_t cxgpu_device(void) { return -1; }
int64_t cxgpu_import(int64_t f, int64_t w, int64_t h, int64_t s, int64_t o, int64_t t, int64_t hi, int64_t lo) { (void)f;(void)w;(void)h;(void)s;(void)o;(void)t;(void)hi;(void)lo; return -1; }
int64_t cxgpu_capture(int64_t slot) { (void)slot; return -1; }
int64_t cxgpu_read(int64_t slot, int64_t off, coax_v_u8 out) { (void)slot;(void)off;(void)out; return -1; }
int64_t cxgpu_drop(int64_t slot) { (void)slot; return -1; }
int64_t cxgpu_stat(int64_t field) { (void)field; return 0; }
int64_t cxgpu_close(void) { return 0; }
#endif
#endif
