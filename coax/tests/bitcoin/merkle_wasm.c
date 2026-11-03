/* Flat test-only Wasm ABI for coax/tests/bitcoin/merkle.py; no tree logic lives here. */
#include "merkle.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("report") int32_t test_report(int32_t op, uint32_t p, uint32_t n, int32_t index, int32_t count, uint32_t o, uint32_t c) {
  return (int32_t)merkle_report_vartime(op, bytes(p, n), index, count, bytes(o, c));
}
