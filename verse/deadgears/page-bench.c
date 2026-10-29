// The Dead Gears page's frame budget, in WebAssembly (dg-creatures): the
// page as /realms/dead-gears?spawn=200 runs it, the wave and 200 more mutants
// closing in while you fire, each frame stepping the rules and writing every
// box, light and the HUD (deadgears.coax deadgears_frame). As bench.c, the
// same frames play several times and each keeps its fastest run; the mean
// must leave most of a 60 Hz frame (16.7 ms) to the GPU: under 6 ms.
// Built and run by page-bench.sh.
#define _POSIX_C_SOURCE 199309L
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include "deadgears.h"

static coax_t_DeadGears page;
static float out[24000 * 16], uniforms[2048];
static coax_v_t_DeadGears view(void) { return (coax_v_t_DeadGears){&page, 1}; }
static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }

static double now_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (double)t.tv_sec * 1000.0 + (double)t.tv_nsec / 1e6;
}

enum { FRAMES = 120, RUNS = 5 };

int main(void) {
  static double fastest[FRAMES];
  for (int i = 0; i < FRAMES; i++) fastest[i] = 1e300;
  int boxes = 0, alive = 0;
  coax_v_f32 o = {out, 24000 * 16}, u = {uniforms, coaxial_deadgears_uniform_floats()};
  for (int run = 0; run < RUNS; run++) {
    memset(&page, 0, sizeof page);
    deadgears_message(view(), 600, str("tab=1\nurl=https://openagents.com/realms/dead-gears?spawn=200"));
    deadgears_key(view(), 'f', 0);
    for (int i = 0; i < 10; i++) deadgears_frame(view(), o, u, 1280, 800, 1, 1.0f / 60, false);
    for (int i = 0; i < FRAMES; i++) {
      double t0 = now_ms();
      deadgears_frame(view(), o, u, 1280, 800, 1, 1.0f / 60, false);
      double ms = now_ms() - t0;
      if (ms < fastest[i]) fastest[i] = ms;
    }
    boxes = (int)deadgears_objects(view());
    alive = (int)page.store.e[0].n.e[0];
  }
  double total = 0, worst = 0;
  for (int i = 0; i < FRAMES; i++) { total += fastest[i]; if (fastest[i] > worst) worst = fastest[i]; }
  double mean = total / FRAMES;
  printf("dead-gears page, ?spawn=200 (wasm): %.3f ms a frame on average, %.3f ms at worst, %d boxes drawn\n", mean, worst, boxes);
  (void)alive;
  int ok = mean < 6.0;
  printf("%s: budget 6 ms of a 16.7 ms frame\n", ok ? "ok" : "FAIL");
  return ok ? 0 : 1;
}
