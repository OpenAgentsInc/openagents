// The keyboards, read from evdev and grabbed so keys reach the realm and not
// the console under it: what verse/kms and verse/gpu's display session use.
// Linux only; the caller needs read access to /dev/input (root).
#ifndef VERSE_KMS_EVDEV_H
#define VERSE_KMS_EVDEV_H
#include "keys.h"
#include <dirent.h>
#include <fcntl.h>
#include <linux/input.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#define MAX_KEYBOARDS 16
static int keyboards[MAX_KEYBOARDS], keyboard_count;

// Every device that has a space bar is a keyboard. Grab it, so keys reach
// the realm and not the console under it.
static void keyboards_open(void) {
  DIR *d = opendir("/dev/input");
  if (!d) return;
  struct dirent *e;
  while ((e = readdir(d)) && keyboard_count < MAX_KEYBOARDS) {
    if (strncmp(e->d_name, "event", 5) != 0) continue;
    char path[300];
    snprintf(path, sizeof path, "/dev/input/%s", e->d_name);
    int fd = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
    if (fd < 0) continue;
    unsigned long bits[KEY_MAX / (8 * sizeof(unsigned long)) + 1] = {0};
    if (ioctl(fd, EVIOCGBIT(EV_KEY, sizeof bits), bits) < 0 ||
        !(bits[KEY_SPACE / (8 * sizeof(unsigned long))] & (1UL << (KEY_SPACE % (8 * sizeof(unsigned long)))))) {
      close(fd);
      continue;
    }
    ioctl(fd, EVIOCGRAB, 1);
    keyboards[keyboard_count++] = fd;
  }
  closedir(d);
}

// Each key to the realm; it returns 1 once Escape is pressed. Shift held on any keyboard makes
// the printable keys their shifted characters, and a key's release is the key its press was
// (a letter pressed, then shift, then released, lets go of the letter it typed).
static int keyboards_shift;
static int keyboards_down[KEY_MAX + 1];
static int keyboards_read(void *state) {
  int escape = 0;
  struct input_event ev[64];
  for (int i = 0; i < keyboard_count; i++) {
    ssize_t n;
    while ((n = read(keyboards[i], ev, sizeof ev)) > 0) {
      for (size_t j = 0; j < (size_t)n / sizeof ev[0]; j++) {
        if (ev[j].type != EV_KEY || ev[j].value == 2 || ev[j].code > KEY_MAX) continue;
        int code = ev[j].code;
        if (code == KEY_LEFTSHIFT || code == KEY_RIGHTSHIFT) keyboards_shift += ev[j].value == 1 ? 1 : (keyboards_shift > 0 ? -1 : 0);
        int k = ev[j].value == 1 ? realm_key_shifted(code, keyboards_shift > 0) : keyboards_down[code];
        if (!k) continue;
        if (ev[j].value == 1) { keyboards_down[code] = k; realm_key(state, k); }
        else { keyboards_down[code] = 0; realm_key_up(state, k); }
        if (k == REALM_ESCAPE && ev[j].value == 1) escape = 1;
      }
    }
  }
  return escape;
}

static void keyboards_close(void) {
  for (int i = 0; i < keyboard_count; i++) {
    ioctl(keyboards[i], EVIOCGRAB, 0);
    close(keyboards[i]);
  }
}
#endif
