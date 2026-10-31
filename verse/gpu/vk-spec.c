// A pass spec as a graph kept on the GPU (vk-spec.h; the directives are vk-passes.c's).
#include "vk-spec.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXR 48
#define MAXOPS 256
#define MAXLETS 16

typedef struct { char name[64]; int kind; VkcBuffer buf; VksImage img; VkSampler sampler; } Res;
// A count: a number, a named count (`$NAME`, its index), or the record `each` is at.
typedef struct { long k; int let; int each; } Count;
enum { OP_PASS = 1, OP_COPY = 2, OP_EACH = 3, OP_END = 4 };
typedef struct {
  int op;
  VksPass pass;
  VksPipe pipe;
  VksBinding binds[MAXR];
  VksImage own;          // a depth of its own ("own")
  int has_own;
  int draws;             // calls the draw list: from and to
  Count from, to, vc, ic;
  VksImage *a, *b;       // a copy's
  int end;               // an each's end, an end's each
} Op;
struct VkSpec {
  Res res[MAXR];
  int nres;
  Op ops[MAXOPS];
  int nops;
  char lets[MAXLETS][64];
  long values[MAXLETS];
  int nlets;
  uint32_t w, h;
  const uint32_t *draws;
  uint32_t *own_draws;
  Count ndraws;
  VkcBuffer index;
  int has_index;
  VksImage targets[8];
  uint32_t ntargets;
  VksImage *last[8];
  int nlast, passes;
  uint32_t drawn;
};

static Res *find(VkSpec *s, const char *name) {
  for (int i = 0; i < s->nres; i++) if (!strcmp(s->res[i].name, name)) return &s->res[i];
  return NULL;
}
static int let_of(VkSpec *s, const char *name) {
  for (int i = 0; i < s->nlets; i++) if (!strcmp(s->lets[i], name)) return i;
  if (s->nlets == MAXLETS) return -1;
  snprintf(s->lets[s->nlets], 64, "%s", name);
  s->values[s->nlets] = 0;
  return s->nlets++;
}
// A count's text: digits, $NAME, or "each"; 0 if it is none of them.
static int count_of(VkSpec *s, const char *t, size_t n, Count *c) {
  char w[64];
  if (!n || n >= sizeof w) return 0;
  memcpy(w, t, n); w[n] = 0;
  *c = (Count){0, -1, 0};
  if (!strcmp(w, "each")) { c->each = 1; return 1; }
  if (w[0] == '$') { c->let = let_of(s, w + 1); return c->let >= 0; }
  char *e;
  c->k = strtol(w, &e, 10);
  return *e == 0;
}
static long value(const VkSpec *s, Count c, long each) { return c.each ? each : c.let >= 0 ? s->values[c.let] : c.k; }

static void *slurp(const char *path, size_t *n) {
  FILE *f = fopen(path, "rb");
  if (!f) return NULL;
  fseek(f, 0, SEEK_END);
  long size = ftell(f);
  fseek(f, 0, SEEK_SET);
  void *p = malloc((size_t)size + 16);
  if (fread(p, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(p); return NULL; }
  fclose(f);
  *n = (size_t)size;
  return p;
}
// A file's bytes, from the host or the disk (freed with `done` if it is ours).
static const void *bytes_of(const VkSpecFiles *files, const char *name, size_t *size, size_t *cap, void **done) {
  *done = NULL;
  if (files && files->file) {
    const void *p = files->file(files->user, name, size, cap);
    if (p) return p;
  }
  void *p = slurp(name, size);
  if (!p) { fprintf(stderr, "vk-spec: cannot read %s\n", name); return NULL; }
  *cap = *size;
  *done = p;
  return p;
}
static int mode(const char *m) {
  static const char *names[] = {"", "add", "alpha", "mul", "over", "under", "keep"};
  for (int i = 1; i < 7; i++) if (!strcmp(m, names[i])) return i;
  return 0;
}

#define FAIL(...) do { fprintf(stderr, "vk-spec: " __VA_ARGS__); fprintf(stderr, "\n"); goto fail; } while (0)
VkSpec *vk_spec_open(Vkc *v, const char *path, const VkSpecFiles *files) {
  FILE *spec = fopen(path, "r");
  if (!spec) { fprintf(stderr, "vk-spec: cannot read %s\n", path); return NULL; }
  VkSpec *s = calloc(1, sizeof *s);
  int open_each = -1;
  char line[4096];
  while (fgets(line, sizeof line, spec)) {
    char a[6][1024] = {{0}};
    int n = sscanf(line, "%1023s %1023s %1023s %1023s %1023s %1023s", a[0], a[1], a[2], a[3], a[4], a[5]);
    if (n <= 0 || a[0][0] == '#') continue;
    if (!strcmp(a[0], "wgsl")) continue;   // the browser's
    if (!strcmp(a[0], "size")) {
      s->w = files && files->w ? files->w : (uint32_t)atoi(a[1]);
      s->h = files && files->h ? files->h : (uint32_t)atoi(a[2]);
      continue;
    }
    if (!strcmp(a[0], "let")) {
      int k = let_of(s, a[1]);
      if (k < 0 || n < 3) FAIL("let NAME V, at most %d", MAXLETS);
      s->values[k] = atol(a[2]);
      continue;
    }
    if (!strcmp(a[0], "draws")) {
      if (!count_of(s, a[2], strlen(a[2]), &s->ndraws)) FAIL("draws FILE N");
      size_t size, cap;
      void *done;
      const void *data = bytes_of(files, a[1], &size, &cap, &done);
      if (!data) goto fail;
      if (done) { s->own_draws = done; s->draws = done; }
      else s->draws = data;
      if (s->ndraws.let < 0 && (size_t)s->ndraws.k * 16 > size) FAIL("a short draw list");
      continue;
    }
    if (!strcmp(a[0], "index") || !strcmp(a[0], "buffer")) {
      const char *file = !strcmp(a[0], "buffer") ? a[2] : a[1];
      size_t size, cap;
      void *done;
      const void *data = bytes_of(files, file, &size, &cap, &done);
      if (!data) goto fail;
      if (cap < size) cap = size;
      VkcBuffer b = vkc_buffer(v, cap < 16 ? 16 : (cap + 3) & ~(size_t)3);
      memcpy(b.map, data, size);
      free(done);
      if (!strcmp(a[0], "index")) { s->index = b; s->has_index = 1; continue; }
      if (s->nres == MAXR) FAIL("at most %d resources", MAXR);
      Res *r = &s->res[s->nres++];
      snprintf(r->name, sizeof r->name, "%.63s", a[1]); r->kind = VKS_STORAGE; r->buf = b;
      continue;
    }
    if (s->nres == MAXR) FAIL("at most %d resources", MAXR);
    if (!strcmp(a[0], "layers")) {
      int lw, lh, ln, levels;
      if (sscanf(line, "%*s %*s %*s %d %d %d %d", &lw, &lh, &ln, &levels) != 4) FAIL("layers NAME FILE W H N LEVELS");
      size_t size, cap;
      void *done;
      const void *data = bytes_of(files, a[2], &size, &cap, &done);
      if (!data) goto fail;
      Res *r = &s->res[s->nres++];
      snprintf(r->name, sizeof r->name, "%.63s", a[1]); r->kind = VKS_IMAGE;
      r->img = vks_image_levels(v, (uint32_t)lw, (uint32_t)lh, (uint32_t)ln, 1, VK_FORMAT_R8G8B8A8_SRGB, VKS_TEXTURE, (uint32_t)levels);
      vks_upload(v, &r->img, data, size);
      free(done);
      continue;
    }
    if (!strcmp(a[0], "sampler")) {
      Res *r = &s->res[s->nres++];
      snprintf(r->name, sizeof r->name, "%.63s", a[1]); r->kind = VKS_SAMPLER;
      char cmp[64] = "none";
      sscanf(line, "%*s %*s %*s %*s %*s %63s", cmp);
      r->sampler = vks_sampler_with(v, strcmp(a[2], "linear") ? VKS_NEAREST : VKS_LINEAR, !strcmp(a[3], "repeat"), strcmp(cmp, "none") != 0,
                                    VK_COMPARE_OP_LESS, !strcmp(a[4], "mip"));
      continue;
    }
    if (!strcmp(a[0], "depth")) {
      Res *r = &s->res[s->nres++];
      snprintf(r->name, sizeof r->name, "%.63s", a[1]); r->kind = VKS_IMAGE;
      // W H, or "frame": the colour targets' size (a host's, when it sizes them)
      uint32_t dw = (uint32_t)atoi(a[2]), dh = (uint32_t)atoi(a[3]);
      if (!strcmp(a[2], "frame")) { if (!s->w) FAIL("depth NAME frame before size"); dw = s->w; dh = s->h; }
      r->img = vks_image(v, dw, dh, 1, 0, VK_FORMAT_D32_SFLOAT, VKS_TARGET | VKS_TEXTURE);
      continue;
    }
    if (!strcmp(a[0], "target")) {
      if (!s->w) FAIL("target before size");
      Res *r = &s->res[s->nres++];
      snprintf(r->name, sizeof r->name, "%.63s", a[1]); r->kind = VKS_IMAGE;
      // W H, or a fraction of the frame's size, /K (rounded up), as a host sized it
      int tw = (int)s->w, th = (int)s->h, k = 0;
      if (sscanf(line, "%*s %*s /%d", &k) == 1 && k > 0) { tw = ((int)s->w + k - 1) / k; th = ((int)s->h + k - 1) / k; }
      else sscanf(line, "%*s %*s %d %d", &tw, &th);
      r->img = vks_image(v, (uint32_t)tw, (uint32_t)th, 1, 0, VK_FORMAT_R16G16B16A16_SFLOAT, VKS_TARGET | VKS_TEXTURE);
      continue;
    }
    if (!strcmp(a[0], "floats")) {
      float f[64];
      int nf = 0, used = 0;
      char *p = line;
      sscanf(p, "%*s %*s%n", &used); p += used;
      while (nf < 64 && sscanf(p, "%f%n", &f[nf], &used) == 1) { p += used; nf++; }
      VkcBuffer b = vkc_buffer(v, nf * 4 < 16 ? 16 : (size_t)nf * 4);
      memcpy(b.map, f, (size_t)nf * 4);
      Res *r = &s->res[s->nres++];
      snprintf(r->name, sizeof r->name, "%.63s", a[1]); r->kind = VKS_STORAGE; r->buf = b;
      continue;
    }
    if (s->nops == MAXOPS) FAIL("at most %d passes and copies", MAXOPS);
    Op *op = &s->ops[s->nops];
    if (!strcmp(a[0], "copy")) {
      Res *x = find(s, a[1]), *y = find(s, a[2]);
      if (!x || !y) FAIL("the spec has no %s", x ? a[2] : a[1]);
      op->op = OP_COPY; op->a = &x->img; op->b = &y->img;
      s->nops++;
      continue;
    }
    if (!strcmp(a[0], "each")) {
      if (open_each >= 0 || strncmp(a[1], "draws:", 6)) FAIL("each draws:A:B, not nested");
      const char *c = a[1] + 6, *colon = strchr(c, ':');
      if (!colon || !count_of(s, c, (size_t)(colon - c), &op->from) || !count_of(s, colon + 1, strlen(colon + 1), &op->to)) FAIL("each draws:A:B");
      op->op = OP_EACH;
      open_each = s->nops++;
      continue;
    }
    if (!strcmp(a[0], "end")) {
      if (open_each < 0) FAIL("end without each");
      op->op = OP_END; op->end = open_each; s->ops[open_each].end = s->nops;
      open_each = -1;
      s->nops++;
      continue;
    }
    if (!strcmp(a[0], "pass")) {
      char tok[16][1024];
      int nt = 0;
      for (char *p = line; nt < 16;) {
        int used = 0;
        if (sscanf(p, "%1023s%n", tok[nt], &used) != 1) break;
        p += used; nt++;
      }
      if (nt < 9) FAIL("pass SPV VS FS TARGETS DEPTH BIAS SLOPE CALLS [options]");
      const char *spv = tok[1], *depth = tok[5], *calls = tok[8];
      size_t bytes;
      uint32_t *words = slurp(spv, &bytes);
      if (!words) FAIL("cannot read %s", spv);
      char bpath[1100];
      snprintf(bpath, sizeof bpath, "%s.bindings", spv);
      FILE *bf = fopen(bpath, "r");
      if (!bf) { free(words); FAIL("no %s", bpath); }
      uint32_t nb = 0;
      char bl[512];
      while (fgets(bl, sizeof bl, bf)) {
        int i; char kind[64], name[128];
        if (sscanf(bl, "binding %d %63s %127s", &i, kind, name) != 3) continue;
        // a bind=NAME:RES option names what NAME is in this pass
        for (int k = 9; k < nt; k++) {
          size_t ln = strlen(name);
          if (!strncmp(tok[k], "bind=", 5) && !strncmp(tok[k] + 5, name, ln) && tok[k][5 + ln] == ':') snprintf(name, sizeof name, "%s", tok[k] + 6 + ln);
        }
        Res *r = find(s, name);
        if (!r) { fclose(bf); free(words); FAIL("the spec has no %s", name); }
        if ((uint32_t)i != nb || nb == MAXR) { fclose(bf); free(words); FAIL("%s's bindings are not in order", bpath); }
        op->binds[nb++] = r->kind == VKS_STORAGE ? (VksBinding){VKS_STORAGE, &r->buf} : r->kind == VKS_IMAGE ? (VksBinding){VKS_IMAGE, .image = &r->img}
                                                                                         : (VksBinding){VKS_SAMPLER, .sampler = r->sampler};
      }
      fclose(bf);
      VksPass *pass = &op->pass;
      *pass = (VksPass){.words = words, .count = bytes / 4, .bindings = op->binds, .nb = nb, .depth_bias = (float)atof(tok[6]), .depth_slope = (float)atof(tok[7])};
      if (tok[4][0] >= '0' && tok[4][0] <= '9') {
        // targets of the spec's own, made once at the size and cleared by each pass
        int k = atoi(tok[4]);
        if (k < 0 || k > 8) { free(words); FAIL("at most 8 targets"); }
        for (; s->ntargets < (uint32_t)k; s->ntargets++) s->targets[s->ntargets] = vks_image(v, s->w, s->h, 1, 0, VK_FORMAT_R16G16B16A16_SFLOAT, VKS_TARGET);
        for (int i = 0; i < k; i++) pass->targets[i] = &s->targets[i];
        pass->ntargets = (uint32_t)k;
      } else {
        for (char *name = strtok(tok[4], ","); name && pass->ntargets < 8; name = strtok(NULL, ",")) {
          Res *r = find(s, name);
          if (!r) { free(words); FAIL("the spec has no %s", name); }
          pass->targets[pass->ntargets++] = &r->img;
        }
      }
      if (!strcmp(depth, "own")) { op->own = vks_image(v, s->w, s->h, 1, 0, VK_FORMAT_D32_SFLOAT, VKS_TARGET); op->has_own = 1; pass->depth = &op->own; }
      else if (strcmp(depth, "-")) {
        Res *r = find(s, depth);
        if (!r) { free(words); FAIL("the spec has no %s", depth); }
        pass->depth = &r->img;
      }
      if (pass->depth) { pass->depth_test = 1; pass->depth_write = 1; }
      for (int i = 9; i < nt; i++) {
        const char *o = tok[i];
        if (!strcmp(o, "keep")) pass->keep = 1;
        else if (!strcmp(o, "nowrite")) pass->depth_write = 0;
        else if (!strcmp(o, "always")) pass->depth_always = 1;
        else if (!strncmp(o, "blend=", 6) && mode(o + 6) > 0) pass->blend = mode(o + 6);
        else if (!strncmp(o, "alpha=", 6) && mode(o + 6) > 0) pass->alpha_blend = mode(o + 6);
        else if (!strncmp(o, "mask=", 5)) pass->masked |= 1u << atoi(o + 5);
        else if (!strncmp(o, "bind=", 5)) continue;
        else { free(words); FAIL("no pass option %s", o); }
      }
      if (!strncmp(calls, "draws", 5)) {
        op->draws = 1;
        op->from = (Count){0, -1, 0}; op->to = s->ndraws;
        if (!strcmp(calls + 5, ":each")) { op->from = (Count){0, -1, 1}; op->to = (Count){1, -1, 1}; }
        else if (calls[5] == ':') {
          const char *c = calls + 6, *colon = strchr(c, ':');
          if (!colon || !count_of(s, c, (size_t)(colon - c), &op->from) || !count_of(s, colon + 1, strlen(colon + 1), &op->to)) { free(words); FAIL("draws:A:B"); }
        } else if (calls[5]) { free(words); FAIL("draws, draws:A:B or draws:each"); }
        pass->index = s->has_index ? &s->index : NULL;
      } else {
        const char *colon = strchr(calls, ':');
        op->ic = (Count){1, -1, 0};
        if (!count_of(s, calls, colon ? (size_t)(colon - calls) : strlen(calls), &op->vc) || (colon && !count_of(s, colon + 1, strlen(colon + 1), &op->ic))) {
          free(words); FAIL("calls V or V:I");
        }
        // the pipeline is made with some vertices to draw; the record sets how many
        pass->vertex_count = 1; pass->instance_count = 1;
      }
      vks_pipe_open(v, &op->pipe, pass);
      free(words);
      pass->words = NULL;
      op->op = OP_PASS;
      s->nops++;
      s->passes++;
      s->nlast = (int)pass->ntargets;
      for (uint32_t i = 0; i < pass->ntargets; i++) s->last[i] = pass->targets[i];
      continue;
    }
    FAIL("no directive %s", a[0]);
  }
  fclose(spec);
  spec = NULL;
  if (open_each >= 0) FAIL("each without end");
  if (!s->passes || !s->nlast) FAIL("the last pass has no colour targets");
  return s;
fail:
  if (spec) fclose(spec);
  vk_spec_close(v, s);
  return NULL;
}

int vk_spec_let(VkSpec *s, const char *name, long value) {
  for (int i = 0; i < s->nlets; i++) if (!strcmp(s->lets[i], name)) { s->values[i] = value; return 1; }
  return 0;
}
VkcBuffer *vk_spec_buffer(VkSpec *s, const char *name) {
  if (!strcmp(name, "index")) return s->has_index ? &s->index : NULL;
  Res *r = find(s, name);
  return r && r->kind == VKS_STORAGE ? &r->buf : NULL;
}
void vk_spec_draws(VkSpec *s, const uint32_t *draws) { s->draws = draws; }

static void record_pass(VkSpec *s, Op *op, VkCommandBuffer cmd, long each) {
  VksPass *p = &op->pass;
  p->vertex_count = 0; p->ndraws = 0;
  if (op->draws) {
    long all = value(s, s->ndraws, each), from = value(s, op->from, each), to = op->to.each ? each + 1 : value(s, op->to, each);
    if (from < 0) from = 0;
    if (to > all) to = all;
    if (from < to && s->draws) { p->draws = s->draws + from * 4; p->ndraws = (uint32_t)(to - from); s->drawn += p->ndraws; }
  } else {
    p->vertex_count = (uint32_t)value(s, op->vc, each);
    p->instance_count = (uint32_t)value(s, op->ic, each);
    if (!p->instance_count) p->vertex_count = 0;
  }
  // a pass with nothing to draw still clears what it does not keep
  vks_pipe_record(&op->pipe, cmd, p);
}
void vk_spec_record(VkSpec *s, VkCommandBuffer cmd) {
  s->drawn = 0;
  for (int i = 0; i < s->nops; i++) {
    Op *op = &s->ops[i];
    if (op->op == OP_COPY) vks_copy_record(cmd, op->a, op->b);
    else if (op->op == OP_PASS) record_pass(s, op, cmd, 0);
    else if (op->op == OP_EACH) {
      long all = value(s, s->ndraws, 0), from = value(s, op->from, 0), to = value(s, op->to, 0);
      if (to > all) to = all;
      for (long r = from < 0 ? 0 : from; r < to; r++)
        for (int k = i + 1; k < op->end; k++) {
          Op *b = &s->ops[k];
          if (b->op == OP_PASS) record_pass(s, b, cmd, r);
          else if (b->op == OP_COPY) vks_copy_record(cmd, b->a, b->b);
        }
      i = op->end;
    }
  }
}
VksImage **vk_spec_last(VkSpec *s, int *n) { *n = s->nlast; return s->last; }
uint32_t vk_spec_width(const VkSpec *s) { return s->w; }
uint32_t vk_spec_height(const VkSpec *s) { return s->h; }
int vk_spec_passes(const VkSpec *s) { return s->passes; }
uint32_t vk_spec_drawn(const VkSpec *s) { return s->drawn; }

void vk_spec_close(Vkc *v, VkSpec *s) {
  if (!s) return;
  vkDeviceWaitIdle(v->dev);
  for (int i = 0; i < s->nops; i++) {
    Op *op = &s->ops[i];
    if (op->op != OP_PASS) continue;
    vks_pipe_close(v, &op->pipe);
    if (op->has_own) vks_image_free(v, &op->own);
  }
  for (int i = 0; i < s->nres; i++) {
    Res *r = &s->res[i];
    if (r->kind == VKS_STORAGE) vkc_free(v, &r->buf);
    else if (r->kind == VKS_IMAGE) vks_image_free(v, &r->img);
    else if (r->kind == VKS_SAMPLER) vkDestroySampler(v->dev, r->sampler, NULL);
  }
  for (uint32_t i = 0; i < s->ntargets; i++) vks_image_free(v, &s->targets[i]);
  if (s->has_index) vkc_free(v, &s->index);
  free(s->own_draws);
  free(s);
}
