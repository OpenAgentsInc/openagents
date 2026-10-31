// Passes of a ported shader natively (docs/verse/native-gpu.md, G5), from a
// spec verse/gpu/everglade-pass.coax writes; verse/gpu/golden/pass.js draws
// the same spec in the browser. The spec, a directive a line (`wgsl FILE`,
// the browser's WGSL, is skipped here):
//   size W H                         the colour targets' size
//   buffer NAME FILE                 a buffer the shaders name (bound as storage here)
//   layers NAME FILE W H N LEVELS    an rgba8 sRGB 2D-array texture: level after level, each layer after layer
//   sampler NAME FILTER WRAP MIP CMP linear|nearest, repeat|clamp, mip|flat, none|less
//   depth NAME W H                   a depth32float texture a pass draws and a later one samples (W H "frame": the size's)
//   target NAME [W H | /K]           an rgba16float colour target (at the size, W by H, or the size over K rounded up),
//                                    which later passes keep or sample
//   floats NAME V...                 a buffer of the floats (a WGSL uniform, bound as storage here)
//   copy FROM TO                     one image into another of its size and format (a depth too)
//   index FILE                       u32 indices for the draw list
//   draws FILE N                     the draw list: N records of u32 first, count, base instance, instances
//   let NAME V                       a count, used as $NAME for a number in a pass's calls or the draw list's N
//   each draws:A:B ... end           the directives between once for each of the list's records A to B (draws:each: that one)
//   pass SPV VS FS TARGETS DEPTH BIAS SLOPE CALLS
//          [keep] [nowrite] [always] [blend=MODE] [alpha=MODE] [mask=I] [bind=NAME:RES]... [vertices=NAME:FLOATS]
//                                    VS and FS the WGSL's entries (for the browser); SPV a module from coax spirv, SPV.bindings its bindings ("binding I KIND NAME"
//                                    a line, as coax spirv prints them); TARGETS a number of rgba16float colour targets
//                                    of its own (cleared to 0), or target NAMEs joined by commas; DEPTH a depth NAME,
//                                    "own" (one at the size) or "-"; depth tests less and writes; BIAS and SLOPE as
//                                    WebGPU's depthBias and depthBiasSlopeScale; CALLS "draws" (the whole list),
//                                    "draws:A:B" (records A to B, B not included), "draws:each", or V or V:I vertices and instances.
//                                    keep: the targets and depth are loaded, not cleared; nowrite: depth is tested,
//                                    not written; always: the depth test always passes; blend: the colour blended,
//                                    a MODE of vks.h's: add, alpha, mul, over, under, keep or max; alpha: the alpha's
//                                    (else as blend's); mask=I: target I left as it is; bind: the shader's NAME is RES
//                                    in this pass; vertices: buffer NAME holds the vertex entry's attributes, FLOATS
//                                    floats a vertex, one vec4f per four
// The last pass's colour targets (it must have some) are written to OUT as float32 RGBA rows, target after target.
//   vk-passes SPEC OUT
#include "vk-spec.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

static float half(uint16_t h) {
  int s = h >> 15, e = (h >> 10) & 31, m = h & 1023;
  float v = e == 0 ? ldexpf((float)m, -24) : e == 31 ? (m ? NAN : INFINITY) : ldexpf(1.0f + m / 1024.0f, e - 15);
  return s ? -v : v;
}
static void record(VkCommandBuffer cmd, void *user) { vk_spec_record(user, cmd); }

int main(int argc, char **argv) {
  if (argc != 3) { fprintf(stderr, "usage: vk-passes SPEC OUT\n"); return 2; }
  Vkc v;
  vkc_open(&v);
  VkSpec *s = vk_spec_open(&v, argv[1], NULL);
  if (!s) return 1;
  // the whole spec in one command buffer, as a host draws a frame
  vks_submit(&v, record, s);
  uint32_t w = vk_spec_width(s), h = vk_spec_height(s);
  int nt;
  VksImage **last = vk_spec_last(s, &nt);
  FILE *out = fopen(argv[2], "wb");
  if (!out) { fprintf(stderr, "vk-passes: cannot write %s\n", argv[2]); return 1; }
  uint16_t *raw = malloc((size_t)w * h * 8);
  float *f = malloc((size_t)w * h * 16);
  for (int i = 0; i < nt; i++) {
    vks_read(&v, last[i], raw, (size_t)w * h * 8);
    for (size_t k = 0; k < (size_t)w * h * 4; k++) f[k] = half(raw[k]);
    fwrite(f, 16, (size_t)w * h, out);
  }
  fclose(out);
  printf("vk-passes: %s, %ux%u, %d passes, %d targets, %u draws; validation %d errors, %d warnings\n", v.props.deviceName, w, h, vk_spec_passes(s),
         nt, vk_spec_drawn(s), vkc_validation_errors, vkc_validation_warnings);
  vk_spec_close(&v, s);
  return vkc_validation_errors ? 1 : 0;
}
