# 15. Bech32

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

NIP-19 writes keys and references as Bech32 strings such as `npub1…`. This
chapter builds the BIP-173 encoding: the 5-bit conversion, the BCH checksum
and the rules for case and length.

Part IV, Encodings. Result: BIP-173 Bech32 (checksum constant 1, never
Bech32m) with the pinned vectors and canonical lowercase re-encoding.

## What this chapter teaches

- The human-readable part, the separator `1` and the data part.
- Converting 8-bit bytes to 5-bit groups with padding rules.
- The polymod checksum with constant 1, and why Bech32m (constant
  `0x2bc830a3`) is refused.
- Mixed case refuses; output is always lowercase.

## What exists today

- `coax/lib/nostr/bech32-core.coax` with the BIP-173 vectors pinned in
  `coax/tests/vectors/upstream/bip173/`.
- Record:
  [coax-nostr-bech32-2026-10-28.md](../../docs/verification/coax-nostr-bech32-2026-10-28.md).

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Done means

- Every valid and invalid BIP-173 Bech32 vector gives its expected result.
- A Bech32m string refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
