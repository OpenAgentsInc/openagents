/* Flat test-only Wasm ABI for coax/tests/bitcoin/pow.py; no proof-of-work logic lives here. */
#include "pow.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
/* index is unsigned so that nBits above 2^31 arrive whole. */
EXPORT("report") int32_t test_report(int32_t op, uint32_t p, uint32_t n, uint32_t index, uint32_t o, uint32_t c) {
  return (int32_t)pow_report_vartime(op, bytes(p, n), (int64_t)index, bytes(o, c));
}
