// Written by metaverse-engine/tools/host-core-flat.coax from metaverse-engine/host/core/host-core.h and
// the headers it includes. Don't edit it: change those, then from the repository root run
//   coax/web/tool.sh metaverse-engine/tools/host-core-flat.coax "$PWD"
// The one C host core (GitHub #13304): what every native host does between a page's face and its
// platform, written once. A host (Linux: metaverse/native/realm-host.c; iOS and the Mac:
// metaverse-engine/host/apple/PageHost.m; Android: apps/everglade-mobile/android/host.c) keeps its
// window, its GPU and its files, and calls this for:
//   the pack          its entries sorted by kind, through the one reader (../pack-table.h)
//   delivery          the pack's parts handed to the face in the browser's order (coax/web/host.js)
//   messages          bytes into the face's inbox, cut at its capacity
//   paged packs       a VPK3 file read whole into its pack image                      [face]
//   block layers      a layer's GPU blocks and their levels, told from an image       [face]
//   the format        the block build a device's GPU takes, by the builds it has      [face]
//   the frame plan    coaxial_frame_plan_v1's records, walked whole or not at all by frame-plan-walk.h
// The core decides nothing the engine decides: each rule is a call into Coax. The base (pack,
// delivery, messages) needs only the pack reader's face. The parts marked [face] need the core's
// own face, metaverse-engine/host/core/core-face.coax as C; a host that has compiled it defines
// ENGINE_CORE_FACE and includes its header first:
//   coax/bin/coax c metaverse-engine/host/core/core-face.coax --library --header aethyr-core-face.h > aethyr-core-face.c
//   #include "aethyr-core-face.h"
//   #define ENGINE_CORE_FACE
//   #include "metaverse-engine/host/core/host-core.h"
// The core face carries the pack reader's exports too, so that host compiles no aethyr-pack-table.c.
//
// Every function is static: a host that includes this file links nothing more than its face.
// The conformance suite (conformance.coax and conformance.c beside this file) runs these same
// functions over fixtures Coax writes and compares what they report with what Coax says holds.
#ifndef ENGINE_HOST_CORE_H
#define ENGINE_HOST_CORE_H
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
// ---- metaverse-engine/host/core/../pack-table.h
// Firm reason: 5 (a thin wrapper with no logic: C spellings over the Coax pack reader's face).
// The one pack reader for a C, Objective-C or Swift host (GitHub #13304). The reader is Coax:
// pack_table in metaverse-engine/host/pack-table.coax, whose C face the host's build compiles as
//   coax c metaverse-engine/host/pack-table-face.coax --library --header aethyr-pack-table.h > aethyr-pack-table.c
// A host hands it the pack's bytes and gets each entry's kind, place and length, every extent
// already checked against the pack's size. No host reads the header or the table itself.
#ifndef ENGINE_PACK_TABLE_H
#define ENGINE_PACK_TABLE_H
#include <stddef.h>
#include <stdint.h>
// A host whose unit already holds the face's C (metaverse/native/realm-host.c includes the committed
// pack-table-gen.h; a host on the host core includes aethyr-core-face.h) defines this first.
#ifndef ENGINE_PACK_TABLE_PROVIDED
#include "aethyr-pack-table.h"
#endif

// The kinds a host sorts entries by (metaverse-engine/host/pack-table.coax PACK_CATALOG to PACK_CELLS).
enum { ENGINE_PACK_CATALOG = 1, ENGINE_PACK_VERTICES = 2, ENGINE_PACK_INDICES = 3, ENGINE_PACK_LAYER = 4, ENGINE_PACK_SOUND = 5, ENGINE_PACK_DATA = 6, ENGINE_PACK_RIG = 7, ENGINE_PACK_CELLS = 8 };

typedef coax_t_PackTable EnginePack;

// Reads the table of the whole pack at bytes[0..<n]. 1 when it is a pack; 0 with engine_pack_why.
static inline int engine_pack_open(EnginePack *pack, const void *bytes, size_t n) {
  return engine_pack_table((coax_string){(const char *)bytes, (int64_t)n}, (int64_t)n, (coax_v_t_PackTable){pack, 1}) ? 1 : 0;
}
// Why the last read was refused, not NUL-terminated: its bytes and their count.
static inline const char *engine_pack_why(const EnginePack *pack, size_t *count) { *count = (size_t)pack->why.count; return pack->why.data; }
static inline int engine_pack_count(const EnginePack *pack) { return (int)pack->count; }
static inline int engine_pack_kind(const EnginePack *pack, int k) { return (int)pack->entries.e[k].kind; }
static inline size_t engine_pack_at(const EnginePack *pack, int k) { return (size_t)pack->entries.e[k].at; }
static inline size_t engine_pack_bytes(const EnginePack *pack, int k) { return (size_t)pack->entries.e[k].n; }
// An entry's name, not NUL-terminated: its bytes and their count (at most 32).
static inline const char *engine_pack_name(const EnginePack *pack, int k, size_t *count) { *count = (size_t)pack->entries.e[k].name.count; return pack->entries.e[k].name.data; }
// A world cell's content in the whole pack at bytes[0..<n] (metaverse-engine/host/pack-cells.coax, #13304): its
// digest and how many byte ranges of the pack it is, or 0 ranges when the catalog has no one good
// line for the cell. key is engine_pack_cell_key(cx, cz). Range k is at[k], bytes[k].
typedef coax_t_PackCell EnginePackCell;
static inline int engine_pack_cell_ranges(EnginePackCell *cell, const void *bytes, size_t n, int64_t key) {
  *cell = engine_pack_cell((coax_string){(const char *)bytes, (int64_t)n}, key);
  return cell->ok ? (int)cell->ranges : 0;
}
static inline uint32_t engine_pack_cell_digest(const EnginePackCell *cell) { return (uint32_t)cell->digest; }
static inline size_t engine_pack_cell_at(const EnginePackCell *cell, int k) { return (size_t)cell->at.e[k]; }
static inline size_t engine_pack_cell_bytes(const EnginePackCell *cell, int k) { return (size_t)cell->n.e[k]; }
#endif
// ---- metaverse-engine/host/core/host-core.h, on

// A face address: a pointer where the face is built for the host's own addresses, else a 32-bit
// offset (metaverse/native/face-address.h has the same rule).
#if defined(COAXIAL_NATIVE_ADDRESS)
typedef uintptr_t EngineAddr;
#else
typedef uint32_t EngineAddr;
#endif

// host.js's message kind for a pack's progress ("LOADED TOTAL").
enum { ENGINE_CORE_PACK_MESSAGE = 605 };
// What engine_core_deliver hands over: the catalog, vertices and indices (and the rig), then the
// layers' count and the page's own assets. A host that uploads textures between the two calls it twice.
enum { ENGINE_CORE_GEOMETRY = 1, ENGINE_CORE_LAYERS = 2 };
enum { ENGINE_CORE_ENTRIES = 256 };

// The face's exports the core calls. A name the page lacks is NULL. `span` is the host's: the
// pointer for a face address it may write `n` bytes at, given the `capacity` the face states, or
// NULL to refuse (a contract-v1 host asks coaxial_host_copy_span; every host refuses n > capacity).
typedef struct {
  EngineAddr (*asset)(EngineAddr state, int32_t index, uint32_t bytes);
  uint32_t (*asset_capacity)(EngineAddr state, int32_t index);
  void (*asset_ready)(EngineAddr state, int32_t index, uint32_t bytes);
  EngineAddr (*inbox)(EngineAddr state);
  uint32_t (*inbox_cap)(void);
  void (*message)(EngineAddr state, int32_t kind, uint32_t bytes);
  // The pack's rig entry (kind 7, #13306): where the face wants its bytes, and that they are in.
  EngineAddr (*rig_v1)(EngineAddr state, uint32_t bytes);
  void (*rig_ready_v1)(EngineAddr state, uint32_t bytes);
  // The frame plan (#13303): the frame's operations as records, or 0 when the engine refuses.
  uint32_t (*frame_plan_v1)(EngineAddr state, uint32_t features, uint32_t *out, uint32_t cap);
  void *(*span)(void *user, EngineAddr at, uint32_t capacity, uint32_t n);
  void *user;
} EngineCoreFace;

// A pack's entries by kind. The image is the host's and outlives this.
typedef struct {
  const uint8_t *image; size_t bytes;
  EnginePack table;
  int catalog, vertices, indices, rig;            // the first entry of each kind, or -1
  int colours, datas, sounds;                     // texture layers, linear data layers, sounds
  uint16_t colour[ENGINE_CORE_ENTRIES], data[ENGINE_CORE_ENTRIES], sound[ENGINE_CORE_ENTRIES];
} EngineCorePack;

// Reads the whole pack at bytes[0..<n]: 1, or 0 when the one reader refuses it (engine_pack_why on
// p->table says why). An entry of a kind this core doesn't know is left where it is.
static inline int engine_core_pack(EngineCorePack *p, const void *bytes, size_t n) {
  p->image = (const uint8_t *)bytes; p->bytes = n;
  p->catalog = p->vertices = p->indices = p->rig = -1;
  p->colours = p->datas = p->sounds = 0;
  if (!bytes || !engine_pack_open(&p->table, bytes, n)) return 0;
  for (int k = 0; k < engine_pack_count(&p->table); k++) {
    switch (engine_pack_kind(&p->table, k)) {
      case ENGINE_PACK_CATALOG: if (p->catalog < 0) p->catalog = k; break;
      case ENGINE_PACK_VERTICES: if (p->vertices < 0) p->vertices = k; break;
      case ENGINE_PACK_INDICES: if (p->indices < 0) p->indices = k; break;
      case ENGINE_PACK_LAYER: p->colour[p->colours++] = (uint16_t)k; break;
      case ENGINE_PACK_DATA: p->data[p->datas++] = (uint16_t)k; break;
      case ENGINE_PACK_SOUND: p->sound[p->sounds++] = (uint16_t)k; break;
      case ENGINE_PACK_RIG: if (p->rig < 0) p->rig = k; break;
      default: break;
    }
  }
  return 1;
}
// Entry k's bytes (NULL and 0 for k < 0: a part the pack lacks).
static inline const uint8_t *engine_core_entry(const EngineCorePack *p, int k, size_t *n) {
  if (k < 0) { *n = 0; return NULL; }
  *n = engine_pack_bytes(&p->table, k);
  return p->image + engine_pack_at(&p->table, k);
}
static inline uint32_t engine_core_entry_bytes(const EngineCorePack *p, int k) { size_t n; (void)engine_core_entry(p, k, &n); return (uint32_t)n; }

// A message into the face's inbox, cut at its capacity (host.js's send).
static inline void engine_core_send(const EngineCoreFace *f, EngineAddr state, int kind, const void *bytes, size_t n) {
  if (!f->message) return;
  uint32_t cap = f->inbox_cap ? f->inbox_cap() : 0;
  if (n > cap) n = cap;
  if (n) {
    void *to = f->inbox ? f->span(f->user, f->inbox(state), cap, (uint32_t)n) : NULL;
    if (to) memcpy(to, bytes, n); else n = 0;
  }
  f->message(state, kind, (uint32_t)n);
}
// One part copied to where the face wants it: the bytes that went in (0: refused, nothing written).
static inline uint32_t engine_core_copy(const EngineCoreFace *f, EngineAddr at, uint32_t capacity, const uint8_t *bytes, size_t n) {
  if (!at || !bytes || n > 0xffffffffu) return 0;
  void *to = f->span(f->user, at, capacity, (uint32_t)n);
  if (!to) return 0;
  memcpy(to, bytes, n);
  return (uint32_t)n;
}
// Hands the pack's parts to the face in the browser's order: the progress message, asset 0 the
// catalog (copied to the face's measured destination, or delivered as 0 bytes when it doesn't
// fit), 1 and 2 the vertex and index bytes' sizes, the rig when the pack has one and the face asks,
// then 3 the layers' count and the page's `extra` own assets from 4. With no pack (p NULL) the
// page's own assets are the only ones, from 0. Returns 1 when the catalog went in whole.
static inline int engine_core_deliver(const EngineCorePack *p, const EngineCoreFace *f, EngineAddr state, int what, int extra, const uint32_t *extra_sizes) {
  if (!f->asset_ready) return 0;
  if (!p) {
    if (what & ENGINE_CORE_LAYERS) for (int i = 0; i < extra; i++) f->asset_ready(state, i, extra_sizes[i]);
    return 1;
  }
  int whole = 1;
  if (what & ENGINE_CORE_GEOMETRY) {
    char loaded[64];
    int ln = snprintf(loaded, sizeof loaded, "%zu %zu", p->bytes, p->bytes);
    engine_core_send(f, state, ENGINE_CORE_PACK_MESSAGE, loaded, (size_t)ln);
    size_t cn; const uint8_t *catalog = engine_core_entry(p, p->catalog, &cn);
    uint32_t went = 0;
    if (catalog && f->asset) {
      EngineAddr at = f->asset(state, 0, (uint32_t)cn);
      went = engine_core_copy(f, at, f->asset_capacity ? f->asset_capacity(state, 0) : (uint32_t)cn, catalog, cn);
    }
    whole = catalog && went == cn;
    f->asset_ready(state, 0, went);
    f->asset_ready(state, 1, engine_core_entry_bytes(p, p->vertices));
    f->asset_ready(state, 2, engine_core_entry_bytes(p, p->indices));
    if (p->rig >= 0 && f->rig_v1 && f->rig_ready_v1) {
      size_t rn; const uint8_t *rig = engine_core_entry(p, p->rig, &rn);
      f->rig_ready_v1(state, engine_core_copy(f, f->rig_v1(state, (uint32_t)rn), (uint32_t)rn, rig, rn));
    }
  }
  if (what & ENGINE_CORE_LAYERS) {
    f->asset_ready(state, 3, (uint32_t)p->colours);
    for (int i = 0; i < extra; i++) f->asset_ready(state, 4 + i, extra_sizes[i]);
  }
  return whole;
}

// ---- the frame plan
// The frame's operations, asked of the page and walked by the one C walk (frame-plan-walk.h,
// GitHub #13303): the operations walked, or -1 with nothing called when the page has no plan, the
// engine refuses these features, or the plan is not one this host's header can walk whole. The
// host then draws its own order, as before. `words` is the host's scratch, ENGINE_CORE_PLAN_WORDS long.
// ---- metaverse-engine/host/core/frame-plan-walk.h
/* The frame plan's walk for C hosts (GitHub #13303, agreed with the host core on #13304).
 *
 * A page's engine hands its host the frame's operations through
 *   uint32_t coaxial_frame_plan_v1(state, uint32_t features, uint32_t *out, uint32_t cap)
 * (metaverse-engine/render/frame-plan.coax states the records; passes.coax decides the order).
 * This file walks those records and calls the host back once an operation, in order. The host
 * maps a pass to its pipeline and bindings (Metal in PageHost.m, Vulkan on Linux and Android);
 * it decides no order and names no pass of its own.
 *
 *   uint32_t words[ENGINE_FP_WORDS * (ENGINE_FP_MOST_OPS + 1)];
 *   uint32_t n = coaxial_frame_plan_v1(state, bits, words, sizeof words / 4);
 *   if (engine_frame_plan_walk(words, n, &host) < 0) { ... the host's own order, as before ... }
 *
 * The whole plan is checked before the first call: a plan of another version, a short buffer, an
 * operation or pass this header does not know, or passes that do not nest gives -1 and no call at
 * all, so a host never draws half a frame or walks a newer plan wrong. The numbers come from
 * frame-plan.h, which frame-plan.coax writes; this file adds none.
 *
 * Checked by metaverse-engine/host/core/check-frame-plan-walk.sh (check.sh reports, frame-plan-walk):
 * Coax writes plans and what a walk of each must call, and this walk must call exactly that. */
#ifndef ENGINE_FRAME_PLAN_WALK_H
#define ENGINE_FRAME_PLAN_WALK_H
#include <stdint.h>
// ---- metaverse-engine/host/core/../frame-plan.h
/* The frame plan's numbers (GitHub #13303). Written by metaverse-engine/render/frame-plan.coax:
 *   coax/web/tool.sh metaverse-engine/tools/frame-plan-header.coax > metaverse-engine/host/frame-plan.h
 * Never edit it by hand. The record layout is stated at the top of frame-plan.coax. */
#ifndef ENGINE_FRAME_PLAN_H
#define ENGINE_FRAME_PLAN_H
#define ENGINE_FP_MAGIC 827344449u
#define ENGINE_FP_VERSION 1u
#define ENGINE_FP_WORDS 8u
#define ENGINE_FP_MOST_OPS 256u
#define ENGINE_FP_FLAG_KEEP 1u
#define ENGINE_FP_FLAG_STORE_DEPTH 2u
#define ENGINE_FP_FLAG_VALID 4u
#define ENGINE_FP_FLAG_DEPTH_WRITE 8u
#define ENGINE_FP_FLAG_AUX_WRITE 16u
#define ENGINE_FP_OP_BEGIN 0u
#define ENGINE_FP_OP_END 1u
#define ENGINE_FP_OP_DRAW 2u
#define ENGINE_FP_OP_COPY 3u
#define ENGINE_FP_OP_EACH_BEGIN 4u
#define ENGINE_FP_OP_EACH_END 5u
#define ENGINE_FP_OP_MEDIA_BEFORE 6u
#define ENGINE_FP_OP_MEDIA_AFTER 7u
#define ENGINE_FP_OP_CULL_EARLY 8u
#define ENGINE_FP_OP_CULL_LATE 9u
#define ENGINE_FP_OP_VIEWPORT 10u
#define ENGINE_FP_OP_SCISSOR 11u
#define ENGINE_FP_OP_PARTICLE_SORT 12u
#define ENGINE_FP_PASS_BACKGROUND 0u
#define ENGINE_FP_PASS_SHADOW 1u
#define ENGINE_FP_PASS_CASTER 2u
#define ENGINE_FP_PASS_GRASS 3u
#define ENGINE_FP_PASS_SURFACE 4u
#define ENGINE_FP_PASS_SCENE 5u
#define ENGINE_FP_PASS_WATER_TRANSMIT 6u
#define ENGINE_FP_PASS_WATER_EMIT 7u
#define ENGINE_FP_PASS_PARTICLE_OVER 8u
#define ENGINE_FP_PASS_PARTICLE 9u
#define ENGINE_FP_PASS_BLOOM 10u
#define ENGINE_FP_PASS_POST 11u
#define ENGINE_FP_PASS_HUD 12u
#define ENGINE_FP_PASS_SHADOW_CLEAR 14u
#define ENGINE_FP_PASS_CHARACTER_CAST 15u
#define ENGINE_FP_PASS_CHARACTER 16u
#define ENGINE_FP_PASS_SCENE_LATE 17u
#define ENGINE_FP_TARGET_NONE 0u
#define ENGINE_FP_TARGET_SCREEN 1u
#define ENGINE_FP_TARGET_LIGHT 2u
#define ENGINE_FP_TARGET_AUX 3u
#define ENGINE_FP_TARGET_SCENE_DEPTH 4u
#define ENGINE_FP_TARGET_SHADOW_MAP 5u
#define ENGINE_FP_TARGET_WATER_LIGHT 6u
#define ENGINE_FP_TARGET_WATER_DEPTH 7u
#define ENGINE_FP_TARGET_Q0 8u
#define ENGINE_FP_TARGET_Q1 9u
#define ENGINE_FP_TARGET_E0 10u
#define ENGINE_FP_TARGET_E1 11u
#define ENGINE_FP_DEPTH_NONE 0u
#define ENGINE_FP_DEPTH_ALWAYS 1u
#define ENGINE_FP_DEPTH_LESS 2u
#define ENGINE_FP_BLEND_REPLACE 0u
#define ENGINE_FP_BLEND_MAXIMUM 1u
#define ENGINE_FP_BLEND_MULTIPLY 2u
#define ENGINE_FP_BLEND_ADD 3u
#define ENGINE_FP_BLEND_KEEP 4u
#define ENGINE_FP_BLEND_OVER 5u
#define ENGINE_FP_BLEND_UNDER 6u
#define ENGINE_FP_SOURCE_VERTICES 0u
#define ENGINE_FP_SOURCE_RECORDS 1u
#define ENGINE_FP_SOURCE_CURRENT 2u
#define ENGINE_FP_SOURCE_SHADOW_RECORDS 3u
#define ENGINE_FP_SOURCE_LATE 4u
#define ENGINE_FP_HOST_SCENE 1u
#define ENGINE_FP_HOST_SHADOW 2u
#define ENGINE_FP_HOST_CASTERS 4u
#define ENGINE_FP_HOST_GRASS 8u
#define ENGINE_FP_HOST_SURFACE 16u
#define ENGINE_FP_HOST_WATER 32u
#define ENGINE_FP_HOST_WATER_COPY 64u
#define ENGINE_FP_HOST_POST 128u
#define ENGINE_FP_HOST_POST_DEPTH 256u
#define ENGINE_FP_HOST_TRACED_DEPTH 512u
#define ENGINE_FP_HOST_PARTICLES 1024u
#define ENGINE_FP_HOST_PARTICLE_DEPTH 2048u
#define ENGINE_FP_HOST_PARTICLE_OVER 4096u
#define ENGINE_FP_HOST_MEDIA 8192u
#define ENGINE_FP_HOST_TILES 16384u
#define ENGINE_FP_HOST_CULL 32768u
#define ENGINE_FP_HOST_CHARACTER 65536u
#define ENGINE_FP_HOST_SORT 131072u
#define ENGINE_FP_HOST_SCENE_READY 262144u
#define ENGINE_FP_HOST_PARTICLES_READY 524288u
#endif
// ---- metaverse-engine/host/core/frame-plan-walk.h, on

typedef struct engine_fp_op {
  uint32_t kind, pass, source, depth_test;
  uint32_t colour0, colour1, depth, from;     /* targets; `from` a copy's or a bloom draw's source */
  uint32_t to, colour_blend, alpha_blend, outputs;
  uint32_t flags, bias, slope;                /* ENGINE_FP_FLAG_* */
  uint32_t first, count, instances;
  uint32_t step[4];                           /* a bloom draw's step */
} engine_fp_op;

/* The host's side. Every member must be set. `item` is the draw of the list a `current` draw
 * stands for (inside an each loop), 0 otherwise. A rectangle of no width is the whole target. */
typedef struct engine_fp_host {
  void *self;
  void (*begin)(void *self, const engine_fp_op *op);
  void (*end)(void *self);
  void (*draw)(void *self, const engine_fp_op *op, uint32_t item);
  void (*copy)(void *self, uint32_t from, uint32_t to);
  void (*rect)(void *self, int scissor, uint32_t x, uint32_t y, uint32_t w, uint32_t h);
  void (*compute)(void *self, uint32_t kind, uint32_t runs);   /* CULL_EARLY, CULL_LATE, PARTICLE_SORT */
  void (*media)(void *self, int after);
} engine_fp_host;

/* The operations in a written plan, or -1 (frame-plan.coax frame_plan_count). */
static inline int32_t engine_frame_plan_count(const uint32_t *words, uint32_t n_words) {
  if (!words || n_words < ENGINE_FP_WORDS) return -1;
  if (words[0] != ENGINE_FP_MAGIC || words[1] != ENGINE_FP_VERSION || words[2] != ENGINE_FP_WORDS) return -1;
  if (words[3] > ENGINE_FP_MOST_OPS || n_words / ENGINE_FP_WORDS < words[3] + 1u) return -1;
  return (int32_t)words[3];
}

static inline engine_fp_op engine_frame_plan_op(const uint32_t *words, uint32_t i) {
  const uint32_t *w = words + (i + 1u) * ENGINE_FP_WORDS;
  engine_fp_op o;
  o.kind = w[0] & 255u; o.pass = (w[0] >> 8) & 255u; o.source = (w[0] >> 16) & 255u; o.depth_test = w[0] >> 24;
  o.colour0 = w[1] & 255u; o.colour1 = (w[1] >> 8) & 255u; o.depth = (w[1] >> 16) & 255u; o.from = w[1] >> 24;
  o.to = w[2] & 255u; o.colour_blend = (w[2] >> 8) & 255u; o.alpha_blend = (w[2] >> 16) & 255u; o.outputs = w[2] >> 24;
  o.flags = w[3] & 255u; o.bias = (w[3] >> 8) & 255u; o.slope = (w[3] >> 16) & 255u;
  o.first = w[4]; o.count = w[5]; o.instances = w[6];
  for (int k = 0; k < 4; k++) o.step[k] = (w[7] >> (8 * k)) & 255u;
  return o;
}

static inline int engine_fp_pass_known(uint32_t p) { return p <= ENGINE_FP_PASS_HUD || (p >= ENGINE_FP_PASS_SHADOW_CLEAR && p <= ENGINE_FP_PASS_SCENE_LATE); }

/* Whether the plan can be walked: every operation known, passes opened and closed in turn, an each
 * loop inside one pass holding only draws. */
static inline int engine_frame_plan_ok(const uint32_t *words, uint32_t n_words) {
  int32_t n = engine_frame_plan_count(words, n_words);
  if (n < 0) return 0;
  int in_pass = 0, in_each = 0;
  for (int32_t i = 0; i < n; i++) {
    engine_fp_op o = engine_frame_plan_op(words, (uint32_t)i);
    if (o.colour0 > ENGINE_FP_TARGET_E1 || o.colour1 > ENGINE_FP_TARGET_E1 || o.depth > ENGINE_FP_TARGET_E1 || o.from > ENGINE_FP_TARGET_E1 || o.to > ENGINE_FP_TARGET_E1) return 0;
    switch (o.kind) {
      case ENGINE_FP_OP_BEGIN: if (in_pass) return 0; in_pass = 1; break;
      case ENGINE_FP_OP_END: if (!in_pass || in_each) return 0; in_pass = 0; break;
      case ENGINE_FP_OP_DRAW: if (!in_pass || !engine_fp_pass_known(o.pass) || o.source > ENGINE_FP_SOURCE_LATE) return 0; break;
      case ENGINE_FP_OP_COPY: case ENGINE_FP_OP_CULL_EARLY: case ENGINE_FP_OP_CULL_LATE: case ENGINE_FP_OP_PARTICLE_SORT: if (in_pass) return 0; break;
      case ENGINE_FP_OP_EACH_BEGIN: if (!in_pass || in_each) return 0; in_each = 1; break;
      case ENGINE_FP_OP_EACH_END: if (!in_each) return 0; in_each = 0; break;
      case ENGINE_FP_OP_MEDIA_BEFORE: case ENGINE_FP_OP_MEDIA_AFTER: case ENGINE_FP_OP_VIEWPORT: case ENGINE_FP_OP_SCISSOR: if (!in_pass || in_each) return 0; break;
      default: return 0;
    }
    if (in_each && o.kind != ENGINE_FP_OP_EACH_BEGIN && o.kind != ENGINE_FP_OP_DRAW) return 0;
  }
  return !in_pass && !in_each;
}

/* Walk the plan. The operations walked, or -1 with nothing called. */
static inline int32_t engine_frame_plan_walk(const uint32_t *words, uint32_t n_words, const engine_fp_host *host) {
  if (!host || !engine_frame_plan_ok(words, n_words)) return -1;
  int32_t n = (int32_t)words[3];
  for (int32_t i = 0; i < n; i++) {
    engine_fp_op o = engine_frame_plan_op(words, (uint32_t)i);
    switch (o.kind) {
      case ENGINE_FP_OP_BEGIN: host->begin(host->self, &o); break;
      case ENGINE_FP_OP_END: host->end(host->self); break;
      case ENGINE_FP_OP_DRAW: host->draw(host->self, &o, 0); break;
      case ENGINE_FP_OP_COPY: host->copy(host->self, o.from, o.to); break;
      case ENGINE_FP_OP_CULL_EARLY: case ENGINE_FP_OP_CULL_LATE: case ENGINE_FP_OP_PARTICLE_SORT: host->compute(host->self, o.kind, o.count); break;
      case ENGINE_FP_OP_VIEWPORT: case ENGINE_FP_OP_SCISSOR:
        host->rect(host->self, o.kind == ENGINE_FP_OP_SCISSOR, o.first & 65535u, o.first >> 16, o.count & 65535u, o.count >> 16); break;
      case ENGINE_FP_OP_MEDIA_BEFORE: host->media(host->self, 0); break;
      case ENGINE_FP_OP_MEDIA_AFTER: host->media(host->self, 1); break;
      case ENGINE_FP_OP_EACH_BEGIN: {
        /* each draw of the list's range in turn through the operations up to each_end */
        int32_t end = i + 1;
        while ((words[(end + 1) * ENGINE_FP_WORDS] & 255u) != ENGINE_FP_OP_EACH_END) end++;
        for (uint32_t d = 0; d < o.count; d++)
          for (int32_t k = i + 1; k < end; k++) { engine_fp_op inner = engine_frame_plan_op(words, (uint32_t)k); host->draw(host->self, &inner, o.first + d); }
        i = end;
        break;
      }
      default: break;
    }
  }
  return n;
}
#endif
// ---- metaverse-engine/host/core/host-core.h, on
enum { ENGINE_CORE_PLAN_WORDS = (ENGINE_FP_MOST_OPS + 1) * ENGINE_FP_WORDS };
static inline int engine_core_plan(const EngineCoreFace *f, EngineAddr state, uint32_t features, uint32_t *words, const engine_fp_host *host) {
  if (!f->frame_plan_v1 || !host) return -1;
  memset(words, 0, ENGINE_FP_WORDS * sizeof *words);
  uint32_t n = f->frame_plan_v1(state, features, words, ENGINE_CORE_PLAN_WORDS);
  if (n == 0 || n > ENGINE_CORE_PLAN_WORDS) return -1;
  return (int)engine_frame_plan_walk(words, n, host);
}

#if defined(ENGINE_CORE_FACE)

// ---- paged packs
// A file as a host found it: 0 when it is the pack itself, the pack image's size when it is a
// VPK3 file of one, -1 when it is a VPK3 file that can't be used.
static inline int64_t engine_core_image_bytes(const void *file, size_t n) {
  return engine_core_paged((coax_string){(const char *)file, (int64_t)n});
}
// A VPK3 file's pack image into image[0..<cap], every page checked: 1, or 0 and the image is not to be read.
static inline int engine_core_image(const void *file, size_t n, void *image, size_t cap) {
  int64_t want = engine_core_image_bytes(file, n);
  if (want <= 0 || (uint64_t)want > cap) return 0;
  return engine_core_unpage((coax_v_u8){(uint8_t *)file, (int64_t)n}, (coax_v_u8){(uint8_t *)image, want}) == want;
}

// ---- the block build a device takes
// The colour format for a device whose GPU has BC and/or ASTC blocks, given which builds stand
// beside the pack (has(user, tag): 1 when NAME.TAG.vpk3 does): its tag in `tag` ("" for the pack
// itself, RGBA8), and the format's code.
static inline int engine_core_format(int bc, int astc, int phone, int (*has)(void *user, const char *tag), void *user, char tag[8]) {
  int64_t features = engine_core_features(bc ? 1 : 0, astc ? 1 : 0), carried = 0;
  for (int64_t format = 1; format < 16; format++) {
    coax_string t = tf_tag(format);
    if (t.count <= 0 || t.count >= 8) continue;
    char name[8]; memcpy(name, t.data, (size_t)t.count); name[t.count] = 0;
    if (has && has(user, name)) carried |= tf_carries(format);
  }
  int64_t format = tf_pick(features, carried, phone ? 1 : 0, 0);
  coax_string t = tf_tag(format);
  size_t tn = t.count > 0 && t.count < 8 ? (size_t)t.count : 0;
  memcpy(tag, t.data, tn); tag[tn] = 0;
  return (int)format;
}

// ---- block layers
// A layer of the pack: GPU blocks with their whole mip chain (block 1), or an image for the
// host's decoder (block 0).
typedef struct {
  int block;
  const uint8_t *bytes; size_t n;
  coax_t_BlockLayer shape;     // format, width, height, levels, the block's side
  char tag[8];                 // the format's tag ("bc7", "astc4", "astc6")
} EngineCoreLayer;
static inline EngineCoreLayer engine_core_layer(const EngineCorePack *p, int entry) {
  EngineCoreLayer l; memset(&l, 0, sizeof l);
  l.bytes = engine_core_entry(p, entry, &l.n);
  if (!l.bytes) return l;
  l.shape = engine_core_block((coax_string){(const char *)l.bytes, (int64_t)l.n});
  l.block = l.shape.ok ? 1 : 0;
  if (l.block) { coax_string t = tf_tag(l.shape.format); if (t.count > 0 && t.count < 8) memcpy(l.tag, t.data, (size_t)t.count); }
  return l;
}
// Level `level` of a block layer: its bytes, its side in texels and its blocks across and down
// (a row of blocks is `across * 16` bytes). NULL past the last level.
static inline const uint8_t *engine_core_level(const EngineCoreLayer *l, int level, size_t *bytes, int *width, int *height, int *across, int *down) {
  if (!l->block || level < 0 || level >= l->shape.levels) return NULL;
  *across = (int)bl_blocks(l->shape, l->shape.width, level); *down = (int)bl_blocks(l->shape, l->shape.height, level);
  int w = (int)(l->shape.width >> level), h = (int)(l->shape.height >> level);
  *width = w ? w : 1; *height = h ? h : 1;
  *bytes = (size_t)bl_level_bytes(l->shape, level);
  return l->bytes + bl_level_at(l->shape, level);
}
// Whether every colour layer of the pack is a block layer of one format, side and chain: what a
// host uploads as one texture array with no decode. 0 when any is an image, or they differ.
static inline int engine_core_blocks(const EngineCorePack *p, EngineCoreLayer *first) {
  if (p->colours == 0) return 0;
  *first = engine_core_layer(p, p->colour[0]);
  if (!first->block) return 0;
  for (int k = 1; k < p->colours; k++) {
    EngineCoreLayer l = engine_core_layer(p, p->colour[k]);
    if (!l.block || l.shape.format != first->shape.format || l.shape.width != first->shape.width || l.shape.height != first->shape.height || l.shape.levels != first->shape.levels) return 0;
  }
  return 1;
}

#endif
#endif
