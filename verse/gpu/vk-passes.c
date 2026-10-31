// Passes of a ported shader natively (docs/verse/native-gpu.md, G5), from a
// spec verse/gpu/everglade-pass.coax writes; verse/gpu/golden/pass.js draws
// the same spec in the browser. The spec, a directive a line (`wgsl FILE`,
// the browser's WGSL, is skipped here):
//   size W H                         the colour targets' size
//   buffer NAME FILE                 a buffer the shaders name (bound as storage here)
//   layers NAME FILE W H N LEVELS    an rgba8 sRGB 2D-array texture: level after level, each layer after layer
//   sampler NAME FILTER WRAP MIP CMP linear|nearest, repeat|clamp, mip|flat, none|less
//   depth NAME W H                   a depth32float texture a pass draws and a later one samples
//   index FILE                       u32 indices for the draw list
//   draws FILE N                     the draw list: N records of u32 first, count, base instance, instances
//   pass SPV VS FS TARGETS DEPTH BIAS SLOPE CALLS
//                                    VS and FS the WGSL's entries (for the browser); SPV a module from coax spirv, SPV.bindings its bindings ("binding I KIND NAME"
//                                    a line, as coax spirv prints them); TARGETS rgba16float colour targets
//                                    (cleared to 0); DEPTH a depth NAME, "own" (one at the size) or "-";
//                                    depth tests less and writes; BIAS and SLOPE as WebGPU's depthBias and
//                                    depthBiasSlopeScale; CALLS "draws", or V or V:I vertices and instances
// The last pass's colour targets (it must have some) are written to OUT as float32 RGBA rows, target after target.
//   vk-passes SPEC OUT
#include "vks.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXR 32
static void *slurp(const char *path, size_t *n) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "vk-passes: cannot read %s\n", path); exit(1); }
  fseek(f, 0, SEEK_END);
  long size = ftell(f);
  fseek(f, 0, SEEK_SET);
  void *p = malloc((size_t)size + 16);
  if (fread(p, 1, (size_t)size, f) != (size_t)size) { fprintf(stderr, "vk-passes: cannot read %s\n", path); exit(1); }
  fclose(f);
  *n = (size_t)size;
  return p;
}
static float half(uint16_t h) {
  int s = h >> 15, e = (h >> 10) & 31, m = h & 1023;
  float v = e == 0 ? ldexpf((float)m, -24) : e == 31 ? (m ? NAN : INFINITY) : ldexpf(1.0f + m / 1024.0f, e - 15);
  return s ? -v : v;
}

typedef struct { char name[64]; int kind; VkcBuffer buf; VksImage img; VkSampler sampler; } Res;
static Res res[MAXR];
static int nres;
static Res *find(const char *name) {
  for (int i = 0; i < nres; i++) if (!strcmp(res[i].name, name)) return &res[i];
  fprintf(stderr, "vk-passes: the spec has no %s\n", name);
  exit(1);
}
typedef struct { VksPipe *pipe; VksPass *pass; } Draw;
static void record(VkCommandBuffer cmd, void *user) { Draw *d = user; vks_pipe_record(d->pipe, cmd, d->pass); }

int main(int argc, char **argv) {
  if (argc != 3) { fprintf(stderr, "usage: vk-passes SPEC OUT\n"); return 2; }
  FILE *spec = fopen(argv[1], "r");
  if (!spec) { fprintf(stderr, "vk-passes: cannot read %s\n", argv[1]); return 1; }
  Vkc v;
  vkc_open(&v);
  uint32_t w = 0, h = 0, ndraws = 0;
  uint32_t *draws = NULL;
  VkcBuffer index = {0};
  int has_index = 0, passes = 0, last_nt = 0;
  VksImage targets[8];
  uint32_t ntargets = 0;
  char line[4096];
  while (fgets(line, sizeof line, spec)) {
    char a[6][1024] = {{0}};
    int n = sscanf(line, "%1023s %1023s %1023s %1023s %1023s %1023s", a[0], a[1], a[2], a[3], a[4], a[5]);
    if (n <= 0 || a[0][0] == '#') continue;
    if (!strcmp(a[0], "wgsl")) continue;   // the browser's
    if (!strcmp(a[0], "size")) { w = (uint32_t)atoi(a[1]); h = (uint32_t)atoi(a[2]); continue; }
    if (!strcmp(a[0], "index") || !strcmp(a[0], "draws") || !strcmp(a[0], "buffer")) {
      size_t bytes;
      void *data = slurp(!strcmp(a[0], "buffer") ? a[2] : a[1], &bytes);
      if (!strcmp(a[0], "draws")) { draws = data; ndraws = (uint32_t)atoi(a[2]); if (ndraws * 16 > bytes) { fprintf(stderr, "vk-passes: a short draw list\n"); return 1; } continue; }
      VkcBuffer b = vkc_buffer(&v, bytes < 16 ? 16 : (bytes + 3) & ~(size_t)3);
      memcpy(b.map, data, bytes);
      free(data);
      if (!strcmp(a[0], "index")) { index = b; has_index = 1; continue; }
      Res *r = &res[nres++];
      snprintf(r->name, sizeof r->name, "%s", a[1]); r->kind = VKS_STORAGE; r->buf = b;
      continue;
    }
    if (!strcmp(a[0], "layers")) {
      int lw, lh, ln, levels;
      if (sscanf(line, "%*s %*s %*s %d %d %d %d", &lw, &lh, &ln, &levels) != 4) { fprintf(stderr, "vk-passes: layers NAME FILE W H N LEVELS\n"); return 1; }
      size_t bytes;
      void *data = slurp(a[2], &bytes);
      Res *r = &res[nres++];
      snprintf(r->name, sizeof r->name, "%s", a[1]); r->kind = VKS_IMAGE;
      r->img = vks_image_levels(&v, (uint32_t)lw, (uint32_t)lh, (uint32_t)ln, 1, VK_FORMAT_R8G8B8A8_SRGB, VKS_TEXTURE, (uint32_t)levels);
      vks_upload(&v, &r->img, data, bytes);
      free(data);
      continue;
    }
    if (!strcmp(a[0], "sampler")) {
      Res *r = &res[nres++];
      snprintf(r->name, sizeof r->name, "%s", a[1]); r->kind = VKS_SAMPLER;
      char cmp[64] = "none";
      sscanf(line, "%*s %*s %*s %*s %*s %63s", cmp);
      int compare = strcmp(cmp, "none") != 0;
      r->sampler = vks_sampler_with(&v, strcmp(a[2], "linear") ? VKS_NEAREST : VKS_LINEAR, !strcmp(a[3], "repeat"), compare,
                                    VK_COMPARE_OP_LESS, !strcmp(a[4], "mip"));
      continue;
    }
    if (!strcmp(a[0], "depth")) {
      Res *r = &res[nres++];
      snprintf(r->name, sizeof r->name, "%s", a[1]); r->kind = VKS_IMAGE;
      r->img = vks_image(&v, (uint32_t)atoi(a[2]), (uint32_t)atoi(a[3]), 1, 0, VK_FORMAT_D32_SFLOAT, VKS_TARGET | VKS_TEXTURE);
      continue;
    }
    if (!strcmp(a[0], "pass")) {
      char spv[1024], depth[64], calls[64];
      int nt;
      float bias, slope;
      if (sscanf(line, "%*s %1023s %*s %*s %d %63s %f %f %63s", spv, &nt, depth, &bias, &slope, calls) != 6 || nt < 0 || nt > 8) {
        fprintf(stderr, "vk-passes: pass SPV VS FS TARGETS DEPTH BIAS SLOPE CALLS\n"); return 1;
      }
      size_t bytes;
      uint32_t *words = slurp(spv, &bytes);
      // the bindings, in the module's order, by name
      char bpath[1100];
      snprintf(bpath, sizeof bpath, "%s.bindings", spv);
      FILE *bf = fopen(bpath, "r");
      if (!bf) { fprintf(stderr, "vk-passes: no %s\n", bpath); return 1; }
      VksBinding binds[MAXR];
      uint32_t nb = 0;
      char bl[512];
      while (fgets(bl, sizeof bl, bf)) {
        int i; char kind[64], name[128];
        if (sscanf(bl, "binding %d %63s %127s", &i, kind, name) != 3) continue;
        Res *r = find(name);
        if ((uint32_t)i != nb) { fprintf(stderr, "vk-passes: %s's bindings are not in order\n", bpath); return 1; }
        binds[nb++] = r->kind == VKS_STORAGE ? (VksBinding){VKS_STORAGE, &r->buf} : r->kind == VKS_IMAGE ? (VksBinding){VKS_IMAGE, .image = &r->img}
                                                                                   : (VksBinding){VKS_SAMPLER, .sampler = r->sampler};
      }
      fclose(bf);
      // the colour targets, made once at the size and cleared by each pass
      for (; ntargets < (uint32_t)nt; ntargets++) targets[ntargets] = vks_image(&v, w, h, 1, 0, VK_FORMAT_R16G16B16A16_SFLOAT, VKS_TARGET);
      VksImage own = {0};
      VksPass pass = {.words = words, .count = bytes / 4, .bindings = binds, .nb = nb, .ntargets = (uint32_t)nt, .depth_bias = bias, .depth_slope = slope};
      for (int i = 0; i < nt; i++) pass.targets[i] = &targets[i];
      if (!strcmp(depth, "own")) { own = vks_image(&v, w, h, 1, 0, VK_FORMAT_D32_SFLOAT, VKS_TARGET); pass.depth = &own; }
      else if (strcmp(depth, "-")) pass.depth = &find(depth)->img;
      if (pass.depth) { pass.depth_test = 1; pass.depth_write = 1; }
      if (!strcmp(calls, "draws")) { pass.draws = draws; pass.ndraws = ndraws; pass.index = has_index ? &index : NULL; }
      else { unsigned vc = 0, ic = 1; sscanf(calls, "%u:%u", &vc, &ic); pass.vertex_count = vc; pass.instance_count = ic; }
      VksPipe pipe;
      vks_pipe_open(&v, &pipe, &pass);
      Draw d = {&pipe, &pass};
      vks_submit(&v, record, &d);
      vks_pipe_close(&v, &pipe);
      if (pass.depth == &own) vks_image_free(&v, &own);
      free(words);
      passes++; last_nt = nt;
      continue;
    }
    fprintf(stderr, "vk-passes: no directive %s\n", a[0]);
    return 1;
  }
  fclose(spec);
  if (!passes || !last_nt) { fprintf(stderr, "vk-passes: the last pass has no colour targets\n"); return 1; }
  FILE *out = fopen(argv[2], "wb");
  if (!out) { fprintf(stderr, "vk-passes: cannot write %s\n", argv[2]); return 1; }
  uint16_t *raw = malloc((size_t)w * h * 8);
  float *f = malloc((size_t)w * h * 16);
  for (uint32_t i = 0; i < (uint32_t)last_nt; i++) {
    vks_read(&v, &targets[i], raw, (size_t)w * h * 8);
    for (size_t k = 0; k < (size_t)w * h * 4; k++) f[k] = half(raw[k]);
    fwrite(f, 16, (size_t)w * h, out);
  }
  fclose(out);
  printf("vk-passes: %s, %ux%u, %d passes, %u targets, %u draws; validation %d errors, %d warnings\n", v.props.deviceName, w, h, passes,
         (uint32_t)last_nt, ndraws, vkc_validation_errors, vkc_validation_warnings);
  return vkc_validation_errors ? 1 : 0;
}
