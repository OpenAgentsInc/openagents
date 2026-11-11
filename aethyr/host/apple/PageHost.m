// Firm reason: 6 (Metal, URLSession, CoreText and ImageIO have no C API; this holds no product logic).
// A realm's page on Metal (page-host.h): the native twin of coax/web/host.js's own passes. Every
// rule here is the browser host's, in the same order, so a realm sees one host on both: the
// messages before the first frame, the pack's parts as assets 0 to 3, the stores uploaded when
// their versions change, the shadow tiles, the scene, the particles, bloom, post and the HUD.
#import "page-host.h"
#import <QuartzCore/QuartzCore.h>
#import <CoreGraphics/CoreGraphics.h>
#import <CoreText/CoreText.h>
#import <ImageIO/ImageIO.h>
#include <TargetConditionals.h>
#include <dlfcn.h>
#include <sys/stat.h>
#include <sys/sysctl.h>
#include <time.h>
#include <zlib.h>
#include "audio-host.h"

typedef uintptr_t A;
// The face (aethyr/host/README.md, and the page's optional v1 exports host.js reads). A name the
// library lacks stays NULL, and the host does what host.js does without it.
#define PH_FACE(F) \
 F(uint32_t,host_contract_version,(void)) F(uint32_t,state_size,(void)) F(void,state_init,(A)) \
 F(uint32_t,max_vertices,(void)) F(uint32_t,vertex_floats,(void)) F(uint32_t,uniform_floats,(void)) \
 F(uint32_t,frame,(A,A,A,float,float,float,float,uint32_t)) F(uint32_t,pixel_budget,(void)) \
 F(uint32_t,key,(A,int32_t,int32_t)) F(uint32_t,key_up,(A,int32_t,int32_t)) F(void,blur,(A)) F(void,focus,(A)) \
 F(uint32_t,pointer,(A,float,float,int32_t,int32_t,int32_t)) F(uint32_t,pointer_buttons_v1,(A,float,float,int32_t,int32_t,int32_t,int32_t)) \
 F(uint32_t,wheel,(A,float)) F(int32_t,mouselook,(A)) F(void,look_delta,(A,float,float)) F(void,pointer_lock,(A,int32_t)) F(int32_t,cursor,(A)) \
 F(uint32_t,wants_text,(A)) F(A,inbox,(A)) F(uint32_t,inbox_cap,(void)) F(void,message,(A,int32_t,uint32_t)) \
 F(A,asset,(A,int32_t,uint32_t)) F(uint32_t,asset_capacity,(A,int32_t)) F(void,asset_ready,(A,int32_t,uint32_t)) \
 F(A,draws,(A)) F(uint32_t,draw_count,(A)) F(A,shadow_draws_v1,(A)) F(uint32_t,shadow_draw_count_v1,(A)) \
 F(uint32_t,shadow_atlas_v1,(A,int32_t,int32_t)) F(int32_t,shadow_v1,(A,int32_t)) F(void,shadow_event_v1,(A,int32_t,int32_t)) \
 F(uint32_t,layers_side_v1,(A)) F(uint32_t,data_layers_v1,(void)) F(uint32_t,bloom_passes_v1,(A)) F(uint32_t,fx_alpha_v1,(void)) \
 F(uint32_t,stores,(void)) F(uint32_t,store_cap,(int32_t)) F(A,store,(A,int32_t)) F(uint32_t,store_bytes,(A,int32_t)) F(uint32_t,store_version,(A,int32_t)) \
 F(uint32_t,frame_store_cap,(void)) F(A,frame_store,(A)) F(uint32_t,frame_store_bytes,(A)) \
 F(uint32_t,particle_cap,(void)) F(A,particles,(A)) F(uint32_t,particle_bytes,(A)) F(uint32_t,particle_instances,(A)) \
 F(int32_t,action,(A)) F(A,outbox,(A)) F(uint32_t,outbox_len,(A)) F(uint32_t,outbox_capacity,(A)) F(void,action_done,(A)) \
 F(A,drive_out,(A)) F(uint32_t,drive_out_len,(A)) F(A,media,(A)) F(uint32_t,media_len,(A)) F(uint32_t,media_capacity,(A)) F(uint32_t,sounds_len,(A)) F(uint32_t,player_appearance_v1,(void)) \
 F(A,sounds,(A)) F(uint32_t,sounds_capacity,(A)) F(A,sound_bank,(A)) F(uint32_t,sound_bank_len,(A)) F(uint32_t,sound_bank_capacity,(A)) \
 F(A,sound_bank_storage,(A)) F(uint32_t,sound_bank_storage_bytes,(A))
typedef struct {
#define PH_FN(result,name,args) result (*name) args;
 PH_FACE(PH_FN)
#undef PH_FN
} PHFace;

@implementation AEPageConfig @end

// One resource a shader reads, from its .bind file (`coax metal`'s reflection).
typedef struct { char name[32]; int kind, slot, stride, stages; } PHUse;   // kind 1 buffer, 2 texture, 3 sampler
@interface PHPipe : NSObject { @public id<MTLRenderPipelineState> state; PHUse uses[32]; int count; }
@end
@implementation PHPipe @end
@interface PHShader : NSObject { @public id<MTLLibrary> library; NSString *vertex, *fragment; PHUse uses[32]; int count; int attributes; }
@end
@implementation PHShader @end
@interface PHMedia : NSObject { @public id<MTLTexture> texture; int w, h; BOOL ready, failed; int unseen; }
@end
@implementation PHMedia @end
@interface PHSocket : NSObject { @public NSURLSessionWebSocketTask *task; int slot; BOOL ended; }
@end
@implementation PHSocket @end
// Something that arrived off the main thread, waiting for the next frame: a socket's event or an HTTP answer.
@interface PHEvent : NSObject { @public int kind; NSData *bytes; id owner; BOOL closes; }
@end
@implementation PHEvent @end

@class AEPageHost;
@interface PHNet : NSObject <NSURLSessionWebSocketDelegate, NSURLSessionTaskDelegate>
@property (weak) AEPageHost *host;
@end

@interface AEPageHost () {
@public
 PHFace x; void *library; A state; NSMutableData *stateBytes, *vertexBytes, *uniformBytes;
 AEPageConfig *config; id<MTLDevice> device; id<MTLCommandQueue> queue; id<MTLCommandBuffer> last;
 NSData *packBytes; const uint8_t *packVertices, *packIndices; size_t packVertexBytes, packIndexBytes;
 id<MTLBuffer> uniformBuffer, frameBuffer, vertexBuffer, storageBuffer, indexBuffer, particleBuffer, fxBuffer;
 NSMutableArray<id<MTLBuffer>> *stores; NSMutableArray<NSNumber *> *storeVersions;
 id<MTLTexture> sheets, dataLayers, sans, shadowMap, light, aux, depth, q0, q1, e0, e1, screen, bloomSource;
 BOOL shadowTiles, shadowFresh; float bloomStep[4];
 id<MTLSamplerState> sheetSampler, sansSampler, shadowSampler, linear;
 id<MTLDepthStencilState> depthAlways, depthLess, depthRead, depthClear;
 NSMutableDictionary<NSString *, PHShader *> *shaders; NSMutableDictionary<NSString *, PHPipe *> *pipes;
 NSMutableDictionary<NSString *, NSNumber *> *sceneNames;   // a scene resource's name -> its place in group 1
 id<MTLRenderPipelineState> clearPipe; NSMutableDictionary<NSNumber *, id<MTLRenderPipelineState>> *boxPipes; id<MTLLibrary> glue;
 NSMutableDictionary<NSString *, PHMedia *> *media; NSString *mediaSaid;
 NSMutableData *soundsTaken, *soundsGiven;   // the sound packets the face said, until the audio host takes them
 PHNet *net; NSURLSession *session; PHSocket *sockets[4]; id http[8];
 NSMutableArray<PHEvent *> *events; NSString *playerKey, *playerId, *playerAccount;
 NSMutableDictionary<NSNumber *, NSValue *> *fingers; NSMutableDictionary<NSNumber *, NSNumber *> *fingerDown;   // a touch's start and the time it began, by its number
 int width, height; float dpr; CGSize points; int quality; double opened; BOOL closed;
}
- (void)push:(int)kind bytes:(NSData *)bytes owner:(id)owner closes:(BOOL)closes;
- (void)socket:(NSURLSessionWebSocketTask *)task event:(int)event bytes:(NSData *)bytes;
- (int32_t)socketOpen:(NSString *)url;
- (void)socketSend:(int32_t)id text:(NSString *)text;
- (void)socketClose:(int32_t)id;
- (int32_t)http:(NSString *)path body:(NSData *)body flags:(int32_t)flags;
- (void)after;
- (id<MTLRenderPipelineState>)boxPipe:(MTLPixelFormat)format;
@end

static __weak AEPageHost *phCurrent;   // the page the face's imports reach: one is open at a time
// AETHYR_PAGE_LOG=1 says on stderr what crosses the network (never a body, a key or a ticket).
static void phLog(NSString *format, ...) NS_FORMAT_FUNCTION(1, 2);
static void phLog(NSString *format, ...) {
 static int on = -1; if (on < 0) on = getenv("AETHYR_PAGE_LOG") != NULL;
 if (!on) return;
 va_list a; va_start(a, format); NSString *line = [[NSString alloc] initWithFormat:format arguments:a]; va_end(a);
 fprintf(stderr, "page host: %s\n", line.UTF8String);
}
static int64_t phMillis(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000; }

// A gzip file's bytes, inflated; nil when it is damaged. The file's last four bytes say how long
// the result is (mod 4 GiB), which sizes the first allocation; the stream decides the real length.
static NSData *phInflate(NSData *gz) {
 uint32_t said; memcpy(&said, (const uint8_t *)gz.bytes + gz.length - 4, 4);
 NSMutableData *out = [NSMutableData dataWithLength:MAX((size_t)said, (size_t)1 << 16)];
 z_stream z = {0};
 if (!out || inflateInit2(&z, 15 + 16) != Z_OK) return nil;
 const uint8_t *in = gz.bytes; size_t left = gz.length, made = 0; int r = Z_OK;
 while (r == Z_OK) {
  if (made == out.length) { out.length += out.length / 2; if (!out.mutableBytes) break; }
  uInt give = (uInt)MIN(left, (size_t)1 << 30), room = (uInt)MIN(out.length - made, (size_t)1 << 30);
  z.next_in = (Bytef *)in; z.avail_in = give; z.next_out = (Bytef *)out.mutableBytes + made; z.avail_out = room;
  r = inflate(&z, Z_NO_FLUSH);
  in += give - z.avail_in; left -= give - z.avail_in; made += room - z.avail_out;
  if (r == Z_OK && !left && z.avail_out) break;   // the file ended before its stream did
 }
 inflateEnd(&z);
 if (r != Z_STREAM_END) return nil;
 out.length = made; return out;
}

@implementation PHNet
- (void)URLSession:(NSURLSession *)s webSocketTask:(NSURLSessionWebSocketTask *)task didOpenWithProtocol:(NSString *)p { [self.host socket:task event:0 bytes:nil]; }
- (void)URLSession:(NSURLSession *)s webSocketTask:(NSURLSessionWebSocketTask *)task didCloseWithCode:(NSURLSessionWebSocketCloseCode)code reason:(NSData *)reason { [self.host socket:task event:2 bytes:nil]; }
- (void)URLSession:(NSURLSession *)s task:(NSURLSessionTask *)task didCompleteWithError:(NSError *)error {
 if (![task isKindOfClass:NSURLSessionWebSocketTask.class]) return;
 if (error) [self.host socket:(NSURLSessionWebSocketTask *)task event:3 bytes:nil];
 [self.host socket:(NSURLSessionWebSocketTask *)task event:2 bytes:nil];
}
// Host contract v1: a redirect is never followed; the answer is then the 3xx itself, which the host refuses.
- (void)URLSession:(NSURLSession *)s task:(NSURLSessionTask *)task willPerformHTTPRedirection:(NSHTTPURLResponse *)r newRequest:(NSURLRequest *)q completionHandler:(void (^)(NSURLRequest *))done { done(nil); }
@end

@implementation AEPageHost

// ---- what a browser keeps in its storage, as files only their owner reads
- (NSString *)kept:(NSString *)name {
 NSData *d = [NSData dataWithContentsOfFile:[config.store stringByAppendingPathComponent:name]];
 return d ? ([[NSString alloc] initWithData:d encoding:NSUTF8StringEncoding] ?: @"") : @"";
}
- (void)keep:(NSString *)name text:(NSString *)text {
 [NSFileManager.defaultManager createDirectoryAtPath:config.store withIntermediateDirectories:YES attributes:@{NSFilePosixPermissions: @0700} error:nil];
 NSString *path = [config.store stringByAppendingPathComponent:name];
 [[text dataUsingEncoding:NSUTF8StringEncoding] writeToFile:path atomically:YES];
 chmod(path.fileSystemRepresentation, 0600);
}
- (NSString *)keptId:(NSString *)name {
 NSString *v = [self kept:name];
 if (v.length) return v;
 uint8_t b[8]; arc4random_buf(b, 8);
 NSMutableString *s = [NSMutableString string]; for (int i = 0; i < 8; i++) [s appendFormat:@"%02x", b[i]];
 [self keep:name text:s]; return s;
}
- (void)readPlayer {
 playerKey = playerId = playerAccount = nil;
 NSData *d = [[self kept:@"verse.player"] dataUsingEncoding:NSUTF8StringEncoding];
 NSDictionary *p = d.length ? [NSJSONSerialization JSONObjectWithData:d options:0 error:nil] : nil;
 if (![p isKindOfClass:NSDictionary.class] || ![p[@"key"] isKindOfClass:NSString.class]) return;
 playerKey = p[@"key"];
 playerId = [p[@"id"] isKindOfClass:NSString.class] ? p[@"id"] : nil;
 playerAccount = [p[@"account"] isKindOfClass:NSString.class] ? p[@"account"] : nil;
}

// ---- messages into the face, and what it asks for after each call (host.js send and after)
- (void)message:(int)kind bytes:(const void *)bytes length:(size_t)n {
 if (closed || !x.message) return;
 size_t cap = x.inbox_cap(); if (n > cap) n = cap;
 if (n) memcpy((void *)x.inbox(state), bytes, n);
 x.message(state, kind, (uint32_t)n);
 [self after];
}
- (void)message:(int)kind text:(NSString *)text { NSData *d = [text dataUsingEncoding:NSUTF8StringEncoding]; [self message:kind bytes:d.bytes length:d.length]; }
// The face's sound packet (aethyr/host/README.md, Audio), once after each handoff: coaxial_sounds_len
// drains it, so it is read once, and a v1 packet must lie inside the allocation the face names or it
// is refused whole. The packets wait, a line each, for the audio host (mf_sound_commands); one
// nobody takes is dropped past 64 KB rather than grown.
- (void)takeSounds {
 if (closed || !x.sounds_len) return;
 uint32_t n = x.sounds_len(state);
 if (!n) return;
 A p = x.sounds ? x.sounds(state) : 0;
 if (!p || !x.sounds_capacity || n > x.sounds_capacity(state)) return;
 if (!soundsTaken) soundsTaken = [NSMutableData data];
 if (soundsTaken.length + n + 1 > 65536) return;
 [soundsTaken appendBytes:(void *)p length:n];
 if (((const uint8_t *)p)[n - 1] != '\n') [soundsTaken appendBytes:"\n" length:1];
}
- (void)after {
 [self takeSounds];
 if (closed || !x.action) return;
 int32_t code = x.action(state);
 if (!code) return;
 // Contract v1: the whole of the action's bytes must lie inside the allocation the face names.
 A p = x.outbox ? x.outbox(state) : 0; size_t n = x.outbox_len ? x.outbox_len(state) : 0;
 BOOL v1 = x.host_contract_version && x.host_contract_version() == 1;
 if (n && (!p || (v1 && (!x.outbox_capacity || n > x.outbox_capacity(state))))) { x.action_done(state); [self message:604 text:@"Action output unavailable"]; return; }
 NSString *text = [[NSString alloc] initWithBytes:(void *)p length:n encoding:NSUTF8StringEncoding] ?: @"";
 x.action_done(state);
 NSString *slot = config.slot ?: @"page";
 if (code == 4) [self keep:[slot stringByAppendingString:@".kept"] text:text];
 else if (code == 5) {
  NSDictionary *o = [NSJSONSerialization JSONObjectWithData:[text dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
  if ([o isKindOfClass:NSDictionary.class] && [o[@"key"] isKindOfClass:NSString.class]) { [self keep:@"verse.player" text:text]; [self readPlayer]; }
 }
 else if (code == 16 && x.player_appearance_v1 && x.player_appearance_v1() == 1) { [self keep:@"aethyr.player.appearance.v1" text:text]; [self message:17 text:text]; }
 else if (code == 11) [self keep:[slot stringByAppendingString:@".place"] text:text];
 else if (code == 12) [self keep:[slot stringByAppendingString:@".bar"] text:text];
 else if (code == 6) { NSArray *names = @[@"high", @"medium", @"low", @"auto"]; NSUInteger q = [names indexOfObject:text]; if (q != NSNotFound) { quality = (int)q; [self keep:@"verse.quality" text:text]; } }
 else if (code == 33) [self keep:@"verse.quality.settled" text:[text substringToIndex:MIN(text.length, 16)]];
 else if (code == 13) [self keep:@"verse.sound" text:[text substringToIndex:MIN(text.length, 16)]];
 else if (code == 14) [self keep:@"verse.settings" text:[text substringToIndex:MIN(text.length, 256)]];
 // 1 (go to another page), 2 (the clipboard), 3 and 19 (the address bar) and the rest have no
 // effect here: a page in this host has no address bar and opens no other page.
}

- (NSString *)facts {
 // The screen in pixels: the one the host names (a phone's, which only UIKit knows), else the Mac's main display.
 CGSize screenPixels = config.screen;
#if !TARGET_OS_IOS
 if (screenPixels.width <= 0) { CGDirectDisplayID display = CGMainDisplayID(); screenPixels = CGSizeMake(CGDisplayPixelsWide(display), CGDisplayPixelsHigh(display)); }
#endif
 NSMutableString *s = [NSMutableString string];
 void (^put)(NSString *, id) = ^(NSString *k, id v) { [s appendFormat:@"%@%@=%@", s.length ? @"\n" : @"", k, [[v description] stringByReplacingOccurrencesOfString:@"\n" withString:@" "]]; };
 NSString *tab = [self keptId:@"coaxial.visitor"];
 put(@"tab", [tab stringByAppendingFormat:@"%04x", (unsigned)(getpid() & 0xffff)]); put(@"fresh", @1); put(@"visitor", tab);
 put(@"now", @((long long)(NSDate.date.timeIntervalSince1970 * 1000))); put(@"nav", [NSString stringWithFormat:@"%.1f", (CACurrentMediaTime() - opened) * 1000]);
 put(@"url", [config.origin stringByAppendingString:config.path ?: @"/"]); put(@"ref", @"");
 put(@"ua", [NSString stringWithFormat:@"AethyrPageHost/1 (%@)", NSProcessInfo.processInfo.operatingSystemVersionString]);
 put(@"lang", NSLocale.preferredLanguages.firstObject ?: @"en"); put(@"tz", NSTimeZone.localTimeZone.name);
 put(@"sw", @((int)screenPixels.width)); put(@"sh", @((int)screenPixels.height));
 put(@"vw", @((int)points.width)); put(@"vh", @((int)points.height)); put(@"dpr", @(dpr > 0 ? dpr : 2));
 put(@"coarse", config.touch ? @1 : @0); put(@"touch", config.touch ? @5 : @0); put(@"gpu", @1); put(@"hidden", @0);
 put(@"cores", @(NSProcessInfo.processInfo.activeProcessorCount)); put(@"memory", @(MIN(8, (int)(NSProcessInfo.processInfo.physicalMemory >> 30))));
 put(@"quality", [self kept:@"verse.quality"]); put(@"settled", [self kept:@"verse.quality.settled"]);
 put(@"gpu_vendor", @"apple"); put(@"gpu_arch", @"metal-3"); put(@"gpu_fallback", @0);
 put(@"sound", [self kept:@"verse.sound"]); put(@"settings", [self kept:@"verse.settings"]);
 put(@"focused", @1); put(@"clip", @0); put(@"webdriver", config.automated ? @1 : @0);
 put(@"player", playerId ?: @""); put(@"account", playerAccount ?: @"");
 put(@"realm", config.realm ?: @""); put(@"slot", config.slot ?: @"");
 put(@"style", config.style ?: @""); put(@"clock", config.clock ?: @"");
 NSDateComponents *c = [NSCalendar.currentCalendar components:NSCalendarUnitHour | NSCalendarUnitMinute | NSCalendarUnitSecond fromDate:NSDate.date];
 put(@"day", [NSString stringWithFormat:@"%.5f", (c.hour * 3600 + c.minute * 60 + c.second) / 86400.0]);
 return s;
}

// ---- shaders: each pair `coax metal` emitted, with the resources its .bind file lists
- (PHShader *)shader:(NSString *)role {
 PHShader *s = shaders[role]; if (s) return s;
 NSString *base = [config.shaders stringByAppendingPathComponent:role];
 NSString *bind = [NSString stringWithContentsOfFile:[base stringByAppendingPathExtension:@"bind"] encoding:NSUTF8StringEncoding error:nil];
 id<MTLLibrary> lib = [device newLibraryWithURL:[NSURL fileURLWithPath:[base stringByAppendingPathExtension:@"metallib"]] error:nil];
 if (!bind || !lib) return nil;
 s = [PHShader new]; s->library = lib;
 for (NSString *line in [bind componentsSeparatedByString:@"\n"]) {
  NSArray<NSString *> *w = [line componentsSeparatedByCharactersInSet:NSCharacterSet.whitespaceCharacterSet];
  if (w.count == 3 && [w[0] isEqual:@"entry"]) { if ([w[1] isEqual:@"vertex"]) s->vertex = w[2]; else s->fragment = w[2]; }
  else if (w.count == 7 && [w[0] isEqual:@"resource"] && s->count < 32) {
   PHUse *u = &s->uses[s->count++];
   u->kind = [w[1] isEqual:@"buffer"] ? 1 : [w[1] isEqual:@"texture"] ? 2 : 3;
   u->slot = w[2].intValue; u->stride = w[4].intValue; u->stages = w[5].intValue;
   strlcpy(u->name, w[6].UTF8String, sizeof u->name);
  }
  else if (w.count == 7 && [w[0] isEqual:@"attribute"]) s->attributes++;
 }
 if (!s->vertex) return nil;
 shaders[role] = s; return s;
}
typedef struct { MTLPixelFormat format; BOOL blend; MTLBlendFactor src, dst, srcAlpha, dstAlpha; BOOL masked; } PHTarget;
- (PHPipe *)pipe:(NSString *)name role:(NSString *)role targets:(const PHTarget *)targets count:(int)count depth:(BOOL)hasDepth {
 PHPipe *p = pipes[name]; if (p) return p;
 PHShader *s = [self shader:role]; if (!s) return nil;
 MTLRenderPipelineDescriptor *d = [MTLRenderPipelineDescriptor new];
 d.vertexFunction = [s->library newFunctionWithName:s->vertex];
 d.fragmentFunction = s->fragment && count ? [s->library newFunctionWithName:s->fragment] : nil;
 // A pass with no colour (the shadow map) still runs the fragment stage: it is what drops a leaf's clear pixels.
 if (!count && s->fragment) d.fragmentFunction = [s->library newFunctionWithName:s->fragment];
 for (int i = 0; i < count; i++) {
  MTLRenderPipelineColorAttachmentDescriptor *a = d.colorAttachments[i];
  a.pixelFormat = targets[i].format;
  if (targets[i].masked) a.writeMask = MTLColorWriteMaskNone;
  if (targets[i].blend) { a.blendingEnabled = YES; a.sourceRGBBlendFactor = targets[i].src; a.destinationRGBBlendFactor = targets[i].dst; a.sourceAlphaBlendFactor = targets[i].srcAlpha; a.destinationAlphaBlendFactor = targets[i].dstAlpha; }
 }
 if (hasDepth) d.depthAttachmentPixelFormat = MTLPixelFormatDepth32Float;
 if (s->attributes) {
  // the HUD's vertices: one float4 for each four floats of the face's vertex, at buffer 30
  MTLVertexDescriptor *v = [MTLVertexDescriptor new]; int floats = (int)x.vertex_floats();
  for (int i = 0; i < s->attributes; i++) { v.attributes[i].format = MTLVertexFormatFloat4; v.attributes[i].offset = 16 * i; v.attributes[i].bufferIndex = 30; }
  v.layouts[30].stride = floats * 4; d.vertexDescriptor = v;
 }
 NSError *e = nil; id<MTLRenderPipelineState> made = [device newRenderPipelineStateWithDescriptor:d error:&e];
 if (!made) { NSLog(@"page host: %@ pipeline: %@", name, e.localizedDescription); return nil; }
 p = [PHPipe new]; p->state = made; p->count = s->count; memcpy(p->uses, s->uses, sizeof s->uses);
 pipes[name] = p; return p;
}
// Sets every resource a pipeline's shaders read, each by its name, and the element counts at buffer 29.
- (void)bind:(PHPipe *)p to:(id<MTLRenderCommandEncoder>)e {
 [e setRenderPipelineState:p->state];
 uint32_t lengths[29] = {0};
 for (int i = 0; i < p->count; i++) {
  PHUse u = p->uses[i]; NSString *name = @(u.name);
  BOOL vs = u.stages & 1, fs = (u.stages & 2) != 0;
  if (u.kind == 1) {
   id<MTLBuffer> b = nil; NSNumber *at = sceneNames[name];
   if ([name isEqual:@"u"]) b = uniformBuffer; else if ([name isEqual:@"frame"]) b = frameBuffer; else if ([name isEqual:@"puffs"]) b = particleBuffer;
   else if ([name isEqual:@"bloom_step"]) { lengths[u.slot] = 4; if (vs) [e setVertexBytes:bloomStep length:16 atIndex:u.slot]; if (fs) [e setFragmentBytes:bloomStep length:16 atIndex:u.slot]; continue; }
   else if (at) b = at.intValue == 0 && storageBuffer ? storageBuffer : (at.intValue - (storageBuffer ? 1 : 0) < (int)stores.count && at.intValue - (storageBuffer ? 1 : 0) >= 0 ? stores[at.intValue - (storageBuffer ? 1 : 0)] : nil);
   if (!b) continue;
   if (u.slot < 29 && u.stride > 0) lengths[u.slot] = (uint32_t)(b.length / u.stride);
   if (vs) [e setVertexBuffer:b offset:0 atIndex:u.slot]; if (fs) [e setFragmentBuffer:b offset:0 atIndex:u.slot];
  } else if (u.kind == 2) {
   id<MTLTexture> t = [name isEqual:@"sheets"] ? sheets : [name isEqual:@"data"] ? dataLayers : [name isEqual:@"sans_atlas"] ? sans : [name isEqual:@"shadow_map"] ? shadowMap
    : [name isEqual:@"bloom_src"] ? bloomSource : [name isEqual:@"post_light"] ? light : [name isEqual:@"post_quarter"] ? q0 : [name isEqual:@"post_eighth"] ? e0
    : [name isEqual:@"post_aux"] ? aux : [name isEqual:@"post_depth"] ? depth : nil;
   if (!t) continue;
   if (vs) [e setVertexTexture:t atIndex:u.slot]; if (fs) [e setFragmentTexture:t atIndex:u.slot];
  } else {
   id<MTLSamplerState> t = [name isEqual:@"sheet"] ? sheetSampler : [name isEqual:@"sans_sampler"] ? sansSampler : [name isEqual:@"shadow_cmp"] ? shadowSampler : linear;
   if (vs) [e setVertexSamplerState:t atIndex:u.slot]; if (fs) [e setFragmentSamplerState:t atIndex:u.slot];
  }
 }
 [e setVertexBytes:lengths length:sizeof lengths atIndex:29]; [e setFragmentBytes:lengths length:sizeof lengths atIndex:29];
 [e setCullMode:MTLCullModeNone];
}

// ---- textures
- (id<MTLTexture>)target:(MTLPixelFormat)format width:(int)w height:(int)h {
 MTLTextureDescriptor *d = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:format width:MAX(1, w) height:MAX(1, h) mipmapped:NO];
 d.usage = MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead; d.storageMode = MTLStorageModePrivate;
 return [device newTextureWithDescriptor:d];
}
// The pack's layers as one array, every layer drawn to `side` and its mips made on the GPU. Colour
// layers are sRGB with straight alpha; data layers (normal and ORM maps) keep their bytes.
- (id<MTLTexture>)layers:(NSArray<NSData *> *)images side:(int)side data:(BOOL)data {
 if (!images.count) return nil;
 MTLTextureDescriptor *d = [MTLTextureDescriptor new];
 d.textureType = MTLTextureType2DArray; d.pixelFormat = data ? MTLPixelFormatRGBA8Unorm : MTLPixelFormatRGBA8Unorm_sRGB;
 d.width = side; d.height = side; d.arrayLength = images.count; d.mipmapLevelCount = 1 + (int)floor(log2(side)); d.usage = MTLTextureUsageShaderRead | MTLTextureUsageRenderTarget;
 id<MTLTexture> t = [device newTextureWithDescriptor:d]; if (!t) return nil;
 size_t row = (size_t)side * 4; NSUInteger n = images.count;
 CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
 __block BOOL ok = YES;
 dispatch_apply(n, dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^(size_t layer) {
  uint8_t *pixels = calloc(row, side);
  CGImageSourceRef src = CGImageSourceCreateWithData((__bridge CFDataRef)images[layer], NULL);
  CGImageRef decoded = src ? CGImageSourceCreateImageAtIndex(src, 0, NULL) : NULL;
  // drawn in the context's own space: the bytes are the file's, with no colour matching
  CGImageRef image = decoded ? CGImageCreateCopyWithColorSpace(decoded, space) : NULL;
  if (!image && decoded) image = CGImageRetain(decoded);
  CGContextRef g = pixels ? CGBitmapContextCreate(pixels, side, side, 8, row, space, (CGBitmapInfo)kCGImageAlphaPremultipliedLast) : NULL;
  if (image && g) {
   CGContextSetBlendMode(g, kCGBlendModeCopy); CGContextSetInterpolationQuality(g, kCGInterpolationHigh);
   CGContextDrawImage(g, CGRectMake(0, 0, side, side), image);
   // CoreGraphics multiplies colour by alpha; the shaders want it straight, as a browser uploads it.
   for (size_t p = 0; p < row * side; p += 4) { unsigned a = pixels[p + 3]; if (a > 0 && a < 255) for (int c = 0; c < 3; c++) { unsigned v = (pixels[p + c] * 255u + a / 2) / a; pixels[p + c] = v > 255 ? 255 : (uint8_t)v; } }
   @synchronized (t) { [t replaceRegion:MTLRegionMake2D(0, 0, side, side) mipmapLevel:0 slice:layer withBytes:pixels bytesPerRow:row bytesPerImage:0]; }
  } else ok = NO;
  if (g) CGContextRelease(g); if (image) CGImageRelease(image); if (decoded) CGImageRelease(decoded); if (src) CFRelease(src); free(pixels);
 });
 CGColorSpaceRelease(space);
 if (!ok) return nil;
 id<MTLCommandBuffer> c = [queue commandBuffer]; id<MTLBlitCommandEncoder> b = [c blitCommandEncoder];
 [b generateMipmapsForTexture:t]; [b endEncoding]; [c commit];
 return t;
}
// The HUD's atlas (host.js sansAtlas with a title face): ASCII 32..126 then sixteen more marks, 16
// columns of 96-texel cells, the pen at x 16 and the baseline at y 72 at 64 px, the reading face in
// red and the title face in green; every mip drawn at its own size.
- (id<MTLTexture>)atlas {
 static const uint32_t extra[16] = {183, 8211, 8212, 8216, 8217, 8220, 8221, 8226, 8230, 8594, 8383, 8722, 215, 8804, 8805, 8800};
 CTFontDescriptorRef faces[2] = {NULL, NULL};
 NSArray *files = @[config.reading ?: @"", config.title ?: @""];
 for (int f = 0; f < 2; f++) {
  NSURL *url = [NSURL fileURLWithPath:[[config.fonts stringByAppendingPathComponent:files[f]] stringByAppendingPathExtension:@"ttf"]];
  NSArray *all = CFBridgingRelease(CTFontManagerCreateFontDescriptorsFromURL((__bridge CFURLRef)url));
  if (!all.count) { if (f == 0) return nil; break; }
  CTFontDescriptorRef d = (__bridge CTFontDescriptorRef)all.firstObject;
  // a variable title face: 'wght' is its weight axis
  faces[f] = f == 1 && config.titleWeight > 0 ? CTFontDescriptorCreateCopyWithAttributes(d, (__bridge CFDictionaryRef)@{(id)kCTFontVariationAttribute: @{@0x77676874: @(config.titleWeight)}}) : CFRetain(d);
 }
 const int cols = 16, count = 95 + 16, rows = (count + cols - 1) / cols, cell = 96, levels = 5;
 MTLTextureDescriptor *td = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatRGBA8Unorm width:cols * cell height:rows * cell mipmapped:YES];
 td.mipmapLevelCount = levels;
 id<MTLTexture> t = [device newTextureWithDescriptor:td];
 CGColorSpaceRef gray = CGColorSpaceCreateDeviceGray();
 for (int level = 0; t && level < levels; level++) {
  int c = cell >> level, w = cols * c, h = rows * c; CGFloat scale = 1 << level;
  uint8_t *out = calloc((size_t)w * h, 4);
  for (int f = 0; f < 2; f++) {
   if (!faces[f]) continue;
   CGContextRef g = CGBitmapContextCreate(NULL, w, h, 8, w, gray, (CGBitmapInfo)kCGImageAlphaNone);
   CGContextSetGrayFillColor(g, 0, 1); CGContextFillRect(g, CGRectMake(0, 0, w, h));
   CGContextSetShouldSmoothFonts(g, false); CGContextSetAllowsAntialiasing(g, true); CGContextSetTextMatrix(g, CGAffineTransformIdentity);
   CTFontRef font = CTFontCreateWithFontDescriptor(faces[f], 64 / scale, NULL);
   CGColorRef white = CGColorCreateGenericGray(1, 1);
   NSDictionary *attrs = @{(id)kCTFontAttributeName: (__bridge id)font, (id)kCTForegroundColorAttributeName: (__bridge id)white};
   for (int i = 0; i < count; i++) {
    uint32_t code = i < 95 ? 32 + i : extra[i - 95];
    NSString *one = [[NSString alloc] initWithBytes:&code length:4 encoding:NSUTF32LittleEndianStringEncoding];
    CTLineRef line = CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)[[NSAttributedString alloc] initWithString:one attributes:attrs]);
    CGContextSetTextPosition(g, (i % cols) * c + 16 / scale, h - ((i / cols) * c + 72 / scale));
    CTLineDraw(line, g); CFRelease(line);
   }
   const uint8_t *drawn = CGBitmapContextGetData(g);
   for (int p = 0; p < w * h; p++) out[p * 4 + f] = drawn[p];
   CGColorRelease(white); CFRelease(font); CGContextRelease(g);
  }
  for (int p = 0; p < w * h; p++) { int a = out[p * 4] + out[p * 4 + 1]; out[p * 4 + 3] = a > 255 ? 255 : a; }
  [t replaceRegion:MTLRegionMake2D(0, 0, w, h) mipmapLevel:level withBytes:out bytesPerRow:(NSUInteger)w * 4];
  free(out);
 }
 CGColorSpaceRelease(gray);
 for (int f = 0; f < 2; f++) if (faces[f]) CFRelease(faces[f]);
 return t;
}

// ---- opening
+ (instancetype)open:(AEPageConfig *)config device:(id<MTLDevice>)device queue:(id<MTLCommandQueue>)queue why:(NSString **)why {
 AEPageHost *h = [AEPageHost new];
 NSString *error = [h open:config device:device queue:queue];
 if (error) { if (why) *why = error; [h close]; return nil; }
 return h;
}
- (NSString *)open:(AEPageConfig *)c device:(id<MTLDevice>)d queue:(id<MTLCommandQueue>)q {
 config = c; device = d; queue = q; opened = CACurrentMediaTime();
 shaders = [NSMutableDictionary dictionary]; pipes = [NSMutableDictionary dictionary]; sceneNames = [NSMutableDictionary dictionary];
 media = [NSMutableDictionary dictionary]; boxPipes = [NSMutableDictionary dictionary]; events = [NSMutableArray array]; stores = [NSMutableArray array]; storeVersions = [NSMutableArray array]; fingers = [NSMutableDictionary dictionary]; fingerDown = [NSMutableDictionary dictionary];
 library = dlopen(c.producer.fileSystemRepresentation, RTLD_NOW | RTLD_LOCAL);
 if (!library) return [NSString stringWithFormat:@"the realm's library did not load: %s", dlerror()];
#define PH_LOAD(result,name,args) x.name = (result (*) args)dlsym(library, "coaxial_" #name);
 PH_FACE(PH_LOAD)
#undef PH_LOAD
 if (!x.state_size || !x.state_init || !x.frame || !x.max_vertices || !x.vertex_floats || !x.uniform_floats || !x.inbox || !x.inbox_cap || !x.message) return @"the realm's library lacks the page's face";
 phCurrent = self;
 stateBytes = [NSMutableData dataWithLength:MAX(x.state_size(), 1024u)];
 vertexBytes = [NSMutableData dataWithLength:(size_t)x.max_vertices() * x.vertex_floats() * 4];
 uniformBytes = [NSMutableData dataWithLength:(size_t)x.uniform_floats() * 4];
 if (!stateBytes || !vertexBytes || !uniformBytes) return @"not enough memory for the realm";
 state = (A)stateBytes.mutableBytes; x.state_init(state);
 [self readPlayer];
 NSString *kept = [self kept:@"verse.quality"]; NSUInteger qi = [@[@"high", @"medium", @"low", @"auto"] indexOfObject:kept];
 // nothing kept: AUTO, the realm's governor sets the pixels (aethyr/quality.coax); a touch screen starts at medium, as host.js starts a phone
 quality = qi == NSNotFound ? (c.touch ? 1 : 3) : (int)qi;
 if (!c.offline) {
  net = [PHNet new]; net.host = self;
  NSURLSessionConfiguration *sc = NSURLSessionConfiguration.ephemeralSessionConfiguration; sc.requestCachePolicy = NSURLRequestReloadIgnoringLocalCacheData;
  session = [NSURLSession sessionWithConfiguration:sc delegate:net delegateQueue:nil];
 }
 // The shaders the page has, and where each scene resource sits in group 1 (page.bindings, the
 // file `coax wgsl` lays the web's module out with): the pack's vertices, the stores in order,
 // then the layers, their sampler and the data layers.
 NSString *bindings = [NSString stringWithContentsOfFile:[c.shaders stringByAppendingPathComponent:@"page.bindings"] encoding:NSUTF8StringEncoding error:nil];
 if (!bindings) return @"the page's shaders are missing";
 NSSet *fixed = [NSSet setWithArray:@[@"bloom_src", @"bloom_lin", @"bloom_step", @"post_light", @"post_quarter", @"post_eighth", @"post_aux", @"post_depth", @"post_lin"]];
 for (NSString *line in [bindings componentsSeparatedByString:@"\n"]) {
  NSArray<NSString *> *w = [[line componentsSeparatedByCharactersInSet:NSCharacterSet.whitespaceCharacterSet] filteredArrayUsingPredicate:[NSPredicate predicateWithFormat:@"length > 0"]];
  if (w.count >= 3 && ![w[0] hasPrefix:@"#"] && w[1].intValue == 1 && ![fixed containsObject:w[0]]) sceneNames[w[0]] = @(w[2].intValue);
 }
 if (![self shader:@"bg"] || ![self shader:@"fx"]) return @"the page's shaders are missing";
 glue = [device newLibraryWithURL:[NSURL fileURLWithPath:[c.shaders stringByAppendingPathComponent:@"page-host.metallib"]] error:nil];
 if (!glue) return @"the host's own shaders are missing";

 // Before the first frame, in the browser's order: what was kept, your character, the page's
 // facts, then where you were and your bar.
 points = CGSizeMake(1280, 800); dpr = 2;
 NSString *slot = c.slot ?: @"page";
 NSString *text = [self kept:[slot stringByAppendingString:@".kept"]]; if (text.length) [self message:4 text:text];
 if (x.player_appearance_v1 && x.player_appearance_v1() == 1) { text = [self kept:@"aethyr.player.appearance.v1"]; if (text.length) [self message:16 text:text]; }
 [self message:600 text:[self facts]];
 text = [self kept:[slot stringByAppendingString:@".place"]]; if (text.length) [self message:11 text:text];
 text = [self kept:[slot stringByAppendingString:@".bar"]]; if (text.length) [self message:12 text:text];
 [self message:606 text:@""];   // nobody is logged in to the site here
 [self message:613 text:@"gpu_vendor=apple\ngpu_arch=metal-3\ngpu_fallback=0"];

 // Buffers the face fills.
 uniformBuffer = [device newBufferWithLength:MAX(uniformBytes.length, 16) options:MTLResourceStorageModeShared];
 fxBuffer = [device newBufferWithLength:MAX(vertexBytes.length, 16) options:MTLResourceStorageModeShared];
 size_t fc = x.frame_store_cap ? x.frame_store_cap() : 16;
 frameBuffer = [device newBufferWithLength:(MAX(fc, 16) + 15) & ~(size_t)15 options:MTLResourceStorageModeShared];
 for (uint32_t k = 0; k < (x.stores ? x.stores() : 0); k++) {
  [stores addObject:[device newBufferWithLength:(MAX(x.store_cap(k), 16u) + 15) & ~15u options:MTLResourceStorageModeShared]]; [storeVersions addObject:@(-1)];
 }
 if (x.particles && x.particle_cap) particleBuffer = [device newBufferWithLength:(MAX(x.particle_cap(), 16u) + 15) & ~15u options:MTLResourceStorageModeShared];

 // The pack (VPK1): a 32-byte header, then a table of 48-byte rows; every extent is checked
 // against the whole file before a byte of it is read.
 NSString *problem = [self loadPack];
 if (problem) return problem;

 MTLSamplerDescriptor *sd = [MTLSamplerDescriptor new];
 sd.minFilter = sd.magFilter = MTLSamplerMinMagFilterLinear; sd.mipFilter = MTLSamplerMipFilterLinear; sd.sAddressMode = sd.tAddressMode = MTLSamplerAddressModeRepeat; sd.maxAnisotropy = 8;
 sheetSampler = [device newSamplerStateWithDescriptor:sd];
 sd = [MTLSamplerDescriptor new]; sd.minFilter = sd.magFilter = MTLSamplerMinMagFilterLinear; sd.mipFilter = MTLSamplerMipFilterLinear;
 sansSampler = [device newSamplerStateWithDescriptor:sd];
 sd = [MTLSamplerDescriptor new]; sd.minFilter = sd.magFilter = MTLSamplerMinMagFilterLinear; sd.compareFunction = MTLCompareFunctionLess;
 shadowSampler = [device newSamplerStateWithDescriptor:sd];
 sd = [MTLSamplerDescriptor new]; sd.minFilter = sd.magFilter = MTLSamplerMinMagFilterLinear; sd.sAddressMode = sd.tAddressMode = MTLSamplerAddressModeClampToEdge;
 linear = [device newSamplerStateWithDescriptor:sd];
 MTLDepthStencilDescriptor *dd = [MTLDepthStencilDescriptor new];
 dd.depthCompareFunction = MTLCompareFunctionAlways; dd.depthWriteEnabled = YES; depthAlways = [device newDepthStencilStateWithDescriptor:dd];
 dd.depthCompareFunction = MTLCompareFunctionLess; depthLess = [device newDepthStencilStateWithDescriptor:dd];
 dd.depthWriteEnabled = NO; depthRead = [device newDepthStencilStateWithDescriptor:dd];
 sans = [self atlas];
 if (!sans) return @"the HUD's fonts are missing";
 return nil;
}
- (NSString *)loadPack {
 packBytes = [NSData dataWithContentsOfFile:config.pack options:NSDataReadingMappedIfSafe error:nil];
 // A pack the site serves gzipped (NAME.pack.gz) is inflated whole first, as host.js does.
 if (packBytes.length > 18 && ((const uint8_t *)packBytes.bytes)[0] == 0x1f && ((const uint8_t *)packBytes.bytes)[1] == 0x8b) packBytes = phInflate(packBytes);
 const uint8_t *b = packBytes.bytes; size_t n = packBytes.length;
 if (n < 32 || memcmp(b, "VPK1", 4)) return @"the realm's pack is missing";
 uint32_t count, table; memcpy(&count, b + 4, 4); memcpy(&table, b + 16, 4);
 if (table < 32 || table > n || count > (n - table) / 48) return @"the realm's pack is damaged";
 NSMutableArray<NSData *> *colour = [NSMutableArray array], *maps = [NSMutableArray array];
 NSData *catalog = nil;
 for (uint32_t k = 0; k < count; k++) {
  const uint8_t *row = b + table + (size_t)k * 48; uint32_t at, len; memcpy(&at, row + 4, 4); memcpy(&len, row + 8, 4);
  if (at > n || len > n - at) return @"the realm's pack is damaged";
  NSData *part = [NSData dataWithBytesNoCopy:(void *)(b + at) length:len freeWhenDone:NO];
  if (row[0] == 1 && !catalog) catalog = part;
  else if (row[0] == 2 && !packVertices) { packVertices = b + at; packVertexBytes = len; }
  else if (row[0] == 3 && !packIndices) { packIndices = b + at; packIndexBytes = len; }
  else if (row[0] == 4) [colour addObject:part];
  else if (row[0] == 6) [maps addObject:part];
 }
 if (!catalog || !packVertices || !packIndices) return @"the realm's pack lacks its catalog, vertices or indices";
 char progress[64]; int pn = snprintf(progress, sizeof progress, "%zu %zu", n, n);
 [self message:605 bytes:progress length:(size_t)pn];
 // asset 0 the catalog, copied in once its destination is measured; 1 and 2 the vertex and index bytes; 3 the layers
 A dest = x.asset ? x.asset(state, 0, (uint32_t)catalog.length) : 0;
 BOOL fits = dest && (!x.asset_capacity || catalog.length <= x.asset_capacity(state, 0));
 if (fits) memcpy((void *)dest, catalog.bytes, catalog.length);
 if (x.asset_ready) {
  x.asset_ready(state, 0, fits ? (uint32_t)catalog.length : 0);
  x.asset_ready(state, 1, (uint32_t)packVertexBytes); x.asset_ready(state, 2, (uint32_t)packIndexBytes);
 }
 storageBuffer = [device newBufferWithBytes:packVertices length:packVertexBytes options:MTLResourceStorageModeShared];
 indexBuffer = [device newBufferWithBytes:packIndices length:packIndexBytes options:MTLResourceStorageModeShared];
 int side = x.layers_side_v1 ? (int)x.layers_side_v1(state) : 0;
 if (side <= 0 || side > 4096) side = 1024;
 // The host's device may ask for smaller layers (a phone's tier, aethyr/render/texture-format.coax).
 int dataSide = 512;
 if (config.layerSide) { int s = config.layerSide(side), ds = config.layerSide(dataSide); if (s > 0 && s < side) side = s; if (ds > 0 && ds < dataSide) dataSide = ds; }
 sheets = [self layers:colour side:side data:NO];
 if (!sheets) return @"the realm's pack has a picture this Mac can't read";
 BOOL wantsData = x.data_layers_v1 && x.data_layers_v1() == 1;
 if (wantsData) {
  dataLayers = maps.count ? [self layers:maps side:dataSide data:YES] : nil;
  if (!dataLayers) {   // a pack without data layers binds one flat-normal texel
   MTLTextureDescriptor *d = [MTLTextureDescriptor new]; d.textureType = MTLTextureType2DArray; d.pixelFormat = MTLPixelFormatRGBA8Unorm; d.width = d.height = 1; d.arrayLength = 1;
   dataLayers = [device newTextureWithDescriptor:d]; const uint8_t flat[4] = {128, 128, 255, 255};
   [dataLayers replaceRegion:MTLRegionMake2D(0, 0, 1, 1) mipmapLevel:0 slice:0 withBytes:flat bytesPerRow:4 bytesPerImage:0];
  }
 }
 if (x.asset_ready) x.asset_ready(state, 3, (uint32_t)colour.count);
 // an asset or pack installed: the shadow tiles are drawn again
 if (x.shadow_event_v1) x.shadow_event_v1(state, 1, 0);
 [self after];
 // The pack's bytes are on the GPU and in the face now: nothing reads the file's copy again (the
 // index count is kept), so an inflated pack's memory goes back.
 packBytes = nil; packVertices = packIndices = NULL;
 return nil;
}

// ---- sockets and HTTP: bytes to the site and back, nothing else
- (void)push:(int)kind bytes:(NSData *)bytes owner:(id)owner closes:(BOOL)closes {
 PHEvent *e = [PHEvent new]; e->kind = kind; e->bytes = bytes; e->owner = owner; e->closes = closes;
 @synchronized (events) { [events addObject:e]; }
}
- (void)socket:(NSURLSessionWebSocketTask *)task event:(int)event bytes:(NSData *)bytes {
 PHSocket *s = nil;
 @synchronized (self) { for (int i = 0; i < 4; i++) if (sockets[i] && sockets[i]->task == task) s = sockets[i]; }
 if (!s) return;
 @synchronized (s) { if (s->ended) return; if (event == 2) s->ended = YES; }
 if (event != 1) phLog(@"socket %d %@", s->slot, @[@"opened", @"", @"closed", @"failed"][event]);
 [self push:256 + 4 * s->slot + event bytes:bytes owner:s closes:event == 2];
}
- (void)listen:(PHSocket *)s {
 __weak AEPageHost *weak = self;
 [s->task receiveMessageWithCompletionHandler:^(NSURLSessionWebSocketMessage *m, NSError *error) {
  AEPageHost *me = weak; if (!me || error) return;   // the task's own end says what happened
  // a text frame only: a socket is a byte pipe of text, as the browser host passes it
  if (m.type == NSURLSessionWebSocketMessageTypeString) [me socket:s->task event:1 bytes:[m.string dataUsingEncoding:NSUTF8StringEncoding]];
  [me listen:s];
 }];
}
- (int32_t)socketOpen:(NSString *)url {
 if (!session || !url) return -1;
 NSURL *u = [NSURL URLWithString:url];
 if (!u || !([u.scheme isEqual:@"wss"] || [u.scheme isEqual:@"ws"])) return -1;
 int slot = -1;
 @synchronized (self) { for (int i = 0; i < 4 && slot < 0; i++) if (!sockets[i]) slot = i; }
 if (slot < 0) return -1;
 NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:u];
 [r setValue:config.origin forHTTPHeaderField:@"Origin"];
 PHSocket *s = [PHSocket new]; s->slot = slot; s->task = [session webSocketTaskWithRequest:r];
 s->task.maximumMessageSize = 8 << 20;
 @synchronized (self) { sockets[slot] = s; }
 phLog(@"socket %d to %@://%@%@", slot, u.scheme, u.host, u.path);
 [self listen:s]; [s->task resume];
 return slot;
}
- (void)socketSend:(int32_t)id text:(NSString *)text {
 PHSocket *s = id >= 0 && id < 4 ? sockets[id] : nil;
 if (!s || !text || s->task.state != NSURLSessionTaskStateRunning) return;
 [s->task sendMessage:[[NSURLSessionWebSocketMessage alloc] initWithString:text] completionHandler:^(NSError *e) {}];
}
- (void)socketClose:(int32_t)id {
 PHSocket *s = id >= 0 && id < 4 ? sockets[id] : nil;
 if (!s) return;
 // The slot is free at once; the face hears the close a turn later, never inside its own call.
 @synchronized (self) { sockets[id] = nil; }
 @synchronized (s) { s->ended = YES; }
 [s->task cancelWithCloseCode:NSURLSessionWebSocketCloseCodeNormalClosure reason:nil];
 [self push:256 + 4 * id + 2 bytes:nil owner:nil closes:NO];
}
- (int32_t)http:(NSString *)path body:(NSData *)body flags:(int32_t)flags {
 // same site only: a path from its root, with nothing that could name another host
 if (!path.length || ![path hasPrefix:@"/"] || [path hasPrefix:@"//"] || [path containsString:@"\\"]) return -1;
 for (NSUInteger i = 0; i < path.length; i++) { unichar ch = [path characterAtIndex:i]; if (ch < 32 || ch == 127) return -1; }
 BOOL post = flags & 1, beacon = (flags & 2) != 0, wanted = (flags & 4) != 0;
 int slot = 0; id token = nil;
 if (wanted) {
  slot = -1; for (int i = 0; i < 8 && slot < 0; i++) if (!http[i]) slot = i;
  if (slot < 0) return -1;
  token = [NSObject new]; http[slot] = token;
 }
 if (!session) {   // offline: an answer that is wanted is the failure every host gives, "0\n"
  if (wanted) [self push:512 + slot bytes:[@"0\n" dataUsingEncoding:NSUTF8StringEncoding] owner:token closes:NO];
  return slot;
 }
 NSString *auth = nil;
 if ((flags & 16) && body.length) {
  const uint8_t *b = body.bytes; const uint8_t *nl = memchr(b, '\n', body.length);
  if (!nl) { if (wanted) http[slot] = nil; return -1; }
  auth = [[NSString alloc] initWithBytes:b length:(size_t)(nl - b) encoding:NSUTF8StringEncoding];
  body = [body subdataWithRange:NSMakeRange((size_t)(nl - b) + 1, body.length - (size_t)(nl - b) - 1)];
 }
 NSURL *u = [NSURL URLWithString:[config.origin stringByAppendingString:[path stringByAddingPercentEncodingWithAllowedCharacters:[NSCharacterSet characterSetWithCharactersInString:@"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~:/?#[]@!$&'()*+,;=%"]]]];
 if (!u) { if (wanted) http[slot] = nil; return -1; }
 NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:u];
 r.HTTPMethod = post ? @"POST" : @"GET"; r.HTTPShouldHandleCookies = NO;
 if (post) { r.HTTPBody = body ?: [NSData data]; [r setValue:@"application/json" forHTTPHeaderField:@"Content-Type"]; }
 [r setValue:config.origin forHTTPHeaderField:@"Origin"];
 if ((flags & 8) && playerKey) [r setValue:playerKey forHTTPHeaderField:@"X-Verse-Player"];
 if (auth) [r setValue:auth forHTTPHeaderField:@"Authorization"];
 __weak AEPageHost *weak = self; size_t cap = x.inbox_cap();
 [[session dataTaskWithRequest:r completionHandler:^(NSData *data, NSURLResponse *response, NSError *error) {
  AEPageHost *me = weak; if (!me || !wanted || beacon) return;
  NSInteger status = [response isKindOfClass:NSHTTPURLResponse.class] ? ((NSHTTPURLResponse *)response).statusCode : 0;
  phLog(@"%@ %@ answered %ld, %lu bytes", post ? @"POST" : @"GET", u.path, (long)status, (unsigned long)data.length);
  NSMutableData *out = [NSMutableData data];
  if (error || status == 301 || status == 302 || status == 303 || status == 307 || status == 308) [out appendBytes:"0\n" length:2];
  else {
   char head[16]; int hn = snprintf(head, sizeof head, "%ld\n", (long)status); [out appendBytes:head length:(size_t)hn];
   // an answer larger than the inbox arrives as its status alone, as the browser host gives it
   if (data.length <= cap - MIN(cap, (size_t)hn)) [out appendData:data];
  }
  [me push:512 + slot bytes:out owner:token closes:NO];
 }] resume];
 return slot;
}
// The events that arrived since the last frame, in order, for a few milliseconds at most.
- (void)drain {
 int64_t began = phMillis();
 for (;;) {
  PHEvent *e = nil;
  @synchronized (events) { if (events.count) { e = events[0]; [events removeObjectAtIndex:0]; } }
  if (!e) return;
  if (e->kind >= 512) {
   int id = e->kind - 512; if (http[id] != e->owner) continue;   // an answer for a request that was given up
   http[id] = nil; [self message:e->kind bytes:e->bytes.bytes length:e->bytes.length];
  } else {
   int id = (e->kind - 256) / 4;
   if (e->owner) {   // a live socket's: nothing from one the face closed, and its slot frees when it hears the close
    if (sockets[id] != e->owner) continue;
    if (e->closes) @synchronized (self) { sockets[id] = nil; }
   }
   // a frame larger than the inbox is an error event, never a cut frame
   if (e->bytes.length > x.inbox_cap()) [self message:256 + 4 * id + 3 bytes:NULL length:0];
   else [self message:e->kind bytes:e->bytes.bytes length:e->bytes.length];
  }
  if (phMillis() - began > 12) return;
 }
}

// ---- the pictures a page names (coaxial_media): each a file of the site's, drawn into its box
- (void)mediaSay {
 NSMutableArray *rows = [NSMutableArray array];
 for (NSString *url in [media.allKeys sortedArrayUsingSelector:@selector(compare:)]) { PHMedia *m = media[url]; if (m->ready || m->failed) [rows addObject:[NSString stringWithFormat:@"%@ %d %d", url, m->failed ? 0 : m->w, m->failed ? 0 : m->h]]; }
 NSString *said = [rows componentsJoinedByString:@"\n"];
 if ([said isEqual:mediaSaid]) return;
 mediaSaid = said; [self message:607 text:said];
}
- (PHMedia *)mediaOpen:(NSString *)url {
 PHMedia *m = [PHMedia new]; media[url] = m;
 // /worlds/NAME/... is the site's realms/NAME/...; no path leaves the site's files
 NSString *path = [url componentsSeparatedByString:@"?"][0];
 if ([path hasPrefix:@"/worlds/"]) path = [@"/realms/" stringByAppendingString:[path substringFromIndex:8]];
 NSData *d = [path containsString:@".."] || !config.web ? nil : [NSData dataWithContentsOfFile:[config.web stringByAppendingPathComponent:path]];
 CGImageSourceRef src = d ? CGImageSourceCreateWithData((__bridge CFDataRef)d, NULL) : NULL;
 CGImageRef image = src ? CGImageSourceCreateImageAtIndex(src, 0, NULL) : NULL;
 if (image) {
  size_t iw = CGImageGetWidth(image), ih = CGImageGetHeight(image); double f = MIN(1.0, 2048.0 / MAX(iw, ih));
  int w = MAX(1, (int)lround(iw * f)), h = MAX(1, (int)lround(ih * f));
  uint8_t *pixels = calloc((size_t)w * h, 4); CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
  CGContextRef g = CGBitmapContextCreate(pixels, w, h, 8, (size_t)w * 4, space, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGImageRef same = CGImageCreateCopyWithColorSpace(image, space);
  CGContextDrawImage(g, CGRectMake(0, 0, w, h), same ?: image);
  MTLTextureDescriptor *td = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatRGBA8Unorm width:w height:h mipmapped:NO];
  m->texture = [device newTextureWithDescriptor:td];
  [m->texture replaceRegion:MTLRegionMake2D(0, 0, w, h) mipmapLevel:0 withBytes:pixels bytesPerRow:(NSUInteger)w * 4];
  m->w = w; m->h = h; m->ready = m->texture != nil; m->failed = !m->ready;
  if (same) CGImageRelease(same); CGContextRelease(g); CGColorSpaceRelease(space); free(pixels); CGImageRelease(image);
 } else m->failed = YES;
 if (src) CFRelease(src);
 return m;
}
- (void)drawMedia:(id<MTLRenderCommandEncoder>)e over:(BOOL)over format:(MTLPixelFormat)format {
 if (!x.media || !x.media_len) return;
 A p = x.media(state); size_t n = x.media_len(state);
 // Contract v1: the packet lies whole inside the allocation the face names, or it is no packet.
 if (x.host_contract_version && x.host_contract_version() == 1 && n && (!x.media_capacity || n > x.media_capacity(state))) n = 0;
 NSMutableSet *wanted = [NSMutableSet set];
 NSString *text = n && p ? [[NSString alloc] initWithBytes:(void *)p length:n encoding:NSUTF8StringEncoding] : nil;
 NSArray<NSString *> *lines = text.length ? [text componentsSeparatedByString:@"\n"] : @[];
 for (NSUInteger li = 0; li < MIN(lines.count, (NSUInteger)112); li++) {
  NSArray<NSString *> *w = [lines[li] componentsSeparatedByString:@" "];
  if (w.count != 6 && w.count != 9) continue;
  BOOL isOver = [w[0] isEqual:@"over"]; NSString *url = w[1];
  if (!(isOver || [w[0] isEqual:@"image"]) || ![url hasPrefix:@"/"] || [url hasPrefix:@"//"]) continue;
  double v[7] = {0}; BOOL finite = YES; for (NSUInteger k = 2; k < w.count; k++) { v[k - 2] = w[k].doubleValue; if (!isfinite(v[k - 2])) finite = NO; }
  if (!finite) continue;
  [wanted addObject:url];
  if (isOver != over) continue;
  PHMedia *m = media[url] ?: [self mediaOpen:url];
  if (!m->ready) continue;
  BOOL band = w.count == 9; double bw = v[2] - v[0], bh = v[3] - v[1];
  if (bw <= 1 || bh <= 1) continue;
  // fit inside the box, centred; a band fills its box with a strip of the picture
  double f = band ? 0 : MIN(bw / m->w, bh / m->h), dw = band ? bw : m->w * f, dh = band ? bh : m->h * f, cx = v[0] + (bw - dw) / 2, cy = v[1] + (bh - dh) / 2;
  float u[8] = {(float)(cx / width * 2 - 1), (float)(1 - cy / height * 2), (float)((cx + dw) / width * 2 - 1), (float)(1 - (cy + dh) / height * 2),
   band ? (float)(v[4] / 100) : 1, band ? (float)(v[5] / 1000) : 0, band ? (float)(v[6] / 1000) : 1, 0};
  [e setRenderPipelineState:[self boxPipe:format]];
  [e setVertexBytes:u length:sizeof u atIndex:0]; [e setFragmentBytes:u length:sizeof u atIndex:0];
  [e setFragmentTexture:m->texture atIndex:0]; [e setFragmentSamplerState:linear atIndex:0];
  [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:6];
 }
 if (!over) for (NSString *url in media.allKeys) { PHMedia *m = media[url]; if ([wanted containsObject:url]) m->unseen = 0; else if (++m->unseen > 60) [media removeObjectForKey:url]; }
}
- (id<MTLRenderPipelineState>)boxPipe:(MTLPixelFormat)format {
 id<MTLRenderPipelineState> p = boxPipes[@(format)]; if (p) return p;
 MTLRenderPipelineDescriptor *d = [MTLRenderPipelineDescriptor new];
 d.vertexFunction = [glue newFunctionWithName:@"ph_box_vs"]; d.fragmentFunction = [glue newFunctionWithName:@"ph_box_fs"];
 d.colorAttachments[0].pixelFormat = format;
 p = [device newRenderPipelineStateWithDescriptor:d error:nil]; boxPipes[@(format)] = p; return p;
}

// ---- a frame (host.js draw)
- (void)drawList:(id<MTLRenderCommandEncoder>)e words:(const uint32_t *)list count:(uint32_t)count {
 size_t indices = packIndexBytes / 4;
 for (uint32_t i = 0; i < count; i++) {
  uint32_t first = list[i * 4], n = list[i * 4 + 1], base = list[i * 4 + 2], instances = list[i * 4 + 3];
  if (!n || !instances) continue;
  // a draw whose base instance has its top bit set draws its vertices in order, with no index buffer
  if (base & 0x80000000u) [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:first vertexCount:n instanceCount:instances baseInstance:base & 0x7fffffffu];
  else if (first <= indices && n <= indices - first) [e drawIndexedPrimitives:MTLPrimitiveTypeTriangle indexCount:n indexType:MTLIndexTypeUInt32 indexBuffer:indexBuffer indexBufferOffset:(NSUInteger)first * 4 instanceCount:instances baseVertex:0 baseInstance:base];
 }
}
- (id<MTLTexture>)frame:(CGSize)size scale:(CGFloat)scale dt:(float)dt wait:(BOOL)wait {
 if (closed) return screen;
 phCurrent = self;
 // The share of the device pixels a frame draws: the realm's own pixel budget, and the kept quality's cap.
 points = size;
 double f = 1, budget = x.pixel_budget ? x.pixel_budget() : 0, cap = quality == 1 ? 1.5 : quality == 2 ? 1 : INFINITY;
 if (budget > 0) f = MIN(1.0, MIN(sqrt(budget / (size.width * MAX(1.0, size.height))) / scale, cap / scale));
 else f = MIN(1.0, cap / scale);
 int w = MAX(1, (int)lround(size.width * scale * f)), h = MAX(1, (int)lround(size.height * scale * f));
 dpr = (float)(w / size.width);
 // The buffers below are shared with the GPU: the last frame is done with them before this one writes.
 [last waitUntilCompleted]; last = nil;
 [self drain];
 if (w != width || h != height || !screen) {
  width = w; height = h;
  light = [self target:MTLPixelFormatRGBA16Float width:w height:h]; aux = [self target:MTLPixelFormatRGBA16Float width:w height:h];
  depth = [self target:MTLPixelFormatDepth32Float width:w height:h];
  int qw = (w + 3) / 4, qh = (h + 3) / 4, ew = (w + 7) / 8, eh = (h + 7) / 8;
  q0 = [self target:MTLPixelFormatRGBA16Float width:qw height:qh]; q1 = [self target:MTLPixelFormatRGBA16Float width:qw height:qh];
  e0 = [self target:MTLPixelFormatRGBA16Float width:ew height:eh]; e1 = [self target:MTLPixelFormatRGBA16Float width:ew height:eh];
  screen = [self target:MTLPixelFormatBGRA8Unorm width:w height:h];
 }
 double t0 = CACurrentMediaTime();
 uint32_t n = x.frame(state, (A)vertexBytes.mutableBytes, (A)uniformBytes.mutableBytes, (float)w, (float)h, dpr, dt, 0);
 double t1 = CACurrentMediaTime();
 [self after];
 n = MIN(n, x.max_vertices());
 memcpy(uniformBuffer.contents, uniformBytes.bytes, uniformBytes.length);
 if (x.frame_store && x.frame_store_bytes) { size_t bytes = MIN((size_t)x.frame_store_bytes(state), frameBuffer.length) & ~(size_t)3; if (bytes) memcpy(frameBuffer.contents, (void *)x.frame_store(state), bytes); }
 if (n) memcpy(fxBuffer.contents, vertexBytes.bytes, (size_t)n * x.vertex_floats() * 4);
 BOOL scene = x.draws && x.draw_count && storageBuffer && sheets;
 uint32_t draws = scene ? x.draw_count(state) : 0;
 BOOL separate = x.shadow_draws_v1 && x.shadow_draw_count_v1;
 uint32_t shadowDraws = scene && separate ? x.shadow_draw_count_v1(state) : draws;
 if (draws || shadowDraws) for (uint32_t k = 0; k < stores.count; k++) {
  int64_t version = x.store_version ? (int64_t)x.store_version(state, (int32_t)k) : -2;
  if (version != -2 && storeVersions[k].longLongValue == version) continue;
  size_t bytes = MIN((size_t)x.store_bytes(state, (int32_t)k), stores[k].length) & ~(size_t)3;
  if (bytes) { memcpy(stores[k].contents, (void *)x.store(state, (int32_t)k), bytes); storeVersions[k] = @(version); }
 }
 const uint32_t *list = draws ? (const uint32_t *)x.draws(state) : NULL;
 const uint32_t *shadowList = separate && shadowDraws ? (const uint32_t *)x.shadow_draws_v1(state) : list;
 id<MTLCommandBuffer> cmd = [queue commandBuffer];
 PHTarget lit[2] = {{MTLPixelFormatRGBA16Float}, {MTLPixelFormatRGBA16Float}};
 PHShader *shadowShader = scene ? [self shader:@"shadow"] : nil;
 PHPipe *shadowPipe = shadowShader ? [self pipe:@"shadow" role:@"shadow" targets:NULL count:0 depth:YES] : nil;

 // The shadow map: the atlas the realm's Coax lays out in tiles (aethyr/render/cascades.coax), or one 2048 square.
 if (shadowPipe) {
  int aw = x.shadow_atlas_v1 && x.shadow_v1 && x.shadow_event_v1 ? (int)x.shadow_atlas_v1(state, config.touch ? 1 : 0, 0) : 0, ah = aw ? (int)x.shadow_atlas_v1(state, config.touch ? 1 : 0, 1) : 0;
  BOOL tiles = aw > 0 && ah > 0 && aw <= 8192 && ah <= 8192;
  if (!tiles) aw = ah = config.touch ? 1024 : 2048;
  if (!shadowMap || (int)shadowMap.width != aw || (int)shadowMap.height != ah || shadowTiles != tiles) { shadowMap = [self target:MTLPixelFormatDepth32Float width:aw height:ah]; shadowTiles = tiles; shadowFresh = tiles; }
  MTLRenderPassDescriptor *pd = [MTLRenderPassDescriptor new]; pd.depthAttachment.texture = shadowMap; pd.depthAttachment.storeAction = MTLStoreActionStore;
  if (tiles) {
   // open the tiles once the atlas is new or the engine has none; then at most the one tile it names, cleared, into its rect
   if (shadowFresh || x.shadow_v1(state, 6) <= 0) { x.shadow_event_v1(state, 0, 0); shadowFresh = NO; }
   int tile = x.shadow_v1(state, 0);
   if (tile >= 0 && !shadowDraws) x.shadow_event_v1(state, 2, 0);
   else if (tile >= 0) {
    pd.depthAttachment.loadAction = MTLLoadActionLoad;
    id<MTLRenderCommandEncoder> e = [cmd renderCommandEncoderWithDescriptor:pd];
    int clears = x.shadow_v1(state, 7), count = x.shadow_v1(state, 6);
    if (!clearPipe) { MTLRenderPipelineDescriptor *cd = [MTLRenderPipelineDescriptor new]; cd.vertexFunction = [glue newFunctionWithName:@"ph_clear_vs"]; cd.depthAttachmentPixelFormat = MTLPixelFormatDepth32Float; clearPipe = [device newRenderPipelineStateWithDescriptor:cd error:nil]; }
    [e setRenderPipelineState:clearPipe]; [e setDepthStencilState:depthAlways];
    for (int k = 0; k < count; k++) if (clears & (1 << k)) {
     int at = 16 + 4 * k;
     [e setViewport:(MTLViewport){x.shadow_v1(state, at), x.shadow_v1(state, at + 1), x.shadow_v1(state, at + 2), x.shadow_v1(state, at + 3), 0, 1}];
     [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3];
    }
    int rx = x.shadow_v1(state, 1), ry = x.shadow_v1(state, 2), rw = x.shadow_v1(state, 3), rh = x.shadow_v1(state, 4);
    [e setViewport:(MTLViewport){0, 0, aw, ah, 0, 1}];
    if (rx >= 0 && ry >= 0 && rw > 0 && rh > 0 && rx + rw <= aw && ry + rh <= ah) [e setScissorRect:(MTLScissorRect){(NSUInteger)rx, (NSUInteger)ry, (NSUInteger)rw, (NSUInteger)rh}];
    [self bind:shadowPipe to:e]; [e setDepthStencilState:depthLess]; [e setDepthBias:2 slopeScale:2 clamp:0];
    [self drawList:e words:shadowList count:shadowDraws];
    [e endEncoding];
   }
  } else if (draws || shadowDraws) {
   pd.depthAttachment.loadAction = MTLLoadActionClear; pd.depthAttachment.clearDepth = 1;
   id<MTLRenderCommandEncoder> e = [cmd renderCommandEncoderWithDescriptor:pd];
   [self bind:shadowPipe to:e]; [e setDepthStencilState:depthLess]; [e setDepthBias:2 slopeScale:2 clamp:0];
   [self drawList:e words:shadowList count:shadowDraws];
   [e endEncoding];
  }
 }

 // The world: the background, the scene's draws, the particles, into the light and its second target.
 BOOL post = scene && [self shader:@"post"] && [self shader:@"bloom"];
 MTLRenderPassDescriptor *pd = [MTLRenderPassDescriptor new];
 for (int i = 0; i < (post ? 2 : 1); i++) {
  pd.colorAttachments[i].texture = post ? (i ? aux : light) : screen; pd.colorAttachments[i].loadAction = MTLLoadActionClear;
  pd.colorAttachments[i].clearColor = MTLClearColorMake(0, 0, 0, 1); pd.colorAttachments[i].storeAction = MTLStoreActionStore;
 }
 if (scene) { pd.depthAttachment.texture = depth; pd.depthAttachment.loadAction = MTLLoadActionClear; pd.depthAttachment.clearDepth = 1; pd.depthAttachment.storeAction = post ? MTLStoreActionStore : MTLStoreActionDontCare; }
 PHTarget flat[1] = {{MTLPixelFormatBGRA8Unorm}};
 id<MTLRenderCommandEncoder> e = [cmd renderCommandEncoderWithDescriptor:pd];
 PHPipe *bg = [self pipe:post ? @"bg-lit" : @"bg" role:@"bg" targets:post ? lit : flat count:post ? 2 : 1 depth:scene];
 if (bg) { [self bind:bg to:e]; if (scene) [e setDepthStencilState:depthAlways]; [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3]; }
 if (draws) {
  PHPipe *sc = [self pipe:post ? @"scene-lit" : @"scene" role:@"scene" targets:post ? lit : flat count:post ? 2 : 1 depth:YES];
  if (sc) { [self bind:sc to:e]; [e setDepthStencilState:depthLess]; [self drawList:e words:list count:draws]; }
  uint32_t sparks = particleBuffer && [self shader:@"particle"] ? x.particle_instances(state) : 0;
  size_t bytes = sparks ? MIN((size_t)x.particle_bytes(state), particleBuffer.length) & ~(size_t)3 : 0;
  if (bytes) {
   memcpy(particleBuffer.contents, (void *)x.particles(state), bytes);
   // smoke first: over what is behind, taking that much of the light's bloom share; then the particles that add
   PHTarget smoke[2] = {{post ? MTLPixelFormatRGBA16Float : MTLPixelFormatBGRA8Unorm, YES, MTLBlendFactorOne, MTLBlendFactorOneMinusSourceAlpha, MTLBlendFactorZero, MTLBlendFactorOneMinusSourceAlpha}, {MTLPixelFormatRGBA16Float, NO, 0, 0, 0, 0, YES}};
   PHTarget added[2] = {{post ? MTLPixelFormatRGBA16Float : MTLPixelFormatBGRA8Unorm, YES, MTLBlendFactorOne, MTLBlendFactorOne, MTLBlendFactorOne, MTLBlendFactorOne}, {MTLPixelFormatRGBA16Float, NO, 0, 0, 0, 0, YES}};
   PHPipe *over = [self shader:@"particle_over"] ? [self pipe:@"particle_over" role:@"particle_over" targets:smoke count:post ? 2 : 1 depth:YES] : nil;
   PHPipe *add = [self pipe:@"particle" role:@"particle" targets:added count:post ? 2 : 1 depth:YES];
   if (over) { [self bind:over to:e]; [e setDepthStencilState:depthRead]; [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:6 instanceCount:sparks]; }
   if (add) { [self bind:add to:e]; [e setDepthStencilState:depthRead]; [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:6 instanceCount:sparks]; }
  }
 }
 if (post) {
  [e endEncoding];
  // Bloom: the bright parts to a quarter of the size, blurred across and down, then to an eighth,
  // blurred across and down; the realm says how many of the five passes this frame runs.
  static const float steps[5][4] = {{0, 0, 0, 4}, {1, 0, 1, 1}, {0, 1, 1, 1}, {1, 0, 1, 2}, {0, 1, 1, 1}};
  id<MTLTexture> into[5] = {q0, q1, q0, e1, e0}, from[5] = {light, q0, q1, q0, e1};
  int blooms = x.bloom_passes_v1 ? MAX(0, MIN(5, (int)x.bloom_passes_v1(state))) : 5;
  PHTarget one[1] = {{MTLPixelFormatRGBA16Float}};
  PHPipe *bloom = [self pipe:@"bloom" role:@"bloom" targets:one count:1 depth:NO];
  for (int k = 0; bloom && k < blooms; k++) {
   MTLRenderPassDescriptor *bd = [MTLRenderPassDescriptor new];
   bd.colorAttachments[0].texture = into[k]; bd.colorAttachments[0].loadAction = MTLLoadActionClear; bd.colorAttachments[0].clearColor = MTLClearColorMake(0, 0, 0, 1); bd.colorAttachments[0].storeAction = MTLStoreActionStore;
   id<MTLRenderCommandEncoder> b = [cmd renderCommandEncoderWithDescriptor:bd];
   bloomSource = from[k]; memcpy(bloomStep, steps[k], 16);
   [self bind:bloom to:b]; [b drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3]; [b endEncoding];
  }
  MTLRenderPassDescriptor *sd = [MTLRenderPassDescriptor new];
  sd.colorAttachments[0].texture = screen; sd.colorAttachments[0].loadAction = MTLLoadActionClear; sd.colorAttachments[0].clearColor = MTLClearColorMake(0, 0, 0, 1); sd.colorAttachments[0].storeAction = MTLStoreActionStore;
  e = [cmd renderCommandEncoderWithDescriptor:sd];
  PHPipe *po = [self pipe:@"post" role:@"post" targets:flat count:1 depth:NO];
  if (po) { [self bind:po to:e]; [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:3]; }
 }
 // The pictures under the strokes, the HUD's strokes, then the pictures drawn over them. Without a
 // post pass the strokes share the scene's pass, whose depth they neither test nor write.
 BOOL flatDepth = scene && !post;
 if (!flatDepth) [self drawMedia:e over:NO format:MTLPixelFormatBGRA8Unorm];
 if (n) {
  BOOL alpha = x.fx_alpha_v1 && x.fx_alpha_v1() == 1;
  // straight alpha over the world for a realm that asks; else the strokes' glow, which never doubles where they meet
  PHTarget stroke[1] = {{MTLPixelFormatBGRA8Unorm, YES, alpha ? MTLBlendFactorSourceAlpha : MTLBlendFactorOne, alpha ? MTLBlendFactorOneMinusSourceAlpha : MTLBlendFactorOne, MTLBlendFactorOne, alpha ? MTLBlendFactorOneMinusSourceAlpha : MTLBlendFactorOne}};
  PHPipe *fx = [self pipe:alpha ? @"fx-alpha" : @"fx-max" role:@"fx" targets:stroke count:1 depth:flatDepth];
  if (fx) {
   [self bind:fx to:e]; if (flatDepth) { if (!depthClear) { MTLDepthStencilDescriptor *dd = [MTLDepthStencilDescriptor new]; dd.depthCompareFunction = MTLCompareFunctionAlways; depthClear = [device newDepthStencilStateWithDescriptor:dd]; } [e setDepthStencilState:depthClear]; }
   [e setVertexBuffer:fxBuffer offset:0 atIndex:30]; [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:n];
  }
 }
 if (!flatDepth) [self drawMedia:e over:YES format:MTLPixelFormatBGRA8Unorm];
 [e endEncoding];
 double t2 = CACurrentMediaTime();
 [cmd commit]; last = cmd;
 if (wait) { [cmd waitUntilCompleted]; last = nil; }
 if (getenv("AETHYR_PAGE_TIMES")) {   // a developer's look at where a frame's time goes
  static double face, encode, gpu; static int frames;
  face += t1 - t0; encode += t2 - t1; if (wait) gpu += cmd.GPUEndTime - cmd.GPUStartTime;
  if (++frames == 120) { fprintf(stderr, "page host: %dx%d face %.2f ms, uploads and encoding %.2f ms, GPU %.2f ms a frame\n", width, height, face / frames * 1000, encode / frames * 1000, gpu / frames * 1000); face = encode = gpu = 0; frames = 0; }
 }
 [self mediaSay];
 return screen;
}
- (void)present:(id<MTLRenderCommandEncoder>)e format:(MTLPixelFormat)format {
 if (!screen) return;
 float u[8] = {-1, 1, 1, -1, 1, 0, 1, 0};
 [e setRenderPipelineState:[self boxPipe:format]];
 [e setVertexBytes:u length:sizeof u atIndex:0]; [e setFragmentBytes:u length:sizeof u atIndex:0];
 [e setFragmentTexture:screen atIndex:0]; [e setFragmentSamplerState:linear atIndex:0];
 [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:6];
}

// ---- input
- (BOOL)key:(int32_t)key mods:(int32_t)mods down:(BOOL)down {
 if (closed) return NO; phCurrent = self;
 uint32_t used = down ? (x.key ? x.key(state, key, mods) : 0) : (x.key_up ? x.key_up(state, key, mods) : 0);
 if (down) [self after];
 return used != 0;
}
- (void)text:(NSString *)typed { phCurrent = self; if (typed.length) [self message:1 text:typed]; }
- (void)pointer:(CGPoint)at phase:(int32_t)phase button:(int32_t)button {
 if (closed) return; phCurrent = self;
 if (x.pointer_buttons_v1) x.pointer_buttons_v1(state, (float)(at.x * dpr), (float)(at.y * dpr), phase, 0, 1, button);
 else if (x.pointer) x.pointer(state, (float)(at.x * dpr), (float)(at.y * dpr), phase, 0, 1);
 [self after];
}
// A finger, as host.js sends a touch: kind 1, its own number, and a cancel that ends a short, still
// touch is that tap's end (iOS cancels a touch a system gesture takes).
- (void)touch:(CGPoint)at phase:(int32_t)phase finger:(int32_t)finger {
 if (closed) return; phCurrent = self;
 NSNumber *k = @(finger); double now = CACurrentMediaTime();
 if (phase == 0) { fingers[k] = [NSValue valueWithBytes:&at objCType:@encode(CGPoint)]; fingerDown[k] = @(now); }
 else if (phase == 3 && fingers[k]) {
  CGPoint from; [fingers[k] getValue:&from];
  if (now - fingerDown[k].doubleValue < 0.6 && hypot(at.x - from.x, at.y - from.y) < 24) phase = 2;
 }
 if (phase >= 2) { [fingers removeObjectForKey:k]; [fingerDown removeObjectForKey:k]; }
 if (x.pointer_buttons_v1) x.pointer_buttons_v1(state, (float)(at.x * dpr), (float)(at.y * dpr), phase, 1, finger, phase == 1 ? -1 : 0);
 else if (x.pointer) x.pointer(state, (float)(at.x * dpr), (float)(at.y * dpr), phase, 1, finger);
 [self after];
}
- (void)wheel:(float)dy { if (!closed && x.wheel) { phCurrent = self; x.wheel(state, dy); } }
- (void)focus:(BOOL)focused { if (closed) return; phCurrent = self; if (focused) { if (x.focus) x.focus(state); } else if (x.blur) x.blur(state); }
- (void)shown:(BOOL)shown {
 if (closed) return; phCurrent = self;
 if (!shown && x.blur) x.blur(state);
 [self message:shown ? 602 : 601 bytes:NULL length:0];
 if (shown && x.focus) x.focus(state);
}
- (void)idle { if (closed) return; phCurrent = self; [self drain]; [self takeSounds]; }
// The agent driver (aethyr/agent/drive.coax), as host.js's coaxial.drive: a command in as message
// 700, the realm's answer out. nil for a realm that has no driver.
- (NSString *)drive:(NSString *)command {
 if (closed || !x.drive_out || !x.drive_out_len) return nil;
 phCurrent = self; [self message:700 text:command];
 A p = x.drive_out(state); size_t n = x.drive_out_len(state);
 return p && n ? [[NSString alloc] initWithBytes:(void *)p length:n encoding:NSUTF8StringEncoding] : @"";
}
- (int32_t)mouselook { return !closed && x.mouselook ? x.mouselook(state) : 0; }
- (void)pointerLock:(int32_t)status { if (!closed && x.pointer_lock) x.pointer_lock(state, status); }
- (void)look:(CGPoint)travel { if (!closed && x.look_delta) x.look_delta(state, (float)travel.x, (float)travel.y); }
- (BOOL)wantsText { return !closed && x.wants_text && x.wants_text(state) == 1; }
- (int32_t)cursor { return !closed && x.cursor ? x.cursor(state) : 0; }
- (void)close {
 if (closed) return;
 closed = YES; [last waitUntilCompleted]; last = nil;
 for (int i = 0; i < 4; i++) { if (sockets[i]) [sockets[i]->task cancel]; sockets[i] = nil; }
 [session invalidateAndCancel]; session = nil;
 if (phCurrent == self) phCurrent = nil;
 // the library stays loaded: its code may still be on a worker's stack, and a page opens once a run
}
- (void)dealloc { [self close]; }
@end

// ---- the face's imports (aethyr/host/README.md, Sockets and HTTP): they reach the page that is open
#define PH_IMPORT __attribute__((used, visibility("default")))
PH_IMPORT int32_t coaxial_socket_open(uintptr_t url, uint32_t length) {
 AEPageHost *h = phCurrent; if (!h || !url) return -1;
 return [h socketOpen:[[NSString alloc] initWithBytes:(void *)url length:length encoding:NSUTF8StringEncoding]];
}
PH_IMPORT void coaxial_socket_send(int32_t id, uintptr_t bytes, uint32_t length) {
 AEPageHost *h = phCurrent; if (!h || !bytes) return;
 [h socketSend:id text:[[NSString alloc] initWithBytes:(void *)bytes length:length encoding:NSUTF8StringEncoding]];
}
PH_IMPORT void coaxial_socket_close(int32_t id) { [phCurrent socketClose:id]; }
PH_IMPORT int32_t coaxial_http(uintptr_t url, uint32_t length, uintptr_t body, uint32_t body_length, int32_t flags) {
 AEPageHost *h = phCurrent; if (!h || !url) return -1;
 NSString *path = [[NSString alloc] initWithBytes:(void *)url length:length encoding:NSUTF8StringEncoding];
 if (!path) return -1;
 return [h http:path body:body && body_length ? [NSData dataWithBytes:(void *)body length:body_length] : nil flags:flags];
}
PH_IMPORT int64_t coaxial_now_ms(void) { return phMillis(); }

// ---- the page's sound, for the Apple audio host (audio-host.h; PageAudio.swift): its bank and its packets
const uint8_t *mf_sound_bank(void *host, size_t *count) {
 AEPageHost *h = (__bridge AEPageHost *)host; *count = 0;
 if (!h || h->closed || !h->x.sound_bank || !h->x.sound_bank_len || !h->x.sound_bank_capacity) return NULL;
 phCurrent = h;
 size_t n = h->x.sound_bank_len(h->state), capacity = h->x.sound_bank_capacity(h->state);
 A p = h->x.sound_bank(h->state);
 if (!p || n > capacity) return NULL;
 *count = n; return (const uint8_t *)p;
}
const uint8_t *mf_sound_bank_storage(void *host, size_t *capacity) {
 AEPageHost *h = (__bridge AEPageHost *)host; *capacity = 0;
 if (!h || h->closed || !h->x.sound_bank_storage || !h->x.sound_bank_storage_bytes) return NULL;
 A p = h->x.sound_bank_storage(h->state);
 if (!p) return NULL;
 *capacity = h->x.sound_bank_storage_bytes(h->state); return (const uint8_t *)p;
}
// The packets taken since the last call; the bytes stay until the next call.
const uint8_t *mf_sound_commands(void *host, size_t *count) {
 AEPageHost *h = (__bridge AEPageHost *)host; *count = 0;
 if (!h) return NULL;
 h->soundsGiven = h->soundsTaken; h->soundsTaken = nil;
 *count = h->soundsGiven.length; return h->soundsGiven.bytes;
}
// A page's face here has no spatial probe: every sound is the packet's own gain and pan.
int32_t mf_sound_probe(void *host, int32_t slot, float right, float ahead, float up) { (void)host; (void)slot; (void)right; (void)ahead; (void)up; return 0; }
