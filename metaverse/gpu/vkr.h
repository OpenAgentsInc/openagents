// A realm on the GPU (docs/verse/native-gpu.md, G2): its two passes from
// `coax spirv` (the fill and the strokes), stepped on wall time, and the
// frame intervals a host reports. A window (vk-window.c) and a display
// session (vk-display.c) present it.
#ifndef VKR_H
#define VKR_H
#include "realm.h"
#include "vkc.h"

#define VKR_FRAMES (60 * 60 * 10)

typedef struct {
  void *state;
  float *vertices, *u;
  int vf, nu, maxv;
  VkcBuffer vb;
  const char *fill_u, *stroke_u;
  float fill_push[32], stroke_push[32];
  VkcPass passes[2];
  // Per frame: when it was presented, and where its time went.
  double presented[VKR_FRAMES], waited[VKR_FRAMES], stepped[VKR_FRAMES], acquiring[VKR_FRAMES];
  int frames;
} Vkr;

// FILL_U and STROKE_U list the realm uniforms each pass's procedures take
// after their own parameters (comma-separated indices, `-` for none); the
// stroke pass gets the width and height first.
void vkr_open(Vkc *v, Vkr *r, const char *fill_spv, const char *fill_u, const char *stroke_spv, const char *stroke_u);
// One step of dt seconds at w by h: the vertices and push constants for the passes.
void vkr_step(Vkr *r, uint32_t w, uint32_t h, float scale, float dt);
void vkr_close(Vkc *v, Vkr *r);
// Intervals past one and a half periods after the first `skip`, the longest
// in *longest; the first ten are printed, with where their time went when
// `detail` is set (the presents carry it).
int vkr_drops(const char *host, const char *what, const Vkr *r, const double *t, int n, int skip, double period, double *longest, int detail);
#endif
