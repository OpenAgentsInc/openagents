/* Flat test-only Wasm ABI for coax/tests/bitcoin/ecdsa_verify.py; no crypto or policy lives here. */
#include "ecdsa_verify.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("verify") uint32_t test_verify(uint32_t h, uint32_t k, uint32_t kn, uint32_t s, uint32_t sn, uint32_t low_s) {
  return ecdsa_verify_der_vartime(bytes(h, 32), bytes(k, kn), bytes(s, sn), low_s != 0);
}
