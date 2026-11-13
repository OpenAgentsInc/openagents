// Firm reason: 6 (AppKit, WebKit and AVFoundation effects; world, setup, chat and voice decisions are Coax).
// The Metaverse Engine for Mac on the shared Mac host (apps/shared/host/apple/ui-host.m, the ui-host.h
// contract OpenAgents and Everglade use; GitHub #13156). The host owns the window, the event loop,
// the menus, the setup panel's components and its pictures; this app holds the world (a WKWebView,
// or the native Metal realm, MetalWorld.swift), the Coder chat's native text views and its HUD
// (drawn on the host's layer, HUD.metal), the bundled child processes and the checks.
#import <AppKit/AppKit.h>
#import <WebKit/WebKit.h>
#import <AVFoundation/AVFoundation.h>
#import <Metal/Metal.h>
#include "../face.h"
#include "../../shared/host/apple/ui-host-apple.h"

@class EditorMetalWorld;

// What a check or a startup step throws: the message the app shows (fail:).
void editor_throw(NSString *message) __attribute__((noreturn));
// A function, not a macro: a condition with Objective-C messages holds commas the preprocessor would split.
void editor_require(BOOL ok, NSString *format, ...) NS_FORMAT_FUNCTION(2, 3);
#define EDITOR_REQUIRE editor_require
NSString *editor_json(id object);          // sorted keys; "{}" when it cannot
NSString *editor_json_pretty(id object);
id editor_parse(NSString *json);           // nil when it is not JSON
void editor_write(NSString *text, NSURL *url);   // atomically; throws on failure
void editor_sleep_ms(int ms);
double editor_uptime(void);
NSString *editor_setup_text(int64_t field);

// Runs a local CLI off the main thread: its stdout (at most 64 KiB), or throws.
NSData *editor_cli(NSURL *executable, NSArray<NSString *> *arguments, double timeout, NSDictionary *environment);

// The Coder chat: transcript and composer text views, buttons and labels as native children of
// the host's view (under its layer), and its two HUDs (the composer's frame and the message feed)
// drawn on the host's layer from Coax's twelve-float strokes (metaverse-engine/ui/feed-draw.coax).
@interface EditorChat : NSView
@property(nonatomic, strong) NSTextView *transcript, *input;
@property(nonatomic, strong) NSScrollView *transcriptScroll, *inputScroll;
@property(nonatomic, strong) NSTextField *router, *heading, *status;
@property(nonatomic, strong) NSButton *folder, *send, *fresh, *allow, *deny;
@property(nonatomic) BOOL busy, pending, composerOpen, stopping, streamedWithoutComposer;
@property(nonatomic) int liveChunks;
@property(nonatomic, copy) NSString *session, *parent, *lastEnding, *selectedEngine, *engineOverride;
@property(nonatomic, strong) NSURL *cwd, *home, *cliOverride, *sceneContextURL, *sceneSessionURL;
@property(nonatomic, copy) void (^onActivity)(void);
@property(nonatomic, copy) void (^onClose)(void);
@property(nonatomic, readonly) int64_t presentation;
@property(nonatomic, readonly) NSString *messageText;
@property(nonatomic, readonly) NSURL *sessions, *cli;
- (instancetype)initWithFrame:(NSRect)frame;
- (void)update;
- (void)openComposer;
- (void)closeComposer;
- (void)append:(NSString *)text color:(NSColor *)color;
- (void)appendMessage:(NSString *)text;
- (void)finishMessages;
- (void)newChat;
- (void)submit;
- (void)startTurn:(NSString *)draft voice:(BOOL)voice;
- (void)receive:(NSString *)line;
- (void)reject;
- (void)stopTurn;
- (void)shutdown;
- (BOOL)hudShown;          // the composer's HUD is drawn
- (BOOL)feedShown;         // the message feed is drawn
- (NSRect)box:(int)kind;
// The HUD's strokes for the whole view (w by h points at scale d): how many vertices, 12 floats each.
- (int)hudVertices:(float *)out capacity:(int)cap width:(float)w height:(float)h scale:(float)d;
// The HUD on the host's pass (ui_layer's encode): the whole view, w by h device pixels at scale d.
- (void)encodeHUD:(id<MTLRenderCommandEncoder>)encoder device:(id<MTLDevice>)device width:(uint32_t)w height:(uint32_t)h scale:(float)d;
// The feed drawn alone, as the old check frame was: twice its box, over a dark matte. Vertices.
- (int)saveFeedFrame:(NSURL *)png device:(id<MTLDevice>)device queue:(id<MTLCommandQueue>)queue;
- (id<MTLRenderPipelineState>)pipelineOn:(id<MTLDevice>)device;   // HUD.metal's, nil when it did not load
@end

@interface EditorApp : NSObject <WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler, WKScriptMessageHandlerWithReply>
@property(nonatomic) UiHost *host;
@property(nonatomic, strong) NSWindow *window;
@property(nonatomic, strong) NSArray<NSString *> *args;
@property(nonatomic, strong) WKWebView *web;
@property(nonatomic, strong) EditorMetalWorld *metalWorld;
@property(nonatomic, strong) NSTextField *notice;
@property(nonatomic, strong) EditorChat *chat;
@property(nonatomic, strong) NSTask *room, *page, *voiceHost;
@property(nonatomic, strong) NSURL *dataURL, *pageURL, *resources;
@property(nonatomic) BOOL sceneReady, stopping, setupHidden, statusMode, checking, verifying, worldPaused;
@property(nonatomic, copy) NSString *setupError;
@property(nonatomic, copy) NSString *refreshTitle;
@property(nonatomic) BOOL microphonePrompt, highRefreshEnabled, retryCheckStarted, retryCheckPassed;
@property(nonatomic) double firstFrameSeconds, longestHiddenSeconds;
@property(nonatomic) int64_t startupPhase;
@property(nonatomic) int microphoneDecisions;
@property(nonatomic) BOOL microphoneCheckFailStartup, microphoneCheckDelay;
@property(nonatomic, copy) AVAuthorizationStatus (^microphoneAuthorization)(void);
@property(nonatomic, copy) BOOL (^microphoneAccessRequest)(void);   // runs off the main thread
@property(nonatomic, strong) NSDictionary *lastPreviewRetired;
@property(nonatomic) int panelDraws, panelVertices;
@property(nonatomic) double panelGPUSeconds;
@property(nonatomic) BOOL switching;
@property(nonatomic, strong) dispatch_group_t inventory;
// the setup panel's buttons (fgu_box kinds 1..5): enabled, from Coax
@property(nonatomic, readonly) BOOL enterEnabled, refreshEnabled, microphoneEnabled;
@property(nonatomic, readonly) NSString *microphoneTitle, *microphoneStatusText;
- (NSString *)arg:(NSString *)flag;     // the value after flag, or nil
- (BOOL)has:(NSString *)flag;
- (BOOL)smoke;
- (NSURL *)report;
- (NSURL *)launchReport;
- (NSURL *)microphoneReport;
- (NSURL *)frameProfileURL;
- (NSURL *)setupProfileURL;
- (NSURL *)offlineReportURL;
- (NSURL *)benchmarkReportURL;
- (BOOL)offlineWorker;
- (BOOL)offlineMode;
- (BOOL)mute;
- (int64_t)worldBackend;
- (NSView *)worldView;
- (NSURL *)settingsURL;
// Off the main thread: the page's answer (nil for none), or throws with WebKit's error.
- (id)js:(NSString *)source;
- (id)callJS:(NSString *)body arguments:(NSDictionary *)arguments;
- (void)main:(dispatch_block_t)block;          // runs block on the main thread and waits
- (void)fail:(NSString *)message;
- (void)stopChildren;
- (void)terminate;                             // ends the event loop; the process exits after cleanup
- (void)enterWorld;
- (void)showSetup;
- (void)showStatus;
- (void)reloadWorld;
- (void)refreshSetup:(BOOL)verify;
- (void)waitForScene;                          // off the main thread
- (void)ensureVoiceHost;                       // off the main thread
- (void)prepareVoice;                          // off the main thread
- (BOOL)authorizeMicrophone;                   // off the main thread
- (void)refreshMicrophoneSetup;
- (AVAuthorizationStatus)microphoneStatus;
- (void)captureCanvas:(NSURL *)report;
- (void)updateSetup;
- (int64_t)previewTarget;                      // fwp_target: the world the preview command switches to, or -1
- (void)togglePreview;
- (BOOL)reloadAllowed;
- (void)metalAction:(int32_t)code text:(NSString *)text;
- (int32_t)webProcessPID;
- (int32_t)gpuProcessPID;
- (NSDictionary *)worldLoop;                   // off the main thread
- (void)capturePanel:(NSURL *)png;
- (NSString *)previewTitle;
- (void)cancelVoice:(NSString *)reason;
- (void)restartStartup;
- (void)focusChat;
- (void)waitInventory;                         // off the main thread: the setup inventory's end
- (NSString *)setupButton:(int)button;         // a setup panel button pressed (fgu_box kinds 1..5); in a --shot picture, what it would open
- (NSString *)launchState;                     // off the main thread
- (void)observeMetal:(NSURL *)path;            // off the main thread, until stopping
@end

// The checks (checks.m), each run off the main thread after startup; each ends the app.
@interface EditorApp (Checks)
- (void)runCheck;          // the one the arguments name, if any
- (void)chatSmokeCheck;
- (void)measureWebPreview:(NSURL *)plan;
- (void)measureMetal:(NSURL *)plan;
- (void)setupProfileCheck;
- (void)offlineCheck;
- (void)captureFrameProfile:(NSURL *)url;
@end
