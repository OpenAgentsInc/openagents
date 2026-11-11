// Firm reason: 6 (the hosts are C; the ring is six vertices appended after the app's own).
// The keyboard focus ring (GitHub #13182): a host that moves focus with Tab (the Mac's, apple/ui-host.m)
// draws this ring around the focused row of the app's element table, after the app's vertices, so every
// app on the shared hosts gets the same ring without drawing it itself. It is one GPUI quad in the
// components' vertex format (components/backend/gpu.coax paint_quad: 28 floats a vertex, kind 0, a clear
// fill and a border in the ring's colour), so it goes through the same shader on every target; the Linux
// and Android hosts include this header when they move focus by key.
#ifndef UI_FOCUS_RING_H
#define UI_FOCUS_RING_H
#include <math.h>
#include <stdint.h>

#define UI_RING_FLOATS 28
#define UI_RING_OUTSET 3.0f   // logical px outside the row's box
#define UI_RING_WIDTH 2.0f    // logical px
// The ring's colour when the app gives none (UiApp.ring): Zeron's accent (components/skin/zeron.coax).
static const float ui_ring_accent[4] = {0x8b / 255.0f, 0x7c / 255.0f, 0xf6 / 255.0f, 1};

// The ring around the box x y w h (logical px, the view's top left), at device scale d, written from
// vertex at; the count after it (at when it does not fit in max vertices). rgba alpha 0: the accent.
static inline uint32_t ui_focus_ring(float *v, uint32_t at, uint32_t max, float d, float x, float y, float w, float h, const float rgba[4]) {
  if (!v || at + 6 > max || w <= 0 || h <= 0) return at;
  const float *c = rgba && rgba[3] > 0 ? rgba : ui_ring_accent;
  float x0 = roundf((x - UI_RING_OUTSET) * d), y0 = roundf((y - UI_RING_OUTSET) * d);
  float x1 = roundf((x + w + UI_RING_OUTSET) * d), y1 = roundf((y + h + UI_RING_OUTSET) * d);
  float border = fmaxf(1, roundf(UI_RING_WIDTH * d));
  // the corner follows the row's height (a button's small corner, the composer's round one), outside it by the outset
  const float xs[6] = {x0, x1, x1, x0, x1, x0}, ys[6] = {y0, y0, y1, y0, y1, y1};
  for (int k = 0; k < 6; k++) {
    float *o = v + (size_t)(at + (uint32_t)k) * UI_RING_FLOATS;
    for (int i = 0; i < UI_RING_FLOATS; i++) o[i] = 0;
    o[0] = xs[k]; o[1] = ys[k];   // o[2] kind 0: a quad; o[3] no blur
    o[4] = x0; o[5] = y0; o[6] = x1 - x0; o[7] = y1 - y0; o[8] = (h * 0.25f + UI_RING_OUTSET) * d; o[9] = border;
    o[16] = c[0]; o[17] = c[1]; o[18] = c[2]; o[19] = c[3];   // the edge; the fill (12..15) stays clear
    o[20] = -1000000.0f; o[21] = -1000000.0f; o[22] = 1000000.0f; o[23] = 1000000.0f;   // no clip
  }
  return at + 6;
}
#endif
