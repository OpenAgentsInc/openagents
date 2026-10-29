// BIP-340 Schnorr over libsecp256k1 (Vendor/secp256k1, v0.6.0) for the
// phone's Nostr key. The secret key comes from the Keychain and stays in the
// caller's buffer; nothing here keeps or logs it. Each call returns 1 on
// success, 0 otherwise.
#pragma once
#include <stdint.h>

int oa_nostr_seckey_valid(const uint8_t seckey[32]);
// The x-only public key of seckey.
int oa_nostr_pubkey(const uint8_t seckey[32], uint8_t pubkey[32]);
// The signature of a 32-byte message (an event id) with 32 bytes of aux randomness.
int oa_nostr_sign(const uint8_t seckey[32], const uint8_t msg[32], const uint8_t aux[32], uint8_t sig[64]);
int oa_nostr_verify(const uint8_t pubkey[32], const uint8_t msg[32], const uint8_t sig[64]);
