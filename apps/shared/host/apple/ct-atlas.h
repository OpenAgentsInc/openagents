// Firm reason: 6 (CoreText and AppKit rasterize the Mac's glyphs and icons; every layout decision is Coax).
// The Mac's glyph and icon atlas, from CoreText (components/spec.md, decision 3: the Mac keeps
// CoreText until the Coax rasterizer has font fallback and colour glyphs). It is what
// components/zeron/MetalUI.swift's zeronAtlas and ZeronAtlas.serve were, for the shared Mac host
// (ui-host.m): pixels in memory that the app hands to ui_atlas, not a texture of its own.
//
// Per font, every glyph at four subpixel x offsets and five dilations (gpui_macos
// rasterize_glyph), then every icon at twice its device size, then the glyph cache's cells
// (gpui.coax), which each chat fills as its text first uses a grapheme: mono ones in the mono
// image's last rows, emoji in a colour image. The fonts and icons are the same for every chat, so
// they are drawn once; each chat keeps only its cache rows and its colour image.
#ifndef CT_ATLAS_H
#define CT_ATLAS_H
#include <stdint.h>

// The app's atlas face (apps/openagents-desktop/face.h's zr_ functions have these shapes).
typedef struct {
  int32_t (*glyph_count)(void);
  uint32_t (*glyph_code)(int32_t g);
  int32_t (*font_count)(void);
  float (*font_size)(int32_t slot);
  int32_t (*font_weight)(int32_t slot);   // 0 Geist, 1 Medium, 2 SemiBold, 3 Geist Mono
  int32_t (*icon_count)(void);
  float (*icon_size)(int32_t i);
  int32_t (*icon_name)(int32_t i, uint8_t *out, int32_t cap);
  void (*set_font)(void *state, int32_t slot, const float *header, const float *advance, const float *kern);
  void (*set_icon)(void *state, int32_t i, float x, float y, float w, float h);
  void (*set_atlas)(void *state, float width, float height);
  void (*set_dynamic)(void *state, float size, float pen_x, float pen_y, float y0, int32_t cols, int32_t color_cols, float color_width, float color_height);
  int32_t (*next_cluster)(void *state);
  int32_t (*cluster)(void *state, int32_t slot, uint8_t *out, int32_t cap);
  void (*measured)(void *state, int32_t slot, float advance, int32_t color);
  int32_t (*next_cell)(void *state);
  int32_t (*cell)(void *state, int32_t cell, float *out);
  void (*cell_ready)(void *state, int32_t cell);
} CtApi;

typedef struct CtAtlas CtAtlas;
// The fonts (resources/fonts/Geist*.ttf) and icons (resources/icons/NAME.svg) drawn at a device
// scale. NULL, with why on stderr, if a font is missing.
CtAtlas *ct_atlas_new(const CtApi *api, const char *resources, float dpr);
// A chat's state told where everything is: its fonts' measurements, icons, size and glyph cache.
void ct_atlas_bind(CtAtlas *t, void *state);
float ct_atlas_dpr(const CtAtlas *t);
uint32_t ct_atlas_width(const CtAtlas *t);
uint32_t ct_atlas_height(const CtAtlas *t);
uint32_t ct_atlas_color_height(const CtAtlas *t);
uint32_t ct_atlas_cache_y0(const CtAtlas *t);   // the glyph cache's first row of the mono image
// The mono image's fonts and icons, width by height bytes; its cache rows are zero.
const uint8_t *ct_atlas_mono(const CtAtlas *t);
// What the last frame of a chat asked for: each new grapheme measured, each wanted cell drawn into
// the chat's cache rows (width by height - cache_y0 bytes) or colour image (width by color_height,
// RGBA, straight alpha). How many cells were drawn; the rows they touched are [y0, y1) of the mono
// image and [cy0, cy1) of the colour one (empty when none).
int ct_atlas_serve(CtAtlas *t, void *state, uint8_t *cache, uint8_t *color, int *y0, int *y1, int *cy0, int *cy1);
void ct_atlas_free(CtAtlas *t);
#endif
