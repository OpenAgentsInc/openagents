// Firm reason: 6 (AppKit's Accessibility API; the check of the shared Mac host's VoiceOver elements, #13180).
// A stand-in app on the shared Mac host (ui-host.m) in a window that is never shown: its element table
// has a button, a disabled button, a field, a tab and a message. The check reads the window's
// Accessibility tree the way VoiceOver does (accessibilityChildren from the NSWindow down), presses
// an element by its id (AXPress), moves focus with Tab and Shift-Tab, presses with Space and Enter,
// and does the same through the host's drive operations (ax, ax-press ID, ax-key KEY), offscreen too.
// No permission is needed: it reads its own process's tree, not another's.
//   apps/shared/host/apple/a11y-check.sh
#import <AppKit/AppKit.h>
#include "ui-host-apple.h"
#include <stdio.h>
#include <string.h>

static char taps[1024];   // every `tap ID` the app was asked, space separated
static char draft[64] = "hello";
static int fails;
static void check(int ok, const char *what) { printf("%s: %s\n", ok ? "ok" : "FAIL", what); if (!ok) fails++; }

static int frame(void *u, float *v, float w, float h, float dpr, float dt) { (void)u; (void)v; (void)w; (void)h; (void)dpr; (void)dt; return 0; }
static int drive(void *u, const char *op, char *out, int cap) {
  (void)u;
  if (!strcmp(op, "tree"))
    return snprintf(out, (size_t)cap,
                    "setup.open button \"Open Everglade\" at 400 600 size 164x36\n"
                    "setup.verify button \"Verify voice model\" at 400 200 size 180x30\n"
                    "composer field \"Do anything\" at 10 700 size 300x60\n"
                    "tab.1 tab \"Chat\" at 10 10 size 80x24\n"
                    "message.1.0 item \"Hello there\" at 10 100 size 300x40\n");
  if (!strncmp(op, "tap ", 4)) {
    if (!strstr("setup.open setup.verify composer tab.1 message.1.0", op + 4)) return snprintf(out, (size_t)cap, "no row %s in the tree\n", op + 4);
    strncat(taps, op + 4, sizeof taps - strlen(taps) - 2); strcat(taps, " ");
    return 0;
  }
  if (!strcmp(op, "about setup.verify")) return snprintf(out, (size_t)cap, "enabled 0\n");
  if (!strcmp(op, "about composer")) return snprintf(out, (size_t)cap, "value %s\n", draft);
  return -1;
}
static int key(void *u, int k, int mods, int down) { (void)u; (void)k; (void)mods; (void)down; return 0; }   // takes no key
static int has(const char *s, const char *part) { return strstr(s, part) != NULL; }
static void say(UiHost *h, const char *op, char *out) { int n = ui_drive(h, op, out, 8192); out[n > 0 ? n : 0] = 0; }

static void run(UiHost *h, int windowed) {
  char out[8192];
  const char *where = windowed ? "window" : "offscreen";
  char what[256];
  if (windowed) {
    // VoiceOver's own walk: the window, its content view, the host's view, the elements
    NSWindow *win = (__bridge NSWindow *)ui_window(h);
    NSMutableArray *found = [NSMutableArray array], *todo = [NSMutableArray arrayWithObject:win];
    while (todo.count) {
      id n = todo.lastObject; [todo removeLastObject];
      if ([n respondsToSelector:@selector(accessibilityIdentifier)] && [n accessibilityIdentifier].length && [n isKindOfClass:NSAccessibilityElement.class]) [found addObject:n];
      for (id k in ([n respondsToSelector:@selector(accessibilityChildren)] ? [n accessibilityChildren] : nil) ?: @[]) [todo addObject:k];
    }
    check(found.count == 5, "the window's Accessibility tree reaches the five rows");
    id open = nil, verify = nil, field = nil, tab = nil, item = nil;
    for (id e in found) {
      NSString *i = [e accessibilityIdentifier];
      if ([i isEqualToString:@"setup.open"]) open = e; else if ([i isEqualToString:@"setup.verify"]) verify = e;
      else if ([i isEqualToString:@"composer"]) field = e; else if ([i isEqualToString:@"tab.1"]) tab = e; else if ([i isEqualToString:@"message.1.0"]) item = e;
    }
    check([[open accessibilityRole] isEqualToString:NSAccessibilityButtonRole] && [[open accessibilityLabel] isEqualToString:@"Open Everglade"], "a button's role and label");
    check(![verify isAccessibilityEnabled], "a disabled button says so");
    check([[field accessibilityRole] isEqualToString:NSAccessibilityTextFieldRole] && [[field accessibilityValue] isEqual:@"hello"], "a field's role and value");
    check([[tab accessibilitySubrole] isEqualToString:NSAccessibilityTabButtonSubrole], "a tab is a tab button");
    check([[item accessibilityRole] isEqualToString:NSAccessibilityStaticTextRole] && [[item accessibilityValue] isEqual:@"Hello there"] && ![item accessibilityLabel], "a message is text, read once as its value");
    NSRect f = [open accessibilityFrame], wf = win.frame, cf = [win contentRectForFrameRect:wf];
    check(fabs(f.origin.x - (cf.origin.x + 400)) < 1 && fabs(f.origin.y - (cf.origin.y + cf.size.height - 636)) < 1 && f.size.width == 164, "a button's frame is on the screen where it is drawn");
    check([open accessibilityParent] == (__bridge id)ui_view(h), "an element's parent is the host's view");
    taps[0] = 0;
    check([open accessibilityPerformPress] && !strcmp(taps, "setup.open "), "AXPress on setup.open taps it");
    taps[0] = 0;
    check(![verify accessibilityPerformPress] && !taps[0], "AXPress on a disabled button does nothing");
  }
  say(h, "ax", out);
  snprintf(what, sizeof what, "%s: ax lists every row with its role, label, value and state", where);
  check(has(out, "setup.open AXButton \"Open Everglade\"") && has(out, "setup.verify AXButton \"Verify voice model\" disabled") &&
        has(out, "composer AXTextField \"Do anything\" value \"hello\"") && has(out, "tab.1 AXRadioButton/AXTabButton \"Chat\"") &&
        has(out, "message.1.0 AXStaticText \"\" value \"Hello there\""), what);
  if (fails) printf("%s", out);
  taps[0] = 0; say(h, "ax-press setup.open", out);
  snprintf(what, sizeof what, "%s: ax-press setup.open presses it", where);
  check(has(out, "pressed setup.open") && !strcmp(taps, "setup.open "), what);
  say(h, "ax-press nothing.here", out);
  snprintf(what, sizeof what, "%s: ax-press of an id that is not drawn says so", where);
  check(has(out, "no element nothing.here"), what);
  // Tab: setup.open, (setup.verify is disabled), composer (tapped: it gets the keyboard), tab.1, around again
  taps[0] = 0;
  say(h, "ax-key tab", out); int a = has(out, "focus setup.open");
  say(h, "ax-key tab", out); int b = has(out, "focus composer");
  say(h, "ax-key tab", out); int c = has(out, "focus tab.1");
  say(h, "ax-key tab", out); int d = has(out, "focus setup.open");
  say(h, "ax-key shift-tab", out); int e = has(out, "focus tab.1");
  snprintf(what, sizeof what, "%s: Tab and Shift-Tab move through what can be pressed, past a disabled button", where);
  check(a && b && c && d && e && !strcmp(taps, "composer "), what);
  say(h, "ax", out);
  snprintf(what, sizeof what, "%s: the focused element says so", where);
  check(has(out, "tab.1 AXRadioButton/AXTabButton \"Chat\" focused"), what);
  taps[0] = 0; say(h, "ax-key space", out);
  snprintf(what, sizeof what, "%s: Space presses what has focus", where);
  check(!strcmp(taps, "tab.1 "), what);
  taps[0] = 0; say(h, "ax-key shift-tab", out); say(h, "ax-key shift-tab", out); say(h, "ax-key enter", out);
  snprintf(what, sizeof what, "%s: Enter presses what has focus (a field only gets the keyboard)", where);
  check(has(out, "focus setup.open") && !strcmp(taps, "composer setup.open "), what);
}

int main(int argc, char **argv) {
  @autoreleasepool {
    if (argc < 2) { fprintf(stderr, "a11y-check SHADER.metal\n"); return 2; }
    UiApp app = {.title = "A11y check", .app_id = "com.openagents.a11y-check", .width = 900, .height = 800, .max_vertices = 64, .shader = argv[1],
                 .frame = frame, .key = key, .drive = drive, .hidden = 1};
    UiHost *h = ui_open(&app, 0);
    if (!h) return 1;
    for (int i = 0; i < 3; i++) ui_frame(h);
    run(h, 1);
    ui_close(h);
    UiHost *off = ui_open(&app, 1);
    if (!off) return 1;
    run(off, 0);
    ui_close(off);
  }
  printf("%s\n", fails ? "a11y-check: FAILED" : "a11y-check: every case passed");
  return fails ? 1 : 0;
}
