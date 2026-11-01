# 11. Verify a Schnorr signature

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A Nostr event is valid only if its `sig` is a BIP-340 Schnorr signature of its
ID by its `pubkey`. This chapter builds verification exactly as BIP-340
specifies it and checks it against the official vectors before any event code
uses it.

Part III, Elliptic curves. Result: BIP-340 verification over byte-view
messages, checked against all 19 rows of the official `test-vectors.csv`,
including messages of 0, 1, 17 and 100 bytes, with a Nostr wrapper that
requires 32-byte event IDs.

## What this chapter teaches

- The algorithm: `P = lift_x(pk)`, `r < p`, `s < n`, `e = int(hash_challenge(r
  || P || m)) mod n`, `R = s*G - e*P`, fail if `R` is infinity, `y(R)` is odd
  or `x(R) != r`.
- Each failure row in the vectors: public key not on the curve, `r` at least
  `p`, `s` at least `n`, odd `R`.
- Variable-length messages (BIP-340 accepts any length) and the Nostr wrapper
  that requires exactly 32 bytes.
- Strict input admission: exact-size lowercase hex for ID, public key and
  signature.

## What exists today

- `coax/lib/crypto/schnorr.coax` verifies; `schnorr_fast.coax` is the faster
  path with a differential check.
- The vectors are pinned in
  `coax/tests/vectors/upstream/bip340/test-vectors.csv`.
- Records:
  [coax-schnorr-verify-2026-10-28.md](../../docs/verification/coax-schnorr-verify-2026-10-28.md),
  [2026-10-29-schnorr-fast.md](../../docs/verification/2026-10-29-schnorr-fast.md).

## Dependencies

- [4. SHA-256](ch004-sha-256.md)
- [8. Scalars mod n](ch008-scalars-mod-n.md)
- [10. Scalar multiplication](ch010-scalar-multiplication.md)

## Done means

- All 19 BIP-340 vectors give their expected result.
- Fast and reference paths agree on every vector and on random signatures.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
