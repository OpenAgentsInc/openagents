// Passes of a ported shader natively (docs/verse/native-gpu.md, G5), from a
// spec verse/gpu/everglade-pass.coax writes; verse/gpu/golden/pass.js draws
// the same spec in the browser. The spec, a directive a line (`wgsl FILE`,
// the browser's WGSL, is skipped here):
//   size W H                         the colour targets' size
//   buffer NAME FILE                 a buffer the shaders name (bound as storage here)
//   layers NAME FILE W H N LEVELS    an rgba8 sRGB 2D-array texture: level after level, each layer after layer
//   sampler NAME FILTER WRAP MIP CMP linear|nearest, repeat|clamp, mip|flat, none|less
//   depth NAME W H                   a depth32float texture a pass draws and a later one samples
//   target NAME                      an rgba16float colour target at the size, which later passes keep or sample
//   copy FROM TO                     one image into another of its size and format (a depth too)
//   index FILE                       u32 indices for the draw list
//   draws FILE N                     the draw list: N records of u32 first, count, base instance, instances
//   pass SPV VS FS TARGETS DEPTH BIAS SLOPE CALLS
//          [keep] [nowrite] [always] [blend=add|mul|alpha] [alphakeep] [mask=I]
//                                    VS and FS the WGSL's entries (for the browser); SPV a module from coax spirv, SPV.bindings its bindings ("binding I KIND NAME"
//                                    a line, as coax spirv prints them); TARGETS a number of rgba16float colour targets
//                                    of its own (cleared to 0), or target NAMEs joined by commas; DEPTH a depth NAME,
//                                    "own" (one at the size) or "-"; depth tests less and writes; BIAS and SLOPE as
//                                    WebGPU's depthBias and depthBiasSlopeScale; CALLS "draws" (the whole list),
//                                    "draws:A:B" (records A to B, B not included), or V or V:I vertices and instances.
//                                    keep: the targets and depth are loaded, not cleared; nowrite: depth is tested,
//                                    not written; always: the depth test always passes; blend: the colour blended
//                                    (one, one; source over; the target times the source); alphakeep: the targets'
//                                    alpha kept; mask=I: target I left as it is
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
  VksImage targets[8], *last[8];
  uint32_t ntargets = 0, used_draws = 0;
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
    if (!strcmp(a[0], "target")) {
      if (!w) { fprintf(stderr, "vk-passes: target before size\n"); return 1; }
      Res *r = &res[nres++];
      snprintf(r->name, sizeof r->name, "%s", a[1]); r->kind = VKS_IMAGE;
      r->img = vks_image(&v, w, h, 1, 0, VK_FORMAT_R16G16B16A16_SFLOAT, VKS_TARGET | VKS_TEXTURE);
      continue;
    }
    if (!strcmp(a[0], "copy")) { vks_copy(&v, &find(a[1])->img, &find(a[2])->img); continue; }
    if (!strcmp(a[0], "pass")) {
      char tok[16][1024];
      int nt = 0;
      for (char *p = line; nt < 16;) {
        int used = 0;
        if (sscanf(p, "%1023s%n", tok[nt], &used) != 1) break;
        p += used; nt++;
      }
      if (nt < 9) { fprintf(stderr, "vk-passes: pass SPV VS FS TARGETS DEPTH BIAS SLOPE CALLS [options]\n"); return 1; }
      const char *spv = tok[1], *depth = tok[5], *calls = tok[8];
      float bias = (float)atof(tok[6]), slope = (float)atof(tok[7]);
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
      VksPass pass = {.words = words, .count = bytes / 4, .bindings = binds, .nb = nb, .depth_bias = bias, .depth_slope = slope};
      if (tok[4][0] >= '0' && tok[4][0] <= '9') {
        // targets of the pass's own, made once at the size and cleared by each pass
        int k = atoi(tok[4]);
        if (k < 0 || k > 8) { fprintf(stderr, "vk-passes: at most 8 targets\n"); return 1; }
        for (; ntargets < (uint32_t)k; ntargets++) targets[ntargets] = vks_image(&v, w, h, 1, 0, VK_FORMAT_R16G16B16A16_SFLOAT, VKS_TARGET);
        for (int i = 0; i < k; i++) pass.targets[i] = &targets[i];
        pass.ntargets = (uint32_t)k;
      } else {
        for (char *name = strtok(tok[4], ","); name && pass.ntargets < 8; name = strtok(NULL, ",")) pass.targets[pass.ntargets++] = &find(name)->img;
      }
      VksImage own = {0};
      if (!strcmp(depth, "own")) { own = vks_image(&v, w, h, 1, 0, VK_FORMAT_D32_SFLOAT, VKS_TARGET); pass.depth = &own; }
      else if (strcmp(depth, "-")) pass.depth = &find(depth)->img;
      if (pass.depth) { pass.depth_test = 1; pass.depth_write = 1; }
      for (int i = 9; i < nt; i++) {
        const char *o = tok[i];
        if (!strcmp(o, "keep")) pass.keep = 1;
        else if (!strcmp(o, "nowrite")) pass.depth_write = 0;
        else if (!strcmp(o, "always")) pass.depth_always = 1;
        else if (!strcmp(o, "alphakeep")) pass.alpha_keep = 1;
        else if (!strcmp(o, "blend=add")) pass.blend = VKS_ADD;
        else if (!strcmp(o, "blend=mul")) pass.blend = VKS_MUL;
        else if (!strcmp(o, "blend=alpha")) pass.blend = VKS_ALPHA;
        else if (!strncmp(o, "mask=", 5)) pass.masked |= 1u << atoi(o + 5);
        else { fprintf(stderr, "vk-passes: no pass option %s\n", o); return 1; }
      }
      unsigned from = 0, to = ndraws;
      if (!strncmp(calls, "draws", 5)) {
        if (calls[5] == ':' && (sscanf(calls + 6, "%u:%u", &from, &to) != 2 || from > to || to > ndraws)) { fprintf(stderr, "vk-passes: draws:A:B within the list\n"); return 1; }
        pass.draws = draws + from * 4; pass.ndraws = to - from; pass.index = has_index ? &index : NULL;
        used_draws += to - from;
      }
      else { unsigned vc = 0, ic = 1; sscanf(calls, "%u:%u", &vc, &ic); pass.vertex_count = vc; pass.instance_count = ic; }
      VksPipe pipe;
      vks_pipe_open(&v, &pipe, &pass);
      Draw d = {&pipe, &pass};
      vks_submit(&v, record, &d);
      vks_pipe_close(&v, &pipe);
      if (pass.depth == &own) vks_image_free(&v, &own);
      free(words);
      passes++;
      last_nt = (int)pass.ntargets;
      for (uint32_t i = 0; i < pass.ntargets; i++) last[i] = pass.targets[i];
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
    vks_read(&v, last[i], raw, (size_t)w * h * 8);
    for (size_t k = 0; k < (size_t)w * h * 4; k++) f[k] = half(raw[k]);
    fwrite(f, 16, (size_t)w * h, out);
  }
  fclose(out);
  printf("vk-passes: %s, %ux%u, %d passes, %u targets, %u draws; validation %d errors, %d warnings\n", v.props.deviceName, w, h, passes,
         (uint32_t)last_nt, used_draws, vkc_validation_errors, vkc_validation_warnings);
  return vkc_validation_errors ? 1 : 0;
}
