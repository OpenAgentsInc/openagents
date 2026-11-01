# 20. NIP-44 encrypted payloads

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

NIP-44 v2 is the encryption NIP-17, NIP-59 and many OpenAgents NIPs use. This
chapter builds it from chapters 5, 6, 14 and 18 and checks it against the
official vector file.

Part V, Encryption. Result: NIP-44 version 2: conversation keys, padding,
ChaCha20, HMAC over nonce‖ciphertext checked before decryption, and every
valid and invalid vector in `nip44.vectors.json`.

## What this chapter teaches

- Conversation key from ECDH x and HKDF-extract with salt `nip44-v2`.
- Message keys: HKDF-expand with the 32-byte nonce to ChaCha20 key, ChaCha20
  nonce and HMAC key.
- Padding: a 2-byte big-endian length, then zeros to the next power-of-two
  chunk (minimum 32).
- Payload: `version (0x02) || nonce || ciphertext || mac`, base64.
- Decryption order: check version, length, MAC in constant time, then decrypt,
  then check padding.
- Profiles: `NIP44_COMPAT` up to 65,535 bytes and `NIP44_EXTENDED` with the
  6-byte length prefix at 65,536 and beyond.

## What exists today

- `coax/lib/nostr/nip44.coax` and `nip44-key.coax`; the vector file is pinned
  in `coax/tests/vectors/upstream/nip44/`.
- Records:
  [2026-10-29-nip44-v2.md](../../docs/verification/2026-10-29-nip44-v2.md),
  [2026-10-30-nip44-conversation-key.md](../../docs/verification/2026-10-30-nip44-conversation-key.md).

## Dependencies

- [6. HKDF](ch006-hkdf.md)
- [14. Shared secrets with ECDH](ch014-shared-secrets-with-ecdh.md)
- [18. ChaCha20](ch018-chacha20.md)
- [5. HMAC-SHA256](ch005-hmac-sha256.md)
- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Done means

- Every `valid` and `invalid` entry in `nip44.vectors.json` gives its expected
  result.
- Lengths 65,535, 65,536 and 65,537 behave as the active profile states.
- A ciphertext with a wrong MAC never produces plaintext output.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-44](../nips/official/44.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
