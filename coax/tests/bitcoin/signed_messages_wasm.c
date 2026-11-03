/* Flat test-only ABI for coax/tests/bitcoin/ecdsa_recover.py, native and
   Wasm; no crypto or policy lives here. sign returns 1 and writes r || s ||
   id, 0 for a refusal, or 2 when the signer left the workspace uncleared. */
#include "signmessage.h"
#include <stdint.h>
#ifdef __wasm__
#define EXPORT(name) __attribute__((export_name(name)))
#else
#define EXPORT(name)
#endif
static coax_t_EcdsaWork work;
static coax_v_u8 bytes(uintptr_t p, int64_t n) { return (coax_v_u8){(uint8_t *)p, n}; }
static coax_string text(uintptr_t p, int64_t n) { return (coax_string){(const char *)p, n}; }
static uint32_t cleared(void) {
  const uint8_t *p = (const uint8_t *)&work;
  uint8_t any = 0;
  for (unsigned i = 0; i < sizeof work; i++) any |= p[i];
  return any == 0;
}
static void dirty(void) { for (unsigned i = 0; i < sizeof work; i++) ((uint8_t *)&work)[i] = 0xa5; }
EXPORT("recover") int32_t test_recover(uintptr_t hash, int32_t hash_n, uintptr_t sig, int32_t id, int32_t form, uintptr_t out) {
  return (int32_t)ecdsa_recover_vartime(bytes(hash, hash_n), bytes(sig, 64), id, form, bytes(out, 65));
}
EXPORT("sign") uint32_t test_sign(uintptr_t key, uintptr_t hash, uintptr_t out) {
  dirty();
  _Bool ok = ecdsa_sign_recoverable(bytes(key, 32), bytes(hash, 32), (coax_v_t_EcdsaWork){&work, 1}, bytes(out, 65), 0);
  return cleared() ? ok : 2;
}
EXPORT("message_sign") int32_t test_message_sign(uintptr_t key, uint32_t compressed, uintptr_t message, int32_t n, uintptr_t out) {
  dirty();
  int32_t r = (int32_t)signed_message_sign(bytes(key, 32), compressed != 0, text(message, n), (coax_v_t_EcdsaWork){&work, 1}, bytes(out, 88));
  return cleared() ? r : -100;
}
EXPORT("message_verify") int32_t test_message_verify(uintptr_t address, int32_t an, uintptr_t sig, int32_t sn, uintptr_t message, int32_t mn, uint32_t testnet) {
  return (int32_t)verify_message_vartime(text(address, an), text(sig, sn), text(message, mn), testnet != 0);
}
EXPORT("message_hash") uint32_t test_message_hash(uintptr_t message, int32_t n, uintptr_t out) {
  return signed_message_hash(text(message, n), bytes(out, 32));
}
