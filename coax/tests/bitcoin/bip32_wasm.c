/* Flat test-only Wasm ABI for coax/tests/bitcoin/bip32.py; no derivation or policy lives here. */
#include "bip32.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
#define WORK 326
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
static coax_string text(uint32_t p, uint32_t n) { return (coax_string){(const char *)(uintptr_t)p, n}; }
EXPORT("master") int32_t test_master(uint32_t p, uint32_t n, uint32_t testnet, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip32_master_ct(bytes(p, n), testnet != 0, bytes(w, WORK), bytes(o, c));
}
EXPORT("ckd") int32_t test_ckd(uint32_t p, uint32_t n, uint32_t lo, int32_t hi, uint32_t w, uint32_t o, uint32_t c) {
  int64_t index = (int64_t)(((uint64_t)(uint32_t)hi << 32) | lo);
  return (int32_t)bip32_ckd_ct(bytes(p, n), index, bytes(w, WORK), bytes(o, c));
}
EXPORT("descend") int32_t test_descend(uint32_t p, uint32_t n, uint32_t ix, uint32_t count, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip32_descend_ct(bytes(p, n), (coax_v_s64){(int64_t *)(uintptr_t)ix, count}, bytes(w, WORK), bytes(o, c));
}
EXPORT("neuter") int32_t test_neuter(uint32_t p, uint32_t n, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip32_neuter_ct(bytes(p, n), bytes(w, WORK), bytes(o, c));
}
EXPORT("fingerprint") int32_t test_fingerprint(uint32_t p, uint32_t n, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip32_fingerprint_ct(bytes(p, n), bytes(w, WORK), bytes(o, c));
}
EXPORT("encode") int32_t test_encode(uint32_t p, uint32_t n, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip32_encode_ct(bytes(p, n), bytes(w, WORK), bytes(o, c));
}
EXPORT("decode") int32_t test_decode(uint32_t p, uint32_t n, uint32_t w, uint32_t o, uint32_t c) {
  return (int32_t)bip32_decode_ct(text(p, n), bytes(w, WORK), bytes(o, c));
}
