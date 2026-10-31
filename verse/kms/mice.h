// The mice, read from evdev and grabbed, as one pointer on a screen with no desktop: their
// relative motion kept as a position in device pixels inside W by H, the left button as
// host.js's pointer down and up (phase 0 and 2, moves 1 between), the wheel as its deltaY
// (CSS pixels down, 100 a notch as Chromium gives). What verse/gpu/vk-realm.c reads.
// Linux only; the caller needs read access to /dev/input (root).
#ifndef VERSE_KMS_MICE_H
#define VERSE_KMS_MICE_H
#include <dirent.h>
#include <fcntl.h>
#include <linux/input.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#define MAX_MICE 8
typedef struct {
  int fds[MAX_MICE], count;
  float x, y;          // the pointer, device pixels
  int down;            // the left button is held
  int moved;           // it moved since the last read
} Mice;
typedef struct {
  void (*pointer)(void *user, float x, float y, int phase);
  void (*wheel)(void *user, float dy);
  void *user;
} MiceTo;

static int mice_bit(const unsigned long *bits, int b) { return (bits[b / (8 * sizeof(unsigned long))] >> (b % (8 * sizeof(unsigned long)))) & 1; }
// Every device with relative X and Y and a left button is a mouse; it starts mid-screen.
static void mice_open(Mice *m, float w, float h) {
  memset(m, 0, sizeof *m);
  m->x = w / 2; m->y = h / 2;
  DIR *d = opendir("/dev/input");
  if (!d) return;
  struct dirent *e;
  while ((e = readdir(d)) && m->count < MAX_MICE) {
    if (strncmp(e->d_name, "event", 5) != 0) continue;
    char path[300];
    snprintf(path, sizeof path, "/dev/input/%s", e->d_name);
    int fd = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
    if (fd < 0) continue;
    unsigned long rel[REL_MAX / (8 * sizeof(unsigned long)) + 1] = {0}, keys[KEY_MAX / (8 * sizeof(unsigned long)) + 1] = {0};
    if (ioctl(fd, EVIOCGBIT(EV_REL, sizeof rel), rel) < 0 || ioctl(fd, EVIOCGBIT(EV_KEY, sizeof keys), keys) < 0 ||
        !mice_bit(rel, REL_X) || !mice_bit(rel, REL_Y) || !mice_bit(keys, BTN_LEFT)) {
      close(fd);
      continue;
    }
    ioctl(fd, EVIOCGRAB, 1);
    m->fds[m->count++] = fd;
  }
  closedir(d);
}
// Every event since the last read, to `to`; motion as one move a read, at the newest place.
static void mice_read(Mice *m, float w, float h, const MiceTo *to) {
  struct input_event ev[64];
  for (int i = 0; i < m->count; i++) {
    ssize_t n;
    while ((n = read(m->fds[i], ev, sizeof ev)) > 0) {
      for (size_t j = 0; j < (size_t)n / sizeof ev[0]; j++) {
        const struct input_event *e = &ev[j];
        if (e->type == EV_REL && (e->code == REL_X || e->code == REL_Y)) {
          if (e->code == REL_X) m->x += (float)e->value;
          else m->y += (float)e->value;
          m->x = m->x < 0 ? 0 : m->x > w - 1 ? w - 1 : m->x;
          m->y = m->y < 0 ? 0 : m->y > h - 1 ? h - 1 : m->y;
          m->moved = 1;
        } else if (e->type == EV_REL && e->code == REL_WHEEL) {
          if (to->wheel) to->wheel(to->user, -100.0f * (float)e->value);
        } else if (e->type == EV_KEY && e->code == BTN_LEFT && e->value != 2) {
          if (m->moved && to->pointer) { to->pointer(to->user, m->x, m->y, 1); m->moved = 0; }
          m->down = e->value == 1;
          if (to->pointer) to->pointer(to->user, m->x, m->y, m->down ? 0 : 2);
        }
      }
    }
  }
  if (m->moved && to->pointer) to->pointer(to->user, m->x, m->y, 1);
  m->moved = 0;
}
static void mice_close(Mice *m) {
  for (int i = 0; i < m->count; i++) { ioctl(m->fds[i], EVIOCGRAB, 0); close(m->fds[i]); }
  m->count = 0;
}
#endif
