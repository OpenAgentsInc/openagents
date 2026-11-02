/* Flat test-only Wasm ABI for coax/tests/bitcoin/sha512.py; no crypto or policy lives here. */
#include "sha512.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("sha_hash") uint32_t test_hash(uint32_t p, uint32_t n, uint32_t out) { return sha512_hash(bytes(p, n), bytes(out, 64)); }
EXPORT("sha_legacy") uint32_t test_legacy(uint32_t p, uint32_t n, uint32_t out) { return sha512(bytes(p, n), bytes(out, 64)) ? 0 : 1; }
EXPORT("sha_stream") uint32_t test_stream(uint32_t p, uint32_t n, uint32_t chunk, uint32_t out) {
  uint8_t state[208];
  coax_v_u8 s = {state, 208};
  if (!chunk) return 2;
  uint32_t status = sha512_init(s);
  if (status) return status;
  for (uint32_t at = 0; at < n;) {
    uint32_t take = n - at < chunk ? n - at : chunk;
    status = sha512_update(s, bytes(p + at, take));
    if (status) return status;
    at += take;
  }
  return sha512_final(s, bytes(out, 64));
}
