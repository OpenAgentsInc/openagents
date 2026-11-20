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
#import <Foundation/Foundation.h>
#import <IOSurface/IOSurface.h>
#import <CoreGraphics/CoreGraphics.h>
#import <ImageIO/ImageIO.h>
#import <objc/message.h>
#include <dlfcn.h>
#include <stdint.h>

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

// The screen as a PNG at path, w by h pixels (scaled from the framebuffer). Its bytes, or -1.
int64_t simshim_shot(coax_v_u8 udid, coax_v_u8 path, int64_t w, int64_t h) {
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
    if ((size_t)w != sw || (size_t)h != sh) {
      CGContextRef dst = CGBitmapContextCreate(NULL, (size_t)w, (size_t)h, 8, 0, cs, kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little);
      CGContextSetInterpolationQuality(dst, kCGInterpolationHigh);
      CGContextDrawImage(dst, CGRectMake(0, 0, w, h), full);
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
