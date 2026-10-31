// vk-kernel: run a `coax spirv --kernel` module on the GPU, headless.
//   vk-kernel MODULE.spv PARAMETERS IN.f32 OUT.f32
// IN holds rows of PARAMETERS little-endian f32 arguments; OUT gets one f32
// per row (a bool result as 1 or 0). The module's interface is the one
// coax/lib/compiler/spirv.coax writes: arguments at binding 0, results at
// binding 1, the row count as a push constant, 64 invocations a group.
// Validation: see vkc.h. Any validation error fails the run.
#include "vkc.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void *slurp(const char *path, size_t *size) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "vk-kernel: cannot read %s\n", path); exit(1); }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  void *p = malloc(n > 0 ? (size_t)n : 1);
  if (!p || fread(p, 1, (size_t)n, f) != (size_t)n) { fprintf(stderr, "vk-kernel: cannot read %s\n", path); exit(1); }
  fclose(f);
  *size = (size_t)n;
  return p;
}

int main(int argc, char **argv) {
  if (argc != 5) { fprintf(stderr, "usage: vk-kernel MODULE.spv PARAMETERS IN.f32 OUT.f32\n"); return 2; }
  size_t module_size, in_size;
  uint32_t *words = slurp(argv[1], &module_size);
  uint32_t params = (uint32_t)atoi(argv[2]);
  float *in = slurp(argv[3], &in_size);
  if (module_size % 4 || !params || in_size % (4u * params)) { fprintf(stderr, "vk-kernel: sizes do not fit\n"); return 2; }
  uint32_t rows = (uint32_t)(in_size / (4u * params));
  if (!rows || (rows + 63) / 64 > 65535) { fprintf(stderr, "vk-kernel: 1 to %u rows\n", 65535u * 64); return 2; }

  Vkc v;
  vkc_open(&v);
  VkcBuffer buffers[2] = {vkc_buffer(&v, in_size), vkc_buffer(&v, (VkDeviceSize)rows * 4)};
  memcpy(buffers[0].map, in, in_size);
  vkc_dispatch(&v, words, module_size / 4, buffers, 2, &rows, sizeof rows, (rows + 63) / 64, 1);
  FILE *f = fopen(argv[4], "wb");
  if (!f || fwrite(buffers[1].map, 4, rows, f) != rows) { fprintf(stderr, "vk-kernel: cannot write %s\n", argv[4]); return 1; }
  fclose(f);
  fprintf(stderr, "vk-kernel: %s, %zu words, %u rows%s\n", v.props.deviceName, module_size / 4, rows,
          v.validating ? "" : " (validation layers not loaded)");
  if (v.validating) fprintf(stderr, "vk-kernel: validation: %d errors, %d warnings\n", vkc_validation_errors, vkc_validation_warnings);
  vkc_free(&v, &buffers[0]);
  vkc_free(&v, &buffers[1]);
  vkc_close(&v);
  return vkc_validation_errors == 0 ? 0 : 1;
}
