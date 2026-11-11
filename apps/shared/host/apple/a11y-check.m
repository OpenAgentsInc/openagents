// Firm reason: 6 (AppKit's Accessibility API; the check of the shared Mac host's VoiceOver elements, #13180).
// A stand-in app on the shared Mac host (ui-host.m) in a window that is never shown: its element table
// has a button, a disabled button, a field, a tab and a message. The check reads the window's
// Accessibility tree the way VoiceOver does (accessibilityChildren from the NSWindow down), presses
// an element by its id (AXPress), moves focus with Tab and Shift-Tab, presses with Space and Enter,
// and does the same through the host's drive operations (ax, ax-press ID, ax-key KEY), offscreen too.
// GitHub #13182: the focus ring shows after a key moves focus and not after a press, its six vertices
// are a bordered quad around the row (focus-ring.h), and a file chooser in a hidden window or offscreen
// opens no panel but waits for the driver's `choose PATH` or `cancel` (a panel would hang this check).
// No permission is needed: it reads its own process's tree, not another's.
//   apps/shared/host/apple/a11y-check.sh
#import <AppKit/AppKit.h>
#include "ui-host-apple.h"
#include "../focus-ring.h"
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
static char chosen[2048];   // every path UiApp.file was given, as "REQUEST:PATH "
static void file(void *u, int request, const char *path) { (void)u; char one[1100]; snprintf(one, sizeof one, "%d:%s ", request, path); strncat(chosen, one, sizeof chosen - strlen(chosen) - 1); }
static int key(void *u, int k, int mods, int down) { (void)u; (void)k; (void)mods; (void)down; return 0; }   // takes no key
static int has(const char *s, const char *part) { return strstr(s, part) != NULL; }
static void say(UiHost *h, const char *op, char *out) { int n = ui_drive(h, op, out, 8192); out[n > 0 ? n : 0] = 0; }

static char folder[1024], afile[1024];   // a folder and a file that exist (the check's shader and its folder)
static void choosing(UiHost *h, const char *where) {
  char out[8192], what[256], want[1200];
  chosen[0] = 0;
  snprintf(what, sizeof what, "%s: a folder chooser opens no panel and waits for the driver", where);
  check(ui_choose(h, 7, 1, 0, "The folder the new session works in", "New Session", NULL) == 1 && (say(h, "choose", out), has(out, "chooser waiting: a folder \"The folder the new session works in\" (request 7)")), what);
  say(h, "choose /no/such/folder", out); int a = has(out, "no file /no/such/folder");
  snprintf(want, sizeof want, "choose %s", afile); say(h, want, out); int b = has(out, "is not a folder");
  snprintf(want, sizeof want, "choose %s", folder); say(h, want, out); int c = has(out, "chose ");
  snprintf(want, sizeof want, "7:%s ", folder);
  snprintf(what, sizeof what, "%s: choose PATH gives the app the folder (a missing path or a file is refused)", where);
  check(a && b && c && !strcmp(chosen, want) && (say(h, "choose", out), has(out, "no chooser waiting")), what);
  chosen[0] = 0;
  ui_choose(h, 1, 0, 1, "Attach files", NULL, NULL);
  snprintf(want, sizeof want, "choose %s", afile); say(h, want, out); say(h, want, out); say(h, "choose done", out);
  snprintf(want, sizeof want, "1:%s 1:%s ", afile, afile);
  snprintf(what, sizeof what, "%s: a chooser of several files takes each, then choose done", where);
  check(has(out, "chose 2") && !strcmp(chosen, want), what);
  chosen[0] = 0;
  ui_choose(h, 2, 1, 0, "The folder", NULL, NULL); say(h, "cancel", out);
  snprintf(what, sizeof what, "%s: cancel ends a chooser with nothing chosen", where);
  check(has(out, "cancelled") && !chosen[0] && (say(h, "choose", out), has(out, "no chooser waiting")), what);
}

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
  // the ring: a key moved focus, so it shows; a press of the pointer hides it until the next key
  say(h, "ax", out);
  snprintf(what, sizeof what, "%s: a key's focus shows the ring", where);
  check(has(out, "setup.open AXButton \"Open Everglade\" focused ring"), what);
  if (windowed) {
    NSView *v = (__bridge NSView *)ui_view(h);
    NSEvent *down = [NSEvent mouseEventWithType:NSEventTypeLeftMouseDown location:NSMakePoint(5, 5) modifierFlags:0 timestamp:0 windowNumber:v.window.windowNumber context:nil eventNumber:0 clickCount:1 pressure:1];
    [v mouseDown:down];
    say(h, "ax", out);
    check(has(out, "setup.open AXButton \"Open Everglade\" focused at") && !has(out, "ring"), "window: a pointer press hides the ring; focus stays");
    say(h, "ax-key tab", out); say(h, "ax", out);
    check(has(out, "composer AXTextField \"Do anything\" value \"hello\" focused ring"), "window: the next Tab shows it again");
  }
  choosing(h, where);
}

int main(int argc, char **argv) {
  @autoreleasepool {
    if (argc < 2) { fprintf(stderr, "a11y-check SHADER.metal\n"); return 2; }
    snprintf(afile, sizeof afile, "%s", argv[1]);
    snprintf(folder, sizeof folder, "%s", argv[1]);
    if (strrchr(folder, '/')) *strrchr(folder, '/') = 0;
    // the ring's vertices: one bordered quad, clear inside, around the box and outside it by the outset
    {
      float v[6 * 28];
      const float red[4] = {1, 0, 0, 1}, none[4] = {0};
      uint32_t n = ui_focus_ring(v, 0, 6, 2, 10, 20, 100, 40, red);
      check(n == 6 && v[2] == 0 && v[0] == 14 && v[1] == 34 && v[6] == 212 && v[7] == 92 && v[9] == 4 && v[15] == 0 && v[16] == 1 && v[19] == 1,
            "the ring is a quad 3 px outside the box, a 2 px border in its colour, clear inside (device px at scale 2)");
      check(ui_focus_ring(v, 1, 6, 2, 10, 20, 100, 40, red) == 1, "the ring is left out when the frame has no room for it");
      ui_focus_ring(v, 0, 6, 1, 0, 0, 10, 10, none);
      check(fabsf(v[16] - 0x8b / 255.0f) < 1e-6f && v[19] == 1, "with no colour of the app's, the ring is Zeron's accent");
    }
    UiApp app = {.title = "A11y check", .app_id = "com.openagents.a11y-check", .width = 900, .height = 800, .max_vertices = 64, .shader = argv[1],
                 .frame = frame, .key = key, .drive = drive, .file = file, .hidden = 1};
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
