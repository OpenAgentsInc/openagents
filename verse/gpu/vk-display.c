// vk-display: a realm straight on the screen with Vulkan, no desktop
// (docs/verse/native-gpu.md, G2: the boot session). It takes DRM master on
// the card, hands the connected connector to Vulkan (VK_EXT_acquire_drm_display),
// and presents through VK_KHR_display in FIFO at the mode's rate: the same
// offscreen pipeline as vk-window (vkr.h, vkp.h). Keys come from evdev,
// grabbed (verse/kms/evdev.h). Escape, the --seconds limit, SIGINT or
// SIGTERM end it, and it lets go of the display; display.sh switched to a
// free VT first and switches back.
//
//   vk-display FILL.spv FILL_U STROKE.spv STROKE_U [--seconds N] [--scale K] [--demo]
//              [--shot OUT.ppm] [--shot-at S]
// --shot copies the image presented at S seconds (default 4) out of the
// swapchain into OUT.ppm (written after the run). DRM cannot say what is on
// the screen meanwhile: the NVIDIA driver flips a leased display through its
// own modesetting, so the planes still name the desktop's last framebuffer.
// Needs root (DRM master, /dev/input).
#define _GNU_SOURCE
#include "evdev.h"
#include "realm.h"
#include "vkd.h"
#include "vkr.h"
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define CHECK(x) do { VkResult r_ = (x); if (r_ != VK_SUCCESS) { fprintf(stderr, "vk-display: %s failed: %d\n", #x, (int)r_); exit(1); } } while (0)

static volatile sig_atomic_t stopping;
static void on_signal(int s) { (void)s; stopping = 1; }

static double now_s(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec / 1e9;
}

// A demo player for a capture nobody plays: the realm names the key (realm_demo_key).
static void demo_keys(void *state, double t) {
  static int held;
  static double last_space = -10;
  int want = realm_demo_key(state);
  if (want == ' ') {
    if (held) { realm_key_up(state, held); held = 0; }
    if (t - last_space > 2.5) { realm_key(state, ' '); realm_key_up(state, ' '); last_space = t; }
    return;
  }
  if (want == held) return;
  if (held) realm_key_up(state, held);
  if (want) realm_key(state, want);
  held = want;
}

int main(int argc, char **argv) {
  if (argc < 5) { fprintf(stderr, "usage: vk-display FILL.spv FILL_U STROKE.spv STROKE_U [--seconds N] [--scale K] [--demo] [--shot OUT.ppm] [--shot-at S]\n"); return 2; }
  double seconds = 60, shot_at = 4;
  float scale = 2;
  int demo = 0;
  const char *shot = NULL;
  for (int i = 5; i < argc; i++) {
    if (!strcmp(argv[i], "--seconds") && i + 1 < argc) seconds = atof(argv[++i]);
    else if (!strcmp(argv[i], "--scale") && i + 1 < argc) scale = (float)atof(argv[++i]);
    else if (!strcmp(argv[i], "--demo")) demo = 1;
    else if (!strcmp(argv[i], "--shot") && i + 1 < argc) shot = argv[++i];
    else if (!strcmp(argv[i], "--shot-at") && i + 1 < argc) shot_at = atof(argv[++i]);
    else { fprintf(stderr, "vk-display: unknown argument %s\n", argv[i]); return 2; }
  }
  signal(SIGINT, on_signal);
  signal(SIGTERM, on_signal);

  Vkc v;
  Vkd screen;
  vkd_open(&v, &screen, "vk-display");
  VkExtent2D extent = screen.extent;
  int plane = screen.plane;
  static Vkp p;
  vkp_open(&v, &p, screen.surface, extent.width, extent.height);
  double period = screen.period;

  static Vkr r;
  vkr_open(&v, &r, argv[1], argv[2], argv[3], argv[4]);
  VkcTarget target;
  vkc_target_open(&v, &target, p.w, p.h, r.passes, 2);
  keyboards_open();
  fprintf(stderr, "vk-display: %s on %ux%u at %.3f Hz, plane %d, %d keyboards; Escape or %.0f s returns the screen\n", realm_name(),
          p.w, p.h, 1 / period, plane, keyboard_count, seconds);

  const char *why = "its time was up";
  double start = now_s(), last = start;
  while (r.frames < VKR_FRAMES) {
    double t = now_s();
    if (t - start >= seconds) break;
    if (stopping) { why = "a signal stopped it"; break; }
    if (keyboards_read(r.state)) { why = "Escape was pressed"; break; }
    if (demo) demo_keys(r.state, t - start);
    vkp_wait(&v, &p);
    t = now_s();
    double dt = t - last;
    last = t;
    if (dt > 0.1) dt = 0.1;
    vkr_step(&r, p.w, p.h, scale, (float)dt);
    double t1 = now_s();
    if (realm_action(r.state)) { why = "the realm navigated away"; break; }
    if (shot && !p.shoot && !p.shot_ready && t - start >= shot_at) p.shoot = 1;
    if (vkp_frame(&v, &p, &target, r.passes, NULL, NULL)) { vkp_resize(&v, &p, p.w, p.h); continue; }
    r.waited[r.frames] = p.waited;
    r.stepped[r.frames] = t1 - t;
    r.acquiring[r.frames] = p.acquiring;
    r.presented[r.frames++] = now_s();
  }
  vkp_wait(&v, &p);
  if (shot && p.shot_ready) vkd_save_ppm(shot, &p);
  double ran = now_s() - start;

  int skip = (int)(1 / period);
  double longest;
  int dropped = vkr_drops("vk-display", "present", &r, r.presented, r.frames, skip, period, &longest, 1);
  printf("vk-display: %s, %s %ux%u, %.3f Hz, %d frames in %.1f s (%.2f a second); it ended because %s\n", v.props.deviceName, realm_name(),
         p.w, p.h, 1 / period, r.frames, ran, r.frames / ran, why);
  printf("vk-display: dropped frames: %d (longest %.1f ms)\n", dropped, longest * 1000);
  if (v.validating) printf("vk-display: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
  else printf("vk-display: validation layers not loaded\n");

  // Let go of the screen: the swapchain, the display, master.
  vkp_close(&v, &p);
  vkc_target_close(&v, &target);
  vkr_close(&v, &r);
  vkd_close(&v, &screen);
  keyboards_close();
  return vkc_validation_errors == 0 ? 0 : 1;
}
