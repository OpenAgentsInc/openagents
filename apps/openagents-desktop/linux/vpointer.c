// Firm reason: 6 (native Wayland virtual-pointer ABI for a test; it decides nothing, the checks are host-main.c run_input).
// A virtual pointer that stays on the seat for a whole test (GitHub #13781): input-test.sh's app
// presses its own controls through it in a headless compositor. A pointer made and dropped per
// command (wlrctl) is gone before a client binds it; this one is made once and kept.
//   vpointer WIDTH HEIGHT        then one command a line on stdin, each answered "ok" on stdout:
//     move X Y                   to X, Y in the output's points (absolute, 0..WIDTH, 0..HEIGHT)
//     down | up | click          the left button
//     wheel DY                   a vertical wheel turn of DY points
#define _GNU_SOURCE
#include <linux/input-event-codes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <wayland-client.h>
#include "wlr-virtual-pointer-unstable-v1-client-protocol.h"

static struct wl_seat *seat;
static struct zwlr_virtual_pointer_manager_v1 *manager;

static void global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t version) {
  (void)d; (void)version;
  if (!strcmp(iface, wl_seat_interface.name) && !seat) seat = wl_registry_bind(r, name, &wl_seat_interface, 1);
  else if (!strcmp(iface, zwlr_virtual_pointer_manager_v1_interface.name)) manager = wl_registry_bind(r, name, &zwlr_virtual_pointer_manager_v1_interface, 1);
}
static void global_remove(void *d, struct wl_registry *r, uint32_t name) { (void)d; (void)r; (void)name; }
static const struct wl_registry_listener listener = {.global = global, .global_remove = global_remove};

static uint32_t now_ms(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (uint32_t)(t.tv_sec * 1000 + t.tv_nsec / 1000000); }

int main(int argc, char **argv) {
  if (argc < 3) { fprintf(stderr, "usage: vpointer WIDTH HEIGHT\n"); return 2; }
  uint32_t w = (uint32_t)atoi(argv[1]), h = (uint32_t)atoi(argv[2]);
  struct wl_display *display = wl_display_connect(NULL);
  if (!display) { fprintf(stderr, "vpointer: no Wayland display\n"); return 1; }
  struct wl_registry *registry = wl_display_get_registry(display);
  wl_registry_add_listener(registry, &listener, NULL);
  wl_display_roundtrip(display);
  if (!manager) { fprintf(stderr, "vpointer: the compositor has no zwlr_virtual_pointer_manager_v1\n"); return 1; }
  struct zwlr_virtual_pointer_v1 *p = zwlr_virtual_pointer_manager_v1_create_virtual_pointer(manager, seat);
  wl_display_roundtrip(display);
  setvbuf(stdout, NULL, _IOLBF, 0);
  char line[256];
  while (fgets(line, sizeof line, stdin)) {
    double x = 0, y = 0;
    if (sscanf(line, "move %lf %lf", &x, &y) == 2) {
      zwlr_virtual_pointer_v1_motion_absolute(p, now_ms(), (uint32_t)(x < 0 ? 0 : x), (uint32_t)(y < 0 ? 0 : y), w, h);
      zwlr_virtual_pointer_v1_frame(p);
    } else if (!strncmp(line, "down", 4) || !strncmp(line, "up", 2) || !strncmp(line, "click", 5)) {
      int click = !strncmp(line, "click", 5);
      if (click || line[0] == 'd') { zwlr_virtual_pointer_v1_button(p, now_ms(), BTN_LEFT, WL_POINTER_BUTTON_STATE_PRESSED); zwlr_virtual_pointer_v1_frame(p); }
      if (click) { wl_display_roundtrip(display); struct timespec nap = {0, 30000000}; nanosleep(&nap, NULL); }
      if (click || line[0] == 'u') { zwlr_virtual_pointer_v1_button(p, now_ms(), BTN_LEFT, WL_POINTER_BUTTON_STATE_RELEASED); zwlr_virtual_pointer_v1_frame(p); }
    } else if (sscanf(line, "wheel %lf", &y) == 1) {
      zwlr_virtual_pointer_v1_axis_source(p, WL_POINTER_AXIS_SOURCE_WHEEL);
      zwlr_virtual_pointer_v1_axis(p, now_ms(), WL_POINTER_AXIS_VERTICAL_SCROLL, wl_fixed_from_double(y));
      zwlr_virtual_pointer_v1_frame(p);
    } else {
      printf("unknown: %s", line);
      continue;
    }
    wl_display_roundtrip(display);
    printf("ok\n");
  }
  zwlr_virtual_pointer_v1_destroy(p);
  wl_display_roundtrip(display);
  wl_display_disconnect(display);
  return 0;
}
