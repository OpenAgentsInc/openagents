// vk-frame: one realm frame drawn on the GPU through Vulkan, headless
// (docs/verse/native-gpu.md, phase G2), against the CPU frame of
// verse/kms --headless. The realm steps as kms.c's headless mode steps it
// (1/60 s a frame, space at frame 20); the GPU draws the full-screen pass
// (`coax spirv --fill`) and then the stroke triangles added one plus one
// (`coax spirv --stroke`) into an RGBA f32 image, which is encoded as kms.c
// encodes and compared with the CPU's PPM.
//
//   vk-frame WxH FRAMES SCALE FILL.spv FILL_U STROKE.spv STROKE_U CPU.ppm [--ppm OUT.ppm]
// FILL_U and STROKE_U list the realm uniforms each pass's procedures take
// after their own parameters (comma-separated indices, `-` for none); the
// stroke pass gets the width and height first. Validation: see vkc.h.
#include "realm.h"
#include "vkc.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void *slurp(const char *path, size_t *size) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "vk-frame: cannot read %s\n", path); exit(1); }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  void *p = malloc(n > 0 ? (size_t)n : 1);
  if (!p || fread(p, 1, (size_t)n, f) != (size_t)n) { fprintf(stderr, "vk-frame: cannot read %s\n", path); exit(1); }
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
    if (i < 0 || i >= nu || n >= 32) { fprintf(stderr, "vk-frame: no uniform %d\n", i); exit(2); }
    push[n++] = u[i];
    if (*p == ',') p++;
    else if (*p) { fprintf(stderr, "vk-frame: bad uniform list %s\n", list); exit(2); }
  }
  return n;
}

static int level(float c) { return (int)(fminf(fmaxf(c, 0.f), 1.f) * 255.f + 0.5f); }

int main(int argc, char **argv) {
  if (argc != 9 && !(argc == 11 && !strcmp(argv[9], "--ppm"))) {
    fprintf(stderr, "usage: vk-frame WxH FRAMES SCALE FILL.spv FILL_U STROKE.spv STROKE_U CPU.ppm [--ppm OUT.ppm]\n");
    return 2;
  }
  uint32_t w, h;
  if (sscanf(argv[1], "%ux%u", &w, &h) != 2 || !w || !h) { fprintf(stderr, "vk-frame: size WxH\n"); return 2; }
  int frames = atoi(argv[2]);
  float scale = (float)atof(argv[3]);
  const char *ppm = argc == 11 ? argv[10] : NULL;

  // The realm, stepped as kms.c's headless mode steps it.
  void *state = calloc(1, realm_state_size());
  int vf = realm_vertex_floats(), nu = realm_uniform_floats();
  float *vertices = calloc((size_t)realm_max_vertices() * vf, sizeof(float));
  float *u = calloc((size_t)nu, sizeof(float));
  int count = 0;
  for (int i = 0; i < frames; i++) {
    if (i == 20) realm_key(state, ' ');
    count = realm_frame(state, vertices, u, (float)w, (float)h, scale, 1.f / 60.f);
  }

  size_t fill_n, stroke_n;
  uint32_t *fill = slurp(argv[4], &fill_n), *stroke = slurp(argv[6], &stroke_n);
  float fill_push[32], stroke_push[32] = {(float)w, (float)h};
  uint32_t fill_k = gather(argv[5], u, nu, fill_push, 0), stroke_k = gather(argv[7], u, nu, stroke_push, 2);

  Vkc v;
  vkc_open(&v);
  VkcBuffer vb = vkc_buffer(&v, (VkDeviceSize)(count ? count : 1) * vf * 4);
  memcpy(vb.map, vertices, (size_t)count * vf * 4);
  VkcPass passes[2] = {
    {.words = fill, .count = fill_n / 4, .push = fill_push, .push_size = fill_k * 4, .vertex_count = 3},
    {.words = stroke, .count = stroke_n / 4, .push = stroke_push, .push_size = stroke_k * 4,
     .vertices = &vb, .stride = (uint32_t)vf * 4, .vertex_count = (uint32_t)count - (uint32_t)count % 3, .additive = 1}};
  float *rgba = malloc((size_t)w * h * 16);
  vkc_render(&v, w, h, passes, 2, rgba);

  // Against the CPU frame, channel by channel after encoding.
  size_t cpu_n;
  unsigned char *cpu = slurp(argv[8], &cpu_n);
  char head[32];
  int header = 0;
  snprintf(head, sizeof head, "P6\n%u %u\n255\n%n", w, h, &header);
  if (cpu_n != (size_t)header + (size_t)w * h * 3 || memcmp(cpu, head, (size_t)header)) { fprintf(stderr, "vk-frame: %s is not a %ux%u P6 frame\n", argv[8], w, h); return 1; }
  const unsigned char *want = cpu + header;
  size_t exact = 0, near = 0, far = 0;
  int worst = 0;
  FILE *out = ppm ? fopen(ppm, "wb") : NULL;
  if (out) fprintf(out, "P6\n%u %u\n255\n", w, h);
  for (size_t i = 0; i < (size_t)w * h; i++) {
    int most = 0;
    for (int c = 0; c < 3; c++) {
      int got = level(rgba[i * 4 + c]), d = abs(got - want[i * 3 + c]);
      if (d > most) most = d;
      if (out) fputc(got, out);
    }
    if (most == 0) exact++;
    else if (most == 1) near++;
    else far++;
    if (most > worst) worst = most;
  }
  if (out) fclose(out);
  printf("vk-frame: %s, %s at %ux%u after %d frames, %d stroke vertices: %zu pixels exact, %zu within 1/255, %zu further (worst %d/255)%s\n",
         v.props.deviceName, realm_name(), w, h, frames, count, exact, near, far, worst, v.validating ? "" : " (validation layers not loaded)");
  if (v.validating) printf("vk-frame: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
  vkc_free(&v, &vb);
  vkc_close(&v);
  return far == 0 && vkc_validation_errors == 0 ? 0 : 1;
}
