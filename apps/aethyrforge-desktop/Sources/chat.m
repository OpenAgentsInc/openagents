// Firm reason: 6 (AppKit text and input method, Metal presentation and process effects).
// The Coder chat (Forge.h ForgeChat). Coder owns agents, tools, history and consent; Coax owns the
// HUD, the presentation and the JSONL events (chat.coax behind face.h).
#import "Forge.h"
#import <Metal/Metal.h>
#include <unistd.h>

// The composer: Enter sends (without Shift, outside an input method's marked text), Escape closes.
@interface ForgeComposer : NSTextView
@property(nonatomic, copy) void (^sendAction)(void);
@property(nonatomic, copy) void (^dismiss)(void);
@end
@implementation ForgeComposer
- (void)keyDown:(NSEvent *)e {
  if ((e.keyCode == 36 || e.keyCode == 76) && !(e.modifierFlags & NSEventModifierFlagShift) && !self.hasMarkedText) { if (self.sendAction) self.sendAction(); return; }
  if (e.keyCode == 53 && !self.hasMarkedText) { if (self.dismiss) self.dismiss(); return; }
  [super keyDown:e];
}
@end

static NSString *utf8(const uint8_t *b, int64_t n) { return n > 0 ? [[NSString alloc] initWithBytes:b length:(NSUInteger)n encoding:NSUTF8StringEncoding] ?: @"" : @""; }
static NSColor *teal(void) { return NSColor.systemTealColor; }
static NSColor *orange(void) { return NSColor.systemOrangeColor; }

@implementation ForgeChat {
  NSTask *_process;
  NSTimer *_clock;
  int64_t _rendered;
  float *_strokes;
  int _strokeCap;
  id<MTLRenderPipelineState> _pipe;
}
- (BOOL)isFlipped { return YES; }
- (BOOL)isOpaque { return NO; }
- (int64_t)clockMs { return (int64_t)(NSProcessInfo.processInfo.systemUptime * 1000); }
- (int64_t)presentation { return fgc_chat_mode(self.composerOpen, self.busy, [self clockMs]); }
- (NSString *)messageText { uint8_t b[32768]; int64_t n = fgc_feed_copy(b, sizeof b); return utf8(b, n); }
- (NSURL *)settings { return [self.home URLByAppendingPathComponent:@".openagents/aethyrforge/chat.json"]; }
- (NSURL *)sessions { return [self.home URLByAppendingPathComponent:@".openagents/coder/sessions"]; }
- (NSURL *)cli { return self.cliOverride ?: [NSFileManager.defaultManager.homeDirectoryForCurrentUser URLByAppendingPathComponent:@".openagents/bin/openagents"]; }

- (instancetype)initWithFrame:(NSRect)frame {
  if (!(self = [super initWithFrame:frame])) return nil;
  NSFileManager *fm = NSFileManager.defaultManager;
  _session = @""; _parent = @""; _lastEnding = @""; _selectedEngine = @"auto";
  _home = fm.homeDirectoryForCurrentUser;
  _cwd = [_home URLByAppendingPathComponent:@".openagents/aethyrforge/workspace"];
  _rendered = -1;
  fgc_reset();
  self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
  _transcript = [NSTextView new]; ForgeComposer *input = [ForgeComposer new]; _input = input;
  for (NSTextView *v in @[ _transcript, _input ]) {
    v.richText = NO; v.font = [NSFont monospacedSystemFontOfSize:13 weight:NSFontWeightRegular]; v.textColor = [NSColor colorWithWhite:0.9 alpha:1];
    v.backgroundColor = NSColor.clearColor; v.drawsBackground = NO; v.textContainerInset = NSMakeSize(4, 6);
    v.automaticQuoteSubstitutionEnabled = NO; v.automaticDashSubstitutionEnabled = NO;
  }
  _transcript.editable = NO; _transcript.selectable = YES;
  _input.font = [NSFont monospacedSystemFontOfSize:12 weight:NSFontWeightRegular];
  _input.textContainerInset = NSMakeSize(4, 3);
  _transcriptScroll = [NSScrollView new]; _inputScroll = [NSScrollView new];
  NSArray *pairs = @[ @[ _transcriptScroll, _transcript ], @[ _inputScroll, _input ] ];
  for (NSArray *pair in pairs) {
    NSScrollView *scroll = pair[0]; NSTextView *text = pair[1];
    scroll.documentView = text; scroll.hasVerticalScroller = YES; scroll.drawsBackground = NO;
    text.verticallyResizable = YES; text.horizontallyResizable = NO;
    text.autoresizingMask = NSViewWidthSizable; text.textContainer.widthTracksTextView = YES;
    [self addSubview:scroll];
  }
  _router = [NSTextField labelWithString:@"Automatic routing · Elephant"];
  _heading = [NSTextField labelWithString:@"CODER / MESSAGES"];
  _status = [NSTextField labelWithString:@"Enter sends · Shift-Enter adds a line · Esc closes"];
  [_router setAccessibilityLabel:@"Automatic task routing"];
  _router.lineBreakMode = NSLineBreakByTruncatingTail;
  _folder = [NSButton new]; _send = [NSButton new]; _fresh = [NSButton new]; _allow = [NSButton new]; _deny = [NSButton new];
  _folder.title = @"Folder…"; _folder.target = self; _folder.action = @selector(chooseFolder);
  _send.title = @"Send ↵"; _send.target = self; _send.action = @selector(submit);
  _fresh.title = @"New chat"; _fresh.target = self; _fresh.action = @selector(newChat);
  _allow.title = @"Allow once"; _allow.target = self; _allow.action = @selector(approve);
  _deny.title = @"Deny"; _deny.target = self; _deny.action = @selector(reject);
  for (NSView *v in @[ _router, _folder, _send, _fresh, _allow, _deny, _heading, _status ]) [self addSubview:v];
  for (NSButton *b in @[ _folder, _send, _fresh, _allow, _deny ]) { b.controlSize = NSControlSizeSmall; b.font = [NSFont systemFontOfSize:11]; }
  _folder.bordered = NO; _fresh.bordered = NO;
  for (NSTextField *l in @[ _router, _heading, _status ]) { l.textColor = [NSColor colorWithCalibratedRed:0.4 green:0.9 blue:0.9 alpha:1]; l.font = [NSFont monospacedSystemFontOfSize:11 weight:NSFontWeightMedium]; }
  _heading.lineBreakMode = NSLineBreakByTruncatingMiddle;
  _status.font = [NSFont monospacedSystemFontOfSize:10 weight:NSFontWeightMedium];
  _status.lineBreakMode = NSLineBreakByTruncatingTail;
  [_input setAccessibilityLabel:@"Message to coding agent"];
  __weak ForgeChat *weak = self;
  input.sendAction = ^{ ForgeChat *s = weak; if (s && !s.busy) [s submit]; };
  input.dismiss = ^{ [weak closeComposer]; };
  [fm createDirectoryAtURL:_cwd withIntermediateDirectories:YES attributes:nil error:nil];
  NSData *data = [NSData dataWithContentsOfURL:self.settings];
  NSDictionary *saved = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
  NSString *path = [saved isKindOfClass:NSDictionary.class] ? saved[@"cwd"] : nil;
  if ([path isKindOfClass:NSString.class] && [fm fileExistsAtPath:path]) _cwd = [NSURL fileURLWithPath:path];
  // One shared decision-door config for Coder and every desktop client.
  NSURL *config = [_home URLByAppendingPathComponent:@".openagents/coder/router.json"];
  if (![fm fileExistsAtPath:config.path]) {
    [fm createDirectoryAtURL:config.URLByDeletingLastPathComponent withIntermediateDirectories:YES attributes:nil error:nil];
    uint8_t b[512]; int64_t n = fgc_router_default(b, sizeof b);
    if (n > 0) [[NSData dataWithBytes:b length:(NSUInteger)n] writeToURL:config options:NSDataWritingWithoutOverwriting error:nil];
  }
  [self update];
  return self;
}
- (void)dealloc { free(_strokes); }

- (NSRect)box:(int)kind {
  float v[4];
  for (int i = 0; i < 4; i++) v[i] = fgc_box(kind, (float)self.bounds.size.width, (float)self.bounds.size.height, i);
  return NSMakeRect(v[0], v[1], v[2], v[3]);
}
- (void)layout {
  [super layout];
  NSRect t = [self box:1];
  _heading.frame = NSMakeRect(NSMinX(t) + 8, NSMinY(t) + 4, t.size.width - 104, 16);
  _fresh.frame = NSMakeRect(NSMaxX(t) - 88, NSMinY(t) + 1, 80, 22);
  _transcriptScroll.frame = NSMakeRect(NSMinX(t) + 4, NSMinY(t) + 28, t.size.width - 8, t.size.height - (_pending ? 64 : 32));
  _allow.frame = NSMakeRect(NSMinX(t) + 4, NSMaxY(t) - 30, 90, 24);
  _deny.frame = NSMakeRect(NSMinX(t) + 100, NSMaxY(t) - 30, 60, 24);
  _router.frame = [self box:3]; _folder.frame = [self box:6];
  _inputScroll.frame = [self box:2]; _send.frame = [self box:7];
  _status.frame = NSMakeRect(28, self.bounds.size.height - 29, self.bounds.size.width - 56, 12);
  for (NSScrollView *scroll in @[ _transcriptScroll, _inputScroll ]) {
    NSTextView *text = scroll.documentView;
    [text setFrameSize:NSMakeSize(scroll.contentSize.width, MAX(text.frame.size.height, scroll.contentSize.height))];
  }
}
// Messages float directly over the world; only the open composer gets a faint tint.
- (void)drawRect:(NSRect)dirty {
  (void)dirty;
  if (self.presentation != 2) return;
  [[NSColor colorWithCalibratedWhite:0.08 alpha:0.22] setFill];
  [[NSBezierPath bezierPathWithRoundedRect:[self box:0] xRadius:6 yRadius:6] fill];
}
// Only the HUD's panels take the pointer; the rest of the world stays the world's.
- (NSView *)hitTest:(NSPoint)point {
  if (self.presentation != 2) return nil;
  NSPoint p = [self convertPoint:point fromView:self.superview];
  BOOL messages = NSPointInRect(p, [self box:1]) && (_transcript.string.length > 0 || _pending);
  if (!NSPointInRect(p, [self box:0]) && !messages && !NSPointInRect(p, _fresh.frame)) return nil;
  return [super hitTest:point];
}
- (BOOL)hudShown { return self.presentation == 2; }
- (BOOL)feedShown { return self.presentation == 1; }

- (int)hudVertices:(float *)out capacity:(int)cap width:(float)w height:(float)h scale:(float)d {
  int64_t mode = self.presentation;
  if (mode == 2) return (int)fgc_draw(out, cap * 12, w, h, d, _busy);
  if (mode == 1) return (int)fgc_feed_draw(out, cap * 12, w, h, d);
  return 0;
}
- (id<MTLRenderPipelineState>)pipelineOn:(id<MTLDevice>)device {
  if (_pipe) return _pipe;
  NSURL *url = [NSBundle.mainBundle URLForResource:@"HUD" withExtension:@"metal"];
  NSString *source = url ? [NSString stringWithContentsOfURL:url encoding:NSUTF8StringEncoding error:nil] : nil;
  id<MTLLibrary> lib = source ? [device newLibraryWithSource:source options:nil error:nil] : nil;
  if (!lib) return nil;
  MTLRenderPipelineDescriptor *p = [MTLRenderPipelineDescriptor new];
  p.vertexFunction = [lib newFunctionWithName:@"hud_vs"]; p.fragmentFunction = [lib newFunctionWithName:@"hud_fs"];
  MTLVertexDescriptor *vd = [MTLVertexDescriptor new];
  for (int i = 0; i < 3; i++) { vd.attributes[i].format = MTLVertexFormatFloat4; vd.attributes[i].offset = 16 * i; vd.attributes[i].bufferIndex = 0; }
  vd.layouts[0].stride = 48; p.vertexDescriptor = vd;
  MTLRenderPipelineColorAttachmentDescriptor *a = p.colorAttachments[0];
  a.pixelFormat = MTLPixelFormatBGRA8Unorm; a.blendingEnabled = YES;
  a.sourceRGBBlendFactor = MTLBlendFactorOne; a.destinationRGBBlendFactor = MTLBlendFactorOneMinusSourceAlpha;
  a.sourceAlphaBlendFactor = MTLBlendFactorOne; a.destinationAlphaBlendFactor = MTLBlendFactorOneMinusSourceAlpha;
  _pipe = [device newRenderPipelineStateWithDescriptor:p error:nil];
  return _pipe;
}
// The HUD on the host's pass (ui_layer encode): the whole view, w by h device pixels.
- (void)encodeHUD:(id<MTLRenderCommandEncoder>)e device:(id<MTLDevice>)device width:(uint32_t)w height:(uint32_t)h scale:(float)d {
  int64_t mode = self.presentation;
  if (mode != 1 && mode != 2) return;
  int cap = mode == 1 ? 2000000 / 12 : 48000 / 12;
  if (_strokeCap < cap) { free(_strokes); _strokes = malloc((size_t)cap * 48); _strokeCap = _strokes ? cap : 0; }
  if (!_strokes) return;
  int n = [self hudVertices:_strokes capacity:cap width:(float)w / d height:(float)h / d scale:d];
  id<MTLRenderPipelineState> pipe = [self pipelineOn:device];
  if (n <= 0 || !pipe) return;
  id<MTLBuffer> buffer = [device newBufferWithBytes:_strokes length:(NSUInteger)n * 48 options:MTLResourceStorageModeShared];
  float size[2] = {(float)w, (float)h};
  [e setRenderPipelineState:pipe];
  [e setVertexBuffer:buffer offset:0 atIndex:0];
  [e setVertexBytes:size length:sizeof size atIndex:1];
  [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:(NSUInteger)n];
}
// Offscreen acceptance uses the exact production stream and pipeline; the matte is this PNG's only.
- (int)saveFeedFrame:(NSURL *)png device:(id<MTLDevice>)device queue:(id<MTLCommandQueue>)queue {
  NSRect frame = NSInsetRect([self box:1], -4, -4);
  int width = (int)(frame.size.width * 2), height = (int)(frame.size.height * 2);
  id<MTLRenderPipelineState> pipe = [self pipelineOn:device];
  FORGE_REQUIRE(pipe, @"HUD Metal unavailable.");
  MTLTextureDescriptor *td = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm width:width height:height mipmapped:NO];
  td.usage = MTLTextureUsageRenderTarget; td.storageMode = MTLStorageModeShared;
  id<MTLTexture> texture = [device newTextureWithDescriptor:td];
  FORGE_REQUIRE(texture, @"HUD check texture unavailable.");
  int cap = 2000000 / 12;
  float *v = malloc((size_t)cap * 48);
  int n = v ? (int)fgc_feed_draw(v, (int64_t)cap * 12, (float)self.bounds.size.width, (float)self.bounds.size.height, 2) : 0;
  if (n <= 0) { free(v); forge_throw(@"HUD feed produced no vertices."); }
  for (int i = 0; i < n; i++) { v[i * 12] -= (float)frame.origin.x * 2; v[i * 12 + 1] -= (float)frame.origin.y * 2; }
  MTLRenderPassDescriptor *pass = [MTLRenderPassDescriptor new];
  pass.colorAttachments[0].texture = texture; pass.colorAttachments[0].loadAction = MTLLoadActionClear;
  pass.colorAttachments[0].storeAction = MTLStoreActionStore; pass.colorAttachments[0].clearColor = MTLClearColorMake(0.04, 0.06, 0.07, 1);
  id<MTLCommandBuffer> cmd = [queue commandBuffer];
  id<MTLRenderCommandEncoder> e = [cmd renderCommandEncoderWithDescriptor:pass];
  id<MTLBuffer> buffer = [device newBufferWithBytes:v length:(NSUInteger)n * 48 options:MTLResourceStorageModeShared];
  free(v);
  float size[2] = {(float)width, (float)height};
  [e setRenderPipelineState:pipe]; [e setVertexBuffer:buffer offset:0 atIndex:0];
  [e setVertexBytes:size length:sizeof size atIndex:1];
  [e drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:(NSUInteger)n]; [e endEncoding];
  [cmd commit]; [cmd waitUntilCompleted];
  FORGE_REQUIRE(cmd.status == MTLCommandBufferStatusCompleted, @"HUD frame readback failed.");
  NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:width pixelsHigh:height bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bitmapFormat:0 bytesPerRow:width * 4 bitsPerPixel:32];
  uint8_t *data = bitmap.bitmapData;
  [texture getBytes:data bytesPerRow:(NSUInteger)width * 4 fromRegion:MTLRegionMake2D(0, 0, width, height) mipmapLevel:0];
  // Metal's attachment is BGRA; the PNG bitmap takes RGBA.
  for (int i = 0; i < width * height * 4; i += 4) { uint8_t b = data[i]; data[i] = data[i + 2]; data[i + 2] = b; }
  NSData *bytes = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
  FORGE_REQUIRE(bytes && [bytes writeToURL:png atomically:YES], @"HUD PNG failed.");
  return n;
}

- (void)append:(NSString *)text color:(NSColor *)color {
  BOOL nearBottom = NSMaxY(_transcript.visibleRect) >= NSMaxY(_transcript.bounds) - 32;
  [_transcript.textStorage appendAttributedString:[[NSAttributedString alloc] initWithString:text attributes:@{NSForegroundColorAttributeName : color ?: [NSColor colorWithWhite:0.9 alpha:1], NSFontAttributeName : [NSFont monospacedSystemFontOfSize:13 weight:NSFontWeightRegular]}]];
  // The durable CLI log keeps the whole history; the view stays bounded.
  if (_transcript.string.length > 200000) [_transcript.textStorage deleteCharactersInRange:NSMakeRange(0, 50000)];
  if (nearBottom) [_transcript scrollToEndOfDocument:nil];
}
- (void)update {
  int64_t mode = self.presentation;
  _rendered = mode;
  for (NSView *v in @[ _inputScroll, _router, _folder, _send, _fresh, _heading, _status, _transcriptScroll ]) v.hidden = mode != 2;
  // A platform clock only: Coax owns whether the feed is alive and when its lifetime ends.
  if (mode == 1 && !_clock) {
    __weak ForgeChat *weak = self;
    _clock = [NSTimer timerWithTimeInterval:0.05 repeats:YES block:^(NSTimer *t) {
      (void)t; ForgeChat *s = weak;
      if (s && s.presentation != s->_rendered) [s update];
    }];
    [NSRunLoop.mainRunLoop addTimer:_clock forMode:NSRunLoopCommonModes];
  } else if (mode != 1) { [_clock invalidate]; _clock = nil; }
  _folder.enabled = !_busy; _fresh.enabled = !_busy;
  _send.title = _busy ? @"Stop" : @"Send ↵";
  _allow.hidden = mode != 2 || !_pending; _deny.hidden = mode != 2 || !_pending;
  _heading.stringValue = _pending ? @"CODER / APPROVAL NEEDED" : _busy ? [NSString stringWithFormat:@"CODER / WORKING · %@", _selectedEngine] : @"CODER / MESSAGES";
  _folder.title = [NSString stringWithFormat:@"%@ ▾", _cwd.lastPathComponent]; _folder.toolTip = _cwd.path;
  self.needsDisplay = YES; self.needsLayout = YES;
}
- (void)openComposer { _composerOpen = YES; if (_onActivity) _onActivity(); [self update]; [self.window makeFirstResponder:_input]; }
- (void)closeComposer { _composerOpen = NO; [self update]; if (_onClose) _onClose(); }
- (void)appendMessage:(NSString *)text {
  NSData *b = [text dataUsingEncoding:NSUTF8StringEncoding];
  fgc_feed_event(1, b.bytes, (int64_t)b.length, [self clockMs]);
  [self update]; [self layoutSubtreeIfNeeded];
}
- (void)clearMessages { fgc_feed_event(0, NULL, 0, [self clockMs]); }
- (void)finishMessages { fgc_feed_event(2, NULL, 0, [self clockMs]); [self update]; }
- (void)save {
  NSData *d = [NSJSONSerialization dataWithJSONObject:@{@"routing" : @"auto", @"cwd" : _cwd.path} options:NSJSONWritingSortedKeys error:nil];
  [d writeToURL:self.settings atomically:YES];
}
- (void)newChat {
  if (_busy) return;
  _parent = @""; _session = @""; _transcript.string = @""; [self clearMessages]; [self update]; [self.window makeFirstResponder:_input];
}
- (void)chooseFolder {
  NSOpenPanel *panel = [NSOpenPanel openPanel];
  panel.canChooseDirectories = YES; panel.canChooseFiles = NO; panel.allowsMultipleSelection = NO; panel.directoryURL = _cwd;
  if ([panel runModal] == NSModalResponseOK && panel.URL) {
    _cwd = panel.URL; _parent = @""; _session = @""; [self save];
    [self append:[NSString stringWithFormat:@"\nWorking folder: %@\n", panel.URL.path] color:teal()]; [self update];
  }
}
- (NSDictionary *)environment {
  NSMutableDictionary *env = [NSProcessInfo.processInfo.environment mutableCopy];
  NSURL *resources = NSBundle.mainBundle.resourceURL;
  NSFileManager *fm = NSFileManager.defaultManager;
  // Launch Services supplies a short PATH; agent CLIs keep their own logins.
  NSURL *realHome = fm.homeDirectoryForCurrentUser;
  NSURL *nodeRoot = [realHome URLByAppendingPathComponent:@".nvm/versions/node"];
  NSArray *versions = [[fm contentsOfDirectoryAtPath:nodeRoot.path error:nil] sortedArrayUsingComparator:^NSComparisonResult(NSString *a, NSString *b) { return [b compare:a options:NSNumericSearch]; }];
  NSMutableArray *path = [@[ [realHome URLByAppendingPathComponent:@".openagents/bin"].path, [realHome URLByAppendingPathComponent:@".local/bin"].path, @"/opt/homebrew/bin", @"/usr/local/bin", env[@"PATH"] ?: @"/usr/bin:/bin" ] mutableCopy];
  for (NSString *v in versions ?: @[]) [path addObject:[nodeRoot URLByAppendingPathComponent:[v stringByAppendingString:@"/bin"]].path];
  env[@"PATH"] = [path componentsJoinedByString:@":"];
  env[@"OPENAGENTS_CODER_BIN"] = [resources URLByAppendingPathComponent:@"coder"].path;
  env[@"CODER_MICRO_WORKER"] = [resources URLByAppendingPathComponent:@"mcworker"].path;
  [env removeObjectForKey:@"CODER_ASK_FILE"]; env[@"CODER_REALM_TOOLS"] = @"0";
  if (_sceneSessionURL) {
    env[@"AETHYRFORGE_SCENE_SESSION"] = _sceneSessionURL.path;
    env[@"AETHYRFORGE_SCENE_TOOL"] = [resources URLByAppendingPathComponent:@"aethyrforge-scene"].path;
  }
  env[@"BROWSER"] = @"/usr/bin/true"; env[@"NO_OPEN_BROWSER"] = @"1";
  if (![_home isEqual:realHome]) env[@"CODER_HOME"] = _home.path;
  return env;
}
- (void)submit {
  if (_busy) { [self stopTurn]; return; }
  [self startTurn:_input.string voice:NO];
}
// Reads a pipe's lines on a thread of its own, each handed to block on the main thread in order.
static void forge_lines(NSFileHandle *h, void (^line)(NSString *), void (^done)(NSString *problem)) {
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    NSMutableData *pending = [NSMutableData data];
    uint8_t bytes[8192];
    NSString *problem = nil;
    int fd = h.fileDescriptor;
    for (;;) {
      ssize_t n = read(fd, bytes, sizeof bytes);
      if (n < 0 && errno == EINTR) continue;
      if (n <= 0) break;
      [pending appendBytes:bytes length:(NSUInteger)n];
      for (;;) {
        const uint8_t *p = pending.bytes;
        const uint8_t *nl = memchr(p, '\n', pending.length);
        if (!nl) break;
        NSString *s = utf8(p, nl - p);
        [pending replaceBytesInRange:NSMakeRange(0, (NSUInteger)(nl - p) + 1) withBytes:NULL length:0];
        dispatch_sync(dispatch_get_main_queue(), ^{ line(s); });
      }
      if (pending.length > 262144) { problem = @"Coder output exceeded its line limit."; break; }
    }
    if (!problem && pending.length) { NSString *s = utf8(pending.bytes, (int64_t)pending.length); dispatch_sync(dispatch_get_main_queue(), ^{ line(s); }); }
    dispatch_async(dispatch_get_main_queue(), ^{ done(problem); });
  });
}
- (void)startTurn:(NSString *)draft voice:(BOOL)voice {
  if (_stopping) return;
  NSString *text = [draft stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
  NSData *b = [text dataUsingEncoding:NSUTF8StringEncoding];
  if (!fgc_send(_busy, b.bytes, (int64_t)b.length)) { _status.stringValue = @"Enter a message of at most 16 KiB."; return; }
  [self save]; if (_onActivity) _onActivity(); if (!voice) _input.string = @"";
  [self clearMessages]; _busy = YES; _pending = NO; _session = @""; _lastEnding = @"";
  _liveChunks = 0; _streamedWithoutComposer = NO;
  [self closeComposer];
  [self append:[NSString stringWithFormat:@"\n%@\n%@\n\n", voice ? @"VOICE" : @"YOU", text] color:NSColor.whiteColor];
  if (voice) [self appendMessage:[NSString stringWithFormat:@"You: %@\n\n", text]];
  NSMutableArray *args = [NSMutableArray arrayWithObject:@"coder"];
  if (_parent.length) [args addObjectsFromArray:@[ @"fork", _parent, text ]]; else [args addObjectsFromArray:@[ @"run", text ]];
  [args addObjectsFromArray:@[ @"--cwd", _cwd.path ]];
  if (_sceneContextURL) [args addObjectsFromArray:@[ @"--context", _sceneContextURL.path ]];
  for (int i = 0; i < 8; i++) {
    uint8_t o[128]; int64_t n = fgc_launch_option(i, o, sizeof o);
    if (n <= 0) break;
    NSString *option = utf8(o, n);
    [args addObject:[option isEqualToString:@"auto"] ? (_engineOverride ?: option) : option];
  }
  NSTask *p = [NSTask new];
  p.executableURL = self.cli; p.arguments = args; p.currentDirectoryURL = _cwd; p.environment = [self environment];
  NSPipe *output = [NSPipe pipe], *errors = [NSPipe pipe];
  p.standardOutput = output; p.standardError = errors; p.standardInput = NSFileHandle.fileHandleWithNullDevice;
  _selectedEngine = _engineOverride ?: @"auto";
  _router.stringValue = @"Automatic routing · deciding…";
  _process = p; _status.stringValue = @"Coder is routing this task…"; [self update];
  NSError *error = nil;
  if (![p launchAndReturnError:&error]) { [self append:[NSString stringWithFormat:@"\nCoder: %@\n", error.localizedDescription] color:orange()]; _lastEnding = @"failed"; [self endTurn]; return; }
  __block NSMutableString *problem = [NSMutableString string];
  __block int open = 2;
  __weak ForgeChat *weak = self;
  void (^finished)(void) = ^{
    ForgeChat *s = weak; if (!s) return;
    if (--open) return;
    [p waitUntilExit];
    if (!s.lastEnding.length) {
      [s append:[NSString stringWithFormat:@"\nCoder exited (%d). %@\n", p.terminationStatus, problem.length > 8192 ? [problem substringToIndex:8192] : problem] color:orange()];
      s.lastEnding = @"failed";
    }
    [s endTurn];
  };
  forge_lines(errors.fileHandleForReading, ^(NSString *line) { if (problem.length < 8192) [problem appendFormat:@"%@\n", line]; }, ^(NSString *why) { (void)why; finished(); });
  forge_lines(output.fileHandleForReading, ^(NSString *line) { if (line.length <= 262144) [weak receive:line]; }, ^(NSString *why) {
    ForgeChat *s = weak;
    if (why && s) {
      if (p.running) { [s stopTurn]; NSDate *end = [NSDate dateWithTimeIntervalSinceNow:2]; while (p.running && end.timeIntervalSinceNow > 0) usleep(50000); if (p.running) [p terminate]; }
      [s append:[NSString stringWithFormat:@"\nCoder: %@\n", why] color:orange()]; s.lastEnding = @"failed";
    }
    finished();
  });
}
- (void)endTurn {
  if (_session.length) _parent = _session;
  _busy = NO; _pending = NO; _process = nil; [self finishMessages];
  _status.stringValue = [NSString stringWithFormat:@"%@ · %@ · %@", _lastEnding, _selectedEngine, _session.length ? _session : _cwd.path];
}
- (NSString *)field:(int)i {
  NSMutableData *b = [NSMutableData dataWithLength:262144];
  int64_t n = fgc_copy(i, b.mutableBytes, (int64_t)b.length);
  return utf8(b.bytes, n);
}
- (void)receive:(NSString *)line {
  NSData *b = [line dataUsingEncoding:NSUTF8StringEncoding];
  int64_t kind = fgc_read(b.bytes, (int64_t)b.length);
  if (kind == 1) { _session = [self field:0]; NSString *engine = [self field:1]; if (engine.length) _selectedEngine = engine; [self update]; return; }
  NSString *text = [self field:1];
  switch (kind) {
  case 3: [self append:[NSString stringWithFormat:@"%@\n%@\n\n", _selectedEngine.uppercaseString, text] color:nil]; [self appendMessage:[text stringByAppendingString:@"\n\n"]]; break;
  case 4: [self append:[NSString stringWithFormat:@"› %@\n", text] color:teal()]; _status.stringValue = text; break;
  case 5: [self append:[text stringByAppendingString:@"\n"] color:NSColor.secondaryLabelColor]; break;
  case 6:
    _pending = YES; [self append:[NSString stringWithFormat:@"\nAPPROVAL NEEDED\n%@\n", text] color:orange()];
    [self appendMessage:[text stringByAppendingString:@"\nPress Enter to review this tool request.\n\n"]];
    _status.stringValue = @"Review the tool request, then Allow once or Deny."; [self update]; break;
  case 7: _lastEnding = text; [self append:[NSString stringWithFormat:@"\n[%@]\n", text] color:[text hasPrefix:@"finished"] ? teal() : orange()]; break;
  case 8: [self append:[NSString stringWithFormat:@"Edited: %@\n", text] color:teal()]; break;
  case 9: [self append:[NSString stringWithFormat:@"Model: %@\n", text] color:teal()]; _router.stringValue = text; _router.toolTip = text; break;
  case 10: [self append:[NSString stringWithFormat:@"Router: %@\n", text] color:teal()]; _router.stringValue = text; _router.toolTip = text; break;
  case 11: case 12:
    if (kind == 11) [self append:[_selectedEngine.uppercaseString stringByAppendingString:@"\n"] color:nil];
    [self append:text color:nil]; [self appendMessage:text]; _liveChunks++;
    if (_busy && !_composerOpen && !self.hudShown && _inputScroll.hidden && self.feedShown) _streamedWithoutComposer = YES;
    break;
  case 13: [self append:@"\n\n" color:nil]; [self appendMessage:@"\n\n"]; break;
  default: break;
  }
}
- (void)approve { [self answer:YES]; }
- (void)reject { [self answer:NO]; }
- (void)answer:(BOOL)yes {
  NSData *b = [_session dataUsingEncoding:NSUTF8StringEncoding];
  if (!fgc_answer(_busy, _pending, b.bytes, (int64_t)b.length)) return;
  NSError *error = nil;
  if ([[(yes ? @"y" : @"n") dataUsingEncoding:NSUTF8StringEncoding] writeToURL:[self.sessions URLByAppendingPathComponent:[_session stringByAppendingString:@".answer"]] options:NSDataWritingAtomic error:&error]) {
    _pending = NO; [self append:yes ? @"Allowed once.\n" : @"Denied.\n" color:teal()]; [self update];
  } else _status.stringValue = [NSString stringWithFormat:@"Cannot answer Coder: %@", error.localizedDescription];
}
- (void)stopTurn {
  if (!_busy) return;
  _pending = NO; [self update]; _status.stringValue = @"Stopping Coder…";
  // The CLI's stop file is polled even while an approval is pending.
  NSData *b = [_session dataUsingEncoding:NSUTF8StringEncoding];
  if (fgc_answer(_busy, YES, b.bytes, (int64_t)b.length))
    [[@"stop" dataUsingEncoding:NSUTF8StringEncoding] writeToURL:[self.sessions URLByAppendingPathComponent:[_session stringByAppendingString:@".stop"]] atomically:YES];
  else [_process terminate];
}
- (void)shutdown {
  [_clock invalidate]; _clock = nil;
  _stopping = YES; [self stopTurn];
  NSTask *p = _process;
  if (p) {
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow:2];
    while (p.running && end.timeIntervalSinceNow > 0) usleep(10000);
    if (p.running) [p terminate];
  }
}
@end
