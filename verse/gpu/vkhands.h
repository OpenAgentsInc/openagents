// Your hand as the pointer (docs/os/2026-11-02-verse-os.md, V6): vk-realm.c --hands SOCKET reads the
// camera daemon's hands lines (os/camera-serve.coax's hands.sock) without waiting on them, asks
// os/verse-hands.coax (verse-hands.h) where the fingertip is on the screen and whether it pinches,
// and gives the realm (or the desk's panel) the same pointer events a mouse would: a move each
// line, down when the pinch begins, up when it ends. The socket is joined again every two seconds
// while it is not there (the daemon restarted, or hands are off). Included once, by vk-realm.c.
#ifndef VKHANDS_H
#define VKHANDS_H
#include "verse-hands.h"
#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

typedef struct {
  const char *path;
  int fd;
  double tried_at;
  char line[16384]; size_t line_n;
  int64_t x, y;          // the smoothed pointer, millionths of the screen; -1 before the first
  int64_t run; int down; // the pinch's run of frames, and the button
  int seen;              // a hand is in view
} VkHands;

static void vkhands_open(VkHands *h, const char *path) { memset(h, 0, sizeof *h); h->path = path; h->fd = -1; h->x = h->y = -1; }

static void vkhands_join(VkHands *h, double now) {
  if (h->fd >= 0 || !h->path || now - h->tried_at < 2) return;
  h->tried_at = now;
  int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK, 0);
  if (fd < 0) return;
  struct sockaddr_un a = {.sun_family = AF_UNIX};
  snprintf(a.sun_path, sizeof a.sun_path, "%s", h->path);
  if (connect(fd, (struct sockaddr *)&a, sizeof a) && errno != EINPROGRESS) { close(fd); return; }
  h->fd = fd; h->line_n = 0;
}

// The lines that came: each moves the pointer; to(x, y, phase) gets it in the screen's pixels
// (W by H), phase 0 down, 1 move, 2 up, as host.js's pointer.
static void vkhands_poll(VkHands *h, double now, uint32_t w, uint32_t hgt, void (*to)(float x, float y, int phase)) {
  vkhands_join(h, now);
  if (h->fd < 0) return;
  char buf[8192];
  ssize_t n;
  while ((n = read(h->fd, buf, sizeof buf)) > 0) {
    for (ssize_t i = 0; i < n; i++) {
      if (buf[i] != '\n') { if (h->line_n < sizeof h->line) h->line[h->line_n++] = buf[i]; continue; }
      coax_string text = {h->line, (int64_t)h->line_n};
      int64_t what = verse_hands_read(text, 0);
      h->line_n = 0;
      int pinched = what == 2;   // os/verse-hands.coax HANDS_PINCH
      h->run = verse_hands_run(h->run, pinched);
      int was = h->down;
      h->down = verse_hands_down(was, h->run);
      if (what == 0) { h->seen = 0; if (was && !h->down && h->x >= 0) to((float)h->x * w / 1e6f, (float)h->y * hgt / 1e6f, 2); continue; }
      h->seen = 1;
      h->x = verse_hands_ease(h->x, verse_hands_screen(verse_hands_read(text, 1), 0));
      h->y = verse_hands_ease(h->y, verse_hands_screen(verse_hands_read(text, 2), 1));
      float sx = (float)h->x * w / 1e6f, sy = (float)h->y * hgt / 1e6f;
      to(sx, sy, h->down && !was ? 0 : !h->down && was ? 2 : 1);
    }
  }
  if (n == 0 || (n < 0 && errno != EAGAIN && errno != EWOULDBLOCK && errno != EINTR)) { close(h->fd); h->fd = -1; }
}
static void vkhands_close(VkHands *h) { if (h->fd >= 0) close(h->fd); h->fd = -1; }
#endif
