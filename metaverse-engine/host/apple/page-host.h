// Firm reason: 6 (Metal, URLSession and CoreText have no C API; this holds no product logic).
// A realm's page on Apple's GPUs: what coax/web/host.js does for a realm in a browser, done with
// Metal for the same face built natively (metaverse/native/build.sh) and the same shaders emitted by
// `coax metal`. It draws any realm that uses the page's own passes (a background, a shadow map, a
// scene of indexed draws over stores, particles, bloom and post, and the HUD's strokes): Everglade
// II, Character Create, Everglade Experimental and the Archery Range today. It decides nothing about
// the game: it hands the face its assets, messages, keys and frames, uploads what the face hands back,
// and carries out its actions. Its sound packets and sound bank go to the Apple audio host
// (audio-host.h: mf_sound_bank, mf_sound_commands; PageAudio.swift plays them).
// The Mac and iOS hosts link it; a host on another GPU needs its own twin of PageHost.m.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>

// What a host says when it opens a page. Paths are files on this machine.
@interface AEPageConfig : NSObject
@property (copy) NSString *producer;   // the realm's library (the face and its Coax, natively)
@property (copy) NSString *shaders;    // ROLE.metallib and ROLE.bind for bg, shadow, scene, bloom, post, fx, particle, particle_over; page.bindings
@property (copy) NSString *pack;       // the realm's VPK1 pack, or that pack gzipped as the site serves it (NAME.pack.gz)
@property (copy) NSString *fonts;      // the HUD's faces: READING.ttf and TITLE.ttf
@property (copy) NSString *reading, *title;   // their file names without .ttf
@property double titleWeight;
@property double titleScale;       // same title-atlas scale as the browser; zero keeps 1
@property (copy) NSString *web;        // the site's files, for the pictures a page names (/worlds/NAME/... is realms/NAME/...)
@property (copy) NSString *store;      // where this page keeps what a browser keeps in its storage
@property (copy) NSString *origin;     // the site: https://openagents.com
@property (copy) NSString *path;       // the page's path and query on it
@property (copy) NSString *realm, *slot, *style, *clock;
@property (copy) NSString *variant;
@property BOOL offline;                // no socket and no HTTP leave this machine (a shot)
@property BOOL automated;              // nobody is at the keys: said in the page's facts, as a browser under a driver says it
@property BOOL touch;                  // a touch screen (a phone or a tablet): said in the page's facts, as a browser's coarse pointer is, so the realm draws its touch controls
@property CGSize screen;               // the screen in points (a browser's screen.width), for the page's facts; zero on a Mac, where the host reads the main display
@property (copy) int32_t (^layerSide)(int32_t declared);   // the side the pack's layers are drawn to, from the one the realm declares (metaverse-engine/render/texture-format.coax tf_side); nil keeps it
// Optional installed account/network adapter. It owns authentication and SDK
// cancellation; credentials never enter the realm or its editor model.
@property (copy) dispatch_block_t (^request)(NSURLRequest *,void (^)(NSData *,NSURLResponse *,NSError *));
@property (copy) void (^action)(int32_t,NSString *);
@property (copy) NSString *viewer;
@property (copy) NSString *player;   // opaque SDK-owned player record, never part of an editor model
@property (copy) void (^assetNeeded)(NSString *);
@end

@interface AEPageHost : NSObject
// For a host that draws a pack itself (the Workshop's model). A pack's image for this device: the
// block build its GPU takes when one stands beside the pack (NAME.bc7.vpk3 or NAME.astc4.vpk3,
// read whole with every page checked), else the pack itself. nil when the file can't be read.
+ (NSData *)packImage:(NSString *)path device:(id<MTLDevice>)device;
// That image's colour layers as one texture array of GPU blocks with their mip chains, uploaded as
// they are. nil when its layers are pictures for a decoder, or this GPU has no such format.
+ (id<MTLTexture>)blockLayers:(NSData *)image device:(id<MTLDevice>)device;
// nil when the page can't open; `why` then says what is missing.
+ (instancetype)open:(AEPageConfig *)config device:(id<MTLDevice>)device queue:(id<MTLCommandQueue>)queue why:(NSString **)why;
// One frame of a view `points` in size at `scale` pixels a point. Returns the picture, drawn by
// the time anything committed to the same queue afterwards reads it.
- (id<MTLTexture>)frame:(CGSize)points scale:(CGFloat)scale dt:(float)dt wait:(BOOL)wait;
// Draws the last picture over the whole of a pass whose colour is `format`.
- (void)present:(id<MTLRenderCommandEncoder>)encoder format:(MTLPixelFormat)format;
// Keys in the contract's codes (metaverse-engine/host/README.md): a character's code point, or a named key's
// negative number. Bits of mods: 1 Shift, 2 Control, 4 Alt, 8 Meta. YES when the page used it.
- (BOOL)key:(int32_t)key mods:(int32_t)mods down:(BOOL)down;
- (void)text:(NSString *)typed;
// Points from the view's top left. phase 0 down, 1 moved, 2 up, 3 cancelled; button 0 left, 2 right.
- (void)pointer:(CGPoint)at phase:(int32_t)phase button:(int32_t)button;
// A finger on a touch screen, in the same points and phases; `finger` is its own number from down to up.
- (void)touch:(CGPoint)at phase:(int32_t)phase finger:(int32_t)finger;
- (void)wheel:(float)dy;
- (void)focus:(BOOL)focused;
// The page out of sight (another world is open, or the window is hidden) and shown again: the realm
// hears it as a hidden browser tab's does (messages 601 and 602), and its held keys are let go.
// While it is out of sight no frame is drawn; call idle as often as a frame so the socket's events
// still reach the realm and never pile up.
- (void)shown:(BOOL)shown;
- (void)idle;
// A command for the realm's agent driver (metaverse-engine/agent/drive.coax: tree, walk-to ID, face ID,
// use ABILITY on ID, status ...), and its answer; nil when the realm has no driver.
- (NSString *)drive:(NSString *)command;
// 2 while the page wants the pointer held (mouselook). The host says what it did: 1 held, 0 let go, 2 couldn't.
- (int32_t)mouselook;
- (void)pointerLock:(int32_t)status;
- (void)look:(CGPoint)travel;
// The phone's turn as look and aim (metaverse-engine/cam/gyro.coax): YES while the page wants it, so the app
// runs CoreMotion only then; the turn since the last call in radians (yaw about the world's up, a
// turn to the left positive; pitch about the screen's across, the top tilted back positive); and
// the sensor's word: 1 its turns come, 2 refused, 3 none.
- (BOOL)wantsMotion;
- (void)tilt:(float)yaw pitch:(float)pitch;
- (void)motionStatus:(int32_t)status;
- (BOOL)wantsText;
// The existing editor-model wire and message boundary, shared with host.js.
- (NSData *)editorModel;
- (int32_t)editorRevision;
- (BOOL)editorOwned;
- (void)message:(int32_t)kind text:(NSString *)text;
- (BOOL)deliverAsset:(int32_t)asset bytes:(NSData *)bytes;
- (void)assetsChanged;
// 0 the arrow, 1 a hand, 2 a crosshair, 3 a text bar, 4 none.
- (int32_t)cursor;
- (void)close;
@end
