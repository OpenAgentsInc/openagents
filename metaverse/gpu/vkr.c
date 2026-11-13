// A realm on the GPU (vkr.h).
#include "vkr.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void *slurp(const char *path, size_t *size) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "vkr: cannot read %s\n", path); exit(1); }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  void *p = malloc(n > 0 ? (size_t)n : 1);
  if (!p || fread(p, 1, (size_t)n, f) != (size_t)n) { fprintf(stderr, "vkr: cannot read %s\n", path); exit(1); }
  fclose(f);
  *size = (size_t)n;
  return p;
}

// The uniforms named by a list such as "0,1,2,4,5", after `first` floats already in push.
static uint32_t gather(const char *list, const float *u, int nu, float *push, uint32_t first) {
  uint32_t n = first;
  if (!strcmp(list, "-")) return n;
  for (const char *p = list; *p;) {
    int i = (int)strtol(p, (char **)&p, 10);
    if (i < 0 || i >= nu || n >= 32) { fprintf(stderr, "vkr: no uniform %d\n", i); exit(2); }
    push[n++] = u[i];
    if (*p == ',') p++;
    else if (*p) { fprintf(stderr, "vkr: bad uniform list %s\n", list); exit(2); }
  }
  return n;
}

void vkr_open(Vkc *v, Vkr *r, const char *fill_spv, const char *fill_u, const char *stroke_spv, const char *stroke_u) {
  memset(r, 0, sizeof *r);
  r->state = calloc(1, realm_state_size());
  r->vf = realm_vertex_floats();
  r->nu = realm_uniform_floats();
  r->maxv = realm_max_vertices();
  r->vertices = calloc((size_t)r->maxv * r->vf, sizeof(float));
  r->u = calloc((size_t)r->nu, sizeof(float));
  r->vb = vkc_buffer(v, (VkDeviceSize)r->maxv * r->vf * 4);
  r->fill_u = fill_u;
  r->stroke_u = stroke_u;
  size_t fill_n, stroke_n;
  uint32_t *fill = slurp(fill_spv, &fill_n), *stroke = slurp(stroke_spv, &stroke_n);
  r->passes[0] = (VkcPass){.words = fill, .count = fill_n / 4, .push = r->fill_push,
                           .push_size = gather(fill_u, r->u, r->nu, r->fill_push, 0) * 4, .vertex_count = 3};
  r->passes[1] = (VkcPass){.words = stroke, .count = stroke_n / 4, .push = r->stroke_push,
                           .push_size = gather(stroke_u, r->u, r->nu, r->stroke_push, 2) * 4,
                           .vertices = &r->vb, .stride = (uint32_t)r->vf * 4, .additive = 1};
}

void vkr_step(Vkr *r, uint32_t w, uint32_t h, float scale, float dt) {
  int count = realm_frame(r->state, r->vertices, r->u, (float)w, (float)h, scale, dt);
  memcpy(r->vb.map, r->vertices, (size_t)count * r->vf * 4);
  gather(r->fill_u, r->u, r->nu, r->fill_push, 0);
  r->stroke_push[0] = (float)w;
  r->stroke_push[1] = (float)h;
  gather(r->stroke_u, r->u, r->nu, r->stroke_push, 2);
  r->passes[1].vertex_count = (uint32_t)(count - count % 3);
}

void vkr_close(Vkc *v, Vkr *r) {
  vkc_free(v, &r->vb);
  free((void *)r->passes[0].words);
  free((void *)r->passes[1].words);
  free(r->vertices);
  free(r->u);
  free(r->state);
}

int vkr_drops(const char *host, const char *what, const Vkr *r, const double *t, int n, int skip, double period, double *longest, int detail) {
  int d = 0;
  *longest = 0;
  for (int i = skip + 1; i < n; i++) {
    double gap = t[i] - t[i - 1];
    if (gap > *longest) *longest = gap;
    if (gap > period * 1.5 && d++ < 10) {
      printf("%s: %s drop at %.3f s: %.1f ms", host, what, t[i] - t[0], gap * 1000);
      if (detail) printf(" (fence %.1f, realm %.1f, acquire %.1f ms)", r->waited[i] * 1000, r->stepped[i] * 1000, r->acquiring[i] * 1000);
      printf("\n");
    }
  }
  return d;
}
