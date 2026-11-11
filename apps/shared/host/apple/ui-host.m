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

@interface UiView : NSView <NSTextInputClient>
@property(nonatomic, assign) UiHost *host;
@property(nonatomic, strong) NSString *marked;
@end

@implementation UiView
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)wantsUpdateLayer { return YES; }
- (CALayer *)makeBackingLayer { return self.host->layer; }
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
- (void)mouseDown:(NSEvent *)e { [self pointer:e phase:0]; }
- (void)mouseDragged:(NSEvent *)e { [self pointer:e phase:1]; }
- (void)mouseMoved:(NSEvent *)e { if (!self.host->held) [self pointer:e phase:1]; }
- (void)mouseUp:(NSEvent *)e { [self pointer:e phase:2]; }
- (void)scrollWheel:(NSEvent *)e {
  UiHost *h = self.host;
  if (!h->app.wheel) return;
  CGFloat dy = e.hasPreciseScrollingDeltas ? e.scrollingDeltaY : e.scrollingDeltaY * 16;
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
- (void)keyDown:(NSEvent *)e {
  UiHost *h = self.host;
  int mods = ui_mods_of(e.modifierFlags);
  NSString *bare = e.charactersIgnoringModifiers;
  int named = ui_named(e.keyCode, bare.length ? [bare characterAtIndex:0] : 0);
  if (h->app.key && self.marked.length == 0) {
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
- (void)insertText:(id)text replacementRange:(NSRange)r {
  (void)r;
  NSString *s = [text isKindOfClass:[NSAttributedString class]] ? [text string] : text;
  self.marked = nil;
  const char *u = s.UTF8String;
  if (self.host->app.text && u) self.host->app.text(self.host->app.user, u, strlen(u));
}
- (void)setMarkedText:(id)text selectedRange:(NSRange)s replacementRange:(NSRange)r {
  (void)s; (void)r;
  self.marked = [text isKindOfClass:[NSAttributedString class]] ? [text string] : text;
}
- (void)unmarkText { self.marked = nil; }
- (NSRange)selectedRange { return NSMakeRange(NSNotFound, 0); }
- (NSRange)markedRange { return self.marked.length ? NSMakeRange(0, self.marked.length) : NSMakeRange(NSNotFound, 0); }
- (BOOL)hasMarkedText { return self.marked.length > 0; }
- (NSAttributedString *)attributedSubstringForProposedRange:(NSRange)r actualRange:(NSRangePointer)a { (void)r; (void)a; return nil; }
- (NSArray<NSAttributedStringKey> *)validAttributesForMarkedText { return @[]; }
- (NSUInteger)characterIndexForPoint:(NSPoint)p { (void)p; return NSNotFound; }
// Where the text box is, for the input method's window.
- (NSRect)firstRectForCharacterRange:(NSRange)r actualRange:(NSRangePointer)a {
  (void)r; (void)a;
  UiHost *h = self.host;
  float rect[4] = {0};
  if (!h->app.text_rect || !h->app.text_rect(h->app.user, rect)) return NSZeroRect;
  NSRect v = NSMakeRect(rect[0] / h->dpr, self.bounds.size.height - (rect[1] + rect[3]) / h->dpr, rect[2] / h->dpr, rect[3] / h->dpr);
  return [self.window convertRectToScreen:[self convertRect:v toView:nil]];
}
@end

@interface UiWindowDelegate : NSObject <NSWindowDelegate>
@property(nonatomic, assign) UiHost *host;
@end
@implementation UiWindowDelegate
- (void)windowWillClose:(NSNotification *)n { (void)n; self.host->closed = 1; }
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
static id<MTLRenderPipelineState> ui_pipeline(id<MTLDevice> dev, id<MTLLibrary> lib) {
  MTLRenderPipelineDescriptor *p = [MTLRenderPipelineDescriptor new];
  p.vertexFunction = [lib newFunctionWithName:@"chat_fx_vs"];
  p.fragmentFunction = [lib newFunctionWithName:@"chat_fx_fs"];
  if (!p.vertexFunction || !p.fragmentFunction) { fprintf(stderr, "ui-host: the shader has no chat_fx_vs and chat_fx_fs\n"); return nil; }
  MTLVertexDescriptor *v = [MTLVertexDescriptor new];
  for (int i = 0; i < 7; i++) { v.attributes[i].format = MTLVertexFormatFloat4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 30; }
  v.layouts[30].stride = UI_VERTEX_BYTES;
  p.vertexDescriptor = v;
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
// One frame into a texture: the background is the shell's colour, then the vertices.
static void ui_draw(UiHost *h, id<MTLTexture> into, id<MTLCommandBuffer> cmd) {
  MTLRenderPassDescriptor *rp = [MTLRenderPassDescriptor renderPassDescriptor];
  rp.colorAttachments[0].texture = into;
  rp.colorAttachments[0].loadAction = MTLLoadActionClear;
  rp.colorAttachments[0].storeAction = MTLStoreActionStore;
  rp.colorAttachments[0].clearColor = MTLClearColorMake(0, 0, 0, 1);
  id<MTLRenderCommandEncoder> e = [cmd renderCommandEncoderWithDescriptor:rp];
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
  NSCursor *c = kind == 1 ? NSCursor.IBeamCursor : kind == 2 ? NSCursor.pointingHandCursor : kind == 3 ? NSCursor.resizeLeftRightCursor : NSCursor.arrowCursor;
  [c set];
}
int ui_copy(UiHost *h, const char *bytes, size_t n) {
  if (h->offscreen) return 0;
  NSString *s = [[NSString alloc] initWithBytes:bytes length:n encoding:NSUTF8StringEncoding];
  if (!s) return 0;
  [NSPasteboard.generalPasteboard clearContents];
  return [NSPasteboard.generalPasteboard setString:s forType:NSPasteboardTypeString] ? 1 : 0;
}
int ui_mods(UiHost *h) { return h->offscreen ? 0 : ui_mods_of(NSEvent.modifierFlags); }
int ui_focused(UiHost *h) { return h->offscreen ? 0 : (h->window.isKeyWindow && NSApp.isActive); }
#else
void ui_cursor(UiHost *h, int kind) { h->cursor_kind = kind; }   // a touch screen has no pointer to shape
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
  if (!lib || !(h->pipe = ui_pipeline(h->dev, lib))) { free(h); return NULL; }
  h->queue = [h->dev newCommandQueue];
  h->verts = [h->dev newBufferWithLength:(NSUInteger)app->max_vertices * UI_VERTEX_BYTES options:MTLResourceStorageModeShared];
  h->frame = malloc((size_t)app->max_vertices * UI_VERTEX_BYTES);
  if (!h->verts || !h->frame) { fprintf(stderr, "ui-host: no memory for a frame's vertices\n"); free(h->frame); free(h); return NULL; }
#if TARGET_OS_OSX
  if (!offscreen) {
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    NSRect r = NSMakeRect(0, 0, app->width, app->height);
    h->window = [[NSWindow alloc] initWithContentRect:r styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
                                              backing:NSBackingStoreBuffered defer:NO];
    h->window.title = [NSString stringWithUTF8String:app->title ? app->title : ""];
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
    [h->window center];
    [h->window makeKeyAndOrderFront:nil];
    [NSApp finishLaunching];
    [NSApp activateIgnoringOtherApps:YES];
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
    NSDate *until = h->stale ? NSDate.distantPast : [NSDate dateWithTimeIntervalSinceNow:1 / 60.0];
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
    if (!h->stale && digest == h->shown) return 1;
    id<CAMetalDrawable> d = [h->layer nextDrawable];
    if (!d) return 1;
    h->shown = digest; h->stale = 0;
    id<MTLCommandBuffer> cmd = [h->queue commandBuffer];
    ui_draw(h, d.texture, cmd);
    [cmd presentDrawable:d];
    [cmd commit];
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
#if TARGET_OS_OSX
  if (h->window) { h->window.delegate = nil; [h->window close]; }
  h->window = nil;
#else
  [h->view removeFromSuperview];
#endif
  // ARC keeps the struct's object fields: let them go before the memory does
  h->view = nil; h->layer = nil; h->pipe = nil; h->queue = nil;
  h->mono = nil; h->color = nil; h->target = nil; h->verts = nil; h->dev = nil;
  free(h->frame);
  free(h);
}
