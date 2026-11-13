// Firm reason: 6 (a Unix socket and the frame clock; the operations' answers are the app's Coax).
// coax drive on the shared hosts (docs/research/exact/04-apps-audit-2026-11-10.md, recommendations 1
// and 3; GitHub #13156). Included by linux/ui-host.c and apple/ui-host.m after their ui_fill, so both
// hosts answer the same operations the same way:
//   clock +MS                     MS of the agent clock, in frames of a sixtieth of a second (at least one)
//   clock settle [within MS]      frames, a sixtieth of a second of real time apart (an engine's answer comes
//                                 in real time), until the app says nothing is coming (UiApp.busy), at most
//                                 MS of the clock (10000)
//   screenshot OUT.png            the frame as it is now (offscreen)
//   quit                          the session ends (ui_drive_serve returns)
// Any other `clock ...` goes to the app first (UiApp.drive), for an app with clock operations of its own.
// The iPhone app (apple/phone-drive.c) includes this file too, with a UiHost of its own and no window.
// A host may answer operations of its own first (UI_DRIVE_HOST: the Mac's ax, ax-press ID and
// ax-key KEY, and the file chooser's choose, choose PATH, choose done and cancel; apple/ui-host.m).
// Everything else is the app's (UiApp.drive): `tree`, `tap ID`, `type ID TEXT` and the app's own.
// The agent clock: once on (ui_agent_clock), a frame gets no time of its own; only `clock` moves the
// app's clock, so between two operations nothing moves and a picture does not depend on the machine.
// The socket speaks one operation a line; each answer is its length in decimal, a newline, then that
// many bytes (coax/web/drive.coax's `drive native`).
#include "drive.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

static double ui_drive_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
// The time a frame gets: its own, or under the agent clock what `clock` is giving it.
static float ui_drive_dt(UiHost *h, float dt) { return h->drive.agent ? h->drive.giving : dt; }
static void ui_drive_step(UiHost *h, float dt) {
  h->drive.giving = dt; h->drive.clock_ms += dt * 1000.0;
  ui_fill(h, dt);
  h->drive.giving = 0; h->stale = 1;
}
void ui_agent_clock(UiHost *h, int on) { h->drive.agent = on != 0; }
void ui_real_time(UiHost *h, int on) { h->drive.real_time = on != 0; }

static int ui_drive_say(char *out, int cap, const char *fmt, ...) __attribute__((format(printf, 3, 4)));
static int ui_drive_say(char *out, int cap, const char *fmt, ...) {
  va_list ap; va_start(ap, fmt);
  int n = vsnprintf(out, (size_t)cap, fmt, ap);
  va_end(ap);
  return n < 0 ? 0 : n >= cap ? cap - 1 : n;
}
int ui_drive(UiHost *h, const char *op, char *out, int cap) {
  if (cap < 2) return 0;
  while (*op == ' ') op++;
  if (!strncmp(op, "clock ", 6)) {
    const char *arg = op + 6;
    struct timespec nap = {0, 16666667};
    if (arg[0] == '+') {
      double ms = atof(arg + 1);
      int frames = (int)(ms * 60 / 1000 + 0.5);
      if (frames < 1) frames = 1;
      for (int i = 0; i < frames; i++) { ui_drive_step(h, 1 / 60.0f); if (h->drive.real_time) nanosleep(&nap, NULL); }
      return ui_drive_say(out, cap, "clock: %.0f ms", h->drive.clock_ms);
    }
    if (!strncmp(arg, "settle", 6)) {
      const char *within = strstr(arg, "within ");
      double bound = within ? atof(within + 7) : 10000, spent = 0;
      // at least a fifth of a second: an engine's first output is already on its way when a send returns
      while (spent < bound) {
        ui_drive_step(h, 1 / 60.0f); nanosleep(&nap, NULL); spent += 1000 / 60.0;
        if (spent >= 200 && !(h->app.busy && h->app.busy(h->app.user))) break;
      }
      if (h->app.busy && h->app.busy(h->app.user)) return ui_drive_say(out, cap, "clock: still busy after %.0f ms", spent);
      return 0;
    }
    // the app's own clock operations (the shared app's fakes: clock data release NAME, apps/shared/openagents-app.coax)
    int own = h->app.drive ? h->app.drive(h->app.user, op, out, cap) : -1;
    if (own >= 0) { h->stale = 1; return own < cap ? own : cap - 1; }
    return ui_drive_say(out, cap, "clock +MS | clock settle [within MS]");
  }
  if (!strncmp(op, "screenshot ", 11)) {
    if (!h->offscreen) return ui_drive_say(out, cap, "screenshot: offscreen only");
    ui_shot_frame(h, h->w, h->rows, 0);
    if (!ui_shot_write(h, op + 11)) return ui_drive_say(out, cap, "screenshot: could not write %s", op + 11);
    return 0;
  }
  if (!strcmp(op, "quit")) { h->drive.quit = 1; return 0; }
  if (!*op) return 0;
#ifdef UI_DRIVE_HOST
  {   // a host's own operations (the Mac's accessibility and its file chooser)
    int n = UI_DRIVE_HOST(h, op, out, cap);
    if (n >= 0) return n < cap ? n : cap - 1;
  }
#endif
  int n = h->app.drive ? h->app.drive(h->app.user, op, out, cap) : -1;
  if (n >= 0) { h->stale = 1; return n < cap ? n : cap - 1; }
  char name[64]; int k = 0;
  while (op[k] && op[k] != ' ' && k < 63) { name[k] = op[k]; k++; }
  name[k] = 0;
  return ui_drive_say(out, cap, "unknown operation '%s'; operations: clock, screenshot, quit%s", name, h->app.drive ? " and the app's (tree, tap ID, type ID TEXT ...)" : "");
}

int ui_drive_listen(UiHost *h, const char *path) {
  struct sockaddr_un addr = {.sun_family = AF_UNIX};
  if (!path || strlen(path) >= sizeof addr.sun_path) return 0;
  strcpy(addr.sun_path, path);
  unlink(path);
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd < 0) return 0;
  if (bind(fd, (struct sockaddr *)&addr, sizeof addr) < 0 || listen(fd, 4) < 0) { close(fd); return 0; }
  fcntl(fd, F_SETFL, fcntl(fd, F_GETFL, 0) | O_NONBLOCK);
  h->drive.listen_fd = fd; h->drive.conn_fd = -1; h->drive.line_n = 0;
  return 1;
}
static void ui_drive_answer(UiHost *h, const char *op) {
  static char out[1 << 16];
  int n = ui_drive(h, op, out, sizeof out);
  char head[24]; int k = snprintf(head, sizeof head, "%d\n", n);
  if (write(h->drive.conn_fd, head, (size_t)k) != k || (n > 0 && write(h->drive.conn_fd, out, (size_t)n) != n)) { close(h->drive.conn_fd); h->drive.conn_fd = -1; }
}
// What the driver sent, answered; waits at most timeout_ms for it. 0 once the driver has asked to quit.
static int ui_drive_pump(UiHost *h, int timeout_ms) {
  if (h->drive.listen_fd <= 0) return 1;
  struct pollfd p = {.fd = h->drive.conn_fd >= 0 ? h->drive.conn_fd : h->drive.listen_fd, .events = POLLIN};
  if (poll(&p, 1, timeout_ms) <= 0) return !h->drive.quit;
  if (h->drive.conn_fd < 0) {
    int c = accept(h->drive.listen_fd, NULL, NULL);
    if (c >= 0) { h->drive.conn_fd = c; h->drive.line_n = 0; }
    return 1;
  }
  char b[4096];
  ssize_t got = read(h->drive.conn_fd, b, sizeof b);
  if (got <= 0) { close(h->drive.conn_fd); h->drive.conn_fd = -1; return !h->drive.quit; }
  for (ssize_t i = 0; i < got && h->drive.conn_fd >= 0; i++) {
    if (b[i] == '\n') { h->drive.line[h->drive.line_n] = 0; ui_drive_answer(h, h->drive.line); h->drive.line_n = 0; }
    else if (h->drive.line_n < UI_DRIVE_LINE - 1) h->drive.line[h->drive.line_n++] = b[i];
  }
  return !h->drive.quit;
}
// In a window: the driver's operations, between frames, without waiting.
static void ui_drive_poll(UiHost *h) { if (h->drive.listen_fd > 0) ui_drive_pump(h, 0); }
int ui_drive_serve(UiHost *h) {
  if (h->drive.listen_fd <= 0) return 0;
  while (ui_drive_pump(h, 1000)) {}
  return 1;
}
static void ui_drive_close(UiHost *h) {
  if (h->drive.listen_fd <= 0) return;
  if (h->drive.conn_fd >= 0) close(h->drive.conn_fd);
  close(h->drive.listen_fd);
  h->drive.conn_fd = -1; h->drive.listen_fd = 0;
}
