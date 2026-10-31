// A realm natively: what coax/web/host.js does for a realm's wasm, for the
// same face built with gcc (face.h). It keeps the realm's state, vertices
// and uniforms in an arena below 4 GB, loads its asset pack from a file
// (verse/tools/pack_realm.py has the format), hands the pack's parts to the
// realm as the browser does, and passes messages, keys and frames. It
// draws nothing; a GPU host (verse/gpu) draws what it hands back.
#ifndef VERSE_NATIVE_REALM_HOST_H
#define VERSE_NATIVE_REALM_HOST_H
#include <stddef.h>
#include <stdint.h>

enum { RH_CATALOG = 1, RH_VERTICES = 2, RH_INDICES = 3, RH_LAYER = 4, RH_SOUND = 5 };
// host.js's message kinds.
enum { RH_PAGE_KIND = 600, RH_PACK_KIND = 605, RH_VIEWER_KIND = 606 };

typedef struct {
  uint8_t kind;
  char name[33];
  const uint8_t *bytes;
  uint32_t length;
} RhEntry;

typedef struct {
  uint8_t *arena;
  size_t arena_size;
  uint32_t state, vertices, uniforms;
  int floats, max_vertices, uniform_floats;
  uint8_t *pack;
  size_t pack_size;
  RhEntry *entries;
  int entry_count;
  int layers;   // how many texture layers the pack has
} RealmHost;

// A pointer the face handed back (a 32-bit offset), and the offset of one of ours.
void *rh_ptr(uint32_t offset);
uint32_t rh_off(const void *p);
// Starts the realm: its arena, zeroed state, golden mode if asked.
void rh_open(RealmHost *h, int golden);
// Reads a pack; 0 on success, else prints why.
int rh_pack(RealmHost *h, const char *path);
// The first entry of a kind, or NULL.
const RhEntry *rh_entry(const RealmHost *h, int kind);
// Gives the realm its assets, in the browser's order: the pack's four parts
// (catalog, vertices, indices, layers) as assets 0 to 3, then `extra` more
// (a page's other assets, such as its particles), each `extra_sizes[i]` bytes.
void rh_deliver(RealmHost *h, int extra, const uint32_t *extra_sizes);
// A message into the realm's inbox (cut at its capacity).
void rh_send(RealmHost *h, int kind, const char *text, size_t length);
// One frame: the vertex count.
uint32_t rh_frame(RealmHost *h, float w, float ht, float scale, float dt);
// The realm's pending action, handled the way a host with no browser can:
// it is printed (when `say`) and marked done. Returns the action, 0 for none.
int rh_after(RealmHost *h, int say);
// The network and what the realm keeps: coaxial_http's requests go to HOST:PORT, each through
// one HELPER process (verse/native/net.coax, the approved TLS); the playtest player's key
// (action 5, every realm's) and this realm's place and bar (actions 11 and 12, as NAME) are
// files in DIR. Until this is called requests are refused and nothing is kept. 0 on success.
int rh_net_open(RealmHost *h, const char *helper, const char *host, int port, const char *dir, const char *name);
// Answers that have come, as messages 512 + id ("STATUS\n" then the body; "0\n" for none or
// past the deadline: a bounded request's own, 35 s otherwise). Call it between frames.
void rh_net_poll(RealmHost *h);
// Gives the realm the place and bar it kept, before its first frame (not in golden mode).
void rh_restore(RealmHost *h);
// Stops what is still running.
void rh_net_close(void);
void rh_close(RealmHost *h);
#endif
