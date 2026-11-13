// Canonical CPU image ownership only. The caller owns actual GPU uploads and
// ledger handles; decoded pixels do not by themselves assert GPU readiness.
#ifndef ENGINE_RENDER_IMAGES_H
#define ENGINE_RENDER_IMAGES_H
#include "RenderTexture.h"
#include <stddef.h>
#include <stdint.h>
typedef struct RhgiImages RhgiImages;
typedef struct {
  coax_string url;int64_t slot,width,height;
  const uint8_t *pixels;size_t bytes;int decoded,failed;
} RhgiImage;
// size() observes the physical encoded extent before allocation; read() must
// return1 only after filling that exact buffer; all other values refuse.
// Neither callback decodes.
// A source identity is immutable for this cache owner, including failures.
typedef struct {
  int64_t (*size)(void *user,coax_string url);
  int (*read)(void *user,coax_string url,uint8_t *out,size_t bytes);
  void *user;
} RhgiSource;
RhgiImages *rhgi_open(void);
// New identities are Coax-admitted before URL/source retention. Cache-full
// and missing/malformed sources are failed/not-ready observations, not traps.
// Cached views live until close; a refused view borrows the supplied URL.
RhgiImage rhgi_get(RhgiImages *cache,int64_t kind,coax_string url,RhgiSource source);
size_t rhgi_count(const RhgiImages *cache);
size_t rhgi_bytes(const RhgiImages *cache);
// Pass actual unique GPU readiness/failed observations. Coax formats the
// whole607 packet atomically, including 0x0 for failed images.
int64_t rhgi_notice(const coax_t_HostMediaNotice *rows,size_t count,uint8_t *out,size_t bytes);
// Return the actual Coax residency after all physical pixel buffers are freed.
int64_t rhgi_close(RhgiImages *cache);
#endif
