# 18. ChaCha20

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

NIP-44 encrypts with ChaCha20. This chapter builds the quarter round, the
block function and the stream, and the rules about nonces and counters that
keep it safe.

Part V, Encryption. Result: RFC 8439 ChaCha20 with caller-supplied nonce and
counter, counter exhaustion refused, and HChaCha20 for NIP-49's XChaCha20.

## What this chapter teaches

- The 4x4 state: constants, 256-bit key, 32-bit counter, 96-bit nonce.
- The quarter round and 20 rounds; the block is the state plus the input.
- Counter overflow refuses; a key and nonce pair is never reused.
- ChaCha20 alone does not authenticate; NIP-44 adds HMAC.

## What exists today

- `coax/lib/crypto/chacha20.coax` and `hchacha20.coax`.
- Records:
  [coax-chacha20-2026-10-28.md](../../docs/verification/coax-chacha20-2026-10-28.md),
  [2026-10-29-hchacha20.md](../../docs/verification/2026-10-29-hchacha20.md).

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Done means

- RFC 8439 sections 2.3.2 and 2.4.2 vectors pass.
- A request that would pass counter `2^32 - 1` refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
