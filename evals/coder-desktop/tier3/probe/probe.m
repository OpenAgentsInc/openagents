// Firm reason: 6 (AppKit test seam: a dylib the Tier 3 runner loads into a scratch Coder.app with DYLD_INSERT_LIBRARIES)
//
// Tier 3 probe for Coder Desktop. The runner (evals/coder-desktop/tier3/run_ui.py) starts a scratch
// build of Coder.app with DYLD_INSERT_LIBRARIES pointing here and CODER_T3_SOCKET naming a Unix
// socket. The app binary is the one build.sh makes, unchanged: this file only adds a socket that
// drives it from inside its own process.
//
// What it does:
//  - Keeps the app from taking focus: NSApplication's activate calls are no-ops, the menu-bar item
//    is hidden, and the window opens parked behind every window with one point on screen
//    (NSWindow -center is replaced; frames are not pulled back on screen).
//  - Serves one request line, one reply line (JSON) on CODER_T3_SOCKET, on the main queue:
//      ping                       {"ok":true}
//      window                     the window's number, frame, content size and backing scale
//      size W H                   content size W by H points, parked as at launch
//      click X Y [N]              mouse down and up at content point X,Y (top-left), click count N
//      move X Y                   mouse moved to X,Y
//      drag X1 Y1 X2 Y2 [STEPS]   mouse down at 1, dragged in STEPS to 2, up
//      scroll X Y DY              a precise scroll of DY points at X,Y
//      type TEXT                  key down and up for each character, to the first responder
//      key NAME                   one key with modifiers: return, escape, tab, delete, up, down,
//                                 left, right, or a character; cmd+, shift+, alt+, ctrl+ prefixes.
//                                 Keys with Command go to the main menu first.
//      menu TITLE                 performs the main-menu item with that title
//      state                      read-only page state: running, settings page, cost, title, editor box,
//                                 the kept sidebar widths, whether a sheet is up
//      composer                   the composer's text and whether it has focus
//      box ID                     the drawn box of the page's row with that id (content points), to click
//                                 what a person sees: {"ok":true,"x":..,"y":..,"w":..,"h":..}
//      followups                  the follow-ups as the page holds them (#13718): the queue's texts,
//                                 whether it is paused and why, each steer's status, the worker's turn
//      views                      the native views over the page (3D pane, live card, Gym): class, frame,
//                                 shown, masked (a page menu cut out of it)
//      hit X Y                    the class of the view a press at X,Y would reach
//      quit                       NSApp terminate
// Events are real NSEvents: keys go through the window (NSWindow sendEvent:, Command keys through
// the main menu first), mouse events to the view under the point. Nothing moves the system
// cursor and nothing goes to another app.
#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#include <dlfcn.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/stat.h>
#include <unistd.h>

static int t3_listen = -1;
static dispatch_source_t t3_accept_src;

// ---- keep focus where it is ----------------------------------------------------------------
static void t3_noop_activate(id self, SEL _cmd, BOOL flag) { (void)self; (void)_cmd; (void)flag; }
static void t3_noop_activate0(id self, SEL _cmd) { (void)self; (void)_cmd; }
// WindowServer captures a window only while some of it is on a screen, so the window is parked at
// the main screen's bottom-right corner with one point showing, behind every other window
// (desktop level). Its whole backing store is still drawn and captured.
static NSPoint t3_parked_origin(NSSize frame) {
  NSRect s = NSScreen.screens.firstObject.frame;
  return NSMakePoint(NSMaxX(s) - 1, NSMinY(s) - frame.height + 1);
}
static void t3_park(NSWindow *w) {
  w.level = kCGDesktopWindowLevel;
  [w setFrameOrigin:t3_parked_origin(w.frame.size)];
}
static void t3_center(id self, SEL _cmd) { (void)_cmd; t3_park((NSWindow *)self); }
static NSRect t3_constrain(id self, SEL _cmd, NSRect r, NSScreen *s) { (void)self; (void)_cmd; (void)s; return r; }   // off-screen frames stay off screen
static IMP t3_status_orig;
static id t3_status_item(id self, SEL _cmd, CGFloat len) {
  NSStatusItem *item = ((id (*)(id, SEL, CGFloat))t3_status_orig)(self, _cmd, len);
  item.visible = NO;
  return item;
}

// ---- helpers --------------------------------------------------------------------------------
static NSWindow *t3_window(void) {
  for (NSWindow *w in NSApp.windows) if ([w.contentView isKindOfClass:NSClassFromString(@"UiView")] || [w.title isEqualToString:@"Coder"]) return w;
  return NSApp.windows.firstObject;
}
static NSView *t3_canvas(void) { return t3_window().contentView; }
static NSString *t3_json(id obj) {
  NSData *d = [NSJSONSerialization dataWithJSONObject:obj options:0 error:nil];
  return d ? [[NSString alloc] initWithData:d encoding:NSUTF8StringEncoding] : @"{\"ok\":false,\"error\":\"json\"}";
}
static NSString *t3_err(NSString *why) { return t3_json(@{@"ok": @NO, @"error": why}); }
static NSPoint t3_window_point(CGFloat x, CGFloat y) {   // content top-left points -> window base coordinates
  NSView *v = t3_canvas();
  return [v convertPoint:NSMakePoint(x, v.bounds.size.height - y) toView:nil];
}
// ---- the native views over the page (the 3D pane, a live 3D card, the Gym; #13817): class, frame in
// content top-left points, shown, and whether a layer mask cuts it (the page's menus over it)
static void t3_views_into(NSView *v, NSMutableArray *out) {
  for (NSView *c in v.subviews) {
    NSString *name = NSStringFromClass(c.class);
    if ([name containsString:@"Ple"] || [name containsString:@"PassThrough"] || [name containsString:@"Gym"]) {
      NSView *content = t3_window().contentView;
      NSRect r = [c convertRect:c.bounds toView:content];
      CGFloat top = content.bounds.size.height - NSMaxY(r);
      [out addObject:@{@"class": name, @"x": @(r.origin.x), @"y": @(top), @"w": @(r.size.width), @"h": @(r.size.height),
                       @"shown": @(!c.isHiddenOrHasHiddenAncestor), @"masked": @(c.layer.mask != nil)}];
    }
    t3_views_into(c, out);
  }
}
static NSString *t3_hit(CGFloat x, CGFloat y) {
  NSView *content = t3_window().contentView;
  NSPoint wp = t3_window_point(x, y);
  NSView *hit = [content hitTest:[content.superview convertPoint:wp fromView:nil]];
  return t3_json(@{@"ok": @YES, @"class": hit ? NSStringFromClass(hit.class) : @""});
}
static NSInteger t3_event_number = 1;
static void t3_mouse(NSEventType type, CGFloat x, CGFloat y, NSInteger clicks) {
  NSWindow *w = t3_window();
  NSEvent *e = [NSEvent mouseEventWithType:type location:t3_window_point(x, y) modifierFlags:0 timestamp:NSProcessInfo.processInfo.systemUptime
                              windowNumber:w.windowNumber context:nil eventNumber:t3_event_number++ clickCount:clicks pressure:(type == NSEventTypeLeftMouseUp ? 0 : 1)];
  // To the view under the point, as NSWindow does for its key window: a window behind an inactive
  // app's would take the first click to come forward instead (acceptsFirstMouse). A drag and its
  // release go to the view the press went to.
  static NSView *pressed;
  NSView *target = pressed;
  if (type == NSEventTypeLeftMouseDown || type == NSEventTypeMouseMoved || !target) {
    NSView *content = w.contentView;
    target = [content hitTest:[content.superview convertPoint:e.locationInWindow fromView:nil]] ?: content;
  }
  switch (type) {
    case NSEventTypeLeftMouseDown: pressed = target; [target mouseDown:e]; break;
    case NSEventTypeLeftMouseDragged: [target mouseDragged:e]; break;
    case NSEventTypeLeftMouseUp: [target mouseUp:e]; pressed = nil; break;
    default: [target mouseMoved:e]; break;
  }
}
static BOOL t3_key_event(NSString *chars, NSString *bare, unsigned short code, NSEventModifierFlags mods) {
  NSWindow *w = t3_window();
  NSTimeInterval t = NSProcessInfo.processInfo.systemUptime;
  NSEvent *down = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:mods timestamp:t windowNumber:w.windowNumber
                                    context:nil characters:chars charactersIgnoringModifiers:bare isARepeat:NO keyCode:code];
  NSEvent *up = [NSEvent keyEventWithType:NSEventTypeKeyUp location:NSZeroPoint modifierFlags:mods timestamp:t windowNumber:w.windowNumber
                                  context:nil characters:chars charactersIgnoringModifiers:bare isARepeat:NO keyCode:code];
  BOOL handled = NO;
  if (mods & NSEventModifierFlagCommand) handled = [NSApp.mainMenu performKeyEquivalent:down];
  if (!handled) { [w sendEvent:down]; handled = YES; }
  [w sendEvent:up];
  return handled;
}
static NSString *t3_key(NSString *spec) {
  NSEventModifierFlags mods = 0;
  NSString *rest = spec;
  NSDictionary *prefixes = @{@"cmd+": @(NSEventModifierFlagCommand), @"shift+": @(NSEventModifierFlagShift),
                             @"alt+": @(NSEventModifierFlagOption), @"ctrl+": @(NSEventModifierFlagControl)};
  BOOL more = YES;
  while (more && rest.length > 1) {
    more = NO;
    for (NSString *p in prefixes) if ([rest hasPrefix:p] && rest.length > p.length) { mods |= [prefixes[p] unsignedLongValue]; rest = [rest substringFromIndex:p.length]; more = YES; }
  }
  NSDictionary *named = @{@"return": @[@"\r", @36], @"escape": @[@"\x1b", @53], @"tab": @[@"\t", @48], @"delete": @[@"\x7f", @51],
                          @"up": @[[NSString stringWithFormat:@"%C", (unichar)NSUpArrowFunctionKey], @126],
                          @"down": @[[NSString stringWithFormat:@"%C", (unichar)NSDownArrowFunctionKey], @125],
                          @"left": @[[NSString stringWithFormat:@"%C", (unichar)NSLeftArrowFunctionKey], @123],
                          @"right": @[[NSString stringWithFormat:@"%C", (unichar)NSRightArrowFunctionKey], @124],
                          @"space": @[@" ", @49]};
  NSArray *n = named[rest];
  NSString *chars = n ? n[0] : rest;
  unsigned short code = n ? [n[1] unsignedShortValue] : 0;
  BOOL ok = t3_key_event(chars, chars, code, mods);
  return t3_json(@{@"ok": @(ok)});
}
static NSMenuItem *t3_menu_item(NSMenu *menu, NSString *title) {
  for (NSMenuItem *it in menu.itemArray) {
    if ([it.title isEqualToString:title]) return it;
    if (it.submenu) { NSMenuItem *found = t3_menu_item(it.submenu, title); if (found) return found; }
  }
  return nil;
}

// The compiled page behind the window (CoderWindow's `page`), for read-only state (host.h's cd_ calls).
static void *t3_page(void) {
  id owner = t3_window().delegate;
  if (!owner) return NULL;
  Ivar iv = class_getInstanceVariable(object_getClass(owner), "page");
  if (!iv) return NULL;
  return *(void **)((char *)(__bridge void *)owner + ivar_getOffset(iv));
}
static NSString *t3_state(void) {
  void *page = t3_page();
  if (!page) return t3_err(@"no page");
  int (*running)(void *) = dlsym(RTLD_DEFAULT, "cd_running");
  int (*settings)(void *) = dlsym(RTLD_DEFAULT, "cd_settings_page");
  int64_t (*cost)(void *) = dlsym(RTLD_DEFAULT, "cd_cost_usd_micros");
  int (*title)(void *, char *, int) = dlsym(RTLD_DEFAULT, "cd_title");
  int (*rowbox)(void *, const char *, float *) = dlsym(RTLD_DEFAULT, "cd_box");
  if (!running || !settings || !cost || !title || !rowbox) return t3_err(@"cd_ symbols missing");
  char t[512] = {0}; int tn = title(page, t, (int)sizeof t - 1); if (tn < 0) tn = 0; t[tn] = 0;
  float box[4] = {0}; int has_box = rowbox(page, "coder.composer", box);   // the composer (the page draws its text, #13742)
  NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
  return t3_json(@{@"ok": @YES, @"running": @(running(page)), @"settings_page": @(settings(page)), @"cost_usd_micros": @(cost(page)),
                   @"title": [NSString stringWithUTF8String:t] ?: @"", @"side_width": @([d floatForKey:@"sideWidth"]), @"pane_width": @([d floatForKey:@"paneWidth"]),
                   @"editor": has_box ? @[@(box[0]), @(box[1]), @(box[2]), @(box[3])] : @[], @"windows": @(NSApp.windows.count),
                   @"sheet": @(t3_window().attachedSheet != nil)});
}

static NSString *t3_followups(void) {
  void *page = t3_page();
  int (*count)(void *) = dlsym(RTLD_DEFAULT, "cd_queue_count");
  int (*paused)(void *) = dlsym(RTLD_DEFAULT, "cd_queue_paused");
  int (*reason)(void *, char *, int) = dlsym(RTLD_DEFAULT, "cd_queue_reason");
  int (*item)(void *, int, char *, int) = dlsym(RTLD_DEFAULT, "cd_queue_item_text");
  int (*steers)(void *) = dlsym(RTLD_DEFAULT, "cd_steer_count");
  int (*status)(void *, int) = dlsym(RTLD_DEFAULT, "cd_steer_status");
  int (*turn)(void *) = dlsym(RTLD_DEFAULT, "cd_worker_turn");
  if (!page || !count || !paused || !reason || !item || !steers || !status || !turn) return t3_err(@"no page or follow-up cd_ symbols");
  char b[512];
  NSMutableArray *items = [NSMutableArray array];
  for (int i = 0; i < count(page); i++) { int n = item(page, i, b, (int)sizeof b - 1); b[n < 0 ? 0 : n] = 0; [items addObject:[NSString stringWithUTF8String:b] ?: @""]; }
  NSMutableArray *st = [NSMutableArray array];
  for (int i = 0; i < steers(page); i++) [st addObject:@(status(page, i))];
  int n = reason(page, b, (int)sizeof b - 1); b[n < 0 ? 0 : n] = 0;
  return t3_json(@{@"ok": @YES, @"queue": items, @"paused": @(paused(page)), @"reason": [NSString stringWithUTF8String:b] ?: @"", @"steers": st, @"turn": @(turn(page))});
}

// ---- requests -------------------------------------------------------------------------------
static NSString *t3_handle(NSString *line) {
  // the driver spoke: Coder's test-launch watchdog (TestLaunch, AppDelegate.swift) starts over
  [NSNotificationCenter.defaultCenter postNotificationName:@"CoderTestDriverActivity" object:nil];
  NSArray *parts = [line componentsSeparatedByString:@" "];
  NSString *op = parts.firstObject ?: @"";
  NSString *rest = parts.count > 1 ? [line substringFromIndex:op.length + 1] : @"";
  NSWindow *w = t3_window();
  if (!w && ![op isEqualToString:@"ping"]) return t3_err(@"no window");
  if ([op isEqualToString:@"ping"]) return t3_json(@{@"ok": @YES, @"window": @(w != nil)});
  if ([op isEqualToString:@"window"]) {
    NSRect f = w.frame, c = [w contentRectForFrameRect:f];
    return t3_json(@{@"ok": @YES, @"id": @(w.windowNumber), @"x": @(f.origin.x), @"y": @(f.origin.y), @"w": @(f.size.width), @"h": @(f.size.height),
                     @"content_w": @(c.size.width), @"content_h": @(c.size.height), @"scale": @(w.backingScaleFactor), @"key": @(w.isKeyWindow),
                     @"visible": @(w.isVisible), @"active": @(NSApp.isActive)});
  }
  if ([op isEqualToString:@"size"] && parts.count >= 3) {
    NSRect frame = [w frameRectForContentRect:NSMakeRect(0, 0, [parts[1] doubleValue], [parts[2] doubleValue])];
    frame.origin = t3_parked_origin(frame.size);
    [w setFrame:frame display:YES];
    w.level = kCGDesktopWindowLevel;
    return t3_json(@{@"ok": @YES});
  }
  if ([op isEqualToString:@"state"]) return t3_state();
  if ([op isEqualToString:@"views"]) { NSMutableArray *a = [NSMutableArray array]; t3_views_into(t3_window().contentView.superview ?: t3_window().contentView, a); return t3_json(@{@"ok": @YES, @"views": a}); }
  if ([op isEqualToString:@"hit"] && parts.count >= 3) return t3_hit([parts[1] doubleValue], [parts[2] doubleValue]);
  if ([op isEqualToString:@"box"] && rest.length) {
    void *page = t3_page();
    int (*box)(void *, const char *, float *) = dlsym(RTLD_DEFAULT, "cd_box");
    if (!page || !box) return t3_err(@"no page or cd_box");
    float b[4] = {0};
    if (!box(page, rest.UTF8String, b)) return t3_err([NSString stringWithFormat:@"no row %@ shows", rest]);
    return t3_json(@{@"ok": @YES, @"x": @(b[0]), @"y": @(b[1]), @"w": @(b[2]), @"h": @(b[3])});
  }
  if ([op isEqualToString:@"followups"]) return t3_followups();
  if ([op isEqualToString:@"click"] && parts.count >= 3) {
    CGFloat x = [parts[1] doubleValue], y = [parts[2] doubleValue];
    NSInteger n = parts.count >= 4 ? [parts[3] integerValue] : 1;
    for (NSInteger i = 1; i <= n; i++) { t3_mouse(NSEventTypeLeftMouseDown, x, y, i); t3_mouse(NSEventTypeLeftMouseUp, x, y, i); }
    return t3_json(@{@"ok": @YES});
  }
  if ([op isEqualToString:@"move"] && parts.count >= 3) { t3_mouse(NSEventTypeMouseMoved, [parts[1] doubleValue], [parts[2] doubleValue], 0); return t3_json(@{@"ok": @YES}); }
  if ([op isEqualToString:@"drag"] && parts.count >= 5) {
    CGFloat x1 = [parts[1] doubleValue], y1 = [parts[2] doubleValue], x2 = [parts[3] doubleValue], y2 = [parts[4] doubleValue];
    int steps = parts.count >= 6 ? MAX(1, [parts[5] intValue]) : 12;
    t3_mouse(NSEventTypeMouseMoved, x1, y1, 0);
    t3_mouse(NSEventTypeLeftMouseDown, x1, y1, 1);
    for (int i = 1; i <= steps; i++) t3_mouse(NSEventTypeLeftMouseDragged, x1 + (x2 - x1) * i / steps, y1 + (y2 - y1) * i / steps, 1);
    t3_mouse(NSEventTypeLeftMouseUp, x2, y2, 1);
    return t3_json(@{@"ok": @YES});
  }
  if ([op isEqualToString:@"scroll"] && parts.count >= 4) {
    CGEventRef ce = CGEventCreateScrollWheelEvent(NULL, kCGScrollEventUnitPixel, 1, (int32_t)[parts[3] intValue]);
    NSPoint wp = t3_window_point([parts[1] doubleValue], [parts[2] doubleValue]);
    NSPoint sp = [w convertPointToScreen:wp];
    CGFloat top = NSMaxY(NSScreen.screens.firstObject.frame);
    CGEventSetLocation(ce, CGPointMake(sp.x, top - sp.y));
    NSEvent *e = [NSEvent eventWithCGEvent:ce];
    CFRelease(ce);
    [w sendEvent:e];
    return t3_json(@{@"ok": @YES});
  }
  if ([op isEqualToString:@"type"]) {
    for (NSUInteger i = 0; i < rest.length; i++) {
      NSString *ch = [rest substringWithRange:NSMakeRange(i, 1)];
      t3_key_event(ch, ch, 0, 0);
    }
    return t3_json(@{@"ok": @YES});
  }
  if ([op isEqualToString:@"key"] && rest.length) return t3_key(rest);
  if ([op isEqualToString:@"menu"] && rest.length) {
    NSMenuItem *it = t3_menu_item(NSApp.mainMenu, rest);
    if (!it) return t3_err([NSString stringWithFormat:@"no menu item %@", rest]);
    NSInteger idx = [it.menu indexOfItem:it];
    [it.menu performActionForItemAtIndex:idx];
    return t3_json(@{@"ok": @YES});
  }
  if ([op isEqualToString:@"composer"]) {
    // the page draws and edits the composer (#13742): its draft, and whether the page's view has the keyboard for it
    void *page = t3_page();
    int (*draft)(void *, char *, int) = dlsym(RTLD_DEFAULT, "cd_draft");
    int (*wants)(void *) = dlsym(RTLD_DEFAULT, "cd_wants_text");
    int (*settings)(void *) = dlsym(RTLD_DEFAULT, "cd_settings_page");
    int (*rowbox)(void *, const char *, float *) = dlsym(RTLD_DEFAULT, "cd_box");
    if (!page || !draft || !wants || !settings || !rowbox) return t3_err(@"no composer");
    static char text[16385]; int n = draft(page, text, (int)sizeof text); text[n < 0 ? 0 : n] = 0;
    float box[4] = {0}; int shown = rowbox(page, "coder.composer", box);
    BOOL focused = w.firstResponder == (id)t3_canvas() && wants(page) && settings(page) < 0;
    return t3_json(@{@"ok": @YES, @"text": [NSString stringWithUTF8String:text] ?: @"", @"focused": @(focused), @"hidden": @(!shown)});
  }
  if ([op isEqualToString:@"open"] && rest.length) {
    // a file opened with Coder, as the Finder or `open` hands it over (application:openURLs:, #13759)
    id<NSApplicationDelegate> d = NSApp.delegate;
    if (![d respondsToSelector:@selector(application:openURLs:)]) return t3_err(@"no application:openURLs:");
    [d application:NSApp openURLs:@[[NSURL fileURLWithPath:rest]]];
    return t3_json(@{@"ok": @YES});
  }
  if ([op isEqualToString:@"file"]) {
    // the file in the pane (page/filepane.coax, #13760): what it holds, its changes, its editor's text
    void *page = t3_page();
    int (*mode)(void *) = dlsym(RTLD_DEFAULT, "cd_file_mode");
    int (*dirty)(void *) = dlsym(RTLD_DEFAULT, "cd_file_dirty");
    int (*text)(void *, char *, int) = dlsym(RTLD_DEFAULT, "cd_file_text");
    int (*name)(void *, char *, int) = dlsym(RTLD_DEFAULT, "cd_file_name");
    if (!page || !mode || !dirty || !text || !name) return t3_err(@"no file cd_ symbols");
    static char body[16385]; int n = text(page, body, (int)sizeof body - 1); body[n < 0 ? 0 : n] = 0;
    char nb[1024]; int k = name(page, nb, (int)sizeof nb); nb[k < 0 ? 0 : k] = 0;
    return t3_json(@{@"ok": @YES, @"mode": @(mode(page)), @"dirty": @(dirty(page)), @"name": [NSString stringWithUTF8String:nb] ?: @"",
                     @"text": [NSString stringWithUTF8String:body] ?: @""});
  }
  if ([op isEqualToString:@"quit"]) {
    dispatch_async(dispatch_get_main_queue(), ^{ [NSApp terminate:nil]; });
    return t3_json(@{@"ok": @YES});
  }
  return t3_err([NSString stringWithFormat:@"unknown op %@", op]);
}

static void t3_serve_client(int fd) {
  __block NSMutableData *buf = [NSMutableData data];
  dispatch_source_t src = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, (uintptr_t)fd, 0, dispatch_get_main_queue());
  dispatch_source_set_event_handler(src, ^{
    char tmp[8192];
    ssize_t n = read(fd, tmp, sizeof tmp);
    if (n <= 0) { dispatch_source_cancel(src); return; }
    [buf appendBytes:tmp length:(NSUInteger)n];
    while (1) {
      const char *bytes = buf.bytes;
      const char *nl = memchr(bytes, '\n', buf.length);
      if (!nl) break;
      NSUInteger len = (NSUInteger)(nl - bytes);
      NSString *line = [[NSString alloc] initWithBytes:bytes length:len encoding:NSUTF8StringEncoding] ?: @"";
      [buf replaceBytesInRange:NSMakeRange(0, len + 1) withBytes:NULL length:0];
      NSString *reply;
      @try { reply = t3_handle(line); } @catch (NSException *x) { reply = t3_err(x.reason ?: @"exception"); }
      NSData *out = [[reply stringByAppendingString:@"\n"] dataUsingEncoding:NSUTF8StringEncoding];
      const char *p = out.bytes; size_t left = out.length;
      while (left > 0) { ssize_t k = write(fd, p, left); if (k <= 0) break; p += k; left -= (size_t)k; }
    }
  });
  dispatch_source_set_cancel_handler(src, ^{ close(fd); });
  dispatch_resume(src);
}

static void t3_start_socket(const char *path) {
  t3_listen = socket(AF_UNIX, SOCK_STREAM, 0);
  if (t3_listen < 0) return;
  struct sockaddr_un addr = {0};
  addr.sun_family = AF_UNIX;
  strlcpy(addr.sun_path, path, sizeof addr.sun_path);
  unlink(path);
  mode_t old = umask(0177);
  int ok = bind(t3_listen, (struct sockaddr *)&addr, sizeof addr) == 0 && listen(t3_listen, 4) == 0;
  umask(old);
  if (!ok) { close(t3_listen); t3_listen = -1; return; }
  t3_accept_src = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, (uintptr_t)t3_listen, 0, dispatch_get_main_queue());
  dispatch_source_set_event_handler(t3_accept_src, ^{
    int fd = accept(t3_listen, NULL, NULL);
    if (fd >= 0) t3_serve_client(fd);
  });
  dispatch_resume(t3_accept_src);
}

__attribute__((constructor)) static void t3_init(void) {
  const char *path = getenv("CODER_T3_SOCKET");
  if (!path || !*path) return;
  NSString *p = [NSString stringWithUTF8String:path];
  // The workers Coder starts inherit its environment: they get neither the probe nor its socket.
  unsetenv("DYLD_INSERT_LIBRARIES"); unsetenv("CODER_T3_SOCKET");
  // a test launch (#13752): Coder stops the coder-host it starts when it quits, and quits itself when
  // this driver goes quiet for CODER_TEST_IDLE_S seconds
  setenv("CODER_TEST_LAUNCH", "tier3", 0);
  Class app = NSApplication.class;
  Method m1 = class_getInstanceMethod(app, @selector(activateIgnoringOtherApps:));
  if (m1) method_setImplementation(m1, (IMP)t3_noop_activate);
  Method m2 = class_getInstanceMethod(app, NSSelectorFromString(@"activate"));
  if (m2) method_setImplementation(m2, (IMP)t3_noop_activate0);
  Method m3 = class_getInstanceMethod(NSWindow.class, @selector(center));
  if (m3) method_setImplementation(m3, (IMP)t3_center);
  Method m5 = class_getInstanceMethod(NSWindow.class, @selector(constrainFrameRect:toScreen:));
  if (m5) method_setImplementation(m5, (IMP)t3_constrain);
  Method m4 = class_getInstanceMethod(NSStatusBar.class, @selector(statusItemWithLength:));
  if (m4) t3_status_orig = method_setImplementation(m4, (IMP)t3_status_item);
  dispatch_async(dispatch_get_main_queue(), ^{ t3_start_socket(p.fileSystemRepresentation); });
}
