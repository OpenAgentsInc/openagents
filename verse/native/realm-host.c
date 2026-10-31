// A realm natively (realm-host.h).
#define _GNU_SOURCE
#include "realm-host.h"
#include "face.h"
#include <malloc.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>

// The face's offsets are 32 bits. Built without PIE, its static data is
// below 2 GB; malloc stays on the brk heap (no mmap), which follows it; and
// the arena is mapped there too. rh_ptr refuses anything else.
void *rh_ptr(uint32_t offset) { return (void *)(uintptr_t)offset; }
uint32_t rh_off(const void *p) {
  uintptr_t a = (uintptr_t)p;
  if (a >> 32) { fprintf(stderr, "realm-host: %p is above 4 GB, where the face cannot point\n", p); exit(1); }
  return (uint32_t)a;
}

static uint32_t align16(uint32_t n) { return (n + 15u) & ~15u; }

void rh_open(RealmHost *h, int golden) {
  memset(h, 0, sizeof *h);
  mallopt(M_MMAP_MAX, 0);
  h->floats = (int)coaxial_vertex_floats();
  h->max_vertices = (int)coaxial_max_vertices();
  h->uniform_floats = (int)coaxial_uniform_floats();
  uint32_t state = align16(coaxial_state_size() > 1024 ? coaxial_state_size() : 1024);
  h->arena_size = (size_t)state + (size_t)h->max_vertices * h->floats * 4 + (size_t)h->uniform_floats * 4 + 64;
  h->arena = mmap(NULL, h->arena_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_32BIT, -1, 0);
  if (h->arena == MAP_FAILED) { perror("realm-host: arena"); exit(1); }
  h->state = rh_off(h->arena);
  h->vertices = h->state + state;
  h->uniforms = h->vertices + (uint32_t)(h->max_vertices * h->floats * 4);
  if (golden) coaxial_golden(1);
}

static uint32_t le32(const uint8_t *p) { return p[0] | p[1] << 8 | p[2] << 16 | (uint32_t)p[3] << 24; }

int rh_pack(RealmHost *h, const char *path) {
  FILE *f = fopen(path, "rb");
  if (!f) { fprintf(stderr, "realm-host: cannot read %s\n", path); return 1; }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  h->pack = malloc(n > 0 ? (size_t)n : 1);
  if (!h->pack || fread(h->pack, 1, (size_t)n, f) != (size_t)n) { fclose(f); fprintf(stderr, "realm-host: cannot read %s\n", path); return 1; }
  fclose(f);
  h->pack_size = (size_t)n;
  // The header and table checked against the length, as host.js unpack does.
  const uint8_t *b = h->pack;
  if (h->pack_size < 32 || memcmp(b, "VPK1", 4) || le32(b + 8) != h->pack_size) { fprintf(stderr, "realm-host: %s is not a whole asset pack\n", path); return 1; }
  uint32_t count = le32(b + 4), table = le32(b + 16);
  if ((uint64_t)table + (uint64_t)count * 48 > h->pack_size) { fprintf(stderr, "realm-host: %s: its table runs past its end\n", path); return 1; }
  h->entries = calloc(count ? count : 1, sizeof *h->entries);
  for (uint32_t k = 0; k < count; k++) {
    const uint8_t *at = b + table + k * 48;
    uint32_t offset = le32(at + 4), length = le32(at + 8);
    if ((uint64_t)offset + length > h->pack_size) { fprintf(stderr, "realm-host: %s: an entry runs past its end\n", path); return 1; }
    RhEntry *e = &h->entries[k];
    e->kind = at[0];
    memcpy(e->name, at + 16, 32);
    e->bytes = b + offset;
    e->length = length;
    if (e->kind == RH_LAYER) h->layers++;
  }
  h->entry_count = (int)count;
  return 0;
}

const RhEntry *rh_entry(const RealmHost *h, int kind) {
  for (int i = 0; i < h->entry_count; i++) if (h->entries[i].kind == kind) return &h->entries[i];
  return NULL;
}

void rh_deliver(RealmHost *h, int extra, const uint32_t *extra_sizes) {
  char loaded[64];
  int n = snprintf(loaded, sizeof loaded, "%zu %zu", h->pack_size, h->pack_size);
  rh_send(h, RH_PACK_KIND, loaded, (size_t)n);
  const RhEntry *catalog = rh_entry(h, RH_CATALOG), *vertices = rh_entry(h, RH_VERTICES), *indices = rh_entry(h, RH_INDICES);
  uint32_t sizes[4] = {catalog ? catalog->length : 0, vertices ? vertices->length : 0, indices ? indices->length : 0, (uint32_t)h->layers};
  if (catalog) {
    uint32_t at = coaxial_asset(h->state, 0, catalog->length);
    if (at) memcpy(rh_ptr(at), catalog->bytes, catalog->length);
  }
  for (int i = 0; i < 4; i++) coaxial_asset_ready(h->state, i, sizes[i]);
  for (int i = 0; i < extra; i++) coaxial_asset_ready(h->state, 4 + i, extra_sizes[i]);
}

void rh_send(RealmHost *h, int kind, const char *text, size_t length) {
  uint32_t cap = coaxial_inbox_cap();
  if (length > cap) length = cap;
  memcpy(rh_ptr(coaxial_inbox(h->state)), text, length);
  coaxial_message(h->state, kind, (uint32_t)length);
  rh_after(h, 0);
}

uint32_t rh_frame(RealmHost *h, float w, float ht, float scale, float dt) {
  uint32_t n = coaxial_frame(h->state, h->vertices, h->uniforms, w, ht, scale, dt, 0);
  return n > (uint32_t)h->max_vertices ? (uint32_t)h->max_vertices : n;
}

int rh_after(RealmHost *h, int say) {
  int action = coaxial_action(h->state);
  if (!action) return 0;
  if (say) {
    uint32_t len = coaxial_outbox_len(h->state);
    printf("realm-host: action %d: %.*s\n", action, (int)(len > 200 ? 200 : len), (const char *)rh_ptr(coaxial_outbox(h->state)));
  }
  coaxial_action_done(h->state);
  return action;
}

void rh_close(RealmHost *h) {
  munmap(h->arena, h->arena_size);
  free(h->entries);
  free(h->pack);
}

// The host's imports (coax/web/host.js imports.env), natively. This host
// has no network yet: requests are refused, as a page offline has them.
static int64_t clock_ms(void) {
  const char *fake = getenv("VERSE_NATIVE_CLOCK_MS");
  if (fake) return atoll(fake);
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
int64_t guard_now_ms(void) { return clock_ms(); }
int32_t guard_http(uint32_t a, uint32_t b, uint32_t c, uint32_t d, uint32_t e, int64_t f) {
  (void)a; (void)b; (void)c; (void)d; (void)e; (void)f;
  return -1;
}
int32_t guard_cancel(int32_t id) { (void)id; return 0; }
int32_t coaxial_http(uint32_t url, uint32_t url_len, uint32_t body, uint32_t body_len, int32_t flags) {
  (void)url; (void)url_len; (void)body; (void)body_len; (void)flags;
  return -1;
}
// The phone's thumbs (verse/grid.coax) draw through these; a desktop has none.
void host_line(int64_t h, double ax, double ay, double bx, double by, double la, double lb, double w) {
  (void)h; (void)ax; (void)ay; (void)bx; (void)by; (void)la; (void)lb; (void)w;
}
void host_dot(int64_t h, double x, double y, double size, double light, double soft, double crisp) {
  (void)h; (void)x; (void)y; (void)size; (void)light; (void)soft; (void)crisp;
}
void host_slot(int64_t h, double x, double y, double w, double ht, double press, double flash, double selected, double icon) {
  (void)h; (void)x; (void)y; (void)w; (void)ht; (void)press; (void)flash; (void)selected; (void)icon;
}
