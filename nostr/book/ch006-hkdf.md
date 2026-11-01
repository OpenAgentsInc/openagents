# 6. HKDF

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

HKDF derives keys from a shared secret. NIP-44 v2 derives a conversation key
with HKDF-extract and the per-message ChaCha20 key, nonce and HMAC key with
HKDF-expand. This chapter builds both halves and the limits on output length.

Part II, Hashes and MACs. Result: HKDF-SHA256 extract and expand with output
bounds, checked against RFC 5869 cases 1–3, and the exact NIP-44 v2 use:
extract with salt `nip44-v2`, expand to 76 bytes.

## What this chapter teaches

- Extract: `PRK = HMAC(salt, IKM)`.
- Expand: `T(i) = HMAC(PRK, T(i-1) || info || i)`, at most 255 blocks.
- NIP-44 v2: `conversation_key = HKDF-extract(salt = "nip44-v2", IKM =
  shared_x)` and `HKDF-expand(conversation_key, nonce, 76)` split into 32 + 12
  + 32 bytes.
- Output lengths over `255 * 32` refuse.

## What exists today

- `coax/lib/crypto/hkdf.coax` implements both halves;
  `coax/tests/nostr/hkdf_rfc.coax` runs the RFC cases.

## Dependencies

- [5. HMAC-SHA256](ch005-hmac-sha256.md)

## Done means

- RFC 5869 SHA-256 cases 1–3 pass.
- An expand request of `255 * 32 + 1` bytes refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
