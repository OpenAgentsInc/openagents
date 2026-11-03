/* Flat test-only Wasm ABI for coax/tests/bitcoin/tx.py; no parsing or policy lives here. */
#include "tx.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
EXPORT("report") int32_t test_report(uint32_t p, uint32_t n, int32_t mode, int32_t nin, int32_t nout, int32_t nitems, uint32_t o, uint32_t c) {
  return (int32_t)tx_report_vartime(bytes(p, n), mode, nin, nout, nitems, bytes(o, c));
}
