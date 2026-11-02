/* Flat test-only ABI for coax/tests/bitcoin/ecdsa_sign.py, native and Wasm;
   no crypto or policy lives here. sign returns 1 and writes r || s, 0 for a
   refusal, or 2 when the signer left the workspace uncleared. */
#include "ecdsa_sign.h"
#include <stdint.h>
#ifdef __wasm__
#define EXPORT(name) __attribute__((export_name(name)))
#else
#define EXPORT(name)
#endif
static coax_t_EcdsaWork work;
static coax_v_u8 bytes(uint8_t *p, int64_t n) { return (coax_v_u8){p, n}; }
/* 1 when every byte of the workspace is zero. */
EXPORT("work_cleared") uint32_t test_work_cleared(void) {
  const uint8_t *p = (const uint8_t *)&work;
  uint8_t any = 0;
  for (unsigned i = 0; i < sizeof work; i++) any |= p[i];
  return any == 0;
}
EXPORT("sign") uint32_t test_sign(uintptr_t key, uintptr_t hash, uintptr_t out) {
  uint8_t *k = (uint8_t *)key, *h = (uint8_t *)hash, *o = (uint8_t *)out;
  for (unsigned i = 0; i < sizeof work; i++) ((uint8_t *)&work)[i] = 0xa5;   /* garbage the signer must clear */
  _Bool ok = ecdsa_sign(bytes(k, 32), bytes(h, 32), (coax_v_t_EcdsaWork){&work, 1}, bytes(o, 64), 0);
  return test_work_cleared() ? ok : 2;   /* 2: the workspace was not cleared */
}
