// vk-check: Coax-built SPIR-V on the GPU through Vulkan, with no display
// (docs/verse/native-gpu.md, phase G0). It asks verse/gpu/spirv.coax for
// the grade module's words, runs it over a w by h image as a compute
// dispatch, reads the pixels back and compares each with the same pixel
// computed by Coax on the CPU (vg_grade_pixel). It takes no screen, VT or
// DRM master, so it can run while someone uses the machine.
//
//   vk-check [WxH] [--ppm OUT.ppm]
// Validation: see vkc.h. Any validation error fails the run.
#include "spirv.h"
#include "vkc.h"
#include <stdio.h>
#include <string.h>

static int channel_off(uint32_t a, uint32_t b) {
  for (int s = 0; s < 32; s += 8) {
    int d = (int)((a >> s) & 255) - (int)((b >> s) & 255);
    if (d > 1 || d < -1) return 1;
  }
  return 0;
}

int main(int argc, char **argv) {
  uint32_t w = 1920, h = 1080;
  const char *ppm = NULL;
  for (int i = 1; i < argc; i++) {
    if (!strcmp(argv[i], "--ppm") && i + 1 < argc) ppm = argv[++i];
    else if (sscanf(argv[i], "%ux%u", &w, &h) != 2) { fprintf(stderr, "usage: vk-check [WxH] [--ppm OUT.ppm]\n"); return 2; }
  }

  // The module, from Coax.
  static uint32_t words[4096];
  int64_t n = vg_grade_module((coax_v_u32){words, 4096});
  if (n <= 0) { fprintf(stderr, "vk-check: Coax refused the module\n"); return 1; }

  Vkc v;
  vkc_open(&v);
  VkcBuffer image = vkc_buffer(&v, (VkDeviceSize)w * h * 4);
  uint32_t push[2] = {w, h};
  vkc_dispatch(&v, words, (size_t)n, &image, 1, push, sizeof push, (w + 7) / 8, (h + 7) / 8);
  const uint32_t *pixels = image.map;

  // Every pixel against Coax on the CPU.
  uint64_t off = 0, exact = 0, step = 0;
  for (uint32_t y = 0; y < h; y++)
    for (uint32_t x = 0; x < w; x++) {
      uint32_t want = vg_grade_pixel(x, y, w, h), got = pixels[(size_t)y * w + x];
      if (got == want) exact++;
      else if (channel_off(got, want)) off++;
      else step++;
    }
  if (ppm) {
    FILE *f = fopen(ppm, "wb");
    if (f) {
      fprintf(f, "P6\n%u %u\n255\n", w, h);
      for (size_t i = 0; i < (size_t)w * h; i++) { unsigned char c[3] = {(unsigned char)pixels[i], (unsigned char)(pixels[i] >> 8), (unsigned char)(pixels[i] >> 16)}; fwrite(c, 1, 3, f); }
      fclose(f);
    }
  }
  printf("vk-check: %s, %u words of Coax SPIR-V, %ux%u: %llu exact, %llu one rounding step off, %llu wrong%s\n", v.props.deviceName, (unsigned)n, w, h,
         (unsigned long long)exact, (unsigned long long)step, (unsigned long long)off,
         v.validating ? "" : " (validation layers not loaded)");
  if (v.validating) printf("vk-check: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
  vkc_free(&v, &image);
  vkc_close(&v);
  return off == 0 && vkc_validation_errors == 0 ? 0 : 1;
}
