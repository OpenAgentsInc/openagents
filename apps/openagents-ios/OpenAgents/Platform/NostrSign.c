#include "NostrSign.h"
#include "../../Vendor/secp256k1/include/secp256k1.h"
#include "../../Vendor/secp256k1/include/secp256k1_extrakeys.h"
#include "../../Vendor/secp256k1/include/secp256k1_schnorrsig.h"
#include <string.h>

static secp256k1_context *context(void) {
  static secp256k1_context *ctx;
  if (!ctx) ctx = secp256k1_context_create(SECP256K1_CONTEXT_NONE);
  return ctx;
}
// Wipes memory the compiler may not drop.
static void wipe(void *p, size_t n) { volatile uint8_t *v = p; while (n--) *v++ = 0; }

int oa_nostr_seckey_valid(const uint8_t seckey[32]) { return secp256k1_ec_seckey_verify(context(), seckey); }

int oa_nostr_pubkey(const uint8_t seckey[32], uint8_t pubkey[32]) {
  secp256k1_keypair pair; secp256k1_xonly_pubkey x;
  int ok = secp256k1_keypair_create(context(), &pair, seckey)
        && secp256k1_keypair_xonly_pub(context(), &x, NULL, &pair)
        && secp256k1_xonly_pubkey_serialize(context(), pubkey, &x);
  wipe(&pair, sizeof pair);
  return ok;
}

int oa_nostr_sign(const uint8_t seckey[32], const uint8_t msg[32], const uint8_t aux[32], uint8_t sig[64]) {
  secp256k1_keypair pair; secp256k1_xonly_pubkey x;
  int ok = secp256k1_keypair_create(context(), &pair, seckey)
        && secp256k1_schnorrsig_sign32(context(), sig, msg, &pair, aux)
        && secp256k1_keypair_xonly_pub(context(), &x, NULL, &pair)
        && secp256k1_schnorrsig_verify(context(), sig, msg, 32, &x);   // never hand out a bad signature
  wipe(&pair, sizeof pair);
  if (!ok) memset(sig, 0, 64);
  return ok;
}

int oa_nostr_verify(const uint8_t pubkey[32], const uint8_t msg[32], const uint8_t sig[64]) {
  secp256k1_xonly_pubkey x;
  return secp256k1_xonly_pubkey_parse(context(), &x, pubkey) && secp256k1_schnorrsig_verify(context(), sig, msg, 32, &x);
}
