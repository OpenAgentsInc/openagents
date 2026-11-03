/* Flat test-only Wasm ABI for coax/tests/bitcoin/bip39.py; no encoding or policy lives here. */
#include "bip39.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
static coax_string text(uint32_t p, uint32_t n) { return (coax_string){(const char *)(uintptr_t)p, n}; }
EXPORT("encode") int32_t test_encode(uint32_t p, uint32_t n, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip39_encode_ct(bytes(p, n), bytes(w, 1520), bytes(o, c));
}
EXPORT("decode") int32_t test_decode(uint32_t p, uint32_t n, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip39_decode_ct(text(p, n), bytes(w, 1520), bytes(o, c));
}
EXPORT("seed") int32_t test_seed(uint32_t p, uint32_t n, uint32_t q, uint32_t m, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip39_seed_ct(text(p, n), text(q, m), bytes(w, 1520), bytes(o, c));
}
