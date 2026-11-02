/* Flat test-only Wasm ABI for coax/tests/bitcoin/base58.py; no encoding or policy lives here. */
#include "base58.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
static coax_string text(uint32_t p, uint32_t n) { return (coax_string){(const char *)(uintptr_t)p, n}; }
EXPORT("encode") int32_t test_encode(uint32_t p, uint32_t n, uint32_t o, uint32_t c) {
  return (int32_t)base58_encode_vartime(bytes(p, n), bytes(o, c));
}
EXPORT("decode") int32_t test_decode(uint32_t p, uint32_t n, uint32_t o, uint32_t c) {
  return (int32_t)base58_decode_vartime(text(p, n), bytes(o, c));
}
EXPORT("check_encode") int32_t test_check_encode(uint32_t p, uint32_t n, uint32_t o, uint32_t c) {
  return (int32_t)base58check_encode_vartime(bytes(p, n), bytes(o, c));
}
EXPORT("check_decode") int32_t test_check_decode(uint32_t p, uint32_t n, uint32_t o, uint32_t c) {
  return (int32_t)base58check_decode_vartime(text(p, n), bytes(o, c));
}
EXPORT("address_decode") int32_t test_address_decode(uint32_t p, uint32_t n, uint32_t o) {
  return (int32_t)base58_address_decode_vartime(text(p, n), bytes(o, 20));
}
EXPORT("wif_encode") int32_t test_wif_encode(uint32_t k, uint32_t compressed, uint32_t testnet, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)wif_encode_ct(bytes(k, 32), compressed != 0, testnet != 0, bytes(w, 42), bytes(o, c));
}
EXPORT("wif_decode") int32_t test_wif_decode(uint32_t p, uint32_t n, uint32_t w, uint32_t o) {
  return (int32_t)wif_decode_ct(text(p, n), bytes(w, 42), bytes(o, 32));
}
