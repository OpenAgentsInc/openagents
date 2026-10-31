// One pass of a ported shader natively (docs/verse/native-gpu.md, G5): a
// module from `coax spirv --vertex V --fragment F` whose only binding is the
// uniform block (a []f32, bound as storage), drawn as VERTICES vertices into
// N rgba16float targets cleared to zero, read back as float32 RGBA rows into
// OUT (target after target). verse/gpu/pass-check.coax compares OUT with
// the same pass drawn by the browser (verse/gpu/golden/pass.js).
//   vk-pass PASS.spv W H N UNIFORMS.bin OUT.f32 [VERTICES]
#include "vks.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void *slurp(const char *path, size_t *n) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "vk-pass: cannot read %s\n", path); exit(1); }
  fseek(f, 0, SEEK_END);
  long size = ftell(f);
  fseek(f, 0, SEEK_SET);
  void *p = malloc((size_t)size + 16);
  if (fread(p, 1, (size_t)size, f) != (size_t)size) { fprintf(stderr, "vk-pass: cannot read %s\n", path); exit(1); }
  fclose(f);
  *n = (size_t)size;
  return p;
}
// IEEE half to float.
static float half(uint16_t h) {
  int s = h >> 15, e = (h >> 10) & 31, m = h & 1023;
  float v = e == 0 ? ldexpf((float)m, -24) : e == 31 ? (m ? NAN : INFINITY) : ldexpf(1.0f + m / 1024.0f, e - 15);
  return s ? -v : v;
}

typedef struct { VksPipe *pipe; VksPass *pass; } Draw;
static void record(VkCommandBuffer cmd, void *user) { Draw *d = user; vks_pipe_record(d->pipe, cmd, d->pass); }

int main(int argc, char **argv) {
  if (argc < 7) { fprintf(stderr, "usage: vk-pass PASS.spv W H N UNIFORMS.bin OUT.f32 [VERTICES]\n"); return 2; }
  uint32_t w = (uint32_t)atoi(argv[2]), h = (uint32_t)atoi(argv[3]), n = (uint32_t)atoi(argv[4]);
  uint32_t vertices = argc > 7 ? (uint32_t)atoi(argv[7]) : 3;
  if (n < 1 || n > 8) { fprintf(stderr, "vk-pass: 1 to 8 targets\n"); return 2; }
  size_t bytes, ubytes;
  uint32_t *words = slurp(argv[1], &bytes);
  void *uniforms = slurp(argv[5], &ubytes);
  Vkc v;
  vkc_open(&v);
  VkcBuffer ub = vkc_buffer(&v, ubytes);
  memcpy(ub.map, uniforms, ubytes);
  VksImage targets[8];
  VksPass pass = {.words = words, .count = bytes / 4, .nb = 1, .vertex_count = vertices, .ntargets = n};
  VksBinding binds[1] = {{VKS_STORAGE, &ub}};
  pass.bindings = binds;
  for (uint32_t i = 0; i < n; i++) {
    targets[i] = vks_image(&v, w, h, 1, 0, VK_FORMAT_R16G16B16A16_SFLOAT, VKS_TARGET);
    pass.targets[i] = &targets[i];
  }
  VksPipe pipe;
  vks_pipe_open(&v, &pipe, &pass);
  Draw d = {&pipe, &pass};
  vks_submit(&v, record, &d);
  FILE *out = fopen(argv[6], "wb");
  if (!out) { fprintf(stderr, "vk-pass: cannot write %s\n", argv[6]); return 1; }
  uint16_t *raw = malloc((size_t)w * h * 8);
  float *f = malloc((size_t)w * h * 16);
  for (uint32_t i = 0; i < n; i++) {
    vks_read(&v, &targets[i], raw, (size_t)w * h * 8);
    for (size_t k = 0; k < (size_t)w * h * 4; k++) f[k] = half(raw[k]);
    fwrite(f, 16, (size_t)w * h, out);
  }
  fclose(out);
  printf("vk-pass: %s, %ux%u, %u targets; validation %d errors, %d warnings\n", v.props.deviceName, w, h, n,
         vkc_validation_errors, vkc_validation_warnings);
  vks_pipe_close(&v, &pipe);
  for (uint32_t i = 0; i < n; i++) vks_image_free(&v, &targets[i]);
  vkc_free(&v, &ub);
  vkc_close(&v);
  return vkc_validation_errors ? 1 : 0;
}
