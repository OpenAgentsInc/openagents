# 46. Keys from a seed phrase

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

NIP-06 derives a Nostr key from BIP-39 words, so a person can restore it from
a phrase. The pinned text marks it `unrecommended: prefer a single nsec`; the
website's words account uses it.

Part X, Every NIP. Result: BIP-39 words to seed with PBKDF2-HMAC-SHA512,
BIP-32 derivation at m/44'/1237'/0'/0/0, and the NIP-06 key and its npub.

## What this chapter teaches

- BIP-39: checksum, PBKDF2-HMAC-SHA512 with 2,048 iterations and salt
  `mnemonic`.
- BIP-32: hardened and normal child keys with HMAC-SHA512.
- The path `m/44'/1237'/<account>'/0/0`.
- The NIP-06 test vectors in the pinned text.

## What exists today

- `coax/lib/nostr/nip06.coax`, `coax/lib/crypto/bip32.coax`, `bip39.coax`,
  `pbkdf2.coax` and `sha512.coax`. The words account on `/login` uses them
  ([docs/web-log-in.md](../../docs/web-log-in.md)).

## Dependencies

- [13. Sign with Schnorr](ch013-sign-with-schnorr.md)
- [5. HMAC-SHA256](ch005-hmac-sha256.md)

## Done means

- Both NIP-06 vectors produce their stated private and public keys.
- An invalid checksum refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-06](../nips/official/06.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
