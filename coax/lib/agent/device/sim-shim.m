// Firm reason: 6 (CoreSimulator and SimulatorKit are private Objective-C frameworks)
// The iOS Simulator helper's private-framework effects (docs/desktop/ios-simulator-spec.md 6.2;
// GitHub #13811), linked into oasim through coax/web/tool.sh's `objc` line and called from
// sim-shim.coax. This is the one file that knows the private names; every one is looked up at
// start and a missing one lowers a capability, never crashes. What to do and whether it is
// allowed is decided in Coax (sim-host.coax, sim-policy.coax, sim-gesture.coax): these calls only
// carry it out.
// Probed on Xcode 26.6 (iOS 26.5 runtime), 2026-11-20:
//   SimServiceContext +sharedServiceContextForDeveloperDir:error:, -defaultDeviceSetWithError:,
//   SimDeviceSet -devices, SimDevice -UDID -io, SimDeviceIOClient -ioPorts, a port's -descriptor
//   with -framebufferSurface (an IOSurface, BGRA, device pixels);
//   SimulatorKit.SimDeviceLegacyHIDClient -initWithDevice:error:
//   -sendWithMessage:freeWhenDone:completionQueue:completion:, and the C builders
//   IndigoHIDMessageForMouseNSEvent (the point as a fraction of the screen, target 0x32, NSEvent
//   types 1 down, 2 up; a move is another down: type 6 makes no message), IndigoHIDMessageForKeyboardArbitrary (HID usage, 1 down / 2 up),
//   IndigoHIDMessageForButton (source, 1 down / 2 up, target 0x33).
//   describe: AccessibilityPlatformTranslation's AXPTranslator +sharedInstance, -setBridgeTokenDelegate:
//   (its -accessibilityTranslationDelegateBridgeCallbackWithToken: answered through -[SimDevice
//   sendAccessibilityRequestAsync:completionQueue:completionHandler:]),
//   -frontmostApplicationWithDisplayId:bridgeDelegateToken: and -macPlatformElementFromTranslation:,
//   whose elements' accessibilityRole/Label/Value/Frame/Children are in device points.
#import <Foundation/Foundation.h>
#import <IOSurface/IOSurface.h>
#import <CoreGraphics/CoreGraphics.h>
#import <ImageIO/ImageIO.h>
#import <objc/message.h>
#include <dlfcn.h>
#include <mach/mach.h>
#include <stdint.h>
#include <unistd.h>

#ifndef COAX_coax_v_u8_DEFINED
#define COAX_coax_v_u8_DEFINED
typedef struct { uint8_t *data; int64_t count; } coax_v_u8;
#endif
#ifndef COAX_coax_v_f64_DEFINED
#define COAX_coax_v_f64_DEFINED
typedef struct { double *data; int64_t count; } coax_v_f64;
#endif

typedef void *(*SsMouseFn)(CGPoint *, CGPoint *, uint32_t, uint64_t, CGSize, uint32_t);
typedef void *(*SsKeyFn)(uint32_t, uint32_t);
typedef void *(*SsButtonFn)(uint32_t, uint32_t, uint32_t);

static id ss_set;                      // the default device set
static SsMouseFn ss_mouse; static SsKeyFn ss_key; static SsButtonFn ss_button;
static Class ss_hid_class;
static NSMutableDictionary<NSString *, id> *ss_hids;
static dispatch_queue_t ss_queue;
static int64_t ss_caps;

static NSString *ss_str(coax_v_u8 v) { return [[NSString alloc] initWithBytes:v.data length:(NSUInteger)v.count encoding:NSUTF8StringEncoding] ?: @""; }
static id ss_send0(id o, const char *sel) { SEL s = sel_registerName(sel); return [o respondsToSelector:s] ? ((id(*)(id, SEL))objc_msgSend)(o, s) : nil; }

// Xcode's developer folder (xcode-select -p), or "" when none.
static NSString *ss_developer_dir(void) {
  FILE *p = popen("/usr/bin/xcode-select -p 2>/dev/null", "r");
  if (!p) return @"";
  char buf[1024] = {0}; size_t n = fread(buf, 1, sizeof buf - 1, p); pclose(p);
  while (n > 0 && (buf[n - 1] == '\n' || buf[n - 1] == ' ')) buf[--n] = 0;
  return [NSString stringWithUTF8String:buf] ?: @"";
}

// Loads the frameworks and resolves every name. Its capabilities: 1 the display (stills from the
// framebuffer), 2 touch, 4 keys and buttons. 0 when CoreSimulator can't be used.
int64_t simshim_start(void) {
  @autoreleasepool {
    if (ss_set) return ss_caps;
    NSString *dev = ss_developer_dir();
    if (dev.length == 0 || [dev hasPrefix:@"/Library/Developer/CommandLineTools"]) return 0;
    if (!dlopen("/Library/Developer/PrivateFrameworks/CoreSimulator.framework/CoreSimulator", RTLD_NOW)) return 0;
    void *sk = dlopen([dev stringByAppendingString:@"/Library/PrivateFrameworks/SimulatorKit.framework/SimulatorKit"].UTF8String, RTLD_NOW);
    Class cc = NSClassFromString(@"SimServiceContext");
    SEL shared = sel_registerName("sharedServiceContextForDeveloperDir:error:");
    if (!cc || ![cc respondsToSelector:shared]) return 0;
    NSError *err = nil;
    id ctx = ((id(*)(id, SEL, id, NSError **))objc_msgSend)(cc, shared, dev, &err);
    SEL defset = sel_registerName("defaultDeviceSetWithError:");
    if (!ctx || ![ctx respondsToSelector:defset]) return 0;
    ss_set = ((id(*)(id, SEL, NSError **))objc_msgSend)(ctx, defset, &err);
    if (!ss_set) return 0;
    ss_caps = 1;
    ss_hids = [NSMutableDictionary dictionary];
    ss_queue = dispatch_queue_create("oasim.hid", DISPATCH_QUEUE_SERIAL);
    if (sk) {
      ss_mouse = (SsMouseFn)dlsym(sk, "IndigoHIDMessageForMouseNSEvent");
      ss_key = (SsKeyFn)dlsym(sk, "IndigoHIDMessageForKeyboardArbitrary");
      ss_button = (SsButtonFn)dlsym(sk, "IndigoHIDMessageForButton");
      ss_hid_class = NSClassFromString(@"SimulatorKit.SimDeviceLegacyHIDClient");
      BOOL client = ss_hid_class && [ss_hid_class instancesRespondToSelector:sel_registerName("initWithDevice:error:")]
                    && [ss_hid_class instancesRespondToSelector:sel_registerName("sendWithMessage:freeWhenDone:completionQueue:completion:")];
      if (client && ss_mouse) ss_caps |= 2;
      if (client && ss_key && ss_button) ss_caps |= 4;
    }
    return ss_caps;
  }
}

static id ss_device(NSString *udid) {
  for (id d in (NSArray *)ss_send0(ss_set, "devices")) {
    if ([[(NSUUID *)ss_send0(d, "UDID") UUIDString] isEqualToString:udid]) return d;
  }
  return nil;
}
static IOSurfaceRef ss_surface(id dev) {
  id io = ss_send0(dev, "io");
  for (id port in (NSArray *)ss_send0(io, "ioPorts")) {
    id desc = ss_send0(port, "descriptor");
    id s = ss_send0(desc, "framebufferSurface");
    if (s) return (__bridge IOSurfaceRef)s;
  }
  return NULL;
}

// The screen's size in device pixels into out[0], out[1]. 1, or 0 when there is no framebuffer
// (the device is not booted, or the display API went).
int64_t simshim_size(coax_v_u8 udid, coax_v_f64 out) {
  @autoreleasepool {
    if (!ss_set || out.count < 2) return 0;
    IOSurfaceRef s = ss_surface(ss_device(ss_str(udid)));
    if (!s) return 0;
    out.data[0] = (double)IOSurfaceGetWidth(s); out.data[1] = (double)IOSurfaceGetHeight(s);
    return 1;
  }
}

// The framebuffer's seed (IOSurfaceGetSeed: it changes when the screen does), or -1 when there is none.
int64_t simshim_seed(coax_v_u8 udid) {
  @autoreleasepool {
    if (!ss_set) return -1;
    IOSurfaceRef s = ss_surface(ss_device(ss_str(udid)));
    return s ? (int64_t)IOSurfaceGetSeed(s) : -1;
  }
}

// The screen as a PNG at path, w by h pixels (scaled from the framebuffer), turned counter-clockwise
// by `turns` quarter turns (the framebuffer stays portrait when the device is turned: 1 for
// landscape left, 2 upside down, 3 landscape right). Its bytes, or -1.
int64_t simshim_shot(coax_v_u8 udid, coax_v_u8 path, int64_t w, int64_t h, int64_t turns) {
  @autoreleasepool {
    if (!ss_set || w <= 0 || h <= 0) return -1;
    IOSurfaceRef s = ss_surface(ss_device(ss_str(udid)));
    if (!s) return -1;
    IOSurfaceLock(s, kIOSurfaceLockReadOnly, NULL);
    size_t sw = IOSurfaceGetWidth(s), sh = IOSurfaceGetHeight(s), bpr = IOSurfaceGetBytesPerRow(s);
    CGColorSpaceRef cs = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef src = CGBitmapContextCreate(IOSurfaceGetBaseAddress(s), sw, sh, 8, bpr, cs, kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little);
    CGImageRef full = src ? CGBitmapContextCreateImage(src) : NULL;
    IOSurfaceUnlock(s, kIOSurfaceLockReadOnly, NULL);
    if (src) CGContextRelease(src);
    if (!full) { CGColorSpaceRelease(cs); return -1; }
    CGImageRef img = full;
    turns = ((turns % 4) + 4) % 4;
    if ((size_t)w != sw || (size_t)h != sh || turns != 0) {
      CGContextRef dst = CGBitmapContextCreate(NULL, (size_t)w, (size_t)h, 8, 0, cs, kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little);
      CGContextSetInterpolationQuality(dst, kCGInterpolationHigh);
      // the picture before turning is w by h, or h by w for a quarter turn
      double iw = (double)w, ih = (double)h;
      if (turns == 1) { CGContextTranslateCTM(dst, w, 0); CGContextRotateCTM(dst, M_PI_2); iw = (double)h; ih = (double)w; }
      if (turns == 2) { CGContextTranslateCTM(dst, w, h); CGContextRotateCTM(dst, M_PI); }
      if (turns == 3) { CGContextTranslateCTM(dst, 0, h); CGContextRotateCTM(dst, -M_PI_2); iw = (double)h; ih = (double)w; }
      CGContextDrawImage(dst, CGRectMake(0, 0, iw, ih), full);
      img = CGBitmapContextCreateImage(dst); CGContextRelease(dst); CGImageRelease(full);
    }
    CGColorSpaceRelease(cs);
    NSURL *url = [NSURL fileURLWithPath:ss_str(path)];
    CGImageDestinationRef d = CGImageDestinationCreateWithURL((__bridge CFURLRef)url, CFSTR("public.png"), 1, NULL);
    BOOL ok = NO;
    if (d) { CGImageDestinationAddImage(d, img, NULL); ok = CGImageDestinationFinalize(d); CFRelease(d); }
    CGImageRelease(img);
    if (!ok) return -1;
    NSNumber *size = nil; [url getResourceValue:&size forKey:NSURLFileSizeKey error:nil];
    return size ? size.longLongValue : 0;
  }
}

static id ss_hid(NSString *udid) {
  id h = ss_hids[udid];
  if (h) return h;
  id dev = ss_device(udid);
  if (!dev || !ss_hid_class) return nil;
  NSError *err = nil;
  h = ((id(*)(id, SEL, id, NSError **))objc_msgSend)([ss_hid_class alloc], sel_registerName("initWithDevice:error:"), dev, &err);
  if (h) ss_hids[udid] = h;
  return h;
}
// Sends one Indigo message and waits for it to be taken (at most a second). 1 when it was.
static int64_t ss_post(NSString *udid, void *msg) {
  if (!msg) return 0;
  id h = ss_hid(udid);
  if (!h) { free(msg); return 0; }
  __block int64_t ok = 0;
  dispatch_semaphore_t done = dispatch_semaphore_create(0);
  ((void(*)(id, SEL, void *, BOOL, id, id))objc_msgSend)(h, sel_registerName("sendWithMessage:freeWhenDone:completionQueue:completion:"), msg, YES, ss_queue,
    ^(NSError *e) { ok = e ? 0 : 1; dispatch_semaphore_signal(done); });
  if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) != 0) return 0;
  if (!ok) [ss_hids removeObjectForKey:udid];   // a client that failed is made again next time
  return ok;
}

// One touch at (x, y) device points on a screen w by h points: phase 0 down, 1 moved, 2 up.
int64_t simshim_touch(coax_v_u8 udid, double x, double y, double w, double h, int64_t phase) {
  @autoreleasepool {
    if (!(ss_caps & 2)) return 0;
    if (w <= 0 || h <= 0) return 0;
    CGPoint p = CGPointMake(x / w, y / h);   // the builder takes the point as a fraction of the screen
    // a moved finger is another down at the new place (the builder makes nothing of a drag, type 6)
    uint64_t type = phase == 2 ? 2 : 1;
    return ss_post(ss_str(udid), ss_mouse(&p, NULL, 0x32, type, CGSizeMake(1, 1), 0));
  }
}
// A key by its HID usage (page 7), down or up.
int64_t simshim_key(coax_v_u8 udid, int64_t usage, int64_t down) {
  @autoreleasepool {
    if (!(ss_caps & 4)) return 0;
    return ss_post(ss_str(udid), ss_key((uint32_t)usage, down ? 1 : 2));
  }
}
// A hardware button by its Indigo source (sim-gesture.coax names them), down or up.
int64_t simshim_button(coax_v_u8 udid, int64_t source, int64_t down) {
  @autoreleasepool {
    if (!(ss_caps & 4)) return 0;
    return ss_post(ss_str(udid), ss_button((uint32_t)source, down ? 1 : 2, 0x33));
  }
}

// Two fingers at (x1, y1) and (x2, y2) device points: phase 0 down, 1 moved, 2 up (the mouse
// builder's second point is the second finger, as Simulator.app's Option-drag pinch sends it).
int64_t simshim_touch2(coax_v_u8 udid, double x1, double y1, double x2, double y2, double w, double h, int64_t phase) {
  @autoreleasepool {
    if (!(ss_caps & 2)) return 0;
    if (w <= 0 || h <= 0) return 0;
    CGPoint a = CGPointMake(x1 / w, y1 / h), b = CGPointMake(x2 / w, y2 / h);
    uint64_t type = phase == 2 ? 2 : 1;
    return ss_post(ss_str(udid), ss_mouse(&a, &b, 0x32, type, CGSizeMake(1, 1), 0));
  }
}

// The device's orientation (UIDeviceOrientation: 1 portrait, 2 upside down, 3 landscape left, 4
// landscape right) as a GSEvent to SpringBoard's PurpleWorkspacePort, the message Simulator.app's
// Rotate sends (type 50 | host flag 0x20000, msgh_id 0x7B, 108 bytes; as idb's SimulatorPurpleHID).
// 1 when it was sent.
int64_t simshim_orient(coax_v_u8 udid, int64_t orientation) {
  @autoreleasepool {
    if (orientation < 1 || orientation > 4) return 0;
    id dev = ss_device(ss_str(udid));
    SEL look = sel_registerName("lookup:error:");
    if (!dev || ![dev respondsToSelector:look]) return 0;
    NSError *err = nil;
    mach_port_t port = ((mach_port_t(*)(id, SEL, id, NSError **))objc_msgSend)(dev, look, @"PurpleWorkspacePort", &err);
    if (port == MACH_PORT_NULL) return 0;
    uint8_t buf[112] = {0};
    uint32_t *w = (uint32_t *)buf;
    w[0] = 0x13; w[1] = 108; w[2] = port; w[5] = 0x7B;
    *(uint32_t *)(buf + 0x18) = 50 | 0x20000;
    *(uint32_t *)(buf + 0x48) = 4;
    *(uint32_t *)(buf + 0x4C) = (uint32_t)orientation;
    kern_return_t kr = mach_msg((mach_msg_header_t *)buf, MACH_SEND_MSG | MACH_SEND_TIMEOUT, 108, 0, MACH_PORT_NULL, 2000, MACH_PORT_NULL);
    return kr == KERN_SUCCESS ? 1 : 0;
  }
}

// The software keyboard: show (1) by disconnecting the simulated hardware keyboard, hide (0) by
// connecting it (SimDevice -setHardwareKeyboardEnabled:keyboardType:error:, Simulator.app's
// I/O > Keyboard). 1 when the device took it.
int64_t simshim_keyboard(coax_v_u8 udid, int64_t show) {
  @autoreleasepool {
    id dev = ss_device(ss_str(udid));
    SEL set = sel_registerName("setHardwareKeyboardEnabled:keyboardType:error:");
    if (!dev || ![dev respondsToSelector:set]) return 0;
    NSError *err = nil;
    BOOL ok = ((BOOL(*)(id, SEL, BOOL, uint8_t, NSError **))objc_msgSend)(dev, set, show ? NO : YES, 0, &err);
    return ok ? 1 : 0;
  }
}

// text on the device's pasteboard (xcrun simctl pbcopy), for a paste: how the helper types what
// the hardware keyboard has no keys for. 1 when simctl took it.
int64_t simshim_pbcopy(coax_v_u8 udid, coax_v_u8 text) {
  @autoreleasepool {
    NSTask *t = [NSTask new];
    t.executableURL = [NSURL fileURLWithPath:@"/usr/bin/xcrun"];
    t.arguments = @[@"simctl", @"pbcopy", ss_str(udid)];
    // simctl reads its input in the locale's encoding: without a UTF-8 one, "é" arrives as "√©"
    NSMutableDictionary *env = [NSProcessInfo.processInfo.environment mutableCopy];
    env[@"LANG"] = @"en_US.UTF-8"; env[@"LC_ALL"] = @"en_US.UTF-8"; t.environment = env;
    NSPipe *in = [NSPipe pipe]; t.standardInput = in;
    t.standardOutput = [NSFileHandle fileHandleWithNullDevice]; t.standardError = [NSFileHandle fileHandleWithNullDevice];
    NSError *err = nil;
    if (![t launchAndReturnError:&err]) return 0;
    [in.fileHandleForWriting writeData:[NSData dataWithBytes:text.data length:(NSUInteger)text.count]];
    [in.fileHandleForWriting closeFile];
    [t waitUntilExit];
    return t.terminationStatus == 0 ? 1 : 0;
  }
}

// ---- describe: the screen's accessibility tree (Phase 3), as idb reads it: AXPTranslator
// (macOS's AccessibilityPlatformTranslation) turns the device's accessibility into macOS
// accessibility elements, asking the device through -[SimDevice
// sendAccessibilityRequestAsync:completionQueue:completionHandler:] on each request it needs.

typedef id (^SsAxCallback)(id request);
@interface SsAxBridge : NSObject
@property (strong) id device;
@property (strong) dispatch_queue_t queue;
@end
@implementation SsAxBridge
- (SsAxCallback)accessibilityTranslationDelegateBridgeCallbackWithToken:(NSString *)token {
  id dev = self.device; dispatch_queue_t q = self.queue;
  return ^id(id request) {
    __block id answer = nil;
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    SEL send = sel_registerName("sendAccessibilityRequestAsync:completionQueue:completionHandler:");
    if (!request || ![dev respondsToSelector:send]) return nil;
    ((void(*)(id, SEL, id, dispatch_queue_t, void (^)(id)))objc_msgSend)(dev, send, request, q, ^(id r) { answer = r; dispatch_semaphore_signal(done); });
    if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) != 0) {
      Class rc = NSClassFromString(@"AXPTranslatorResponse");
      return rc ? ((id(*)(id, SEL))objc_msgSend)(rc, sel_registerName("emptyResponse")) : nil;
    }
    return answer;
  };
}
- (CGRect)accessibilityTranslationConvertPlatformFrameToSystem:(CGRect)rect withToken:(NSString *)token { return rect; }
- (id)accessibilityTranslationRootParentWithToken:(NSString *)token { return nil; }
@end

static SsAxBridge *ss_ax;
static id ss_ax_value(id el, const char *sel) { id v = ss_send0(el, sel); return v; }
static void ss_ax_put(NSMutableString *out, id el, NSString *token, int depth, int *count) {
  if (*count >= 600 || depth > 40) return;
  (*count)++;
  id tr = ss_send0(el, "translation");
  if (tr && [tr respondsToSelector:sel_registerName("setBridgeDelegateToken:")]) ((void(*)(id, SEL, id))objc_msgSend)(tr, sel_registerName("setBridgeDelegateToken:"), token);
  NSString *role = ss_ax_value(el, "accessibilityRole");
  id label = ss_ax_value(el, "accessibilityLabel"), value = ss_ax_value(el, "accessibilityValue");
  CGRect f = CGRectZero;
  if ([el respondsToSelector:sel_registerName("accessibilityFrame")]) f = ((CGRect(*)(id, SEL))objc_msgSend)(el, sel_registerName("accessibilityFrame"));
  BOOL enabled = YES;
  if ([el respondsToSelector:sel_registerName("isAccessibilityEnabled")]) enabled = ((BOOL(*)(id, SEL))objc_msgSend)(el, sel_registerName("isAccessibilityEnabled"));
  NSString *r = [role isKindOfClass:NSString.class] ? ([role hasPrefix:@"AX"] ? [role substringFromIndex:2] : role) : @"Element";
  for (int i = 0; i < depth; i++) [out appendString:@"  "];
  [out appendString:r];
  if ([label isKindOfClass:NSString.class] && [label length]) [out appendFormat:@" \"%@\"", [[label stringByReplacingOccurrencesOfString:@"\n" withString:@" "] stringByReplacingOccurrencesOfString:@"\"" withString:@"'"]];
  if (value && ![value isKindOfClass:NSNull.class]) {
    NSString *v = [value description];
    if (v.length && v.length < 200) [out appendFormat:@" = %@", [v stringByReplacingOccurrencesOfString:@"\n" withString:@" "]];
  }
  [out appendFormat:@" (%.0f, %.0f, %.0f, %.0f)%@\n", f.origin.x, f.origin.y, f.size.width, f.size.height, enabled ? @"" : @" disabled"];
  NSArray *kids = ss_ax_value(el, "accessibilityChildren");
  if ([kids isKindOfClass:NSArray.class]) for (id k in kids) ss_ax_put(out, k, token, depth + 1, count);
}

// The frontmost app's accessibility tree into out, one element a line, indented by depth:
// `Role "label" = value (x, y, w, h)` in device points. The bytes written; -1 when the
// accessibility translation is not there, -2 when the device did not answer.
int64_t simshim_describe(coax_v_u8 udid, coax_v_u8 out) {
  @autoreleasepool {
    id dev = ss_device(ss_str(udid));
    if (!dev || ![dev respondsToSelector:sel_registerName("sendAccessibilityRequestAsync:completionQueue:completionHandler:")]) return -1;
    if (!dlopen("/System/Library/PrivateFrameworks/AccessibilityPlatformTranslation.framework/AccessibilityPlatformTranslation", RTLD_NOW)) return -1;
    Class tc = NSClassFromString(@"AXPTranslator");
    id tr = tc ? ((id(*)(id, SEL))objc_msgSend)(tc, sel_registerName("sharedInstance")) : nil;
    SEL front = sel_registerName("frontmostApplicationWithDisplayId:bridgeDelegateToken:");
    SEL mac = sel_registerName("macPlatformElementFromTranslation:");
    if (!tr || ![tr respondsToSelector:front] || ![tr respondsToSelector:mac]) return -1;
    if (!ss_ax) { ss_ax = [SsAxBridge new]; ss_ax.queue = dispatch_queue_create("oasim.ax", DISPATCH_QUEUE_SERIAL); }
    ss_ax.device = dev;
    ((void(*)(id, SEL, id))objc_msgSend)(tr, sel_registerName("setBridgeTokenDelegate:"), ss_ax);
    if ([tr respondsToSelector:sel_registerName("setSupportsDelegateTokens:")]) ((void(*)(id, SEL, BOOL))objc_msgSend)(tr, sel_registerName("setSupportsDelegateTokens:"), YES);
    if ([tr respondsToSelector:sel_registerName("setAccessibilityEnabled:")]) ((void(*)(id, SEL, BOOL))objc_msgSend)(tr, sel_registerName("setAccessibilityEnabled:"), YES);
    NSString *token = [[NSUUID UUID] UUIDString];
    id t = ((id(*)(id, SEL, uint32_t, id))objc_msgSend)(tr, front, 0, token);
    if (!t) { usleep(400000); t = ((id(*)(id, SEL, uint32_t, id))objc_msgSend)(tr, front, 0, token); }   // an app coming up answers a moment later
    if (!t) return -2;
    if ([t respondsToSelector:sel_registerName("setBridgeDelegateToken:")]) ((void(*)(id, SEL, id))objc_msgSend)(t, sel_registerName("setBridgeDelegateToken:"), token);
    id el = ((id(*)(id, SEL, id))objc_msgSend)(tr, mac, t);
    if (!el) return -2;
    NSMutableString *s = [NSMutableString string]; int count = 0;
    ss_ax_put(s, el, token, 0, &count);
    NSData *d = [s dataUsingEncoding:NSUTF8StringEncoding];
    NSUInteger n = MIN(d.length, (NSUInteger)out.count);
    while (n > 0 && n < d.length && (((const uint8_t *)d.bytes)[n] & 0xC0) == 0x80) n--;   // whole characters
    memcpy(out.data, d.bytes, n);
    return (int64_t)n;
  }
}
