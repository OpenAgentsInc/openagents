/* Flat test-only Wasm ABI for coax/tests/bitcoin/secp_bench.sh --wasm; no crypto or policy lives here. */
#include "secp_bench.h"
#include <stdint.h>
#include <string.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("ecdsa") uint32_t bench_ecdsa(uint32_t h, uint32_t k, uint32_t s) { return ecdsa_verify_vartime(bytes(h, 32), bytes(k, 33), bytes(s, 64), 1); }
EXPORT("schnorr") uint32_t bench_schnorr(uint32_t h, uint32_t k, uint32_t s) { return schnorr_verify_vartime(bytes(h, 32), bytes(k, 32), bytes(s, 64)); }
static coax_t_EcdsaWork ew; static coax_t_SignWork sw;
EXPORT("ecdsa_sign") uint32_t bench_ecdsa_sign(uint32_t d, uint32_t h, uint32_t out) { memset(&ew, 0, sizeof ew); return ecdsa_sign(bytes(d, 32), bytes(h, 32), (coax_v_t_EcdsaWork){&ew, 1}, bytes(out, 64), 0); }
EXPORT("schnorr_sign") uint32_t bench_schnorr_sign(uint32_t d, uint32_t a, uint32_t h, uint32_t out) {
  int64_t status, calls = 0; memset(&sw, 0, sizeof sw);
  do { status = schnorr_sign_step(bytes(d, 32), bytes(a, 32), bytes(h, 32), (coax_v_t_SignWork){&sw, 1}, bytes(out, 64), 0); calls++; } while (status == 1 && calls < 16);
  return status == 2;
}
