/* The frame plan's walk for C hosts (GitHub #13303, agreed with the host core on #13304).
 *
 * A page's engine hands its host the frame's operations through
 *   uint32_t coaxial_frame_plan_v1(state, uint32_t features, uint32_t *out, uint32_t cap)
 * (metaverse-engine/render/frame-plan.coax states the records; passes.coax decides the order).
 * This file walks those records and calls the host back once an operation, in order. The host
 * maps a pass to its pipeline and bindings (Metal in PageHost.m, Vulkan on Linux and Android);
 * it decides no order and names no pass of its own.
 *
 *   uint32_t words[AETHYR_FP_WORDS * (AETHYR_FP_MOST_OPS + 1)];
 *   uint32_t n = coaxial_frame_plan_v1(state, bits, words, sizeof words / 4);
 *   if (aethyr_frame_plan_walk(words, n, &host) < 0) { ... the host's own order, as before ... }
 *
 * The whole plan is checked before the first call: a plan of another version, a short buffer, an
 * operation or pass this header does not know, or passes that do not nest gives -1 and no call at
 * all, so a host never draws half a frame or walks a newer plan wrong. The numbers come from
 * frame-plan.h, which frame-plan.coax writes; this file adds none.
 *
 * Checked by metaverse-engine/host/core/check-frame-plan-walk.sh (check.sh reports, frame-plan-walk):
 * Coax writes plans and what a walk of each must call, and this walk must call exactly that. */
#ifndef AETHYR_FRAME_PLAN_WALK_H
#define AETHYR_FRAME_PLAN_WALK_H
#include <stdint.h>
#include "../frame-plan.h"

typedef struct aethyr_fp_op {
  uint32_t kind, pass, source, depth_test;
  uint32_t colour0, colour1, depth, from;     /* targets; `from` a copy's or a bloom draw's source */
  uint32_t to, colour_blend, alpha_blend, outputs;
  uint32_t flags, bias, slope;                /* AETHYR_FP_FLAG_* */
  uint32_t first, count, instances;
  uint32_t step[4];                           /* a bloom draw's step */
} aethyr_fp_op;

/* The host's side. Every member must be set. `item` is the draw of the list a `current` draw
 * stands for (inside an each loop), 0 otherwise. A rectangle of no width is the whole target. */
typedef struct aethyr_fp_host {
  void *self;
  void (*begin)(void *self, const aethyr_fp_op *op);
  void (*end)(void *self);
  void (*draw)(void *self, const aethyr_fp_op *op, uint32_t item);
  void (*copy)(void *self, uint32_t from, uint32_t to);
  void (*rect)(void *self, int scissor, uint32_t x, uint32_t y, uint32_t w, uint32_t h);
  void (*compute)(void *self, uint32_t kind, uint32_t runs);   /* CULL_EARLY, CULL_LATE, PARTICLE_SORT */
  void (*media)(void *self, int after);
} aethyr_fp_host;

/* The operations in a written plan, or -1 (frame-plan.coax frame_plan_count). */
static inline int32_t aethyr_frame_plan_count(const uint32_t *words, uint32_t n_words) {
  if (!words || n_words < AETHYR_FP_WORDS) return -1;
  if (words[0] != AETHYR_FP_MAGIC || words[1] != AETHYR_FP_VERSION || words[2] != AETHYR_FP_WORDS) return -1;
  if (words[3] > AETHYR_FP_MOST_OPS || n_words / AETHYR_FP_WORDS < words[3] + 1u) return -1;
  return (int32_t)words[3];
}

static inline aethyr_fp_op aethyr_frame_plan_op(const uint32_t *words, uint32_t i) {
  const uint32_t *w = words + (i + 1u) * AETHYR_FP_WORDS;
  aethyr_fp_op o;
  o.kind = w[0] & 255u; o.pass = (w[0] >> 8) & 255u; o.source = (w[0] >> 16) & 255u; o.depth_test = w[0] >> 24;
  o.colour0 = w[1] & 255u; o.colour1 = (w[1] >> 8) & 255u; o.depth = (w[1] >> 16) & 255u; o.from = w[1] >> 24;
  o.to = w[2] & 255u; o.colour_blend = (w[2] >> 8) & 255u; o.alpha_blend = (w[2] >> 16) & 255u; o.outputs = w[2] >> 24;
  o.flags = w[3] & 255u; o.bias = (w[3] >> 8) & 255u; o.slope = (w[3] >> 16) & 255u;
  o.first = w[4]; o.count = w[5]; o.instances = w[6];
  for (int k = 0; k < 4; k++) o.step[k] = (w[7] >> (8 * k)) & 255u;
  return o;
}

static inline int aethyr_fp_pass_known(uint32_t p) { return p <= AETHYR_FP_PASS_HUD || (p >= AETHYR_FP_PASS_SHADOW_CLEAR && p <= AETHYR_FP_PASS_SCENE_LATE); }

/* Whether the plan can be walked: every operation known, passes opened and closed in turn, an each
 * loop inside one pass holding only draws. */
static inline int aethyr_frame_plan_ok(const uint32_t *words, uint32_t n_words) {
  int32_t n = aethyr_frame_plan_count(words, n_words);
  if (n < 0) return 0;
  int in_pass = 0, in_each = 0;
  for (int32_t i = 0; i < n; i++) {
    aethyr_fp_op o = aethyr_frame_plan_op(words, (uint32_t)i);
    if (o.colour0 > AETHYR_FP_TARGET_E1 || o.colour1 > AETHYR_FP_TARGET_E1 || o.depth > AETHYR_FP_TARGET_E1 || o.from > AETHYR_FP_TARGET_E1 || o.to > AETHYR_FP_TARGET_E1) return 0;
    switch (o.kind) {
      case AETHYR_FP_OP_BEGIN: if (in_pass) return 0; in_pass = 1; break;
      case AETHYR_FP_OP_END: if (!in_pass || in_each) return 0; in_pass = 0; break;
      case AETHYR_FP_OP_DRAW: if (!in_pass || !aethyr_fp_pass_known(o.pass) || o.source > AETHYR_FP_SOURCE_LATE) return 0; break;
      case AETHYR_FP_OP_COPY: case AETHYR_FP_OP_CULL_EARLY: case AETHYR_FP_OP_CULL_LATE: case AETHYR_FP_OP_PARTICLE_SORT: if (in_pass) return 0; break;
      case AETHYR_FP_OP_EACH_BEGIN: if (!in_pass || in_each) return 0; in_each = 1; break;
      case AETHYR_FP_OP_EACH_END: if (!in_each) return 0; in_each = 0; break;
      case AETHYR_FP_OP_MEDIA_BEFORE: case AETHYR_FP_OP_MEDIA_AFTER: case AETHYR_FP_OP_VIEWPORT: case AETHYR_FP_OP_SCISSOR: if (!in_pass || in_each) return 0; break;
      default: return 0;
    }
    if (in_each && o.kind != AETHYR_FP_OP_EACH_BEGIN && o.kind != AETHYR_FP_OP_DRAW) return 0;
  }
  return !in_pass && !in_each;
}

/* Walk the plan. The operations walked, or -1 with nothing called. */
static inline int32_t aethyr_frame_plan_walk(const uint32_t *words, uint32_t n_words, const aethyr_fp_host *host) {
  if (!host || !aethyr_frame_plan_ok(words, n_words)) return -1;
  int32_t n = (int32_t)words[3];
  for (int32_t i = 0; i < n; i++) {
    aethyr_fp_op o = aethyr_frame_plan_op(words, (uint32_t)i);
    switch (o.kind) {
      case AETHYR_FP_OP_BEGIN: host->begin(host->self, &o); break;
      case AETHYR_FP_OP_END: host->end(host->self); break;
      case AETHYR_FP_OP_DRAW: host->draw(host->self, &o, 0); break;
      case AETHYR_FP_OP_COPY: host->copy(host->self, o.from, o.to); break;
      case AETHYR_FP_OP_CULL_EARLY: case AETHYR_FP_OP_CULL_LATE: case AETHYR_FP_OP_PARTICLE_SORT: host->compute(host->self, o.kind, o.count); break;
      case AETHYR_FP_OP_VIEWPORT: case AETHYR_FP_OP_SCISSOR:
        host->rect(host->self, o.kind == AETHYR_FP_OP_SCISSOR, o.first & 65535u, o.first >> 16, o.count & 65535u, o.count >> 16); break;
      case AETHYR_FP_OP_MEDIA_BEFORE: host->media(host->self, 0); break;
      case AETHYR_FP_OP_MEDIA_AFTER: host->media(host->self, 1); break;
      case AETHYR_FP_OP_EACH_BEGIN: {
        /* each draw of the list's range in turn through the operations up to each_end */
        int32_t end = i + 1;
        while ((words[(end + 1) * AETHYR_FP_WORDS] & 255u) != AETHYR_FP_OP_EACH_END) end++;
        for (uint32_t d = 0; d < o.count; d++)
          for (int32_t k = i + 1; k < end; k++) { aethyr_fp_op inner = aethyr_frame_plan_op(words, (uint32_t)k); host->draw(host->self, &inner, o.first + d); }
        i = end;
        break;
      }
      default: break;
    }
  }
  return n;
}
#endif
