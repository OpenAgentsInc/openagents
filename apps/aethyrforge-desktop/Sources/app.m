// Firm reason: 6 (AppKit, WebKit and AVFoundation effects; world, setup, chat and voice decisions are Coax).
// The Metaverse Engine for Mac on the shared Mac host (Forge.h). The host owns the window, the event loop and
// the menus; the world (WKWebView, or the native Metal realm), the chat's text views and the
// notice are native children under its layer (ui_child), and its layer draws the chat's HUD and
// the setup panel's components (chrome.coax) over them.
//   Aethyrforge [--world web|metal] [--data-dir DIR] [--mute-audio] [--offline]
//   Aethyrforge --shot OUT.png [--size WxH] [--scene setup|status|feed|composer] [--do "OP ; OP"] [--drive SOCKET]
//       draws one frame offscreen and exits: the setup panel, the status panel, or the chat's
//       message feed or composer, with no window, no world and no child process. --do and --drive then run
//       coax drive's operations on it: the panel's (tree, tap ID, about ID) and the host's (ax, ax-press ID, ax-key KEY).
//   the checks: --smoke-test, --chat-smoke, --input-smoke, --hotbar-smoke, --scene-smoke, --npc-smoke,
//       --audio-smoke, --voice-smoke, --voice-warm-smoke, --setup-smoke REPORT (a window that is
//       never shown); --launch-check, --microphone-check, --setup-profile, --offline-check,
//       --frame-profile, --web-preview-plan, --metal-measure-plan, --metal-observe (shown).
#import "Forge.h"
#import "Aethyrforge-Swift.h"
#import <QuartzCore/QuartzCore.h>
#include "../../shared/host/apple/ct-atlas.h"
#include <objc/message.h>
#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/file.h>
#include <sys/socket.h>
#include <time.h>

// ---- small effects shared by the files

void forge_throw(NSString *message) { @throw [NSException exceptionWithName:@"Aethyrforge" reason:message ?: @"failed" userInfo:nil]; }
NSString *forge_json(id object) {
  if (!object) return @"null";
  NSData *d = [NSJSONSerialization dataWithJSONObject:object options:NSJSONWritingSortedKeys | NSJSONWritingWithoutEscapingSlashes | NSJSONWritingFragmentsAllowed error:nil];
  return d ? [[NSString alloc] initWithData:d encoding:NSUTF8StringEncoding] : @"{}";
}
NSString *forge_json_pretty(id object) {
  NSData *d = [NSJSONSerialization dataWithJSONObject:object options:NSJSONWritingSortedKeys | NSJSONWritingPrettyPrinted error:nil];
  return d ? [[NSString alloc] initWithData:d encoding:NSUTF8StringEncoding] : @"{}";
}
id forge_parse(NSString *json) {
  if (![json isKindOfClass:NSString.class]) return nil;
  return [NSJSONSerialization JSONObjectWithData:[json dataUsingEncoding:NSUTF8StringEncoding] options:NSJSONReadingFragmentsAllowed error:nil];
}
void forge_write(NSString *text, NSURL *url) {
  NSError *e = nil;
  if (![[text dataUsingEncoding:NSUTF8StringEncoding] writeToURL:url options:NSDataWritingAtomic error:&e]) forge_throw(e.localizedDescription);
}
void forge_require(BOOL ok, NSString *format, ...) {
  if (ok) return;
  va_list a; va_start(a, format);
  NSString *message = [[NSString alloc] initWithFormat:format arguments:a];
  va_end(a);
  forge_throw(message);
}
void forge_sleep_ms(int ms) { usleep((useconds_t)ms * 1000); }
double forge_uptime(void) { return NSProcessInfo.processInfo.systemUptime; }
NSString *forge_setup_text(int64_t field) {
  NSMutableData *b = [NSMutableData data];
  for (int i = 0; i < 8192; i++) { int64_t c = fgs_text_byte(field, i); if (c < 0) break; uint8_t u = (uint8_t)c; [b appendBytes:&u length:1]; }
  return [[NSString alloc] initWithData:b encoding:NSUTF8StringEncoding] ?: @"";
}
NSData *forge_cli(NSURL *executable, NSArray<NSString *> *arguments, double timeout, NSDictionary *environment) {
  NSTask *p = [NSTask new];
  p.executableURL = executable; p.arguments = arguments;
  NSMutableDictionary *env = [(environment ?: NSProcessInfo.processInfo.environment) mutableCopy];
  NSString *shared = [NSFileManager.defaultManager.homeDirectoryForCurrentUser URLByAppendingPathComponent:@".openagents/bin"].path;
  env[@"PATH"] = [NSString stringWithFormat:@"%@:%@", shared, env[@"PATH"] ?: @"/usr/bin:/bin:/usr/sbin:/sbin"];
  p.environment = env;
  NSPipe *pipe = [NSPipe pipe];
  p.standardOutput = pipe; p.standardError = NSFileHandle.fileHandleWithNullDevice;
  NSError *error = nil;
  if (![p launchAndReturnError:&error]) forge_throw(error.localizedDescription);
  dispatch_block_t limit = dispatch_block_create(0, ^{ if (p.running) [p terminate]; });
  dispatch_block_t kill9 = dispatch_block_create(0, ^{ if (p.running) kill(p.processIdentifier, SIGKILL); });
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(timeout * NSEC_PER_SEC)), dispatch_get_global_queue(0, 0), limit);
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)((timeout + 2) * NSEC_PER_SEC)), dispatch_get_global_queue(0, 0), kill9);
  NSMutableData *data = [NSMutableData data];
  BOOL over = NO;
  for (;;) {
    NSData *part = [pipe.fileHandleForReading readDataUpToLength:8192 error:nil];
    if (!part.length) break;
    [data appendData:part];
    if (data.length > 65536) { [p terminate]; over = YES; break; }
  }
  [p waitUntilExit];
  dispatch_block_cancel(limit); dispatch_block_cancel(kill9);
  if (over) forge_throw(@"The local CLI report exceeded its size limit.");
  if (!data.length) forge_throw(@"The local CLI did not return a status report.");
  return data;
}

// Keep the reservations until distinct ports are chosen; the children bind right after release.
static int reserve_port(int *port) {
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd < 0) forge_throw(@"Cannot reserve a local port.");
  struct sockaddr_in a = {0};
  a.sin_len = sizeof a; a.sin_family = AF_INET; a.sin_addr.s_addr = inet_addr("127.0.0.1");
  socklen_t n = sizeof a;
  if (bind(fd, (struct sockaddr *)&a, sizeof a) != 0 || getsockname(fd, (struct sockaddr *)&a, &n) != 0) { close(fd); forge_throw(@"Cannot bind a local port."); }
  *port = ntohs(a.sin_port);
  return fd;
}

// The setup panel's atlas face (setup-face.c's fgu_ functions).
static const CtApi forge_ct = {fgu_glyph_count, fgu_glyph_code, fgu_font_count, fgu_font_size, fgu_font_weight, fgu_icon_count, fgu_icon_size, fgu_icon_name,
                               fgu_set_font, fgu_set_icon, fgu_set_atlas, fgu_set_dynamic, fgu_next_cluster, fgu_cluster, fgu_measured, fgu_next_cell, fgu_cell, fgu_cell_ready};

enum { C_QUIT = 1, C_RELOAD, C_PREVIEW, C_SETUP, C_STATUS, C_CHAT, C_FOLDER };
#define FORGE_MAX_VERTICES 32768

@interface ForgeApp ()
- (int)frame:(float *)v width:(float)w height:(float)h dpr:(float)dpr;
- (void)pointerAt:(float)x y:(float)y phase:(int)phase;
- (int)key:(int)key mods:(int)mods down:(int)down;
- (int)event:(NSEvent *)e;
- (void)life:(int)event;
- (void)command:(int)id;
- (int)commandState:(int)id;
- (void)focus:(int)on;
- (int)drive:(const char *)op out:(char *)out cap:(int)cap;
@end

static int cb_frame(void *u, float *v, float w, float h, float dpr, float dt) { (void)dt; return [(__bridge ForgeApp *)u frame:v width:w height:h dpr:dpr]; }
static void cb_pointer(void *u, float x, float y, int phase, int clicks, int mods) { (void)clicks; (void)mods; [(__bridge ForgeApp *)u pointerAt:x y:y phase:phase]; }
static void cb_wheel(void *u, float dy) { (void)u; (void)dy; }
static int cb_key(void *u, int key, int mods, int down) { return [(__bridge ForgeApp *)u key:key mods:mods down:down]; }
static void cb_text(void *u, const char *b, size_t n) { (void)u; (void)b; (void)n; }
static int cb_wants_text(void *u) { (void)u; return 0; }
static int cb_text_rect(void *u, float r[4]) { (void)u; (void)r; return 0; }
static int cb_hit(void *u, float x, float y) { (void)x; (void)y; return !((__bridge ForgeApp *)u).setupHidden; }
static void cb_life(void *u, int event) { [(__bridge ForgeApp *)u life:event]; }
static int cb_event(void *u, void *e) { return [(__bridge ForgeApp *)u event:(__bridge NSEvent *)e]; }
static void cb_command(void *u, int id) { [(__bridge ForgeApp *)u command:id]; }
static int cb_command_state(void *u, int id) { return [(__bridge ForgeApp *)u commandState:id]; }
static void cb_focus(void *u, int on) { [(__bridge ForgeApp *)u focus:on]; }
static int cb_drive(void *u, const char *op, char *out, int cap) { return [(__bridge ForgeApp *)u drive:op out:out cap:cap]; }
static void cb_prepare(void *u, void *cmd, uint32_t w, uint32_t h, float dpr);
static void cb_encode(void *u, void *enc, uint32_t w, uint32_t h, float dpr);
static int cb_changed(void *u);

@implementation ForgeApp {
  NSMutableArray<NSFileHandle *> *_logs;
  NSMutableSet<NSNumber *> *_coderHeldKeys;
  int _voiceReservation, _voicePort, _lockFD;
  int _voiceWarmEpoch, _voiceCaptureEpoch;
  int64_t _voiceCaptureRequest;
  BOOL _voiceWarmReady, _voiceWarming, _voiceStarting, _microphoneAsking;
  NSNumber *_worldPauseWanted;
  NSString *_metalLocalOrigin;
  dispatch_source_t _termination;
  double _startedAt;
  double _hiddenSince;
  // the setup panel's atlas: CoreText's fonts and icons, and its glyph cache
  CtAtlas *_ct;
  uint8_t *_full, *_cache, *_color;
  float _atlasDpr;
  UiHost *_shot;   // capturePanel's and --shot's offscreen host
  BOOL _picture;   // --shot: a press opens nothing (no world, no Finder, no Settings, no prompt; #13182)
  BOOL _pressing;  // a press on the setup panel, to its release
  float _panelW, _panelH;   // the panel's last frame, logical px (coax drive's rows are laid out in it)
  int _pressed;
}

- (NSString *)arg:(NSString *)flag {
  NSUInteger i = [self.args indexOfObject:flag];
  return i != NSNotFound && i + 1 < self.args.count ? self.args[i + 1] : nil;
}
- (BOOL)has:(NSString *)flag { return [self.args containsObject:flag]; }
- (NSURL *)fileArg:(NSString *)flag { NSString *v = [self arg:flag]; return v ? [NSURL fileURLWithPath:v] : nil; }
- (NSArray *)smokeFlags { return @[ @"--hotbar-smoke", @"--voice-warm-smoke", @"--audio-smoke", @"--npc-smoke", @"--scene-smoke", @"--input-smoke", @"--chat-smoke", @"--voice-smoke", @"--setup-smoke", @"--smoke-test" ]; }
- (BOOL)smoke { for (NSString *f in [self smokeFlags]) if ([self has:f]) return YES; return NO; }
- (NSURL *)report { for (NSString *f in [self smokeFlags]) if ([self has:f]) return [self fileArg:f]; return nil; }
- (NSURL *)launchReport { return [self fileArg:@"--launch-check"]; }
- (NSURL *)microphoneReport { return [self fileArg:@"--microphone-check"]; }
- (NSURL *)frameProfileURL { return [self fileArg:@"--frame-profile"]; }
- (NSURL *)setupProfileURL { return [self fileArg:@"--setup-profile"]; }
- (NSURL *)offlineReportURL { return [self fileArg:@"--offline-check"]; }
- (NSURL *)benchmarkReportURL { return [self has:@"--frame-benchmark"] ? self.frameProfileURL : nil; }
- (BOOL)offlineWorker { return [self has:@"--offline-worker"]; }
- (BOOL)offlineMode { return self.offlineReportURL != nil || [self has:@"--offline"]; }
- (BOOL)mute { return self.smoke || [self has:@"--mute-audio"]; }
- (BOOL)interactive { return !self.smoke && !self.launchReport && !self.microphoneReport && !self.frameProfileURL && !self.setupProfileURL && !self.offlineReportURL; }
- (int64_t)worldBackend {
  NSUInteger i = [self.args indexOfObject:@"--world"];
  if (i != NSNotFound && i + 1 == self.args.count) return -1;
  NSData *name = [(i != NSNotFound ? self.args[i + 1] : @"") dataUsingEncoding:NSUTF8StringEncoding];
  return forge_world_backend_bytes(name.bytes, (int64_t)name.length);
}
- (BOOL)nativeWorld { return self.worldBackend == 1; }
- (NSView *)worldView { return self.metalWorld ?: (NSView *)self.web; }
- (NSURL *)settingsURL {
  NSURL *isolated = (self.smoke ? self.report : self.microphoneReport).URLByDeletingLastPathComponent;
  if (isolated) return [isolated URLByAppendingPathComponent:@"setup.json"];
  return [NSFileManager.defaultManager.homeDirectoryForCurrentUser URLByAppendingPathComponent:@".openagents/aethyrforge/setup.json"];
}
- (NSURL *)localCLI {
  NSString *explicit = NSProcessInfo.processInfo.environment[@"OPENAGENTS_CLI"];
  if (explicit.length) return [NSURL fileURLWithPath:explicit];
  if (self.smoke) return [self.resources URLByAppendingPathComponent:@"openagents"];
  return [NSFileManager.defaultManager.homeDirectoryForCurrentUser URLByAppendingPathComponent:@".openagents/bin/openagents"];
}

// ---- off the main thread

- (void)main:(dispatch_block_t)block {
  if (NSThread.isMainThread) block(); else dispatch_sync(dispatch_get_main_queue(), block);
}
static NSString *webError(NSError *e) { return e.userInfo[@"WKJavaScriptExceptionMessage"] ?: e.localizedDescription; }
- (id)js:(NSString *)source {
  __block id result = nil; __block NSError *error = nil;
  dispatch_semaphore_t done = dispatch_semaphore_create(0);
  dispatch_async(dispatch_get_main_queue(), ^{
    [self.web evaluateJavaScript:source completionHandler:^(id r, NSError *e) { result = r; error = e; dispatch_semaphore_signal(done); }];
  });
  dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
  if (error) forge_throw(webError(error));
  return result == NSNull.null ? nil : result;
}
- (id)callJS:(NSString *)body arguments:(NSDictionary *)arguments {
  __block id result = nil; __block NSError *error = nil;
  dispatch_semaphore_t done = dispatch_semaphore_create(0);
  dispatch_async(dispatch_get_main_queue(), ^{
    [self.web callAsyncJavaScript:body arguments:arguments ?: @{} inFrame:nil inContentWorld:WKContentWorld.pageWorld completionHandler:^(id r, NSError *e) { result = r; error = e; dispatch_semaphore_signal(done); }];
  });
  dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
  if (error) forge_throw(webError(error));
  return result == NSNull.null ? nil : result;
}
static void forge_async(dispatch_block_t block) { dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), block); }

// ---- the window

- (void)openWindow {
  BOOL hidden = self.smoke;
  UiApp app = {0};
  app.title = "Aethyrforge"; app.app_id = "com.openagents.aethyrforge";
  app.width = 1280; app.height = 800; app.max_vertices = FORGE_MAX_VERTICES;
  NSString *shader = [self.resources URLByAppendingPathComponent:@"Component.metal"].path;
  app.shader = shader.fileSystemRepresentation;
  app.user = (__bridge void *)self;
  app.frame = cb_frame; app.pointer = cb_pointer; app.wheel = cb_wheel; app.key = cb_key; app.text = cb_text;
  app.wants_text = cb_wants_text; app.text_rect = cb_text_rect; app.command = cb_command; app.command_state = cb_command_state;
  app.focus = cb_focus; app.hit = cb_hit; app.life = cb_life; app.event = cb_event; app.hidden = hidden; app.drive = cb_drive;
  BOOL bench = self.benchmarkReportURL != nil;
  app.min_width = 900; app.min_height = bench ? 500 : 740;
  self.host = ui_open(&app, 0);
  if (!self.host) { fprintf(stderr, "Aethyrforge: the window did not open\n"); exit(1); }
  self.window = (__bridge NSWindow *)ui_window(self.host);
  NSString *test = [self arg:@"--test-window-title"];
  self.window.title = test ? [test stringByAppendingString:@" · Aethyrforge"] : @"Aethyrforge";
  self.window.subtitle = @"Everglade · Local edits · Experimental";
  if (bench) [self.window setContentSize:NSMakeSize(1280, 720)];
  [self.window center];
  UiLayer layer = {(__bridge void *)self, 0, cb_prepare, cb_encode, cb_changed};
  ui_layer(self.host, &layer);
  static const UiMenuItem menus[] = {
    {"Aethyrforge", "Quit Aethyrforge", "q", 8, C_QUIT, NULL},
    {"Edit", "Cut", "x", 8, -1, "cut:"}, {"Edit", "Copy", "c", 8, -1, "copy:"}, {"Edit", "Paste", "v", 8, -1, "paste:"}, {"Edit", "Select All", "a", 8, -1, "selectAll:"},
    {"View", "Reload Everglade", "r", 8, C_RELOAD, NULL}, {"View", "Check as Players See It", "", 0, C_PREVIEW, NULL},
    {"View", "Setup…", ",", 8, C_SETUP, NULL}, {"View", "Status…", "i", 8, C_STATUS, NULL},
    {"View", "Message Coding Agent", "l", 8, C_CHAT, NULL}, {"View", "Open Local World Folder", "o", 8 | 1, C_FOLDER, NULL},
    {"Window", "Minimize", "m", 8, -1, "performMiniaturize:"},
  };
  ui_menus(self.host, menus, (int)(sizeof menus / sizeof menus[0]));
  ui_menu_title(self.host, C_PREVIEW, [self previewTitle].UTF8String);

  WKWebViewConfiguration *config = [WKWebViewConfiguration new];
  WKUserContentController *ucc = config.userContentController;
  void (^script)(NSString *) = ^(NSString *s) { [ucc addUserScript:[[WKUserScript alloc] initWithSource:s injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]]; };
  if (bench) {
    NSString *bs = [NSString stringWithContentsOfURL:[self.resources URLByAppendingPathComponent:@"host/everglade-benchmark.js"] encoding:NSUTF8StringEncoding error:nil];
    if (bs) script([NSString stringWithFormat:@"globalThis.coaxialBenchmarkOptions={mode:'replay',fixture:'/build/bench/raid.json',timestamps:%@};\n%@", [self has:@"--bench-no-timestamps"] ? @"false" : @"true", bs]);
  }
  NSInteger displayHz = NSScreen.mainScreen.maximumFramesPerSecond ?: 60;
  if (forge_high_refresh(displayHz)) self.highRefreshEnabled = [self configureHighRefresh:config.preferences];
  NSInteger preferredHz = bench ? 60 : self.highRefreshEnabled ? displayHz : 60;
  script([NSString stringWithFormat:@"globalThis.coaxialPreferredFrameRate=%ld;globalThis.coaxialMinimumFrameRate=60;", (long)preferredHz]);
  if (self.frameProfileURL) script(@"globalThis.coaxialFrameTrace=true;");
  [ucc addScriptMessageHandler:self name:@"forgeVoiceStatus"];
  [ucc addScriptMessageHandler:self name:@"forgeVoiceCoder"];
  [ucc addScriptMessageHandler:self name:@"forgeMetalMessage"];
  [ucc addScriptMessageHandlerWithReply:self contentWorld:WKContentWorld.pageWorld name:@"forgeVoiceCapture"];
  if (!self.smoke || [self has:@"--voice-smoke"] || [self has:@"--voice-warm-smoke"])
    script(@"globalThis.coaxialVoicePrepareCapture = capture => webkit.messageHandlers.forgeVoiceCapture.postMessage({schema:'aethyrforge.microphone-capture.v1',kind:'prepare',capture});\n"
           @"globalThis.coaxialVoiceCancelCapture = capture => webkit.messageHandlers.forgeVoiceCapture.postMessage({schema:'aethyrforge.microphone-capture.v1',kind:'cancel',capture});\n");
  { uint8_t vs[1024]; int64_t vn = fgv_page_script(vs, sizeof vs); if (vn > 0) script([[NSString alloc] initWithBytes:vs length:(NSUInteger)vn encoding:NSUTF8StringEncoding]); }
  config.websiteDataStore = WKWebsiteDataStore.nonPersistentDataStore;
  if (self.offlineReportURL) script(@"globalThis.__forgeRoomWorkers=0;const NativeWorker=Worker;globalThis.Worker=class extends NativeWorker{constructor(...args){super(...args);__forgeRoomWorkers++;}};");
  if (self.setupProfileURL) script(@"globalThis.__forgeErrors=[];addEventListener('error',e=>__forgeErrors.push(String(e.message)));addEventListener('unhandledrejection',e=>__forgeErrors.push(String(e.reason)));");
  if (self.launchReport) script(@"globalThis.__forgeFrameCount=0;const nativeRAF=requestAnimationFrame;globalThis.requestAnimationFrame=f=>nativeRAF(t=>{__forgeFrameCount++;f(t);});");
  config.mediaTypesRequiringUserActionForPlayback = WKAudiovisualMediaTypeNone;
  [self configureAudioOutput:config muted:self.mute || self.launchReport || self.microphoneReport || self.frameProfileURL || self.setupProfileURL || self.offlineReportURL];
  if (self.smoke) {
    // The check pumps the callback: WebKit throttles animation callbacks and timers in an unordered window.
    script(@"globalThis.requestAnimationFrame = f => { globalThis.__forgeFrame = f; return 1; }; globalThis.cancelAnimationFrame = () => {globalThis.__forgeFrame = null;}; Object.defineProperty(document,'hidden',{get:()=>false}); Object.defineProperty(document,'visibilityState',{get:()=> 'visible'}); document.hasFocus=()=>true;");
    script(@"globalThis.__forgeRequests=[]; const originalFetch=fetch; globalThis.fetch=(...a)=>{const e={url:String(a[0]),method:a[1]?.method||'GET',status:'pending'};if(e.url.includes('world/everglade'))__forgeRequests.push(e);return originalFetch(...a).then(r=>{e.status=r.status;return r;},x=>{e.status=String(x);throw x;});};");
  }
  NSView *content = (__bridge NSView *)ui_view(self.host);
  NSRect bounds = content.bounds;
  self.web = [[WKWebView alloc] initWithFrame:bounds configuration:config];
  self.web.navigationDelegate = self; self.web.UIDelegate = self;
  self.web.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  ui_child(self.host, (__bridge void *)self.web, 0);
  if (self.nativeWorld) {
    self.web.hidden = YES;
    ForgeMetalWorld *view = [self makeMetalWorld:bounds];
    view.preferredHz = (float)preferredHz; self.metalWorld = view;
    ui_child(self.host, (__bridge void *)view, 0);
  }
  self.notice = [NSTextField labelWithString:@"Starting local Everglade…"];
  self.notice.textColor = NSColor.whiteColor; self.notice.backgroundColor = NSColor.blackColor; self.notice.drawsBackground = YES;
  self.notice.alignment = NSTextAlignmentCenter; self.notice.frame = NSMakeRect(0, 0, bounds.size.width, 28);
  self.notice.autoresizingMask = NSViewWidthSizable | NSViewMaxYMargin;
  ui_child(self.host, (__bridge void *)self.notice, 0);
  self.chat = [[ForgeChat alloc] initWithFrame:bounds];
  self.chat.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  __weak ForgeApp *weak = self;
  self.chat.onActivity = ^{ [weak cancelVoice:@"coder-activity"]; };
  self.chat.onClose = ^{ ForgeApp *s = weak; [s.window makeFirstResponder:s.worldView]; };
  ui_child(self.host, (__bridge void *)self.chat, 0);
  fgs_reset();
  NSData *saved = [NSData dataWithContentsOfURL:self.settingsURL];
  if (saved) @try { [self acceptSetup:saved kind:3]; } @catch (NSException *e) { (void)e; }
  self.setupHidden = fgs_completed();
  self.refreshTitle = @"Refresh"; self.checking = YES;
  if (self.launchReport || self.microphoneReport) self.window.level = NSFloatingWindowLevel;
  [self updateSetup];
  [self refreshMicrophoneSetup];
}
- (BOOL)configureHighRefresh:(WKPreferences *)preferences {
  // This experimental, ad-hoc signed shell uses WebKit's feature SPI; it never changes Safari's or the system's defaults.
  SEL features = NSSelectorFromString(@"_features"), setter = NSSelectorFromString(@"_setEnabled:forFeature:"), getter = NSSelectorFromString(@"_isEnabledForFeature:");
  if (![WKPreferences respondsToSelector:features] || ![preferences respondsToSelector:setter] || ![preferences respondsToSelector:getter]) return NO;
  NSArray *all = ((id (*)(id, SEL))objc_msgSend)(WKPreferences.class, features);
  id feature = nil;
  for (NSObject *f in all) if ([[f valueForKey:@"key"] isEqual:@"PreferPageRenderingUpdatesNear60FPSEnabled"]) feature = f;
  if (!feature) return NO;
  ((void (*)(id, SEL, BOOL, id))objc_msgSend)(preferences, setter, NO, feature);
  return !((BOOL (*)(id, SEL, id))objc_msgSend)(preferences, getter, feature);
}
- (void)configureAudioOutput:(WKWebViewConfiguration *)config muted:(BOOL)muted {
  // Test output stays connected so AudioWorklets run; ordinary windows use the realm's normal destination.
  if (!muted) return;
  [config.userContentController addUserScript:[[WKUserScript alloc] initWithSource:@"globalThis.coaxialAudioTap = c => {const g=c.createGain();g.gain.value=0;g.connect(c.destination);return g;};" injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]];
}
- (ForgeMetalWorld *)makeMetalWorld:(NSRect)frame {
  ForgeMetalWorld *view = [[ForgeMetalWorld alloc] initWithFrame:frame];
  view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  __weak ForgeApp *weak = self;
  view.failed = ^(NSString *m) { [weak fail:m]; };
  view.action = ^(int32_t code, NSString *text) { [weak metalAction:code text:text]; };
  return view;
}

// ---- the setup panel on the host's layer

- (BOOL)enterEnabled { return self.statusMode || (!self.setupError && !self.checking && !self.verifying && fgs_can_enter(self.sceneReady)); }
- (BOOL)refreshEnabled { return !self.checking && !self.verifying; }
- (BOOL)microphoneEnabled { return fgs_status(3) != 0; }
- (NSString *)microphoneTitle { return forge_setup_text(15); }
- (NSString *)microphoneStatusText { return forge_setup_text(14); }
- (void)updateSetup {
  NSData *e = [self.setupError ?: @"" dataUsingEncoding:NSUTF8StringEncoding];
  fgu_update(self.sceneReady, self.checking, self.verifying, self.statusMode, self.worldPaused, e.bytes, (int64_t)e.length);
  self.refreshTitle = self.setupError ? @"Retry startup" : @"Refresh";
}
- (BOOL)atlasAt:(float)dpr {
  if (_ct && _atlasDpr == dpr) return YES;
  ct_atlas_free(_ct); free(_full); free(_cache); free(_color); _full = _cache = _color = NULL;
  _ct = ct_atlas_new(&forge_ct, self.resources.path.fileSystemRepresentation, dpr);
  if (!_ct) return NO;
  _atlasDpr = dpr;
  uint32_t w = ct_atlas_width(_ct), rows = ct_atlas_height(_ct), y0 = ct_atlas_cache_y0(_ct);
  _full = malloc((size_t)w * rows); _cache = calloc(1, (size_t)w * (rows - y0)); _color = calloc(1, (size_t)w * ct_atlas_color_height(_ct) * 4);
  if (!_full || !_cache || !_color) return NO;
  memcpy(_full, ct_atlas_mono(_ct), (size_t)w * rows);
  ct_atlas_bind(_ct, fgu_state());
  ui_atlas(self.host, _full, w, rows, _color, w, ct_atlas_color_height(_ct));
  if (_shot) ui_atlas(_shot, _full, w, rows, _color, w, ct_atlas_color_height(_ct));
  return YES;
}
// The graphemes and cells the last frame wanted, drawn; the rows that changed, uploaded.
- (void)serveAtlas {
  int y0, y1, cy0, cy1;
  if (ct_atlas_serve(_ct, fgu_state(), _cache, _color, &y0, &y1, &cy0, &cy1) <= 0) return;
  uint32_t w = ct_atlas_width(_ct), top = ct_atlas_cache_y0(_ct);
  if (y1 > y0) memcpy(_full + (size_t)y0 * w, _cache + (size_t)(y0 - (int)top) * w, (size_t)(y1 - y0) * w);
  ui_atlas_rows(self.host, _full, (uint32_t)y0, (uint32_t)y1, _color, (uint32_t)cy0, (uint32_t)cy1);
  if (_shot) ui_atlas_rows(_shot, _full, (uint32_t)y0, (uint32_t)y1, _color, (uint32_t)cy0, (uint32_t)cy1);
}
- (int)frame:(float *)v width:(float)w height:(float)h dpr:(float)dpr {
  if (dpr > 0) { _panelW = w / dpr; _panelH = h / dpr; }
  if (self.setupHidden || ![self atlasAt:dpr]) return 0;
  [self updateSetup];
  int64_t n = 0;
  for (int i = 0; i < 3; i++) { n = fgu_draw(v, (int64_t)FORGE_MAX_VERTICES * 28, w, h, dpr); [self serveAtlas]; }
  self.panelVertices = (int)n;
  return (int)n;
}
- (void)pointerAt:(float)x y:(float)y phase:(int)phase {
  if (self.setupHidden) return;
  float d = ui_dpr(self.host);
  NSSize s = ((__bridge NSView *)ui_view(self.host)).bounds.size;
  [self updateSetup];
  int b = (int)fgu_hit((float)s.width, (float)s.height, x / d, y / d);
  if (phase == 0) { _pressing = YES; _pressed = b; return; }
  if (phase == 2 && _pressing) { _pressing = NO; if (b && b == _pressed) [self setupButton:b]; }
}
// What a press does; in a --shot picture, what it would open instead of opening it (nil when it acted).
- (NSString *)setupButton:(int)b {
  if (_picture && b != 2 && b != 3) return b == 1 ? @"would enter the world (a picture starts nothing)" : b == 4 ? @"would open ~/.openagents in Finder (a picture opens nothing)" : @"would ask for the microphone (a picture opens nothing)";
  switch (b) {
  case 1: [self enterWorld]; break;
  case 2: if (self.stopping) [self restartStartup]; else [self refreshSetup:NO]; break;
  case 3: [self refreshSetup:YES]; break;
  case 4: [NSWorkspace.sharedWorkspace openURL:[NSFileManager.defaultManager.homeDirectoryForCurrentUser URLByAppendingPathComponent:@".openagents"]]; break;
  case 5: [self microphoneSetupPressed]; break;
  }
  return nil;
}
// coax drive's operations on the setup panel (chrome.coax fui_tree): `tree`, `tap ID` and `about ID`. The shared
// Mac host makes the same rows VoiceOver's elements and presses them with `tap` (ui-host.m, #13180).
- (int)drive:(const char *)op out:(char *)out cap:(int)cap {
  if (cap < 2) return 0;
  int n = -1;
  BOOL tap = !strncmp(op, "tap ", 4), about = !strncmp(op, "about ", 6);
  if (!strcmp(op, "tree")) {
    [self updateSetup];
    n = self.setupHidden || _panelW <= 0 ? snprintf(out, (size_t)cap, "(nothing yet: no rows drawn)\n") : (int)fgu_tree(_panelW, _panelH, (uint8_t *)out, cap);
  } else if (tap || about) {
    const char *id = op + (tap ? 4 : 6);
    int64_t b = self.setupHidden ? 0 : fgu_element(id, (int64_t)strlen(id));
    [self updateSetup];
    if (!b) n = snprintf(out, (size_t)cap, "no row %s in the tree\n", id);
    else if (about) n = (int)fgu_about(b, (uint8_t *)out, cap);
    else {
      // pressed where it is drawn, as a click would be: a button that is not enabled takes nothing
      float x = fgu_box(b, _panelW, _panelH, 0) + fgu_box(b, _panelW, _panelH, 2) / 2, y = fgu_box(b, _panelW, _panelH, 1) + fgu_box(b, _panelW, _panelH, 3) / 2;
      if (fgu_hit(_panelW, _panelH, x, y) == b) { NSString *note = [self setupButton:(int)b]; n = note ? snprintf(out, (size_t)cap, "%s\n", note.UTF8String) : 0; }
      else n = snprintf(out, (size_t)cap, "%s is not enabled\n", id);
    }
  }
  return n < 0 ? -1 : n >= cap ? cap - 1 : n;
}
- (int)key:(int)key mods:(int)mods down:(int)down {
  (void)mods;
  if (self.setupHidden || !down) return 0;
  if (key == -2 || key == -1) { if (self.enterEnabled) [self enterWorld]; return 1; }
  return 0;
}
- (void)focus:(int)on {
  if (on) { [self.metalWorld focus:YES]; [self refreshMicrophoneSetup]; }
  else { [self.metalWorld focus:NO]; [_coderHeldKeys removeAllObjects]; }
}
- (void)life:(int)event {
  switch (event) {
  case UI_LIFE_ACTIVE: [self refreshMicrophoneSetup]; break;
  case UI_LIFE_INACTIVE: if (!self.microphonePrompt) [self cancelVoice:@"focus-lost"]; break;
  case UI_LIFE_HIDDEN: [self cancelVoice:@"app-hidden"]; [self updateWorldVisibility]; break;
  case UI_LIFE_SHOWN: case UI_LIFE_OCCLUSION: case UI_LIFE_RESTORED: [self updateWorldVisibility]; break;
  case UI_LIFE_MINIMIZED: [self cancelVoice:@"window-minimized"]; [self updateWorldVisibility]; break;
  case UI_LIFE_QUIT: [self stopChildren]; break;
  }
}
// The window's keys go to Coder's Enter route first (fgc_enter_route).
- (int)event:(NSEvent *)e {
  if (e.type != NSEventTypeKeyDown && e.type != NSEventTypeKeyUp) return 0;
  BOOL down = e.type == NSEventTypeKeyDown;
  BOOL modified = (e.modifierFlags & (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption)) != 0;
  BOOL ready = self.sceneReady && self.setupHidden && !NSApp.modalWindow && !self.window.attachedSheet;
  NSResponder *first = self.window.firstResponder;
  BOOL realmFocused = [first isKindOfClass:NSView.class] && [(NSView *)first isDescendantOf:self.worldView];
  NSNumber *code = @(e.keyCode);
  int64_t route = fgc_enter_route(ready, self.chat.composerOpen && !realmFocused, e.keyCode, down, down && e.isARepeat, modified, [_coderHeldKeys containsObject:code]);
  if (route != 0) { if (down) [_coderHeldKeys addObject:code]; else [_coderHeldKeys removeObject:code]; }
  if (route == 2) [self focusChat];
  return route == 1 || route == 2;
}
- (void)command:(int)id {
  switch (id) {
  case C_QUIT: [self terminate]; break;
  case C_RELOAD: [self reloadWorld]; break;
  case C_PREVIEW: [self togglePreview]; break;
  case C_SETUP: [self showSetup]; break;
  case C_STATUS: [self showStatus]; break;
  case C_CHAT: [self focusChat]; break;
  case C_FOLDER: if (self.dataURL) [NSWorkspace.sharedWorkspace openURL:self.dataURL]; break;
  }
}
- (int)commandState:(int)id {
  if (id == C_PREVIEW) { ui_menu_title(self.host, C_PREVIEW, [self previewTitle].UTF8String); return self.previewTarget >= 0; }
  if (id == C_RELOAD) return [self reloadAllowed];
  return 1;
}
- (BOOL)reloadAllowed { return !self.stopping && !self.switching; }
- (NSString *)previewTitle {
  NSMutableData *b = [NSMutableData data];
  for (int i = 0; i < 64; i++) { int64_t c = fwp_label_byte(self.metalWorld ? 1 : 0, i); if (c < 0) break; uint8_t u = (uint8_t)c; [b appendBytes:&u length:1]; }
  return [[NSString alloc] initWithData:b encoding:NSUTF8StringEncoding] ?: @"";
}
- (int64_t)previewTarget { return fwp_target(self.worldBackend, self.metalWorld ? 1 : 0, self.sceneReady && !self.stopping, self.switching); }

// The host's layer: the panel's GPU time (prepare) and the chat's HUD (encode).
static void cb_prepare(void *u, void *cmd, uint32_t w, uint32_t h, float dpr) {
  (void)w; (void)h; (void)dpr;
  ForgeApp *app = (__bridge ForgeApp *)u;
  if (app.setupHidden) return;
  app.panelDraws++;   // a frame drawn with the panel (the host draws only changed frames)
  __weak ForgeApp *weak = app;
  [(__bridge id<MTLCommandBuffer>)cmd addCompletedHandler:^(id<MTLCommandBuffer> c) {
    double seconds = MAX(0, c.GPUEndTime - c.GPUStartTime);
    dispatch_async(dispatch_get_main_queue(), ^{ weak.panelGPUSeconds += seconds; });
  }];
}
static void cb_encode(void *u, void *enc, uint32_t w, uint32_t h, float dpr) {
  ForgeApp *app = (__bridge ForgeApp *)u;
  [app.chat encodeHUD:(__bridge id<MTLRenderCommandEncoder>)enc device:(__bridge id<MTLDevice>)ui_device(app.host) width:w height:h scale:dpr];
}
// The chat's HUD moves while it is shown (the feed's lifetime, the composer's caret), and once more as it goes.
static int cb_changed(void *u) {
  ForgeApp *app = (__bridge ForgeApp *)u;
  static int64_t drawn = 0;
  int64_t now = app.chat.presentation;
  int moved = now != 0 || now != drawn;
  drawn = now;
  return moved;
}
// A picture of the panel, from an offscreen host with the same callbacks (the setup profile's).
- (void)capturePanel:(NSURL *)png {
  __block BOOL ok = NO;
  [self main:^{
    NSView *view = (__bridge NSView *)ui_view(self.host);
    float d = ui_dpr(self.host);
    ok = [self shotOf:png width:(uint32_t)lround(view.bounds.size.width * d) height:(uint32_t)lround(view.bounds.size.height * d) dpr:d];
  }];
  FORGE_REQUIRE(ok, @"Panel readback failed.");
}
- (BOOL)shotOf:(NSURL *)png width:(uint32_t)w height:(uint32_t)h dpr:(float)dpr {
  if (!_shot) {
    UiApp app = {0};
    app.app_id = "com.openagents.aethyrforge"; app.max_vertices = FORGE_MAX_VERTICES;
    NSString *shader = [self.resources URLByAppendingPathComponent:@"Component.metal"].path;
    app.shader = shader.fileSystemRepresentation; app.user = (__bridge void *)self; app.frame = cb_frame; app.drive = cb_drive;
    _shot = ui_open(&app, 1);
    if (!_shot) return NO;
    UiLayer layer = {(__bridge void *)self, 0, NULL, cb_encode};
    ui_layer(_shot, &layer);
    if (_ct) ui_atlas(_shot, _full, ct_atlas_width(_ct), ct_atlas_height(_ct), _color, ct_atlas_width(_ct), ct_atlas_color_height(_ct));
  }
  UiHost *was = self.host;
  if (!was) self.host = _shot;   // --shot: the offscreen host is the only one
  ui_set_dpr(_shot, dpr);
  [self atlasAt:dpr];
  ui_atlas(_shot, _full, ct_atlas_width(_ct), ct_atlas_height(_ct), _color, ct_atlas_width(_ct), ct_atlas_color_height(_ct));
  BOOL ok = ui_shot_frame(_shot, w, h, 0) && ui_shot_frame(_shot, w, h, 0) && ui_shot_write(_shot, png.fileSystemRepresentation);
  self.host = was;
  return ok;
}

// ---- startup

- (void)startChild:(NSString *)name arguments:(NSArray *)arguments fatal:(BOOL)fatal into:(NSTask *__strong *)slot {
  NSFileManager *fm = NSFileManager.defaultManager;
  NSTask *p = [NSTask new];
  p.executableURL = [self.resources URLByAppendingPathComponent:name];
  p.arguments = arguments;
  NSMutableDictionary *env = [NSProcessInfo.processInfo.environment mutableCopy];
  for (NSString *key in env.allKeys) if ([key isEqualToString:@"PORT"] || [key hasPrefix:@"OA_ROOM_"]) [env removeObjectForKey:key];
  env[@"COAXIAL_MODE"] = @"dev";
  p.environment = env;
  NSURL *logURL = [self.dataURL URLByAppendingPathComponent:[name stringByAppendingString:@".log"]];
  if ([name isEqualToString:@"voice-host"] && [fm fileExistsAtPath:logURL.path]) {
    NSURL *previous = [self.dataURL URLByAppendingPathComponent:@"voice-host.previous.log"];
    [fm removeItemAtURL:previous error:nil]; [fm moveItemAtURL:logURL toURL:previous error:nil];
  }
  [fm createFileAtPath:logURL.path contents:nil attributes:nil];
  NSFileHandle *log = [NSFileHandle fileHandleForWritingToURL:logURL error:nil];
  if (!log) forge_throw([NSString stringWithFormat:@"Cannot write %@.", logURL.path]);
  [self main:^{ [self->_logs addObject:log]; }];
  p.standardOutput = log; p.standardError = log;
  __weak ForgeApp *weak = self;
  p.terminationHandler = ^(NSTask *child) {
    dispatch_async(dispatch_get_main_queue(), ^{
      ForgeApp *s = weak;
      if (!s || s.stopping || (child != s.page && child != s.room && child != s.voiceHost)) return;
      if (fatal) [s fail:[NSString stringWithFormat:@"%@ stopped (%d). See %@.", name, child.terminationStatus, logURL.path]];
      else {
        s->_voiceWarmReady = NO; fgs_voice_ready(3); [s updateSetup];
        s.notice.stringValue = [NSString stringWithFormat:@"Local voice stopped. See %@.", logURL.path]; s.notice.hidden = NO;
      }
    });
  };
  NSError *error = nil;
  if (![p launchAndReturnError:&error]) forge_throw(error.localizedDescription);
  [self main:^{ *slot = p; }];
}
- (void)waitForHTTP:(NSURL *)url {
  double end = forge_uptime() + 30;
  while (forge_uptime() < end && !self.stopping) {
    NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:url]; r.timeoutInterval = 1;
    __block NSInteger status = 0;
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    [[NSURLSession.sharedSession dataTaskWithRequest:r completionHandler:^(NSData *d, NSURLResponse *resp, NSError *e) { (void)d; (void)e; status = [(NSHTTPURLResponse *)resp statusCode]; dispatch_semaphore_signal(done); }] resume];
    dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
    if (status == 200) return;
    forge_sleep_ms(100);
  }
  forge_throw(@"Local server did not start. Check room.log and serve.log in the local world folder.");
}
- (void)addPageScript:(NSString *)source {
  [self main:^{ [self.web.configuration.userContentController addUserScript:[[WKUserScript alloc] initWithSource:source injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]]; }];
}
// Off the main thread, as the Swift host's start() was.
- (void)start {
  @try {
    NSFileManager *fm = NSFileManager.defaultManager;
    NSString *dir = [self arg:@"--data-dir"];
    NSURL *isolated = self.smoke ? self.report : (self.microphoneReport ?: self.setupProfileURL ?: self.offlineReportURL ?: self.benchmarkReportURL);
    if (dir) self.dataURL = [NSURL fileURLWithPath:dir isDirectory:YES];
    else if (isolated) self.dataURL = [isolated.URLByDeletingLastPathComponent URLByAppendingPathComponent:@"world"];
    else {
      NSURL *base = [fm.homeDirectoryForCurrentUser URLByAppendingPathComponent:@".openagents/aethyrforge" isDirectory:YES];
      [fm createDirectoryAtURL:base withIntermediateDirectories:YES attributes:nil error:nil];
      self.dataURL = [base URLByAppendingPathComponent:@"everglade" isDirectory:YES];
      NSURL *legacy = [fm.homeDirectoryForCurrentUser URLByAppendingPathComponent:@"Library/Application Support/Aethyrforge/everglade" isDirectory:YES];
      if (![fm fileExistsAtPath:self.dataURL.path] && [fm fileExistsAtPath:legacy.path]) {
        NSError *e = nil;
        if (![fm moveItemAtURL:legacy toURL:self.dataURL error:&e]) forge_throw(e.localizedDescription);
        if ([fm contentsOfDirectoryAtPath:legacy.URLByDeletingLastPathComponent.path error:nil].count == 0) [fm removeItemAtURL:legacy.URLByDeletingLastPathComponent error:nil];
      }
    }
    NSError *e = nil;
    if (![fm createDirectoryAtURL:self.dataURL withIntermediateDirectories:YES attributes:nil error:&e]) forge_throw(e.localizedDescription);
    _lockFD = open([self.dataURL URLByAppendingPathComponent:@".lock"].path.fileSystemRepresentation, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR);
    if (_lockFD < 0 || flock(_lockFD, LOCK_EX | LOCK_NB) != 0) forge_throw(@"This local world is already open in Aethyrforge.");
    for (NSString *name in @[ @"world-everglade.json", @"room-control.txt" ]) {
      NSURL *destination = [self.dataURL URLByAppendingPathComponent:name];
      if (![fm fileExistsAtPath:destination.path] && ![fm copyItemAtURL:[self.resources URLByAppendingPathComponent:[@"seed/" stringByAppendingString:name]] toURL:destination error:&e]) forge_throw(e.localizedDescription);
    }
    int roomPort, pagePort, voicePort;
    int r = reserve_port(&roomPort), p = -1;
    @try { p = reserve_port(&pagePort); _voiceReservation = reserve_port(&voicePort); _voicePort = voicePort; }
    @catch (NSException *x) { close(r); if (p >= 0) close(p); @throw; }
    [self addPageScript:[NSString stringWithFormat:@"globalThis.coaxialVoiceHost='ws://127.0.0.1:%d';", voicePort]];
    if (!self.offlineWorker) [self addPageScript:[NSString stringWithFormat:@"globalThis.coaxialNativeRoomHost='ws://127.0.0.1:%d';", roomPort]];
    NSMutableData *bytes = [NSMutableData data];
    for (int i = 0; i < 256; i++) {
      int64_t c = self.benchmarkReportURL ? forge_benchmark_url_byte(pagePort, i) : self.offlineMode ? forge_offline_url_byte(pagePort, i) : self.nativeWorld ? forge_native_url_byte(pagePort, i) : forge_url_byte(pagePort, roomPort, i);
      if (c < 0) break;
      uint8_t u = (uint8_t)c; [bytes appendBytes:&u length:1];
    }
    close(r); close(p);
    NSURL *url = bytes.length ? [NSURL URLWithString:[[NSString alloc] initWithData:bytes encoding:NSUTF8StringEncoding]] : nil;
    if (!url) forge_throw(@"Invalid local ports.");
    self.pageURL = url;
    NSTask *__strong room = nil, *__strong page = nil;
    [self startChild:@"room" arguments:@[ @"--listen", @"127.0.0.1", @"--port", @(roomPort).stringValue, @"--dev", self.dataURL.path, @"--native-local" ] fatal:YES into:&room];
    [self main:^{ self.room = room; }];
    [self startChild:@"serve" arguments:@[ [self.resources URLByAppendingPathComponent:@"web"].path, @"--host-dir", [self.resources URLByAppendingPathComponent:@"host"].path, @"--static", @"--cache", @"0", @"--listen", [NSString stringWithFormat:@"127.0.0.1:%d", pagePort], @"--api", @(roomPort).stringValue ] fatal:YES into:&page];
    [self main:^{ self.page = page; }];
    [self waitForHTTP:[NSURL URLWithString:[NSString stringWithFormat:@"http://127.0.0.1:%d/health", roomPort]]];
    [self waitForHTTP:[NSURL URLWithString:[NSString stringWithFormat:@"http://127.0.0.1:%d/worlds/everglade", pagePort]]];
    forge_write([url.absoluteString stringByAppendingString:@"\n"], [self.dataURL URLByAppendingPathComponent:@"local-url.txt"]);
    [self main:^{
      self.notice.stringValue = @"Loading Everglade…";
      [self refreshSetup:NO];
      if (self.metalWorld) {
        self->_metalLocalOrigin = [NSString stringWithFormat:@"ws://127.0.0.1:%d", roomPort];
        NSError *oe = nil;
        if (![self.metalWorld openResources:self.resources storage:self.dataURL page:url local:self->_metalLocalOrigin muted:self.mute error:&oe]) forge_throw(oe.localizedDescription);
        [self.web loadRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:[NSString stringWithFormat:@"http://127.0.0.1:%d/metal-voice.html", pagePort]]]];
      } else [self.web loadRequest:[NSURLRequest requestWithURL:url]];
    }];
    if (self.smoke && !self.nativeWorld) {
      forge_async(^{
        while (!self.stopping) {
          dispatch_semaphore_t done = dispatch_semaphore_create(0);
          dispatch_async(dispatch_get_main_queue(), ^{
            [self.web evaluateJavaScript:@"if(globalThis.__forgeFrame){const f=globalThis.__forgeFrame;globalThis.__forgeFrame=null;f(performance.now());} true" completionHandler:^(id x, NSError *y) { (void)x; (void)y; dispatch_semaphore_signal(done); }];
          });
          dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
          forge_sleep_ms(16);
        }
      });
    }
    [self waitForScene];
    self.firstFrameSeconds = forge_uptime() - _startedAt;
    [self main:^{
      self.notice.hidden = YES;
      self.sceneReady = YES; [self updateSetup];
      [self updateWorldVisibility];
      [self warmVoiceIfNeeded];
      if (self.setupHidden) [self.window makeFirstResponder:self.worldView];
    }];
    [self startSceneControls];
    NSString *plan = [self arg:@"--web-preview-plan"];
    if (plan) { [self measureWebPreview:[NSURL fileURLWithPath:plan]]; return; }
    plan = [self arg:@"--metal-measure-plan"];
    if (plan) { [self measureMetal:[NSURL fileURLWithPath:plan]]; return; }
    NSString *observe = [self arg:@"--metal-observe"];
    if (observe) forge_async(^{ [self observeMetal:[NSURL fileURLWithPath:observe]]; });
    [self runCheck];
  } @catch (NSException *x) {
    if (!self.stopping) { NSString *m = x.reason; [self main:^{ [self fail:m]; }]; }
  }
}
- (void)observeMetal:(NSURL *)path {
  while (!self.stopping) {
    __block NSDictionary *state = nil;
    [self main:^{
      ForgeMetalWorld *w = self.metalWorld; void *h = w.host;
      if (!h) return;
      state = @{@"frames" : @(mf_info(h, 0)), @"completed" : @(mf_info(h, 10)), @"pending" : @(mf_info(h, 2)), @"draws" : @(mf_info(h, 1)), @"ledgerLive" : @(mf_info(h, 3)), @"ledgerWords" : @(mf_info(h, 4)), @"gpuNS" : @(mf_info(h, 11)),
                @"net" : @(mf_info(h, 7)), @"seat" : @(mf_info(h, 8)), @"paused" : @(w.paused), @"setup" : @(!self.setupHidden), @"width" : @(self.worldView.bounds.size.width), @"height" : @(self.worldView.bounds.size.height),
                @"scale" : @(self.window.backingScaleFactor), @"x" : @(mf_probe(h, 0)), @"z" : @(mf_probe(h, 2)), @"messages" : @(w.delivered), @"miniaturized" : @(self.window.miniaturized), @"visible" : @(self.window.visible),
                @"key" : @(self.window.keyWindow), @"captured" : @(w.captured), @"audio" : [w audioObservations]};
    }];
    if (state) [[forge_json(state) dataUsingEncoding:NSUTF8StringEncoding] writeToURL:path atomically:YES];
    forge_sleep_ms(250);
  }
}
// The scene tool's session: the player's place and the local API, for the prebuilt Coax scene tool.
- (void)startSceneControls {
  NSURL *base = self.dataURL.URLByDeletingLastPathComponent;
  NSURL *session = [base URLByAppendingPathComponent:@"scene-session.json"], *instructions = [base URLByAppendingPathComponent:@"scene-coder.txt"];
  uint8_t b[8192]; int64_t n = fgc_scene_instructions(b, sizeof b);
  if (n <= 0) forge_throw(@"Scene tool context is unavailable.");
  NSError *e = nil;
  if (![[NSData dataWithBytes:b length:(NSUInteger)n] writeToURL:instructions options:NSDataWritingAtomic error:&e]) forge_throw(e.localizedDescription);
  [NSFileManager.defaultManager setAttributes:@{NSFilePosixPermissions : @0600} ofItemAtPath:instructions.path error:nil];
  [self main:^{ self.chat.sceneSessionURL = session; self.chat.sceneContextURL = instructions; }];
  [self updateSceneSession];
  forge_async(^{
    while (!self.stopping) { forge_sleep_ms(500); if (!self.stopping) @try { [self updateSceneSession]; } @catch (NSException *x) { (void)x; } }
  });
}
- (void)updateSceneSession {
  NSURL *session = self.chat.sceneSessionURL;
  if (self.stopping || !self.sceneReady || !session || !self.pageURL) return;
  __block NSString *value = nil;
  [self main:^{
    void *h = self.metalWorld.host;
    if (h) value = [NSString stringWithFormat:@"{\"x_cm\":%ld,\"z_cm\":%ld}", lround(mf_probe(h, 0) / 10.0), lround(mf_probe(h, 2) / 10.0)];
  }];
  if (!value) value = [self js:@"JSON.stringify({x_cm:Math.round(coaxial.fieldProbe(0)/10),z_cm:Math.round(coaxial.fieldProbe(2)/10)})"] ?: @"{}";
  NSDictionary *at = forge_parse(value);
  NSNumber *x = [at isKindOfClass:NSDictionary.class] ? at[@"x_cm"] : nil, *z = [at isKindOfClass:NSDictionary.class] ? at[@"z_cm"] : nil;
  struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
  // The file's bytes are scene-session.coax's (fs_session_put); a position the page
  // didn't give is left outside the reader's bounds, as before, so the CLI waits for a fresh one.
  uint8_t b[512];
  int64_t n = fgc_scene_session(self.pageURL.port.integerValue, [x isKindOfClass:NSNumber.class] ? x.longLongValue : -16001,
                                [z isKindOfClass:NSNumber.class] ? z.longLongValue : -16001, getpid(),
                                (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000, b, sizeof b);
  if (self.stopping || n <= 0) return;
  forge_write([[NSString alloc] initWithBytes:b length:(NSUInteger)n encoding:NSUTF8StringEncoding], session);
  [NSFileManager.defaultManager setAttributes:@{NSFilePosixPermissions : @0600} ofItemAtPath:session.path error:nil];
}
- (NSString *)launchState {
  NSString *raw = [self js:@"JSON.stringify({ready:globalThis.coaxial?.ready,scene:globalThis.coaxial?.sceneReady,frames:globalThis.__forgeFrameCount,hidden:document.hidden,focused:document.hasFocus(),canvas:!!document.querySelector('canvas'),probe:globalThis.coaxial?.probe?.()})"];
  NSMutableDictionary *state = [forge_parse(raw) mutableCopy] ?: [NSMutableDictionary dictionary];
  [self main:^{
    state[@"nativeReady"] = @(self.sceneReady); state[@"enterEnabled"] = @(self.enterEnabled);
    state[@"waitPhase"] = @(self.startupPhase); state[@"longestHiddenSeconds"] = @(self.longestHiddenSeconds);
    state[@"retryCheckPassed"] = @(self.retryCheckPassed); state[@"setupVisible"] = @(!self.setupHidden); state[@"failed"] = @(self.stopping);
    state[@"firstFrameSeconds"] = @(self.firstFrameSeconds);
    state[@"hudHeight"] = @(self.chat.hudShown ? NSInsetRect([self.chat box:0], -4, -4).size.height : 0); state[@"windowHeight"] = @(self.web.frame.size.height);
  }];
  return forge_json(state);
}
- (void)waitForScene {
  if (self.metalWorld) {
    double end = forge_uptime() + 60;
    while (!self.stopping && forge_uptime() < end) {
      __block BOOL done = NO; __block NSString *error = nil;
      [self main:^{ done = self.metalWorld.completed; const char *m = self.metalWorld.host ? mf_error(self.metalWorld.host) : NULL; if (m) error = @(m); }];
      if (done) return;
      if (error) forge_throw(error);
      forge_sleep_ms(20);
    }
    __block NSString *seen = @"";
    [self main:^{
      ForgeMetalWorld *w = self.metalWorld;
      seen = [NSString stringWithFormat:@" (paused %d, frames %lld, ticks %ld, without a surface %ld, in a window %d, on a screen %d, %.0f by %.0f)", w.paused, w.host ? (long long)mf_info(w.host, 0) : -1LL, (long)w.ticks, (long)w.surfaceless,
              w.window != nil, w.window.screen != nil, w.bounds.size.width, w.bounds.size.height];
    }];
    forge_throw([@"Metal did not complete its first real frame." stringByAppendingString:seen]);
  }
  // During a retry WKWebView still evaluates in the previous document until navigation commits.
  NSString *current = [NSString stringWithFormat:@"location.origin === 'http://127.0.0.1:%@'", self.pageURL.port ?: @0];
  int64_t budget = 60000;
  double previous = forge_uptime(), hiddenSince = -1;
  BOOL wasVisible = NO;
  while (!self.stopping) {
    if (self.launchReport) @try { forge_write([self launchState], self.launchReport); } @catch (NSException *x) { (void)x; }
    id ready = nil; @try { ready = [self js:[NSString stringWithFormat:@"%@ && globalThis.coaxial?.sceneReady === true", current]]; } @catch (NSException *x) { (void)x; }
    if ([ready isEqual:@YES]) return;
    NSString *message = nil;
    @try { message = [self js:[NSString stringWithFormat:@"(%@ && document.querySelector('#coaxial-load[data-state=fail] .le')?.textContent) || ''", current]]; } @catch (NSException *x) { (void)x; }
    if ([message isKindOfClass:NSString.class] && message.length) forge_throw(message);
    BOOL pageVisible = NO; @try { pageVisible = [[self js:@"!document.hidden"] isEqual:@YES]; } @catch (NSException *x) { (void)x; }
    __block BOOL visible = self.smoke;
    if (!visible) [self main:^{ visible = self.window.visible && !self.window.miniaturized && (self.window.occlusionState & NSWindowOcclusionStateVisible) && pageVisible; }];
    double now = forge_uptime();
    if (visible) hiddenSince = -1; else { if (hiddenSince < 0) hiddenSince = now; self.longestHiddenSeconds = MAX(self.longestHiddenSeconds, now - hiddenSince); }
    int64_t elapsed = (int64_t)(MAX(0, now - previous) * 1000);
    self.startupPhase = forge_startup_phase(budget, elapsed, visible && wasVisible);
    budget = forge_startup_budget(budget, elapsed, visible && wasVisible);
    previous = now; wasVisible = visible;
    if (self.startupPhase == 2) break;
    forge_sleep_ms(200);
  }
  if (self.smoke) @try { printf("Hidden view: %s\n", [[self js:@"JSON.stringify({ready:globalThis.coaxial?.ready,scene:globalThis.coaxial?.sceneReady,gpu:!!navigator.gpu,body:document.body.innerText.slice(0,1200)})"] description].UTF8String); } @catch (NSException *x) { (void)x; }
  forge_throw(@"Everglade did not finish its first visible scene frame. Press Retry startup.");
}

- (void)fail:(NSString *)message {
  [self stopChildren];
  self.window.level = NSNormalWindowLevel;
  self.notice.hidden = NO; self.notice.stringValue = message;
  self.setupError = message; self.checking = NO; self.verifying = NO;
  self.setupHidden = NO; [self updateSetup];
  NSURL *report = self.setupProfileURL ?: self.offlineReportURL ?: self.benchmarkReportURL;
  NSString *what = self.setupProfileURL ? @"setup profile" : self.offlineReportURL ? @"offline check" : @"benchmark";
  if (report) {
    [[message dataUsingEncoding:NSUTF8StringEncoding] writeToURL:[report URLByAppendingPathExtension:@"error"] atomically:YES];
    fprintf(stderr, "Aethyrforge %s: %s\n", what.UTF8String, message.UTF8String); exit(1);
  }
  if (self.smoke) {
    if (self.report) [[[@"FAILED: " stringByAppendingString:message] dataUsingEncoding:NSUTF8StringEncoding] writeToURL:self.report atomically:YES];
    fprintf(stderr, "Aethyrforge: %s\n", message.UTF8String); exit(1);
  }
}
- (void)restartStartup {
  self.stopping = NO; self.sceneReady = NO; _voiceStarting = NO;
  self.room = nil; self.page = nil; self.voiceHost = nil;
  self.chat.stopping = NO;
  if (self.launchReport) self.window.level = NSFloatingWindowLevel;
  self.setupError = nil; [self updateSetup];
  forge_async(^{ [self start]; });
}
- (void)acceptSetup:(NSData *)data kind:(int64_t)kind {
  if (data.length > 65536) forge_throw(@"The setup report is too large.");
  const uint8_t *b = data.bytes;
  for (NSUInteger i = 0; i < data.length; i++) fgs_input_byte((int64_t)i, b[i]);
  if (fgs_accept(kind, (int64_t)data.length) != 1) forge_throw(@"The local CLI returned an invalid setup report.");
}
- (void)refreshSetup:(BOOL)verify {
  [self refreshMicrophoneSetup];
  if (self.verifying || (self.inventory && self.checking)) return;
  self.setupError = nil; self.checking = YES; self.verifying = NO; [self updateSetup];
  dispatch_group_t group = dispatch_group_create();
  self.inventory = group;
  NSURL *cli = self.localCLI;
  dispatch_group_async(group, dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    NSString *error = nil;
    @try {
      __block NSData *version = nil, *agents = nil, *models = nil; __block NSString *problem = nil;
      dispatch_group_t three = dispatch_group_create();
      NSArray *calls = @[ @[ @"version", @"--json" ], @[ @"coder", @"agents", @"--json" ], @[ @"models", @"ls", @"--json" ] ];
      for (int i = 0; i < 3; i++) dispatch_group_async(three, dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        @try { NSData *d = forge_cli(cli, calls[i], 15, nil); if (i == 0) version = d; else if (i == 1) agents = d; else models = d; }
        @catch (NSException *x) { @synchronized(calls) { if (!problem) problem = x.reason; } }
      });
      dispatch_group_wait(three, DISPATCH_TIME_FOREVER);
      if (problem) forge_throw(problem);
      __block NSString *bad = nil;
      [self main:^{
        @try {
          [self acceptSetup:version kind:0]; [self acceptSetup:agents kind:1]; [self acceptSetup:models kind:2];
          NSData *kept = [NSData dataWithContentsOfURL:self.settingsURL];
          if (kept) @try { [self acceptSetup:kept kind:3]; } @catch (NSException *x) { (void)x; }
          self.checking = NO; [self updateSetup];
          if (verify && fgs_status(1) > 0) { self.verifying = YES; [self updateSetup]; }
        } @catch (NSException *x) { bad = x.reason; }
      }];
      if (bad) forge_throw(bad);
      if (verify && self.verifying) {
        NSData *verified = forge_cli(cli, @[ @"models", @"verify", @"parakeet", @"--json" ], 120, nil);
        [self main:^{ @try { [self acceptSetup:verified kind:2]; } @catch (NSException *x) { self.setupError = x.reason; } }];
      }
    } @catch (NSException *x) { error = x.reason; }
    [self main:^{
      if (error) self.setupError = error;
      self.checking = NO; self.verifying = NO; [self updateSetup];
      [self warmVoiceIfNeeded];
    }];
  });
}
- (void)waitInventory { dispatch_group_t g = self.inventory; if (g) dispatch_group_wait(g, DISPATCH_TIME_FOREVER); }
- (void)enterWorld {
  if (!fgs_can_enter(self.sceneReady)) return;
  NSMutableData *bytes = [NSMutableData data];
  for (int i = 0; i < 512; i++) { int64_t c = fgs_settings_byte(i); if (c < 0) break; uint8_t u = (uint8_t)c; [bytes appendData:[NSData dataWithBytes:&u length:1]]; }
  @try {
    NSError *e = nil;
    if (![bytes writeToURL:self.settingsURL options:NSDataWritingAtomic error:&e]) forge_throw(e.localizedDescription);
    [self acceptSetup:bytes kind:3];
    self.setupHidden = YES; [self.window makeFirstResponder:self.worldView];
    self.statusMode = NO; [self updateWorldVisibility]; [self updateSetup];
  } @catch (NSException *x) { self.setupError = [@"Could not save setup: " stringByAppendingString:x.reason]; [self updateSetup]; }
}
- (void)showSetup {
  [self cancelVoice:@"cancel"]; [self.chat closeComposer];
  self.statusMode = NO; self.setupHidden = NO; [self updateWorldVisibility]; [self updateSetup];
  [self.window makeFirstResponder:(__bridge NSView *)ui_view(self.host)];
  [self refreshMicrophoneSetup];
}
- (void)showStatus { [self showSetup]; self.statusMode = YES; [self updateSetup]; }
- (void)updateWorldVisibility {
  if (!self.web || !self.window) return;
  BOOL smoke = self.smoke;
  BOOL paused = forge_world_paused(self.sceneReady, !self.setupHidden, smoke || (self.window.visible && !NSApp.hidden), !smoke && self.window.miniaturized, !smoke && !(self.window.occlusionState & NSWindowOcclusionStateVisible));
  self.worldPaused = paused;
  if (_worldPauseWanted && _worldPauseWanted.boolValue == paused) return;
  _worldPauseWanted = @(paused); [self updateSetup];
  if (self.metalWorld) [self.metalWorld pauseWorld:paused];
  else [self.web evaluateJavaScript:[NSString stringWithFormat:@"globalThis.coaxial?.setWorldPaused(%@); true", paused ? @"true" : @"false"] completionHandler:nil];
}
- (void)focusChat { if (self.setupHidden) [self.chat openComposer]; }

// ---- voice

- (void)warmVoiceIfNeeded {
  if (!self.window) return;
  if (!forge_voice_warm(!self.stopping && self.pageURL, self.window.visible && !self.window.miniaturized, self.microphoneStatus == AVAuthorizationStatusAuthorized, fgs_status(1) > 0, self.interactive, _voiceWarming, _voiceWarmReady)) return;
  forge_async(^{ @try { [self prepareVoice]; } @catch (NSException *x) { (void)x; } });
}
- (void)prepareVoice {
  __block BOOL pending = NO; __block int epoch = 0;
  [self main:^{ pending = self->_voiceWarming; if (!pending) { self->_voiceWarming = YES; } epoch = self->_voiceWarmEpoch; }];
  if (pending) {
    while (true) { __block BOOL still = NO; [self main:^{ still = self->_voiceWarming; }]; if (!still) break; forge_sleep_ms(20); }
    __block BOOL ready = NO; [self main:^{ ready = self->_voiceWarmReady; }];
    if (!ready) forge_throw(@"Voice did not warm.");
    return;
  }
  @try {
    [self main:^{ fgs_voice_ready(1); [self updateSetup]; }];
    [self ensureVoiceHost];
    if (self.stopping || epoch != _voiceWarmEpoch) forge_throw(@"cancelled");
    id ready = [self callJS:@"const end=performance.now()+8000;\n"
                            @"while(!globalThis.coaxial?.voice && performance.now()<end)await new Promise(r=>setTimeout(r,20));\n"
                            @"if(!globalThis.coaxial?.voice)throw new Error('Voice page is not ready');\n"
                            @"return await coaxial.voice.warm();"
                  arguments:nil];
    if (![ready isEqual:@YES] || self.stopping || epoch != _voiceWarmEpoch) forge_throw(@"cancelled");
    [self main:^{ self->_voiceWarmReady = YES; fgs_voice_ready(2); [self updateSetup]; if (epoch == self->_voiceWarmEpoch) self->_voiceWarming = NO; }];
  } @catch (NSException *x) {
    [self main:^{
      if (epoch == self->_voiceWarmEpoch) {
        self->_voiceWarming = NO;
        if (!self.stopping) { self->_voiceWarmReady = NO; fgs_voice_ready(3); [self updateSetup]; [self traceVoice:[@"warm:error:" stringByAppendingString:x.reason]]; }
      }
    }];
    @throw;
  }
}
- (void)ensureVoiceHost {
  for (;;) {
    __block BOOL starting = NO, running = NO;
    [self main:^{ starting = self->_voiceStarting; running = self.voiceHost.running; if (!starting && !running) self->_voiceStarting = YES; }];
    if (running) return;
    if (!starting) break;
    forge_sleep_ms(20);   // another caller is starting it
  }
  @try {
    NSString *model = forge_setup_text(6);
    if (!self.resources || !model.length) forge_throw(@"The shared Parakeet model is unavailable.");
    [self main:^{ if (self->_voiceReservation >= 0) { close(self->_voiceReservation); self->_voiceReservation = -1; } }];
    NSTask *__strong voice = nil;
    [self startChild:@"voice-host" arguments:@[ @"--listen", [NSString stringWithFormat:@"127.0.0.1:%d", _voicePort], @"--provider", @"local", @"--recognizer", @"parakeet", @"--synthesizer", @"local", @"--agent", @"echo", @"--parakeet-model", model, @"--parakeet-device", @"gpu" ] fatal:NO into:&voice];
    [self main:^{ self.voiceHost = voice; }];
    [self waitForHTTP:[NSURL URLWithString:[NSString stringWithFormat:@"http://127.0.0.1:%d/health", _voicePort]]];
  } @finally {
    [self main:^{ self->_voiceStarting = NO; }];
  }
}
- (void)cancelVoice:(NSString *)reason {
  [self traceVoice:[@"capture:cancel reason=" stringByAppendingString:reason]];
  _voiceCaptureEpoch++; _voiceCaptureRequest = 0;
  fgv_cancel(); [self.web evaluateJavaScript:@"globalThis.coaxialVoiceCancel?.(); true" completionHandler:nil];
}
- (void)armVoiceCapture {
  int64_t ticket = fgv_arm();
  if (ticket <= 0) forge_throw(@"Voice capture generation is exhausted; reopen the app.");
  [self js:[NSString stringWithFormat:@"globalThis.coaxialCoderVoiceTicket=%lld; true", ticket]];
}
- (AVAuthorizationStatus)microphoneStatus { return self.microphoneAuthorization ? self.microphoneAuthorization() : [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]; }
- (void)updateMicrophoneSetup {
  fgs_microphone(self.microphoneStatus, self.microphonePrompt);
  [self updateSetup];
  [self warmVoiceIfNeeded];
}
- (void)refreshMicrophoneSetup {
  if (!self.window) return;
  [self updateMicrophoneSetup];
  if (!fm_setup_request(!self.stopping && self.window.visible && !self.window.miniaturized, NSApp.active, !self.setupHidden, fgs_completed(), self.microphoneStatus, self.microphonePrompt, self.interactive)) return;
  forge_async(^{ [self authorizeMicrophone]; });
}
// Off the main thread: one system request at a time; a second caller waits for the first's answer.
- (BOOL)authorizeMicrophone {
  __block BOOL asking = NO, mine = NO; __block AVAuthorizationStatus status = AVAuthorizationStatusNotDetermined;
  [self main:^{ asking = self->_microphoneAsking; status = self.microphoneStatus; if (!asking && status == AVAuthorizationStatusNotDetermined) { self->_microphoneAsking = mine = YES; self.microphonePrompt = YES; [self updateMicrophoneSetup]; } }];
  if (asking) {
    for (;;) { __block BOOL still = NO; [self main:^{ still = self->_microphoneAsking; }]; if (!still) break; forge_sleep_ms(10); }
    __block BOOL ok = NO; [self main:^{ ok = self.microphoneStatus == AVAuthorizationStatusAuthorized; }];
    return ok;
  }
  if (!mine) { __block BOOL ok = NO; [self main:^{ [self updateMicrophoneSetup]; ok = self.microphoneStatus == AVAuthorizationStatusAuthorized; }]; return ok; }
  BOOL granted;
  if (self.microphoneAccessRequest) granted = self.microphoneAccessRequest();
  else {
    __block BOOL g = NO; dispatch_semaphore_t done = dispatch_semaphore_create(0);
    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL ok) { g = ok; dispatch_semaphore_signal(done); }];
    dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER); granted = g;
  }
  __block BOOL ok = NO;
  [self main:^{ self->_microphoneAsking = NO; self.microphonePrompt = NO; [self updateMicrophoneSetup]; ok = granted && self.microphoneStatus == AVAuthorizationStatusAuthorized; }];
  return ok;
}
- (void)microphoneSetupPressed {
  if (fgs_status(3) == 1) [self refreshMicrophoneSetup];
  else if (fgs_status(3) == 2) [NSWorkspace.sharedWorkspace openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"]];
}
- (BOOL)voiceAvailable { return fm_capture(!self.stopping && self.sceneReady && self.setupHidden, self.window.visible && !self.window.miniaturized, NSApp.active); }
- (BOOL)trustedVoiceFrame:(WKFrameInfo *)frame {
  WKSecurityOrigin *o = frame.securityOrigin;
  return frame.mainFrame && [o.protocol isEqualToString:@"http"] && [o.host isEqualToString:@"127.0.0.1"] && o.port == self.pageURL.port.integerValue;
}
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)message replyHandler:(void (^)(id, NSString *))reply {
  (void)controller;
  NSDictionary *body = [message.body isKindOfClass:NSDictionary.class] ? message.body : nil;
  NSNumber *captureNumber = [body[@"capture"] isKindOfClass:NSNumber.class] ? body[@"capture"] : nil;
  int64_t capture = captureNumber.longLongValue;
  if (![message.name isEqualToString:@"forgeVoiceCapture"] || ![self trustedVoiceFrame:message.frameInfo] || ![body[@"schema"] isEqual:@"aethyrforge.microphone-capture.v1"] || !captureNumber || capture <= 0 || capture > 9007199254740991LL) { reply(nil, @"Invalid local microphone request."); return; }
  if ([body[@"kind"] isEqual:@"cancel"]) {
    if (_voiceCaptureRequest == capture) { _voiceCaptureEpoch++; _voiceCaptureRequest = 0; fgv_cancel(); }
    reply(@YES, nil); return;
  }
  BOOL fixture = [self has:@"--voice-smoke"] || [self has:@"--voice-warm-smoke"];
  if (![body[@"kind"] isEqual:@"prepare"] || (self.smoke && !fixture) || !(fixture || self.voiceAvailable) || !(fixture || [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio] == AVAuthorizationStatusAuthorized)) {
    reply(nil, @"Voice stopped. Return to Aethyrforge and hold TALK again."); return;
  }
  _voiceCaptureEpoch++; _voiceCaptureRequest = capture;
  int epoch = _voiceCaptureEpoch;
  forge_async(^{
    @try {
      if (self.microphoneReport && self.microphoneCheckFailStartup) { self.microphoneCheckFailStartup = NO; forge_throw(@"Local voice test startup failure."); }
      if (self.microphoneReport && self.microphoneCheckDelay) forge_sleep_ms(500);
      [self ensureVoiceHost];
      __block BOOL live = NO; [self main:^{ live = epoch == self->_voiceCaptureEpoch && (fixture || self.voiceAvailable); }];
      if (!live) forge_throw(@"Voice stopped. Hold TALK again.");
      [self armVoiceCapture];
      [self main:^{ live = epoch == self->_voiceCaptureEpoch && (fixture || self.voiceAvailable); }];
      if (!live) forge_throw(@"Voice stopped. Hold TALK again.");
      dispatch_async(dispatch_get_main_queue(), ^{ reply(@YES, nil); });
    } @catch (NSException *x) { NSString *m = x.reason; dispatch_async(dispatch_get_main_queue(), ^{ reply(nil, m); }); }
  });
}
- (void)traceVoice:(NSString *)line {
  if (!self.dataURL || [line lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 1024) return;
  NSString *path = [self.dataURL URLByAppendingPathComponent:@"voice-access.log"].path;
  if (![NSFileManager.defaultManager fileExistsAtPath:path]) [NSFileManager.defaultManager createFileAtPath:path contents:nil attributes:nil];
  NSFileHandle *f = [NSFileHandle fileHandleForWritingAtPath:path];
  NSISO8601DateFormatter *iso = [NSISO8601DateFormatter new];
  [f seekToEndReturningOffset:nil error:nil];
  [f writeData:[[NSString stringWithFormat:@"%@ %@\n", [iso stringFromDate:NSDate.date], line] dataUsingEncoding:NSUTF8StringEncoding] error:nil];
  [f closeAndReturnError:nil];
}
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)message {
  (void)controller;
  WKSecurityOrigin *o = message.frameInfo.securityOrigin;
  NSString *line = [message.body isKindOfClass:NSString.class] ? message.body : nil;
  if (!message.frameInfo.mainFrame || ![o.protocol isEqualToString:@"http"] || ![o.host isEqualToString:@"127.0.0.1"] || o.port != self.pageURL.port.integerValue || !line) return;
  if ([message.name isEqualToString:@"forgeMetalMessage"]) {
    NSDictionary *packet = forge_parse(line);
    if ([packet isKindOfClass:NSDictionary.class] && [packet[@"kind"] isKindOfClass:NSNumber.class] && [packet[@"text"] isKindOfClass:NSString.class]) [self.metalWorld message:[packet[@"kind"] intValue] text:packet[@"text"]];
    return;
  }
  if ([message.name isEqualToString:@"forgeVoiceStatus"]) {
    [self traceVoice:line];
    if ([line isEqualToString:@"warm:ready"]) { _voiceWarmReady = YES; fgs_voice_ready(2); [self updateSetup]; }
    if ([line isEqualToString:@"warm:lost"]) { _voiceWarmReady = NO; fgs_voice_ready(3); [self updateSetup]; [self warmVoiceIfNeeded]; }
    return;
  }
  if (![message.name isEqualToString:@"forgeVoiceCoder"] || [line lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 131072) return;
  BOOL available = [self has:@"--voice-smoke"] || self.voiceAvailable;
  NSData *b = [line dataUsingEncoding:NSUTF8StringEncoding];
  int64_t action = fgv_message(b.bytes, (int64_t)b.length, available, self.chat.busy);
  if (action == 1) {
    uint8_t text[16384]; int64_t n = fgv_copy(text, sizeof text);
    if (n > 0) [self.chat startTurn:[[NSString alloc] initWithBytes:text length:(NSUInteger)n encoding:NSUTF8StringEncoding] voice:YES];
  } else if (action == 2) {
    NSString *notice = @"Coder is busy. This voice message was not sent.\n";
    [self.chat append:notice color:NSColor.systemOrangeColor]; [self.chat appendMessage:notice];
    [self cancelVoice:@"cancel"];
  }
}

// ---- the native realm, the preview and reload

- (void)metalAction:(int32_t)code text:(NSString *)text {
  switch (code) {
  case 7: case 8: case 9: {
    NSString *method = code == 7 ? @"toggle" : code == 8 ? @"face" : @"say";
    [self.web callAsyncJavaScript:@"return coaxial.voice[method](method==='toggle' ? (text==='end' ? 'end':text==='hold' ? 'hold':text==='on') : text);" arguments:@{@"method" : method, @"text" : text} inFrame:nil inContentWorld:WKContentWorld.pageWorld completionHandler:nil];
    break;
  }
  case 5: {
    NSData *data = [text dataUsingEncoding:NSUTF8StringEncoding];
    NSDictionary *p = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if ([p isKindOfClass:NSDictionary.class] && [p[@"key"] isKindOfClass:NSString.class]) {
      [self.metalWorld setPlayerKey:p[@"key"]];
      [data writeToURL:[self.dataURL URLByAppendingPathComponent:@"metal-player.json"] atomically:YES];
    }
    break;
  }
  case 13: [[text dataUsingEncoding:NSUTF8StringEncoding] writeToURL:[self.dataURL URLByAppendingPathComponent:@"metal-sound.txt"] atomically:YES]; break;
  case 1: {
    NSURL *url = [NSURL URLWithString:text];
    if ([url.scheme isEqualToString:@"https"] || [url.scheme isEqualToString:@"lightning"]) [NSWorkspace.sharedWorkspace openURL:url];
    break;
  }
  default: break;
  }
}
- (void)togglePreview {
  int64_t destination = self.previewTarget;
  if (destination < 0) return;
  self.switching = YES;
  forge_async(^{
    @try { [self switchWorldView:destination]; }
    @catch (NSException *x) { if (!self.stopping) { NSString *m = x.reason; [self main:^{ [self fail:m]; }]; } }
    [self main:^{ self.switching = NO; }];
  });
}
// Off the main thread.
- (void)switchWorldView:(int64_t)destination {
  if (!self.resources || !self.pageURL) forge_throw(@"World preview is not ready.");
  [self main:^{
    self.sceneReady = NO; self->_worldPauseWanted = nil; [self updateSetup];
    [self cancelVoice:@"cancel"];
    self->_voiceWarmEpoch++; self->_voiceWarming = NO; self->_voiceWarmReady = NO; fgs_voice_ready(0);
    self.notice.stringValue = [NSString stringWithFormat:@"Loading %@…", destination == 0 ? @"web preview" : @"native world"]; self.notice.hidden = NO;
    [self.window makeFirstResponder:nil];
  }];
  if (destination == 0) {
    [self main:^{
      ForgeMetalWorld *previous = self.metalWorld;
      if (previous) {
        [previous focus:NO]; [previous close]; [previous removeFromSuperview]; self.metalWorld = nil;
        self.lastPreviewRetired = @{@"ledgerLive" : @(mf_closed_info(0)), @"ledgerWords" : @(mf_closed_info(1)), @"mediaBytes" : @(mf_closed_info(2)), @"audio" : previous.closedAudio};
      }
      self.web.hidden = NO;
      [self.web loadRequest:[NSURLRequest requestWithURL:self.pageURL]];
    }];
  } else {
    @try { [self js:@"globalThis.coaxial?.setWorldPaused(true); true"]; } @catch (NSException *x) { (void)x; }
    NSURL *auxiliary = [NSURL URLWithString:[NSString stringWithFormat:@"http://127.0.0.1:%@/metal-voice.html", self.pageURL.port]];
    [self main:^{ [self.web loadRequest:[NSURLRequest requestWithURL:auxiliary]]; }];
    double end = forge_uptime() + 10;
    for (;;) {
      __block BOOL there = NO; [self main:^{ there = [self.web.URL isEqual:auxiliary] && !self.web.loading; }];
      if (there) break;
      if (forge_uptime() >= end) forge_throw(@"The web preview did not close its document.");
      forge_sleep_ms(20);
    }
    __block NSString *problem = nil;
    [self main:^{
      self.web.hidden = YES;
      ForgeMetalWorld *view = [self makeMetalWorld:self.web.frame];
      [self.web.superview addSubview:view positioned:NSWindowAbove relativeTo:self.web]; self.metalWorld = view;
      NSError *e = nil;
      if (![view openResources:self.resources storage:self.dataURL page:self.pageURL local:self->_metalLocalOrigin ?: @"" muted:self.mute error:&e]) problem = e.localizedDescription;
    }];
    if (problem) forge_throw(problem);
  }
  [self waitForScene];
  [self main:^{
    self.sceneReady = YES; self.notice.hidden = YES; [self updateSetup];
    [self updateWorldVisibility]; [self warmVoiceIfNeeded];
    if (self.setupHidden) [self.window makeFirstResponder:self.worldView];
  }];
}
- (void)reloadWorld {
  if (self.stopping || self.switching || !self.pageURL) return;
  self.sceneReady = NO; _worldPauseWanted = nil; [self updateSetup];
  [self cancelVoice:@"cancel"];
  _voiceWarmEpoch++; _voiceWarming = NO; _voiceWarmReady = NO; fgs_voice_ready(0);
  if (self.metalWorld) {
    NSError *e = nil;
    if (![self.metalWorld openResources:self.resources storage:self.dataURL page:self.pageURL local:_metalLocalOrigin ?: @"" muted:self.mute error:&e]) { [self fail:e.localizedDescription]; return; }
    forge_async(^{
      @try { [self waitForScene]; [self main:^{ self.sceneReady = YES; [self updateSetup]; [self updateWorldVisibility]; }]; }
      @catch (NSException *x) { NSString *m = x.reason; [self main:^{ [self fail:m]; }]; }
    });
  } else [self.web reload];
}
- (void)stopChildren {
  self.stopping = YES;
  [self.metalWorld close];
  _voiceWarmEpoch++; _voiceWarming = NO; _voiceWarmReady = NO; fgs_voice_ready(0);
  [self cancelVoice:@"cancel"];
  [self.chat shutdown];
  if (_voiceReservation >= 0) { close(_voiceReservation); _voiceReservation = -1; }
  NSArray *children = [@[ self.page ?: NSNull.null, self.room ?: NSNull.null, self.voiceHost ?: NSNull.null ] filteredArrayUsingPredicate:[NSPredicate predicateWithFormat:@"self != nil AND self != %@", NSNull.null]];
  for (NSTask *c in children) if (c.running) [c terminate];
  // Bounded shutdown; reap before releasing the single-world lock.
  NSDate *end = [NSDate dateWithTimeIntervalSinceNow:2];
  for (;;) { BOOL any = NO; for (NSTask *c in children) any |= c.running; if (!any || end.timeIntervalSinceNow <= 0) break; usleep(10000); }
  for (NSTask *c in children) if (c.running) kill(c.processIdentifier, SIGKILL);
  for (NSTask *c in children) [c waitUntilExit];
  for (NSFileHandle *log in _logs) [log closeAndReturnError:nil];
  [_logs removeAllObjects];
  if (_lockFD >= 0) { flock(_lockFD, LOCK_UN); close(_lockFD); _lockFD = -1; }
}
- (void)terminate { [self main:^{ ui_quit(self.host); }]; }

- (int32_t)webProcessPID {
  SEL s = NSSelectorFromString(@"_webProcessIdentifier");
  return [self.web respondsToSelector:s] ? ((int32_t (*)(id, SEL))objc_msgSend)(self.web, s) : 0;
}
- (int32_t)gpuProcessPID {
  // WebKit's guarded diagnostic PID property; no behavior depends on it.
  SEL s = NSSelectorFromString(@"_gpuProcessIdentifier");
  return [self.web respondsToSelector:s] ? ((int32_t (*)(id, SEL))objc_msgSend)(self.web, s) : 0;
}
- (NSDictionary *)worldLoop { id r = [self callJS:@"return coaxial.worldLoop();" arguments:nil]; return [r isKindOfClass:NSDictionary.class] ? r : @{}; }
- (void)captureCanvas:(NSURL *)report {
  NSString *data = [self js:@"document.querySelector('canvas').toDataURL('image/png')"];
  NSRange comma = [data isKindOfClass:NSString.class] ? [data rangeOfString:@","] : NSMakeRange(NSNotFound, 0);
  if (comma.location == NSNotFound) return;
  NSData *png = [[NSData alloc] initWithBase64EncodedString:[data substringFromIndex:comma.location + 1] options:0];
  [png writeToURL:[report.URLByDeletingPathExtension URLByAppendingPathExtension:@"png"] atomically:NO];
}

// ---- web view

- (void)webView:(WKWebView *)w decidePolicyForNavigationAction:(WKNavigationAction *)a decisionHandler:(void (^)(WKNavigationActionPolicy))decide {
  (void)w; NSURL *u = a.request.URL;
  decide([u.scheme isEqualToString:@"http"] && [u.host isEqualToString:@"127.0.0.1"] && [u.port isEqual:self.pageURL.port] ? WKNavigationActionPolicyAllow : WKNavigationActionPolicyCancel);
}
- (void)webView:(WKWebView *)w didFailProvisionalNavigation:(WKNavigation *)n withError:(NSError *)error {
  (void)w; (void)n;
  if (!self.stopping && error.code != NSURLErrorCancelled) [self fail:error.localizedDescription];
}
- (void)webView:(WKWebView *)w didStartProvisionalNavigation:(WKNavigation *)n { (void)w; (void)n; self.sceneReady = NO; _worldPauseWanted = nil; [self updateSetup]; }
- (void)webView:(WKWebView *)w didFinishNavigation:(WKNavigation *)n {
  (void)w; (void)n;
  if (self.switching) return;
  [self warmVoiceIfNeeded];
  if (self.firstFrameSeconds > 0) forge_async(^{
    @try { [self waitForScene]; [self main:^{ self.sceneReady = YES; [self updateSetup]; [self updateWorldVisibility]; }]; }
    @catch (NSException *x) { if (!self.stopping) { NSString *m = x.reason; [self main:^{ [self fail:m]; }]; } }
  });
}
- (void)webViewWebContentProcessDidTerminate:(WKWebView *)w { (void)w; [self fail:@"The world view stopped. Quit and reopen Aethyrforge."]; }
- (void)webView:(WKWebView *)w requestMediaCapturePermissionForOrigin:(WKSecurityOrigin *)origin initiatedByFrame:(WKFrameInfo *)frame type:(WKMediaCaptureType)type decisionHandler:(void (^)(WKPermissionDecision))decide {
  (void)w;
  [self traceVoice:[NSString stringWithFormat:@"microphone request: main=%@ local=%@ key=%@ setup=%@ authorization=%ld", frame.mainFrame ? @"true" : @"false",
                                              [origin.host isEqualToString:@"127.0.0.1"] && origin.port == self.pageURL.port.integerValue ? @"true" : @"false", self.window.keyWindow ? @"true" : @"false",
                                              !self.setupHidden ? @"true" : @"false", (long)[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]]];
  if (type != WKMediaCaptureTypeMicrophone || ![self trustedVoiceFrame:frame] || self.smoke || self.stopping) { decide(WKPermissionDecisionDeny); return; }
  forge_async(^{
    BOOL authorized = [self authorizeMicrophone];
    [self main:^{
      BOOL allowed = fm_permission([self trustedVoiceFrame:frame], type == WKMediaCaptureTypeMicrophone, authorized);
      self.microphoneDecisions++;
      [self traceVoice:[NSString stringWithFormat:@"microphone permission: %@ key=%@", allowed ? @"true" : @"false", self.window.keyWindow ? @"true" : @"false"]];
      decide(allowed ? WKPermissionDecisionGrant : WKPermissionDecisionDeny);
    }];
  });
}

// ---- the process

- (int)run {
  _logs = [NSMutableArray array]; _coderHeldKeys = [NSMutableSet set];
  _voiceReservation = -1; _lockFD = -1; _startedAt = forge_uptime();
  self.resources = NSBundle.mainBundle.resourceURL;
  NSString *shot = [self arg:@"--shot"];
  if (shot) return [self shot:shot];
  if (self.worldBackend < 0) { fprintf(stderr, "Aethyrforge: --world requires web or metal.\n"); return 2; }
  signal(SIGTERM, SIG_IGN);
  _termination = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGTERM, 0, dispatch_get_main_queue());
  dispatch_source_set_event_handler(_termination, ^{ [self terminate]; });
  dispatch_resume(_termination);
  [self openWindow];
  if (self.launchReport && [self has:@"--launch-check-background"]) {
    // Let AppKit finish its first activation before minimizing; a reopen event can undo a synchronous hide.
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), dispatch_get_main_queue(), ^{
      [self.window miniaturize:nil];
      dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 65 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
        if (self.stopping) return;
        [self.window deminiaturize:nil]; [self.window makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES];
      });
    });
  }
  if ([self has:@"--chat-smoke"]) forge_async(^{ [self chatSmokeCheck]; });
  else forge_async(^{ [self start]; });
  while (ui_frame(self.host)) {}
  [self stopChildren];
  ui_close(self.host);
  return 0;
}
// --shot: one frame offscreen (Forge.h).
- (int)shot:(NSString *)path {
  uint32_t w = 1280, h = 800;
  NSString *size = [self arg:@"--size"];
  if (size) sscanf(size.UTF8String, "%ux%u", &w, &h);
  NSString *scene = [self arg:@"--scene"] ?: @"setup";
  float d = 2;
  NSRect bounds = NSMakeRect(0, 0, w, h);
  _picture = YES;
  fgs_reset();
  self.chat = [[ForgeChat alloc] initWithFrame:bounds];
  [self.chat layoutSubtreeIfNeeded];
  self.checking = YES;
  if ([scene isEqualToString:@"status"]) { self.statusMode = YES; self.sceneReady = YES; self.checking = NO; }
  if ([scene isEqualToString:@"feed"] || [scene isEqualToString:@"composer"]) {
    self.setupHidden = YES;
    if ([scene isEqualToString:@"feed"]) {
      self.chat.busy = YES;
      [self.chat receive:@"{\"kind\":\"reply\",\"text\":\"Active reply on the left\"}"];
      [self.chat appendMessage:@"Aethyr draws this message and owns its five-second lifetime.\nCurrent folder: ~/.openagents/aethyrforge/workspace\n"];
    } else self.chat.composerOpen = YES;
  }
  if (![self shotOf:[NSURL fileURLWithPath:path] width:(uint32_t)(w * d) height:(uint32_t)(h * d) dpr:d]) { fprintf(stderr, "Aethyrforge: the picture was not written\n"); return 1; }
  // --do "OP ; OP": coax drive's operations on the picture's host (tree, tap ID, about ID, and the host's ax,
  // ax-press ID, ax-key KEY), each answer printed; --drive SOCKET: then whatever coax drive asks, until quit.
  NSString *script = [self arg:@"--do"], *socket = [self arg:@"--drive"];
  if (script || socket) {
    self.host = _shot;
    static char answer[1 << 16];
    for (NSString *part in [script ?: @"" componentsSeparatedByString:@" ; "]) {
      NSString *one = [part stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceCharacterSet];
      if (!one.length) continue;
      int k = ui_drive(_shot, one.UTF8String, answer, (int)sizeof answer);
      if (k > 0) printf("%.*s%s", k, answer, answer[k - 1] == '\n' ? "" : "\n");
    }
    if (socket && !ui_drive_listen(_shot, socket.fileSystemRepresentation)) { fprintf(stderr, "Aethyrforge: could not listen on %s\n", socket.UTF8String); return 1; }
    if (socket) ui_drive_serve(_shot);
    self.host = NULL;
  }
  if ([scene isEqualToString:@"feed"]) {   // and the feed alone, as --input-smoke's frame
    NSURL *frame = [[NSURL fileURLWithPath:path].URLByDeletingPathExtension URLByAppendingPathExtension:@"frame.png"];
    if ([self.chat saveFeedFrame:frame device:(__bridge id<MTLDevice>)ui_device(_shot) queue:(__bridge id<MTLCommandQueue>)ui_queue(_shot)] <= 0) return 1;
  }
  printf("%s: %s %ux%u\n", path.UTF8String, scene.UTF8String, (unsigned)(w * d), (unsigned)(h * d));
  return 0;
}
@end

int main(int argc, char **argv) {
  @autoreleasepool {
    ForgeApp *app = [ForgeApp new];
    NSMutableArray *args = [NSMutableArray array];
    for (int i = 0; i < argc; i++) [args addObject:@(argv[i])];
    app.args = args;
    @try { return [app run]; }
    @catch (NSException *x) { fprintf(stderr, "Aethyrforge: %s\n", x.reason.UTF8String); return 1; }
  }
}
