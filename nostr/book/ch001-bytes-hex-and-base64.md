# 1. Bytes, hex and base64

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Every Nostr value starts as bytes: 32-byte public keys and event IDs, 64-byte
signatures, 12-byte and 32-byte nonces. This chapter defines how Coax holds
them, in caller-owned buffers with explicit lengths, and how it reads and
writes them as hex and base64. Later chapters never allocate a secret and
never accept a malformed encoding.

Part I, Foundations. Result: Caller-owned byte views, big- and little-endian
loads and stores, strict lowercase hex and canonical padded base64, with every
decoder refusing bad input instead of guessing.

## What this chapter teaches

- How a byte view carries its length, and why every output goes to a
  caller-owned buffer with a status result.
- Endian loads and stores: `load32_be`, `store64_le` and the bounds each one
  checks.
- Strict hex for Nostr: exactly 64 lowercase characters for an ID or public
  key and 128 for a signature. Uppercase, odd lengths and non-hex characters
  are refused.
- Canonical padded base64 (RFC 4648 section 4) for NIP-44 payloads and NIP-98
  headers: wrong padding, non-zero trailing bits and insufficient output space
  are refused.
- A worked trace: the 32-byte key `0x79be…f81798` to hex and back.

## What exists today

- `coax/lib/crypto/bytes.coax` has endian loads and stores, checked lengths
  and constant-time equality and select.
- `coax/lib/nostr/hex.coax` and `coax/lib/nostr/base64.coax` decode and encode
  with native oracle and sanitizer checks (commit `bc88057ec1`).
- The record:
  [coax-nostr-codecs-2026-10-28.md](../../docs/verification/coax-nostr-codecs-2026-10-28.md).

## Dependencies

- [Coax chapter 5, Structs, enums and
  unions](../../coax/book/ch05-structs-enums-unions.md)
- [Coax chapter 6, Arrays, strings, memory and the
  context](../../coax/book/ch06-memory.md)

## Done means

- Every RFC 4648 test vector for base64 passes, and every non-canonical form
  refuses.
- Hex and base64 round trips hold for every length up to the profile limit in
  a `#forall` check.
- `coax mutate` leaves zero survivors on the codec files.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
