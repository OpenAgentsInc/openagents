/* Flat test-only Wasm ABI for coax/tests/bitcoin/address.py; no encoding or policy lives here. */
#include "address.h"
#include <stdint.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_v_u8 bytes(uint32_t p, uint32_t n) { return (coax_v_u8){(uint8_t *)(uintptr_t)p, n}; }
static coax_string text(uint32_t p, uint32_t n) { return (coax_string){(const char *)(uintptr_t)p, n}; }
/* which is 1 to 6: P2PKH, P2WPKH and P2SH-P2WPKH from a key, P2SH and P2WSH from a script, P2TR from an x-only key. */
EXPORT("build") int32_t test_build(int32_t which, uint32_t p, uint32_t n, uint32_t o, uint32_t c) {
  coax_v_u8 in = bytes(p, n), out = bytes(o, c);
  switch (which) {
    case 1: return (int32_t)p2pkh_script_vartime(in, out);
    case 2: return (int32_t)p2wpkh_script_vartime(in, out);
    case 3: return (int32_t)p2sh_p2wpkh_script_vartime(in, out);
    case 4: return (int32_t)p2sh_script_vartime(in, out);
    case 5: return (int32_t)p2wsh_script_vartime(in, out);
    default: return (int32_t)p2tr_script_vartime(in, out);
  }
}
EXPORT("tweak") int32_t test_tweak(uint32_t p, uint32_t n, uint32_t o, uint32_t c) {
  return (int32_t)taproot_tweak_vartime(bytes(p, n), bytes(o, c));
}
EXPORT("kind") int32_t test_kind(uint32_t p, uint32_t n) { return (int32_t)script_kind_vartime(bytes(p, n)); }
EXPORT("encode") int32_t test_encode(int32_t network, uint32_t p, uint32_t n, uint32_t o, uint32_t c) {
  return (int32_t)address_encode_vartime(network, bytes(p, n), bytes(o, c));
}
/* Writes the kind as an int32 at info. */
EXPORT("decode") int32_t test_decode(int32_t network, uint32_t p, uint32_t n, uint32_t o, uint32_t c, uint32_t info) {
  coax_t_Address_Script a = address_decode_vartime(network, text(p, n), bytes(o, c));
  ((int32_t *)(uintptr_t)info)[0] = (int32_t)a.kind;
  return (int32_t)a.length;
}
