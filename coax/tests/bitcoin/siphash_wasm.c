/* Flat test-only Wasm ABI for coax/tests/bitcoin/siphash.py; no crypto or policy lives here. */
#include "siphash.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("sip_hash") uint32_t test_hash(uint32_t key, uint32_t p, uint32_t n, uint32_t out) {
  return siphash24_checked(bytes(key, 16), bytes(p, n), bytes(out, 8), 0);
}
