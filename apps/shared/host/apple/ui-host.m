// Firm reason: 6 (AppKit and Metal are Objective-C APIs; every app's behaviour is Coax).
// The shared Apple host (the Mac, and iOS) for apps drawn with the component system
// (components/spec.md, docs/apps/2026-11-10-apps-roadmap.md, GitHub #13156): the same contract
// as the Linux one (../linux/ui-host.h). On the Mac: an NSWindow with a CAMetalLayer, AppKit's keyboard (NSTextInputClient,
// so an input method's marked text and its window work) and pointer, and one pipeline that draws
// the components' vertices (28 floats each, components/zeron/gpui.coax) with the component
// shader, components/backend/gpu-shader.coax emitted as Metal (`coax metal --vertex chat_fx_vs
// --fragment chat_fx_fs`; its bindings: the vertices at buffer 30, ChatU's floats at 0, the
// lengths at 29, the atlases at textures 0 and 1), over the atlas the app hands it.
//
// UiApp.shader is the emitted Metal source (.metal) or a compiled library (.metallib).
// Offscreen (ui_open(app, 1)) it makes no window, no Dock icon and no menu: ui_shot_frame and
// ui_shot_write only.
//
// On iOS an app owns its UIApplication and window, so ui_open(app, 0) makes a view
// (ui-host-apple.h: ui_view) the app puts in its window; touches are the pointer (the first
// finger), the app's display link calls ui_frame, and the rest is the same as the Mac's.
#include <TargetConditionals.h>
#if TARGET_OS_OSX
#import <AppKit/AppKit.h>
#else
#import <UIKit/UIKit.h>
#endif
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <ImageIO/ImageIO.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#import <objc/runtime.h>
#include "../linux/ui-host.h"
#include "ui-host-apple.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define UI_VERTEX_BYTES 112   /* 28 floats: components/zeron/gpui.coax GP_VERTEX_FLOATS */

@class UiView;
struct UiHost {
  UiApp app;
  int offscreen, closed;
  id<MTLDevice> dev;
  id<MTLCommandQueue> queue;
  id<MTLRenderPipelineState> pipe;
  id<MTLTexture> mono, color, target;
  id<MTLBuffer> verts;
  float *frame;
  uint32_t count, w, rows;
  float dpr;
  double last;
  uint64_t shown; int stale;
#if TARGET_OS_OSX
  NSWindow *window;
#endif
  UiView *view;
  CAMetalLayer *layer;
  int cursor_kind;
  // the pointer: double and triple clicks are AppKit's clickCount
  int held;
  uint64_t presented;
#if TARGET_OS_OSX
  NSPasteboard *pasteboard;   // the general one; offscreen, the app's own
  id menu_target;             // the menus' items send their ids through it
  NSEventModifierFlags flags; // the modifiers last reported as keys (UiApp.modifier_keys)
  NSView *root;               // with native children (ui_child): the content view holding them and the host's view
  int under;                  // children under the components: the layer is clear where nothing is drawn
  NSMutableArray *observers;  // UiApp.life's notifications
#endif
  int looking;                // ui_hold_pointer
  // the app's own layer (ui_layer)
  UiLayer layer_of; int layered;
  id<MTLLibrary> lib;
  id<MTLTexture> depth;
  id<MTLDepthStencilState> flat;
};

static double ui_now(void) { return CACurrentMediaTime(); }
#if TARGET_OS_OSX
static int ui_mods_of(NSEventModifierFlags f) {
  return ((f & NSEventModifierFlagShift) ? 1 : 0) | ((f & NSEventModifierFlagControl) ? 2 : 0) |
         ((f & NSEventModifierFlagOption) ? 4 : 0) | ((f & NSEventModifierFlagCommand) ? 8 : 0);
}
// AppKit's function keys to the contract's named keys (ui-host.h).
static int ui_named(unsigned short code, unichar c) {
  switch (code) {
    case 53: return -1; case 36: case 76: return -2; case 51: return -3;
    case 123: return -4; case 124: return -5; case 126: return -6; case 125: return -7;
    case 115: return -8; case 119: return -9; case 116: return -10; case 121: return -11; case 117: return -12; case 48: return -15;
  }
  (void)c;
  return 0;
}

// ---- the view: input and the drawable

static int ui_paste_from(UiHost *h, NSPasteboard *pb);

@interface UiView : NSView <NSTextInputClient>
@property(nonatomic, assign) UiHost *host;
@property(nonatomic, strong) NSString *marked;
@end

@implementation UiView
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)wantsUpdateLayer { return YES; }
- (CALayer *)makeBackingLayer { return self.host->layer; }
// With native views under the components, a point the app leaves goes on to them.
- (NSView *)hitTest:(NSPoint)p {
  UiHost *h = self.host;
  NSView *hit = [super hitTest:p];
  if (hit != self || !h->under || !h->app.hit) return hit;
  NSPoint q = [self convertPoint:p fromView:self.superview];
  return h->app.hit(h->app.user, (float)(q.x * h->dpr), (float)((self.bounds.size.height - q.y) * h->dpr)) ? self : nil;
}
- (NSPoint)devicePoint:(NSEvent *)e {
  NSPoint p = [self convertPoint:e.locationInWindow fromView:nil];
  return NSMakePoint(p.x * self.host->dpr, (self.bounds.size.height - p.y) * self.host->dpr);
}
- (void)pointer:(NSEvent *)e phase:(int)phase {
  UiHost *h = self.host;
  if (!h->app.pointer) return;
  NSPoint p = [self devicePoint:e];
  if (phase == 0) h->held = 1;
  if (phase == 2) h->held = 0;
  h->app.pointer(h->app.user, (float)p.x, (float)p.y, phase, e.clickCount > 0 ? (int)MIN(e.clickCount, 3) : 1, ui_mods_of(e.modifierFlags));
}
- (void)motion:(NSEvent *)e { UiHost *h = self.host; if (h->app.motion) h->app.motion(h->app.user, (float)e.deltaX, (float)e.deltaY); }
- (void)mouseDown:(NSEvent *)e { [self pointer:e phase:0]; }
- (void)mouseDragged:(NSEvent *)e { [self motion:e]; [self pointer:e phase:1]; }
- (void)mouseMoved:(NSEvent *)e { [self motion:e]; if (!self.host->held) [self pointer:e phase:1]; }
- (void)mouseUp:(NSEvent *)e { [self pointer:e phase:2]; }
// The other buttons (UiApp.button): 2 the right, 1 the middle.
- (void)other:(NSEvent *)e phase:(int)phase button:(int)b {
  UiHost *h = self.host;
  if (!h->app.button) return;
  NSPoint p = [self devicePoint:e];
  h->app.button(h->app.user, (float)p.x, (float)p.y, phase, b, ui_mods_of(e.modifierFlags));
}
- (void)rightMouseDown:(NSEvent *)e { if (self.host->app.button) [self other:e phase:0 button:2]; else [super rightMouseDown:e]; }
- (void)rightMouseDragged:(NSEvent *)e { [self motion:e]; [self other:e phase:1 button:2]; }
- (void)rightMouseUp:(NSEvent *)e { [self other:e phase:2 button:2]; }
- (void)otherMouseDown:(NSEvent *)e { [self other:e phase:0 button:1]; }
- (void)otherMouseDragged:(NSEvent *)e { [self motion:e]; [self other:e phase:1 button:1]; }
- (void)otherMouseUp:(NSEvent *)e { [self other:e phase:2 button:1]; }
// Shift, Control and Alt as keys of their own (UiApp.modifier_keys).
- (void)flagsChanged:(NSEvent *)e {
  UiHost *h = self.host;
  if (!h->app.modifier_keys || !h->app.key) { [super flagsChanged:e]; return; }
  NSEventModifierFlags now = e.modifierFlags & (NSEventModifierFlagShift | NSEventModifierFlagControl | NSEventModifierFlagOption);
  const NSEventModifierFlags flag[3] = {NSEventModifierFlagShift, NSEventModifierFlagControl, NSEventModifierFlagOption};
  const int key[3] = {-13, -14, -16};
  for (int i = 0; i < 3; i++)
    if ((now & flag[i]) != (h->flags & flag[i])) h->app.key(h->app.user, key[i], ui_mods_of(e.modifierFlags), (now & flag[i]) ? 1 : 0);
  h->flags = now;
}
- (void)scrollWheel:(NSEvent *)e {
  UiHost *h = self.host;
  if (h->app.scroll) { h->app.scroll(h->app.user, (float)e.scrollingDeltaY, e.hasPreciseScrollingDeltas ? 1 : 0); return; }
  if (!h->app.wheel) return;
  // GPUI's pixel delta: precise deltas as they come, a mouse's lines times the line height (22.75)
  CGFloat dy = e.hasPreciseScrollingDeltas ? e.scrollingDeltaY : e.scrollingDeltaY * 22.75;
  h->app.wheel(h->app.user, (float)(-dy * h->dpr));
}
- (void)updateTrackingAreas {
  for (NSTrackingArea *a in self.trackingAreas) [self removeTrackingArea:a];
  [self addTrackingArea:[[NSTrackingArea alloc] initWithRect:self.bounds options:NSTrackingMouseMoved | NSTrackingActiveInKeyWindow | NSTrackingInVisibleRect | NSTrackingCursorUpdate owner:self userInfo:nil]];
  [super updateTrackingAreas];
}
- (void)cursorUpdate:(NSEvent *)e { (void)e; ui_cursor(self.host, self.host->cursor_kind); }
// A key: the app's chords first (named keys and anything with Command or Control); the rest goes
// through the input method, which calls insertText or setMarkedText.
- (BOOL)composing {
  UiHost *h = self.host;
  return h->app.ime ? h->app.ime->range(h->app.user, 2) >= 0 : self.marked.length > 0;
}
- (void)keyDown:(NSEvent *)e {
  UiHost *h = self.host;
  int mods = ui_mods_of(e.modifierFlags);
  NSString *bare = e.charactersIgnoringModifiers;
  int named = ui_named(e.keyCode, bare.length ? [bare characterAtIndex:0] : 0);
  if (h->app.key && ![self composing]) {
    if (named && h->app.key(h->app.user, named, mods, 1)) return;
    if (!named && (mods & (2 | 8)) && bare.length) {
      int c = (int)[bare characterAtIndex:0];
      if (c >= 'A' && c <= 'Z') c += 32;
      if (h->app.key(h->app.user, c, mods, 1)) return;
    }
  }
  if (h->app.wants_text && h->app.wants_text(h->app.user)) [self interpretKeyEvents:@[ e ]];
  else if (h->app.key && bare.length) h->app.key(h->app.user, (int)[bare characterAtIndex:0], mods, 1);
}
- (void)keyUp:(NSEvent *)e {
  UiHost *h = self.host;
  NSString *bare = e.charactersIgnoringModifiers;
  int named = ui_named(e.keyCode, 0);
  if (h->app.key) h->app.key(h->app.user, named ? named : (bare.length ? (int)[bare characterAtIndex:0] : 0), ui_mods_of(e.modifierFlags), 0);
}
- (void)doCommandBySelector:(SEL)s { (void)s; }
// The input method. With UiApp.ime the text box's own ranges answer it (marked text is the app's);
// without, marked text is held here and only committed text reaches the app.
static int ui_from(NSRange r) { return r.location == NSNotFound ? -1 : (int)r.location; }
static int ui_to(NSRange r) { return r.location == NSNotFound ? -1 : (int)(r.location + r.length); }
- (void)insertText:(id)text replacementRange:(NSRange)r {
  UiHost *h = self.host;
  NSString *s = [text isKindOfClass:[NSAttributedString class]] ? [text string] : text;
  self.marked = nil;
  const char *u = s.UTF8String;
  if (!u) return;
  if (h->app.ime) h->app.ime->replace(h->app.user, ui_from(r), ui_to(r), u, strlen(u));
  else if (h->app.text) h->app.text(h->app.user, u, strlen(u));
}
- (void)setMarkedText:(id)text selectedRange:(NSRange)s replacementRange:(NSRange)r {
  UiHost *h = self.host;
  NSString *t = [text isKindOfClass:[NSAttributedString class]] ? [text string] : text;
  if (!h->app.ime) { self.marked = t; return; }
  const char *u = t.UTF8String;
  h->app.ime->mark(h->app.user, ui_from(r), ui_to(r), u ? u : "", u ? strlen(u) : 0, ui_from(s), ui_to(s));
}
- (void)unmarkText { UiHost *h = self.host; self.marked = nil; if (h->app.ime) h->app.ime->unmark(h->app.user); }
- (NSRange)selectedRange {
  UiHost *h = self.host;
  if (!h->app.ime) return NSMakeRange(NSNotFound, 0);
  int a = h->app.ime->range(h->app.user, 0), b = h->app.ime->range(h->app.user, 1);
  return NSMakeRange((NSUInteger)a, (NSUInteger)(b - a));
}
- (NSRange)markedRange {
  UiHost *h = self.host;
  if (!h->app.ime) return self.marked.length ? NSMakeRange(0, self.marked.length) : NSMakeRange(NSNotFound, 0);
  int a = h->app.ime->range(h->app.user, 2), b = h->app.ime->range(h->app.user, 3);
  return a < 0 ? NSMakeRange(NSNotFound, 0) : NSMakeRange((NSUInteger)a, (NSUInteger)(b - a));
}
- (BOOL)hasMarkedText { return [self composing]; }
- (NSAttributedString *)attributedSubstringForProposedRange:(NSRange)r actualRange:(NSRangePointer)actual {
  UiHost *h = self.host;
  if (!h->app.ime) return nil;
  int total = h->app.ime->range(h->app.user, 4);
  int a = (int)MIN((NSUInteger)total, r.location), b = (int)MIN((NSUInteger)total, r.location + r.length);
  if (a > b) return nil;
  char bytes[8192];
  int n = h->app.ime->text(h->app.user, a, b, bytes, (int)sizeof bytes);
  if (actual) *actual = NSMakeRange((NSUInteger)a, (NSUInteger)(b - a));
  NSString *s = [[NSString alloc] initWithBytes:bytes length:(NSUInteger)(n > 0 ? n : 0) encoding:NSUTF8StringEncoding];
  return [[NSAttributedString alloc] initWithString:s ? s : @""];
}
- (NSArray<NSAttributedStringKey> *)validAttributesForMarkedText { return @[]; }
- (NSUInteger)characterIndexForPoint:(NSPoint)p {
  UiHost *h = self.host;
  if (!h->app.ime || !self.window) return NSNotFound;
  NSPoint v = [self convertPoint:[self.window convertPointFromScreen:p] fromView:nil];
  int i = h->app.ime->index(h->app.user, (float)(v.x * h->dpr), (float)((self.bounds.size.height - v.y) * h->dpr));
  return i < 0 ? NSNotFound : (NSUInteger)i;
}
// Where the text box is, for the input method's window.
- (NSRect)firstRectForCharacterRange:(NSRange)r actualRange:(NSRangePointer)a {
  UiHost *h = self.host;
  float rect[4] = {0};
  if (h->app.ime) {
    if (!h->app.ime->rect(h->app.user, r.location == NSNotFound ? 0 : (int)r.location, rect)) return NSZeroRect;
    if (a) *a = r;
  } else if (!h->app.text_rect || !h->app.text_rect(h->app.user, rect)) return NSZeroRect;
  NSRect v = NSMakeRect(rect[0] / h->dpr, self.bounds.size.height - (rect[1] + rect[3]) / h->dpr, rect[2] / h->dpr, rect[3] / h->dpr);
  return [self.window convertRectToScreen:[self convertRect:v toView:nil]];
}
// Files dropped on the window go to UiApp.file.
- (NSDragOperation)draggingEntered:(id<NSDraggingInfo>)s { (void)s; return self.host->app.file ? NSDragOperationCopy : NSDragOperationNone; }
- (BOOL)performDragOperation:(id<NSDraggingInfo>)s { return ui_paste_from(self.host, s.draggingPasteboard) > 0; }
@end

// The menus' items: each sends its id to the app, which says whether it can run and is checked.
@interface UiMenuTarget : NSObject <NSMenuItemValidation>
@property(nonatomic, assign) UiHost *host;
@end
@implementation UiMenuTarget
- (void)command:(NSMenuItem *)item { UiHost *h = self.host; if (h->app.command) h->app.command(h->app.user, (int)item.tag); }
- (BOOL)validateMenuItem:(NSMenuItem *)item {
  UiHost *h = self.host;
  if (!h->app.command_state) return YES;
  int st = h->app.command_state(h->app.user, (int)item.tag);
  item.state = (st & 2) ? NSControlStateValueOn : NSControlStateValueOff;
  return (st & 1) != 0;
}
@end

// The window: every event goes to UiApp.event first.
@interface UiWindow : NSWindow
@property(nonatomic, assign) UiHost *host;
@end
@implementation UiWindow
- (void)sendEvent:(NSEvent *)e {
  UiHost *h = self.host;
  if (h && h->app.event && h->app.event(h->app.user, (__bridge void *)e)) return;
  [super sendEvent:e];
}
- (BOOL)canBecomeKeyWindow { return self.host && self.host->app.hidden ? NO : [super canBecomeKeyWindow]; }
@end

@interface UiWindowDelegate : NSObject <NSWindowDelegate>
@property(nonatomic, assign) UiHost *host;
@end
@implementation UiWindowDelegate
- (void)windowWillClose:(NSNotification *)n { (void)n; self.host->closed = 1; }
- (void)windowDidBecomeKey:(NSNotification *)n { (void)n; UiHost *h = self.host; if (h->app.focus) h->app.focus(h->app.user, 1); }
- (void)windowDidResignKey:(NSNotification *)n {
  (void)n; UiHost *h = self.host;
  ui_hold_pointer(h, 0);
  if (h->app.focus) h->app.focus(h->app.user, 0);
}
- (void)life:(int)event { UiHost *h = self.host; if (h->app.life) h->app.life(h->app.user, event); }
- (void)windowDidMiniaturize:(NSNotification *)n { (void)n; [self life:UI_LIFE_MINIMIZED]; }
- (void)windowDidDeminiaturize:(NSNotification *)n { (void)n; [self life:UI_LIFE_RESTORED]; }
- (void)windowDidChangeOcclusionState:(NSNotification *)n { (void)n; [self life:UI_LIFE_OCCLUSION]; }
@end
#else
// iOS: the view the app puts in its window. Its layer is the CAMetalLayer frames are presented to.
@interface UiView : UIView
@property(nonatomic, assign) UiHost *host;
@end
@implementation UiView
+ (Class)layerClass { return [CAMetalLayer class]; }
- (void)pointer:(NSSet<UITouch *> *)touches phase:(int)phase {
  UiHost *h = self.host;
  UITouch *t = touches.anyObject;
  if (!h->app.pointer || !t) return;
  CGPoint p = [t locationInView:self];
  h->app.pointer(h->app.user, (float)(p.x * h->dpr), (float)(p.y * h->dpr), phase, (int)MIN(MAX(t.tapCount, 1), 3), 0);
}
- (void)touchesBegan:(NSSet<UITouch *> *)t withEvent:(UIEvent *)e { (void)e; [self pointer:t phase:0]; }
- (void)touchesMoved:(NSSet<UITouch *> *)t withEvent:(UIEvent *)e { (void)e; [self pointer:t phase:1]; }
- (void)touchesEnded:(NSSet<UITouch *> *)t withEvent:(UIEvent *)e { (void)e; [self pointer:t phase:2]; }
- (void)touchesCancelled:(NSSet<UITouch *> *)t withEvent:(UIEvent *)e { (void)e; [self pointer:t phase:2]; }
@end
#endif

// ---- the pipeline

static id<MTLLibrary> ui_library(id<MTLDevice> dev, const char *path) {
  NSString *p = [NSString stringWithUTF8String:path];
  NSError *err = nil;
  id<MTLLibrary> lib = nil;
  if ([p hasSuffix:@".metallib"]) lib = [dev newLibraryWithURL:[NSURL fileURLWithPath:p] error:&err];
  else {
    NSString *src = [NSString stringWithContentsOfFile:p encoding:NSUTF8StringEncoding error:&err];
    if (src) lib = [dev newLibraryWithSource:src options:nil error:&err];
  }
  if (!lib) fprintf(stderr, "ui-host: no shader at %s (%s)\n", path, err ? err.localizedDescription.UTF8String : "unreadable");
  return lib;
}
static id<MTLRenderPipelineState> ui_pipeline(id<MTLDevice> dev, id<MTLLibrary> lib, int depth) {
  MTLRenderPipelineDescriptor *p = [MTLRenderPipelineDescriptor new];
  p.vertexFunction = [lib newFunctionWithName:@"chat_fx_vs"];
  p.fragmentFunction = [lib newFunctionWithName:@"chat_fx_fs"];
  if (!p.vertexFunction || !p.fragmentFunction) { fprintf(stderr, "ui-host: the shader has no chat_fx_vs and chat_fx_fs\n"); return nil; }
  MTLVertexDescriptor *v = [MTLVertexDescriptor new];
  for (int i = 0; i < 7; i++) { v.attributes[i].format = MTLVertexFormatFloat4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 30; }
  v.layouts[30].stride = UI_VERTEX_BYTES;
  p.vertexDescriptor = v;
  if (depth) p.depthAttachmentPixelFormat = MTLPixelFormatDepth32Float;
  MTLRenderPipelineColorAttachmentDescriptor *a = p.colorAttachments[0];
  a.pixelFormat = MTLPixelFormatBGRA8Unorm; a.blendingEnabled = YES;
  a.rgbBlendOperation = MTLBlendOperationAdd; a.alphaBlendOperation = MTLBlendOperationAdd;
  a.sourceRGBBlendFactor = MTLBlendFactorSourceAlpha; a.sourceAlphaBlendFactor = MTLBlendFactorOne;
  a.destinationRGBBlendFactor = MTLBlendFactorOneMinusSourceAlpha; a.destinationAlphaBlendFactor = MTLBlendFactorOneMinusSourceAlpha;
  NSError *err = nil;
  id<MTLRenderPipelineState> s = [dev newRenderPipelineStateWithDescriptor:p error:&err];
  if (!s) fprintf(stderr, "ui-host: %s\n", err.localizedDescription.UTF8String);
  return s;
}
void ui_atlas(UiHost *h, const uint8_t *mono, uint32_t w, uint32_t rows, const uint8_t *color, uint32_t cw, uint32_t crows) {
  if (!h->mono || h->mono.width != w || h->mono.height != rows) {
    MTLTextureDescriptor *d = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatR8Unorm width:w height:rows mipmapped:NO];
    h->mono = [h->dev newTextureWithDescriptor:d];
  }
  if (!h->color || h->color.width != cw || h->color.height != crows) {
    MTLTextureDescriptor *d = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatRGBA8Unorm width:cw height:crows mipmapped:NO];
    h->color = [h->dev newTextureWithDescriptor:d];
  }
  [h->mono replaceRegion:MTLRegionMake2D(0, 0, w, rows) mipmapLevel:0 withBytes:mono bytesPerRow:w];
  [h->color replaceRegion:MTLRegionMake2D(0, 0, cw, crows) mipmapLevel:0 withBytes:color bytesPerRow:(NSUInteger)cw * 4];
  h->stale = 1;
}
void ui_atlas_rows(UiHost *h, const uint8_t *mono, uint32_t y0, uint32_t y1, const uint8_t *color, uint32_t cy0, uint32_t cy1) {
  if (h->mono && y1 > y0 && y1 <= h->mono.height) {
    NSUInteger w = h->mono.width;
    [h->mono replaceRegion:MTLRegionMake2D(0, y0, w, y1 - y0) mipmapLevel:0 withBytes:mono + (size_t)y0 * w bytesPerRow:w];
    h->stale = 1;
  }
  if (h->color && cy1 > cy0 && cy1 <= h->color.height) {
    NSUInteger w = h->color.width;
    [h->color replaceRegion:MTLRegionMake2D(0, cy0, w, cy1 - cy0) mipmapLevel:0 withBytes:color + (size_t)cy0 * w * 4 bytesPerRow:w * 4];
    h->stale = 1;
  }
}
static uint64_t ui_digest(const UiHost *h) {
  const uint64_t *words = (const uint64_t *)h->frame;
  size_t n = (size_t)h->count * UI_VERTEX_BYTES / 8;
  uint64_t d = 0x9e3779b97f4a7c15ull ^ h->count ^ ((uint64_t)h->w << 32) ^ ((uint64_t)h->rows << 48);
  for (size_t i = 0; i < n; i++) { d ^= words[i]; d *= 0xff51afd7ed558ccdull; d ^= d >> 29; }
  return d;
}
static void ui_fill(UiHost *h, float dt) {
  int n = h->app.frame ? h->app.frame(h->app.user, h->frame, (float)h->w, (float)h->rows, h->dpr, dt) : 0;
  if (n < 0) n = 0;
  if ((uint32_t)n > h->app.max_vertices) n = (int)h->app.max_vertices;
  h->count = (uint32_t)n;
}
void ui_layer(UiHost *h, const UiLayer *layer) {
  int depth = layer && layer->depth;
  if (depth != (h->layered && h->layer_of.depth)) {
    id<MTLRenderPipelineState> p = ui_pipeline(h->dev, h->lib, depth);
    if (p) h->pipe = p;
  }
  if (layer) { h->layer_of = *layer; h->layered = 1; } else { memset(&h->layer_of, 0, sizeof h->layer_of); h->layered = 0; }
  if (!h->flat) {
    MTLDepthStencilDescriptor *d = [MTLDepthStencilDescriptor new];
    d.depthCompareFunction = MTLCompareFunctionAlways; d.depthWriteEnabled = NO;
    h->flat = [h->dev newDepthStencilStateWithDescriptor:d];
  }
  h->stale = 1;
}
void *ui_device(UiHost *h) { return (__bridge void *)h->dev; }
void *ui_queue(UiHost *h) { return (__bridge void *)h->queue; }
// One frame into a texture: the background is the shell's colour, the app's own layer, then the vertices.
static void ui_draw(UiHost *h, id<MTLTexture> into, id<MTLCommandBuffer> cmd) {
  int depth = h->layered && h->layer_of.depth;
  if (h->layered && h->layer_of.prepare) h->layer_of.prepare(h->layer_of.user, (__bridge void *)cmd, h->w, h->rows, h->dpr);
  MTLRenderPassDescriptor *rp = [MTLRenderPassDescriptor renderPassDescriptor];
  rp.colorAttachments[0].texture = into;
  rp.colorAttachments[0].loadAction = MTLLoadActionClear;
  rp.colorAttachments[0].storeAction = MTLStoreActionStore;
#if TARGET_OS_OSX
  rp.colorAttachments[0].clearColor = MTLClearColorMake(0, 0, 0, h->under ? 0 : 1);
#else
  rp.colorAttachments[0].clearColor = MTLClearColorMake(0, 0, 0, 1);
#endif
  if (depth) {
    if (!h->depth || h->depth.width != into.width || h->depth.height != into.height) {
      MTLTextureDescriptor *d = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatDepth32Float width:into.width height:into.height mipmapped:NO];
      d.usage = MTLTextureUsageRenderTarget; d.storageMode = MTLStorageModePrivate;
      h->depth = [h->dev newTextureWithDescriptor:d];
    }
    rp.depthAttachment.texture = h->depth;
    rp.depthAttachment.loadAction = MTLLoadActionClear; rp.depthAttachment.clearDepth = 1;
    rp.depthAttachment.storeAction = MTLStoreActionDontCare;
  }
  id<MTLRenderCommandEncoder> e = [cmd renderCommandEncoderWithDescriptor:rp];
  if (h->layered && h->layer_of.encode) {
    h->layer_of.encode(h->layer_of.user, (__bridge void *)e, h->w, h->rows, h->dpr);
    if (h->flat) [e setDepthStencilState:h->flat];
  }
  if (h->count && h->pipe && h->mono) {
    memcpy(h->verts.contents, h->frame, (size_t)h->count * UI_VERTEX_BYTES);
    float u[4] = {(float)h->w, (float)h->rows, 0, 0};
    uint32_t lengths[29] = {0}; lengths[0] = 4;
    [e setRenderPipelineState:h->pipe];
    [e setVertexBuffer:h->verts offset:0 atIndex:30];
    [e setVertexBytes:u length:sizeof u atIndex:0]; [e setFragmentBytes:u length:sizeof u atIndex:0];
    [e setVertexBytes:lengths length:sizeof lengths atIndex:29]; [e setFragmentBytes:lengths length:sizeof lengths atIndex:29];
    [e setFragmentTexture:h->mono atIndex:0]; [e setFragmentTexture:h->color atIndex:1];
    [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:h->count];
  }
  [e endEncoding];
}

// ---- the seat

#if TARGET_OS_OSX
void ui_cursor(UiHost *h, int kind) {
  h->cursor_kind = kind;
  if (h->offscreen) return;
  NSCursor *c = kind == 1 ? NSCursor.IBeamCursor : kind == 2 ? NSCursor.pointingHandCursor : kind == 3 ? NSCursor.resizeLeftRightCursor : kind == 4 ? NSCursor.crosshairCursor : NSCursor.arrowCursor;
  [c set];
}
void ui_hold_pointer(UiHost *h, int on) {
  on = on ? 1 : 0;
  if (h->offscreen || on == h->looking) { h->looking = on; return; }
  h->looking = on;
  CGAssociateMouseAndMouseCursorPosition(on ? 0 : 1);
  if (on) [NSCursor hide]; else [NSCursor unhide];
}
int ui_copy(UiHost *h, const char *bytes, size_t n) {
  NSString *s = [[NSString alloc] initWithBytes:bytes length:n encoding:NSUTF8StringEncoding];
  if (!s) return 0;
  [h->pasteboard clearContents];
  return [h->pasteboard setString:s forType:NSPasteboardTypeString] ? 1 : 0;
}
int ui_mods(UiHost *h) { return h->offscreen ? 0 : ui_mods_of(NSEvent.modifierFlags); }
int ui_focused(UiHost *h) { return h->offscreen ? 0 : (h->window.isKeyWindow && NSApp.isActive); }
int ui_paste_text(UiHost *h, char *out, size_t cap) {
  NSString *s = [h->pasteboard stringForType:NSPasteboardTypeString];
  const char *u = s.UTF8String;
  if (!u || !cap) return 0;
  size_t n = strlen(u); if (n >= cap) n = cap - 1;
  memcpy(out, u, n); out[n] = 0;
  return (int)n;
}
static int ui_paste_from(UiHost *h, NSPasteboard *pb) {
  if (!h->app.file) return 0;
  NSArray<NSURL *> *urls = [pb readObjectsForClasses:@[ NSURL.class ] options:@{NSPasteboardURLReadingFileURLsOnlyKey : @YES}];
  int n = 0;
  for (NSURL *u in urls) { h->app.file(h->app.user, 0, u.path.fileSystemRepresentation); n++; }
  if (n) return n;
  // an image with no file: written as a PNG, then given like a file
  NSData *png = [pb dataForType:NSPasteboardTypePNG];
  if (!png) {
    NSData *tiff = [pb dataForType:NSPasteboardTypeTIFF];
    NSBitmapImageRep *rep = tiff ? [NSBitmapImageRep imageRepWithData:tiff] : nil;
    png = rep ? [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] : nil;
  }
  if (!png) return 0;
  NSString *dir = [NSTemporaryDirectory() stringByAppendingPathComponent:[NSUUID UUID].UUIDString];
  [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:nil error:nil];
  NSString *path = [dir stringByAppendingPathComponent:@"pasted-image.png"];
  if (![png writeToFile:path atomically:YES]) return 0;
  h->app.file(h->app.user, 0, path.fileSystemRepresentation);
  return 1;
}
int ui_paste_files(UiHost *h) { return ui_paste_from(h, h->pasteboard); }

static NSEventModifierFlags ui_flags(int mods) {
  return ((mods & 1) ? NSEventModifierFlagShift : 0) | ((mods & 2) ? NSEventModifierFlagControl : 0) |
         ((mods & 4) ? NSEventModifierFlagOption : 0) | ((mods & 8) ? NSEventModifierFlagCommand : 0);
}
static NSMenuItem *ui_item(UiHost *h, const UiMenuItem *it) {
  if (!it->title) return [NSMenuItem separatorItem];
  NSString *title = [NSString stringWithUTF8String:it->title];
  if (!strcmp(it->title, "Services")) {
    NSMenuItem *i = [[NSMenuItem alloc] initWithTitle:title action:nil keyEquivalent:@""];
    NSMenu *services = [[NSMenu alloc] initWithTitle:title];
    i.submenu = services;
    if (!h->offscreen) NSApp.servicesMenu = services;
    return i;
  }
  SEL action = it->selector ? NSSelectorFromString([NSString stringWithUTF8String:it->selector]) : (it->id >= 0 ? @selector(command:) : nil);
  NSMenuItem *i = [[NSMenuItem alloc] initWithTitle:title action:action keyEquivalent:[NSString stringWithUTF8String:it->key ? it->key : ""]];
  i.keyEquivalentModifierMask = ui_flags(it->mods);
  i.tag = it->id;
  if (!it->selector && it->id >= 0) i.target = h->menu_target;
  return i;
}
void ui_menus(UiHost *h, const UiMenuItem *items, int n) {
  if (h->offscreen) return;
  NSMenu *bar = [NSMenu new];
  for (int k = 0; k < n;) {
    const char *name = items[k].menu;
    NSMenuItem *top = [NSMenuItem new];
    NSMenu *sub = [[NSMenu alloc] initWithTitle:[NSString stringWithUTF8String:name]];
    for (; k < n && !strcmp(items[k].menu, name); k++) [sub addItem:ui_item(h, &items[k])];
    top.submenu = sub;
    [bar addItem:top];
    if (!strcmp(name, "Window")) NSApp.windowsMenu = sub;
  }
  NSApp.mainMenu = bar;
}
void ui_menu_key(UiHost *h, int id, const char *key, int mods) {
  if (h->offscreen) return;
  for (NSMenuItem *top in NSApp.mainMenu.itemArray)
    for (NSMenuItem *i in top.submenu.itemArray)
      if (i.target == h->menu_target && i.tag == id) { i.keyEquivalent = [NSString stringWithUTF8String:key]; i.keyEquivalentModifierMask = ui_flags(mods); }
}
void ui_popup(UiHost *h, const UiMenuItem *items, int n, float x, float y, int checked) {
  if (h->offscreen || !h->view) return;
  NSMenu *m = [NSMenu new];
  m.autoenablesItems = NO;
  for (int k = 0; k < n; k++) {
    NSMenuItem *i = ui_item(h, &items[k]);
    if (items[k].title && items[k].id < 0) i.enabled = NO;
    if (items[k].title && items[k].id == checked) i.state = NSControlStateValueOn;
    [m addItem:i];
  }
  [m popUpMenuPositioningItem:nil atLocation:NSMakePoint(x / h->dpr, h->view.bounds.size.height - y / h->dpr) inView:h->view];
}
void ui_choose(UiHost *h, int request, int folders, int many, const char *message, const char *prompt, const char *start) {
  if (h->offscreen || !h->app.file) return;
  NSOpenPanel *p = [NSOpenPanel openPanel];
  p.canChooseFiles = !folders; p.canChooseDirectories = folders != 0; p.allowsMultipleSelection = many != 0;
  p.canCreateDirectories = folders != 0;
  if (message) p.message = [NSString stringWithUTF8String:message];
  if (prompt) p.prompt = [NSString stringWithUTF8String:prompt];
  if (start && *start) p.directoryURL = [NSURL fileURLWithPath:[NSString stringWithUTF8String:start]];
  void (^done)(NSModalResponse) = ^(NSModalResponse r) {
    if (r != NSModalResponseOK) return;
    for (NSURL *u in p.URLs) h->app.file(h->app.user, request, u.path.fileSystemRepresentation);
  };
  if (h->window.isVisible) [p beginSheetModalForWindow:h->window completionHandler:done];
  else done([p runModal]);
}
int ui_window_frame(UiHost *h, float f[4]) {
  if (h->offscreen || !h->window) return 0;
  NSRect r = h->window.frame;
  f[0] = (float)r.origin.x; f[1] = (float)r.origin.y; f[2] = (float)r.size.width; f[3] = (float)r.size.height;
  return 1;
}
void ui_quit(UiHost *h) { h->closed = 1; }
void ui_child(UiHost *h, void *view, int above) {
  if (h->offscreen || !view) return;
  NSView *v = (__bridge NSView *)view;
  if (!h->root) {
    // the content view becomes a plain view holding the host's and the app's
    h->root = [[NSView alloc] initWithFrame:h->view.frame];
    h->root.autoresizesSubviews = YES;
    h->window.contentView = h->root;
    h->view.frame = h->root.bounds; h->view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    [h->root addSubview:h->view];
    [h->window makeFirstResponder:h->view];
  }
  [h->root addSubview:v positioned:above ? NSWindowAbove : NSWindowBelow relativeTo:above ? nil : h->view];
  if (!above && !h->under) { h->under = 1; h->layer.opaque = NO; h->stale = 1; }
}
void *ui_window(UiHost *h) { return (__bridge void *)h->window; }
void ui_menu_title(UiHost *h, int id, const char *title) {
  if (h->offscreen || !title) return;
  for (NSMenuItem *top in NSApp.mainMenu.itemArray)
    for (NSMenuItem *it in top.submenu.itemArray)
      if (it.target == h->menu_target && it.tag == id) it.title = [NSString stringWithUTF8String:title];
}
void ui_open_url(const char *url) {
  NSURL *u = [NSURL URLWithString:[NSString stringWithUTF8String:url]];
  if (u) [NSWorkspace.sharedWorkspace openURL:u];
}
void ui_terminal(const char *command) {
  NSString *c = [[NSString stringWithUTF8String:command] stringByReplacingOccurrencesOfString:@"\"" withString:@"\\\""];
  NSString *script = [NSString stringWithFormat:@"tell application \"Terminal\"\nactivate\ndo script \"%@\"\nend tell", c];
  [[[NSAppleScript alloc] initWithSource:script] executeAndReturnError:nil];
}
#else
void ui_menus(UiHost *h, const UiMenuItem *items, int n) { (void)h; (void)items; (void)n; }
void ui_menu_key(UiHost *h, int id, const char *key, int mods) { (void)h; (void)id; (void)key; (void)mods; }
void ui_popup(UiHost *h, const UiMenuItem *items, int n, float x, float y, int checked) { (void)h; (void)items; (void)n; (void)x; (void)y; (void)checked; }
int ui_paste_text(UiHost *h, char *out, size_t cap) {
  (void)h;
  const char *u = UIPasteboard.generalPasteboard.string.UTF8String;
  if (!u || !cap) return 0;
  size_t n = strlen(u); if (n >= cap) n = cap - 1;
  memcpy(out, u, n); out[n] = 0;
  return (int)n;
}
int ui_paste_files(UiHost *h) { (void)h; return 0; }
void ui_choose(UiHost *h, int request, int folders, int many, const char *message, const char *prompt, const char *start) {
  (void)h; (void)request; (void)folders; (void)many; (void)message; (void)prompt; (void)start;
}
int ui_window_frame(UiHost *h, float f[4]) { (void)h; (void)f; return 0; }
void ui_quit(UiHost *h) { h->closed = 1; }
void ui_open_url(const char *url) {
  NSURL *u = [NSURL URLWithString:[NSString stringWithUTF8String:url]];
  if (u) [UIApplication.sharedApplication openURL:u options:@{} completionHandler:nil];
}
void ui_terminal(const char *command) { (void)command; }
void ui_child(UiHost *h, void *view, int above) { (void)h; (void)view; (void)above; }
void *ui_window(UiHost *h) { (void)h; return NULL; }
void ui_menu_title(UiHost *h, int id, const char *title) { (void)h; (void)id; (void)title; }
void ui_cursor(UiHost *h, int kind) { h->cursor_kind = kind; }   // a touch screen has no pointer to shape
void ui_hold_pointer(UiHost *h, int on) { h->looking = on ? 1 : 0; }
int ui_copy(UiHost *h, const char *bytes, size_t n) {
  if (h->offscreen) return 0;
  NSString *s = [[NSString alloc] initWithBytes:bytes length:n encoding:NSUTF8StringEncoding];
  if (!s) return 0;
  UIPasteboard.generalPasteboard.string = s;
  return 1;
}
int ui_mods(UiHost *h) { (void)h; return 0; }
int ui_focused(UiHost *h) { return h->offscreen ? 0 : h->view.window.isKeyWindow; }
#endif
void *ui_view(UiHost *h) { return (__bridge void *)h->view; }
uint64_t ui_presented(UiHost *h) { return h->presented; }
// Two pictures' pixels as RGBX, w by rows.
static uint8_t *ui_pixels(const char *path, size_t *w, size_t *rows) {
  CGImageSourceRef src = CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:[NSString stringWithUTF8String:path]], NULL);
  CGImageRef img = src ? CGImageSourceCreateImageAtIndex(src, 0, NULL) : NULL;
  if (src) CFRelease(src);
  if (!img) return NULL;
  *w = CGImageGetWidth(img); *rows = CGImageGetHeight(img);
  uint8_t *px = calloc(*w * *rows, 4);
  CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
  CGContextRef g = px ? CGBitmapContextCreate(px, *w, *rows, 8, *w * 4, cs, (CGBitmapInfo)kCGImageAlphaNoneSkipLast) : NULL;
  if (g) { CGContextDrawImage(g, CGRectMake(0, 0, *w, *rows), img); CGContextRelease(g); } else { free(px); px = NULL; }
  CGColorSpaceRelease(cs); CGImageRelease(img);
  return px;
}
int ui_compare(const char *a, const char *b, double limit) {
  size_t aw = 0, ar = 0, bw = 0, br = 0;
  uint8_t *pa = ui_pixels(a, &aw, &ar), *pb = ui_pixels(b, &bw, &br);
  int status = 1;
  if (!pa || !pb) printf("compare: cannot read %s or %s\n", a, b);
  else if (aw != bw || ar != br) printf("compare: %zux%zu against %zux%zu\n", aw, ar, bw, br);
  else {
    size_t over = 0; int most = 0;
    for (size_t p = 0; p < aw * ar; p++) {
      int worst = 0;
      for (int c = 0; c < 3; c++) { int d = abs((int)pa[p * 4 + c] - (int)pb[p * 4 + c]); if (d > worst) worst = d; }
      if (worst > most) most = worst;
      if (worst > 48) over++;
    }
    double percent = 100.0 * (double)over / (double)(aw * ar);
    printf("%zu of %zu pixels differ by more than 48 (%.4f%%, at most %d)\n", over, aw * ar, percent, most);
    status = percent > limit ? 1 : 0;
  }
  free(pa); free(pb);
  return status;
}
float ui_dpr(UiHost *h) { return h->dpr; }
void ui_set_dpr(UiHost *h, float dpr) { if (dpr > 0) h->dpr = dpr; }

// ---- open, frame, close

UiHost *ui_open(const UiApp *app, int offscreen) {
  UiHost *h = calloc(1, sizeof *h);
  if (!h) return NULL;
  h->app = *app; h->offscreen = offscreen; h->dpr = 1;
  h->dev = MTLCreateSystemDefaultDevice();
  if (!h->dev) { fprintf(stderr, "ui-host: no Metal device\n"); free(h); return NULL; }
  id<MTLLibrary> lib = ui_library(h->dev, app->shader);
  if (!lib || !(h->pipe = ui_pipeline(h->dev, lib, 0))) { free(h); return NULL; }
  h->lib = lib;
  h->queue = [h->dev newCommandQueue];
  h->verts = [h->dev newBufferWithLength:(NSUInteger)app->max_vertices * UI_VERTEX_BYTES options:MTLResourceStorageModeShared];
  h->frame = malloc((size_t)app->max_vertices * UI_VERTEX_BYTES);
  if (!h->verts || !h->frame) { fprintf(stderr, "ui-host: no memory for a frame's vertices\n"); free(h->frame); free(h); return NULL; }
#if TARGET_OS_OSX
  // a picture's Copy and Paste use a pasteboard of the app's own, never the one you copy to
  h->pasteboard = offscreen ? [NSPasteboard pasteboardWithName:[NSString stringWithFormat:@"%s.shot", app->app_id ? app->app_id : "ui-host"]] : NSPasteboard.generalPasteboard;
  UiMenuTarget *target = [UiMenuTarget new]; target.host = h; h->menu_target = target;
  if (!offscreen) {
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:app->hidden ? NSApplicationActivationPolicyProhibited : NSApplicationActivationPolicyRegular];
    NSRect r = NSMakeRect(0, 0, app->width, app->height);
    NSWindowStyleMask style = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable;
    if (app->chrome) style |= NSWindowStyleMaskFullSizeContentView;
    UiWindow *win = [[UiWindow alloc] initWithContentRect:r styleMask:style backing:NSBackingStoreBuffered defer:NO];
    win.host = h; h->window = win;
    h->window.title = [NSString stringWithUTF8String:app->title ? app->title : ""];
    if (app->chrome) { h->window.titlebarAppearsTransparent = YES; h->window.titleVisibility = NSWindowTitleHidden; }
    h->window.backgroundColor = [NSColor colorWithRed:13 / 255.0 green:13 / 255.0 blue:13 / 255.0 alpha:1];
    if (app->min_width && app->min_height) h->window.minSize = NSMakeSize(app->min_width, app->min_height);
    h->window.releasedWhenClosed = NO;
    UiWindowDelegate *wd = [UiWindowDelegate new]; wd.host = h;
    h->window.delegate = wd;
    objc_setAssociatedObject(h->window, "ui-host", wd, OBJC_ASSOCIATION_RETAIN);
    h->layer = [CAMetalLayer layer];
    h->layer.device = h->dev; h->layer.pixelFormat = MTLPixelFormatBGRA8Unorm; h->layer.framebufferOnly = YES;
    h->view = [[UiView alloc] initWithFrame:r];
    h->view.host = h; h->view.wantsLayer = YES;
    h->window.contentView = h->view;
    [h->window makeFirstResponder:h->view];
    h->window.acceptsMouseMovedEvents = YES;
    if (app->file) [h->view registerForDraggedTypes:@[ NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeTIFF ]];
    [h->window center];
    // the frame it had at quit, kept on a screen
    if (app->place[2] >= (app->min_width ? app->min_width : 1) && app->place[3] >= (app->min_height ? app->min_height : 1))
      [h->window setFrame:[h->window constrainFrameRect:NSMakeRect(app->place[0], app->place[1], app->place[2], app->place[3]) toScreen:NSScreen.mainScreen] display:NO];
    if (!app->hidden) [h->window makeKeyAndOrderFront:nil];
    [NSApp finishLaunching];
    if (!app->hidden) [NSApp activateIgnoringOtherApps:YES];
    if (app->life) {
      h->observers = [NSMutableArray array];
      NSString *names[5] = {NSApplicationDidBecomeActiveNotification, NSApplicationDidResignActiveNotification, NSApplicationDidHideNotification, NSApplicationDidUnhideNotification, NSApplicationWillTerminateNotification};
      const int events[5] = {UI_LIFE_ACTIVE, UI_LIFE_INACTIVE, UI_LIFE_HIDDEN, UI_LIFE_SHOWN, UI_LIFE_QUIT};
      for (int i = 0; i < 5; i++) {
        int event = events[i];
        [h->observers addObject:[NSNotificationCenter.defaultCenter addObserverForName:names[i] object:nil queue:nil usingBlock:^(NSNotification *n) {
          (void)n; if (h->app.life) h->app.life(h->app.user, event);
        }]];
      }
    }
    h->dpr = (float)h->window.backingScaleFactor;
  }
#else
  if (!offscreen) {
    h->view = [[UiView alloc] initWithFrame:CGRectMake(0, 0, app->width, app->height)];
    h->view.host = h; h->view.multipleTouchEnabled = NO;
    h->layer = (CAMetalLayer *)h->view.layer;
    h->layer.device = h->dev; h->layer.pixelFormat = MTLPixelFormatBGRA8Unorm; h->layer.framebufferOnly = YES;
    h->dpr = (float)UIScreen.mainScreen.scale;
    h->view.contentScaleFactor = h->dpr;
  }
#endif
  h->last = ui_now();
  return h;
}
int ui_frame(UiHost *h) {
  if (h->offscreen) return 0;
  @autoreleasepool {
#if TARGET_OS_OSX
    // the seat's events, without waiting: a frame's time at most when nothing changed
    int every = h->layered && !h->layer_of.changed;   // a layer that changes every frame (a world)
    NSDate *until = h->stale || every ? NSDate.distantPast : [NSDate dateWithTimeIntervalSinceNow:1 / 60.0];
    NSEvent *e;
    while ((e = [NSApp nextEventMatchingMask:NSEventMaskAny untilDate:until inMode:NSDefaultRunLoopMode dequeue:YES])) {
      [NSApp sendEvent:e];
      until = NSDate.distantPast;
    }
    if (h->closed) return 0;
    h->dpr = (float)h->window.backingScaleFactor;
#else
    // iOS: UIKit delivers the touches; this is one frame, called by the app's display link
    h->dpr = (float)h->view.contentScaleFactor;
#endif
    CGSize s = h->view.bounds.size;
    uint32_t w = (uint32_t)lround(s.width * h->dpr), rows = (uint32_t)lround(s.height * h->dpr);
    if (!w || !rows) return 1;
    if (w != h->w || rows != h->rows) { h->w = w; h->rows = rows; h->layer.contentsScale = h->dpr; h->layer.drawableSize = CGSizeMake(w, rows); h->stale = 1; }
    double now = ui_now();
    float dt = (float)(now - h->last); h->last = now;
    if (dt > 0.1f) dt = 0.1f;
    ui_fill(h, dt);
    uint64_t digest = ui_digest(h);
#if TARGET_OS_OSX
    int moved = every || (h->layered && h->layer_of.changed(h->layer_of.user));
#else
    int moved = h->layered && (!h->layer_of.changed || h->layer_of.changed(h->layer_of.user));
#endif
    if (!h->stale && !moved && digest == h->shown) return 1;
    id<CAMetalDrawable> d = [h->layer nextDrawable];
    if (!d) return 1;
    h->shown = digest; h->stale = 0;
    id<MTLCommandBuffer> cmd = [h->queue commandBuffer];
    ui_draw(h, d.texture, cmd);
    [cmd presentDrawable:d];
    [cmd commit];
    h->presented++;
  }
  return 1;
}
int ui_shot_frame(UiHost *h, uint32_t w, uint32_t rows, float dt) {
  if (!h->offscreen || !w || !rows) return 0;
  @autoreleasepool {
    if (!h->target || h->w != w || h->rows != rows) {
      MTLTextureDescriptor *d = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm width:w height:rows mipmapped:NO];
      d.usage = MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead;
      d.storageMode = MTLStorageModeShared;
      h->target = [h->dev newTextureWithDescriptor:d];
      h->w = w; h->rows = rows;
    }
    ui_fill(h, dt);
    id<MTLCommandBuffer> cmd = [h->queue commandBuffer];
    ui_draw(h, h->target, cmd);
    [cmd commit];
    [cmd waitUntilCompleted];
  }
  return 1;
}
int ui_shot_write(UiHost *h, const char *png) {
  if (!h->target) return 0;
  int ok = 0;
  @autoreleasepool {
    size_t w = h->w, rows = h->rows;
    uint8_t *px = malloc(w * rows * 4);
    if (!px) return 0;
    [h->target getBytes:px bytesPerRow:w * 4 fromRegion:MTLRegionMake2D(0, 0, w, rows) mipmapLevel:0];
    CGColorSpaceRef cs = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef g = CGBitmapContextCreate(px, w, rows, 8, w * 4, cs, kCGImageAlphaNoneSkipFirst | kCGBitmapByteOrder32Little);
    CGImageRef img = g ? CGBitmapContextCreateImage(g) : NULL;
    CFURLRef url = (__bridge CFURLRef)[NSURL fileURLWithPath:[NSString stringWithUTF8String:png]];
    CGImageDestinationRef dst = img ? CGImageDestinationCreateWithURL(url, (__bridge CFStringRef)UTTypePNG.identifier, 1, NULL) : NULL;
    if (dst) { CGImageDestinationAddImage(dst, img, NULL); ok = CGImageDestinationFinalize(dst) ? 1 : 0; CFRelease(dst); }
    if (img) CGImageRelease(img);
    if (g) CGContextRelease(g);
    CGColorSpaceRelease(cs);
    free(px);
  }
  return ok;
}
void ui_close(UiHost *h) {
  if (!h) return;
  ui_hold_pointer(h, 0);
#if TARGET_OS_OSX
  for (id o in h->observers) [NSNotificationCenter.defaultCenter removeObserver:o];
  h->observers = nil;
  if (h->window) { h->window.delegate = nil; ((UiWindow *)h->window).host = NULL; [h->window close]; }
  h->window = nil; h->root = nil;
#else
  [h->view removeFromSuperview];
#endif
  // ARC keeps the struct's object fields: let them go before the memory does
  h->view = nil; h->layer = nil; h->pipe = nil; h->queue = nil;
#if TARGET_OS_OSX
  h->pasteboard = nil; h->menu_target = nil;
#endif
  h->mono = nil; h->color = nil; h->target = nil; h->verts = nil; h->dev = nil;
  h->lib = nil; h->depth = nil; h->flat = nil;
  free(h->frame);
  free(h);
}
