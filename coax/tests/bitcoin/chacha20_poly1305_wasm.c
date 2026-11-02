/* Flat test-only Wasm ABI for coax/tests/bitcoin/chacha20_poly1305.py; no crypto or policy lives here. */
#include "chacha20_poly1305.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("seal") uint32_t test_seal(uint32_t k, uint32_t kn, uint32_t n, uint32_t nn, uint32_t a, uint32_t an, uint32_t p, uint32_t pn, uint32_t o, uint32_t on) {
  return chacha20_poly1305_seal(bytes(k, kn), bytes(n, nn), bytes(a, an), bytes(p, pn), bytes(o, on), 0);
}
EXPORT("open") uint32_t test_open(uint32_t k, uint32_t kn, uint32_t n, uint32_t nn, uint32_t a, uint32_t an, uint32_t p, uint32_t pn, uint32_t o, uint32_t on) {
  return chacha20_poly1305_open(bytes(k, kn), bytes(n, nn), bytes(a, an), bytes(p, pn), bytes(o, on), 0);
}
EXPORT("poly") uint32_t test_poly(uint32_t k, uint32_t m, uint32_t mn, uint32_t o) {
  return poly1305_checked(bytes(k, 32), bytes(m, mn), bytes(o, 16), 0) ? 0 : 1;
}
EXPORT("poly_stream") uint32_t test_poly_stream(uint32_t k, uint32_t m, uint32_t mn, uint32_t chunk, uint32_t o) {
  coax_t_Poly1305State state[1];
  coax_v_t_Poly1305State s = {state, 1};
  if (!chunk) return 99;
  uint32_t status = poly1305_init(s, bytes(k, 32), 0);
  for (uint32_t at = 0; status == 0 && at < mn;) {
    uint32_t take = mn - at < chunk ? mn - at : chunk;
    status = poly1305_update(s, bytes(m + at, take), 0);
    at += take;
  }
  return status ? status : poly1305_final(s, bytes(o, 16), 0);
}
