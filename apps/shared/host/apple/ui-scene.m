// Firm reason: 6 (Metal pipeline and buffer objects are Objective-C APIs; the scene's shaders and numbers are Coax).
// See ui-scene.h.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#include "ui-scene.h"
#include <stdlib.h>
#include <string.h>

@interface UiSceneState : NSObject {
 @public
  UiSceneApp app;
  UiHost *host;
  id<MTLRenderPipelineState> background, strokes;
  id<MTLBuffer> vertices;
  float *uniforms;
  int count, ready, failed, closed, dirty, installed;
  uint32_t w, rows;
  double last;
  uint64_t frames;
}
@end
@implementation UiSceneState
- (void)dealloc { free(uniforms); }
@end

static UiSceneState *ui_scene_of(void *p) { return (__bridge UiSceneState *)p; }
static int ui_scene_moving(UiSceneState *s) { return s->app.moving ? s->app.moving(s->app.user) != 0 : 1; }
static int ui_scene_changed(void *user) {
  UiSceneState *s = ui_scene_of(user);
  return s->ready && !s->closed && (s->dirty || ui_scene_moving(s));
}
static void ui_scene_encode(void *user, void *encoder, uint32_t w, uint32_t rows, float dpr) {
  UiSceneState *s = ui_scene_of(user);
  if (!s->ready || s->closed) return;
  id<MTLRenderCommandEncoder> e = (__bridge id<MTLRenderCommandEncoder>)encoder;
  int moving = ui_scene_moving(s);
  if (s->dirty || moving || w != s->w || rows != s->rows) {
    double now = CACurrentMediaTime();
    float dt = moving && s->last > 0 ? (float)(now - s->last) : 0;
    if (dt > 0.1f) dt = 0.1f;
    s->last = now;
    int n = s->app.frame ? s->app.frame(s->app.user, (float *)s->vertices.contents, s->uniforms, (float)w, (float)rows, dpr, dt) : 0;
    s->count = n < 0 ? 0 : (uint32_t)n > s->app.max_vertices ? (int)s->app.max_vertices : n;
    s->w = w; s->rows = rows; s->dirty = 0; s->frames++;
  }
  uint32_t lengths[29] = {0}; lengths[0] = s->app.uniform_floats;
  NSUInteger bytes = (NSUInteger)s->app.uniform_floats * sizeof(float);
  [e setVertexBytes:s->uniforms length:bytes atIndex:0]; [e setFragmentBytes:s->uniforms length:bytes atIndex:0];
  [e setVertexBytes:lengths length:sizeof lengths atIndex:29]; [e setFragmentBytes:lengths length:sizeof lengths atIndex:29];
  [e setRenderPipelineState:s->background];
  [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
  if (s->count > 0) {
    [e setRenderPipelineState:s->strokes];
    [e setVertexBuffer:s->vertices offset:0 atIndex:30];
    [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:(NSUInteger)s->count];
  }
}
static id<MTLRenderPipelineState> ui_scene_pipeline(id<MTLDevice> dev, NSString *source, NSString *vs, NSString *fs, uint32_t vertex_floats, BOOL lighten) {
  NSError *err = nil;
  id<MTLLibrary> lib = [dev newLibraryWithSource:source options:nil error:&err];
  if (!lib) { fprintf(stderr, "ui-scene: %s\n", err.localizedDescription.UTF8String); return nil; }
  MTLRenderPipelineDescriptor *p = [MTLRenderPipelineDescriptor new];
  p.vertexFunction = [lib newFunctionWithName:vs]; p.fragmentFunction = [lib newFunctionWithName:fs];
  if (!p.vertexFunction || !p.fragmentFunction) { fprintf(stderr, "ui-scene: the shader has no %s and %s\n", vs.UTF8String, fs.UTF8String); return nil; }
  if (vertex_floats) {
    MTLVertexDescriptor *v = [MTLVertexDescriptor new];
    for (uint32_t i = 0; i < (vertex_floats + 3) / 4; i++) { v.attributes[i].format = MTLVertexFormatFloat4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 30; }
    v.layouts[30].stride = vertex_floats * sizeof(float);
    p.vertexDescriptor = v;
  }
  MTLRenderPipelineColorAttachmentDescriptor *a = p.colorAttachments[0];
  a.pixelFormat = MTLPixelFormatBGRA8Unorm;
  if (lighten) {
    a.blendingEnabled = YES;
    a.rgbBlendOperation = MTLBlendOperationMax; a.alphaBlendOperation = MTLBlendOperationMax;
    a.sourceRGBBlendFactor = MTLBlendFactorOne; a.sourceAlphaBlendFactor = MTLBlendFactorOne;
    a.destinationRGBBlendFactor = MTLBlendFactorOne; a.destinationAlphaBlendFactor = MTLBlendFactorOne;
  }
  id<MTLRenderPipelineState> state = [dev newRenderPipelineStateWithDescriptor:p error:&err];
  if (!state) fprintf(stderr, "ui-scene: %s\n", err.localizedDescription.UTF8String);
  return state;
}

UiScene *ui_scene_open(UiHost *h, const UiSceneApp *app) {
  if (!h || !app || !app->background || !app->strokes || !app->uniform_floats || !app->max_vertices || app->vertex_floats % 4) return NULL;
  id<MTLDevice> dev = (__bridge id<MTLDevice>)ui_device(h);
  NSString *background = [NSString stringWithContentsOfFile:[NSString stringWithUTF8String:app->background] encoding:NSUTF8StringEncoding error:nil];
  NSString *strokes = [NSString stringWithContentsOfFile:[NSString stringWithUTF8String:app->strokes] encoding:NSUTF8StringEncoding error:nil];
  if (!dev || !background || !strokes) return NULL;
  UiSceneState *s = [UiSceneState new];
  s->app = *app; s->host = h;
  s->uniforms = calloc(app->uniform_floats, sizeof(float));
  s->vertices = [dev newBufferWithLength:(NSUInteger)app->max_vertices * app->vertex_floats * sizeof(float) options:MTLResourceStorageModeShared];
  if (!s->uniforms || !s->vertices) return NULL;
  NSString *bvs = [NSString stringWithUTF8String:app->background_vs], *bfs = [NSString stringWithUTF8String:app->background_fs];
  NSString *svs = [NSString stringWithUTF8String:app->strokes_vs], *sfs = [NSString stringWithUTF8String:app->strokes_fs];
  uint32_t floats = app->vertex_floats;
  // the app's strings may not outlive this call: nothing below reads them
  s->app.background = s->app.strokes = s->app.background_vs = s->app.background_fs = s->app.strokes_vs = s->app.strokes_fs = NULL;
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    id<MTLRenderPipelineState> b = ui_scene_pipeline(dev, background, bvs, bfs, 0, NO);
    id<MTLRenderPipelineState> f = b ? ui_scene_pipeline(dev, strokes, svs, sfs, floats, YES) : nil;
    dispatch_async(dispatch_get_main_queue(), ^{
      if (s->closed) return;
      if (!b || !f) { s->failed = 1; return; }
      s->background = b; s->strokes = f; s->ready = 1; s->dirty = 1; s->installed = 1;
      UiLayer layer = {(__bridge void *)s, 0, NULL, ui_scene_encode, ui_scene_changed};
      ui_layer(s->host, &layer);
    });
  });
  return (UiScene *)(__bridge_retained void *)s;
}
int ui_scene_ready(UiScene *p) { return p && ui_scene_of(p)->ready; }
int ui_scene_failed(UiScene *p) { return !p || ui_scene_of(p)->failed; }
void ui_scene_touch(UiScene *p) { if (p) ui_scene_of(p)->dirty = 1; }
uint64_t ui_scene_frames(UiScene *p) { return p ? ui_scene_of(p)->frames : 0; }
void ui_scene_close(UiHost *h, UiScene *p) {
  if (!p) return;
  UiSceneState *s = (__bridge_transfer UiSceneState *)(void *)p;
  s->closed = 1;
  if (s->installed && h) ui_layer(h, NULL);
  s->installed = 0; s->ready = 0; s->background = nil; s->strokes = nil; s->vertices = nil;
}
