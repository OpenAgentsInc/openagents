/* The unlock opener's Wasm face over unlock-nostr.coax: fixed buffers the
 * TypeScript opener (lib.ts, coaxNostr) fills and reads, and one export per
 * Coax procedure. No Nostr policy here: every verdict is the Coax one. */
#include "unlock-nostr.h"
#include <stdint.h>
#include <string.h>
#define EXPORT(name) __attribute__((export_name(name)))
#define IN_CAP 262144
#define OUT_CAP 262144
static unsigned char in_bytes[2][IN_CAP], out_bytes[OUT_CAP], secret[32], aux[32];
static coax_v_u8 view(unsigned char *p, int64_t n) { return (coax_v_u8){p, n}; }
static int64_t fit(int32_t n) { return n >= 0 && n <= IN_CAP ? n : -1; }

EXPORT("un_in") uint32_t un_in(uint32_t i) { return i < 2 ? (uint32_t)(uintptr_t)in_bytes[i] : 0; }
EXPORT("un_in_cap") uint32_t un_in_cap(void) { return IN_CAP; }
EXPORT("un_out") uint32_t un_out(void) { return (uint32_t)(uintptr_t)out_bytes; }
EXPORT("un_secret") uint32_t un_secret(void) { return (uint32_t)(uintptr_t)secret; }
EXPORT("un_aux") uint32_t un_aux(void) { return (uint32_t)(uintptr_t)aux; }
/* The secret and aux leave memory after each use. */
EXPORT("un_wipe") void un_wipe(void) { memset(secret, 0, sizeof secret); memset(aux, 0, sizeof aux); }
/* The secret's x-only public key into out (32 bytes): 1, or 0. */
EXPORT("un_pubkey") int32_t un_pubkey(void) { return unlock_pubkey(view(secret, 32), view(out_bytes, 32)) ? 1 : 0; }
/* in[0] the tags JSON, in[1] the content: the signed event's length in out, or -1. */
EXPORT("un_sign") int32_t un_sign(int64_t created_at, int32_t kind, int32_t tags_n, int32_t content_n) {
  int64_t t = fit(tags_n), c = fit(content_n);
  if (t < 0 || c < 0) return -1;
  int64_t n = unlock_sign(view(secret, 32), view(aux, 32), created_at, kind, view(in_bytes[0], t), view(in_bytes[1], c), view(out_bytes, OUT_CAP));
  return n > 0 && n <= OUT_CAP ? (int32_t)n : -1;
}
/* in[0] an event's JSON: 0 verified, 1 a bad id, 2 a bad signature. */
EXPORT("un_verify") int32_t un_verify(int32_t n) { int64_t m = fit(n); return m < 0 ? 1 : (int32_t)unlock_verify(view(in_bytes[0], m)); }
/* in[0] an event's JSON: 1 when its id is the hash of its fields. */
EXPORT("un_id_matches") int32_t un_id_matches(int32_t n) { int64_t m = fit(n); return m >= 0 && unlock_id_matches(view(in_bytes[0], m)) ? 1 : 0; }
