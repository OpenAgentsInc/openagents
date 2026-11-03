/* Test-only secret-flow probe of the ECDSA signer (Bitcoin chapter 20), in
   the ctgrind style: memcheck treats the secret key as undefined, so every
   branch or memory address in the compiled signer that depends on it is
   reported. The hash is public. Only the signature in out and the call's
   result are declassified, after the call. Inside the call the signer
   reveals one fact, the validity bit at ec_declassify; the script accepts
   reports only at that line. It signs twice: with ecdsa_sign and with
   chapter 22's ecdsa_sign_recoverable, whose recovery ID is computed from
   the nonce point with masks. A report anywhere else is a key-dependent
   branch or address in the compiled signer.
   Run: coax/tests/bitcoin/ecdsa_sign_secret_probe.sh */
#include "es.h"
#include <valgrind/memcheck.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static uint8_t key[32], hash[32], out[64], rec[65];
static coax_t_EcdsaWork work;
int main(int argc, char **argv) {
  /* A test key (tiny-secp256k1's row 4) and SHA-256("Satoshi Nakamoto"). */
  static const uint8_t k[32] = {0x69,0xec,0x59,0xea,0xa1,0xf4,0xf2,0xe3,0x6b,0x63,0x97,0x16,0xb7,0xc3,0x0c,0xa8,
                                0x6d,0x9a,0x53,0x75,0xc7,0xb3,0x8d,0x89,0x18,0xbd,0x9c,0x0e,0xbc,0x80,0xba,0x64};
  static const uint8_t h[32] = {0xa0,0xdc,0x65,0xff,0xca,0x79,0x98,0x73,0xcb,0xea,0x0a,0xc2,0x74,0x01,0x5b,0x95,
                                0x26,0x50,0x5d,0xaa,0xae,0xd3,0x85,0x15,0x54,0x25,0xf7,0x33,0x77,0x04,0x88,0x3e};
  memcpy(key, k, 32); memcpy(hash, h, 32);
  VALGRIND_MAKE_MEM_UNDEFINED(key, 32);
  if (argc > 1) { volatile int seen = 0; if (key[31] == 0x64) seen = 1; printf("control %d\n", seen); return 0; }  /* a branch on the key: must be flagged */
  coax_v_u8 kv = {key, 32}, hv = {hash, 32}, ov = {out, 64};
  coax_v_t_EcdsaWork wv = {&work, 1};
  bool ok = ecdsa_sign(kv, hv, wv, ov, false);
  VALGRIND_MAKE_MEM_DEFINED(&ok, sizeof ok);
  VALGRIND_MAKE_MEM_DEFINED(out, 64);
  printf("signed %d sig %02x%02x..%02x%02x\n", ok, out[0], out[1], out[62], out[63]);
  /* Chapter 22's recoverable signer: the same signer, a 65th byte. */
  coax_v_u8 rv = {rec, 65};
  bool rok = ecdsa_sign_recoverable(kv, hv, wv, rv, false);
  VALGRIND_MAKE_MEM_DEFINED(&rok, sizeof rok);
  VALGRIND_MAKE_MEM_DEFINED(rec, 65);
  printf("recoverable %d id %d same %d\n", rok, rec[64], memcmp(rec, out, 64) == 0);
  return ok && rok ? 0 : 1;
}
