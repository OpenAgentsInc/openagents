# 14. Shared secrets with ECDH

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

NIP-44 encrypts with a key two parties derive from their own private key and
the other's public key. Unlike libsecp256k1's default, NIP-44 needs the raw x
coordinate of the shared point, not its hash. This chapter builds that ECDH
and its peer validation.

Part III, Elliptic curves. Result: x-only ECDH that validates the peer key and
returns the raw shared x, checked against labelled Wycheproof secp256k1
inputs, for NIP-44 conversation keys.

## What this chapter teaches

- Lift the peer's x-only key to a point, refusing keys off the curve.
- Multiply by the private scalar on the constant-time path.
- Return the 32-byte x coordinate unhashed.
- Wycheproof cases adapted to x-only keys, each labelled with its derivation.

## What exists today

- `coax/lib/crypto/ecdh.coax`, `ecdh_fast.coax` and the owned lifecycle in
  `ecdh_lifecycle.coax`.
- Records:
  [2026-10-29-ecdh-acceptance.md](../../docs/verification/2026-10-29-ecdh-acceptance.md),
  [2026-10-29-ecdh-owned-lifecycle.md](../../docs/verification/2026-10-29-ecdh-owned-lifecycle.md).

## Dependencies

- [10. Scalar multiplication](ch010-scalar-multiplication.md)
- [12. Keys and randomness](ch012-keys-and-randomness.md)

## Done means

- The NIP-44 `get_conversation_key` vectors give the expected keys.
- Every invalid Wycheproof public key refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
