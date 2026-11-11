// Firm reason: 6 (CoreText and AppKit rasterize the Mac's glyphs and icons; every layout decision is Coax).
// The Mac's CoreText atlas (ct-atlas.h). Ported from components/zeron/MetalUI.swift (Zeron by Wing,
// MIT, and GPUI's platform rasterization; see components/zeron/NOTICE) with the same cells,
// dilations, subpixel offsets, kerning and icon tiles, so the pictures are the same.
#import <AppKit/AppKit.h>
#import <CoreText/CoreText.h>
#include "ct-atlas.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CT_WIDTH 8192
#define CT_FONTS 16
#define CT_ICONS 64

typedef struct { CTFontRef font; int cell, pen_x, pen_y, y0, cols, mono; CGFloat size; float header[7]; float *advance, *kern; } CtSlot;
struct CtAtlas {
  CtApi api;
  float dpr;
  int glyphs, nfonts, nicons;
  CtSlot slots[CT_FONTS];
  float tiles[CT_ICONS][3];
  int dyn_cell, dyn_pen_x, dyn_pen_y, dyn_y0, dyn_cols, color_cols, height, color_height;
  uint8_t *mono;
};

static CGFontRef ct_geist(const char *resources, int weight) {
  static const char *const names[4] = {"Geist", "Geist-Medium", "Geist-SemiBold", "GeistMono"};
  char path[4096];
  snprintf(path, sizeof path, "%s/fonts/%s.ttf", resources, names[weight < 0 ? 0 : weight > 3 ? 3 : weight]);
  CGDataProviderRef p = CGDataProviderCreateWithFilename(path);
  if (!p) { fprintf(stderr, "ct-atlas: no font at %s\n", path); return NULL; }
  CGFontRef f = CGFontCreateWithDataProvider(p);
  CGDataProviderRelease(p);
  return f;
}
static void ct_quality(CGContextRef g) {
  CGContextSetAllowsAntialiasing(g, true); CGContextSetShouldAntialias(g, true);
  CGContextSetAllowsFontSubpixelPositioning(g, true); CGContextSetShouldSubpixelPositionFonts(g, true);
  CGContextSetAllowsFontSubpixelQuantization(g, false); CGContextSetShouldSubpixelQuantizeFonts(g, false);
  CGContextSetTextDrawingMode(g, kCGTextFill);
}
// gpui_macos rasterize_glyph: alpha only; a dilated level smooths with fill luminance level/4.
static void ct_level(CGContextRef g, int level) {
  if (level > 0) { CGContextSetShouldSmoothFonts(g, true); CGContextSetGrayFillColor(g, (CGFloat)level * 0.25, 1); }
  else { CGContextSetShouldSmoothFonts(g, false); CGContextSetGrayFillColor(g, 0, 1); }
}

CtAtlas *ct_atlas_new(const CtApi *api, const char *resources, float dpr) {
  CtAtlas *t = calloc(1, sizeof *t);
  if (!t) return NULL;
  t->api = *api; t->dpr = dpr;
  t->glyphs = api->glyph_count(); t->nfonts = api->font_count(); t->nicons = api->icon_count();
  if (t->nfonts > CT_FONTS || t->nicons > CT_ICONS || t->glyphs < 95) { free(t); return NULL; }
  int y0 = 0;
  for (int f = 0; f < t->nfonts; f++) {
    CtSlot *s = &t->slots[f];
    s->size = api->font_size(f);
    int weight = api->font_weight(f);
    CGFontRef cg = ct_geist(resources, weight);
    if (!cg) { ct_atlas_free(t); return NULL; }
    s->font = CTFontCreateWithGraphicsFont(cg, s->size, NULL, NULL);
    CGFontRelease(cg);
    s->cell = (int)ceil(s->size * dpr * 2.4); s->cols = CT_WIDTH / s->cell;
    s->pen_x = (int)round((double)s->cell * 0.2); s->pen_y = (int)round((double)s->cell * 0.7);
    s->y0 = y0; s->mono = weight == 3;
    int cells = t->glyphs * 4 * 5;
    y0 += ((cells + s->cols - 1) / s->cols) * s->cell;
  }
  int icon_y = y0, icon_x = 0, row = 0;
  for (int i = 0; i < t->nicons; i++) {
    int px = (int)ceil(api->icon_size(i) * dpr * 2);
    if (icon_x + px > CT_WIDTH) { icon_x = 0; icon_y += row; row = 0; }
    t->tiles[i][0] = (float)icon_x; t->tiles[i][1] = (float)icon_y; t->tiles[i][2] = (float)px;
    icon_x += px + 2; if (px + 2 > row) row = px + 2;
  }
  // the glyph cache's mono cells, sized for the largest font
  t->dyn_cell = 64;
  for (int f = 0; f < t->nfonts; f++) if (f == 0 || t->slots[f].cell > t->dyn_cell) t->dyn_cell = t->slots[f].cell;
  t->dyn_y0 = icon_y + row + 1; t->dyn_cols = CT_WIDTH / t->dyn_cell;
  t->height = t->dyn_y0 + ((2048 + t->dyn_cols - 1) / t->dyn_cols) * t->dyn_cell;
  t->color_cols = CT_WIDTH / t->dyn_cell;
  t->color_height = ((256 + t->color_cols - 1) / t->color_cols) * t->dyn_cell;
  t->dyn_pen_x = (int)round((double)t->dyn_cell * 0.2); t->dyn_pen_y = (int)round((double)t->dyn_cell * 0.7);

  t->mono = calloc((size_t)CT_WIDTH * (size_t)t->height, 1);
  CGColorSpaceRef gray = CGColorSpaceCreateDeviceGray();
  CGContextRef g = t->mono ? CGBitmapContextCreate(t->mono, CT_WIDTH, (size_t)t->height, 8, CT_WIDTH, gray, (CGBitmapInfo)kCGImageAlphaOnly) : NULL;
  CGColorSpaceRelease(gray);
  if (!g) { ct_atlas_free(t); return NULL; }
  int glyphs = t->glyphs;
  UniChar *codes = calloc((size_t)glyphs, sizeof *codes);
  CGGlyph *ids = calloc((size_t)glyphs, sizeof *ids);
  CGSize *adv = calloc((size_t)glyphs, sizeof *adv);
  for (int f = 0; f < t->nfonts; f++) {
    CtSlot *s = &t->slots[f];
    float header[7] = {(float)CTFontGetAscent(s->font), (float)CTFontGetDescent(s->font), (float)s->cell, (float)s->pen_x, (float)s->pen_y, (float)s->y0, (float)s->cols};
    memcpy(s->header, header, sizeof header);
    for (int i = 0; i < glyphs; i++) codes[i] = (UniChar)api->glyph_code(i);
    CTFontGetGlyphsForCharacters(s->font, codes, ids, glyphs);
    CTFontGetAdvancesForGlyphs(s->font, kCTFontOrientationHorizontal, ids, adv, glyphs);
    s->advance = calloc((size_t)glyphs, sizeof(float));
    s->kern = calloc((size_t)glyphs * (size_t)glyphs, sizeof(float));
    for (int i = 0; i < glyphs; i++) s->advance[i] = (float)adv[i].width;
    // Kerning as CoreText shapes it (what GPUI's line layout gets from CTLine): a pair's width less its advances.
    // (a monospaced font has none)
    if (!s->mono) {
      NSDictionary *attrs = @{(__bridge NSString *)kCTFontAttributeName : (__bridge id)s->font};
      for (int a = 1; a < 95 && a < glyphs; a++)
        for (int b = 1; b < 95 && b < glyphs; b++) {
          char pair[3] = {(char)(32 + a), (char)(32 + b), 0};
          NSAttributedString *text = [[NSAttributedString alloc] initWithString:[NSString stringWithUTF8String:pair] attributes:attrs];
          CTLineRef line = CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)text);
          float k = (float)CTLineGetTypographicBounds(line, NULL, NULL, NULL) - s->advance[a] - s->advance[b];
          CFRelease(line);
          if (fabsf(k) > 0.001f) s->kern[a * glyphs + b] = k;
        }
    }
    ct_quality(g);
    CTFontRef font = CTFontCreateCopyWithAttributes(s->font, s->size, NULL, NULL);
    for (int level = 0; level < 5; level++) {
      ct_level(g, level);
      for (int variant = 0; variant < 4; variant++)
        for (int i = 0; i < glyphs; i++) {
          int cell = (level * 4 + variant) * glyphs + i;
          int cx = (cell % s->cols) * s->cell, cy = s->y0 + (cell / s->cols) * s->cell;
          CGContextSaveGState(g);
          CGContextClipToRect(g, CGRectMake(cx, t->height - cy - s->cell, s->cell, s->cell));
          CGContextTranslateCTM(g, cx + s->pen_x, t->height - cy - s->pen_y);
          CGContextScaleCTM(g, dpr, dpr);
          CGGlyph glyph = ids[i];
          CGPoint at = CGPointMake((CGFloat)variant / 4 / dpr, 0);
          CTFontDrawGlyphs(font, &glyph, &at, 1, g);
          CGContextRestoreGState(g);
        }
    }
    CFRelease(font);
  }
  free(codes); free(ids); free(adv);
  CGContextRelease(g);
  // Icons: Zeron's SVGs, their 1em size replaced by the tile's, the alpha kept.
  for (int i = 0; i < t->nicons; i++) {
    uint8_t name[65]; int n = api->icon_name(i, name, 64);
    name[n > 0 ? n : 0] = 0;
    int x = (int)t->tiles[i][0], y = (int)t->tiles[i][1], px = (int)t->tiles[i][2];
    NSString *path = [NSString stringWithFormat:@"%s/icons/%s.svg", resources, (const char *)name];
    NSString *svg = [NSString stringWithContentsOfFile:path encoding:NSUTF8StringEncoding error:nil];
    if (!svg) continue;
    // sized to the tile (a viewBox that isn't square fills it, which lands closest to usvg's pixmap)
    NSRange open = [svg rangeOfString:@"<svg"];
    if (open.location != NSNotFound) {
      NSRange close = [svg rangeOfString:@">" options:0 range:NSMakeRange(NSMaxRange(open), svg.length - NSMaxRange(open))];
      if (close.location != NSNotFound) {
        NSRange headRange = NSMakeRange(open.location, close.location - open.location);
        NSRegularExpression *sizes = [NSRegularExpression regularExpressionWithPattern:@"\\s(width|height)=\"[^\"]*\"" options:0 error:nil];
        NSString *head = [sizes stringByReplacingMatchesInString:[svg substringWithRange:headRange] options:0 range:NSMakeRange(0, headRange.length) withTemplate:@""];
        svg = [svg stringByReplacingCharactersInRange:headRange withString:head];
      }
    }
    svg = [svg stringByReplacingOccurrencesOfString:@"<svg " withString:[NSString stringWithFormat:@"<svg width=\"%d\" height=\"%d\" ", px, px]];
    NSImage *image = [[NSImage alloc] initWithData:[svg dataUsingEncoding:NSUTF8StringEncoding]];
    if (!image) continue;
    CGColorSpaceRef rgb = CGColorSpaceCreateDeviceRGB();
    CGContextRef rgba = CGBitmapContextCreate(NULL, (size_t)px, (size_t)px, 8, (size_t)px * 4, rgb, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
    CGColorSpaceRelease(rgb);
    if (!rgba) continue;
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithCGContext:rgba flipped:NO];
    [image drawInRect:NSMakeRect(0, 0, px, px)];
    [NSGraphicsContext restoreGraphicsState];
    const uint8_t *src = CGBitmapContextGetData(rgba);
    for (int r = 0; r < px; r++)
      for (int c = 0; c < px; c++) t->mono[(size_t)(y + r) * CT_WIDTH + (size_t)(x + c)] = src[((size_t)r * (size_t)px + (size_t)c) * 4 + 3];
    CGContextRelease(rgba);
  }
  return t;
}

void ct_atlas_bind(CtAtlas *t, void *state) {
  for (int f = 0; f < t->nfonts; f++) t->api.set_font(state, f, t->slots[f].header, t->slots[f].advance, t->slots[f].kern);
  for (int i = 0; i < t->nicons; i++) t->api.set_icon(state, i, t->tiles[i][0], t->tiles[i][1], t->tiles[i][2], t->tiles[i][2]);
  t->api.set_atlas(state, CT_WIDTH, (float)t->height);
  t->api.set_dynamic(state, (float)t->dyn_cell, (float)t->dyn_pen_x, (float)t->dyn_pen_y, (float)t->dyn_y0, t->dyn_cols, t->color_cols, CT_WIDTH, (float)t->color_height);
}
float ct_atlas_dpr(const CtAtlas *t) { return t->dpr; }
uint32_t ct_atlas_width(const CtAtlas *t) { (void)t; return CT_WIDTH; }
uint32_t ct_atlas_height(const CtAtlas *t) { return (uint32_t)t->height; }
uint32_t ct_atlas_color_height(const CtAtlas *t) { return (uint32_t)t->color_height; }
uint32_t ct_atlas_cache_y0(const CtAtlas *t) { return (uint32_t)t->dyn_y0; }
const uint8_t *ct_atlas_mono(const CtAtlas *t) { return t->mono; }

// A grapheme as a line in its slot's font, the colour from the context.
static CTLineRef ct_shape(CtAtlas *t, void *state, int slot) {
  uint8_t bytes[64];
  int code = t->api.cluster(state, slot, bytes, sizeof bytes);
  int k = code / 256, n = code % 256;
  if (k < 0) k = 0;
  if (k >= t->nfonts) k = t->nfonts - 1;
  NSString *text = [[NSString alloc] initWithBytes:bytes length:(NSUInteger)n encoding:NSUTF8StringEncoding];
  NSDictionary *attrs = @{(__bridge NSString *)kCTFontAttributeName : (__bridge id)t->slots[k].font,
                          (__bridge NSString *)kCTForegroundColorFromContextAttributeName : @YES};
  NSAttributedString *a = [[NSAttributedString alloc] initWithString:text ? text : @"" attributes:attrs];
  return CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)a);
}
static void ct_draw(CtAtlas *t, CTLineRef line, CGContextRef g, int level, int variant) {
  ct_quality(g); ct_level(g, level);
  CGContextTranslateCTM(g, t->dyn_pen_x, t->dyn_cell - t->dyn_pen_y);
  CGContextScaleCTM(g, t->dpr, t->dpr);
  CGContextSetTextPosition(g, (CGFloat)variant / 4 / t->dpr, 0);
  CTLineDraw(line, g);
}
static void ct_span(int *a, int *b, int y, int h) { if (*b <= *a) { *a = y; *b = y + h; } if (y < *a) *a = y; if (y + h > *b) *b = y + h; }

int ct_atlas_serve(CtAtlas *t, void *state, uint8_t *cache, uint8_t *color, int *y0, int *y1, int *cy0, int *cy1) {
  *y0 = *y1 = *cy0 = *cy1 = 0;
  int drawn = 0;
  @autoreleasepool {
    // each new grapheme's advance, and whether CoreText draws it in colour (Apple Color Emoji)
    for (int guard = 0, slot; guard < 600 && (slot = t->api.next_cluster(state)) >= 0; guard++) {
      CTLineRef line = ct_shape(t, state, slot);
      float width = (float)CTLineGetTypographicBounds(line, NULL, NULL, NULL);
      int colour = 0;
      CFArrayRef runs = CTLineGetGlyphRuns(line);
      for (CFIndex r = 0; r < CFArrayGetCount(runs); r++) {
        CTFontRef f = CFDictionaryGetValue(CTRunGetAttributes(CFArrayGetValueAtIndex(runs, r)), kCTFontAttributeName);
        if (f && (CTFontGetSymbolicTraits(f) & kCTFontTraitColorGlyphs)) colour = 1;
      }
      CFRelease(line);
      t->api.measured(state, slot, width, colour);
    }
    int side = t->dyn_cell;
    float info[6];
    for (int guard = 0, index; guard < 3000 && (index = t->api.next_cell(state)) >= 0; guard++) {
      t->api.cell(state, index, info);
      CTLineRef line = ct_shape(t, state, (int)info[0]);
      int level = (int)info[1], variant = (int)info[2], tx = (int)info[4], ty = (int)info[5];
      if (info[3] > 0.5f) {
        CGColorSpaceRef rgb = CGColorSpaceCreateDeviceRGB();
        CGContextRef g = CGBitmapContextCreate(NULL, (size_t)side, (size_t)side, 8, (size_t)side * 4, rgb, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
        CGColorSpaceRelease(rgb);
        if (g) {
          ct_draw(t, line, g, 0, 0);
          uint8_t *px = CGBitmapContextGetData(g);
          // premultiplied to straight alpha (swap_rgba_pa_to_bgra, in RGBA order here)
          for (int i = 0; i < side * side; i++) {
            int a = px[i * 4 + 3];
            if (a > 0 && a < 255) for (int k = 0; k < 3; k++) { int v = px[i * 4 + k] * 255 / a; px[i * 4 + k] = (uint8_t)(v > 255 ? 255 : v); }
          }
          for (int r = 0; r < side && ty + r < t->color_height; r++)
            if (tx + side <= CT_WIDTH) memcpy(color + ((size_t)(ty + r) * CT_WIDTH + (size_t)tx) * 4, px + (size_t)r * (size_t)side * 4, (size_t)side * 4);
          CGContextRelease(g);
          ct_span(cy0, cy1, ty, side);
        }
      } else {
        CGColorSpaceRef gray = CGColorSpaceCreateDeviceGray();
        CGContextRef g = CGBitmapContextCreate(NULL, (size_t)side, (size_t)side, 8, (size_t)side, gray, (CGBitmapInfo)kCGImageAlphaOnly);
        CGColorSpaceRelease(gray);
        if (g) {
          ct_draw(t, line, g, level, variant);
          const uint8_t *px = CGBitmapContextGetData(g);
          for (int r = 0; r < side && ty + r < t->height; r++)
            if (ty + r >= t->dyn_y0 && tx + side <= CT_WIDTH) memcpy(cache + (size_t)(ty + r - t->dyn_y0) * CT_WIDTH + (size_t)tx, px + (size_t)r * (size_t)side, (size_t)side);
          CGContextRelease(g);
          ct_span(y0, y1, ty, side);
        }
      }
      CFRelease(line);
      t->api.cell_ready(state, index);
      drawn++;
    }
  }
  if (*y1 > t->height) *y1 = t->height;
  if (*cy1 > t->color_height) *cy1 = t->color_height;
  return drawn;
}

void ct_atlas_free(CtAtlas *t) {
  if (!t) return;
  for (int f = 0; f < CT_FONTS; f++) { if (t->slots[f].font) CFRelease(t->slots[f].font); free(t->slots[f].advance); free(t->slots[f].kern); }
  free(t->mono);
  free(t);
}
