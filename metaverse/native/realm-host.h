// A realm natively: what coax/web/host.js does for a realm's wasm, for the
// same face built with gcc (face.h). It keeps the realm's state, vertices
// and uniforms in an arena (below 4 GB for a legacy address face), loads its asset pack from a file
// (metaverse/tools/pack_realm.py has the format), hands the pack's parts to the
// realm as the browser does, and passes messages, keys and frames. It
// draws nothing; a GPU host (metaverse/gpu) draws what it hands back.
#ifndef METAVERSE_NATIVE_REALM_HOST_H
#define METAVERSE_NATIVE_REALM_HOST_H
#include <stddef.h>
#include <stdint.h>
#include "face-address.h"

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
  cx_face_addr state, vertices, uniforms;
  int floats, max_vertices, uniform_floats;
  uint8_t *pack;
  size_t pack_size;
  RhEntry *entries;
  int entry_count;
  int layers;   // how many texture layers the pack has
  struct RhAudio *audio;
  struct RhAudioDevice *audio_device;
  int64_t audio_status,audio_command;
  int (*clipboard)(void *user,const char *text,size_t length);void *clipboard_user;int action_refused;char *location_hash;size_t location_hash_bytes;
} RealmHost;

// Face addresses and the matching address of one of our measured allocations.
void *rh_ptr(cx_face_addr offset);
cx_face_addr rh_off(const void *p);
uint64_t rh_address_limit(void);
// Starts the realm: its arena, zeroed state, golden mode if asked.
void rh_open(RealmHost *h, int golden);
// Reads a pack ("-": none, as a page that names no pack); 0 on success, else prints why.
int rh_pack(RealmHost *h, const char *path);
// The first entry of a kind, or NULL.
const RhEntry *rh_entry(const RealmHost *h, int kind);
// Gives the realm its assets, in the browser's order: the pack's four parts
// (catalog, vertices, indices, layers) as assets 0 to 3, then `extra` more
// (a page's other assets, such as its particles), each `extra_sizes[i]` bytes.
void rh_deliver(RealmHost *h, int extra, const uint32_t *extra_sizes);
// A message into the realm's inbox (cut at its capacity).
void rh_send(RealmHost *h, int kind, const char *text, size_t length);
// Text commits are whole offers: a smaller producer inbox refuses with604.
int rh_text(RealmHost *h,const char *text,size_t length);
void rh_refuse(RealmHost *h);
// One frame: the vertex count.
uint32_t rh_frame(RealmHost *h, float w, float ht, float scale, float dt);
// The explicit still-frame form used by deterministic host probes/previews.
uint32_t rh_frame_input(RealmHost *h, float w, float ht, float scale, float dt, int still);
// Version1 clock offer: -1 unsupported,0 refused,1 produced (even zero vertices).
// The SDK caller reports actual affordability before offering producer time.
int rh_frame_at(RealmHost *h,float w,float ht,float scale,int64_t monotonic_us,int64_t pending,int still,uint32_t *vertices);
int rh_host_version(void);
// Shared lifecycle actions; released input is applied to the actual producer.
int64_t rh_lifecycle_event(RealmHost *h,int kind,int64_t monotonic_us,int64_t arg);
int64_t rh_lifecycle_info(int field);
// Optional v1 audio SDK. Device starts inactive; actual input gestures start
// this engine's clock. Muting changes only the concrete output samples.
int rh_audio_enable(RealmHost *h,int muted);
// A film's sound (#13003): the same mixer with no device, its session started, rendered by the
// caller in step with the frames (rh_audio_take: FRAMES stereo f32 frames at 48 kHz).
int rh_audio_offline(RealmHost *h);
int rh_audio_take(RealmHost *h,float *out,size_t frames);
void rh_audio_gesture(RealmHost *h);
void rh_audio_pause(RealmHost *h,int paused);
void rh_audio_retire(RealmHost *h);
void rh_audio_close(RealmHost *h);
// Fields0..10 are core observations,20..30 device observations,40 bank
// admission status,41 last command,42 device closed,43 context closed.
int64_t rh_audio_info(RealmHost *h,int field);
// Copy a complete declared data asset, then deliver readiness. Contract v1
// faces use the shared measured-span authority; refusal delivers zero bytes.
int rh_asset_data(RealmHost *h, int index, const void *bytes, size_t length);
// SDK dispatch of already normalized host inputs (device-pixel pointer
// coordinates, CSS-pixel wheel delta and contract key/modifier values).
uint32_t rh_key(RealmHost *h, int key, int mods, int down);
void rh_pointer(RealmHost *h, float x, float y, int phase, int kind, int id);
void rh_blur(RealmHost *h);
void rh_wheel(RealmHost *h, float dy);
// The realm's pending action, handled the way a host with no browser can:
// it is printed (when `say`) and marked done. Returns the action, 0 for none.
int rh_after(RealmHost *h, int say);
// The SDK copies admitted text synchronously; unavailable/refused offers
// acknowledge once and report604 on a later poll, outside producer dispatch.
const char *rh_hash(RealmHost *h,size_t *bytes);
void rh_set_clipboard(RealmHost *h,int (*copy)(void *,const char *,size_t),void *user);
// The page to draw (host.js's data-page), before the page's facts.
void rh_page(RealmHost *h, int page);
// Travel: a navigation (action 1) since the last call, as the realm it opens in SLUG (cap bytes):
// 1 when it names one, 0 when there was none or it is the web's (a tip page, the builder, a site).
int rh_went(char *slug, size_t cap);
// The realm a path opens (/worlds the same as /realms): /realms (every realm's way out) the galaxy, the verse app's page 1 ("galaxy"), / and /new-land the Grid
// ("the-grid"), /realms/SLUG (then nothing, /, ? or #) SLUG; an https://openagents.com
// prefix is the same site. 0 for any other path.
int rh_realm_of(const char *path, size_t len, char *slug, size_t cap);
// The network and what the realm keeps: coaxial_http's requests go to HOST:PORT, each through
// one HELPER process (metaverse/native/net.coax, the approved TLS); the playtest player's key
// (action 5, every realm's) and this realm's place and bar (actions 11 and 12, as NAME) are
// files in DIR. Until this is called requests are refused and nothing is kept. 0 on success.
int rh_net_open(RealmHost *h, const char *helper, const char *host, int port, const char *dir, const char *name);
// Answers that have come, as messages 512 + id ("STATUS\n" then the body; "0\n" for none or
// past the deadline: a bounded request's own, 35 s otherwise). Call it between frames.
void rh_net_poll(RealmHost *h);
// Next-completed-frame image handoff, outside producer and GPU encoding.
int rh_http_image_due(void);
void rh_http_image_ready(const uint8_t *jpeg_base64,size_t bytes);
// Gives the realm the place and bar it kept, before its first frame (not in golden mode).
void rh_restore(RealmHost *h);
// Stops what is still running.
void rh_net_close(void);
// Benchmark-only replay of already checked room transport records (OA-RPLY1).
int rh_replay_open(const char *path);
void rh_replay_poll(RealmHost *h);
double rh_replay_seconds(void);
void rh_close(RealmHost *h);
#endif
