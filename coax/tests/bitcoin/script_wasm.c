/* Flat test-only Wasm ABI for coax/tests/bitcoin/script.py; no parsing or policy lives here. */
#include "script.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("verify") int32_t test_verify(uint32_t tx, uint32_t ntx, uint32_t spk, uint32_t nspk, int32_t index, double amount, int32_t flags) {
  return (int32_t)script_report_vartime(bytes(tx, ntx), bytes(spk, nspk), index, (int64_t)amount, flags);
}
EXPORT("sighash") int32_t test_sighash(uint32_t tx, uint32_t ntx, uint32_t code, uint32_t ncode, int32_t index, double amount, int32_t hashtype, int32_t v0, uint32_t out) {
  return (int32_t)script_sighash_report_vartime(bytes(tx, ntx), bytes(code, ncode), index, (int64_t)amount, hashtype, v0, bytes(out, 32));
}
