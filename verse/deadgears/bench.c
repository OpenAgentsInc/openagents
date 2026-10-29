// The Dead Gears frame budget, in WebAssembly (dg-ecs): 200 mutants
// closing in while you fire, every rule system at the 60 Hz step, each
// system timed on its own. As the demo's twoHundredUnderTwoMs, the same two
// seconds play several times from the same start and each frame keeps its
// fastest run, so time lost to other processes does not count; the mean of
// those frames must be under 2 ms. Built and run by bench.sh.
#define _POSIX_C_SOURCE 199309L
#include <stdio.h>
#include <string.h>
#include <time.h>
#include "Rules.h"

static coax_t_Store store;
static coax_v_t_Store view(void) { return (coax_v_t_Store){&store, 1}; }

static double now_ms(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (double)t.tv_sec * 1000.0 + (double)t.tv_nsec / 1e6;
}

enum { FRAMES = 120, RUNS = 7, SYSTEMS = 8 };

int main(void) {
  static double fastest[FRAMES], system_ms[SYSTEMS];
  for (int i = 0; i < FRAMES; i++) fastest[i] = 1e300;
  for (int k = 0; k < SYSTEMS; k++) system_ms[k] = 1e300;
  int played = 0, alive = 0, entities = 0;
  const float dt = 1.0f / 60;
  for (; played < RUNS; played++) {
    if (played >= 3) {
      double total = 0;
      for (int i = 0; i < FRAMES; i++) total += fastest[i];
      if (total / FRAMES < 2.0) break;
    }
    memset(&store, 0, sizeof store);
    verse_dg_setup(view());
    verse_dg_spawn_crowd(view(), 200);
    for (int i = 0; i < 10; i++) { verse_dg_input(view(), 0, 9, 1, 0, 0, -1); verse_dg_frame(view(), dt); }
    double per[SYSTEMS] = {0};
    for (int i = 0; i < FRAMES; i++) {
      verse_dg_input(view(), 0, 9, 1, 0, 0, -1);
      double t0 = now_ms();
      int64_t steps = verse_dg_frame_begin(view(), dt);
      for (int64_t s = 0; s < steps; s++) {
        for (int k = 0; k < SYSTEMS; k++) {
          double a = now_ms();
          verse_dg_run(view(), k);
          double ms = now_ms() - a;
          verse_dg_timed(view(), k, ms);
          per[k] += ms;
        }
        verse_dg_step_end(view());
      }
      verse_dg_frame_end(view());
      double ms = now_ms() - t0;
      if (ms < fastest[i]) fastest[i] = ms;
    }
    for (int k = 0; k < SYSTEMS; k++) if (per[k] / FRAMES < system_ms[k]) system_ms[k] = per[k] / FRAMES;
    alive = (int)verse_dg_alive(view());
    entities = (int)verse_dg_entities(view());
  }
  double total = 0, worst = 0;
  for (int i = 0; i < FRAMES; i++) { total += fastest[i]; if (fastest[i] > worst) worst = fastest[i]; }
  double mean = total / FRAMES;
  printf("200 mutants (wasm): %.3f ms a frame on average (each frame's fastest of %d runs), %.3f ms at worst, "
         "%d alive, %d entities\n", mean, played, worst, alive, entities);
  for (int k = 0; k < SYSTEMS; k++) {
    coax_string name = store.names.e[k];
    printf("  %.*s %.3f ms\n", (int)name.count, name.data, system_ms[k]);
  }
  int ok = mean < 2.0;
  printf("%s: budget 2 ms\n", ok ? "ok" : "FAIL");
  return ok ? 0 : 1;
}
