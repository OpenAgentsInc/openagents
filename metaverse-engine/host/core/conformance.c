// The host core's conformance driver (GitHub #13304): it runs metaverse-engine/host/core/host-core.h
// over the fixtures conformance.coax wrote into DIR, against a face that only records what it is
// handed, and prints what the core did. It judges nothing: conformance.coax wrote DIR/expected.txt
// from the engine's own modules and compares. Every platform builds this one file over the same
// core (Linux and the Mac with cc, the iPhone simulator and Android with their SDKs' clang):
//   cc -std=gnu11 -I BUILD -I ROOT ROOT/metaverse-engine/host/core/conformance.c BUILD/aethyr-core-face.c -lm -o conformance
//   ./conformance DIR
// It is built with and without -DCOAXIAL_NATIVE_ADDRESS: a face address is an offset into the
// arena below either way, so both address widths run on any machine.
#include <stdio.h>
#include <stdlib.h>
#include "aethyr-core-face.h"
#define AETHYR_PACK_TABLE_PROVIDED
#define AETHYR_CORE_FACE
#include "metaverse-engine/host/core/host-core.h"

// The fake face: an arena whose first bytes are the inbox, then asset 0's place, then the rig's.
enum { INBOX_AT = 16, INBOX_CAP = 64, ASSET_AT = 128, RIG_AT = 8192, ARENA = 16384 };
static uint8_t arena[ARENA];
static uint32_t asset_cap = 4096;
static const char *label = "deliver";
static int packed = 1;   // asset 0 is the pack's catalog (not, with no pack, the page's own first asset)
static uint32_t plan_words[AETHYR_CORE_PLAN_WORDS];
static uint32_t plan_records;

static uint32_t fnv(const uint8_t *b, size_t n) { uint32_t h = 2166136261u; for (size_t i = 0; i < n; i++) { h ^= b[i]; h *= 16777619u; } return h; }
static void *span(void *user, AethyrAddr at, uint32_t capacity, uint32_t n) {
  (void)user;
  if (n > capacity || (uint64_t)at + n > ARENA) return NULL;
  return arena + at;
}
static AethyrAddr f_asset(AethyrAddr state, int32_t index, uint32_t bytes) { (void)state; (void)bytes; return index == 0 ? ASSET_AT : 0; }
static uint32_t f_asset_capacity(AethyrAddr state, int32_t index) { (void)state; return index == 0 ? asset_cap : 0; }
static void f_asset_ready(AethyrAddr state, int32_t index, uint32_t bytes) {
  (void)state;
  if (index == 0 && bytes && packed) printf("%s ready 0 %u hash %u\n", label, bytes, fnv(arena + ASSET_AT, bytes));
  else printf("%s ready %d %u\n", label, index, bytes);
}
static AethyrAddr f_inbox(AethyrAddr state) { (void)state; return INBOX_AT; }
static uint32_t f_inbox_cap(void) { return INBOX_CAP; }
static void f_message(AethyrAddr state, int32_t kind, uint32_t bytes) { (void)state; printf("%s message %d %.*s\n", label, kind, (int)bytes, (const char *)arena + INBOX_AT); }
static AethyrAddr f_rig(AethyrAddr state, uint32_t bytes) { (void)state; (void)bytes; return RIG_AT; }
static void f_rig_ready(AethyrAddr state, uint32_t bytes) { (void)state; printf("%s rig %u hash %u\n", label, bytes, fnv(arena + RIG_AT, bytes)); }
static uint32_t f_plan(AethyrAddr state, uint32_t features, uint32_t *out, uint32_t cap) {
  (void)state; (void)features;
  if (plan_records == 0 || plan_records > cap) return 0;   // plan_records is the words written
  memcpy(out, plan_words, (size_t)plan_records * sizeof *out);
  return plan_records;
}

static uint8_t *read_file(const char *dir, const char *name, size_t *n) {
  char path[2048]; snprintf(path, sizeof path, "%s/%s", dir, name);
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "conformance: cannot read %s\n", path); exit(2); }
  fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
  uint8_t *b = malloc(size > 0 ? (size_t)size : 1);
  if (!b || fread(b, 1, (size_t)size, f) != (size_t)size) { fprintf(stderr, "conformance: cannot read %s\n", path); exit(2); }
  fclose(f); *n = (size_t)size;
  return b;
}

static const char *have;   // the tags of the block builds "beside the pack", comma separated
static int has(void *user, const char *tag) {
  (void)user;
  size_t n = strlen(tag);
  for (const char *p = have; *p; p++) if ((p == have || p[-1] == ',') && !strncmp(p, tag, n) && (p[n] == ',' || !p[n])) return 1;
  return 0;
}
static void format(int bc, int astc, int phone, const char *builds) {
  char tag[8]; have = builds;
  (void)aethyr_core_format(bc, astc, phone, has, NULL, tag);
  printf("format bc %d astc %d phone %d has %s -> %s\n", bc, astc, phone, *builds ? builds : "-", *tag ? tag : "rgba8");
}

// The plan's host: every call the walk makes, as a line.
static void p_begin(void *self, const aethyr_fp_op *o) { (void)self; printf("plan begin %u %u %u %u\n", o->pass, o->colour0, o->colour1, o->depth); }
static void p_end(void *self) { (void)self; printf("plan end\n"); }
static void p_draw(void *self, const aethyr_fp_op *o, uint32_t item) { (void)self; printf("plan draw %u %u %u %u %u item %u\n", o->pass, o->source, o->first, o->count, o->instances, item); }
static void p_copy(void *self, uint32_t from, uint32_t to) { (void)self; printf("plan copy %u %u\n", from, to); }
static void p_rect(void *self, int scissor, uint32_t x, uint32_t y, uint32_t w, uint32_t h) { (void)self; printf("plan rect %d %u %u %u %u\n", scissor, x, y, w, h); }
static void p_compute(void *self, uint32_t kind, uint32_t runs) { (void)self; printf("plan compute %u %u\n", kind, runs); }
static void p_media(void *self, int after) { (void)self; printf("plan media %d\n", after); }

int main(int argc, char **argv) {
  if (argc != 2) { fprintf(stderr, "usage: conformance DIR\n"); return 2; }
  size_t pn, vn, wn;
  uint8_t *pack = read_file(argv[1], "conformance.pack", &pn), *paged = read_file(argv[1], "conformance.vpk3", &vn), *words = read_file(argv[1], "conformance.plan", &wn);
  AethyrCoreFace face = {f_asset, f_asset_capacity, f_asset_ready, f_inbox, f_inbox_cap, f_message, f_rig, f_rig_ready, f_plan, span, NULL};
  static AethyrCorePack p;
  // the pack: its entries by kind
  if (!aethyr_core_pack(&p, pack, pn)) { printf("pack refused\n"); return 0; }
  printf("pack entries %d catalog %d vertices %d indices %d rig %d colours %d datas %d sounds %d\n", aethyr_pack_count(&p.table), p.catalog, p.vertices, p.indices, p.rig, p.colours, p.datas, p.sounds);
  for (int k = 0; k < aethyr_pack_count(&p.table); k++) { size_t n; const uint8_t *b = aethyr_core_entry(&p, k, &n); printf("entry %d kind %d bytes %zu hash %u\n", k, aethyr_pack_kind(&p.table, k), n, fnv(b, n)); }
  static AethyrCorePack none;
  printf("pack damaged %d\n", aethyr_core_pack(&none, pack, pn - 1));
  // delivery: whole, with a catalog that doesn't fit, and with no pack
  uint32_t extra[1] = {77};
  int whole = aethyr_core_deliver(&p, &face, 0, AETHYR_CORE_GEOMETRY | AETHYR_CORE_LAYERS, 1, extra);
  printf("deliver whole %d\n", whole);
  label = "tight"; asset_cap = 255; memset(arena, 0, sizeof arena);
  whole = aethyr_core_deliver(&p, &face, 0, AETHYR_CORE_GEOMETRY, 0, NULL);
  printf("tight whole %d\n", whole);
  label = "nopack"; packed = 0; (void)aethyr_core_deliver(NULL, &face, 0, AETHYR_CORE_GEOMETRY | AETHYR_CORE_LAYERS, 1, extra);
  // the layers: GPU blocks and their levels, or an image
  for (int k = 0; k < p.colours; k++) {
    AethyrCoreLayer l = aethyr_core_layer(&p, p.colour[k]);
    printf("layer %d block %d %s %lld %lld %lld\n", k, l.block, l.block ? l.tag : "-", (long long)l.shape.width, (long long)l.shape.height, (long long)l.shape.levels);
    for (int level = 0; ; level++) {
      size_t n; int w, h, across, down;
      const uint8_t *b = aethyr_core_level(&l, level, &n, &w, &h, &across, &down);
      if (!b) break;
      printf("level %d %d %d %d %d %zu hash %u\n", level, w, h, across, down, n, fnv(b, n));
    }
  }
  AethyrCoreLayer first;
  printf("layers uniform %d\n", aethyr_core_blocks(&p, &first));
  for (int k = 0; k < p.datas; k++) { AethyrCoreLayer l = aethyr_core_layer(&p, p.data[k]); printf("data %d block %d bytes %zu\n", k, l.block, l.n); }
  // the world cells of its catalog
  for (int c = 0; c < 2; c++) {
    int64_t key = aethyr_pack_cell_key(c == 0 ? 1 : 3, c == 0 ? -2 : 3);
    AethyrPackCell cell; int ranges = aethyr_pack_cell_ranges(&cell, pack, pn, key);
    if (!ranges) { printf("cell %lld none\n", (long long)key); continue; }
    printf("cell %lld digest %u ranges", (long long)key, aethyr_pack_cell_digest(&cell));
    for (int r = 0; r < ranges; r++) printf(" %zu,%zu", aethyr_pack_cell_at(&cell, r), aethyr_pack_cell_bytes(&cell, r));
    printf("\n");
  }
  // the paged file: its image, a damaged one, and a file that is the pack itself
  int64_t want = aethyr_core_image_bytes(paged, vn);
  uint8_t *image = malloc(want > 0 ? (size_t)want : 1);
  int ok = aethyr_core_image(paged, vn, image, want > 0 ? (size_t)want : 0);
  printf("paged image %lld hash %u\n", (long long)(ok ? want : -1), ok ? fnv(image, (size_t)want) : 0);
  paged[vn - 1] ^= 1;
  printf("paged damaged %d\n", aethyr_core_image(paged, vn, image, (size_t)want));
  printf("paged plain %lld\n", (long long)aethyr_core_image_bytes(pack, pn));
  // the block build a device takes
  format(1, 0, 0, "bc7,astc4"); format(1, 1, 1, "bc7,astc4"); format(0, 1, 0, "bc7"); format(1, 1, 0, ""); format(1, 1, 0, "astc4"); format(0, 1, 1, "astc4,astc6");
  // the frame plan: walked whole, refused by its version, refused when this host lacks a pass, absent
  static uint32_t scratch[AETHYR_CORE_PLAN_WORDS];
  aethyr_fp_host host = {NULL, p_begin, p_end, p_draw, p_copy, p_rect, p_compute, p_media};
  memcpy(plan_words, words, wn < sizeof plan_words ? wn : sizeof plan_words); plan_records = (uint32_t)(wn / 4);
  printf("plan walked %d\n", aethyr_core_plan(&face, 0, 0, scratch, &host));
  plan_words[1] += 1;
  printf("plan version %d\n", aethyr_core_plan(&face, 0, 0, scratch, &host));
  plan_words[1] -= 1; plan_records -= 1;
  printf("plan short %d\n", aethyr_core_plan(&face, 0, 0, scratch, &host));
  face.frame_plan_v1 = NULL;
  printf("plan absent %d\n", aethyr_core_plan(&face, 0, 0, scratch, &host));
  return 0;
}
