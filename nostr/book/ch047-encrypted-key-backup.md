# 47. Encrypted key backup

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

NIP-49 encrypts a private key with a password so it can be stored or moved.
This chapter builds it with the limits that keep an untrusted `log_n` from
exhausting memory.

Part X, Every NIP. Result: NIP-49 `ncryptsec`: NFKC-normalized passwords,
scrypt with `log_n` from the payload, XChaCha20-Poly1305 with the key-security
byte as associated data, and bounded KDF memory.

## What this chapter teaches

- Password normalized to NFKC.
- scrypt (Salsa20/8, BlockMix, ROMix) with `r = 8`, `p = 1` and `N = 2^log_n`.
- XChaCha20-Poly1305: HChaCha20 subkey, 24-byte nonce, Poly1305 tag.
- Payload: version `0x02`, `log_n`, salt, nonce, key-security byte,
  ciphertext; Bech32 `ncryptsec`.
- A `log_n` above the profile limit refuses before any allocation.

## What exists today

- `coax/lib/crypto/scrypt_nip49.coax` with resumable scrypt,
  `xchacha20_poly1305.coax`, `poly1305.coax` and `coax/lib/text/nfkc.coax`.
  The full `ncryptsec` import and export are not assembled.
- Record:
  [2026-10-29-scrypt-nip49.md](../../docs/verification/2026-10-29-scrypt-nip49.md).

## Dependencies

- [18. ChaCha20](ch018-chacha20.md)
- [5. HMAC-SHA256](ch005-hmac-sha256.md)
- [15. Bech32](ch015-bech32.md)

## Done means

- The NIP-49 test vector decrypts to its stated key.
- A `log_n` of 22 refuses on a profile limited to 20.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-49](../nips/official/49.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
