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
#include "../pack-table.h"

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
#include "frame-plan-walk.h"
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
