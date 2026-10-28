/* A C host for pulse.coax. It runs the Coax kernel and a plain C reference
   side by side and checks, every frame, that both produce the same bytes. */
#pragma STDC FP_CONTRACT OFF
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "pulse.h"

enum { COUNT = 100000, FRAMES = 600, CHANNELS = 64 };

static int64_t reference_advance(coax_t_Pulse *pulses, int64_t count, float dt) {
  int64_t arrived = 0;
  for (int64_t i = 0; i < count; i++) {
    coax_t_Pulse p = pulses[i];
    if (p.hops == 0) continue;
    p.position = p.position + p.speed * dt;
    while (p.position >= 1.0f) {
      p.position = p.position - 1.0f;
      p.channel = (p.channel * 17 + 5) % CHANNELS;
      p.hops -= 1;
      if (p.hops == 0) { arrived++; break; }
    }
    pulses[i] = p;
  }
  return arrived;
}

static double now(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (double)t.tv_sec + (double)t.tv_nsec * 1e-9;
}

int main(void) {
  coax_t_Pulse *a = malloc(sizeof(coax_t_Pulse) * COUNT), *b = malloc(sizeof(coax_t_Pulse) * COUNT);
  uint32_t seed = 12345;
  for (int i = 0; i < COUNT; i++) {
    seed = seed * 1664525u + 1013904223u;
    a[i].position = (float)(seed >> 8) / 16777216.0f;
    a[i].speed = 0.5f + (float)(seed % 1000) / 250.0f;
    a[i].channel = (int32_t)(seed % CHANNELS);
    a[i].hops = 1 + (int32_t)(seed % 40);
  }
  memcpy(b, a, sizeof(coax_t_Pulse) * COUNT);

  float dt = 1.0f / 60.0f;
  int64_t arrived = 0;
  double coax_time = 0, c_time = 0;
  for (int frame = 0; frame < FRAMES; frame++) {
    double t0 = now();
    int64_t got = advance_pulses((coax_v_t_Pulse){a, COUNT}, dt);
    double t1 = now();
    int64_t want = reference_advance(b, COUNT, dt);
    double t2 = now();
    coax_time += t1 - t0;
    c_time += t2 - t1;
    if (got != want || memcmp(a, b, sizeof(coax_t_Pulse) * COUNT) != 0) {
      printf("frame %d: Coax and C differ\n", frame);
      return 1;
    }
    arrived += got;
  }
  int32_t load[CHANNELS];
  channel_load((coax_v_t_Pulse){a, COUNT}, (coax_v_s32){load, CHANNELS});
  int32_t waiting = 0;
  for (int c = 0; c < CHANNELS; c++) waiting += load[c];

  printf("%d pulses x %d frames: Coax matches the C reference byte for byte\n", COUNT, FRAMES);
  printf("arrived %lld, still moving %d\n", (long long)arrived, waiting);
  printf("time per frame: Coax %.3f ms, C %.3f ms\n", coax_time * 1e3 / FRAMES, c_time * 1e3 / FRAMES);
  free(a);
  free(b);
  return 0;
}
