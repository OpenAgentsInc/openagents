/* Flat test-only Wasm ABI for coax/tests/bitcoin/secp_bench.sh --wasm; no crypto or policy lives here. */
#include "secp_bench.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("ecdsa") uint32_t bench_ecdsa(uint32_t h, uint32_t k, uint32_t s) { return ecdsa_verify_vartime(bytes(h, 32), bytes(k, 33), bytes(s, 64), 1); }
EXPORT("schnorr") uint32_t bench_schnorr(uint32_t h, uint32_t k, uint32_t s) { return schnorr_verify_vartime(bytes(h, 32), bytes(k, 32), bytes(s, 64)); }
