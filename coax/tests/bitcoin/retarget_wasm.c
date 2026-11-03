/* Flat test-only Wasm ABI for coax/tests/bitcoin/retarget.py; no retargeting logic lives here. */
#include "retarget.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("report") int32_t test_report(int32_t op, uint32_t p, uint32_t n, uint32_t index, uint32_t o, uint32_t c) {
  (void)index;
  return (int32_t)retarget_report_vartime(op, bytes(p, n), bytes(o, c));
}
