// The G4 check (docs/verse/native-gpu.md): verse/gpu/shaders/quad.coax drawn
// on the GPU from `coax spirv --vertex quad_vs --fragment quad_fs`, with a
// storage buffer of colors, the uniforms, a texture and a sampler bound,
// against quad_fs run on the CPU at every pixel's center.
//   vk-shader W H quad.spv
#include "vks.h"
#include "quad.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t *read_words(const char *path, size_t *count) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "vk-shader: cannot read %s\n", path); exit(1); }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  uint32_t *w = malloc((size_t)n);
  if (fread(w, 1, (size_t)n, f) != (size_t)n) { fprintf(stderr, "vk-shader: cannot read %s\n", path); exit(1); }
  fclose(f);
  *count = (size_t)n / 4;
  return w;
}

typedef struct { VksPipe *pipe; VksPass *pass; } Draw;
static void record(VkCommandBuffer cmd, void *user) { Draw *d = user; vks_pipe_record(d->pipe, cmd, d->pass); }

int main(int argc, char **argv) {
  if (argc != 4) { fprintf(stderr, "usage: vk-shader W H quad.spv\n"); return 2; }
  uint32_t w = (uint32_t)atoi(argv[1]), h = (uint32_t)atoi(argv[2]);
  size_t count;
  uint32_t *words = read_words(argv[3], &count);
  Vkc v;
  vkc_open(&v);
  printf("vk-shader: %s, %ux%u\n", v.props.deviceName, w, h);

  coax_t_V4 colors[4] = {{0.1f, 0.2f, 0.3f, 1}, {0.9f, 0.1f, 0.1f, 1}, {0.1f, 0.8f, 0.2f, 1}, {0.25f, 0.5f, 1, 1}};
  float uniforms[1] = {0.375f};
  coax_t_V4 texels[4] = {{1, 0, 0, 1}, {0, 1, 0, 1}, {0, 0, 1, 1}, {1, 1, 1, 1}};
  // Bindings in the order coax spirv reported them: uniforms, colors, image, smooth.
  VkcBuffer ub = vkc_buffer(&v, sizeof uniforms), cb = vkc_buffer(&v, sizeof colors);
  memcpy(ub.map, uniforms, sizeof uniforms);
  memcpy(cb.map, colors, sizeof colors);
  VksImage tex = vks_image(&v, 2, 2, 1, 0, VK_FORMAT_R32G32B32A32_SFLOAT, VKS_TEXTURE);
  vks_upload(&v, &tex, texels, sizeof texels);
  VkSampler near = vks_sampler(&v, VKS_NEAREST, 0, 0);
  VksImage target = vks_image(&v, w, h, 1, 0, VK_FORMAT_R32G32B32A32_SFLOAT, VKS_TARGET);
  VksBinding binds[4] = {{VKS_STORAGE, &ub}, {VKS_STORAGE, &cb}, {VKS_IMAGE, .image = &tex}, {VKS_SAMPLER, .sampler = near}};
  VksPass pass = {.words = words, .count = count, .bindings = binds, .nb = 4, .vertex_count = 3, .targets = {&target}, .ntargets = 1};
  VksPipe pipe;
  vks_pipe_open(&v, &pipe, &pass);
  Draw d = {&pipe, &pass};
  vks_submit(&v, record, &d);
  float *gpu = malloc((size_t)w * h * 16);
  vks_read(&v, &target, gpu, (size_t)w * h * 16);

  coax_t_Tex2D image = {2, 2, {texels, 4}};
  coax_t_Sampler sampler = {0};
  long wrong = 0, exact = 0;
  double worst = 0;
  for (uint32_t y = 0; y < h; y++)
    for (uint32_t x = 0; x < w; x++) {
      coax_t_QuadOut in = {{x + 0.5f, y + 0.5f, 0, 1}, {(x + 0.5f) / (float)w, 1 - (y + 0.5f) / (float)h}, 0};
      coax_t_V4 c = quad_fs(in, 1, (coax_v_t_V4){colors, 4}, (coax_v_f32){uniforms, 1}, image, sampler);
      const float *g = gpu + ((size_t)y * w + x) * 4;
      float want[4] = {c.x, c.y, c.z, c.w};
      int same = 1;
      for (int k = 0; k < 4; k++) {
        double e = fabs((double)g[k] - want[k]);
        if (e > worst) worst = e;
        if (e > 1e-6) same = 0;
        if (g[k] != want[k]) same = same == 1 ? 2 : same;
      }
      if (!same) { if (wrong < 5) printf("vk-shader: (%u,%u) gpu %g %g %g %g cpu %g %g %g %g\n", x, y, g[0], g[1], g[2], g[3], want[0], want[1], want[2], want[3]); wrong++; }
      else if (same == 1) exact++;
    }
  printf("vk-shader: %ld pixels exact, %ld within 1e-6, %ld wrong (largest difference %.3g); validation %d errors, %d warnings\n",
         exact, (long)w * h - exact - wrong, wrong, worst, vkc_validation_errors, vkc_validation_warnings);
  vks_pipe_close(&v, &pipe);
  vks_image_free(&v, &target);
  vks_image_free(&v, &tex);
  vkDestroySampler(v.dev, near, NULL);
  vkc_free(&v, &ub);
  vkc_free(&v, &cb);
  vkc_close(&v);
  return wrong || vkc_validation_errors ? 1 : 0;
}
