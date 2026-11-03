/* Flat test-only Wasm ABI for coax/tests/bitcoin/der.py; no parsing or policy lives here. */
#include "der.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("check") int32_t test_check(uint32_t s, uint32_t n) { return (int32_t)der_check_vartime(bytes(s, n)); }
EXPORT("script") int32_t test_script(uint32_t s, uint32_t n, uint32_t flags) {
  return (int32_t)der_signature_check_vartime(bytes(s, n), flags);
}
EXPORT("lax") uint32_t test_lax(uint32_t s, uint32_t n, uint32_t out) {
  return der_read_lax_vartime(bytes(s, n), bytes(out, 64));
}
