/* Flat test-only Wasm ABI for coax/tests/bitcoin/segwit.py; no encoding or policy lives here. */
#include "segwit.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
static coax_string text(uint32_t p, uint32_t n) { return (coax_string){(const char *)(uintptr_t)p, n}; }
EXPORT("variant") int32_t test_variant(uint32_t p, uint32_t n) {
  return (int32_t)segwit_variant_vartime(text(p, n));
}
/* Writes the network and the version as two int32 values at info. */
EXPORT("decode") int32_t test_decode(uint32_t p, uint32_t n, uint32_t o, uint32_t c, uint32_t info) {
  coax_t_Segwit_Address a = segwit_decode_vartime(text(p, n), bytes(o, c));
  ((int32_t *)(uintptr_t)info)[0] = (int32_t)a.network;
  ((int32_t *)(uintptr_t)info)[1] = (int32_t)a.version;
  return (int32_t)a.length;
}
EXPORT("encode") int32_t test_encode(int32_t network, int32_t version, uint32_t p, uint32_t n, uint32_t o, uint32_t c) {
  return (int32_t)segwit_encode_vartime(network, version, bytes(p, n), bytes(o, c));
}
