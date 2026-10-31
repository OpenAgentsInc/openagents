// vk-escape: a virtual keyboard (uinput) that presses Escape after a delay,
// to check that Escape returns the screen with nobody at the keyboard.
//   vk-escape SECONDS        (root; start it before vk-display opens the keyboards)
// It has a space bar too, so verse/kms/evdev.h takes it for a keyboard.
#define _DEFAULT_SOURCE
#include <fcntl.h>
#include <linux/uinput.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

static void emit(int fd, int type, int code, int value) {
  struct input_event e = {.type = (unsigned short)type, .code = (unsigned short)code, .value = value};
  if (write(fd, &e, sizeof e) != (ssize_t)sizeof e) perror("vk-escape: write");
}

int main(int argc, char **argv) {
  if (argc != 2) { fprintf(stderr, "usage: vk-escape SECONDS\n"); return 2; }
  int fd = open("/dev/uinput", O_WRONLY | O_NONBLOCK);
  if (fd < 0) { perror("vk-escape: /dev/uinput"); return 1; }
  ioctl(fd, UI_SET_EVBIT, EV_KEY);
  ioctl(fd, UI_SET_KEYBIT, KEY_ESC);
  ioctl(fd, UI_SET_KEYBIT, KEY_SPACE);
  struct uinput_setup setup = {.id = {.bustype = BUS_VIRTUAL, .vendor = 0x1, .product = 0x1}};
  strcpy(setup.name, "verse escape check");
  if (ioctl(fd, UI_DEV_SETUP, &setup) < 0 || ioctl(fd, UI_DEV_CREATE) < 0) { perror("vk-escape: uinput"); return 1; }
  usleep((useconds_t)(atof(argv[1]) * 1e6));
  emit(fd, EV_KEY, KEY_ESC, 1);
  emit(fd, EV_SYN, SYN_REPORT, 0);
  usleep(50000);
  emit(fd, EV_KEY, KEY_ESC, 0);
  emit(fd, EV_SYN, SYN_REPORT, 0);
  usleep(500000);
  ioctl(fd, UI_DEV_DESTROY);
  close(fd);
  fprintf(stderr, "vk-escape: pressed Escape\n");
  return 0;
}
